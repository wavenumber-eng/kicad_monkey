use super::build::ComponentCandidate;
use super::resource::{StringBudget, ensure_capacity, schematic_error};
use super::{KiCadNetlistComponent, KiCadNetlistLimits};
use crate::SourceBundleError;
use std::collections::{BTreeMap, HashSet};

pub(super) fn merge_component_group(
    mut candidates: Vec<ComponentCandidate>,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<KiCadNetlistComponent, SourceBundleError> {
    candidates.sort_by_key(|candidate| candidate.order);
    let (primary_index, extra_indices) = select_primary(&candidates)?;
    let primary_order = candidates[primary_index].order;
    let uuids = collect_uuids(&candidates, primary_index, &extra_indices, budget)?;

    candidates.sort_by_key(|candidate| (candidate.unit <= 0, candidate.unit, candidate.order));
    let mut selected = select_standard_fields(&candidates, budget)?;
    let fields = merge_fields(&candidates, &selected, limits, budget)?;

    let primary_position = candidates
        .iter()
        .position(|candidate| candidate.order == primary_order)
        .ok_or_else(|| schematic_error("KiCad netlist merge primary is missing"))?;
    let mut primary = candidates.remove(primary_position).component;
    primary.description = selected.pop().unwrap_or_default();
    primary.datasheet = selected.pop().unwrap_or_default();
    primary.footprint = selected.pop().unwrap_or_default();
    primary.value = selected.pop().unwrap_or_default();
    primary.instance_uuids = uuids;
    primary.fields = fields;
    Ok(primary)
}

fn select_primary(
    candidates: &[ComponentCandidate],
) -> Result<(usize, Vec<usize>), SourceBundleError> {
    if candidates.is_empty() {
        return Err(schematic_error("KiCad netlist merge group is empty"));
    }
    let mut primary_index = 0;
    let mut extra_indices = Vec::with_capacity(candidates.len().saturating_sub(1));
    for index in 1..candidates.len() {
        let primary_uuid = candidates[primary_index].component.instance_uuids.first();
        let candidate_uuid = candidates[index].component.instance_uuids.first();
        if primary_uuid.is_some_and(|value| !value.is_empty())
            && candidate_uuid.is_some_and(|value| !value.is_empty())
            && candidate_uuid < primary_uuid
        {
            extra_indices.push(primary_index);
            primary_index = index;
        } else {
            extra_indices.push(index);
        }
    }
    Ok((primary_index, extra_indices))
}

fn collect_uuids(
    candidates: &[ComponentCandidate],
    primary_index: usize,
    extra_indices: &[usize],
    budget: &mut StringBudget,
) -> Result<Vec<String>, SourceBundleError> {
    let mut seen_uuids = HashSet::new();
    let mut uuids = Vec::new();
    // KiCad retains nonprimary units in their equivalent-key insertion order,
    // then appends the lexically lowest-UUID primary unit last.
    for index in extra_indices.iter().chain(std::iter::once(&primary_index)) {
        for uuid in &candidates[*index].component.instance_uuids {
            if seen_uuids.insert(uuid.as_str()) {
                budget.reserve(uuid.len())?;
                uuids.push(uuid.clone());
            }
        }
    }
    Ok(uuids)
}

fn select_standard_fields(
    candidates: &[ComponentCandidate],
    budget: &mut StringBudget,
) -> Result<Vec<String>, SourceBundleError> {
    ["value", "footprint", "datasheet", "description"]
        .into_iter()
        .map(|field| {
            let value = candidates
                .iter()
                .map(|candidate| component_string(&candidate.component, field))
                .find(|value| !value.is_empty())
                .unwrap_or_default();
            budget.reserve(value.len())?;
            Ok(value.to_owned())
        })
        .collect()
}

fn merge_fields(
    candidates: &[ComponentCandidate],
    selected: &[String],
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<BTreeMap<String, String>, SourceBundleError> {
    let mut fields = BTreeMap::new();
    for candidate in candidates {
        for (name, value) in &candidate.component.fields {
            if !fields.contains_key(name) {
                insert_merged_field(&mut fields, name, value, limits, budget)?;
            }
        }
    }
    set_merged_field(&mut fields, "Footprint", &selected[1], limits, budget)?;
    set_merged_field(&mut fields, "Datasheet", &selected[2], limits, budget)?;
    set_merged_field(&mut fields, "Description", &selected[3], limits, budget)?;
    Ok(fields)
}

fn component_string<'a>(component: &'a KiCadNetlistComponent, field: &str) -> &'a str {
    match field {
        "value" => &component.value,
        "footprint" => &component.footprint,
        "datasheet" => &component.datasheet,
        _ => &component.description,
    }
}

fn insert_merged_field(
    fields: &mut BTreeMap<String, String>,
    name: &str,
    value: &str,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<(), SourceBundleError> {
    ensure_capacity(
        fields.len(),
        limits.max_component_fields,
        "KiCad netlist merged component field count exceeds its limit",
    )?;
    budget.reserve_many([name.len(), value.len()])?;
    fields.insert(name.to_owned(), value.to_owned());
    Ok(())
}

fn set_merged_field(
    fields: &mut BTreeMap<String, String>,
    name: &str,
    value: &str,
    limits: KiCadNetlistLimits,
    budget: &mut StringBudget,
) -> Result<(), SourceBundleError> {
    if let Some(existing) = fields.get_mut(name) {
        budget.reserve(value.len())?;
        *existing = value.to_owned();
        Ok(())
    } else {
        insert_merged_field(fields, name, value, limits, budget)
    }
}

#[cfg(test)]
mod tests {
    use super::merge_component_group;
    use crate::kicad_netlist::build::ComponentCandidate;
    use crate::kicad_netlist::resource::StringBudget;
    use crate::{KiCadNetlistComponent, KiCadNetlistLimits, SourceBundleErrorKind};
    use std::collections::BTreeMap;

    #[test]
    fn unit_order_wins_fields_while_uuid_order_selects_primary_and_deduplicates_all_ids() {
        let candidates = candidates();
        let mut budget = StringBudget::new(10_000);
        let merged = merge_component_group(
            candidates,
            KiCadNetlistLimits {
                max_component_fields: 4,
                ..KiCadNetlistLimits::default()
            },
            &mut budget,
        )
        .expect("exact merged-field limit");
        assert_eq!(merged.value, "unit-one");
        assert_eq!(merged.reference, "PRIMARY");
        assert_eq!(merged.fields["Custom"], "unit-one");
        assert_eq!(
            merged.instance_uuids,
            ["z-uuid", "duplicate", "m-uuid", "a-uuid"]
        );
    }

    #[test]
    fn merged_fields_and_retained_strings_accept_exact_limits_and_reject_one_under() {
        let mut measuring = StringBudget::new(10_000);
        merge_component_group(
            candidates(),
            KiCadNetlistLimits {
                max_component_fields: 4,
                ..KiCadNetlistLimits::default()
            },
            &mut measuring,
        )
        .expect("measurement");
        let exact_bytes = measuring.used();

        let mut exact = StringBudget::new(exact_bytes);
        merge_component_group(
            candidates(),
            KiCadNetlistLimits {
                max_component_fields: 4,
                ..KiCadNetlistLimits::default()
            },
            &mut exact,
        )
        .expect("exact retained-string limit");

        let mut one_under = StringBudget::new(exact_bytes - 1);
        assert_eq!(
            merge_component_group(
                candidates(),
                KiCadNetlistLimits {
                    max_component_fields: 4,
                    ..KiCadNetlistLimits::default()
                },
                &mut one_under,
            )
            .expect_err("one-under retained-string limit")
            .kind,
            SourceBundleErrorKind::ResourceLimit
        );

        let mut fields_one_under = StringBudget::new(10_000);
        assert_eq!(
            merge_component_group(
                candidates(),
                KiCadNetlistLimits {
                    max_component_fields: 3,
                    ..KiCadNetlistLimits::default()
                },
                &mut fields_one_under,
            )
            .expect_err("one-under merged-field limit")
            .kind,
            SourceBundleErrorKind::ResourceLimit
        );
    }

    fn candidates() -> Vec<ComponentCandidate> {
        vec![
            ComponentCandidate {
                unit: 1,
                order: 0,
                component: component("UNIT", "unit-one", ["z-uuid", "duplicate"], "unit-one"),
            },
            ComponentCandidate {
                unit: 2,
                order: 1,
                component: component("PRIMARY", "unit-two", ["a-uuid", "duplicate"], "unit-two"),
            },
            ComponentCandidate {
                unit: 3,
                order: 2,
                component: component("EXTRA", "unit-three", ["m-uuid", "duplicate"], "unit-three"),
            },
        ]
    }

    fn component(
        reference: &str,
        value: &str,
        uuids: [&str; 2],
        custom: &str,
    ) -> KiCadNetlistComponent {
        KiCadNetlistComponent {
            reference: reference.to_owned(),
            value: value.to_owned(),
            footprint: "footprint".to_owned(),
            datasheet: "datasheet".to_owned(),
            description: "description".to_owned(),
            fields: BTreeMap::from([("Custom".to_owned(), custom.to_owned())]),
            libsource_lib: String::new(),
            libsource_part: String::new(),
            libsource_description: String::new(),
            sheet_path_names: "/".to_owned(),
            sheet_path_uuids: "/".to_owned(),
            instance_uuids: uuids.into_iter().map(str::to_owned).collect(),
            properties: BTreeMap::new(),
            units: Vec::new(),
            in_bom: true,
            on_board: true,
            dnp: false,
        }
    }
}
