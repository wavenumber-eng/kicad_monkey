//! PCB-embedded footprint records in the established Python family order.

use super::layer_policy::BoardLayerFlashResolver;
use super::text::{
    BoardTextHAlign, BoardTextOperation, BoardTextRenderCacheCoordinateSpace, BoardTextVAlign,
};
use super::text_cache::{
    apply_knockout, attach_authored_cache_mapped, attach_native_cache_mapped,
    cache_is_valid_without_angle, parse_render_cache,
};
use super::{
    BoardFootprintBlock, BoardFootprintBlockAttributes, BoardFootprintChildAttributes,
    BoardFootprintChildMetadata, BoardFootprintOperation, BoardFootprintPlacement,
    BoardFootprintRecord, BoardNetClassAssignments, BoardPlotLimits, BoardPlotRecord,
    BoardTextVariables, BudgetTracker, input_point_limit_error, input_polygon_limit_error,
    limit_error, metadata_limit_error, normalize_input_limit_error, point_limit_error,
    text_limit_error,
};
use crate::footprint_plotter_text::{
    TextOperationInput, alignments, operation_from_effects, text_box_border_width,
};
use crate::pcb::{
    PcbFootprint, PcbFootprintGraphic, PcbFootprintProperty, PcbFootprintText, PcbFootprintTextBox,
    PcbGraphicKind, PcbHoleShape, PcbPad, PcbView,
};
use crate::plotter_ir::{
    FootprintPadParseLimits, footprint_graphic_operations_from_range,
    footprint_pad_operations_from_range, mm_to_nm,
};
use crate::plotter_text_cache::{PlotterTextCacheSession, PlotterTextLayout};
use crate::plotter_types::{
    PlotterFill, PlotterOperation, PlotterRect, PlotterText, PlotterTextHAlign, PlotterTextVAlign,
};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Limits, Position, parse_with_limits};
use crate::{TextHorizontalAlignment, TextVerticalAlignment};
use std::ops::Range;

struct FootprintInputs {
    footprint: PcbFootprint,
    properties: Vec<PcbFootprintProperty>,
    texts: Vec<PcbFootprintText>,
    text_boxes: Vec<PcbFootprintTextBox>,
    graphics: Vec<PcbFootprintGraphic>,
    pads: Vec<PcbPad>,
}

pub(super) struct FootprintAppendContext<'a, 'source, 'font> {
    pub source: &'source str,
    pub view: &'a PcbView<'source>,
    pub net_classes: &'a BoardNetClassAssignments,
    pub text_cache: Option<&'a PlotterTextCacheSession<'font>>,
    pub board_mask_clearance: f64,
    pub limits: BoardPlotLimits,
    pub layer_flash_resolver: &'a BoardLayerFlashResolver,
}

struct FootprintRenderContext<'a, 'font> {
    source: &'a str,
    footprint: &'a PcbFootprint,
    variables: &'a BoardTextVariables,
    placement: BoardFootprintPlacement,
    net_classes: &'a BoardNetClassAssignments,
    text_cache: Option<&'a PlotterTextCacheSession<'font>>,
    board_mask_clearance: f64,
    limits: BoardPlotLimits,
    layer_flash_resolver: &'a BoardLayerFlashResolver,
}

struct ChildDescriptor<'a> {
    source_uuid: Option<&'a str>,
    data_ref: &'a str,
    primitive: &'a str,
    object_id: &'a str,
    index: usize,
    sub_index: Option<usize>,
    layer: &'a str,
    text_role: Option<String>,
    property_name: Option<String>,
    fp_text_type: Option<String>,
    graphic_kind: Option<String>,
}

struct CacheDescriptor<'a> {
    range: Range<usize>,
    has_authored_cache: bool,
    effects: &'a crate::KiCadTextEffects,
    native_context: NativeCacheContext,
    knockout: bool,
}

pub(super) fn append_footprint_records(
    context: FootprintAppendContext<'_, '_, '_>,
    budget: &mut BudgetTracker,
    decoded_inputs: &mut (usize, usize),
    records: &mut Vec<BoardPlotRecord>,
) -> Result<(), Error> {
    let inputs = decode_inputs(
        context.view,
        context.limits,
        &mut decoded_inputs.0,
        &mut decoded_inputs.1,
    )?;
    for input in inputs {
        budget.ensure_capacity(0, 0)?;
        let record = footprint_record(&context, input, budget)?;
        records.push(BoardPlotRecord::Footprint(record));
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "bounded family collection is kept explicit"
)]
fn decode_inputs(
    view: &PcbView<'_>,
    limits: BoardPlotLimits,
    decoded_input_points: &mut usize,
    decoded_input_polygons: &mut usize,
) -> Result<Vec<FootprintInputs>, Error> {
    let footprints = view
        .footprints()
        .collect::<Result<Vec<_>, _>>()
        .map_err(normalize_input_limit_error)?;
    let mut inputs = footprints
        .into_iter()
        .map(|footprint| FootprintInputs {
            footprint,
            properties: Vec::new(),
            texts: Vec::new(),
            text_boxes: Vec::new(),
            graphics: Vec::new(),
            pads: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut carriers = inputs.len();
    let mut charge_carrier = || -> Result<(), Error> {
        carriers = carriers.checked_add(1).ok_or_else(limit_error)?;
        if carriers > limits.max_graphics {
            return Err(limit_error());
        }
        Ok(())
    };
    for property in view.footprint_properties() {
        let property = property.map_err(normalize_input_limit_error)?;
        charge_carrier()?;
        input_mut(&mut inputs, property.footprint_index)?
            .properties
            .push(property);
    }
    for text in view.footprint_texts() {
        let text = text.map_err(normalize_input_limit_error)?;
        charge_carrier()?;
        input_mut(&mut inputs, text.footprint_index)?
            .texts
            .push(text);
    }
    for text_box in view.footprint_text_boxes() {
        let text_box = text_box.map_err(normalize_input_limit_error)?;
        charge_carrier()?;
        *decoded_input_points = decoded_input_points
            .checked_add(text_box.polygon_points.len())
            .filter(|count| *count <= limits.max_input_points)
            .ok_or_else(input_point_limit_error)?;
        input_mut(&mut inputs, text_box.footprint_index)?
            .text_boxes
            .push(text_box);
    }
    for graphic in view.footprint_graphics() {
        let graphic = graphic.map_err(normalize_input_limit_error)?;
        charge_carrier()?;
        if graphic.graphic.kind == PcbGraphicKind::Poly {
            *decoded_input_polygons = decoded_input_polygons
                .checked_add(1)
                .filter(|count| *count <= limits.max_input_polygons)
                .ok_or_else(input_polygon_limit_error)?;
        }
        *decoded_input_points = decoded_input_points
            .checked_add(graphic.graphic.points.len())
            .filter(|count| *count <= limits.max_input_points)
            .ok_or_else(input_point_limit_error)?;
        input_mut(&mut inputs, graphic.footprint_index)?
            .graphics
            .push(graphic);
    }
    for pad in view.pads() {
        let pad = pad.map_err(normalize_input_limit_error)?;
        charge_carrier()?;
        let pad_polygons = pad
            .custom_primitives
            .iter()
            .filter(|primitive| primitive.kind == "gr_poly")
            .count();
        let pad_points = pad
            .custom_primitives
            .iter()
            .try_fold(0usize, |count, primitive| {
                count.checked_add(primitive.points.len())
            })
            .ok_or_else(input_point_limit_error)?;
        *decoded_input_polygons = decoded_input_polygons
            .checked_add(pad_polygons)
            .filter(|count| *count <= limits.max_input_polygons)
            .ok_or_else(input_polygon_limit_error)?;
        *decoded_input_points = decoded_input_points
            .checked_add(pad_points)
            .filter(|count| *count <= limits.max_input_points)
            .ok_or_else(input_point_limit_error)?;
        input_mut(&mut inputs, pad.footprint_index)?.pads.push(pad);
    }
    Ok(inputs)
}

fn input_mut(inputs: &mut [FootprintInputs], index: usize) -> Result<&mut FootprintInputs, Error> {
    inputs.get_mut(index).ok_or_else(|| {
        Error::at(
            ErrorPhase::Tree,
            ErrorKind::InvalidBuildValue,
            "Footprint child refers to an unknown parent",
            Position::START,
        )
    })
}

fn footprint_record(
    outer: &FootprintAppendContext<'_, '_, '_>,
    input: FootprintInputs,
    budget: &mut BudgetTracker,
) -> Result<BoardFootprintRecord, Error> {
    let FootprintInputs {
        footprint,
        properties,
        texts,
        text_boxes,
        graphics,
        pads,
    } = input;
    let x = footprint.at_x.unwrap_or(0.0);
    let y = footprint.at_y.unwrap_or(0.0);
    let angle = finite(footprint.angle.unwrap_or(0.0), "footprint placement angle")?;
    let placement = BoardFootprintPlacement {
        x_nm: mm_to_nm(x)?,
        y_nm: mm_to_nm(y)?,
        angle_deg: angle,
    };
    charge_record_metadata(&footprint, budget)?;
    let variables = BoardTextVariables::from_entries(
        properties
            .iter()
            .map(|property| (&property.name, &property.value)),
    );
    let mut operations = Vec::new();
    let context = FootprintRenderContext {
        source: outer.source,
        footprint: &footprint,
        variables: &variables,
        placement,
        net_classes: outer.net_classes,
        text_cache: outer.text_cache,
        board_mask_clearance: outer.board_mask_clearance,
        limits: outer.limits,
        layer_flash_resolver: outer.layer_flash_resolver,
    };
    append_properties(&context, &properties, &mut operations, budget)?;
    append_texts(&context, &texts, &mut operations, budget)?;
    append_text_boxes(&context, &text_boxes, &mut operations, budget)?;
    append_graphics(&context, &graphics, &mut operations, budget)?;
    append_pads(&context, &pads, &mut operations, budget)?;
    Ok(BoardFootprintRecord {
        uuid: footprint.uuid.unwrap_or_default(),
        library_link: footprint.library_link,
        reference: footprint.reference.unwrap_or_default(),
        value: footprint.value.unwrap_or_default(),
        layer: footprint.layer.unwrap_or_else(|| "F.Cu".to_owned()),
        locked: footprint.locked,
        descr: footprint.description,
        tags: footprint.tags,
        attr: footprint.attributes,
        placement,
        operations,
    })
}

fn append_properties(
    context: &FootprintRenderContext<'_, '_>,
    properties: &[PcbFootprintProperty],
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let reference = properties
        .iter()
        .position(|property| property.name == "Reference");
    let value = properties
        .iter()
        .position(|property| property.name == "Value");
    let ordered = reference.into_iter().chain(value).chain(
        properties
            .iter()
            .enumerate()
            .filter(|(_, property)| !matches!(property.name.as_str(), "Reference" | "Value"))
            .map(|(index, _)| index),
    );
    for (object_index, property_index) in ordered.enumerate() {
        let property = &properties[property_index];
        if property.hidden || property.value.is_empty() || !property.graphical {
            continue;
        }
        let mut text = property.value.clone();
        if property.render_cache_range.is_some() || property.effects.font.face.is_some() {
            text = context
                .variables
                .substitute_bounded(&text, budget.remaining_text_bytes()?)?;
        }
        let plotter = operation_from_effects(
            text,
            TextOperationInput {
                x: property.at.x,
                y: property.at.y,
                angle: property.angle,
                layer: &property.layer,
                effects: &property.effects,
                default_h: PlotterTextHAlign::Left,
                default_v: PlotterTextVAlign::Bottom,
                multiline: false,
            },
        )?;
        let mut operation = board_text_operation(plotter);
        attach_footprint_cache(
            context,
            CacheDescriptor {
                range: property.source_range.clone(),
                has_authored_cache: property.render_cache_range.is_some(),
                effects: &property.effects,
                native_context: NativeCacheContext::Simple {
                    unlocked: property.unlocked,
                },
                knockout: false,
            },
            &mut operation,
            budget,
        )?;
        append_child_text(
            context,
            ChildDescriptor {
                source_uuid: property.uuid.as_deref(),
                data_ref: "property",
                primitive: "footprint-text",
                object_id: &property.name,
                index: object_index,
                sub_index: None,
                layer: &property.layer,
                text_role: Some(text_role(&property.name, Some(&property.name))),
                property_name: Some(property.name.clone()),
                fp_text_type: None,
                graphic_kind: None,
            },
            operation,
            operations,
            budget,
        )?;
    }
    Ok(())
}

fn append_texts(
    context: &FootprintRenderContext<'_, '_>,
    texts: &[PcbFootprintText],
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    for (index, text) in texts.iter().enumerate() {
        if text.hidden {
            continue;
        }
        let raw = match text.kind.as_str() {
            "reference" => context
                .variables
                .get("Reference")
                .or_else(|| context.variables.get("REFERENCE"))
                .unwrap_or(&text.text),
            "value" => context
                .variables
                .get("Value")
                .or_else(|| context.variables.get("VALUE"))
                .unwrap_or(&text.text),
            _ => &text.text,
        };
        let resolved = context
            .variables
            .substitute_bounded(raw, budget.remaining_text_bytes()?)?;
        if resolved.is_empty() {
            continue;
        }
        let plotter = operation_from_effects(
            resolved,
            TextOperationInput {
                x: text.at.x,
                y: text.at.y,
                angle: text.angle,
                layer: &text.layer,
                effects: &text.effects,
                default_h: PlotterTextHAlign::Left,
                default_v: PlotterTextVAlign::Bottom,
                multiline: false,
            },
        )?;
        let mut operation = board_text_operation(plotter);
        attach_footprint_cache(
            context,
            CacheDescriptor {
                range: text.source_range.clone(),
                has_authored_cache: text.render_cache_range.is_some(),
                effects: &text.effects,
                native_context: NativeCacheContext::Simple {
                    unlocked: text.unlocked,
                },
                knockout: text.knockout,
            },
            &mut operation,
            budget,
        )?;
        let object_id = if text.kind.is_empty() {
            index.to_string()
        } else {
            format!("{}:{index}", text.kind)
        };
        append_child_text(
            context,
            ChildDescriptor {
                source_uuid: text.uuid.as_deref(),
                data_ref: "fp_text",
                primitive: "footprint-text",
                object_id: &object_id,
                index,
                sub_index: None,
                layer: &text.layer,
                text_role: Some(text_role(&text.kind, None)),
                property_name: None,
                fp_text_type: Some(text.kind.clone()),
                graphic_kind: None,
            },
            operation,
            operations,
            budget,
        )?;
    }
    Ok(())
}

fn append_text_boxes(
    context: &FootprintRenderContext<'_, '_>,
    text_boxes: &[PcbFootprintTextBox],
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    for (index, text_box) in text_boxes.iter().enumerate() {
        let local = text_box_operations(context, text_box, budget)?;
        for (sub_index, operation) in local.into_iter().enumerate() {
            let object_id = format!("text_box:{index}");
            match operation {
                EitherOperation::Text(operation) => append_child_text(
                    context,
                    ChildDescriptor {
                        source_uuid: text_box.uuid.as_deref(),
                        data_ref: "fp_text_box",
                        primitive: "footprint-text",
                        object_id: &object_id,
                        index,
                        sub_index: Some(sub_index),
                        layer: &text_box.layer,
                        text_role: Some("user".to_owned()),
                        property_name: None,
                        fp_text_type: None,
                        graphic_kind: None,
                    },
                    operation,
                    operations,
                    budget,
                )?,
                EitherOperation::Geometry(operation) => append_child_geometry(
                    context,
                    ChildDescriptor {
                        source_uuid: text_box.uuid.as_deref(),
                        data_ref: "fp_text_box",
                        primitive: "footprint-graphic",
                        object_id: &object_id,
                        index,
                        sub_index: Some(sub_index),
                        layer: &text_box.layer,
                        text_role: None,
                        property_name: None,
                        fp_text_type: None,
                        graphic_kind: Some("text-box-border".to_owned()),
                    },
                    operation,
                    operations,
                    budget,
                )?,
            }
        }
    }
    Ok(())
}

fn text_box_operations(
    context: &FootprintRenderContext<'_, '_>,
    text_box: &PcbFootprintTextBox,
    budget: &BudgetTracker,
) -> Result<Vec<EitherOperation>, Error> {
    let mut local = Vec::with_capacity(2);
    if text_box.border.unwrap_or(false) {
        local.push(EitherOperation::Geometry(PlotterOperation::Rect(
            PlotterRect {
                x1: mm_to_nm(text_box.start.x)?,
                y1: mm_to_nm(text_box.start.y)?,
                x2: mm_to_nm(text_box.end.x)?,
                y2: mm_to_nm(text_box.end.y)?,
                fill: PlotterFill::NoFill,
                width_nm: text_box_border_width(text_box.stroke_width)?,
                corner_radius_nm: 0,
                layer: Some(text_box.layer.clone()),
                stroke_color: None,
                fill_color: None,
                line_style: None,
            },
        )));
    }
    if text_box.text.is_empty() {
        return Ok(local);
    }
    let effects = text_box.effects.clone().unwrap_or_default();
    let (authored_h, authored_v) = alignments(&effects);
    let h = authored_h.unwrap_or(PlotterTextHAlign::Left);
    let v = authored_v.unwrap_or(PlotterTextVAlign::Top);
    let x1 = text_box.start.x.min(text_box.end.x);
    let y1 = text_box.start.y.min(text_box.end.y);
    let x2 = text_box.start.x.max(text_box.end.x);
    let y2 = text_box.start.y.max(text_box.end.y);
    let [left, top, right, bottom] = text_box.margins;
    let x = match h {
        PlotterTextHAlign::Right => x2 - right,
        PlotterTextHAlign::Center => (x1 + x2) / 2.0,
        PlotterTextHAlign::Left => x1 + left,
    };
    let y = match v {
        PlotterTextVAlign::Bottom => y2 - bottom,
        PlotterTextVAlign::Center => (y1 + y2) / 2.0,
        PlotterTextVAlign::Top => y1 + top,
    };
    let resolved = context
        .variables
        .substitute_bounded(&text_box.text, budget.remaining_text_bytes()?)?;
    let size_x_nm = mm_to_nm(effects.font.size_x)?;
    let wrap_size_x_nm = if size_x_nm == 0 { 1_270_000 } else { size_x_nm };
    let wrapped = super::text_wrap::wrap_text_box(
        &resolved,
        ((x2 - x1) - left - right).max(0.0),
        wrap_size_x_nm,
    );
    let multiline = wrapped.contains('\n') || resolved.contains('\n');
    let plotter = operation_from_effects(
        wrapped,
        TextOperationInput {
            x,
            y,
            angle: text_box.angle,
            layer: &text_box.layer,
            effects: &effects,
            default_h: h,
            default_v: v,
            multiline,
        },
    )?;
    let mut operation = board_text_operation(plotter);
    attach_footprint_cache(
        context,
        CacheDescriptor {
            range: text_box.source_range.clone(),
            has_authored_cache: text_box.render_cache_range.is_some(),
            effects: &effects,
            native_context: NativeCacheContext::TextBox,
            knockout: text_box.knockout.unwrap_or(false),
        },
        &mut operation,
        budget,
    )?;
    local.push(EitherOperation::Text(operation));
    Ok(local)
}

enum EitherOperation {
    Geometry(PlotterOperation),
    Text(BoardTextOperation),
}

fn append_graphics(
    context: &FootprintRenderContext<'_, '_>,
    graphics: &[PcbFootprintGraphic],
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let families = [
        (PcbGraphicKind::Line, "fp_line", "line"),
        (PcbGraphicKind::Arc, "fp_arc", "arc"),
        (PcbGraphicKind::Circle, "fp_circle", "circle"),
        (PcbGraphicKind::Rect, "fp_rect", "rect"),
        (PcbGraphicKind::Poly, "fp_poly", "poly"),
    ];
    for (kind, data_ref, graphic_kind) in families {
        for (index, graphic) in graphics
            .iter()
            .filter(|item| item.graphic.kind == kind)
            .enumerate()
        {
            let remaining_operations = budget.remaining_operations()?;
            let additions = footprint_graphic_operations_from_range(
                context.source,
                graphic.graphic.source_range.clone(),
                data_ref,
                remaining_operations,
                budget.remaining_points()?,
                context.limits.max_depth,
                context.limits.max_parse_nodes,
            )?;
            let layer = graphic.graphic.layer.as_deref().unwrap_or("F.SilkS");
            for (sub_index, operation) in additions.into_iter().enumerate() {
                let metadata_sub_index =
                    matches!(kind, PcbGraphicKind::Line | PcbGraphicKind::Arc).then_some(sub_index);
                append_child_geometry(
                    context,
                    ChildDescriptor {
                        source_uuid: graphic.graphic.uuid.as_deref(),
                        data_ref,
                        primitive: "footprint-graphic",
                        object_id: &format!("{graphic_kind}:{index}"),
                        index,
                        sub_index: metadata_sub_index,
                        layer,
                        text_role: None,
                        property_name: None,
                        fp_text_type: None,
                        graphic_kind: Some(graphic_kind.to_owned()),
                    },
                    operation,
                    operations,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

fn append_pads(
    context: &FootprintRenderContext<'_, '_>,
    pads: &[PcbPad],
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    for (index, pad) in pads.iter().enumerate() {
        let margin = pad
            .solder_mask_margin
            .or(context.footprint.solder_mask_margin)
            .unwrap_or(context.board_mask_clearance);
        let parsed = footprint_pad_operations_from_range(
            context.source,
            pad.source_range.clone(),
            margin,
            -context.footprint.angle.unwrap_or(0.0),
            FootprintPadParseLimits {
                max_operations: budget.remaining_operations()?,
                max_points: budget.remaining_points()?,
                max_depth: context.limits.max_depth,
                max_nodes: context.limits.max_parse_nodes,
            },
        )?;
        if !parsed.flash.is_empty() {
            append_pad_block(context, pad, index, false, parsed.flash, operations, budget)?;
        }
        if !parsed.drill.is_empty() {
            append_pad_block(context, pad, index, true, parsed.drill, operations, budget)?;
        }
    }
    Ok(())
}

fn append_pad_block(
    context: &FootprintRenderContext<'_, '_>,
    pad: &PcbPad,
    index: usize,
    hole: bool,
    mut payloads: Vec<PlotterOperation>,
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let Some((layers, operation_layers)) = pad_block_layers(context, pad, hole) else {
        return Ok(());
    };
    for operation in &mut payloads {
        set_pad_operation_layers(operation, &operation_layers);
    }
    let inner_layers = &operation_layers;
    let label = pad_label(context.footprint, pad, index, hole);
    let base_label = pad_label(context.footprint, pad, index, false);
    let component = context.footprint.reference.as_deref().unwrap_or("");
    let number = &pad.number;
    let classes = context
        .net_classes
        .extras_for_bounded(pad.net.name.as_deref(), budget)?;
    let pad_designator = if !component.is_empty() && !number.is_empty() {
        format!("{component}-{number}")
    } else {
        number.clone()
    };
    let attrs = BoardFootprintBlockAttributes {
        primitive: if hole { "pad-hole" } else { "pad" }.to_owned(),
        component: nonempty(component),
        component_uid: nonempty(context.footprint.uuid.as_deref().unwrap_or("")),
        component_uuid: nonempty(context.footprint.uuid.as_deref().unwrap_or("")),
        footprint: nonempty(&context.footprint.library_link),
        pad_number: nonempty(number),
        pad_designator: nonempty(&pad_designator),
        pad_type: nonempty(&pad.kind),
        pad_shape: nonempty(&pad.shape),
        layer_names: nonempty(&inner_layers.join(",")),
        net_index: pad.net.ordinal.map(|value| value.to_string()),
        net_id: pad.net.ordinal.map(|value| value.to_string()),
        net: pad.net.name.as_deref().and_then(nonempty),
        net_class: classes.net_class.as_deref().and_then(nonempty),
        net_classes: nonempty(&classes.net_classes.join(",")),
        hole_owner: hole.then_some(base_label),
        hole_kind: hole.then(|| hole_kind(pad).to_owned()),
        hole_plating: hole.then(|| {
            if pad.kind == "np_thru_hole" {
                "non_plated"
            } else {
                "plated"
            }
            .to_owned()
        }),
        hole_render: hole.then_some("drill".to_owned()),
        hole_diameter_mm: hole
            .then(|| hole_dimension(pad, HoleDimension::Diameter))
            .flatten(),
        hole_width_mm: hole
            .then(|| hole_dimension(pad, HoleDimension::Width))
            .flatten(),
        hole_height_mm: hole
            .then(|| hole_dimension(pad, HoleDimension::Height))
            .flatten(),
    };
    let data_ref = if hole { "pad_hole" } else { "pad" };
    let object_id = if number.is_empty() { "pad" } else { number };
    charge_block_metadata(&label, data_ref, object_id, &layers, &attrs, budget)?;
    let payload_count = payloads.len();
    let payload_points = operation_points(&payloads);
    budget.ensure_capacity(payload_count.saturating_add(2), payload_points)?;
    operations.push(BoardFootprintOperation::StartBlock(BoardFootprintBlock {
        label: label.clone(),
        data_uuid: label,
        data_ref: data_ref.to_owned(),
        object_id: object_id.to_owned(),
        layers,
        extra_attrs: attrs,
    }));
    operations.extend(payloads.into_iter().map(BoardFootprintOperation::Pad));
    operations.push(BoardFootprintOperation::EndBlock);
    budget.charge(payload_count.saturating_add(2), payload_points)?;
    Ok(())
}

fn pad_block_layers(
    context: &FootprintRenderContext<'_, '_>,
    pad: &PcbPad,
    hole: bool,
) -> Option<(Vec<String>, Vec<String>)> {
    let layers = match (hole, pad.kind.as_str()) {
        (true, "np_thru_hole") => Vec::new(),
        (true, _) => context.layer_flash_resolver.pad_drill_layers(pad),
        (false, _) => context.layer_flash_resolver.pad_item_layers(pad),
    };
    let legacy_unlayered_flash = pad.layers.is_empty() && pad.remove_unused_layers != Some(true);
    if !hole && layers.is_empty() && !legacy_unlayered_flash {
        return None;
    }
    let operation_layers = if hole && pad.kind == "np_thru_hole" {
        pad.layers.clone()
    } else {
        layers.clone()
    };
    Some((layers, operation_layers))
}

fn set_pad_operation_layers(operation: &mut PlotterOperation, layers: &[String]) {
    let target = match operation {
        PlotterOperation::FlashPadCircle(value) => &mut value.layers,
        PlotterOperation::FlashPadOval(value) => &mut value.layers,
        PlotterOperation::FlashPadRect(value) => &mut value.layers,
        PlotterOperation::FlashPadRoundRect(value) => &mut value.layers,
        PlotterOperation::FlashPadCustom(value) => &mut value.layers,
        PlotterOperation::FlashPadTrapez(value) => &mut value.layers,
        PlotterOperation::Circle(value) => &mut value.layers,
        PlotterOperation::ThickSegment(value) => &mut value.layers,
        _ => return,
    };
    layers.clone_into(target);
}

fn append_child_text(
    context: &FootprintRenderContext<'_, '_>,
    descriptor: ChildDescriptor<'_>,
    operation: BoardTextOperation,
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let metadata = child_metadata(context.footprint, descriptor);
    let points = cache_point_count(&operation)?;
    budget.ensure_capacity(1, points)?;
    budget.charge_text(retained_text_bytes(&operation)?)?;
    charge_child_metadata(&metadata, budget)?;
    operations.push(BoardFootprintOperation::Text {
        operation,
        metadata,
    });
    budget.charge(1, points)
}

fn append_child_geometry(
    context: &FootprintRenderContext<'_, '_>,
    descriptor: ChildDescriptor<'_>,
    operation: PlotterOperation,
    operations: &mut Vec<BoardFootprintOperation>,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let metadata = child_metadata(context.footprint, descriptor);
    let points = operation_point_count(&operation);
    budget.ensure_capacity(1, points)?;
    charge_child_metadata(&metadata, budget)?;
    operations.push(BoardFootprintOperation::Geometry {
        operation,
        metadata,
    });
    budget.charge(1, points)
}

fn child_metadata(
    footprint: &PcbFootprint,
    descriptor: ChildDescriptor<'_>,
) -> BoardFootprintChildMetadata {
    let ChildDescriptor {
        source_uuid,
        data_ref,
        primitive,
        object_id,
        index,
        sub_index,
        layer,
        text_role,
        property_name,
        fp_text_type,
        graphic_kind,
    } = descriptor;
    let owner = footprint
        .uuid
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if footprint.library_link.is_empty() {
                "footprint"
            } else {
                &footprint.library_link
            }
        });
    let source_uuid = source_uuid.filter(|value| !value.is_empty());
    let suffix = sub_index.map_or_else(String::new, |value| format!(":{value}"));
    let label = format!(
        "{}:{primitive}:{index}{suffix}",
        source_uuid.unwrap_or(owner)
    );
    BoardFootprintChildMetadata {
        data_uuid: source_uuid.unwrap_or(&label).to_owned(),
        label,
        data_ref: data_ref.to_owned(),
        object_id: object_id.to_owned(),
        extra_attrs: BoardFootprintChildAttributes {
            component: footprint.reference.clone().unwrap_or_default(),
            component_uid: footprint.uuid.clone().unwrap_or_default(),
            component_uuid: footprint.uuid.clone().unwrap_or_default(),
            footprint: footprint.library_link.clone(),
            layer_name: (!layer.is_empty()).then(|| layer.to_owned()),
            layer_role: (!layer.is_empty()).then(|| layer_role(layer).to_owned()),
            primitive: primitive.to_owned(),
            footprint_primitive: data_ref.to_owned(),
            footprint_object_index: index,
            footprint_subop_index: sub_index,
            footprint_text_role: text_role,
            property_name,
            fp_text_type,
            footprint_graphic_kind: graphic_kind,
        },
    }
}

fn board_text_operation(value: PlotterText) -> BoardTextOperation {
    BoardTextOperation {
        x: value.x,
        y: value.y,
        text: value.text,
        color: value.color,
        orient_deg: value.orient_deg,
        size_x_nm: value.size_x_nm,
        size_y_nm: value.size_y_nm,
        h_align: match value.h_align {
            PlotterTextHAlign::Left => BoardTextHAlign::Left,
            PlotterTextHAlign::Center => BoardTextHAlign::Center,
            PlotterTextHAlign::Right => BoardTextHAlign::Right,
        },
        v_align: match value.v_align {
            PlotterTextVAlign::Top => BoardTextVAlign::Top,
            PlotterTextVAlign::Center => BoardTextVAlign::Center,
            PlotterTextVAlign::Bottom => BoardTextVAlign::Bottom,
        },
        pen_width_nm: value.pen_width_nm,
        italic: value.italic,
        bold: value.bold,
        multiline: value.multiline,
        font_face: value.font_face,
        layer: value.layer,
        mirror: false,
        text_as_polygons: false,
        polyline_per_segment: false,
        knockout: false,
        render_cache_polygons: Vec::new(),
        render_cache: None,
    }
}

fn attach_footprint_cache(
    context: &FootprintRenderContext<'_, '_>,
    descriptor: CacheDescriptor<'_>,
    operation: &mut BoardTextOperation,
    budget: &BudgetTracker,
) -> Result<(), Error> {
    let CacheDescriptor {
        range,
        has_authored_cache,
        effects,
        native_context,
        knockout,
    } = descriptor;
    let cache = if has_authored_cache {
        let form = parse_selected(context.source, range, context.limits)?;
        parse_render_cache(
            &form,
            context.limits.max_input_points,
            context.limits.max_cache_polygons,
            context.limits.max_cache_contours,
        )?
    } else {
        None
    };
    let valid = cache
        .as_ref()
        .filter(|cache| cache_is_valid_without_angle(cache, &operation.text));
    if let Some(cache) = valid {
        budget.ensure_text_capacity(
            operation
                .text
                .len()
                .checked_add(cache.text().map_or(0, str::len))
                .ok_or_else(text_limit_error)?,
        )?;
        attach_authored_cache_mapped(
            operation,
            cache,
            false,
            budget.remaining_points()?,
            knockout,
            BoardTextRenderCacheCoordinateSpace::FootprintLocal,
            |x, y| footprint_local_nm(x, y, context.placement),
        )?;
    } else if effects.font.face.is_some()
        && let Some(resources) = context.text_cache
    {
        if !operation.text.trim().is_empty() {
            budget.ensure_text_capacity(
                operation
                    .text
                    .len()
                    .checked_mul(2)
                    .ok_or_else(text_limit_error)?,
            )?;
        }
        let layout = footprint_native_layout(operation, effects, context.placement, native_context);
        let generated = resources.generate(
            layout,
            budget.remaining_points()?,
            context.limits.max_cache_polygons,
            context.limits.max_cache_contours,
        )?;
        attach_native_cache_mapped(
            operation,
            generated,
            budget.remaining_points()?,
            BoardTextRenderCacheCoordinateSpace::FootprintLocal,
            |x, y| footprint_local_nm(x, y, context.placement),
        )?;
    }
    if knockout {
        let margin = (effects.font.thickness.unwrap_or(0.0) / 2.0).max(effects.font.size_y / 9.0);
        apply_knockout(
            operation,
            mm_to_nm(margin)?,
            budget.remaining_points()?,
            context.limits.max_cache_contours,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum NativeCacheContext {
    Simple { unlocked: bool },
    TextBox,
}

fn footprint_native_layout<'a>(
    operation: &'a BoardTextOperation,
    effects: &'a crate::KiCadTextEffects,
    placement: BoardFootprintPlacement,
    context: NativeCacheContext,
) -> PlotterTextLayout<'a> {
    let local_x = operation.x as f64 / 1_000_000.0;
    let local_y = operation.y as f64 / 1_000_000.0;
    let angle = placement.angle_deg.to_radians();
    let board_x =
        local_x * angle.cos() + local_y * angle.sin() + placement.x_nm as f64 / 1_000_000.0;
    let board_y =
        local_y * angle.cos() - local_x * angle.sin() + placement.y_nm as f64 / 1_000_000.0;
    let (position_x, position_y, angle_degrees, horizontal_alignment, vertical_alignment) =
        match context {
            NativeCacheContext::Simple { unlocked } => {
                let (h, v) = native_effect_alignments(effects);
                (
                    board_x,
                    board_y,
                    if unlocked {
                        operation.orient_deg
                    } else {
                        keep_upright_angle(operation.orient_deg)
                    },
                    h,
                    v,
                )
            }
            // Text-box source geometry is rotated into board coordinates by
            // Python before its ordinary board text-box layout runs. The
            // operation anchor is an exact local representation of that
            // draw point; composing placement here preserves it.
            NativeCacheContext::TextBox => (
                board_x,
                board_y,
                (operation.orient_deg + placement.angle_deg).rem_euclid(360.0),
                native_h(operation.h_align),
                native_v(operation.v_align),
            ),
        };
    PlotterTextLayout {
        text: &operation.text,
        face: &operation.font_face,
        bold: operation.bold,
        italic: operation.italic,
        size_x: effects.font.size_x,
        size_y: effects.font.size_y,
        position_x,
        position_y,
        angle_degrees,
        mirrored: effects.justify.iter().any(|token| token == "mirror"),
        horizontal_alignment,
        vertical_alignment,
        line_spacing: effects.font.line_spacing.unwrap_or(1.0),
        stroke_width: effects.font.thickness.unwrap_or(0.0),
    }
}

fn native_effect_alignments(
    effects: &crate::KiCadTextEffects,
) -> (TextHorizontalAlignment, TextVerticalAlignment) {
    let (horizontal, vertical) = alignments(effects);
    (
        horizontal.map_or(TextHorizontalAlignment::Center, |value| {
            native_h(match value {
                PlotterTextHAlign::Left => BoardTextHAlign::Left,
                PlotterTextHAlign::Center => BoardTextHAlign::Center,
                PlotterTextHAlign::Right => BoardTextHAlign::Right,
            })
        }),
        vertical.map_or(TextVerticalAlignment::Center, |value| {
            native_v(match value {
                PlotterTextVAlign::Top => BoardTextVAlign::Top,
                PlotterTextVAlign::Center => BoardTextVAlign::Center,
                PlotterTextVAlign::Bottom => BoardTextVAlign::Bottom,
            })
        }),
    )
}

fn keep_upright_angle(angle: f64) -> f64 {
    let mut angle = angle.rem_euclid(360.0);
    while angle > 90.0 {
        angle -= 180.0;
    }
    while angle <= -90.0 {
        angle += 180.0;
    }
    angle
}

fn parse_selected(
    source: &str,
    range: Range<usize>,
    limits: BoardPlotLimits,
) -> Result<crate::sexpr::Sexp, Error> {
    let selected = source.get(range).ok_or_else(|| {
        Error::at(
            ErrorPhase::Tree,
            ErrorKind::InvalidSpan,
            "Footprint child span is outside source",
            Position::START,
        )
    })?;
    parse_with_limits(
        selected,
        Limits {
            max_source_bytes: selected.len(),
            max_depth: limits.max_depth,
            max_nodes: limits.max_parse_nodes,
            max_decoded_string_bytes: selected.len(),
        },
    )
}

fn footprint_local_nm(
    x: f64,
    y: f64,
    placement: BoardFootprintPlacement,
) -> Result<[i64; 2], Error> {
    let origin_x = placement.x_nm as f64 / 1_000_000.0;
    let origin_y = placement.y_nm as f64 / 1_000_000.0;
    let dx = x - origin_x;
    let dy = y - origin_y;
    let angle = placement.angle_deg.to_radians();
    Ok([
        mm_to_nm(dx * angle.cos() - dy * angle.sin())?,
        mm_to_nm(dx * angle.sin() + dy * angle.cos())?,
    ])
}

fn cache_point_count(operation: &BoardTextOperation) -> Result<usize, Error> {
    operation.render_cache.as_ref().map_or(Ok(0), |cache| {
        cache.polygons.iter().try_fold(0usize, |total, polygon| {
            let retained = polygon.iter().try_fold(0usize, |count, contour| {
                count
                    .checked_add(contour.len())
                    .ok_or_else(point_limit_error)
            })?;
            let exterior = polygon.first().map_or(0, Vec::len);
            total
                .checked_add(retained)
                .and_then(|value| value.checked_add(exterior))
                .ok_or_else(point_limit_error)
        })
    })
}

fn retained_text_bytes(operation: &BoardTextOperation) -> Result<usize, Error> {
    operation
        .render_cache
        .as_ref()
        .map_or(Some(operation.text.len()), |cache| {
            operation.text.len().checked_add(cache.text.len())
        })
        .ok_or_else(text_limit_error)
}

fn operation_points(operations: &[PlotterOperation]) -> usize {
    operations.iter().map(operation_point_count).sum()
}

fn operation_point_count(operation: &PlotterOperation) -> usize {
    match operation {
        PlotterOperation::PlotPoly(value) => value.points.len(),
        PlotterOperation::FlashPadCustom(value) => value.polygons.iter().map(Vec::len).sum(),
        _ => 0,
    }
}

fn charge_record_metadata(
    footprint: &PcbFootprint,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let bytes = [
        footprint.library_link.len(),
        footprint.reference.as_ref().map_or(0, String::len),
        footprint.value.as_ref().map_or(0, String::len),
        footprint.layer.as_deref().unwrap_or("F.Cu").len(),
        footprint.uuid.as_ref().map_or(0, String::len),
        footprint.description.len(),
        footprint.tags.len(),
    ]
    .into_iter()
    .chain(footprint.attributes.iter().map(String::len))
    .try_fold(0usize, usize::checked_add)
    .ok_or_else(metadata_limit_error)?;
    budget.charge_metadata(bytes)
}

fn charge_child_metadata(
    metadata: &BoardFootprintChildMetadata,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let attrs = &metadata.extra_attrs;
    let bytes = [
        metadata.label.len(),
        metadata.data_uuid.len(),
        metadata.data_ref.len(),
        metadata.object_id.len(),
        attrs.component.len(),
        attrs.component_uid.len(),
        attrs.component_uuid.len(),
        attrs.footprint.len(),
        attrs.primitive.len(),
        attrs.footprint_primitive.len(),
    ]
    .into_iter()
    .chain(
        [
            attrs.layer_name.as_ref(),
            attrs.layer_role.as_ref(),
            attrs.footprint_text_role.as_ref(),
            attrs.property_name.as_ref(),
            attrs.fp_text_type.as_ref(),
            attrs.footprint_graphic_kind.as_ref(),
        ]
        .into_iter()
        .flatten()
        .map(String::len),
    )
    .try_fold(0usize, usize::checked_add)
    .ok_or_else(metadata_limit_error)?;
    budget.charge_metadata(bytes)
}

fn charge_block_metadata(
    label: &str,
    data_ref: &str,
    object_id: &str,
    layers: &[String],
    attrs: &BoardFootprintBlockAttributes,
    budget: &mut BudgetTracker,
) -> Result<(), Error> {
    let bytes = label
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(data_ref.len()))
        .and_then(|value| value.checked_add(object_id.len()))
        .and_then(|value| value.checked_add(attrs.primitive.len()))
        .and_then(|value| {
            layers
                .iter()
                .try_fold(value, |total, layer| total.checked_add(layer.len()))
        })
        .and_then(|value| {
            block_values(attrs).try_fold(value, |total, item| total.checked_add(item.len()))
        })
        .ok_or_else(metadata_limit_error)?;
    budget.charge_metadata(bytes)
}

fn block_values(attrs: &BoardFootprintBlockAttributes) -> impl Iterator<Item = &String> {
    [
        attrs.component.as_ref(),
        attrs.component_uid.as_ref(),
        attrs.component_uuid.as_ref(),
        attrs.footprint.as_ref(),
        attrs.pad_number.as_ref(),
        attrs.pad_designator.as_ref(),
        attrs.pad_type.as_ref(),
        attrs.pad_shape.as_ref(),
        attrs.layer_names.as_ref(),
        attrs.net_index.as_ref(),
        attrs.net_id.as_ref(),
        attrs.net.as_ref(),
        attrs.net_class.as_ref(),
        attrs.net_classes.as_ref(),
        attrs.hole_owner.as_ref(),
        attrs.hole_kind.as_ref(),
        attrs.hole_plating.as_ref(),
        attrs.hole_render.as_ref(),
        attrs.hole_diameter_mm.as_ref(),
        attrs.hole_width_mm.as_ref(),
        attrs.hole_height_mm.as_ref(),
    ]
    .into_iter()
    .flatten()
}

fn pad_label(footprint: &PcbFootprint, pad: &PcbPad, index: usize, hole: bool) -> String {
    let base = pad
        .uuid
        .clone()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let owner = footprint
                .uuid
                .as_deref()
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| {
                    if footprint.library_link.is_empty() {
                        "footprint"
                    } else {
                        &footprint.library_link
                    }
                });
            let number = if pad.number.is_empty() {
                index.to_string()
            } else {
                pad.number.clone()
            };
            format!("{owner}:pad:{index}:{number}")
        });
    if hole { format!("{base}:hole") } else { base }
}

enum HoleDimension {
    Diameter,
    Width,
    Height,
}

fn hole_dimension(pad: &PcbPad, dimension: HoleDimension) -> Option<String> {
    let Some(drill) = pad.drill.as_ref() else {
        return matches!(dimension, HoleDimension::Diameter)
            .then_some(pad.size_x.min(pad.size_y))
            .filter(|value| pad.kind == "np_thru_hole" && *value > 0.0)
            .map(python_float);
    };
    match (drill.shape, dimension) {
        (PcbHoleShape::Round, HoleDimension::Diameter) if drill.width != 0.0 => {
            Some(python_float(drill.width))
        }
        (PcbHoleShape::Oval, HoleDimension::Width) => Some(python_float(drill.width)),
        (PcbHoleShape::Oval, HoleDimension::Height) => drill.height.map(python_float),
        _ => None,
    }
}

fn hole_kind(pad: &PcbPad) -> &'static str {
    if pad
        .drill
        .as_ref()
        .is_some_and(|drill| drill.shape == PcbHoleShape::Oval && drill.height.is_some())
    {
        "slot"
    } else {
        "round"
    }
}

fn python_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn text_role(kind: &str, property_name: Option<&str>) -> String {
    let source = property_name.unwrap_or(kind).to_ascii_lowercase();
    match source.as_str() {
        "reference" => "designator",
        "value" => "value",
        _ if property_name.is_some() => "property",
        _ => "user",
    }
    .to_owned()
}

fn layer_role(layer: &str) -> &'static str {
    if layer.ends_with(".Cu") || matches!(layer, "*.Cu" | "F&B.Cu") {
        "copper"
    } else if layer.ends_with(".SilkS") {
        "silkscreen"
    } else if layer.ends_with(".Mask") || layer == "*.Mask" {
        "soldermask"
    } else if layer.ends_with(".Paste") {
        "paste"
    } else if layer.ends_with(".Fab") {
        "fab"
    } else if layer.ends_with(".Courtyard") {
        "courtyard"
    } else if layer == "Edge.Cuts" {
        "board-outline"
    } else if layer == "DRILLS" {
        "drill"
    } else if layer.ends_with(".User") || layer.starts_with("User.") {
        "user"
    } else {
        "other"
    }
}

const fn native_h(value: BoardTextHAlign) -> TextHorizontalAlignment {
    match value {
        BoardTextHAlign::Left => TextHorizontalAlignment::Left,
        BoardTextHAlign::Center => TextHorizontalAlignment::Center,
        BoardTextHAlign::Right => TextHorizontalAlignment::Right,
    }
}

const fn native_v(value: BoardTextVAlign) -> TextVerticalAlignment {
    match value {
        BoardTextVAlign::Top => TextVerticalAlignment::Top,
        BoardTextVAlign::Center => TextVerticalAlignment::Center,
        BoardTextVAlign::Bottom => TextVerticalAlignment::Bottom,
    }
}

fn finite(value: f64, field: &'static str) -> Result<f64, Error> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::at(
            ErrorPhase::Tree,
            ErrorKind::InvalidBuildValue,
            field,
            Position::START,
        ))
    }
}
