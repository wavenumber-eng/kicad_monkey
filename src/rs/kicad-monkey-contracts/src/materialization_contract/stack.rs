//! Stack-region rules: every region names known slices in stack order,
//! every slice lies in some region, and only flexible regions bend.

use super::shapes::validate_shape;
use super::{Keys, check_length};
use crate::generated::materialization_document::{Shape2d, Stack, StackRegionKind};
use crate::{ValidationError, validation_error};
use std::collections::BTreeSet;

pub(super) fn validate_regions(stack: &Stack, keys: &Keys<'_>) -> Result<(), ValidationError> {
    if stack.regions.is_empty() {
        return Err(validation_error(
            "missing_stack_region",
            "$.stack.regions",
            "a stack has at least one region",
        ));
    }
    let mut region_keys = BTreeSet::new();
    for (position, region) in stack.regions.iter().enumerate() {
        if !region_keys.insert(region.key.as_str()) {
            return Err(validation_error(
                "duplicate_stack_region",
                format!("$.stack.regions[{position}].key"),
                "stack region keys are unique",
            ));
        }
    }
    let several = stack.regions.len() > 1;
    let mut covered = BTreeSet::new();
    for (position, region) in stack.regions.iter().enumerate() {
        let path = format!("$.stack.regions[{position}]");
        let orders = region
            .slice_keys
            .iter()
            .map(|key| keys.order(key))
            .collect::<Option<Vec<u32>>>()
            .filter(|orders| !orders.is_empty() && orders.windows(2).all(|p| p[0] < p[1]));
        if orders.is_none() {
            return Err(validation_error(
                "invalid_region_slices",
                format!("{path}.slice_keys"),
                "a region lists known slices, at least one, in stack order",
            ));
        }
        covered.extend(region.slice_keys.iter().map(String::as_str));
        validate_boundary(&region.boundary, several, &path)?;
        validate_bends(region.kind, &region.bend_lines, &path)?;
    }
    if covered.len() != keys.slices.len() {
        return Err(validation_error(
            "uncovered_slice",
            "$.stack.regions",
            "every slice lies in at least one region",
        ));
    }
    Ok(())
}

/// With several regions each has a boundary; every boundary area is a
/// valid planar region.
fn validate_boundary(
    boundary: &[crate::generated::materialization_document::PlanarRegion2d],
    several: bool,
    path: &str,
) -> Result<(), ValidationError> {
    if several && boundary.is_empty() {
        return Err(validation_error(
            "missing_region_boundary",
            format!("{path}.boundary"),
            "when a stack has several regions, each has a boundary",
        ));
    }
    for (index, area) in boundary.iter().enumerate() {
        validate_shape(
            &Shape2d::PlanarRegion2d(area.clone()),
            &format!("{path}.boundary[{index}]"),
        )?;
    }
    Ok(())
}

fn validate_bends(
    kind: StackRegionKind,
    bends: &[crate::generated::materialization_document::BendLine],
    path: &str,
) -> Result<(), ValidationError> {
    if kind == StackRegionKind::Rigid && !bends.is_empty() {
        return Err(validation_error(
            "rigid_region_bends",
            format!("{path}.bend_lines"),
            "only flexible regions carry bend lines",
        ));
    }
    for (index, bend) in bends.iter().enumerate() {
        let bend_path = format!("{path}.bend_lines[{index}]");
        check_length(*bend.radius_nm, &format!("{bend_path}.radius_nm"))?;
        if !bend.angle_deg.is_finite() {
            return Err(validation_error(
                "invalid_bend_angle",
                format!("{bend_path}.angle_deg"),
                "a bend angle is a finite number of degrees",
            ));
        }
    }
    Ok(())
}
