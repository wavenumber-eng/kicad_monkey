use super::types::SchematicDesignNetLimits;
use crate::{
    SchematicBundleIndex, SchematicLabelDriver, SchematicPoint, SourceBundleError,
    SourceBundleErrorKind,
};
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct SheetPinTargetIndex {
    by_parent: HashMap<usize, ParentSheetPinTargets>,
}

#[derive(Default)]
struct ParentSheetPinTargets {
    by_uuid: HashMap<String, String>,
    by_point: HashMap<SchematicPoint, HashMap<String, String>>,
}

impl SheetPinTargetIndex {
    pub(super) fn build(
        index: &SchematicBundleIndex,
        limits: SchematicDesignNetLimits,
    ) -> Result<Self, SourceBundleError> {
        let mut result = Self::default();
        let mut target_count = 0_usize;
        let mut target_string_bytes = 0_usize;
        for child in index
            .occurrences()
            .filter(|value| value.parent_index.is_some())
        {
            let (Some(parent_index), Some(parent_sheet_index)) =
                (child.parent_index, child.parent_sheet_index)
            else {
                continue;
            };
            let parent = index
                .occurrence(parent_index)
                .ok_or_else(|| limit_error(None, "sheet-pin target parent is missing"))?;
            let definition = index.definition(&parent.source_path).ok_or_else(|| {
                limit_error(
                    Some(&parent.source_path),
                    "sheet-pin target parent definition is missing",
                )
            })?;
            let sheet = definition.sheets.get(parent_sheet_index).ok_or_else(|| {
                limit_error(
                    Some(&parent.source_path),
                    "sheet-pin target parent sheet index is out of range",
                )
            })?;
            if sheet.on_board {
                continue;
            }
            for pin in &sheet.pins {
                target_count = target_count.checked_add(1).ok_or_else(|| {
                    limit_error(
                        Some(&parent.source_path),
                        "sheet-pin target count overflows",
                    )
                })?;
                if target_count > limits.max_sheet_pin_targets {
                    return Err(limit_error(
                        Some(&parent.source_path),
                        "sheet-pin target count exceeds its limit",
                    ));
                }
                let targets = result.by_parent.entry(parent_index).or_default();
                if pin.uuid.is_empty() {
                    let by_name = targets.by_point.entry(pin.at).or_default();
                    if by_name.contains_key(pin.name.as_str()) {
                        continue;
                    }
                    target_string_bytes = add_target_string_bytes(
                        target_string_bytes,
                        pin.name.len(),
                        child.human_address.len(),
                        limits.max_target_index_bytes,
                        &parent.source_path,
                    )?;
                    by_name.insert(pin.name.clone(), child.human_address.clone());
                } else {
                    if targets.by_uuid.contains_key(pin.uuid.as_str()) {
                        continue;
                    }
                    target_string_bytes = add_target_string_bytes(
                        target_string_bytes,
                        pin.uuid.len(),
                        child.human_address.len(),
                        limits.max_target_index_bytes,
                        &parent.source_path,
                    )?;
                    targets
                        .by_uuid
                        .insert(pin.uuid.clone(), child.human_address.clone());
                }
            }
        }
        Ok(result)
    }

    pub(super) fn target_path(
        &self,
        parent_occurrence_index: usize,
        label: &SchematicLabelDriver,
    ) -> Option<&str> {
        let targets = self.by_parent.get(&parent_occurrence_index)?;
        if label.source_uuid.is_empty() {
            targets
                .by_point
                .get(&label.at)?
                .get(label.text.as_str())
                .map(String::as_str)
        } else {
            targets
                .by_uuid
                .get(label.source_uuid.as_str())
                .map(String::as_str)
        }
    }
}

fn add_target_string_bytes(
    current: usize,
    key_bytes: usize,
    value_bytes: usize,
    limit: usize,
    source_path: &str,
) -> Result<usize, SourceBundleError> {
    let total = current
        .checked_add(key_bytes)
        .and_then(|value| value.checked_add(value_bytes))
        .ok_or_else(|| limit_error(Some(source_path), "sheet-pin target string bytes overflow"))?;
    if total > limit {
        return Err(limit_error(
            Some(source_path),
            "sheet-pin target string bytes exceed their limit",
        ));
    }
    Ok(total)
}

fn limit_error(path: Option<&str>, message: &str) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, path, message)
}
