use super::glob::GlobPattern;
use super::resource::{
    StringBudget, check_count, ensure_capacity, limit_error, project_error, schematic_error,
};
use super::{
    KiCadDesignSheet, KiCadLibPart, KiCadLibPartPin, KiCadNet, KiCadNetlist, KiCadNetlistComponent,
    KiCadNetlistComponentUnit, KiCadNetlistLimits, KiCadNetlistTerminal,
};
use crate::{
    ProjectNetSettings, ProjectView, SchematicBundleIndex, SchematicEffectiveSymbol,
    SchematicLibrarySymbol, SchematicOccurrence, SchematicPlacedSymbol, SourceBundleError,
    build_schematic_scalar_design_nets,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn build_kicad_netlist(
    index: &SchematicBundleIndex,
    project: Option<ProjectView<'_>>,
    limits: KiCadNetlistLimits,
) -> Result<KiCadNetlist, SourceBundleError> {
    let mut budget = StringBudget::new(limits.max_retained_string_bytes);
    let mut design_limits = limits.design;
    design_limits.max_nets = design_limits.max_nets.min(limits.max_nets);
    design_limits.max_terminals = design_limits.max_terminals.min(limits.max_terminals);
    let scalar = build_schematic_scalar_design_nets(index, 1, design_limits)?;
    let project_settings = project
        .map(|view| view.net_settings())
        .transpose()
        .map_err(project_error)?;
    let project_variables = project
        .map(|view| view.text_variables())
        .transpose()
        .map_err(project_error)?
        .unwrap_or_default();
    let variable_index = project_variables
        .into_iter()
        .map(|(name, value)| (name.to_lowercase(), value))
        .collect::<HashMap<_, _>>();
    let mut netlist = KiCadNetlist {
        nets: build_nets(scalar.nets, project_settings.as_ref(), limits, &mut budget)?,
        components: build_components(index, &variable_index, limits, &mut budget)?,
        libparts: build_libparts(index, limits, &mut budget)?,
        libraries: Vec::new(),
        sheets: build_sheets(index, limits, &mut budget)?,
    };
    // The model is complete before publication. This also catches accidental
    // divergence between nested and aggregate limits during later expansion.
    validate_counts(&netlist, limits)?;
    netlist.nets.shrink_to_fit();
    Ok(netlist)
}

fn build_nets(
    nets: Vec<crate::SchematicDesignNet>,
    settings: Option<&ProjectNetSettings>,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<Vec<KiCadNet>, SourceBundleError> {
    check_count(nets.len(), limits.max_nets, "KiCad netlist net count")?;
    let class_names = settings.map(class_names).unwrap_or_default();
    let exact_classes = settings.map(exact_class_assignments).unwrap_or_default();
    let patterns = settings.map(compile_class_patterns).unwrap_or_default();
    let mut terminal_count = 0usize;
    let mut result = Vec::with_capacity(nets.len());
    for net in nets {
        terminal_count = terminal_count
            .checked_add(net.terminals.len())
            .ok_or_else(|| limit_error("KiCad netlist terminal count overflows"))?;
        check_count(
            terminal_count,
            limits.max_terminals,
            "KiCad netlist terminal count",
        )?;
        budget.reserve(net.name.len())?;
        let mut terminals = Vec::with_capacity(net.terminals.len());
        for terminal in net.terminals {
            budget.reserve_many([
                terminal.designator.len(),
                terminal.pin.len(),
                terminal.pin_name.len(),
                terminal.pin_type.len(),
                terminal.sheet_path.len(),
                terminal.source_pin_id.len(),
                terminal.svg_id.len(),
            ])?;
            terminals.push(KiCadNetlistTerminal {
                designator: terminal.designator,
                pin: terminal.pin,
                pin_name: terminal.pin_name,
                pin_type: terminal.pin_type,
                sheet_path: terminal.sheet_path,
                source_pin_id: terminal.source_pin_id,
                svg_id: terminal.svg_id,
            });
        }
        let net_class = resolve_class(&net.name, &class_names, &exact_classes, &patterns);
        let driver_kind = net
            .driver_kind
            .map_or_else(String::new, |kind| kind.as_str().to_owned());
        budget.reserve_many([net_class.len(), driver_kind.len()])?;
        result.push(KiCadNet {
            name: net.name,
            code: net.code,
            terminals,
            driver_priority: net.driver_priority as i8,
            driver_kind,
            auto_named: net.auto_named,
            net_class,
        });
    }
    Ok(result)
}

fn class_names(settings: &ProjectNetSettings) -> HashSet<&str> {
    settings
        .classes
        .iter()
        .map(|class| class.name.as_str())
        .chain(std::iter::once("Default"))
        .collect()
}

fn exact_class_assignments(settings: &ProjectNetSettings) -> HashMap<&str, &[String]> {
    settings
        .assignments
        .iter()
        .map(|(net, classes)| (net.as_str(), classes.as_slice()))
        .collect()
}

fn compile_class_patterns(settings: &ProjectNetSettings) -> Vec<(GlobPattern, &str)> {
    settings
        .patterns
        .iter()
        .filter(|pattern| !pattern.pattern.is_empty())
        .map(|pattern| {
            (
                GlobPattern::compile(&pattern.pattern),
                pattern.netclass_name.as_str(),
            )
        })
        .collect()
}

fn resolve_class(
    net: &str,
    classes: &HashSet<&str>,
    assignments: &HashMap<&str, &[String]>,
    patterns: &[(GlobPattern, &str)],
) -> String {
    assignments
        .get(net)
        .and_then(|assigned| {
            assigned
                .iter()
                .map(String::as_str)
                .find(|class| classes.contains(class))
        })
        .or_else(|| {
            patterns
                .iter()
                .find(|(pattern, class)| classes.contains(class) && pattern.matches(net))
                .map(|(_, class)| *class)
        })
        .unwrap_or("Default")
        .to_owned()
}

#[derive(Clone)]
struct ComponentCandidate {
    unit: i64,
    order: usize,
    component: KiCadNetlistComponent,
}

fn build_components(
    index: &SchematicBundleIndex,
    project_variables: &HashMap<String, String>,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<Vec<KiCadNetlistComponent>, SourceBundleError> {
    let mut collection = ComponentCollection::new(project_variables, limits, budget);
    for occurrence in index.occurrences() {
        collection.collect_occurrence(index, occurrence)?;
    }
    collection.finish()
}

struct ComponentCollection<'a> {
    materializer: ComponentMaterializer<'a>,
    limits: KiCadNetlistLimits,
    groups: Vec<Vec<ComponentCandidate>>,
    group_index: HashMap<(String, String), usize>,
    order: usize,
    candidate_count: usize,
}

impl<'a> ComponentCollection<'a> {
    fn new(
        project_variables: &'a HashMap<String, String>,
        limits: KiCadNetlistLimits,
        budget: &'a mut StringBudget,
    ) -> Self {
        Self {
            materializer: ComponentMaterializer {
                project_variables,
                limits,
                budget,
            },
            limits,
            groups: Vec::new(),
            group_index: HashMap::new(),
            order: 0,
            candidate_count: 0,
        }
    }

    fn collect_occurrence(
        &mut self,
        index: &SchematicBundleIndex,
        occurrence: &SchematicOccurrence,
    ) -> Result<(), SourceBundleError> {
        let definition = index
            .definition(&occurrence.source_path)
            .ok_or_else(|| schematic_error("KiCad netlist occurrence definition is missing"))?;
        let parent_properties = parent_sheet_properties(index, occurrence)?;
        for effective in index.effective_symbols(occurrence.index, None)? {
            let placed = definition
                .symbols
                .get(effective.symbol_index)
                .ok_or_else(|| {
                    schematic_error("KiCad netlist effective symbol index is invalid")
                })?;
            let library = definition.library_symbol_for_placement(placed);
            if omitted_component(&effective, library) {
                continue;
            }
            self.add_candidate(occurrence, placed, effective, library, parent_properties)?;
        }
        Ok(())
    }

    fn add_candidate(
        &mut self,
        occurrence: &SchematicOccurrence,
        placed: &SchematicPlacedSymbol,
        effective: SchematicEffectiveSymbol,
        library: Option<&SchematicLibrarySymbol>,
        parent_properties: &[crate::SchematicSymbolProperty],
    ) -> Result<(), SourceBundleError> {
        ensure_capacity(
            self.candidate_count,
            self.limits.max_component_candidates,
            "KiCad netlist component candidate count exceeds its limit",
        )?;
        self.candidate_count += 1;
        let component = self.materializer.materialize_symbol(
            occurrence,
            placed,
            &effective,
            library,
            parent_properties,
        )?;
        let key = (
            occurrence.legacy_address.clone(),
            effective.reference.clone(),
        );
        let group = self.group_for(key)?;
        self.groups[group].push(ComponentCandidate {
            unit: effective.unit,
            order: self.order,
            component,
        });
        self.order = self.order.saturating_add(1);
        Ok(())
    }

    fn group_for(&mut self, key: (String, String)) -> Result<usize, SourceBundleError> {
        if let Some(group) = self.group_index.get(&key) {
            return Ok(*group);
        }
        ensure_capacity(
            self.groups.len(),
            self.limits.max_components,
            "KiCad netlist component count exceeds its limit",
        )?;
        let group = self.groups.len();
        self.group_index.insert(key, group);
        self.groups.push(Vec::new());
        Ok(group)
    }

    fn finish(self) -> Result<Vec<KiCadNetlistComponent>, SourceBundleError> {
        let mut result = Vec::with_capacity(self.groups.len());
        let mut emitted_multi = HashSet::new();
        for group in self.groups {
            let component = merge_component_group(group)?;
            if component.units.len() > 1
                && !emitted_multi.insert(component.reference.to_lowercase())
            {
                continue;
            }
            result.push(component);
        }
        Ok(result)
    }
}

fn omitted_component(
    effective: &SchematicEffectiveSymbol,
    library: Option<&SchematicLibrarySymbol>,
) -> bool {
    !effective.on_board
        || effective.reference.starts_with('#')
        || library.is_some_and(|symbol| symbol.power)
        || effective.lib_id == "power:PWR_FLAG"
}

struct ComponentMaterializer<'a> {
    project_variables: &'a HashMap<String, String>,
    limits: KiCadNetlistLimits,
    budget: &'a mut StringBudget,
}

impl ComponentMaterializer<'_> {
    fn materialize_symbol(
        &mut self,
        occurrence: &SchematicOccurrence,
        placed: &SchematicPlacedSymbol,
        effective: &SchematicEffectiveSymbol,
        library: Option<&SchematicLibrarySymbol>,
        parent_properties: &[crate::SchematicSymbolProperty],
    ) -> Result<KiCadNetlistComponent, SourceBundleError> {
        let value = expand_variables(
            &blank(&effective.value),
            &effective.fields,
            self.project_variables,
            "Value",
        );
        let footprint = shown_field(effective, self.project_variables, "Footprint");
        let datasheet = shown_field(effective, self.project_variables, "Datasheet");
        let description = shown_field(effective, self.project_variables, "Description");
        let fields = user_fields(effective, self.project_variables, self.limits)?;
        let properties = component_properties(
            occurrence,
            effective,
            library,
            parent_properties,
            &fields,
            self.limits,
        )?;
        let (libsource_lib, libsource_part) = component_libsource(placed);
        let libsource_description = library
            .and_then(|symbol| library_property(symbol, "Description"))
            .unwrap_or_default()
            .to_owned();
        let units = library
            .map(|symbol| component_units(symbol, self.limits))
            .transpose()?
            .unwrap_or_default();
        check_count(
            units.len(),
            self.limits.max_component_units,
            "KiCad netlist component unit count",
        )?;
        let unit_pin_count = units.iter().map(|unit| unit.pins.len()).sum();
        check_count(
            unit_pin_count,
            self.limits.max_component_unit_pins,
            "KiCad netlist component-unit pin count",
        )?;
        for (key, item) in fields.iter().chain(properties.iter()) {
            self.budget.reserve_many([key.len(), item.len()])?;
        }
        self.budget.reserve_many([
            effective.reference.len(),
            value.len(),
            footprint.len(),
            datasheet.len(),
            description.len(),
            libsource_lib.len(),
            libsource_part.len(),
            libsource_description.len(),
            occurrence.human_address.len(),
            occurrence.legacy_address.len(),
            effective.uuid.len(),
        ])?;
        for unit in &units {
            self.budget.reserve(unit.name.len())?;
            for pin in &unit.pins {
                self.budget.reserve(pin.len())?;
            }
        }
        Ok(KiCadNetlistComponent {
            reference: effective.reference.clone(),
            value,
            footprint,
            datasheet,
            description,
            fields,
            libsource_lib,
            libsource_part,
            libsource_description,
            sheet_path_names: nonempty_owned(&occurrence.human_address, "/"),
            sheet_path_uuids: nonempty_owned(&occurrence.legacy_address, "/"),
            instance_uuids: nonempty_vec(&effective.uuid),
            properties,
            units,
            in_bom: effective.in_bom,
            on_board: effective.on_board,
            dnp: effective.dnp,
        })
    }
}

fn component_properties(
    occurrence: &SchematicOccurrence,
    effective: &SchematicEffectiveSymbol,
    library: Option<&SchematicLibrarySymbol>,
    parent_properties: &[crate::SchematicSymbolProperty],
    fields: &BTreeMap<String, String>,
    limits: KiCadNetlistLimits,
) -> Result<BTreeMap<String, String>, SourceBundleError> {
    let mut properties = fields.clone();
    for property in parent_properties {
        if !property.key.is_empty() {
            properties
                .entry(property.key.clone())
                .or_insert_with(|| property.value.clone());
        }
    }
    let (sheet_name, sheet_file) = if occurrence.parent_index.is_none() {
        (
            portable_stem(&occurrence.source_path),
            portable_name(&occurrence.source_path).to_owned(),
        )
    } else {
        (occurrence.sheet_name.clone(), occurrence.sheet_file.clone())
    };
    properties
        .entry("Sheetname".to_owned())
        .or_insert(sheet_name);
    properties
        .entry("Sheetfile".to_owned())
        .or_insert(sheet_file);
    for (excluded, name) in [
        (!effective.in_bom, "exclude_from_bom"),
        (!effective.on_board, "exclude_from_board"),
        (!effective.in_pos_files, "exclude_from_pos_files"),
        (effective.dnp, "dnp"),
    ] {
        if excluded {
            properties.entry(name.to_owned()).or_default();
        }
    }
    if let Some(library) = library {
        for name in ["ki_keywords", "ki_fp_filters"] {
            if let Some(value) = library_property(library, name)
                && !value.is_empty()
            {
                properties
                    .entry(name.to_owned())
                    .or_insert(value.to_owned());
            }
        }
    }
    check_count(
        properties.len(),
        limits.max_component_fields,
        "KiCad netlist component field count",
    )?;
    Ok(properties)
}

fn parent_sheet_properties<'a>(
    index: &'a SchematicBundleIndex,
    occurrence: &SchematicOccurrence,
) -> Result<&'a [crate::SchematicSymbolProperty], SourceBundleError> {
    let (Some(parent_index), Some(sheet_index)) =
        (occurrence.parent_index, occurrence.parent_sheet_index)
    else {
        return Ok(&[]);
    };
    let parent = index
        .occurrence(parent_index)
        .ok_or_else(|| schematic_error("KiCad netlist parent occurrence is missing"))?;
    let definition = index
        .definition(&parent.source_path)
        .ok_or_else(|| schematic_error("KiCad netlist parent definition is missing"))?;
    definition
        .sheets
        .get(sheet_index)
        .map(|sheet| sheet.properties.as_slice())
        .ok_or_else(|| schematic_error("KiCad netlist parent sheet is missing"))
}

fn merge_component_group(
    mut candidates: Vec<ComponentCandidate>,
) -> Result<KiCadNetlistComponent, SourceBundleError> {
    candidates.sort_by_key(|candidate| candidate.order);
    let primary_index = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| !candidate.component.instance_uuids.is_empty())
        .min_by_key(|(_, candidate)| &candidate.component.instance_uuids[0])
        .map_or(0, |(index, _)| index);
    let mut primary = candidates[primary_index].component.clone();
    let mut uuids = candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != primary_index)
        .rev()
        .filter_map(|(_, candidate)| candidate.component.instance_uuids.first().cloned())
        .collect::<Vec<_>>();
    if let Some(uuid) = primary.instance_uuids.first().cloned() {
        uuids.push(uuid);
    }
    uuids.dedup();
    primary.instance_uuids = uuids;
    candidates.sort_by_key(|candidate| (candidate.unit <= 0, candidate.unit, candidate.order));
    for field in ["value", "footprint", "datasheet", "description"] {
        if component_string(&primary, field).is_empty()
            && let Some(value) = candidates
                .iter()
                .map(|candidate| component_string(&candidate.component, field))
                .find(|value| !value.is_empty())
                .map(str::to_owned)
        {
            set_component_string(&mut primary, field, value);
        }
    }
    let mut fields = BTreeMap::new();
    for candidate in &candidates {
        for (name, value) in &candidate.component.fields {
            fields.entry(name.clone()).or_insert_with(|| value.clone());
        }
    }
    fields.insert("Footprint".to_owned(), primary.footprint.clone());
    fields.insert("Datasheet".to_owned(), primary.datasheet.clone());
    fields.insert("Description".to_owned(), primary.description.clone());
    primary.fields = fields;
    Ok(primary)
}

fn component_string<'a>(component: &'a KiCadNetlistComponent, field: &str) -> &'a str {
    match field {
        "value" => &component.value,
        "footprint" => &component.footprint,
        "datasheet" => &component.datasheet,
        _ => &component.description,
    }
}

fn set_component_string(component: &mut KiCadNetlistComponent, field: &str, value: String) {
    match field {
        "value" => component.value = value,
        "footprint" => component.footprint = value,
        "datasheet" => component.datasheet = value,
        _ => component.description = value,
    }
}

fn build_libparts(
    index: &SchematicBundleIndex,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<Vec<KiCadLibPart>, SourceBundleError> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    let mut pin_count = 0usize;
    // Match the compiled-design walk: only reachable schematic occurrences
    // contribute library parts, and their first-seen order follows the
    // hierarchy rather than the caller's SourceBundle insertion order.
    for occurrence in index.occurrences() {
        let definition = index
            .definition(&occurrence.source_path)
            .ok_or_else(|| schematic_error("KiCad netlist occurrence definition is missing"))?;
        for symbol in &definition.library_symbols {
            if symbol.name.is_empty() {
                continue;
            }
            let (library, part) = split_lib_id(&symbol.name);
            if !seen.insert((library, part)) {
                continue;
            }
            ensure_capacity(
                result.len(),
                limits.max_libparts,
                "KiCad netlist libpart count",
            )?;
            let pins = collect_libpart_pins(symbol, limits, budget, &mut pin_count)?;
            result.push(materialize_libpart(symbol, library, part, pins, budget)?);
        }
    }
    Ok(result)
}

fn collect_libpart_pins(
    symbol: &SchematicLibrarySymbol,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
    total: &mut usize,
) -> Result<Vec<KiCadLibPartPin>, SourceBundleError> {
    let mut pins = Vec::new();
    let mut seen = HashSet::new();
    for pin in symbol
        .subsymbols
        .iter()
        .flat_map(|subsymbol| &subsymbol.pins)
    {
        if pin.number.is_empty() || !seen.insert(pin.number.as_str()) {
            continue;
        }
        *total = total
            .checked_add(1)
            .ok_or_else(|| limit_error("KiCad netlist libpart pin count overflows"))?;
        check_count(
            *total,
            limits.max_libpart_pins,
            "KiCad netlist libpart pin count",
        )?;
        budget.reserve_many([pin.number.len(), pin.name.len(), pin.electrical_type.len()])?;
        pins.push(KiCadLibPartPin {
            number: pin.number.clone(),
            name: pin.name.clone(),
            pin_type: pin.electrical_type.clone(),
        });
    }
    pins.sort_by(|left, right| natural_pin_cmp(&left.number, &right.number));
    Ok(pins)
}

fn materialize_libpart(
    symbol: &SchematicLibrarySymbol,
    library: &str,
    part: &str,
    pins: Vec<KiCadLibPartPin>,
    budget: &mut StringBudget,
) -> Result<KiCadLibPart, SourceBundleError> {
    let description = library_property(symbol, "Description")
        .unwrap_or_default()
        .to_owned();
    let docs = library_property(symbol, "Datasheet")
        .unwrap_or_default()
        .to_owned();
    let fields = symbol
        .properties
        .iter()
        .filter(|property| is_standard_field(&property.key) && !property.value.is_empty())
        .map(|property| (property.key.clone(), property.value.clone()))
        .collect::<BTreeMap<_, _>>();
    budget.reserve_many([library.len(), part.len(), description.len(), docs.len()])?;
    for (name, value) in &fields {
        budget.reserve_many([name.len(), value.len()])?;
    }
    Ok(KiCadLibPart {
        lib: library.to_owned(),
        part: part.to_owned(),
        description,
        docs,
        footprints_filter: Vec::new(),
        fields,
        pins,
    })
}

fn build_sheets(
    index: &SchematicBundleIndex,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<Vec<KiCadDesignSheet>, SourceBundleError> {
    check_count(
        index.occurrences().len(),
        limits.max_sheets,
        "KiCad netlist sheet count",
    )?;
    index
        .occurrences()
        .enumerate()
        .map(|(position, occurrence)| {
            let definition = index
                .definition(&occurrence.source_path)
                .ok_or_else(|| schematic_error("KiCad netlist occurrence definition is missing"))?;
            let name = nonempty_owned(&occurrence.human_address, "/");
            let tstamps = nonempty_owned(&occurrence.legacy_address, "/");
            let (title, company, revision, date) =
                definition
                    .title_block
                    .as_ref()
                    .map_or(("", "", "", ""), |block| {
                        (
                            block.title.as_str(),
                            block.company.as_str(),
                            block.revision.as_str(),
                            block.date.as_str(),
                        )
                    });
            budget.reserve_many([
                name.len(),
                tstamps.len(),
                title.len(),
                company.len(),
                revision.len(),
                date.len(),
            ])?;
            Ok(KiCadDesignSheet {
                number: position + 1,
                name,
                tstamps,
                title: title.to_owned(),
                company: company.to_owned(),
                revision: revision.to_owned(),
                date: date.to_owned(),
            })
        })
        .collect()
}

fn component_units(
    symbol: &SchematicLibrarySymbol,
    limits: KiCadNetlistLimits,
) -> Result<Vec<KiCadNetlistComponentUnit>, SourceBundleError> {
    let unit_count = symbol
        .subsymbols
        .iter()
        .map(|subsymbol| subsymbol.unit)
        .max()
        .unwrap_or(1)
        .max(1);
    let unit_count_usize = usize::try_from(unit_count)
        .map_err(|_| limit_error("KiCad netlist component unit count overflows"))?;
    check_count(
        unit_count_usize,
        limits.max_component_units,
        "KiCad netlist component unit count",
    )?;
    (1..=unit_count)
        .map(|unit| {
            let mut pins = symbol
                .subsymbols
                .iter()
                .filter(|subsymbol| matches!(subsymbol.unit, 0) || subsymbol.unit == unit)
                .flat_map(|subsymbol| subsymbol.pins.iter())
                .collect::<Vec<_>>();
            pins.sort_by_key(|pin| (pin.at.x_iu, -pin.at.y_iu));
            let mut seen = HashSet::new();
            let mut numbers = Vec::new();
            for pin in pins {
                if pin.number.is_empty() {
                    continue;
                }
                let remaining = limits.max_component_unit_pins.saturating_sub(numbers.len());
                let expanded = crate::schematic_connectivity::stacked_pins::expand_stacked_pin(
                    &pin.number,
                    remaining,
                    limits.max_retained_string_bytes,
                    "",
                    "KiCad netlist component-unit pin count",
                )?;
                for number in expanded {
                    if !number.is_empty() && seen.insert(number.clone()) {
                        ensure_capacity(
                            numbers.len(),
                            limits.max_component_unit_pins,
                            "KiCad netlist component-unit pin count exceeds its limit",
                        )?;
                        numbers.push(number);
                    }
                }
            }
            Ok(KiCadNetlistComponentUnit {
                name: unit_name(unit),
                pins: numbers,
            })
        })
        .collect()
}

fn user_fields(
    effective: &SchematicEffectiveSymbol,
    project_variables: &HashMap<String, String>,
    limits: KiCadNetlistLimits,
) -> Result<BTreeMap<String, String>, SourceBundleError> {
    let mut fields = BTreeMap::new();
    for (name, value) in &effective.fields {
        if is_standard_field(name) {
            continue;
        }
        ensure_capacity(
            fields.len(),
            limits.max_component_fields,
            "KiCad netlist component field count",
        )?;
        fields.insert(
            name.clone(),
            expand_variables(&blank(value), &effective.fields, project_variables, name),
        );
    }
    Ok(fields)
}

fn shown_field(
    effective: &SchematicEffectiveSymbol,
    project_variables: &HashMap<String, String>,
    name: &str,
) -> String {
    effective
        .fields
        .get(name)
        .map_or_else(String::new, |value| {
            expand_variables(&blank(value), &effective.fields, project_variables, name)
        })
}

fn expand_variables(
    text: &str,
    fields: &BTreeMap<String, String>,
    project_variables: &HashMap<String, String>,
    skip: &str,
) -> String {
    let mut result = text.to_owned();
    for _ in 0..10 {
        let next = expand_variables_once(&result, fields, project_variables, skip);
        if next == result {
            break;
        }
        result = next;
    }
    result
}

fn expand_variables_once(
    text: &str,
    fields: &BTreeMap<String, String>,
    project_variables: &HashMap<String, String>,
    skip: &str,
) -> String {
    let mut output = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find("${") {
        output.push_str(&remaining[..start]);
        let Some(end) = remaining[start + 2..].find('}') else {
            output.push_str(&remaining[start..]);
            return output;
        };
        let close = start + 2 + end;
        let name = remaining[start + 2..close].trim();
        let replacement = (!name.eq_ignore_ascii_case(skip))
            .then(|| {
                fields
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(name))
                    .map(|(_, value)| blank(value))
                    .or_else(|| project_variables.get(&name.to_lowercase()).cloned())
            })
            .flatten();
        if let Some(replacement) = replacement {
            output.push_str(&replacement);
        } else {
            output.push_str(&remaining[start..=close]);
        }
        remaining = &remaining[close + 1..];
    }
    output.push_str(remaining);
    output
}

fn validate_counts(
    netlist: &KiCadNetlist,
    limits: KiCadNetlistLimits,
) -> Result<(), SourceBundleError> {
    check_count(
        netlist.nets.len(),
        limits.max_nets,
        "KiCad netlist net count",
    )?;
    check_count(
        netlist.components.len(),
        limits.max_components,
        "KiCad netlist component count",
    )?;
    check_count(
        netlist.libparts.len(),
        limits.max_libparts,
        "KiCad netlist libpart count",
    )?;
    check_count(
        netlist.sheets.len(),
        limits.max_sheets,
        "KiCad netlist sheet count",
    )
}

fn component_libsource(symbol: &SchematicPlacedSymbol) -> (String, String) {
    if symbol.lib_name.is_empty() {
        let (library, part) = split_lib_id(&symbol.lib_id);
        (library.to_owned(), part.to_owned())
    } else {
        (String::new(), symbol.lib_name.clone())
    }
}

fn library_property<'a>(symbol: &'a SchematicLibrarySymbol, name: &str) -> Option<&'a str> {
    symbol
        .properties
        .iter()
        .find(|property| property.key == name)
        .map(|property| property.value.as_str())
}

fn split_lib_id(value: &str) -> (&str, &str) {
    value.split_once(':').unwrap_or(("", value))
}

fn natural_pin_cmp(left: &str, right: &str) -> Ordering {
    match (left.parse::<i128>(), right.parse::<i128>()) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => left.cmp(right),
    }
}

fn unit_name(unit: i64) -> String {
    u32::try_from(unit.saturating_sub(1))
        .ok()
        .and_then(|offset| char::from_u32(u32::from(b'A').saturating_add(offset)))
        .map_or_else(|| unit.to_string(), |value| value.to_string())
}

fn is_standard_field(value: &str) -> bool {
    matches!(
        value,
        "Reference" | "Value" | "Footprint" | "Datasheet" | "Description"
    )
}

fn blank(value: &str) -> String {
    if value == "~" {
        String::new()
    } else {
        value.to_owned()
    }
}

fn portable_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn portable_stem(path: &str) -> String {
    portable_name(path)
        .strip_suffix(".kicad_sch")
        .unwrap_or_else(|| portable_name(path))
        .to_owned()
}

fn nonempty_owned(value: &str, fallback: &str) -> String {
    if value.is_empty() {
        fallback.to_owned()
    } else {
        value.to_owned()
    }
}

fn nonempty_vec(value: &str) -> Vec<String> {
    if value.is_empty() {
        Vec::new()
    } else {
        vec![value.to_owned()]
    }
}
