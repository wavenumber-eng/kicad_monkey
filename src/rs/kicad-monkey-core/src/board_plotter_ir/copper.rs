//! Board segment/track-arc/via/zone record emission with net extras.

use super::{
    BoardNetClassAssignments, BoardSegmentRecord, BoardTrackArcRecord, BoardViaFabrication,
    BoardViaOperation, BoardViaOperationKind, BoardViaRecord, BoardViaType, BoardZoneRecord,
    BudgetTracker, layerless_segment, net_parts,
};
use crate::pcb::{PcbRoutingArc, PcbSegment, PcbVia, PcbZone};
use crate::plotter_ir::mm_to_nm;
use crate::plotter_types::{ArcThreePoint, PlotterFill, PlotterOperation, PlotterPoly};
use crate::sexpr::Error;

pub(super) fn segment_record(
    segment: PcbSegment,
    net_classes: &BoardNetClassAssignments,
    budget: &mut BudgetTracker,
) -> Result<BoardSegmentRecord, Error> {
    let (net_id, net_name) = net_parts(&segment.net);
    let extras = net_classes.extras_for_bounded(net_name.as_deref(), budget)?;
    // Track widths are emitted verbatim (negative values included), unlike
    // the non-positive -> 0 normalization applied to graphic strokes.
    let width_nm = mm_to_nm(segment.width.unwrap_or(0.0))?;
    let operation = layerless_segment(
        [mm_to_nm(segment.start_x)?, mm_to_nm(segment.start_y)?],
        [mm_to_nm(segment.end_x)?, mm_to_nm(segment.end_y)?],
        width_nm,
    );
    Ok(BoardSegmentRecord {
        uuid: segment.uuid.unwrap_or_default(),
        layer: segment.layer.unwrap_or_default(),
        locked: segment.locked,
        net_classes: extras,
        net_id,
        net_name,
        operations: vec![operation],
    })
}

pub(super) fn track_arc_record(
    arc: PcbRoutingArc,
    net_classes: &BoardNetClassAssignments,
    budget: &mut BudgetTracker,
) -> Result<BoardTrackArcRecord, Error> {
    let (net_id, net_name) = net_parts(&arc.net);
    let extras = net_classes.extras_for_bounded(net_name.as_deref(), budget)?;
    // The Python serializer plots routing arcs from the file-order end point
    // back to the start point.
    let operation = PlotterOperation::ArcThreePoint(ArcThreePoint {
        start_x: mm_to_nm(arc.end.x)?,
        start_y: mm_to_nm(arc.end.y)?,
        mid_x: mm_to_nm(arc.mid.x)?,
        mid_y: mm_to_nm(arc.mid.y)?,
        end_x: mm_to_nm(arc.start.x)?,
        end_y: mm_to_nm(arc.start.y)?,
        fill: PlotterFill::NoFill,
        width_nm: mm_to_nm(arc.width.unwrap_or(0.0))?,
        layer: None,
        stroke_color: None,
        fill_color: None,
        line_style: None,
    });
    Ok(BoardTrackArcRecord {
        uuid: arc.uuid.unwrap_or_default(),
        layer: arc.layer.unwrap_or_default(),
        net_classes: extras,
        net_id,
        net_name,
        operations: vec![operation],
    })
}

/// Python `_via_exposed_mask_layers`: a side is exposed only when its
/// tenting option is explicitly `no` and the via reaches outer copper.
fn exposed_mask_layers(via: &PcbVia) -> Vec<&'static str> {
    let reaches_outer = |outer: &str| {
        via.layers
            .iter()
            .any(|layer| layer == outer || layer == "*.Cu")
    };
    let side = |tented: Option<bool>, outer: &str, mask: &'static str| {
        (tented == Some(false) && reaches_outer(outer)).then_some(mask)
    };
    let tenting_front = via.tenting.as_ref().and_then(|tenting| tenting.front);
    let tenting_back = via.tenting.as_ref().and_then(|tenting| tenting.back);
    side(tenting_front, "F.Cu", "F.Mask")
        .into_iter()
        .chain(side(tenting_back, "B.Cu", "B.Mask"))
        .collect()
}

pub(super) fn via_operation_count(via: &PcbVia) -> usize {
    2 + 2 * exposed_mask_layers(via).len()
}

pub(super) fn via_record(
    via: PcbVia,
    mask_clearance: f64,
    net_classes: &BoardNetClassAssignments,
    budget: &mut BudgetTracker,
) -> Result<BoardViaRecord, Error> {
    let (net_id, net_name) = net_parts(&via.net);
    let extras = net_classes.extras_for_bounded(net_name.as_deref(), budget)?;
    let x = mm_to_nm(via.at_x)?;
    let y = mm_to_nm(via.at_y)?;
    let size_nm = mm_to_nm(via.size)?;
    // Python falls back to half the pad size for missing or non-positive
    // drill diameters.
    let drill_mm = if via.drill > 0.0 {
        via.drill
    } else {
        via.size * 0.5
    };
    let drill_nm = mm_to_nm(drill_mm)?;
    let mut operations = vec![
        BoardViaOperation {
            kind: BoardViaOperationKind::Aperture,
            x,
            y,
            diameter_nm: size_nm,
            layers: via.layers.clone(),
        },
        BoardViaOperation {
            kind: BoardViaOperationKind::Drill,
            x,
            y,
            diameter_nm: drill_nm,
            layers: via.layers.clone(),
        },
    ];
    let opening_nm = mm_to_nm(via.size + 2.0 * mask_clearance)?;
    for mask in exposed_mask_layers(&via) {
        operations.push(BoardViaOperation {
            kind: BoardViaOperationKind::MaskOpening,
            x,
            y,
            diameter_nm: opening_nm,
            layers: vec![mask.to_owned()],
        });
        operations.push(BoardViaOperation {
            kind: BoardViaOperationKind::MaskDrill,
            x,
            y,
            diameter_nm: drill_nm,
            layers: vec![mask.to_owned()],
        });
    }
    Ok(BoardViaRecord {
        uuid: via.uuid.clone().unwrap_or_default(),
        layers: via.layers.clone(),
        drill: via.drill,
        size: via.size,
        via_type: match via.via_type.as_deref() {
            Some("blind") => BoardViaType::Blind,
            Some("buried") => BoardViaType::Buried,
            Some("micro") => BoardViaType::Micro,
            _ => BoardViaType::Through,
        },
        fabrication: BoardViaFabrication {
            tenting_front: via.tenting.as_ref().and_then(|value| value.front),
            tenting_back: via.tenting.as_ref().and_then(|value| value.back),
            covering_front: via.covering.as_ref().and_then(|value| value.front),
            covering_back: via.covering.as_ref().and_then(|value| value.back),
            plugging_front: via.plugging.as_ref().and_then(|value| value.front),
            plugging_back: via.plugging.as_ref().and_then(|value| value.back),
            capping: via.capping,
            filling: via.filling,
        },
        net_classes: extras,
        net_id,
        net_name,
        operations,
    })
}

pub(super) fn zone_record(
    zone: PcbZone,
    net_classes: &BoardNetClassAssignments,
    budget: &mut BudgetTracker,
) -> Result<BoardZoneRecord, Error> {
    let (net_id, net_name) = net_parts(&zone.net);
    let extras = net_classes.extras_for_bounded(net_name.as_deref(), budget)?;
    let mut operations = Vec::with_capacity(zone.filled_polygons.len());
    let mut fill_layers = Vec::with_capacity(zone.filled_polygons.len());
    let mut fill_island = Vec::with_capacity(zone.filled_polygons.len());
    for filled in &zone.filled_polygons {
        // Python emits one filled zero-width poly per `filled_polygon`
        // ring, empty point lists included.
        let points = filled
            .points
            .iter()
            .map(|point| Ok([mm_to_nm(point.x)?, mm_to_nm(point.y)?]))
            .collect::<Result<Vec<_>, Error>>()?;
        operations.push(PlotterOperation::PlotPoly(PlotterPoly {
            points,
            fill: PlotterFill::FilledShape,
            width_nm: 0,
            layer: None,
            stroke_color: None,
            fill_color: None,
            line_style: None,
        }));
        fill_layers.push(filled.layer.clone());
        fill_island.push(filled.island);
    }
    Ok(BoardZoneRecord {
        uuid: zone.uuid.clone().unwrap_or_default(),
        layers: zone.layers.clone(),
        fill_layers,
        fill_island,
        net_classes: extras,
        net_id,
        net_name,
        operations,
    })
}
