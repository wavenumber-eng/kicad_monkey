//! Counterbores and countersinks: the copper and dielectric they remove.
//!
//! Copper follows `PAD::GetPostMachiningKnockout` in KiCad 10.0.6: a copper
//! layer closer to the machined side than the depth (KiCad's
//! `BOARD_STACKUP::GetLayerDistance`) loses a disk of the counterbore
//! diameter, or of the countersink diameter at that layer. KiCad then
//! treats the pad or via as having no annular ring there. Its Gerber plot
//! keeps the land, so this cut is physical intent beyond the plot.
//!
//! Dielectric follows KiCad's STEP export: a dielectric slice whose surface
//! nearer the machined side lies within the depth loses a disk of the
//! diameter at that surface.

use super::emit::{Emitter, Material};
use super::geometry::origin;
use super::holes::HoleSpec;
use kicad_monkey_contracts::generated::materialization_document::{
    BoardSide, Disk2d, MaterialAction, NonNegativeNm, PostMachining, PostMachiningMode, Shape2d,
    SliceRole, Stack, StackSlice, SurfaceMaterial,
};

/// Conductor and dielectric slices seen from one side of the board.
pub(crate) struct Depths<'a> {
    front: Vec<&'a StackSlice>,
}

impl<'a> Depths<'a> {
    pub(crate) fn new(stack: &'a Stack) -> Self {
        let physical: Vec<&StackSlice> = stack
            .slices
            .iter()
            .filter(|slice| matches!(slice.role, SliceRole::Conductor | SliceRole::Dielectric))
            .collect();
        // KiCad measures from the outer copper layer.
        let first = physical
            .iter()
            .position(|slice| slice.role == SliceRole::Conductor);
        let last = physical
            .iter()
            .rposition(|slice| slice.role == SliceRole::Conductor);
        let front = match (first, last) {
            (Some(first), Some(last)) => physical[first..=last].to_vec(),
            _ => Vec::new(),
        };
        Self { front }
    }

    fn from(&self, side: BoardSide) -> Vec<&'a StackSlice> {
        match side {
            BoardSide::Front => self.front.clone(),
            BoardSide::Back => self.front.iter().rev().copied().collect(),
        }
    }

    /// KiCad's distance from the outer copper on `side` to the copper slice
    /// `key`: every slice from that outer copper through `key`, an inner
    /// target layer counting half. `None` when a thickness is unknown.
    fn copper_distance(&self, side: BoardSide, key: &str) -> Option<i64> {
        let slices = self.from(side);
        if slices.first()?.key == key {
            return Some(0);
        }
        let target = slices.iter().position(|slice| slice.key == key)?;
        let inner = target + 1 < slices.len();
        let mut total = 0;
        for (position, slice) in slices[..=target].iter().enumerate() {
            let thickness = **slice.thickness_nm.as_ref()?;
            total += if position == target && inner {
                thickness / 2
            } else {
                thickness
            };
        }
        Some(total)
    }

    /// Distance from the outer copper surface on `side` to the nearer
    /// surface of slice `key`.
    fn near_surface(&self, side: BoardSide, key: &str) -> Option<i64> {
        let slices = self.from(side);
        let target = slices.iter().position(|slice| slice.key == key)?;
        slices[..target]
            .iter()
            .map(|slice| slice.thickness_nm.as_ref().map(|value| **value))
            .sum()
    }
}

/// Half the cone angle in radians, computed as KiCad does from the angle
/// it stores in tenths of a degree, so results match it to the nanometre.
fn half_angle(angle_deg: f64) -> f64 {
    let tenths = (angle_deg * 10.0).round();
    (tenths / 10.0) * std::f64::consts::PI / 180.0 / 2.0
}

/// Diameter, depth, and half cone angle (radians, countersink only) of one
/// machining, as KiCad derives them; `None` when it removes nothing.
fn geometry(machining: &PostMachining) -> Option<(i64, i64, Option<f64>)> {
    let diameter = **machining.diameter_nm.as_ref()?;
    let half = (machining.mode == PostMachiningMode::Countersink)
        .then_some(machining.angle_deg)
        .flatten()
        .map(half_angle);
    let depth = match (machining.depth_nm.as_ref(), half) {
        (Some(depth), _) => **depth,
        // A countersink without a depth runs to its cone apex.
        (None, Some(half)) => ((diameter as f64 / 2.0) / half.tan()) as i64,
        (None, None) => 0,
    };
    (diameter > 0 && depth > 0).then_some((diameter, depth, half))
}

/// Diameter removed `distance` below the machined surface.
fn diameter_at(diameter: i64, distance: i64, half: Option<f64>) -> i64 {
    match half {
        Some(half) => (diameter - (2.0 * distance as f64 * half.tan()) as i64).max(0),
        None => diameter,
    }
}

fn disk(diameter: i64) -> Shape2d {
    Shape2d::Disk2d(Disk2d {
        kind: "disk_2d".to_owned(),
        center_nm: origin(),
        radius_nm: NonNegativeNm(diameter / 2),
    })
}

fn report_unknown_depth(hole: &HoleSpec, emitter: &mut Emitter) {
    emitter.warn(
        "post_machining_not_cut",
        "the stack has no thickness for a layer this counterbore or countersink reaches; \
         nothing is removed"
            .to_owned(),
        Some(hole.origin.uid.clone()),
    );
}

/// Copper each hole's machining removes, one subtract per affected copper
/// slice on the hole's net. The front machining decides a layer it
/// reaches; the back machining only the layers the front leaves.
pub(crate) fn copper_knockouts(holes: &[HoleSpec], stack: &Stack, emitter: &mut Emitter) {
    let depths = Depths::new(stack);
    let copper: Vec<&StackSlice> = stack
        .slices
        .iter()
        .filter(|slice| slice.role == SliceRole::Conductor)
        .collect();
    for hole in holes
        .iter()
        .filter(|hole| !hole.facts.post_machining.is_empty())
    {
        let mut unknown = false;
        for slice in &copper {
            let removed = hole.facts.post_machining.iter().find_map(|machining| {
                let (diameter, depth, half) = geometry(machining)?;
                let Some(distance) = depths.copper_distance(machining.side, &slice.key) else {
                    unknown = true;
                    return None;
                };
                (distance < depth).then(|| diameter_at(diameter, distance, half))
            });
            if let Some(diameter) = removed.filter(|diameter| *diameter > 0) {
                let material = Material {
                    material: SurfaceMaterial::Copper,
                    action: MaterialAction::Subtract,
                    net_key: Some(hole.origin.net_key.clone()),
                    group_key: None,
                };
                emitter.surface(
                    &hole.origin,
                    &slice.key,
                    material,
                    disk(diameter),
                    hole.affine.clone(),
                );
            }
        }
        if unknown {
            report_unknown_depth(hole, emitter);
        }
    }
}

/// The disk `hole`'s machining removes from dielectric slice `key`, if any.
/// Unknown thickness is reported once per hole by the caller's first slice.
pub(crate) fn dielectric_cut(
    hole: &HoleSpec,
    key: &str,
    depths: &Depths<'_>,
) -> Result<Option<Shape2d>, ()> {
    let mut widest: Option<i64> = None;
    for machining in &hole.facts.post_machining {
        let Some((diameter, depth, half)) = geometry(machining) else {
            continue;
        };
        let distance = depths.near_surface(machining.side, key).ok_or(())?;
        if distance < depth {
            let here = diameter_at(diameter, distance, half);
            widest = Some(widest.map_or(here, |value| value.max(here)));
        }
    }
    Ok(widest.filter(|diameter| *diameter > 0).map(disk))
}

/// Report holes whose machining depth cannot be placed in the stack.
pub(crate) fn report_dielectric_unknown(
    holes: &[HoleSpec],
    stack: &Stack,
    depths: &Depths<'_>,
    emitter: &mut Emitter,
) {
    for hole in holes
        .iter()
        .filter(|hole| !hole.facts.post_machining.is_empty())
    {
        let unknown = stack
            .slices
            .iter()
            .filter(|slice| slice.role == SliceRole::Dielectric)
            .any(|slice| dielectric_cut(hole, &slice.key, depths).is_err());
        if unknown {
            report_unknown_depth(hole, emitter);
        }
    }
}
