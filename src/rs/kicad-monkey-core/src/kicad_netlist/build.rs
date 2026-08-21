use super::glob::{GlobPattern, GlobWorkBudget};
use super::merge::merge_component_group;
use super::resource::{
    StringBudget, check_count, ensure_capacity, limit_error, project_error, schematic_error,
};
use super::variables::{ExpansionWorkBudget, VariableResolver};
use super::{
    KiCadDesignSheet, KiCadLibPart, KiCadLibPartPin, KiCadNet, KiCadNetClass, KiCadNetlist,
    KiCadNetlistComponent, KiCadNetlistComponentUnit, KiCadNetlistEndpoint,
    KiCadNetlistGraphicalIds, KiCadNetlistLimits, KiCadNetlistTerminal,
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
    let mut expansion_work = ExpansionWorkBudget::new(limits.max_variable_expansion_work_bytes);
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
    let variable_index = project_variable_index(project_variables, &mut expansion_work)?;
    let mut netlist = KiCadNetlist {
        nets: build_nets(scalar.nets, project_settings.as_ref(), limits, &mut budget)?,
        components: build_components(
            index,
            &variable_index,
            limits,
            &mut budget,
            &mut expansion_work,
        )?,
        libparts: build_libparts(index, limits, &mut budget)?,
        libraries: Vec::new(),
        net_classes: build_net_classes(project_settings.as_ref(), &mut budget)?,
        sheets: build_sheets(index, limits, &mut budget)?,
    };
    // The model is complete before publication. This also catches accidental
    // divergence between nested and aggregate limits during later expansion.
    validate_counts(&netlist, limits)?;
    netlist.nets.shrink_to_fit();
    Ok(netlist)
}

fn build_net_classes(
    settings: Option<&ProjectNetSettings>,
    budget: &mut StringBudget,
) -> Result<Vec<KiCadNetClass>, SourceBundleError> {
    let Some(settings) = settings else {
        return Ok(Vec::new());
    };
    let by_name = settings
        .classes
        .iter()
        .filter(|class| !class.name.is_empty())
        .map(|class| (class.name.as_str(), class))
        .collect::<HashMap<_, _>>();
    let mut classes = Vec::with_capacity(settings.classes.len().saturating_add(1));
    let mut seen = HashSet::with_capacity(settings.classes.len().saturating_add(1));
    for declared in &settings.classes {
        if declared.name.is_empty() || !seen.insert(declared.name.as_str()) {
            continue;
        }
        let class = by_name[declared.name.as_str()];
        budget.reserve_many([class.name.len(), class.description.len()])?;
        classes.push(KiCadNetClass {
            name: class.name.clone(),
            description: class.description.clone(),
        });
    }
    if seen.insert("Default") {
        budget.reserve("Default".len())?;
        classes.push(KiCadNetClass {
            name: "Default".to_owned(),
            description: String::new(),
        });
    }
    Ok(classes)
}

fn project_variable_index(
    variables: Vec<(String, String)>,
    work: &mut ExpansionWorkBudget,
) -> Result<HashMap<String, String>, SourceBundleError> {
    let mut index = HashMap::with_capacity(variables.len());
    for (name, value) in variables {
        index.insert(work.fold(&name)?, value);
    }
    Ok(index)
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
    let mut wildcard_work = GlobWorkBudget::new(limits.max_wildcard_match_work);
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
        let net_class = settings
            .map(|_| {
                resolve_class(
                    &net.name,
                    &class_names,
                    &exact_classes,
                    &patterns,
                    &mut wildcard_work,
                )
            })
            .transpose()?
            .unwrap_or_default();
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
            aliases: Vec::new(),
            graphical: KiCadNetlistGraphicalIds {
                wires: net.graphical.wires,
                junctions: net.graphical.junctions,
                labels: net.graphical.labels,
                power_ports: net.graphical.power_ports,
                ports: net.graphical.ports,
                sheet_entries: net.graphical.sheet_entries,
            },
            endpoints: net
                .endpoints
                .into_iter()
                .map(|endpoint| KiCadNetlistEndpoint {
                    endpoint_id: endpoint.endpoint_id,
                    role: endpoint.role,
                    element_id: endpoint.element_id,
                    object_id: endpoint.object_id,
                    name: endpoint.name,
                    source_sheet: endpoint.source_sheet,
                    connection_point: endpoint
                        .connection_point
                        .map(|point| (point.x_iu, point.y_iu)),
                })
                .collect(),
        });
    }
    Ok(result)
}

fn class_names(settings: &ProjectNetSettings) -> HashSet<&str> {
    settings
        .classes
        .iter()
        .filter(|class| !class.name.is_empty())
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
    work: &mut GlobWorkBudget,
) -> Result<String, SourceBundleError> {
    if let Some(class) = assignments.get(net).and_then(|assigned| {
        assigned
            .iter()
            .map(String::as_str)
            .find(|class| classes.contains(class))
    }) {
        return Ok(class.to_owned());
    }
    for (pattern, class) in patterns {
        if classes.contains(class) && pattern.matches(net, work)? {
            return Ok((*class).to_owned());
        }
    }
    Ok("Default".to_owned())
}

#[derive(Clone)]
pub(super) struct ComponentCandidate {
    pub(super) unit: i64,
    pub(super) order: usize,
    pub(super) component: KiCadNetlistComponent,
}

fn build_components(
    index: &SchematicBundleIndex,
    project_variables: &HashMap<String, String>,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
    expansion_work: &mut ExpansionWorkBudget,
) -> Result<Vec<KiCadNetlistComponent>, SourceBundleError> {
    let mut collection =
        ComponentCollection::new(project_variables, limits, budget, expansion_work);
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
    seen_symbol_uuids: HashSet<(usize, String)>,
}

impl<'a> ComponentCollection<'a> {
    fn new(
        project_variables: &'a HashMap<String, String>,
        limits: KiCadNetlistLimits,
        budget: &'a mut StringBudget,
        expansion_work: &'a mut ExpansionWorkBudget,
    ) -> Self {
        Self {
            materializer: ComponentMaterializer {
                project_variables,
                limits,
                budget,
                expansion_work,
            },
            limits,
            groups: Vec::new(),
            group_index: HashMap::new(),
            order: 0,
            candidate_count: 0,
            seen_symbol_uuids: HashSet::new(),
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
            if omitted_component(placed, &effective, library) {
                continue;
            }
            if !effective.uuid.is_empty()
                && !self
                    .seen_symbol_uuids
                    .insert((occurrence.index, effective.uuid.clone()))
            {
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
            let component = merge_component_group(group, self.limits, self.materializer.budget)?;
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
    placed: &SchematicPlacedSymbol,
    effective: &SchematicEffectiveSymbol,
    library: Option<&SchematicLibrarySymbol>,
) -> bool {
    !placed.on_board
        || effective.reference.starts_with('#')
        || library.is_some_and(|symbol| symbol.power)
        || effective.lib_id == "power:PWR_FLAG"
}

struct ComponentMaterializer<'a> {
    project_variables: &'a HashMap<String, String>,
    limits: KiCadNetlistLimits,
    budget: &'a mut StringBudget,
    expansion_work: &'a mut ExpansionWorkBudget,
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
        let mut variables = VariableResolver::new(
            &effective.fields,
            self.project_variables,
            self.limits.max_expanded_string_bytes,
            self.expansion_work,
        )?;
        let value = variables.expand_blank(&effective.value, "Value")?;
        let footprint = shown_field(effective, &mut variables, "Footprint")?;
        let datasheet = shown_field(effective, &mut variables, "Datasheet")?;
        let description = shown_field(effective, &mut variables, "Description")?;
        let fields = user_fields(effective, &mut variables, self.limits, self.budget)?;
        let properties = component_properties(
            occurrence,
            effective,
            library,
            parent_properties,
            &fields,
            self.limits,
            self.budget,
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
    budget: &mut StringBudget,
) -> Result<BTreeMap<String, String>, SourceBundleError> {
    let mut properties = BTreeMap::new();
    for (name, value) in fields {
        insert_property(&mut properties, name, value, limits, budget)?;
    }
    for property in parent_properties {
        if !property.key.is_empty() {
            insert_property(
                &mut properties,
                &property.key,
                &property.value,
                limits,
                budget,
            )?;
        }
    }
    let (sheet_name, sheet_file) = if occurrence.parent_index.is_none() {
        (
            portable_stem(&occurrence.source_path),
            portable_name(&occurrence.source_path),
        )
    } else {
        (
            occurrence.sheet_name.as_str(),
            occurrence.sheet_file.as_str(),
        )
    };
    insert_property(&mut properties, "Sheetname", sheet_name, limits, budget)?;
    insert_property(&mut properties, "Sheetfile", sheet_file, limits, budget)?;
    for (excluded, name) in [
        (!effective.in_bom, "exclude_from_bom"),
        (!effective.on_board, "exclude_from_board"),
        (!effective.in_pos_files, "exclude_from_pos_files"),
        (effective.dnp, "dnp"),
    ] {
        if excluded {
            insert_property(&mut properties, name, "", limits, budget)?;
        }
    }
    if let Some(library) = library {
        for name in ["ki_keywords", "ki_fp_filters"] {
            if let Some(value) = library_property(library, name)
                && !value.is_empty()
            {
                insert_property(&mut properties, name, value, limits, budget)?;
            }
        }
    }
    Ok(properties)
}

fn insert_property(
    properties: &mut BTreeMap<String, String>,
    name: &str,
    value: &str,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<(), SourceBundleError> {
    if properties.contains_key(name) {
        return Ok(());
    }
    ensure_capacity(
        properties.len(),
        limits.max_component_fields,
        "KiCad netlist component field count exceeds its limit",
    )?;
    budget.reserve_many([name.len(), value.len()])?;
    properties.insert(name.to_owned(), value.to_owned());
    Ok(())
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

fn build_libparts(
    index: &SchematicBundleIndex,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<Vec<KiCadLibPart>, SourceBundleError> {
    let mut seen = HashSet::new();
    let mut seen_definitions = HashSet::new();
    let mut result = Vec::new();
    let mut pin_count = 0usize;
    // Match the compiled-design walk: only reachable schematic occurrences
    // contribute library parts, and their first-seen order follows the
    // hierarchy rather than the caller's SourceBundle insertion order.
    for occurrence in index.occurrences() {
        if !seen_definitions.insert(occurrence.source_path.as_str()) {
            continue;
        }
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
    let mut common = Vec::new();
    let mut by_unit = vec![Vec::new(); unit_count_usize];
    for subsymbol in &symbol.subsymbols {
        if subsymbol.unit == 0 {
            common.extend(&subsymbol.pins);
        } else if let Ok(index) = usize::try_from(subsymbol.unit.saturating_sub(1))
            && let Some(pins) = by_unit.get_mut(index)
        {
            pins.extend(&subsymbol.pins);
        }
    }
    by_unit
        .into_iter()
        .enumerate()
        .map(|(index, unit_pins)| {
            let unit = i64::try_from(index)
                .map_err(|_| limit_error("KiCad netlist component unit index overflows"))?
                .saturating_add(1);
            let mut pins = common.iter().copied().chain(unit_pins).collect::<Vec<_>>();
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
    variables: &mut VariableResolver<'_, '_>,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
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
        let expanded = variables.expand_blank(value, name)?;
        budget.reserve_many([name.len(), expanded.len()])?;
        fields.insert(name.clone(), expanded);
    }
    Ok(fields)
}

fn shown_field(
    effective: &SchematicEffectiveSymbol,
    variables: &mut VariableResolver<'_, '_>,
    name: &str,
) -> Result<String, SourceBundleError> {
    effective.fields.get(name).map_or_else(
        || Ok(String::new()),
        |value| variables.expand_blank(value, name),
    )
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

fn portable_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn portable_stem(path: &str) -> &str {
    portable_name(path)
        .strip_suffix(".kicad_sch")
        .unwrap_or_else(|| portable_name(path))
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
