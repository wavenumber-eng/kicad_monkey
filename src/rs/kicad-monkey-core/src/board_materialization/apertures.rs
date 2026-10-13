//! Solder-mask and paste openings of pads and vias, as KiCad plots them.
//!
//! Margins resolve pad, then footprint, then board (`PAD::
//! GetSolderMaskExpansion`, `PAD::GetSolderPasteMargin`). Pads without
//! copper layers get no margin. Each pad shape then grows or shrinks the way
//! KiCad's layer plotter does: circles, ovals, rectangles, rounded and
//! chamfered rectangles stay closed-form shapes, while trapezoids, chamfered
//! rectangles grown on both axes alike, and custom pads keep their shape
//! with a round-cornered `offset_nm`. Custom-rule (`.kicad_dru`) margins are
//! not evaluated.

use super::geometry::{mm, nm};
use super::pads::as_loaded;
use crate::pcb::{PcbFootprint, PcbPad, PcbResolvedPadCopperLayer, PcbSetup, PcbVia};

/// Board-level defaults from `(setup ...)`.
pub(crate) struct BoardMargins {
    pub mask_nm: i64,
    pub paste_nm: i64,
    pub paste_ratio: f64,
    pub tent_front: bool,
    pub tent_back: bool,
    pub mask_min_width_nm: i64,
    /// Pre-9.0 boards (format 20240201 and older) wrote 0 for "inherit" in
    /// pad and footprint margins.
    pub zero_inherits: bool,
}

/// Last board format in which a local margin of 0 meant "inherit".
const ZERO_MARGIN_INHERITS_THROUGH: i64 = 20_240_201;

impl BoardMargins {
    /// KiCad's tenting fallback: the legacy `viasonmask` plot option, else
    /// the declared `(tenting ...)`, else tented on both sides.
    pub(crate) fn from_setup(setup: Option<&PcbSetup>, version: i64) -> Self {
        let zero_inherits = version <= ZERO_MARGIN_INHERITS_THROUGH;
        let Some(setup) = setup else {
            return Self {
                mask_nm: 0,
                paste_nm: 0,
                paste_ratio: 0.0,
                tent_front: true,
                tent_back: true,
                mask_min_width_nm: 0,
                zero_inherits,
            };
        };
        // KiCad parses the plot options after `(tenting ...)`, so a legacy
        // `viasonmask` wins when both are present.
        let (tent_front, tent_back) = match setup.legacy_vias_on_mask {
            Some(on_mask) => (!on_mask, !on_mask),
            None if setup.has_tenting => (setup.tenting_front, setup.tenting_back),
            None => (true, true),
        };
        Self {
            mask_nm: i64::from(nm(setup.pad_to_mask_clearance)),
            paste_nm: i64::from(nm(setup.pad_to_paste_clearance)),
            paste_ratio: setup.pad_to_paste_clearance_ratio,
            tent_front,
            tent_back,
            mask_min_width_nm: i64::from(nm(setup.solder_mask_min_width)),
            zero_inherits,
        }
    }

    /// A pad or footprint local value, or `None` when it inherits.
    fn local(&self, value: Option<f64>) -> Option<f64> {
        value.filter(|value| !(self.zero_inherits && *value == 0.0))
    }
}

/// Whether a KiCad layer list names `layer`, directly or by wildcard.
pub(crate) fn on_layer(layers: &[String], layer: &str) -> bool {
    layers.iter().any(|name| {
        name == layer
            || name
                .strip_prefix('*')
                .is_some_and(|suffix| layer.ends_with(suffix))
    })
}

fn has_copper(pad: &PcbPad) -> bool {
    pad.layers.iter().any(|layer| layer.ends_with(".Cu"))
}

fn size_nm(resolved: &PcbResolvedPadCopperLayer<'_>) -> (i64, i64) {
    (
        i64::from(nm(resolved.size.x)),
        i64::from(nm(resolved.size.y)),
    )
}

/// Solder-mask expansion of `pad`, clamped so the opening stays
/// non-negative.
pub(crate) fn mask_margin(
    pad: &PcbPad,
    footprint: &PcbFootprint,
    resolved: &PcbResolvedPadCopperLayer<'_>,
    board: &BoardMargins,
) -> (i64, i64) {
    if !has_copper(pad) {
        return (0, 0);
    }
    let margin = board
        .local(pad.solder_mask_margin)
        .or(board.local(footprint.solder_mask_margin))
        .map_or(board.mask_nm, |value| i64::from(nm(value)));
    let (width, height) = size_nm(resolved);
    let margin = margin.max(-(width.min(height) / 2));
    (margin, margin)
}

/// Solder-paste margin of `pad` per axis: the absolute margin plus the
/// ratio of the pad size, clamped for non-custom pads.
pub(crate) fn paste_margin(
    pad: &PcbPad,
    footprint: &PcbFootprint,
    resolved: &PcbResolvedPadCopperLayer<'_>,
    board: &BoardMargins,
) -> (i64, i64) {
    if !has_copper(pad) {
        return (0, 0);
    }
    let margin = board
        .local(pad.solder_paste_margin)
        .or(board.local(footprint.solder_paste_margin))
        .map_or(board.paste_nm, |value| i64::from(nm(value)));
    let ratio = board
        .local(pad.solder_paste_margin_ratio)
        .or(board.local(footprint.solder_paste_margin_ratio))
        .unwrap_or(board.paste_ratio);
    let (width, height) = size_nm(resolved);
    let along = |size: i64| {
        let grown = margin as f64 + (size as f64 * ratio).round();
        let value = grown.clamp(-(NM_LIMIT as f64), NM_LIMIT as f64) as i64;
        if resolved.shape == "custom" {
            value
        } else {
            value.max(-(size / 2))
        }
    };
    (along(width), along(height))
}

/// A pad shape adjusted for an opening, with the offsets KiCad applies to
/// its base shape and to custom primitives.
pub(crate) struct Aperture<'a> {
    pub resolved: PcbResolvedPadCopperLayer<'a>,
    pub base_offset_nm: i32,
    pub primitive_offset_nm: i32,
}

/// Largest magnitude a margin or a grown size may take: the 32-bit
/// nanometre range of the contract.
const NM_LIMIT: i64 = i32::MAX as i64;

fn offset(value_nm: i64) -> i32 {
    i32::try_from(value_nm).unwrap_or(if value_nm < 0 { i32::MIN } else { i32::MAX })
}

/// Adjust `resolved` by `margin` (x, y) as KiCad's plotter does, or `None`
/// when the opening vanishes.
pub(crate) fn aperture<'a>(
    resolved: &PcbResolvedPadCopperLayer<'a>,
    margin: (i64, i64),
) -> Option<Aperture<'a>> {
    let resolved = &as_loaded(resolved);
    let margin = (
        margin.0.clamp(-NM_LIMIT, NM_LIMIT),
        margin.1.clamp(-NM_LIMIT, NM_LIMIT),
    );
    if resolved.shape == "custom" {
        return Some(custom_aperture(resolved, margin));
    }
    let (width, height) = size_nm(resolved);
    // Grown sizes saturate at the contract range, and every derived ratio
    // uses the saturated size, so shapes stay valid.
    let plot = (
        (width + 2 * margin.0).min(NM_LIMIT),
        (height + 2 * margin.1).min(NM_LIMIT),
    );
    if plot.0 <= 0 || plot.1 <= 0 {
        return None;
    }
    let (adjusted, base_offset_nm) = standard_aperture(resolved, margin, plot);
    Some(Aperture {
        resolved: adjusted,
        base_offset_nm,
        primitive_offset_nm: 0,
    })
}

/// A custom pad: its primitives are offset by the margin; a shrinking
/// margin shrinks the anchor instead of offsetting it.
fn custom_aperture<'a>(
    resolved: &PcbResolvedPadCopperLayer<'a>,
    margin: (i64, i64),
) -> Aperture<'a> {
    let (width, height) = size_nm(resolved);
    let mut adjusted = *resolved;
    let mut base_offset_nm = 0;
    if margin.0 < 0 {
        adjusted.size.x = mm((width + 2 * margin.0).max(0));
        adjusted.size.y = mm((height + 2 * margin.1).max(0));
    } else {
        base_offset_nm = offset(margin.0);
    }
    Aperture {
        resolved: adjusted,
        base_offset_nm,
        primitive_offset_nm: offset(margin.0),
    }
}

/// A standard pad resized to `plot`, or kept with a base offset, per
/// KiCad's `plotPadLayer` shape rules.
fn standard_aperture<'a>(
    resolved: &PcbResolvedPadCopperLayer<'a>,
    margin: (i64, i64),
    plot: (i64, i64),
) -> (PcbResolvedPadCopperLayer<'a>, i32) {
    let mut adjusted = *resolved;
    adjusted.size.x = mm(plot.0);
    adjusted.size.y = mm(plot.1);
    let chamfered = resolved.chamfer_ratio > 0.0 && !resolved.chamfer_corners.is_empty();
    let same = margin.0 == margin.1;
    match resolved.shape {
        "rect" if margin.0 > 0 => {
            adjusted.shape = "roundrect";
            adjusted.roundrect_rratio = mm(margin.0) / mm(plot.0.min(plot.1));
            (adjusted, 0)
        }
        "roundrect" if chamfered && margin.0 > 0 && same => (*resolved, offset(margin.0)),
        "roundrect" if !chamfered && same => {
            let (width, height) = size_nm(resolved);
            let short = width.min(height);
            let radius = (short as f64 * resolved.roundrect_rratio).round() as i64;
            let radius = radius.clamp(0, short / 2);
            adjusted.roundrect_rratio = mm((radius + margin.0).max(0)) / mm(plot.0.min(plot.1));
            (adjusted, 0)
        }
        "trapezoid" => (*resolved, offset(margin.0)),
        _ => (adjusted, 0),
    }
}

/// Diameter of the solder-mask opening of `via` on the front (`true`) or
/// back side, or `None` when that side is tented or not an outer layer.
pub(crate) fn via_opening_nm(via: &PcbVia, front: bool, board: &BoardMargins) -> Option<i64> {
    let (outer, layer) = if front {
        ("F.Cu", via.layers.first())
    } else {
        ("B.Cu", via.layers.last())
    };
    if layer.map(String::as_str) != Some(outer) {
        return None;
    }
    let authored = via
        .tenting
        .as_ref()
        .and_then(|value| if front { value.front } else { value.back });
    let tented = authored.unwrap_or(if front {
        board.tent_front
    } else {
        board.tent_back
    });
    let diameter = (i64::from(nm(via.size)) + 2 * board.mask_nm).min(2 * NM_LIMIT);
    (!tented && diameter > 0).then_some(diameter)
}
