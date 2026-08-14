use crate::schematic_bundle_indexes::{
    index_bundle_legacy_instances, index_library_symbols, portable_file_stem,
    resolve_library_pin_owners,
};
use crate::schematic_effective::{SchematicEffectiveSymbol, resolve_effective_symbols};
use crate::schematic_source::{
    SchematicBusAlias, SchematicBusEntry, SchematicConnectivity, SchematicJunction, SchematicLabel,
    SchematicLegacySymbolInstance, SchematicLibrarySymbol, SchematicNoConnect,
    SchematicPlacedSymbol, SchematicPolyline, SchematicSheetPin, parse_bus_aliases,
    parse_embedded_library_symbols, parse_legacy_symbol_instances, parse_placed_symbols,
    parse_sheet_pin, parse_source_carriers,
};
use crate::schematic_terminals::{SchematicSymbolTerminal, resolve_symbol_terminals};
use crate::sexpr::{Lexer, Token, TokenKind, decode_quoted};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::source_bundle::{SourceBundle, SourceBundleError, SourceBundleErrorKind};
use kicad_monkey_contracts::generated::source_bundle_manifest::SourceKind;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};

mod path_helpers;
use path_helpers::{join_occurrence_path, nonempty_or, portable_file_name, portable_parent};

pub use crate::schematic_bundle_limits::SchematicBundleLimits;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSheet {
    pub uuid: String,
    pub sheet_name: String,
    pub sheet_file: String,
    pub in_bom: bool,
    pub on_board: bool,
    pub dnp: bool,
    pub exclude_from_sim: bool,
    pub pins: Vec<SchematicSheetPin>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicDefinition {
    pub source_path: String,
    pub version: Option<String>,
    pub generator: Option<String>,
    pub generator_version: Option<String>,
    pub uuid: Option<String>,
    pub sheets: Vec<SchematicSheet>,
    pub symbols: Vec<SchematicPlacedSymbol>,
    pub library_symbols: Vec<SchematicLibrarySymbol>,
    pub legacy_symbol_instances: Vec<SchematicLegacySymbolInstance>,
    pub wires: Vec<SchematicPolyline>,
    pub buses: Vec<SchematicPolyline>,
    pub bus_entries: Vec<SchematicBusEntry>,
    pub bus_aliases: Vec<SchematicBusAlias>,
    pub junctions: Vec<SchematicJunction>,
    pub no_connects: Vec<SchematicNoConnect>,
    pub labels: Vec<SchematicLabel>,
    pub connectivity: SchematicConnectivity,
    legacy_symbol_instance_by_path: HashMap<String, usize>,
    pub(crate) library_symbol_by_key: HashMap<String, usize>,
    pub(crate) library_pin_owner_by_symbol: Vec<Option<usize>>,
}

impl SchematicDefinition {
    pub fn legacy_symbol_instance(&self, path: &str) -> Option<&SchematicLegacySymbolInstance> {
        self.legacy_symbol_instance_by_path
            .get(path.trim_end_matches('/'))
            .map(|index| &self.legacy_symbol_instances[*index])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicOccurrence {
    pub index: usize,
    pub source_path: String,
    pub parent_index: Option<usize>,
    pub parent_sheet_index: Option<usize>,
    pub sheet_uuid: Option<String>,
    pub sheet_name: String,
    pub sheet_file: String,
    pub occurrence_address: String,
    pub legacy_address: String,
    pub human_address: String,
    pub effective_in_bom: bool,
    pub effective_on_board: bool,
    pub effective_dnp: bool,
    pub effective_exclude_from_sim: bool,
}

#[derive(Clone, Debug)]
pub struct SchematicBundleIndex {
    limits: SchematicBundleLimits,
    project_name: String,
    project_file: String,
    source_anchor: String,
    subpart_settings: crate::SchematicSubpartSettings,
    definitions: Vec<SchematicDefinition>,
    definition_by_path: HashMap<String, usize>,
    occurrences: Vec<SchematicOccurrence>,
    legacy_symbol_instance_by_path: HashMap<String, (usize, usize)>,
}

#[derive(Clone, Debug, Default)]
struct SheetParseState {
    property_count: usize,
    uuid_seen: bool,
    sheet_name_seen: bool,
    sheet_file_seen: bool,
    in_bom_seen: bool,
    on_board_seen: bool,
    dnp_seen: bool,
    exclude_from_sim_seen: bool,
}

impl SchematicBundleIndex {
    /// Scan every schematic source once, then realize the reachable hierarchy.
    pub fn build(
        bundle: &SourceBundle,
        limits: SchematicBundleLimits,
    ) -> Result<Self, SourceBundleError> {
        let mut definitions = Vec::new();
        let mut definition_by_path = HashMap::new();
        let root_path = bundle.root_schematic_path();
        let root = parse_schematic_definition(bundle.root_schematic(), limits)?;
        definition_by_path.insert(root.source_path.clone(), 0);
        definitions.push(root);
        for source in bundle.sources() {
            if source.kind() != SourceKind::Schematic || source.path() == root_path {
                continue;
            }
            let definition = parse_schematic_definition(source, limits)?;
            definition_by_path.insert(definition.source_path.clone(), definitions.len());
            definitions.push(definition);
        }
        let occurrences = realize_occurrences(bundle, &definitions, &definition_by_path, limits)?;
        let legacy_symbol_instance_by_path = index_bundle_legacy_instances(&definitions);
        let identity_path = bundle
            .project_path()
            .unwrap_or_else(|| bundle.root_schematic_path());
        Ok(Self {
            limits,
            project_name: bundle
                .project_path()
                .map_or_else(String::new, portable_file_stem),
            project_file: portable_file_name(identity_path).to_owned(),
            source_anchor: portable_parent(identity_path).to_owned(),
            subpart_settings: crate::schematic_project::project_subpart_settings(bundle.project())?,
            definitions,
            definition_by_path,
            occurrences,
            legacy_symbol_instance_by_path,
        })
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &SchematicDefinition> {
        self.definitions.iter()
    }

    pub fn subpart_settings(&self) -> crate::SchematicSubpartSettings {
        self.subpart_settings
    }

    pub fn definition(&self, source_path: &str) -> Option<&SchematicDefinition> {
        self.definition_by_path
            .get(source_path)
            .map(|index| &self.definitions[*index])
    }

    pub fn occurrences(&self) -> impl ExactSizeIterator<Item = &SchematicOccurrence> {
        self.occurrences.iter()
    }

    pub fn occurrence(&self, occurrence_index: usize) -> Option<&SchematicOccurrence> {
        occurrence_index
            .checked_sub(1)
            .and_then(|index| self.occurrences.get(index))
            .filter(|occurrence| occurrence.index == occurrence_index)
    }

    pub fn project_name(&self) -> &str {
        &self.project_name
    }

    /// Portable filename used by the compiled-graph identity scope.
    pub fn project_file(&self) -> &str {
        &self.project_file
    }

    /// Return a source path relative to the project/top-schematic directory.
    pub fn portable_source_path<'a>(&self, source_path: &'a str) -> &'a str {
        if self.source_anchor.is_empty() {
            return source_path;
        }
        source_path
            .strip_prefix(&self.source_anchor)
            .and_then(|value| value.strip_prefix('/'))
            .unwrap_or(source_path)
    }

    pub fn effective_symbols(
        &self,
        occurrence_index: usize,
        variant_name: Option<&str>,
    ) -> Result<Vec<SchematicEffectiveSymbol>, SourceBundleError> {
        let occurrence = occurrence_index
            .checked_sub(1)
            .and_then(|index| self.occurrences.get(index))
            .filter(|occurrence| occurrence.index == occurrence_index)
            .ok_or_else(|| {
                SourceBundleError::new(
                    SourceBundleErrorKind::Schematic,
                    None,
                    "schematic occurrence index is out of range",
                )
            })?;
        let definition = self.definition(&occurrence.source_path).ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::MissingSource,
                Some(&occurrence.source_path),
                "schematic occurrence definition is missing",
            )
        })?;
        resolve_effective_symbols(
            definition,
            occurrence,
            &self.project_name,
            variant_name,
            &self.definitions,
            &self.legacy_symbol_instance_by_path,
        )
    }

    pub fn symbol_terminals(
        &self,
        occurrence_index: usize,
    ) -> Result<Vec<SchematicSymbolTerminal>, SourceBundleError> {
        let occurrence = occurrence_index
            .checked_sub(1)
            .and_then(|index| self.occurrences.get(index))
            .filter(|occurrence| occurrence.index == occurrence_index)
            .ok_or_else(|| {
                SourceBundleError::new(
                    SourceBundleErrorKind::Schematic,
                    None,
                    "schematic occurrence index is out of range",
                )
            })?;
        let definition = self.definition(&occurrence.source_path).ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::MissingSource,
                Some(&occurrence.source_path),
                "schematic occurrence definition is missing",
            )
        })?;
        let effective = self.effective_symbols(occurrence_index, None)?;
        resolve_symbol_terminals(definition, occurrence, &effective, self.limits)
    }
}

fn parse_schematic_definition(
    source: &crate::SourceFile,
    limits: SchematicBundleLimits,
) -> Result<SchematicDefinition, SourceBundleError> {
    let text = source.text()?;
    let spans = scan_form_spans_with_limits(
        text,
        &schematic_selector(),
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: limits.max_selected_forms_per_source,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| schematic_error(source.path(), error))?;
    let roots = spans
        .iter()
        .filter(|span| span.depth == 0)
        .collect::<Vec<_>>();
    if roots.len() != 1 || roots[0].head.as_deref() != Some("kicad_sch") {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source.path()),
            "schematic source must contain exactly one kicad_sch root",
        ));
    }

    let mut definition = SchematicDefinition {
        source_path: source.path().to_owned(),
        version: None,
        generator: None,
        generator_version: None,
        uuid: None,
        sheets: Vec::new(),
        symbols: Vec::new(),
        library_symbols: Vec::new(),
        legacy_symbol_instances: Vec::new(),
        wires: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        bus_aliases: Vec::new(),
        junctions: Vec::new(),
        no_connects: Vec::new(),
        labels: Vec::new(),
        connectivity: SchematicConnectivity::default(),
        legacy_symbol_instance_by_path: HashMap::new(),
        library_symbol_by_key: HashMap::new(),
        library_pin_owner_by_symbol: Vec::new(),
    };
    populate_schematic_definition(&mut definition, text, &spans, source.path(), limits)?;
    definition.symbols = parse_placed_symbols(text, source.path(), &spans, limits)?;
    definition.library_symbols =
        parse_embedded_library_symbols(text, source.path(), &spans, limits)?;
    definition.library_symbol_by_key = index_library_symbols(
        &definition.library_symbols,
        source.path(),
        limits.max_library_lookup_key_bytes_per_source,
    )?;
    definition.library_pin_owner_by_symbol = resolve_library_pin_owners(
        &definition.library_symbols,
        &definition.library_symbol_by_key,
    );
    definition.bus_aliases = parse_bus_aliases(text, source.path(), &spans, limits)?;
    definition.legacy_symbol_instances =
        parse_legacy_symbol_instances(text, source.path(), &spans, limits)?;
    for (index, instance) in definition.legacy_symbol_instances.iter().enumerate() {
        let path = instance.path.trim_end_matches('/');
        if !path.is_empty() {
            definition
                .legacy_symbol_instance_by_path
                .entry(path.to_owned())
                .or_insert(index);
        }
    }
    let carriers = parse_source_carriers(text, source.path(), &spans, limits)?;
    definition.wires = carriers.wires;
    definition.buses = carriers.buses;
    definition.bus_entries = carriers.bus_entries;
    definition.junctions = carriers.junctions;
    definition.no_connects = carriers.no_connects;
    definition.labels = carriers.labels;
    definition.connectivity = carriers.connectivity;
    if definition
        .sheets
        .iter()
        .any(|sheet| sheet.sheet_file.is_empty())
    {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source.path()),
            "hierarchical sheet is missing Sheetfile",
        ));
    }
    Ok(definition)
}

fn populate_schematic_definition(
    definition: &mut SchematicDefinition,
    text: &str,
    spans: &[FormSpan],
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    let mut current_sheet: Option<usize> = None;
    let mut sheet_states = Vec::new();
    for span in spans {
        match (span.depth, span.head.as_deref()) {
            (1, Some("version")) => {
                set_once_scalar(&mut definition.version, text, span, source_path, limits)?
            }
            (1, Some("generator")) => {
                set_once_scalar(&mut definition.generator, text, span, source_path, limits)?
            }
            (1, Some("generator_version")) => set_once_scalar(
                &mut definition.generator_version,
                text,
                span,
                source_path,
                limits,
            )?,
            (1, Some("uuid")) => {
                set_once_scalar(&mut definition.uuid, text, span, source_path, limits)?
            }
            (1, Some("sheet")) => {
                if definition.sheets.len() >= limits.max_sheets_per_source {
                    return Err(schematic_limit(
                        source_path,
                        "sheet count exceeds its limit",
                    ));
                }
                definition.sheets.push(default_sheet());
                sheet_states.push(SheetParseState::default());
                current_sheet = Some(definition.sheets.len() - 1);
            }
            (2, head) => {
                let Some(index) = current_sheet else {
                    return Err(SourceBundleError::new(
                        SourceBundleErrorKind::Schematic,
                        Some(source_path),
                        "selected sheet child has no owning sheet",
                    ));
                };
                parse_sheet_child(
                    &mut definition.sheets[index],
                    &mut sheet_states[index],
                    head,
                    text,
                    span,
                    source_path,
                    limits,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn default_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: String::new(),
        sheet_name: String::new(),
        sheet_file: String::new(),
        in_bom: true,
        on_board: true,
        dnp: false,
        exclude_from_sim: false,
        pins: Vec::new(),
    }
}

fn parse_sheet_child(
    sheet: &mut SchematicSheet,
    state: &mut SheetParseState,
    head: Option<&str>,
    text: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    match head {
        Some("uuid") => {
            if state.uuid_seen {
                return Err(SourceBundleError::new(
                    SourceBundleErrorKind::Schematic,
                    Some(source_path),
                    "duplicate sheet uuid form",
                ));
            }
            state.uuid_seen = true;
            set_once_required_string(&mut sheet.uuid, text, span, source_path, limits)?;
        }
        Some("property") => parse_sheet_property(sheet, state, text, span, source_path, limits)?,
        Some("pin") => {
            if sheet.pins.len() >= limits.max_sheet_pins_per_sheet {
                return Err(schematic_limit(
                    source_path,
                    "sheet pin count exceeds its limit",
                ));
            }
            sheet
                .pins
                .push(parse_sheet_pin(text, span, source_path, limits)?);
        }
        Some(head) => parse_sheet_flag(sheet, state, head, text, span, source_path, limits)?,
        None => {}
    }
    Ok(())
}

fn parse_sheet_property(
    sheet: &mut SchematicSheet,
    state: &mut SheetParseState,
    text: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    state.property_count = state.property_count.saturating_add(1);
    if state.property_count > limits.max_sheet_properties {
        return Err(schematic_limit(
            source_path,
            "sheet property count exceeds its limit",
        ));
    }
    let values = direct_scalar_strings(text, span, 2, source_path, limits)?;
    if values.len() < 2 {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            "sheet property requires a name and value",
        ));
    }
    match values[0].as_str() {
        "Sheetname" | "Sheet name" if !state.sheet_name_seen => {
            state.sheet_name_seen = true;
            sheet.sheet_name.clone_from(&values[1]);
        }
        "Sheetfile" | "Sheet file" if !state.sheet_file_seen => {
            state.sheet_file_seen = true;
            sheet.sheet_file.clone_from(&values[1]);
        }
        _ => {}
    }
    Ok(())
}

fn parse_sheet_flag(
    sheet: &mut SchematicSheet,
    state: &mut SheetParseState,
    head: &str,
    text: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    match head {
        "in_bom" => set_first_boolean(
            &mut state.in_bom_seen,
            &mut sheet.in_bom,
            text,
            span,
            source_path,
            limits,
        ),
        "on_board" => set_first_boolean(
            &mut state.on_board_seen,
            &mut sheet.on_board,
            text,
            span,
            source_path,
            limits,
        ),
        "dnp" => set_first_boolean(
            &mut state.dnp_seen,
            &mut sheet.dnp,
            text,
            span,
            source_path,
            limits,
        ),
        "exclude_from_sim" => set_first_boolean(
            &mut state.exclude_from_sim_seen,
            &mut sheet.exclude_from_sim,
            text,
            span,
            source_path,
            limits,
        ),
        _ => Ok(()),
    }
}

fn set_first_boolean(
    seen: &mut bool,
    target: &mut bool,
    text: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    if !*seen {
        *seen = true;
        *target = direct_yes_no(text, span, source_path, limits)?;
    }
    Ok(())
}

fn set_once_scalar(
    target: &mut Option<String>,
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    if target.is_some() {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            format!(
                "duplicate {} form",
                span.head.as_deref().unwrap_or("scalar")
            ),
        ));
    }
    let values = direct_scalar_strings(source, span, 1, source_path, limits)?;
    let value = values.into_iter().next().ok_or_else(|| {
        SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            format!(
                "{} form is missing its value",
                span.head.as_deref().unwrap_or("scalar")
            ),
        )
    })?;
    *target = Some(value);
    Ok(())
}

fn set_once_required_string(
    target: &mut String,
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    if !target.is_empty() {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            format!(
                "duplicate {} form",
                span.head.as_deref().unwrap_or("scalar")
            ),
        ));
    }
    let values = direct_scalar_strings(source, span, 1, source_path, limits)?;
    *target = values.into_iter().next().ok_or_else(|| {
        SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            format!(
                "{} form is missing its value",
                span.head.as_deref().unwrap_or("scalar")
            ),
        )
    })?;
    Ok(())
}

fn schematic_selector() -> Selector {
    let paths = [
        &["kicad_sch"][..],
        &["kicad_sch", "version"],
        &["kicad_sch", "generator"],
        &["kicad_sch", "generator_version"],
        &["kicad_sch", "uuid"],
        &["kicad_sch", "sheet"],
        &["kicad_sch", "sheet", "uuid"],
        &["kicad_sch", "sheet", "property"],
        &["kicad_sch", "sheet", "in_bom"],
        &["kicad_sch", "sheet", "on_board"],
        &["kicad_sch", "sheet", "dnp"],
        &["kicad_sch", "sheet", "exclude_from_sim"],
        &["kicad_sch", "sheet", "pin"],
        &["kicad_sch", "wire"],
        &["kicad_sch", "bus"],
        &["kicad_sch", "bus_entry"],
        &["kicad_sch", "bus_alias"],
        &["kicad_sch", "junction"],
        &["kicad_sch", "no_connect"],
        &["kicad_sch", "label"],
        &["kicad_sch", "global_label"],
        &["kicad_sch", "hierarchical_label"],
        &["kicad_sch", "symbol"],
        &["kicad_sch", "lib_symbols"],
        &["kicad_sch", "symbol_instances"],
    ]
    .into_iter()
    .map(|path| path.iter().map(|part| (*part).to_owned()).collect())
    .collect::<BTreeSet<Vec<String>>>();
    Selector {
        paths: Some(paths),
        min_depth: Some(0),
        max_depth: Some(2),
        ..Selector::default()
    }
}

fn direct_yes_no(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    match direct_scalar_strings(source, span, 1, source_path, limits)?
        .first()
        .map(String::as_str)
    {
        Some("yes") => Ok(true),
        Some("no") => Ok(false),
        _ => Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            "schematic boolean requires yes or no",
        )),
    }
}

fn direct_scalar_strings(
    source: &str,
    span: &FormSpan,
    maximum: usize,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<String>, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| schematic_error(source_path, error))?;
    let mut lexer = Lexer::new(text);
    let left = next_token(&mut lexer, source_path)?;
    let head = next_token(&mut lexer, source_path)?;
    if left.kind != TokenKind::Left || head.kind != TokenKind::Atom {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Schematic,
            Some(source_path),
            "selected form has an invalid header",
        ));
    }
    let mut depth = 1_usize;
    let mut values = Vec::new();
    for token in lexer {
        let token = token.map_err(|error| schematic_error(source_path, error))?;
        if collect_direct_scalar(token, &mut depth, &mut values, maximum, source_path, limits)? {
            break;
        }
    }
    Ok(values)
}

fn collect_direct_scalar(
    token: Token<'_>,
    depth: &mut usize,
    values: &mut Vec<String>,
    maximum: usize,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    match token.kind {
        TokenKind::Left => *depth = depth.saturating_add(1),
        TokenKind::Right => {
            *depth = depth.saturating_sub(1);
            return Ok(*depth == 0);
        }
        _ if *depth == 1 => push_direct_scalar(token, values, maximum, source_path, limits)?,
        _ => {}
    }
    Ok(false)
}

fn push_direct_scalar(
    token: Token<'_>,
    values: &mut Vec<String>,
    maximum: usize,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    if values.len() >= maximum {
        return Err(schematic_limit(
            source_path,
            "direct scalar count exceeds its limit",
        ));
    }
    if token.lexeme.len() > limits.max_decoded_string_bytes.saturating_add(2) {
        return Err(schematic_limit(
            source_path,
            "decoded string exceeds its limit",
        ));
    }
    let value = decoded(token);
    if value.len() > limits.max_decoded_string_bytes {
        return Err(schematic_limit(
            source_path,
            "decoded string exceeds its limit",
        ));
    }
    values.push(value.into_owned());
    Ok(())
}

fn next_token<'a>(
    lexer: &mut Lexer<'a>,
    source_path: &str,
) -> Result<Token<'a>, SourceBundleError> {
    lexer
        .next()
        .transpose()
        .map_err(|error| schematic_error(source_path, error))?
        .ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::Schematic,
                Some(source_path),
                "selected form ended before its header",
            )
        })
}

fn decoded(token: Token<'_>) -> Cow<'_, str> {
    if token.kind == TokenKind::QuotedString {
        Cow::Owned(decode_quoted(token.lexeme))
    } else {
        Cow::Borrowed(token.lexeme)
    }
}

#[derive(Clone, Debug)]
enum HierarchyWork {
    Enter(Box<HierarchyEnter>),
    Exit(String),
}

#[derive(Clone, Debug)]
struct HierarchyEnter {
    source_path: String,
    parent_index: Option<usize>,
    parent_sheet_index: Option<usize>,
    sheet: Option<SchematicSheet>,
    parent_address: String,
    parent_legacy_address: String,
    parent_human_address: String,
    effective_in_bom: bool,
    effective_on_board: bool,
    effective_dnp: bool,
    effective_exclude_from_sim: bool,
}

fn realize_occurrences(
    bundle: &SourceBundle,
    definitions: &[SchematicDefinition],
    definition_by_path: &HashMap<String, usize>,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicOccurrence>, SourceBundleError> {
    let mut occurrences = Vec::new();
    let mut active_sources = HashSet::new();
    let mut work = vec![HierarchyWork::Enter(Box::new(HierarchyEnter {
        source_path: bundle.root_schematic_path().to_owned(),
        parent_index: None,
        parent_sheet_index: None,
        sheet: None,
        parent_address: String::new(),
        parent_legacy_address: String::new(),
        parent_human_address: String::new(),
        effective_in_bom: true,
        effective_on_board: true,
        effective_dnp: false,
        effective_exclude_from_sim: false,
    }))];
    while let Some(item) = work.pop() {
        match item {
            HierarchyWork::Exit(source_path) => {
                active_sources.remove(&source_path);
            }
            HierarchyWork::Enter(enter) => {
                if !active_sources.insert(enter.source_path.clone()) {
                    return Err(SourceBundleError::new(
                        SourceBundleErrorKind::HierarchyCycle,
                        Some(&enter.source_path),
                        "schematic hierarchy contains a source cycle",
                    ));
                }
                if occurrences.len() >= limits.max_occurrences {
                    return Err(schematic_limit(
                        &enter.source_path,
                        "hierarchy occurrence count exceeds its limit",
                    ));
                }
                let definition_index =
                    *definition_by_path.get(&enter.source_path).ok_or_else(|| {
                        SourceBundleError::new(
                            SourceBundleErrorKind::MissingSource,
                            Some(&enter.source_path),
                            "resolved schematic was not indexed",
                        )
                    })?;
                let definition = &definitions[definition_index];
                let index = occurrences.len() + 1;
                let occurrence = materialize_occurrence(&enter, definition, index, limits)?;
                let occurrence_address = occurrence.occurrence_address.clone();
                let legacy_address = occurrence.legacy_address.clone();
                let human_address = occurrence.human_address.clone();
                occurrences.push(occurrence);
                work.push(HierarchyWork::Exit(enter.source_path.clone()));
                for (child_index, child) in definition.sheets.iter().enumerate().rev() {
                    let child_source = bundle.resolve_schematic(
                        &enter.source_path,
                        &child.sheet_file,
                        limits.max_path_bytes,
                    )?;
                    let child_path = child_source.path().to_owned();
                    work.push(HierarchyWork::Enter(Box::new(HierarchyEnter {
                        source_path: child_path,
                        parent_index: Some(index),
                        parent_sheet_index: Some(child_index),
                        sheet: Some(child.clone()),
                        parent_address: occurrence_address.clone(),
                        parent_legacy_address: legacy_address.clone(),
                        parent_human_address: human_address.clone(),
                        effective_in_bom: enter.effective_in_bom && child.in_bom,
                        effective_on_board: enter.effective_on_board && child.on_board,
                        effective_dnp: enter.effective_dnp || child.dnp,
                        effective_exclude_from_sim: enter.effective_exclude_from_sim
                            || child.exclude_from_sim,
                    })));
                }
            }
        }
    }
    Ok(occurrences)
}

fn materialize_occurrence(
    enter: &HierarchyEnter,
    definition: &SchematicDefinition,
    index: usize,
    limits: SchematicBundleLimits,
) -> Result<SchematicOccurrence, SourceBundleError> {
    let (occurrence_address, legacy_address, human_address) =
        occurrence_paths(enter, definition, limits)?;
    let (sheet_uuid, sheet_name, sheet_file) = enter.sheet.as_ref().map_or_else(
        || (None, "root".to_owned(), String::new()),
        |value| {
            (
                (!value.uuid.is_empty()).then(|| value.uuid.clone()),
                if value.sheet_name.is_empty() {
                    value.sheet_file.clone()
                } else {
                    value.sheet_name.clone()
                },
                value.sheet_file.clone(),
            )
        },
    );
    Ok(SchematicOccurrence {
        index,
        source_path: enter.source_path.clone(),
        parent_index: enter.parent_index,
        parent_sheet_index: enter.parent_sheet_index,
        sheet_uuid,
        sheet_name,
        sheet_file,
        occurrence_address,
        legacy_address,
        human_address,
        effective_in_bom: enter.effective_in_bom,
        effective_on_board: enter.effective_on_board,
        effective_dnp: enter.effective_dnp,
        effective_exclude_from_sim: enter.effective_exclude_from_sim,
    })
}

fn occurrence_paths(
    enter: &HierarchyEnter,
    definition: &SchematicDefinition,
    limits: SchematicBundleLimits,
) -> Result<(String, String, String), SourceBundleError> {
    let uuid_segment = enter
        .sheet
        .as_ref()
        .map(|sheet| nonempty_or(&sheet.uuid, &sheet.sheet_file))
        .or(definition.uuid.as_deref())
        .unwrap_or("root");
    let occurrence = join_occurrence_path(&enter.parent_address, uuid_segment, false);
    let legacy = enter.sheet.as_ref().map_or_else(
        || "/".to_owned(),
        |_| join_occurrence_path(&enter.parent_legacy_address, uuid_segment, true),
    );
    let human = enter.sheet.as_ref().map_or_else(
        || "/".to_owned(),
        |sheet| {
            join_occurrence_path(
                &enter.parent_human_address,
                nonempty_or(&sheet.sheet_name, &sheet.sheet_file),
                true,
            )
        },
    );
    if [occurrence.len(), legacy.len(), human.len()]
        .into_iter()
        .any(|bytes| bytes > limits.max_path_bytes)
    {
        return Err(schematic_limit(
            &enter.source_path,
            "schematic occurrence path exceeds max_path_bytes",
        ));
    }
    Ok((occurrence, legacy, human))
}

fn schematic_error(path: &str, error: crate::Error) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Schematic,
        Some(path),
        error.to_string(),
    )
}

fn schematic_limit(path: &str, message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, Some(path), message)
}
