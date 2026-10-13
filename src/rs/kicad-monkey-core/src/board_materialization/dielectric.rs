//! Dielectric product: each dielectric slice is the board profile with its
//! cutouts, minus every hole whose span crosses that slice, minus the wider
//! back-drill and tertiary-drill bores that cross it.
//!
//! As in KiCad's own 3D model, a back-drill removes material from its
//! drilled outer copper layer through its end layer inclusive, so every
//! dielectric slice between the two is cut at the back-drill diameter.
//! Counterbores and countersinks cut the slices they reach (`machining.rs`).

use super::emit::{Emitter, Material, Origin};
use super::geometry::{identity, origin};
use super::holes::HoleSpec;
use super::machining::{Depths, dielectric_cut, report_dielectric_unknown};
use kicad_monkey_contracts::generated::materialization_document::{
    Backdrill, BindingSourceKind, BoardSide, Disk2d, MaterialAction, NonNegativeNm, PlanarRegion2d,
    Shape2d, SliceRole, Stack, SurfaceMaterial,
};
use std::collections::BTreeMap;

fn dielectric(action: MaterialAction) -> Material {
    Material {
        material: SurfaceMaterial::Dielectric,
        action,
        net_key: None,
        group_key: None,
    }
}

/// Whether a hole from `start` to `end` (copper slice orders) passes
/// through the slice at `order`.
fn crosses(order: u32, start: u32, end: u32) -> bool {
    start.min(end) < order && order < start.max(end)
}

/// Slice orders by key, plus the outermost copper orders.
struct Orders<'a> {
    by_key: BTreeMap<&'a str, u32>,
    front: Option<u32>,
    back: Option<u32>,
}

impl<'a> Orders<'a> {
    fn new(stack: &'a Stack) -> Self {
        let copper = || {
            stack
                .slices
                .iter()
                .filter(|slice| slice.role == SliceRole::Conductor)
                .map(|slice| slice.order)
        };
        Self {
            by_key: stack
                .slices
                .iter()
                .map(|slice| (slice.key.as_str(), slice.order))
                .collect(),
            front: copper().min(),
            back: copper().max(),
        }
    }

    fn of(&self, key: &str) -> Option<u32> {
        self.by_key.get(key).copied()
    }

    /// The copper span `(drilled surface, end layer)` of `drill`.
    fn bore_span(&self, drill: &Backdrill) -> Option<(u32, u32)> {
        let surface = match drill.side {
            BoardSide::Front => self.front,
            BoardSide::Back => self.back,
        }?;
        Some((surface, self.of(drill.until_slice_key.as_deref()?)?))
    }
}

fn bore(diameter: &NonNegativeNm) -> Shape2d {
    Shape2d::Disk2d(Disk2d {
        kind: "disk_2d".to_owned(),
        center_nm: origin(),
        radius_nm: NonNegativeNm(**diameter / 2),
    })
}

/// Report back-drills whose bore cannot be placed, once per material run.
fn report_unplaceable(holes: &[HoleSpec], orders: &Orders<'_>, emitter: &mut Emitter) {
    for hole in holes {
        for drill in &hole.facts.backdrills {
            let reason = if drill.diameter_nm.is_none() {
                "has no diameter"
            } else if orders.bore_span(drill).is_none() {
                "has no copper end layer"
            } else {
                continue;
            };
            emitter.warn(
                "backdrill_not_cut",
                format!("back-drill {reason}; dielectric keeps only the primary hole"),
                Some(hole.origin.uid.clone()),
            );
        }
    }
}

fn subtract(emitter: &mut Emitter, hole: &HoleSpec, slice_key: &str, shape: Shape2d) {
    emitter.surface(
        &hole.origin,
        slice_key,
        dielectric(MaterialAction::Subtract),
        shape,
        hole.affine.clone(),
    );
}

/// Primary hole and back-drill bores of `hole` that cross the slice.
fn cut_hole(
    hole: &HoleSpec,
    slice_key: &str,
    order: u32,
    (orders, depths): (&Orders<'_>, &Depths<'_>),
    emitter: &mut Emitter,
) {
    let span = orders
        .of(&hole.facts.start_slice_key)
        .zip(orders.of(&hole.facts.end_slice_key));
    if span.is_some_and(|(start, end)| crosses(order, start, end)) {
        subtract(emitter, hole, slice_key, hole.shape.clone());
    }
    for drill in &hole.facts.backdrills {
        if let (Some(diameter), Some((surface, end))) =
            (&drill.diameter_nm, orders.bore_span(drill))
            && crosses(order, surface, end)
        {
            subtract(emitter, hole, slice_key, bore(diameter));
        }
    }
    if let Ok(Some(shape)) = dielectric_cut(hole, slice_key, depths) {
        subtract(emitter, hole, slice_key, shape);
    }
}

pub(crate) fn dielectric_regions(
    regions: &[PlanarRegion2d],
    holes: &[HoleSpec],
    stack: &Stack,
    emitter: &mut Emitter,
) {
    if regions.is_empty() {
        return;
    }
    let orders = Orders::new(stack);
    report_unplaceable(holes, &orders, emitter);
    let depths = Depths::new(stack);
    report_dielectric_unknown(holes, stack, &depths, emitter);
    for slice in stack
        .slices
        .iter()
        .filter(|slice| slice.role == SliceRole::Dielectric)
    {
        for (position, region) in regions.iter().enumerate() {
            let origin = Origin::routed(
                BindingSourceKind::Profile,
                format!("board_profile#{position}"),
                String::new(),
            );
            let shape = Shape2d::PlanarRegion2d(region.clone());
            emitter.surface(
                &origin,
                &slice.key,
                dielectric(MaterialAction::Add),
                shape,
                identity(),
            );
        }
        for hole in holes {
            cut_hole(hole, &slice.key, slice.order, (&orders, &depths), emitter);
        }
    }
}
