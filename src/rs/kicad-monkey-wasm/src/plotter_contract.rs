//! Shared core-to-TypeSpec plotter operation projection.

use kicad_monkey_contracts::JavaScriptSafeInteger;
use kicad_monkey_contracts::generated::footprint_plot_document::*;
use kicad_monkey_core::{
    PlotterFill as CorePlotterFill, PlotterLineStyle as CorePlotterLineStyle,
    PlotterOperation as CorePlotterOperation,
};

macro_rules! project_plotter_operation {
    ($source_index:expr, $source_operation:expr) => {{
        let index = u32::try_from($source_index).unwrap_or(u32::MAX);
        let operation = $source_operation;
        match operation {
            CorePlotterOperation::ThickSegment(operation) => ThickSegmentOperation {
                end_x: safe_integer(operation.end_x)?,
                end_y: safe_integer(operation.end_y)?,
                index,
                kind: "ThickSegment".to_owned(),
                layer: operation.layer,
                layers: operation.layers,
                mask_margin_nm: optional_safe_integer(operation.mask_margin_nm)?,
                pad_size_x_nm: optional_safe_integer(operation.pad_size_x_nm)?,
                pad_size_y_nm: optional_safe_integer(operation.pad_size_y_nm)?,
                role: contract_drill_role(operation.role.as_deref())?,
                start_x: safe_integer(operation.start_x)?,
                start_y: safe_integer(operation.start_y)?,
                width_nm: safe_integer(operation.width_nm)?,
            }
            .into(),
            CorePlotterOperation::ArcThreePoint(operation) => ArcThreePointOperation {
                end_x: safe_integer(operation.end_x)?,
                end_y: safe_integer(operation.end_y)?,
                fill: contract_fill(operation.fill),
                fill_color: operation.fill_color,
                index,
                kind: "ArcThreePoint".to_owned(),
                layer: operation.layer,
                line_style: operation.line_style.map(contract_line_style),
                mid_x: safe_integer(operation.mid_x)?,
                mid_y: safe_integer(operation.mid_y)?,
                start_x: safe_integer(operation.start_x)?,
                start_y: safe_integer(operation.start_y)?,
                stroke_color: operation.stroke_color,
                width_nm: safe_integer(operation.width_nm)?,
            }
            .into(),
            CorePlotterOperation::Circle(operation) => CircleOperation {
                cx: safe_integer(operation.cx)?,
                cy: safe_integer(operation.cy)?,
                diameter_nm: safe_integer(operation.diameter_nm)?,
                fill: contract_fill(operation.fill),
                fill_color: operation.fill_color,
                index,
                kind: "Circle".to_owned(),
                layer: operation.layer,
                layers: operation.layers,
                line_style: operation.line_style.map(contract_line_style),
                mask_margin_nm: optional_safe_integer(operation.mask_margin_nm)?,
                pad_size_x_nm: optional_safe_integer(operation.pad_size_x_nm)?,
                pad_size_y_nm: optional_safe_integer(operation.pad_size_y_nm)?,
                role: contract_drill_role(operation.role.as_deref())?,
                stroke_color: operation.stroke_color,
                width_nm: safe_integer(operation.width_nm)?,
            }
            .into(),
            CorePlotterOperation::Rect(operation) => RectOperation {
                corner_radius_nm: safe_integer(operation.corner_radius_nm)?,
                fill: contract_fill(operation.fill),
                fill_color: operation.fill_color,
                index,
                kind: "Rect".to_owned(),
                layer: operation.layer,
                line_style: operation.line_style.map(contract_line_style),
                stroke_color: operation.stroke_color,
                width_nm: safe_integer(operation.width_nm)?,
                x1: safe_integer(operation.x1)?,
                x2: safe_integer(operation.x2)?,
                y1: safe_integer(operation.y1)?,
                y2: safe_integer(operation.y2)?,
            }
            .into(),
            CorePlotterOperation::PlotPoly(operation) => PlotPolyOperation {
                fill: contract_fill(operation.fill),
                fill_color: operation.fill_color,
                index,
                kind: "PlotPoly".to_owned(),
                layer: operation.layer,
                line_style: operation.line_style.map(contract_line_style),
                points: operation
                    .points
                    .into_iter()
                    .map(|[x, y]| Ok(PlotterPoint::from([safe_integer(x)?, safe_integer(y)?])))
                    .collect::<Result<Vec<_>, String>>()?,
                stroke_color: operation.stroke_color,
                width_nm: safe_integer(operation.width_nm)?,
            }
            .into(),
            CorePlotterOperation::BezierCurve(operation) => BezierCurveOperation {
                ctrl1_x: safe_integer(operation.ctrl1_x)?,
                ctrl1_y: safe_integer(operation.ctrl1_y)?,
                ctrl2_x: safe_integer(operation.ctrl2_x)?,
                ctrl2_y: safe_integer(operation.ctrl2_y)?,
                end_x: safe_integer(operation.end_x)?,
                end_y: safe_integer(operation.end_y)?,
                index,
                kind: "BezierCurve".to_owned(),
                layer: operation.layer,
                line_style: operation.line_style.map(contract_line_style),
                start_x: safe_integer(operation.start_x)?,
                start_y: safe_integer(operation.start_y)?,
                stroke_color: operation.stroke_color,
                tolerance_nm: safe_integer(operation.tolerance_nm)?,
                width_nm: safe_integer(operation.width_nm)?,
            }
            .into(),
            CorePlotterOperation::FlashPadCircle(operation) => FlashPadCircleOperation {
                diameter_nm: safe_integer(operation.diameter_nm)?,
                index,
                kind: "FlashPadCircle".to_owned(),
                layers: operation.layers,
                mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
                x: safe_integer(operation.x)?,
                y: safe_integer(operation.y)?,
            }
            .into(),
            CorePlotterOperation::FlashPadOval(operation) => FlashPadOvalOperation {
                index,
                kind: "FlashPadOval".to_owned(),
                layers: operation.layers,
                mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
                orient_deg: operation.orient_deg,
                size_x_nm: safe_integer(operation.size_x_nm)?,
                size_y_nm: safe_integer(operation.size_y_nm)?,
                x: safe_integer(operation.x)?,
                y: safe_integer(operation.y)?,
            }
            .into(),
            CorePlotterOperation::FlashPadRect(operation) => FlashPadRectOperation {
                index,
                kind: "FlashPadRect".to_owned(),
                layers: operation.layers,
                mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
                orient_deg: operation.orient_deg,
                size_x_nm: safe_integer(operation.size_x_nm)?,
                size_y_nm: safe_integer(operation.size_y_nm)?,
                x: safe_integer(operation.x)?,
                y: safe_integer(operation.y)?,
            }
            .into(),
            CorePlotterOperation::FlashPadRoundRect(operation) => FlashPadRoundRectOperation {
                corner_radius_nm: safe_integer(operation.corner_radius_nm)?,
                index,
                kind: "FlashPadRoundRect".to_owned(),
                layers: operation.layers,
                mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
                orient_deg: operation.orient_deg,
                size_x_nm: safe_integer(operation.size_x_nm)?,
                size_y_nm: safe_integer(operation.size_y_nm)?,
                x: safe_integer(operation.x)?,
                y: safe_integer(operation.y)?,
            }
            .into(),
            CorePlotterOperation::FlashPadCustom(operation) => FlashPadCustomOperation {
                anchor_shape: operation.anchor_shape,
                index,
                kind: "FlashPadCustom".to_owned(),
                layers: operation.layers,
                mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
                orient_deg: operation.orient_deg,
                polygon_widths_nm: operation
                    .polygon_widths_nm
                    .unwrap_or_default()
                    .into_iter()
                    .map(safe_integer)
                    .collect::<Result<Vec<_>, String>>()?,
                polygons: operation
                    .polygons
                    .into_iter()
                    .map(|polygon| {
                        polygon
                            .into_iter()
                            .map(|[x, y]| {
                                Ok(PlotterPoint::from([safe_integer(x)?, safe_integer(y)?]))
                            })
                            .collect::<Result<Vec<_>, String>>()
                    })
                    .collect::<Result<Vec<_>, String>>()?,
                size_x_nm: safe_integer(operation.size_x_nm)?,
                size_y_nm: safe_integer(operation.size_y_nm)?,
                x: safe_integer(operation.x)?,
                y: safe_integer(operation.y)?,
            }
            .into(),
            CorePlotterOperation::FlashPadTrapez(operation) => FlashPadTrapezOperation {
                corners: contract_quad(operation.corners)?,
                index,
                kind: "FlashPadTrapez".to_owned(),
                layers: operation.layers,
                mask_margin_nm: safe_integer(operation.mask_margin_nm)?,
                orient_deg: operation.orient_deg,
                x: safe_integer(operation.x)?,
                y: safe_integer(operation.y)?,
            }
            .into(),
        }
    }};
}

pub(crate) fn contract_plotter_operation(
    index: usize,
    operation: CorePlotterOperation,
) -> Result<PlotterOperation, String> {
    Ok(project_plotter_operation!(index, operation))
}

fn contract_fill(fill: CorePlotterFill) -> PlotterFill {
    match fill {
        CorePlotterFill::NoFill => PlotterFill::NoFill,
        CorePlotterFill::FilledShape => PlotterFill::FilledShape,
        CorePlotterFill::FilledWithBackgroundBodyColor => PlotterFill::FilledWithBgBodycolor,
        CorePlotterFill::FilledWithColor => PlotterFill::FilledWithColor,
        CorePlotterFill::Hatch => PlotterFill::Hatch,
        CorePlotterFill::ReverseHatch => PlotterFill::ReverseHatch,
        CorePlotterFill::CrossHatch => PlotterFill::CrossHatch,
    }
}

fn contract_line_style(style: CorePlotterLineStyle) -> PlotterLineStyle {
    match style {
        CorePlotterLineStyle::Default => PlotterLineStyle::Default,
        CorePlotterLineStyle::Solid => PlotterLineStyle::Solid,
        CorePlotterLineStyle::Dash => PlotterLineStyle::Dash,
        CorePlotterLineStyle::Dot => PlotterLineStyle::Dot,
        CorePlotterLineStyle::DashDot => PlotterLineStyle::DashDot,
        CorePlotterLineStyle::DashDotDot => PlotterLineStyle::DashDotDot,
    }
}

fn safe_integer(value: i64) -> Result<JavaScriptSafeInteger, String> {
    JavaScriptSafeInteger::try_from(value).map_err(|error| error.to_string())
}

fn optional_safe_integer(value: Option<i64>) -> Result<Option<JavaScriptSafeInteger>, String> {
    value.map(safe_integer).transpose()
}

fn contract_drill_role(value: Option<&str>) -> Result<Option<PlotterDrillRole>, String> {
    value
        .map(|role| PlotterDrillRole::try_from(role).map_err(|error| error.to_string()))
        .transpose()
}

fn contract_quad(corners: [[i64; 2]; 4]) -> Result<PlotterQuad, String> {
    let points = corners
        .into_iter()
        .map(|[x, y]| Ok(PlotterPoint::from([safe_integer(x)?, safe_integer(y)?])))
        .collect::<Result<Vec<_>, String>>()?;
    let points: [PlotterPoint; 4] = points
        .try_into()
        .map_err(|_| "plotter quad must contain four points".to_owned())?;
    Ok(PlotterQuad::from(points))
}
