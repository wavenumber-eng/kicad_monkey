//! One-scan schematic inventory and hierarchy realization over [`SourceBundle`].

use crate::schematic_source::{
    SchematicBusEntry, SchematicConnectivity, SchematicJunction, SchematicLabel,
    SchematicNoConnect, SchematicPolyline, SchematicSheetPin, parse_sheet_pin,
    parse_source_carriers,
};
use crate::sexpr::{Lexer, Token, TokenKind, decode_quoted};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::source_bundle::{SourceBundle, SourceBundleError, SourceBundleErrorKind};
use kicad_monkey_contracts::generated::source_bundle_manifest::SourceKind;
use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};

/// Resource ceilings for schematic indexing and hierarchy realization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicBundleLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_selected_forms_per_source: usize,
    pub max_sheets_per_source: usize,
    pub max_sheet_properties: usize,
    pub max_sheet_pins_per_sheet: usize,
    pub max_decoded_string_bytes: usize,
    pub max_connectivity_objects_per_source: usize,
    pub max_wires_per_source: usize,
    pub max_buses_per_source: usize,
    pub max_bus_entries_per_source: usize,
    pub max_junctions_per_source: usize,
    pub max_no_connects_per_source: usize,
    pub max_labels_per_source: usize,
    pub max_points_per_connectivity_object: usize,
    pub max_connectivity_points_per_source: usize,
    pub max_occurrences: usize,
    pub max_path_bytes: usize,
}

impl Default for SchematicBundleLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 512 * 1024 * 1024,
            max_depth: 512,
            max_selected_forms_per_source: 4_000_000,
            max_sheets_per_source: 1_000_000,
            max_sheet_properties: 1_000_000,
            max_sheet_pins_per_sheet: 1_000_000,
            max_decoded_string_bytes: 64 * 1024 * 1024,
            max_connectivity_objects_per_source: 4_000_000,
            max_wires_per_source: 4_000_000,
            max_buses_per_source: 4_000_000,
            max_bus_entries_per_source: 4_000_000,
            max_junctions_per_source: 4_000_000,
            max_no_connects_per_source: 4_000_000,
            max_labels_per_source: 4_000_000,
            max_points_per_connectivity_object: 1_000_000,
            max_connectivity_points_per_source: 8_000_000,
            max_occurrences: 4_000_000,
            max_path_bytes: 32 * 1024,
        }
    }
}

/// One hierarchical sheet placement decoded from its parent schematic.
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

/// One schematic source definition, indexed exactly once per bundle build.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicDefinition {
    pub source_path: String,
    pub version: Option<String>,
    pub generator: Option<String>,
    pub generator_version: Option<String>,
    pub uuid: Option<String>,
    pub sheets: Vec<SchematicSheet>,
    pub wires: Vec<SchematicPolyline>,
    pub buses: Vec<SchematicPolyline>,
    pub bus_entries: Vec<SchematicBusEntry>,
    pub junctions: Vec<SchematicJunction>,
    pub no_connects: Vec<SchematicNoConnect>,
    pub labels: Vec<SchematicLabel>,
    pub connectivity: SchematicConnectivity,
}

/// One root or child occurrence realized in parent-first source order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicOccurrence {
    pub index: usize,
    pub source_path: String,
    pub parent_index: Option<usize>,
    pub sheet_uuid: Option<String>,
    pub sheet_name: String,
    pub sheet_file: String,
    pub occurrence_address: String,
    pub effective_in_bom: bool,
    pub effective_on_board: bool,
    pub effective_dnp: bool,
    pub effective_exclude_from_sim: bool,
}

/// Parsed definitions and realized occurrences ready for later connectivity work.
#[derive(Clone, Debug)]
pub struct SchematicBundleIndex {
    definitions: Vec<SchematicDefinition>,
    definition_by_path: HashMap<String, usize>,
    occurrences: Vec<SchematicOccurrence>,
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
        Ok(Self {
            definitions,
            definition_by_path,
            occurrences,
        })
    }

    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &SchematicDefinition> {
        self.definitions.iter()
    }

    pub fn definition(&self, source_path: &str) -> Option<&SchematicDefinition> {
        self.definition_by_path
            .get(source_path)
            .map(|index| &self.definitions[*index])
    }

    pub fn occurrences(&self) -> impl ExactSizeIterator<Item = &SchematicOccurrence> {
        self.occurrences.iter()
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
        wires: Vec::new(),
        buses: Vec::new(),
        bus_entries: Vec::new(),
        junctions: Vec::new(),
        no_connects: Vec::new(),
        labels: Vec::new(),
        connectivity: SchematicConnectivity::default(),
    };
    populate_schematic_definition(&mut definition, text, &spans, source.path(), limits)?;
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
        &["kicad_sch", "junction"],
        &["kicad_sch", "no_connect"],
        &["kicad_sch", "label"],
        &["kicad_sch", "global_label"],
        &["kicad_sch", "hierarchical_label"],
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
    Enter(HierarchyEnter),
    Exit(String),
}

#[derive(Clone, Debug)]
struct HierarchyEnter {
    source_path: String,
    parent_index: Option<usize>,
    sheet: Option<SchematicSheet>,
    parent_address: String,
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
    let mut work = vec![HierarchyWork::Enter(HierarchyEnter {
        source_path: bundle.root_schematic_path().to_owned(),
        parent_index: None,
        sheet: None,
        parent_address: String::new(),
        effective_in_bom: true,
        effective_on_board: true,
        effective_dnp: false,
        effective_exclude_from_sim: false,
    })];
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
                occurrences.push(occurrence);
                work.push(HierarchyWork::Exit(enter.source_path.clone()));
                for child in definition.sheets.iter().rev() {
                    let child_source = bundle.resolve_schematic(
                        &enter.source_path,
                        &child.sheet_file,
                        limits.max_path_bytes,
                    )?;
                    let child_path = child_source.path().to_owned();
                    work.push(HierarchyWork::Enter(HierarchyEnter {
                        source_path: child_path,
                        parent_index: Some(index),
                        sheet: Some(child.clone()),
                        parent_address: occurrence_address.clone(),
                        effective_in_bom: enter.effective_in_bom && child.in_bom,
                        effective_on_board: enter.effective_on_board && child.on_board,
                        effective_dnp: enter.effective_dnp || child.dnp,
                        effective_exclude_from_sim: enter.effective_exclude_from_sim
                            || child.exclude_from_sim,
                    }));
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
    let segment = enter
        .sheet
        .as_ref()
        .map(|value| {
            if value.uuid.is_empty() {
                &value.sheet_file
            } else {
                &value.uuid
            }
        })
        .or(definition.uuid.as_ref())
        .map_or("root", String::as_str);
    let occurrence_address = if enter.parent_address.is_empty() {
        format!("/{segment}")
    } else {
        format!("{}/{segment}", enter.parent_address.trim_end_matches('/'))
    };
    if occurrence_address.len() > limits.max_path_bytes {
        return Err(schematic_limit(
            &enter.source_path,
            "occurrence address exceeds max_path_bytes",
        ));
    }
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
        sheet_uuid,
        sheet_name,
        sheet_file,
        occurrence_address,
        effective_in_bom: enter.effective_in_bom,
        effective_on_board: enter.effective_on_board,
        effective_dnp: enter.effective_dnp,
        effective_exclude_from_sim: enter.effective_exclude_from_sim,
    })
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
