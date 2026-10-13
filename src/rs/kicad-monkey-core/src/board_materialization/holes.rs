//! Via and pad holes with slice spans, protection, and back-drill.
//!
//! Holes are collected as [`HoleSpec`]s so the holes product and the
//! dielectric product (which subtracts them) see the same holes. Plating
//! thickness is not stated by KiCad and stays unknown.

use super::apertures::BoardMargins;
use super::emit::{Emitter, HoleFacts, Origin, source_uid};
use super::geometry::{nm, origin, rotation_translation};
use super::nets::net_key;
use super::pads::{PadFrame, oval};
use super::stack::CopperStack;
use crate::pcb::{
    PcbDrillProperties, PcbFrontBackOptionalBool, PcbHoleShape, PcbPad, PcbPostMachiningProperties,
    PcbSetup, PcbVia,
};
use kicad_monkey_contracts::generated::materialization_document::{
    Affine2D, Backdrill, BindingSourceKind, BoardSide, Disk2d, HolePlating, HoleProtection,
    NonNegativeNm, PostMachining, PostMachiningMode, ProtectionState, Shape2d, SideProtection,
};

/// One hole before it becomes an operation.
pub(crate) struct HoleSpec {
    pub origin: Origin,
    pub facts: HoleFacts,
    pub shape: Shape2d,
    pub affine: Affine2D,
}

fn round_hole(diameter: f64) -> Shape2d {
    Shape2d::Disk2d(Disk2d {
        kind: "disk_2d".to_owned(),
        center_nm: origin(),
        radius_nm: NonNegativeNm(i64::from(nm(diameter / 2.0))),
    })
}

/// Last board format in which a via's absent covering, plugging, filling
/// and capping meant "off" rather than "as the board" (KiCad's parser).
const PROTECTION_OFF_BEFORE: i64 = 20_250_228;

/// The board's via protection, which a via's unstated sides follow
/// (`PCB_VIA`'s `FROM_BOARD` modes, from `(setup (tenting ...) (covering
/// ...) (plugging ...) (capping ...) (filling ...))`); tenting follows
/// KiCad's fallbacks (see [`BoardMargins::from_setup`]). Boards older than
/// format 20250228 did not write the other four, so they are off there.
pub(crate) struct ViaDefaults {
    tented: (bool, bool),
    covered: (bool, bool),
    plugged: (bool, bool),
    filled: bool,
    capped: bool,
}

impl ViaDefaults {
    pub(crate) fn from_setup(setup: Option<&PcbSetup>, version: i64) -> Self {
        let margins = BoardMargins::from_setup(setup, version);
        let tented = (margins.tent_front, margins.tent_back);
        let setup = setup.filter(|_| version >= PROTECTION_OFF_BEFORE);
        setup.map_or(
            Self {
                tented,
                covered: (false, false),
                plugged: (false, false),
                filled: false,
                capped: false,
            },
            |setup| Self {
                tented,
                covered: (setup.covering_front, setup.covering_back),
                plugged: (setup.plugging_front, setup.plugging_back),
                filled: setup.filling,
                capped: setup.capping,
            },
        )
    }
}

fn resolved(authored: Option<bool>, board: bool) -> ProtectionState {
    if authored.unwrap_or(board) {
        ProtectionState::Yes
    } else {
        ProtectionState::No
    }
}

fn sides(
    value: Option<&PcbFrontBackOptionalBool>,
    board: (bool, bool),
) -> (ProtectionState, ProtectionState) {
    (
        resolved(value.and_then(|value| value.front), board.0),
        resolved(value.and_then(|value| value.back), board.1),
    )
}

/// Via protection as fabricated: each side as the via states it, else as
/// the board does (KiCad's `FROM_BOARD` modes).
fn via_protection(via: &PcbVia, board: &ViaDefaults) -> HoleProtection {
    let (tented_front, tented_back) = sides(via.tenting.as_ref(), board.tented);
    let (covered_front, covered_back) = sides(via.covering.as_ref(), board.covered);
    let (plugged_front, plugged_back) = sides(via.plugging.as_ref(), board.plugged);
    HoleProtection {
        front: SideProtection {
            tented: tented_front,
            covered: covered_front,
            plugged: plugged_front,
        },
        back: SideProtection {
            tented: tented_back,
            covered: covered_back,
            plugged: plugged_back,
        },
        filled: resolved(via.filling, board.filled),
        capped: resolved(via.capping, board.capped),
    }
}

/// Back-drill and tertiary drill records. The drilled side is the side of
/// the start layer; `until_slice_key` is the authored end layer.
fn backdrills(drills: [Option<&PcbDrillProperties>; 2], stack: &CopperStack) -> Vec<Backdrill> {
    drills
        .into_iter()
        .flatten()
        .map(|drill| Backdrill {
            side: if drill.layers.start == "B.Cu" {
                BoardSide::Back
            } else {
                BoardSide::Front
            },
            diameter_nm: drill
                .size
                .filter(|size| *size > 0.0)
                .map(|size| NonNegativeNm(i64::from(nm(size)))),
            until_slice_key: stack.key(&drill.layers.end).map(str::to_owned),
        })
        .collect()
}

fn facts(plating: HolePlating, layers: &[String], stack: &CopperStack) -> Option<HoleFacts> {
    let start = layers
        .first()
        .and_then(|name| stack.key(name))
        .or_else(|| stack.front_key())?;
    let end = layers
        .last()
        .and_then(|name| stack.key(name))
        .or_else(|| stack.back_key())?;
    Some(HoleFacts {
        plating,
        start_slice_key: start.to_owned(),
        end_slice_key: end.to_owned(),
        plating_thickness_nm: None,
        protection: None,
        backdrills: Vec::new(),
        post_machining: Vec::new(),
        castellated: None,
    })
}

/// Front and back counterbores and countersinks as authored. A size,
/// depth, or angle that is zero or absent is unknown, as KiCad writes only
/// positive ones.
fn post_machining(
    front: Option<&PcbPostMachiningProperties>,
    back: Option<&PcbPostMachiningProperties>,
) -> Vec<PostMachining> {
    let positive = |value: Option<f64>| value.filter(|value| *value > 0.0);
    [(BoardSide::Front, front), (BoardSide::Back, back)]
        .into_iter()
        .filter_map(|(side, machining)| {
            let machining = machining?;
            let mode = match machining.mode.as_str() {
                "counterbore" => PostMachiningMode::Counterbore,
                "countersink" => PostMachiningMode::Countersink,
                _ => return None,
            };
            Some(PostMachining {
                side,
                mode,
                diameter_nm: positive(machining.size)
                    .map(|size| NonNegativeNm(i64::from(nm(size)))),
                depth_nm: positive(machining.depth)
                    .map(|depth| NonNegativeNm(i64::from(nm(depth)))),
                angle_deg: positive(machining.angle),
            })
        })
        .collect()
}

pub(crate) fn via_holes(
    vias: &[PcbVia],
    board: &ViaDefaults,
    stack: &CopperStack,
    holes: &mut Vec<HoleSpec>,
    emitter: &mut Emitter,
) {
    for via in vias.iter().filter(|via| via.drill > 0.0) {
        let uid = source_uid("via", via.uuid.as_deref(), via.source_range.start);
        let unknown = stack.unknown(&via.layers);
        if !unknown.is_empty() {
            // A span to an undeclared layer has no depth on this board.
            emitter.unknown_layers(&unknown, &uid);
            continue;
        }
        let Some(mut facts) = facts(HolePlating::Plated, &stack.expand(&via.layers), stack) else {
            continue;
        };
        facts.protection = Some(via_protection(via, board));
        facts.backdrills = backdrills([via.backdrill.as_ref(), via.tertiary_drill.as_ref()], stack);
        facts.post_machining = post_machining(
            via.front_post_machining.as_ref(),
            via.back_post_machining.as_ref(),
        );
        holes.push(HoleSpec {
            origin: Origin::routed(BindingSourceKind::Via, uid, net_key(&via.net)),
            facts,
            shape: round_hole(via.drill),
            affine: rotation_translation(0.0, via.at_x, via.at_y),
        });
    }
}

/// The hole of `pad`, if it has one. Every pad hole is drilled through the
/// whole board, plated or not. The hole sits at the pad position; the
/// `(drill (offset ...))` moves the copper shape, not the hole. A round
/// drill uses its X size, as KiCad does.
pub(crate) fn pad_hole(
    pad: &PcbPad,
    frame: &PadFrame,
    origin: Origin,
    stack: &CopperStack,
) -> Option<HoleSpec> {
    let drill = pad.drill.as_ref()?;
    let height = drill.height.unwrap_or(drill.width);
    if drill.width <= 0.0 || height <= 0.0 {
        return None;
    }
    let plated = pad.plated.unwrap_or(pad.kind != "np_thru_hole");
    let layers = stack.names.clone();
    let plating = if plated {
        HolePlating::Plated
    } else {
        HolePlating::Unplated
    };
    let mut facts = facts(plating, &layers, stack)?;
    facts.backdrills = backdrills([pad.backdrill.as_ref(), pad.tertiary_drill.as_ref()], stack);
    facts.post_machining = post_machining(
        pad.front_post_machining.as_ref(),
        pad.back_post_machining.as_ref(),
    );
    facts.castellated =
        (pad.fabrication_property.as_deref() == Some("pad_prop_castellated")).then_some(true);
    let shape = match drill.shape {
        PcbHoleShape::Oval if (drill.width - height).abs() > 1e-9 => oval(drill.width, height),
        PcbHoleShape::Oval => round_hole(drill.width.min(height)),
        _ => round_hole(drill.width),
    };
    Some(HoleSpec {
        origin,
        facts,
        shape,
        affine: frame.affine(0.0, 0.0),
    })
}
