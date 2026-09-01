//! Direct typed Plotter-IR rendering without JSON or Serde reconciliation.

use kicad_monkey_contracts::generated::{
    board_plot_document as board, footprint_plot_document as footprint,
    schematic_plot_document as schematic, symbol_plot_document as symbol,
};
use kicad_monkey_contracts::{
    validate_board_plot_document, validate_footprint_plot_document,
    validate_schematic_plot_document, validate_symbol_plot_document,
};
use kicad_monkey_core::{ProjectedBoardPlotArtifact, ProjectedSchematicPagePlotArtifact};

use crate::context::{
    PlotterOperationKind, SvgBackground, SvgColor, SvgFillMode, SvgIdentityMode, SvgLineStyle,
    SvgSemanticRole, SvgStyleOverride, ValidatedSvgRenderContextA1,
};
use crate::sink::SvgSink;
use crate::{SvgArtifact, SvgError, SvgErrorKind, SvgMetrics};

type Point = (i64, i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgViewport {
    pub min_x_nm: i64,
    pub min_y_nm: i64,
    pub width_nm: u64,
    pub height_nm: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgBounds {
    pub min_x_nm: i64,
    pub min_y_nm: i64,
    pub max_x_nm: i64,
    pub max_y_nm: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SvgWarning {
    BoundsUnavailableForUncachedText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgFitOptions {
    pub padding_nm: u64,
    pub min_extent_nm: u64,
    pub fallback: Option<SvgViewport>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewportPolicy {
    Explicit(SvgViewport),
    Fit(SvgFitOptions),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SvgRenderLimits {
    pub max_records: usize,
    pub max_operations: usize,
    pub max_points: usize,
    pub max_text_bytes: usize,
    pub max_image_encoded_bytes: usize,
    pub max_block_depth: usize,
    pub max_svg_elements: usize,
    pub max_render_work: usize,
    pub max_svg_bytes: usize,
    pub max_result_bytes: usize,
    pub max_bounds_work: usize,
}

impl Default for SvgRenderLimits {
    fn default() -> Self {
        Self {
            max_records: 1_000_000,
            max_operations: 4_000_000,
            max_points: 16_000_000,
            max_text_bytes: 256 * 1024 * 1024,
            max_image_encoded_bytes: 256 * 1024 * 1024,
            max_block_depth: 4096,
            max_svg_elements: 8_000_000,
            max_render_work: 64_000_000,
            max_svg_bytes: 512 * 1024 * 1024,
            max_result_bytes: 768 * 1024 * 1024,
            max_bounds_work: 64_000_000,
        }
    }
}

#[derive(Clone, Debug)]
struct PrimitiveStyle<'a> {
    layer: Option<&'a str>,
    role: Option<String>,
    layers: &'a [String],
    stroke: Option<&'a str>,
    fill: Option<&'a str>,
    width_nm: i64,
    line_style: Option<String>,
    filled: bool,
}

#[derive(Clone, Debug)]
struct TextData<'a> {
    x: i64,
    y: i64,
    text: &'a str,
    color: &'a str,
    orient_deg: f64,
    size_y_nm: i64,
    h_align: String,
    v_align: String,
    italic: bool,
    bold: bool,
    multiline: bool,
    font_face: &'a str,
    layer: Option<&'a str>,
    cache: Vec<Vec<Vec<Point>>>,
    legacy_cache: Vec<Vec<Point>>,
}

#[derive(Clone, Debug)]
enum OperationData<'a> {
    Owned {
        ownership: OwnershipData<'a>,
        operation: Box<OperationData<'a>>,
    },
    Segment {
        start: Point,
        end: Point,
        style: PrimitiveStyle<'a>,
    },
    Arc {
        start: Point,
        mid: Point,
        end: Point,
        style: PrimitiveStyle<'a>,
    },
    Circle {
        center: Point,
        diameter_nm: i64,
        style: PrimitiveStyle<'a>,
    },
    Rect {
        first: Point,
        second: Point,
        corner_radius_nm: i64,
        style: PrimitiveStyle<'a>,
    },
    Poly {
        points: Vec<Point>,
        style: PrimitiveStyle<'a>,
    },
    Bezier {
        points: [Point; 4],
        style: PrimitiveStyle<'a>,
    },
    Text(TextData<'a>),
    Image {
        center: Point,
        width_nm: i64,
        height_nm: i64,
        format: &'a str,
        data: &'a str,
    },
    PadCircle {
        center: Point,
        diameter_nm: i64,
        layers: &'a [String],
    },
    PadOval {
        center: Point,
        size: Point,
        angle_deg: f64,
        layers: &'a [String],
    },
    PadRect {
        center: Point,
        size: Point,
        angle_deg: f64,
        radius_nm: Option<i64>,
        layers: &'a [String],
    },
    PadCustom {
        center: Point,
        angle_deg: f64,
        polygons: Vec<Vec<Point>>,
        layers: &'a [String],
    },
    PadTrapez {
        center: Point,
        angle_deg: f64,
        corners: Vec<Point>,
        layers: &'a [String],
    },
    StartBlock {
        label: &'a str,
        data_uuid: &'a str,
        data_ref: String,
        object_id: &'a str,
        extra_attrs: Vec<(String, String)>,
    },
    EndBlock,
}

#[derive(Clone, Debug)]
struct OwnershipData<'a> {
    label: Option<&'a str>,
    data_uuid: Option<&'a str>,
    data_ref: Option<String>,
    object_id: Option<&'a str>,
    extra_attrs: Vec<(String, String)>,
}

impl OwnershipData<'_> {
    fn is_empty(&self) -> bool {
        self.label.is_none()
            && self.data_uuid.is_none()
            && self.data_ref.is_none()
            && self.object_id.is_none()
            && self.extra_attrs.is_empty()
    }
}

#[derive(Clone, Copy, Default)]
struct Preflight {
    operations: usize,
    points: usize,
    text_bytes: usize,
    image_bytes: usize,
}

impl Preflight {
    fn add(&mut self, value: Self) -> Result<(), SvgError> {
        self.operations = checked_add(self.operations, value.operations, "operations")?;
        self.points = checked_add(self.points, value.points, "points")?;
        self.text_bytes = checked_add(self.text_bytes, value.text_bytes, "text bytes")?;
        self.image_bytes = checked_add(self.image_bytes, value.image_bytes, "image bytes")?;
        Ok(())
    }
}

macro_rules! text_usage {
    ($value:expr) => {{
        let cache_points = $value.render_cache.as_ref().map_or(Ok(0usize), |cache| {
            cache
                .polygons
                .iter()
                .flat_map(|polygon| &polygon.contours)
                .try_fold(0usize, |sum, contour| {
                    checked_add(sum, contour.len(), "points")
                })
        })?;
        let legacy_points = $value
            .render_cache_polygons
            .iter()
            .try_fold(0usize, |sum, polygon| {
                checked_add(sum, polygon.len(), "points")
            })?;
        Ok::<Preflight, SvgError>(Preflight {
            operations: 1,
            points: checked_add(
                checked_add(1, cache_points, "points")?,
                legacy_points,
                "points",
            )?,
            text_bytes: $value.text.len(),
            image_bytes: 0,
        })
    }};
}

macro_rules! common_operation_usage {
    ($operation:expr, $module:path) => {{
        use $module as m;
        match $operation {
            m::PlotterOperation::ThickSegmentOperation(_) => Ok(Preflight {
                operations: 1,
                points: 2,
                ..Preflight::default()
            }),
            m::PlotterOperation::ArcThreePointOperation(_) => Ok(Preflight {
                operations: 1,
                points: 3,
                ..Preflight::default()
            }),
            m::PlotterOperation::CircleOperation(_)
            | m::PlotterOperation::FlashPadCircleOperation(_)
            | m::PlotterOperation::FlashPadOvalOperation(_)
            | m::PlotterOperation::FlashPadRectOperation(_)
            | m::PlotterOperation::FlashPadRoundRectOperation(_) => Ok(Preflight {
                operations: 1,
                points: 1,
                ..Preflight::default()
            }),
            m::PlotterOperation::RectOperation(_) => Ok(Preflight {
                operations: 1,
                points: 2,
                ..Preflight::default()
            }),
            m::PlotterOperation::PlotPolyOperation(value) => Ok(Preflight {
                operations: 1,
                points: value.points.len(),
                ..Preflight::default()
            }),
            m::PlotterOperation::BezierCurveOperation(_)
            | m::PlotterOperation::FlashPadTrapezOperation(_) => Ok(Preflight {
                operations: 1,
                points: 4,
                ..Preflight::default()
            }),
            m::PlotterOperation::TextOperation(value) => text_usage!(value),
            m::PlotterOperation::PlotImageOperation(value) => Ok(Preflight {
                operations: 1,
                points: 1,
                text_bytes: 0,
                image_bytes: value.image_data_b64.len(),
            }),
            m::PlotterOperation::FlashPadCustomOperation(value) => Ok(Preflight {
                operations: 1,
                points: value.polygons.iter().try_fold(1usize, |sum, polygon| {
                    checked_add(sum, polygon.len(), "points")
                })?,
                ..Preflight::default()
            }),
        }
    }};
}

fn board_operation_usage(operation: &board::PlotterOperation) -> Result<Preflight, SvgError> {
    common_operation_usage!(
        operation,
        kicad_monkey_contracts::generated::board_plot_document
    )
}

fn footprint_operation_usage(
    operation: &footprint::PlotterOperation,
) -> Result<Preflight, SvgError> {
    common_operation_usage!(
        operation,
        kicad_monkey_contracts::generated::footprint_plot_document
    )
}

fn symbol_operation_usage(operation: &symbol::PlotterOperation) -> Result<Preflight, SvgError> {
    common_operation_usage!(
        operation,
        kicad_monkey_contracts::generated::symbol_plot_document
    )
}

fn schematic_operation_usage(
    operation: &schematic::PlotterOperation,
) -> Result<Preflight, SvgError> {
    common_operation_usage!(
        operation,
        kicad_monkey_contracts::generated::schematic_plot_document
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive generated board-footprint operation preflight"
)]
fn board_footprint_operation_usage(
    operation: &board::BoardFootprintOperation,
) -> Result<Preflight, SvgError> {
    use board::BoardFootprintOperation as O;
    let fixed = |points| {
        Ok(Preflight {
            operations: 1,
            points,
            ..Preflight::default()
        })
    };
    match operation {
        O::ThickSegmentOperation(_) | O::RectOperation(_) => fixed(2),
        O::ArcThreePointOperation(_) => fixed(3),
        O::CircleOperation(_)
        | O::FlashPadCircleOperation(_)
        | O::FlashPadOvalOperation(_)
        | O::FlashPadRectOperation(_)
        | O::FlashPadRoundRectOperation(_) => fixed(1),
        O::PlotPolyOperation(value) => fixed(value.points.len()),
        O::BezierCurveOperation(_) | O::FlashPadTrapezOperation(_) => fixed(4),
        O::TextOperation(value) => text_usage!(value),
        O::FlashPadCustomOperation(value) => Ok(Preflight {
            operations: 1,
            points: value.polygons.iter().try_fold(1usize, |sum, polygon| {
                checked_add(sum, polygon.len(), "points")
            })?,
            ..Preflight::default()
        }),
        O::StartBlockOperation(_) | O::EndBlockOperation(_) => fixed(0),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive generated schematic-symbol operation preflight"
)]
fn schematic_symbol_operation_usage(
    operation: &schematic::SchematicSymbolOperation,
) -> Result<Preflight, SvgError> {
    use schematic::SchematicSymbolOperation as O;
    let fixed = |points| {
        Ok(Preflight {
            operations: 1,
            points,
            ..Preflight::default()
        })
    };
    match operation {
        O::ThickSegmentOperation(_) | O::RectOperation(_) => fixed(2),
        O::ArcThreePointOperation(_) => fixed(3),
        O::CircleOperation(_)
        | O::FlashPadCircleOperation(_)
        | O::FlashPadOvalOperation(_)
        | O::FlashPadRectOperation(_)
        | O::FlashPadRoundRectOperation(_) => fixed(1),
        O::PlotPolyOperation(value) => fixed(value.points.len()),
        O::BezierCurveOperation(_) | O::FlashPadTrapezOperation(_) => fixed(4),
        O::TextOperation(value) => text_usage!(value),
        O::PlotImageOperation(value) => Ok(Preflight {
            operations: 1,
            points: 1,
            image_bytes: value.image_data_b64.len(),
            ..Preflight::default()
        }),
        O::FlashPadCustomOperation(value) => Ok(Preflight {
            operations: 1,
            points: value.polygons.iter().try_fold(1usize, |sum, polygon| {
                checked_add(sum, polygon.len(), "points")
            })?,
            ..Preflight::default()
        }),
        O::SchematicSymbolStartBlockOperation(_) | O::SchematicSymbolEndBlockOperation(_) => {
            fixed(0)
        }
    }
}

fn schematic_sheet_operation_usage(
    operation: &schematic::SchematicSheetOperation,
) -> Result<Preflight, SvgError> {
    use schematic::SchematicSheetOperation as O;
    match operation {
        O::ThickSegmentOperation(_) | O::RectOperation(_) => Ok(Preflight {
            operations: 1,
            points: 2,
            ..Preflight::default()
        }),
        O::PlotPolyOperation(value) => Ok(Preflight {
            operations: 1,
            points: value.points.len(),
            ..Preflight::default()
        }),
        O::TextOperation(value) => text_usage!(value),
        O::SchematicSheetStartBlockOperation(_) | O::SchematicSheetEndBlockOperation(_) => {
            Ok(Preflight {
                operations: 1,
                ..Preflight::default()
            })
        }
    }
}

fn add_operation_usages<O>(
    counts: &mut Preflight,
    operations: &[O],
    usage: fn(&O) -> Result<Preflight, SvgError>,
) -> Result<(), SvgError> {
    for operation in operations {
        counts.add(usage(operation)?)?;
    }
    Ok(())
}

fn enforce_original_preflight(
    record_count: usize,
    counts: Preflight,
    limits: SvgRenderLimits,
) -> Result<(), SvgError> {
    for (actual, maximum, name) in [
        (record_count, limits.max_records, "records"),
        (counts.operations, limits.max_operations, "operations"),
        (counts.points, limits.max_points, "points"),
        (counts.text_bytes, limits.max_text_bytes, "text bytes"),
        (
            counts.image_bytes,
            limits.max_image_encoded_bytes,
            "image bytes",
        ),
    ] {
        ensure(actual, maximum, name)?;
    }
    let work = checked_add(
        checked_add(counts.operations, counts.points, "render work")?,
        checked_add(counts.text_bytes, counts.image_bytes, "render work")?,
        "render work",
    )?;
    ensure(work, limits.max_render_work, "render work")?;
    ensure(
        checked_add(counts.operations, counts.points, "bounds work")?,
        limits.max_bounds_work,
        "bounds work",
    )
}

trait OperationLayers {
    fn operation_layers(&self) -> &[String];
}

impl OperationLayers for Vec<String> {
    fn operation_layers(&self) -> &[String] {
        self
    }
}

impl OperationLayers for Option<Vec<String>> {
    fn operation_layers(&self) -> &[String] {
        self.as_deref().unwrap_or_default()
    }
}

fn operation_layers(value: &impl OperationLayers) -> &[String] {
    value.operation_layers()
}

macro_rules! style {
    ($value:expr, $filled:expr) => {
        PrimitiveStyle {
            layer: $value.layer.as_deref(),
            role: None,
            layers: &[],
            stroke: $value.stroke_color.as_deref(),
            fill: $value.fill_color.as_deref(),
            width_nm: $value.width_nm.get(),
            line_style: $value.line_style.as_ref().map(ToString::to_string),
            filled: $filled,
        }
    };
}

macro_rules! define_operation_adapter {
    ($name:ident, $operation:path, $module:path) => {
        #[allow(
            clippy::too_many_lines,
            reason = "exhaustive generated PlotterOperation adapter"
        )]
        fn $name<'a>(operation: &'a $operation) -> OperationData<'a> {
            use $module as m;
            match operation {
                m::PlotterOperation::ThickSegmentOperation(value) => OperationData::Segment {
                    start: (value.start_x.get(), value.start_y.get()),
                    end: (value.end_x.get(), value.end_y.get()),
                    style: PrimitiveStyle {
                        layer: value.layer.as_deref(),
                        role: value.role.as_ref().map(ToString::to_string),
                        layers: operation_layers(&value.layers),
                        stroke: value.stroke_color.as_deref(),
                        fill: None,
                        width_nm: value.width_nm.get(),
                        line_style: None,
                        filled: false,
                    },
                },
                m::PlotterOperation::ArcThreePointOperation(value) => OperationData::Arc {
                    start: (value.start_x.get(), value.start_y.get()),
                    mid: (value.mid_x.get(), value.mid_y.get()),
                    end: (value.end_x.get(), value.end_y.get()),
                    style: style!(value, value.fill.to_string() != "NO_FILL"),
                },
                m::PlotterOperation::CircleOperation(value) => OperationData::Circle {
                    center: (value.cx.get(), value.cy.get()),
                    diameter_nm: value.diameter_nm.get(),
                    style: PrimitiveStyle {
                        layer: value.layer.as_deref(),
                        role: value.role.as_ref().map(ToString::to_string),
                        layers: operation_layers(&value.layers),
                        stroke: value.stroke_color.as_deref(),
                        fill: value.fill_color.as_deref(),
                        width_nm: value.width_nm.get(),
                        line_style: value.line_style.as_ref().map(ToString::to_string),
                        filled: value.fill.to_string() != "NO_FILL",
                    },
                },
                m::PlotterOperation::RectOperation(value) => OperationData::Rect {
                    first: (value.x1.get(), value.y1.get()),
                    second: (value.x2.get(), value.y2.get()),
                    corner_radius_nm: value.corner_radius_nm.get(),
                    style: style!(value, value.fill.to_string() != "NO_FILL"),
                },
                m::PlotterOperation::PlotPolyOperation(value) => OperationData::Poly {
                    points: value
                        .points
                        .iter()
                        .map(|point| (point.0[0].get(), point.0[1].get()))
                        .collect(),
                    style: style!(value, value.fill.to_string() != "NO_FILL"),
                },
                m::PlotterOperation::BezierCurveOperation(value) => OperationData::Bezier {
                    points: [
                        (value.start_x.get(), value.start_y.get()),
                        (value.ctrl1_x.get(), value.ctrl1_y.get()),
                        (value.ctrl2_x.get(), value.ctrl2_y.get()),
                        (value.end_x.get(), value.end_y.get()),
                    ],
                    style: PrimitiveStyle {
                        layer: value.layer.as_deref(),
                        role: None,
                        layers: &[],
                        stroke: value.stroke_color.as_deref(),
                        fill: None,
                        width_nm: value.width_nm.get(),
                        line_style: value.line_style.as_ref().map(ToString::to_string),
                        filled: false,
                    },
                },
                m::PlotterOperation::TextOperation(value) => OperationData::Text(TextData {
                    x: value.x.get(),
                    y: value.y.get(),
                    text: &value.text,
                    color: &value.color,
                    orient_deg: value.orient_deg,
                    size_y_nm: value.size_y_nm.get(),
                    h_align: value.h_align.to_string(),
                    v_align: value.v_align.to_string(),
                    italic: value.italic,
                    bold: value.bold,
                    multiline: value.multiline,
                    font_face: &value.font_face,
                    layer: value.layer.as_deref(),
                    cache: value.render_cache.as_ref().map_or_else(Vec::new, |cache| {
                        cache
                            .polygons
                            .iter()
                            .map(|polygon| {
                                polygon
                                    .contours
                                    .iter()
                                    .map(|contour| {
                                        contour
                                            .iter()
                                            .map(|point| (point.0[0].get(), point.0[1].get()))
                                            .collect()
                                    })
                                    .collect()
                            })
                            .collect()
                    }),
                    legacy_cache: value
                        .render_cache_polygons
                        .iter()
                        .map(|polygon| {
                            polygon
                                .iter()
                                .map(|point| (point.0[0].get(), point.0[1].get()))
                                .collect()
                        })
                        .collect(),
                }),
                m::PlotterOperation::PlotImageOperation(value) => OperationData::Image {
                    center: (value.x.get(), value.y.get()),
                    width_nm: value.width_nm.get(),
                    height_nm: value.height_nm.get(),
                    format: &value.image_format,
                    data: &value.image_data_b64,
                },
                m::PlotterOperation::FlashPadCircleOperation(value) => OperationData::PadCircle {
                    center: (value.x.get(), value.y.get()),
                    diameter_nm: value.diameter_nm.get(),
                    layers: operation_layers(&value.layers),
                },
                m::PlotterOperation::FlashPadOvalOperation(value) => OperationData::PadOval {
                    center: (value.x.get(), value.y.get()),
                    size: (value.size_x_nm.get(), value.size_y_nm.get()),
                    angle_deg: value.orient_deg,
                    layers: operation_layers(&value.layers),
                },
                m::PlotterOperation::FlashPadRectOperation(value) => OperationData::PadRect {
                    center: (value.x.get(), value.y.get()),
                    size: (value.size_x_nm.get(), value.size_y_nm.get()),
                    angle_deg: value.orient_deg,
                    radius_nm: None,
                    layers: operation_layers(&value.layers),
                },
                m::PlotterOperation::FlashPadRoundRectOperation(value) => OperationData::PadRect {
                    center: (value.x.get(), value.y.get()),
                    size: (value.size_x_nm.get(), value.size_y_nm.get()),
                    angle_deg: value.orient_deg,
                    radius_nm: Some(value.corner_radius_nm.get()),
                    layers: operation_layers(&value.layers),
                },
                m::PlotterOperation::FlashPadCustomOperation(value) => OperationData::PadCustom {
                    center: (value.x.get(), value.y.get()),
                    angle_deg: value.orient_deg,
                    polygons: value
                        .polygons
                        .iter()
                        .map(|polygon| {
                            polygon
                                .iter()
                                .map(|point| (point.0[0].get(), point.0[1].get()))
                                .collect()
                        })
                        .collect(),
                    layers: operation_layers(&value.layers),
                },
                m::PlotterOperation::FlashPadTrapezOperation(value) => OperationData::PadTrapez {
                    center: (value.x.get(), value.y.get()),
                    angle_deg: value.orient_deg,
                    corners: value
                        .corners
                        .0
                        .iter()
                        .map(|point| (point.0[0].get(), point.0[1].get()))
                        .collect(),
                    layers: operation_layers(&value.layers),
                },
            }
        }
    };
}

define_operation_adapter!(
    footprint_operation,
    footprint::PlotterOperation,
    kicad_monkey_contracts::generated::footprint_plot_document
);
define_operation_adapter!(
    symbol_operation,
    symbol::PlotterOperation,
    kicad_monkey_contracts::generated::symbol_plot_document
);
define_operation_adapter!(
    board_operation,
    board::PlotterOperation,
    kicad_monkey_contracts::generated::board_plot_document
);

macro_rules! schematic_common_operation {
    ($value:expr, ThickSegmentOperation) => {
        OperationData::Segment {
            start: ($value.start_x.get(), $value.start_y.get()),
            end: ($value.end_x.get(), $value.end_y.get()),
            style: PrimitiveStyle {
                layer: $value.layer.as_deref(),
                role: $value.role.as_ref().map(ToString::to_string),
                layers: operation_layers(&$value.layers),
                stroke: $value.stroke_color.as_deref(),
                fill: None,
                width_nm: $value.width_nm.get(),
                line_style: None,
                filled: false,
            },
        }
    };
    ($value:expr, ArcThreePointOperation) => {
        OperationData::Arc {
            start: ($value.start_x.get(), $value.start_y.get()),
            mid: ($value.mid_x.get(), $value.mid_y.get()),
            end: ($value.end_x.get(), $value.end_y.get()),
            style: style!($value, $value.fill.to_string() != "NO_FILL"),
        }
    };
    ($value:expr, CircleOperation) => {
        OperationData::Circle {
            center: ($value.cx.get(), $value.cy.get()),
            diameter_nm: $value.diameter_nm.get(),
            style: PrimitiveStyle {
                layer: $value.layer.as_deref(),
                role: $value.role.as_ref().map(ToString::to_string),
                layers: operation_layers(&$value.layers),
                stroke: $value.stroke_color.as_deref(),
                fill: $value.fill_color.as_deref(),
                width_nm: $value.width_nm.get(),
                line_style: $value.line_style.as_ref().map(ToString::to_string),
                filled: $value.fill.to_string() != "NO_FILL",
            },
        }
    };
    ($value:expr, RectOperation) => {
        OperationData::Rect {
            first: ($value.x1.get(), $value.y1.get()),
            second: ($value.x2.get(), $value.y2.get()),
            corner_radius_nm: $value.corner_radius_nm.get(),
            style: style!($value, $value.fill.to_string() != "NO_FILL"),
        }
    };
    ($value:expr, PlotPolyOperation) => {
        OperationData::Poly {
            points: $value
                .points
                .iter()
                .map(|point| (point.0[0].get(), point.0[1].get()))
                .collect(),
            style: style!($value, $value.fill.to_string() != "NO_FILL"),
        }
    };
    ($value:expr, BezierCurveOperation) => {
        OperationData::Bezier {
            points: [
                ($value.start_x.get(), $value.start_y.get()),
                ($value.ctrl1_x.get(), $value.ctrl1_y.get()),
                ($value.ctrl2_x.get(), $value.ctrl2_y.get()),
                ($value.end_x.get(), $value.end_y.get()),
            ],
            style: PrimitiveStyle {
                layer: $value.layer.as_deref(),
                role: None,
                layers: &[],
                stroke: $value.stroke_color.as_deref(),
                fill: None,
                width_nm: $value.width_nm.get(),
                line_style: $value.line_style.as_ref().map(ToString::to_string),
                filled: false,
            },
        }
    };
    ($value:expr, TextOperation) => {
        OperationData::Text(TextData {
            x: $value.x.get(),
            y: $value.y.get(),
            text: &$value.text,
            color: &$value.color,
            orient_deg: $value.orient_deg,
            size_y_nm: $value.size_y_nm.get(),
            h_align: $value.h_align.to_string(),
            v_align: $value.v_align.to_string(),
            italic: $value.italic,
            bold: $value.bold,
            multiline: $value.multiline,
            font_face: &$value.font_face,
            layer: $value.layer.as_deref(),
            cache: $value.render_cache.as_ref().map_or_else(Vec::new, |cache| {
                cache
                    .polygons
                    .iter()
                    .map(|polygon| {
                        polygon
                            .contours
                            .iter()
                            .map(|contour| {
                                contour
                                    .iter()
                                    .map(|point| (point.0[0].get(), point.0[1].get()))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect()
            }),
            legacy_cache: $value
                .render_cache_polygons
                .iter()
                .map(|polygon| {
                    polygon
                        .iter()
                        .map(|point| (point.0[0].get(), point.0[1].get()))
                        .collect()
                })
                .collect(),
        })
    };
    ($value:expr, PlotImageOperation) => {
        OperationData::Image {
            center: ($value.x.get(), $value.y.get()),
            width_nm: $value.width_nm.get(),
            height_nm: $value.height_nm.get(),
            format: &$value.image_format,
            data: &$value.image_data_b64,
        }
    };
    ($value:expr, FlashPadCircleOperation) => {
        OperationData::PadCircle {
            center: ($value.x.get(), $value.y.get()),
            diameter_nm: $value.diameter_nm.get(),
            layers: operation_layers(&$value.layers),
        }
    };
    ($value:expr, FlashPadOvalOperation) => {
        OperationData::PadOval {
            center: ($value.x.get(), $value.y.get()),
            size: ($value.size_x_nm.get(), $value.size_y_nm.get()),
            angle_deg: $value.orient_deg,
            layers: operation_layers(&$value.layers),
        }
    };
    ($value:expr, FlashPadRectOperation) => {
        OperationData::PadRect {
            center: ($value.x.get(), $value.y.get()),
            size: ($value.size_x_nm.get(), $value.size_y_nm.get()),
            angle_deg: $value.orient_deg,
            radius_nm: None,
            layers: operation_layers(&$value.layers),
        }
    };
    ($value:expr, FlashPadRoundRectOperation) => {
        OperationData::PadRect {
            center: ($value.x.get(), $value.y.get()),
            size: ($value.size_x_nm.get(), $value.size_y_nm.get()),
            angle_deg: $value.orient_deg,
            radius_nm: Some($value.corner_radius_nm.get()),
            layers: operation_layers(&$value.layers),
        }
    };
    ($value:expr, FlashPadCustomOperation) => {
        OperationData::PadCustom {
            center: ($value.x.get(), $value.y.get()),
            angle_deg: $value.orient_deg,
            polygons: $value
                .polygons
                .iter()
                .map(|polygon| {
                    polygon
                        .iter()
                        .map(|point| (point.0[0].get(), point.0[1].get()))
                        .collect()
                })
                .collect(),
            layers: operation_layers(&$value.layers),
        }
    };
    ($value:expr, FlashPadTrapezOperation) => {
        OperationData::PadTrapez {
            center: ($value.x.get(), $value.y.get()),
            angle_deg: $value.orient_deg,
            corners: $value
                .corners
                .0
                .iter()
                .map(|point| (point.0[0].get(), point.0[1].get()))
                .collect(),
            layers: operation_layers(&$value.layers),
        }
    };
}

fn schematic_operation(operation: &schematic::PlotterOperation) -> OperationData<'_> {
    match operation {
        schematic::PlotterOperation::ThickSegmentOperation(value) => {
            schematic_common_operation!(value, ThickSegmentOperation)
        }
        schematic::PlotterOperation::ArcThreePointOperation(value) => {
            schematic_common_operation!(value, ArcThreePointOperation)
        }
        schematic::PlotterOperation::CircleOperation(value) => {
            schematic_common_operation!(value, CircleOperation)
        }
        schematic::PlotterOperation::RectOperation(value) => {
            schematic_common_operation!(value, RectOperation)
        }
        schematic::PlotterOperation::PlotPolyOperation(value) => {
            schematic_common_operation!(value, PlotPolyOperation)
        }
        schematic::PlotterOperation::BezierCurveOperation(value) => {
            schematic_common_operation!(value, BezierCurveOperation)
        }
        schematic::PlotterOperation::TextOperation(value) => {
            schematic_common_operation!(value, TextOperation)
        }
        schematic::PlotterOperation::PlotImageOperation(value) => {
            schematic_common_operation!(value, PlotImageOperation)
        }
        schematic::PlotterOperation::FlashPadCircleOperation(value) => {
            schematic_common_operation!(value, FlashPadCircleOperation)
        }
        schematic::PlotterOperation::FlashPadOvalOperation(value) => {
            schematic_common_operation!(value, FlashPadOvalOperation)
        }
        schematic::PlotterOperation::FlashPadRectOperation(value) => {
            schematic_common_operation!(value, FlashPadRectOperation)
        }
        schematic::PlotterOperation::FlashPadRoundRectOperation(value) => {
            schematic_common_operation!(value, FlashPadRoundRectOperation)
        }
        schematic::PlotterOperation::FlashPadCustomOperation(value) => {
            schematic_common_operation!(value, FlashPadCustomOperation)
        }
        schematic::PlotterOperation::FlashPadTrapezOperation(value) => {
            schematic_common_operation!(value, FlashPadTrapezOperation)
        }
    }
}

macro_rules! enriched_style {
    ($value:expr, $filled:expr) => {
        PrimitiveStyle {
            layer: $value.layer.as_deref(),
            role: None,
            layers: &[],
            stroke: $value.stroke_color.as_deref(),
            fill: $value.fill_color.as_deref(),
            width_nm: $value.width_nm.get(),
            line_style: $value.line_style.as_ref().map(ToString::to_string),
            filled: $filled,
        }
    };
}

#[allow(
    clippy::cognitive_complexity,
    reason = "exhaustive typed metadata field inventory is intentionally linear"
)]
fn board_pad_block_attrs(attrs: &board::BoardFootprintPadBlockAttrs) -> Vec<(String, String)> {
    let mut values = Vec::with_capacity(22);
    macro_rules! push_string {
        ($field:ident) => {
            if let Some(value) = &attrs.$field {
                values.push((stringify!($field).replace('_', "-"), value.clone()));
            }
        };
    }
    macro_rules! push_display {
        ($field:ident) => {
            if let Some(value) = attrs.$field {
                values.push((stringify!($field).replace('_', "-"), value.to_string()));
            }
        };
    }
    push_string!(component);
    push_string!(component_uid);
    push_string!(component_uuid);
    push_string!(footprint);
    push_string!(hole_diameter_mm);
    push_string!(hole_height_mm);
    push_display!(hole_kind);
    push_string!(hole_owner);
    push_display!(hole_plating);
    push_string!(hole_render);
    push_string!(hole_width_mm);
    push_string!(layer_names);
    push_string!(net);
    push_string!(net_class);
    push_string!(net_classes);
    push_string!(net_id);
    push_string!(net_index);
    push_string!(pad_designator);
    push_string!(pad_number);
    push_string!(pad_shape);
    push_string!(pad_type);
    values.push(("primitive".to_owned(), attrs.primitive.to_string()));
    values
}

fn board_child_attrs(attrs: &board::BoardFootprintChildAttrs) -> Vec<(String, String)> {
    let mut values = vec![
        ("component".to_owned(), attrs.component.clone()),
        ("component-uid".to_owned(), attrs.component_uid.clone()),
        ("component-uuid".to_owned(), attrs.component_uuid.clone()),
        ("footprint".to_owned(), attrs.footprint.clone()),
        (
            "footprint-object-index".to_owned(),
            attrs.footprint_object_index.to_string(),
        ),
        (
            "footprint-primitive".to_owned(),
            attrs.footprint_primitive.to_string(),
        ),
        ("primitive".to_owned(), attrs.primitive.to_string()),
    ];
    macro_rules! push_string {
        ($field:ident) => {
            if let Some(value) = &attrs.$field {
                values.push((stringify!($field).replace('_', "-"), value.clone()));
            }
        };
    }
    macro_rules! push_display {
        ($field:ident) => {
            if let Some(value) = attrs.$field {
                values.push((stringify!($field).replace('_', "-"), value.to_string()));
            }
        };
    }
    push_display!(footprint_graphic_kind);
    push_display!(footprint_subop_index);
    push_display!(footprint_text_role);
    push_string!(fp_text_type);
    push_string!(layer_name);
    push_display!(layer_role);
    push_string!(property_name);
    values.sort_by(|left, right| left.0.cmp(&right.0));
    values
}

fn board_footprint_ownership(
    operation: &board::BoardFootprintOperation,
) -> Option<OwnershipData<'_>> {
    macro_rules! ownership {
        ($value:expr) => {{
            let ownership = OwnershipData {
                label: $value.label.as_deref(),
                data_uuid: $value.data_uuid.as_deref(),
                data_ref: $value.data_ref.as_ref().map(ToString::to_string),
                object_id: $value.object_id.as_deref(),
                extra_attrs: $value
                    .extra_attrs
                    .as_ref()
                    .map_or_else(Vec::new, board_child_attrs),
            };
            (!ownership.is_empty()).then_some(ownership)
        }};
    }
    match operation {
        board::BoardFootprintOperation::ThickSegmentOperation(value) => ownership!(value),
        board::BoardFootprintOperation::ArcThreePointOperation(value) => ownership!(value),
        board::BoardFootprintOperation::CircleOperation(value) => ownership!(value),
        board::BoardFootprintOperation::RectOperation(value) => ownership!(value),
        board::BoardFootprintOperation::PlotPolyOperation(value) => ownership!(value),
        board::BoardFootprintOperation::BezierCurveOperation(value) => ownership!(value),
        board::BoardFootprintOperation::TextOperation(value) => ownership!(value),
        board::BoardFootprintOperation::FlashPadCircleOperation(value) => ownership!(value),
        board::BoardFootprintOperation::FlashPadOvalOperation(value) => ownership!(value),
        board::BoardFootprintOperation::FlashPadRectOperation(value) => ownership!(value),
        board::BoardFootprintOperation::FlashPadRoundRectOperation(value) => ownership!(value),
        board::BoardFootprintOperation::FlashPadCustomOperation(value) => ownership!(value),
        board::BoardFootprintOperation::FlashPadTrapezOperation(value) => ownership!(value),
        board::BoardFootprintOperation::StartBlockOperation(_)
        | board::BoardFootprintOperation::EndBlockOperation(_) => None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive embedded-footprint operation adapter"
)]
fn board_footprint_operation(operation: &board::BoardFootprintOperation) -> OperationData<'_> {
    let rendered = match operation {
        board::BoardFootprintOperation::ThickSegmentOperation(value) => OperationData::Segment {
            start: (value.start_x.get(), value.start_y.get()),
            end: (value.end_x.get(), value.end_y.get()),
            style: PrimitiveStyle {
                layer: value.layer.as_deref(),
                role: value.role.as_ref().map(ToString::to_string),
                layers: operation_layers(&value.layers),
                stroke: value.stroke_color.as_deref(),
                fill: None,
                width_nm: value.width_nm.get(),
                line_style: None,
                filled: false,
            },
        },
        board::BoardFootprintOperation::ArcThreePointOperation(value) => OperationData::Arc {
            start: (value.start_x.get(), value.start_y.get()),
            mid: (value.mid_x.get(), value.mid_y.get()),
            end: (value.end_x.get(), value.end_y.get()),
            style: enriched_style!(value, value.fill.to_string() != "NO_FILL"),
        },
        board::BoardFootprintOperation::CircleOperation(value) => OperationData::Circle {
            center: (value.cx.get(), value.cy.get()),
            diameter_nm: value.diameter_nm.get(),
            style: PrimitiveStyle {
                layer: value.layer.as_deref(),
                role: value.role.as_ref().map(ToString::to_string),
                layers: operation_layers(&value.layers),
                stroke: value.stroke_color.as_deref(),
                fill: value.fill_color.as_deref(),
                width_nm: value.width_nm.get(),
                line_style: value.line_style.as_ref().map(ToString::to_string),
                filled: value.fill.to_string() != "NO_FILL",
            },
        },
        board::BoardFootprintOperation::RectOperation(value) => OperationData::Rect {
            first: (value.x1.get(), value.y1.get()),
            second: (value.x2.get(), value.y2.get()),
            corner_radius_nm: value.corner_radius_nm.get(),
            style: enriched_style!(value, value.fill.to_string() != "NO_FILL"),
        },
        board::BoardFootprintOperation::PlotPolyOperation(value) => OperationData::Poly {
            points: value
                .points
                .iter()
                .map(|point| (point.0[0].get(), point.0[1].get()))
                .collect(),
            style: enriched_style!(value, value.fill.to_string() != "NO_FILL"),
        },
        board::BoardFootprintOperation::BezierCurveOperation(value) => OperationData::Bezier {
            points: [
                (value.start_x.get(), value.start_y.get()),
                (value.ctrl1_x.get(), value.ctrl1_y.get()),
                (value.ctrl2_x.get(), value.ctrl2_y.get()),
                (value.end_x.get(), value.end_y.get()),
            ],
            style: PrimitiveStyle {
                layer: value.layer.as_deref(),
                role: None,
                layers: &[],
                stroke: value.stroke_color.as_deref(),
                fill: None,
                width_nm: value.width_nm.get(),
                line_style: value.line_style.as_ref().map(ToString::to_string),
                filled: false,
            },
        },
        board::BoardFootprintOperation::TextOperation(value) => OperationData::Text(TextData {
            x: value.x.get(),
            y: value.y.get(),
            text: &value.text,
            color: &value.color,
            orient_deg: value.orient_deg,
            size_y_nm: value.size_y_nm.get(),
            h_align: value.h_align.to_string(),
            v_align: value.v_align.to_string(),
            italic: value.italic,
            bold: value.bold,
            multiline: value.multiline,
            font_face: &value.font_face,
            layer: value.layer.as_deref(),
            cache: value.render_cache.as_ref().map_or_else(Vec::new, |cache| {
                cache
                    .polygons
                    .iter()
                    .map(|polygon| {
                        polygon
                            .contours
                            .iter()
                            .map(|contour| {
                                contour
                                    .iter()
                                    .map(|point| (point.0[0].get(), point.0[1].get()))
                                    .collect()
                            })
                            .collect()
                    })
                    .collect()
            }),
            legacy_cache: value
                .render_cache_polygons
                .iter()
                .map(|polygon| {
                    polygon
                        .iter()
                        .map(|point| (point.0[0].get(), point.0[1].get()))
                        .collect()
                })
                .collect(),
        }),
        board::BoardFootprintOperation::FlashPadCircleOperation(value) => {
            OperationData::PadCircle {
                center: (value.x.get(), value.y.get()),
                diameter_nm: value.diameter_nm.get(),
                layers: operation_layers(&value.layers),
            }
        }
        board::BoardFootprintOperation::FlashPadOvalOperation(value) => OperationData::PadOval {
            center: (value.x.get(), value.y.get()),
            size: (value.size_x_nm.get(), value.size_y_nm.get()),
            angle_deg: value.orient_deg,
            layers: operation_layers(&value.layers),
        },
        board::BoardFootprintOperation::FlashPadRectOperation(value) => OperationData::PadRect {
            center: (value.x.get(), value.y.get()),
            size: (value.size_x_nm.get(), value.size_y_nm.get()),
            angle_deg: value.orient_deg,
            radius_nm: None,
            layers: operation_layers(&value.layers),
        },
        board::BoardFootprintOperation::FlashPadRoundRectOperation(value) => {
            OperationData::PadRect {
                center: (value.x.get(), value.y.get()),
                size: (value.size_x_nm.get(), value.size_y_nm.get()),
                angle_deg: value.orient_deg,
                radius_nm: Some(value.corner_radius_nm.get()),
                layers: operation_layers(&value.layers),
            }
        }
        board::BoardFootprintOperation::FlashPadCustomOperation(value) => {
            OperationData::PadCustom {
                center: (value.x.get(), value.y.get()),
                angle_deg: value.orient_deg,
                polygons: value
                    .polygons
                    .iter()
                    .map(|polygon| {
                        polygon
                            .iter()
                            .map(|point| (point.0[0].get(), point.0[1].get()))
                            .collect()
                    })
                    .collect(),
                layers: operation_layers(&value.layers),
            }
        }
        board::BoardFootprintOperation::FlashPadTrapezOperation(value) => {
            OperationData::PadTrapez {
                center: (value.x.get(), value.y.get()),
                angle_deg: value.orient_deg,
                corners: value
                    .corners
                    .0
                    .iter()
                    .map(|point| (point.0[0].get(), point.0[1].get()))
                    .collect(),
                layers: operation_layers(&value.layers),
            }
        }
        board::BoardFootprintOperation::StartBlockOperation(value) => OperationData::StartBlock {
            label: &value.label,
            data_uuid: &value.data_uuid,
            data_ref: value.data_ref.to_string(),
            object_id: &value.object_id,
            extra_attrs: board_pad_block_attrs(&value.extra_attrs),
        },
        board::BoardFootprintOperation::EndBlockOperation(_) => OperationData::EndBlock,
    };
    if let Some(ownership) = board_footprint_ownership(operation) {
        OperationData::Owned {
            ownership,
            operation: Box::new(rendered),
        }
    } else {
        rendered
    }
}

fn block_attrs(
    attrs: &impl std::ops::Deref<Target = std::collections::BTreeMap<String, String>>,
) -> Vec<(String, String)> {
    attrs
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive placed-symbol operation adapter"
)]
fn schematic_symbol_operation(
    operation: &schematic::SchematicSymbolOperation,
) -> OperationData<'_> {
    match operation {
        schematic::SchematicSymbolOperation::ThickSegmentOperation(value) => {
            schematic_common_operation!(value, ThickSegmentOperation)
        }
        schematic::SchematicSymbolOperation::ArcThreePointOperation(value) => {
            schematic_common_operation!(value, ArcThreePointOperation)
        }
        schematic::SchematicSymbolOperation::CircleOperation(value) => {
            schematic_common_operation!(value, CircleOperation)
        }
        schematic::SchematicSymbolOperation::RectOperation(value) => {
            schematic_common_operation!(value, RectOperation)
        }
        schematic::SchematicSymbolOperation::PlotPolyOperation(value) => {
            schematic_common_operation!(value, PlotPolyOperation)
        }
        schematic::SchematicSymbolOperation::BezierCurveOperation(value) => {
            schematic_common_operation!(value, BezierCurveOperation)
        }
        schematic::SchematicSymbolOperation::TextOperation(value) => {
            schematic_common_operation!(value, TextOperation)
        }
        schematic::SchematicSymbolOperation::PlotImageOperation(value) => {
            schematic_common_operation!(value, PlotImageOperation)
        }
        schematic::SchematicSymbolOperation::FlashPadCircleOperation(value) => {
            schematic_common_operation!(value, FlashPadCircleOperation)
        }
        schematic::SchematicSymbolOperation::FlashPadOvalOperation(value) => {
            schematic_common_operation!(value, FlashPadOvalOperation)
        }
        schematic::SchematicSymbolOperation::FlashPadRectOperation(value) => {
            schematic_common_operation!(value, FlashPadRectOperation)
        }
        schematic::SchematicSymbolOperation::FlashPadRoundRectOperation(value) => {
            schematic_common_operation!(value, FlashPadRoundRectOperation)
        }
        schematic::SchematicSymbolOperation::FlashPadCustomOperation(value) => {
            schematic_common_operation!(value, FlashPadCustomOperation)
        }
        schematic::SchematicSymbolOperation::FlashPadTrapezOperation(value) => {
            schematic_common_operation!(value, FlashPadTrapezOperation)
        }
        schematic::SchematicSymbolOperation::SchematicSymbolStartBlockOperation(value) => {
            OperationData::StartBlock {
                label: &value.label,
                data_uuid: &value.data_uuid,
                data_ref: value.data_ref.clone(),
                object_id: &value.object_id,
                extra_attrs: block_attrs(&value.extra_attrs),
            }
        }
        schematic::SchematicSymbolOperation::SchematicSymbolEndBlockOperation(_) => {
            OperationData::EndBlock
        }
    }
}

fn schematic_sheet_operation(operation: &schematic::SchematicSheetOperation) -> OperationData<'_> {
    match operation {
        schematic::SchematicSheetOperation::ThickSegmentOperation(value) => {
            schematic_common_operation!(value, ThickSegmentOperation)
        }
        schematic::SchematicSheetOperation::RectOperation(value) => {
            schematic_common_operation!(value, RectOperation)
        }
        schematic::SchematicSheetOperation::PlotPolyOperation(value) => {
            schematic_common_operation!(value, PlotPolyOperation)
        }
        schematic::SchematicSheetOperation::TextOperation(value) => {
            schematic_common_operation!(value, TextOperation)
        }
        schematic::SchematicSheetOperation::SchematicSheetStartBlockOperation(value) => {
            OperationData::StartBlock {
                label: &value.label,
                data_uuid: &value.data_uuid,
                data_ref: value.data_ref.clone(),
                object_id: &value.object_id,
                extra_attrs: block_attrs(&value.extra_attrs),
            }
        }
        schematic::SchematicSheetOperation::SchematicSheetEndBlockOperation(_) => {
            OperationData::EndBlock
        }
    }
}

pub fn render_footprint_svg(
    document: &footprint::FootprintPlotDocumentA0,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    validate_footprint_plot_document(document).map_err(|error| {
        SvgError::new(
            SvgErrorKind::InvalidDocument,
            format!("invalid footprint plot document: {error}"),
        )
    })?;
    let mut preflight = Preflight::default();
    for record in &document.records {
        add_operation_usages(
            &mut preflight,
            &record.operations,
            footprint_operation_usage,
        )?;
    }
    enforce_original_preflight(document.records.len(), preflight, limits)?;
    validate_footprint_layer_selection(document, context)?;
    render_typed_document(
        "MOD",
        &document.document_id,
        document.records.len(),
        document.records.iter().map(|record| RecordView {
            uuid: &record.uuid,
            kind: &record.kind,
            object_id: &record.object_id,
            operations: &record.operations,
        }),
        footprint_operation,
        viewport,
        context,
        limits,
    )
}

pub fn render_symbol_svg(
    document: &symbol::SymbolPlotDocumentA0,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    validate_symbol_plot_document(document).map_err(|error| {
        direct_error(
            SvgErrorKind::InvalidDocument,
            format!("invalid symbol plot document: {error}"),
        )
    })?;
    if !context.layer_selection().is_all() {
        return Err(SvgError::new(
            SvgErrorKind::UnsupportedSelector,
            "symbol rendering does not accept PCB layer selectors".to_owned(),
        ));
    }
    let mut preflight = Preflight::default();
    for record in &document.records {
        match record {
            symbol::SymbolPlotRecord::SymbolHeaderPlotRecord(record) => {
                add_operation_usages(&mut preflight, &record.operations, symbol_operation_usage)?
            }
            symbol::SymbolPlotRecord::LibSubsymbolPlotRecord(record) => {
                add_operation_usages(&mut preflight, &record.operations, symbol_operation_usage)?
            }
        }
    }
    enforce_original_preflight(document.records.len(), preflight, limits)?;
    let records = document.records.iter().map(|record| match record {
        symbol::SymbolPlotRecord::SymbolHeaderPlotRecord(record) => RecordView {
            uuid: &record.uuid,
            kind: &record.kind,
            object_id: &record.object_id,
            operations: &record.operations,
        },
        symbol::SymbolPlotRecord::LibSubsymbolPlotRecord(record) => RecordView {
            uuid: &record.uuid,
            kind: &record.kind,
            object_id: &record.object_id,
            operations: &record.operations,
        },
    });
    render_typed_document(
        "SYM",
        &document.document_id,
        document.records.len(),
        records,
        symbol_operation,
        viewport,
        context,
        limits,
    )
}

struct NormalizedRecord<'a> {
    uuid: &'a str,
    kind: String,
    object_id: &'a str,
    layer: Option<&'a str>,
    layers: &'a [String],
    placement: Option<(i64, i64, f64)>,
    operations: Vec<OperationData<'a>>,
}

pub fn render_board_svg(
    artifact: &ProjectedBoardPlotArtifact,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    validate_board_layer_selection(artifact, context)?;
    render_board_document(
        artifact.document(),
        viewport,
        context,
        limits,
        Some(artifact.render_facts().copper_stack()),
    )
}

pub fn render_board_document_svg(
    document: &board::BoardPlotDocumentA0,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    if !context.layer_selection().is_all() {
        return Err(SvgError::new(
            SvgErrorKind::UnsupportedSelector,
            "typed board documents require all-layer visibility; use ProjectedBoardPlotArtifact for filtering"
                .to_owned(),
        ));
    }
    render_board_document(document, viewport, context, limits, None)
}

fn render_board_document(
    document: &board::BoardPlotDocumentA0,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
    copper_stack: Option<&[String]>,
) -> Result<SvgArtifact, SvgError> {
    validate_board_plot_document(document).map_err(|error| {
        SvgError::new(
            SvgErrorKind::InvalidDocument,
            format!("invalid board plot document: {error}"),
        )
    })?;
    preflight_board_document(document, limits)?;
    let records = normalize_board_records(document);
    render_normalized_document(
        "PCB",
        &document.document_id,
        records,
        viewport,
        context,
        limits,
        None,
        copper_stack,
    )
}

pub fn render_schematic_svg(
    document: &schematic::SchematicPlotDocumentA0,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    render_schematic_document(document, viewport, context, limits, None)
}

pub fn render_schematic_page_svg(
    artifact: &ProjectedSchematicPagePlotArtifact,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    render_schematic_document(
        artifact.document(),
        viewport,
        context,
        limits,
        Some(artifact.occurrence_address()),
    )
}

fn render_schematic_document(
    document: &schematic::SchematicPlotDocumentA0,
    viewport: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
    occurrence_address: Option<&str>,
) -> Result<SvgArtifact, SvgError> {
    validate_schematic_plot_document(document).map_err(|error| {
        let kind = if matches!(
            error.code,
            "invalid_symbol_pin_block" | "invalid_sheet_pin_block"
        ) {
            SvgErrorKind::UnbalancedBlock
        } else {
            SvgErrorKind::InvalidDocument
        };
        SvgError::new(kind, format!("invalid schematic plot document: {error}"))
    })?;
    if !matches!(context.layer_selection(), crate::LayerSelection::All) {
        return Err(SvgError::new(
            SvgErrorKind::UnsupportedSelector,
            "schematic rendering does not accept PCB layer selectors".to_owned(),
        ));
    }
    preflight_schematic_document(document, limits)?;
    render_normalized_document(
        "SCH",
        &document.document_id,
        normalize_schematic_records(document),
        viewport,
        context,
        limits,
        occurrence_address,
        None,
    )
}

fn preflight_board_document(
    document: &board::BoardPlotDocumentA0,
    limits: SvgRenderLimits,
) -> Result<(), SvgError> {
    let mut counts = Preflight::default();
    for record in &document.records {
        match record {
            board::BoardPlotRecord::BoardGraphicPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::TrackSegmentPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::TrackArcPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::ViaPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::TablePlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::DimensionPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::ZoneFillPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::BoardTextPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::BoardTextBoxPlotRecord(value) => {
                add_operation_usages(&mut counts, &value.operations, board_operation_usage)?
            }
            board::BoardPlotRecord::BoardFootprintPlotRecord(value) => add_operation_usages(
                &mut counts,
                &value.operations,
                board_footprint_operation_usage,
            )?,
        }
    }
    enforce_original_preflight(document.records.len(), counts, limits)
}

macro_rules! schematic_common_record_preflight {
    ($counts:expr, $value:expr) => {
        add_operation_usages($counts, &$value.operations, schematic_operation_usage)?
    };
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive schematic record-union preflight before normalization"
)]
fn preflight_schematic_document(
    document: &schematic::SchematicPlotDocumentA0,
    limits: SvgRenderLimits,
) -> Result<(), SvgError> {
    let mut counts = Preflight::default();
    for record in &document.records {
        match record {
            schematic::SchematicPlotRecord::SheetHeaderPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::WirePlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::BusPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::BusEntryPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::JunctionPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::NoConnectPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::LabelPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::GlobalLabelPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::HierarchicalLabelPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::NetclassFlagPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::TextPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::TextBoxPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::GraphicPolylinePlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::GraphicArcPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::GraphicCirclePlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::GraphicRectanglePlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::GraphicBezierPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::RuleAreaPlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::ImagePlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::TablePlotRecord(value) => {
                schematic_common_record_preflight!(&mut counts, value)
            }
            schematic::SchematicPlotRecord::SymbolInstancePlotRecord(value) => {
                add_operation_usages(
                    &mut counts,
                    &value.operations,
                    schematic_symbol_operation_usage,
                )?
            }
            schematic::SchematicPlotRecord::SymbolOverplotPlotRecord(value) => {
                add_operation_usages(
                    &mut counts,
                    &value.operations,
                    schematic_symbol_operation_usage,
                )?
            }
            schematic::SchematicPlotRecord::SheetPlotRecord(value) => add_operation_usages(
                &mut counts,
                &value.operations,
                schematic_sheet_operation_usage,
            )?,
        }
    }
    enforce_original_preflight(document.records.len(), counts, limits)
}

macro_rules! normalize_schematic_common_record {
    ($value:expr) => {
        NormalizedRecord {
            uuid: &$value.uuid,
            kind: $value.kind.clone(),
            object_id: &$value.object_id,
            layer: None,
            layers: &[],
            placement: None,
            operations: $value.operations.iter().map(schematic_operation).collect(),
        }
    };
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive schematic record-union inventory"
)]
fn normalize_schematic_records(
    document: &schematic::SchematicPlotDocumentA0,
) -> Vec<NormalizedRecord<'_>> {
    document
        .records
        .iter()
        .map(|record| match record {
            schematic::SchematicPlotRecord::SheetHeaderPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::WirePlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::BusPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::BusEntryPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::JunctionPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::NoConnectPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::LabelPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::GlobalLabelPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::HierarchicalLabelPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::NetclassFlagPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::TextPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::TextBoxPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::GraphicPolylinePlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::GraphicArcPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::GraphicCirclePlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::GraphicRectanglePlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::GraphicBezierPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::RuleAreaPlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::ImagePlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::TablePlotRecord(value) => {
                normalize_schematic_common_record!(value)
            }
            schematic::SchematicPlotRecord::SymbolInstancePlotRecord(value) => NormalizedRecord {
                uuid: &value.uuid,
                kind: value.kind.clone(),
                object_id: &value.object_id,
                layer: None,
                layers: &[],
                placement: None,
                operations: value
                    .operations
                    .iter()
                    .map(schematic_symbol_operation)
                    .collect(),
            },
            schematic::SchematicPlotRecord::SymbolOverplotPlotRecord(value) => NormalizedRecord {
                uuid: &value.uuid,
                kind: value.kind.clone(),
                object_id: &value.object_id,
                layer: None,
                layers: &[],
                placement: None,
                operations: value
                    .operations
                    .iter()
                    .map(schematic_symbol_operation)
                    .collect(),
            },
            schematic::SchematicPlotRecord::SheetPlotRecord(value) => NormalizedRecord {
                uuid: &value.uuid,
                kind: value.kind.clone(),
                object_id: &value.object_id,
                layer: None,
                layers: &[],
                placement: None,
                operations: value
                    .operations
                    .iter()
                    .map(schematic_sheet_operation)
                    .collect(),
            },
        })
        .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive board record-union inventory"
)]
fn normalize_board_records(document: &board::BoardPlotDocumentA0) -> Vec<NormalizedRecord<'_>> {
    document
        .records
        .iter()
        .map(|record| match record {
            board::BoardPlotRecord::BoardGraphicPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.to_string(),
                &value.object_id,
                value.layer.as_deref(),
                &[],
                &value.operations,
            ),
            board::BoardPlotRecord::TrackSegmentPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                Some(&value.layer),
                &[],
                &value.operations,
            ),
            board::BoardPlotRecord::TrackArcPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                Some(&value.layer),
                &[],
                &value.operations,
            ),
            board::BoardPlotRecord::ViaPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                None,
                &value.layers,
                &value.operations,
            ),
            board::BoardPlotRecord::TablePlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                None,
                &value.layers,
                &value.operations,
            ),
            board::BoardPlotRecord::DimensionPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                None,
                &value.layers,
                &value.operations,
            ),
            board::BoardPlotRecord::ZoneFillPlotRecord(value) => {
                let mut record = normalized_common(
                    &value.uuid,
                    value.kind.clone(),
                    &value.object_id,
                    None,
                    &value.layers,
                    &value.operations,
                );
                for (operation, layer) in record.operations.iter_mut().zip(&value.fill_layers) {
                    set_operation_layer(operation, layer);
                }
                record
            }
            board::BoardPlotRecord::BoardTextPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                Some(&value.layer),
                &[],
                &value.operations,
            ),
            board::BoardPlotRecord::BoardTextBoxPlotRecord(value) => normalized_common(
                &value.uuid,
                value.kind.clone(),
                &value.object_id,
                Some(&value.layer),
                &[],
                &value.operations,
            ),
            board::BoardPlotRecord::BoardFootprintPlotRecord(value) => NormalizedRecord {
                uuid: &value.uuid,
                kind: value.kind.clone(),
                object_id: &value.object_id,
                // The footprint side is placement metadata. Its children and
                // ownership blocks carry the authoritative render layers.
                layer: None,
                layers: &[],
                placement: Some((
                    value.placement.x_nm.get(),
                    value.placement.y_nm.get(),
                    value.placement.angle_deg,
                )),
                operations: value
                    .operations
                    .iter()
                    .map(board_footprint_operation)
                    .collect(),
            },
        })
        .collect()
}

fn normalized_common<'a>(
    uuid: &'a str,
    kind: String,
    object_id: &'a str,
    layer: Option<&'a str>,
    layers: &'a [String],
    operations: &'a [board::PlotterOperation],
) -> NormalizedRecord<'a> {
    NormalizedRecord {
        uuid,
        kind,
        object_id,
        layer,
        layers,
        placement: None,
        operations: operations.iter().map(board_operation).collect(),
    }
}

fn set_operation_layer<'a>(operation: &mut OperationData<'a>, layer: &'a str) {
    match operation {
        OperationData::Owned { operation, .. } => set_operation_layer(operation, layer),
        OperationData::Segment { style, .. }
        | OperationData::Arc { style, .. }
        | OperationData::Circle { style, .. }
        | OperationData::Rect { style, .. }
        | OperationData::Poly { style, .. }
        | OperationData::Bezier { style, .. } => style.layer = Some(layer),
        OperationData::Text(text) => text.layer = Some(layer),
        _ => {}
    }
}

fn validate_board_layer_selection(
    artifact: &ProjectedBoardPlotArtifact,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    let crate::LayerSelection::Include {
        patterns,
        strict: true,
    } = context.layer_selection()
    else {
        return Ok(());
    };
    for pattern in patterns {
        if !matches!(pattern, crate::LayerPattern::Unlayered)
            && !artifact
                .render_facts()
                .enabled_layers()
                .iter()
                .any(|enabled| pattern.matches(Some(enabled)))
        {
            return Err(direct_error(
                SvgErrorKind::UnsupportedSelector,
                format!("strict board layer selector {pattern:?} matches no enabled layer"),
            ));
        }
    }
    Ok(())
}

fn validate_footprint_layer_selection(
    document: &footprint::FootprintPlotDocumentA0,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    let crate::LayerSelection::Include {
        patterns,
        strict: true,
    } = context.layer_selection()
    else {
        return Ok(());
    };
    let represented = document
        .records
        .iter()
        .flat_map(|record| &record.operations)
        .flat_map(|operation| {
            let operation = footprint_operation(operation);
            let (layer, layers, _) = operation_scope(&operation);
            layer
                .into_iter()
                .chain(layers.iter().map(String::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for pattern in patterns {
        let matched = match pattern {
            crate::LayerPattern::Unlayered => document.records.iter().any(|record| {
                record.operations.iter().any(|operation| {
                    let operation = footprint_operation(operation);
                    let (layer, layers, _) = operation_scope(&operation);
                    layer.is_none() && layers.is_empty()
                })
            }),
            _ => represented.iter().any(|layer| pattern.matches(Some(layer))),
        };
        if !matched {
            return Err(direct_error(
                SvgErrorKind::UnsupportedSelector,
                format!("strict footprint layer selector {pattern:?} matches no represented layer"),
            ));
        }
    }
    Ok(())
}

struct RecordView<'a, O> {
    uuid: &'a str,
    kind: &'a str,
    object_id: &'a str,
    operations: &'a [O],
}

#[derive(Clone, Copy, Debug)]
struct BoundsTransform {
    translate_x: f64,
    translate_y: f64,
    angle_deg: f64,
}

impl Default for BoundsTransform {
    fn default() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            angle_deg: 0.0,
        }
    }
}

impl BoundsTransform {
    fn point(self, point: Point) -> (f64, f64) {
        self.point_f64((point.0 as f64, point.1 as f64))
    }

    fn point_f64(self, point: (f64, f64)) -> (f64, f64) {
        let (x, y) = rotate_offset(point.0, point.1, self.angle_deg);
        (x + self.translate_x, y + self.translate_y)
    }
}

#[derive(Default)]
struct BoundsAccumulator {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    has_geometry: bool,
    unsupported_text: bool,
}

impl BoundsAccumulator {
    fn point(&mut self, point: (f64, f64), radius: f64) {
        let (min_x, min_y) = (point.0 - radius, point.1 - radius);
        let (max_x, max_y) = (point.0 + radius, point.1 + radius);
        if self.has_geometry {
            self.min_x = self.min_x.min(min_x);
            self.min_y = self.min_y.min(min_y);
            self.max_x = self.max_x.max(max_x);
            self.max_y = self.max_y.max(max_y);
        } else {
            self.min_x = min_x;
            self.min_y = min_y;
            self.max_x = max_x;
            self.max_y = max_y;
            self.has_geometry = true;
        }
    }

    fn finish(&self) -> Result<Option<SvgBounds>, SvgError> {
        if !self.has_geometry {
            return Ok(None);
        }
        for value in [self.min_x, self.min_y, self.max_x, self.max_y] {
            if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
                return Err(direct_error(
                    SvgErrorKind::ArithmeticOverflow,
                    "SVG visible bounds exceed i64",
                ));
            }
        }
        Ok(Some(SvgBounds {
            min_x_nm: self.min_x.floor() as i64,
            min_y_nm: self.min_y.floor() as i64,
            max_x_nm: self.max_x.ceil() as i64,
            max_y_nm: self.max_y.ceil() as i64,
        }))
    }
}

fn resolve_typed_viewport<'a, O: 'a>(
    records: impl Iterator<Item = RecordView<'a, O>>,
    adapter: fn(&'a O) -> OperationData<'a>,
    source_kind: &str,
    policy: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(SvgViewport, Option<SvgBounds>, Vec<SvgWarning>), SvgError> {
    let mut bounds = BoundsAccumulator::default();
    for record in records {
        let scope = RenderScope {
            semantic_role: Some(record_semantic_role(source_kind, record.kind)),
            ..RenderScope::default()
        };
        for operation in record.operations {
            add_operation_bounds(
                &adapter(operation),
                scope,
                context,
                BoundsTransform::default(),
                &mut bounds,
            )?;
        }
    }
    resolve_viewport(policy, bounds)
}

fn resolve_normalized_viewport(
    records: &[NormalizedRecord<'_>],
    source_kind: &str,
    policy: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    copper_stack: Option<&[String]>,
) -> Result<(SvgViewport, Option<SvgBounds>, Vec<SvgWarning>), SvgError> {
    let mut bounds = BoundsAccumulator::default();
    for record in records {
        let scope = RenderScope {
            layer: record.layer,
            layers: record.layers,
            semantic_role: Some(record_semantic_role(source_kind, &record.kind)),
            pin_text: None,
            pin_number: None,
            copper_stack,
        };
        let transform = record
            .placement
            .map_or_else(BoundsTransform::default, |value| BoundsTransform {
                translate_x: value.0 as f64,
                translate_y: value.1 as f64,
                angle_deg: -value.2,
            });
        add_operation_sequence_bounds(&record.operations, scope, context, transform, &mut bounds)?;
    }
    resolve_viewport(policy, bounds)
}

fn add_operation_sequence_bounds(
    operations: &[OperationData<'_>],
    scope: RenderScope<'_>,
    context: &ValidatedSvgRenderContextA1,
    transform: BoundsTransform,
    bounds: &mut BoundsAccumulator,
) -> Result<(), SvgError> {
    let mut index = 0usize;
    while index < operations.len() {
        match &operations[index] {
            start @ OperationData::StartBlock { .. } => {
                let end = matching_block_end(operations, index)?;
                add_operation_sequence_bounds(
                    &operations[index + 1..end],
                    block_scope(scope, start),
                    context,
                    transform,
                    bounds,
                )?;
                index = end + 1;
            }
            OperationData::EndBlock => {
                return Err(direct_error(
                    SvgErrorKind::UnbalancedBlock,
                    "plot document contains an orphan EndBlock",
                ));
            }
            operation => {
                add_operation_bounds(
                    operation,
                    block_child_scope(scope, operation),
                    context,
                    transform,
                    bounds,
                )?;
                index += 1;
            }
        }
    }
    Ok(())
}

fn resolve_viewport(
    policy: ViewportPolicy,
    bounds: BoundsAccumulator,
) -> Result<(SvgViewport, Option<SvgBounds>, Vec<SvgWarning>), SvgError> {
    let visible_bounds = bounds.finish()?;
    let mut warnings = Vec::new();
    match policy {
        ViewportPolicy::Explicit(viewport) => {
            validate_viewport(viewport)?;
            if bounds.unsupported_text {
                warnings.push(SvgWarning::BoundsUnavailableForUncachedText);
                Ok((viewport, None, warnings))
            } else {
                Ok((viewport, visible_bounds, warnings))
            }
        }
        ViewportPolicy::Fit(options) => {
            if bounds.unsupported_text {
                let viewport = options.fallback.ok_or_else(|| {
                    direct_error(
                        SvgErrorKind::UnsupportedFitText,
                        "fit viewport requires deterministic cached text bounds",
                    )
                })?;
                validate_viewport(viewport)?;
                warnings.push(SvgWarning::BoundsUnavailableForUncachedText);
                return Ok((viewport, None, warnings));
            }
            let Some(visible_bounds) = visible_bounds else {
                let viewport = options.fallback.ok_or_else(|| {
                    direct_error(
                        SvgErrorKind::EmptyBounds,
                        "fit viewport has no visible geometry",
                    )
                })?;
                validate_viewport(viewport)?;
                return Ok((viewport, None, warnings));
            };
            Ok((
                fit_viewport(visible_bounds, options)?,
                Some(visible_bounds),
                warnings,
            ))
        }
    }
}

fn validate_viewport(viewport: SvgViewport) -> Result<(), SvgError> {
    if viewport.width_nm == 0 || viewport.height_nm == 0 {
        Err(direct_error(
            SvgErrorKind::InvalidViewport,
            "explicit SVG viewport must be positive",
        ))
    } else {
        Ok(())
    }
}

fn fit_viewport(bounds: SvgBounds, options: SvgFitOptions) -> Result<SvgViewport, SvgError> {
    let padding = i128::from(options.padding_nm);
    let mut min_x = i128::from(bounds.min_x_nm) - padding;
    let mut min_y = i128::from(bounds.min_y_nm) - padding;
    let mut width = i128::from(bounds.max_x_nm) - i128::from(bounds.min_x_nm) + padding * 2;
    let mut height = i128::from(bounds.max_y_nm) - i128::from(bounds.min_y_nm) + padding * 2;
    let minimum = i128::from(options.min_extent_nm.max(1));
    if width < minimum {
        let difference = minimum - width;
        min_x -= difference / 2;
        width = minimum;
    }
    if height < minimum {
        let difference = minimum - height;
        min_y -= difference / 2;
        height = minimum;
    }
    Ok(SvgViewport {
        min_x_nm: i64::try_from(min_x)
            .map_err(|_| overflow_error("fitted viewport X exceeds i64"))?,
        min_y_nm: i64::try_from(min_y)
            .map_err(|_| overflow_error("fitted viewport Y exceeds i64"))?,
        width_nm: u64::try_from(width)
            .map_err(|_| overflow_error("fitted viewport width exceeds u64"))?,
        height_nm: u64::try_from(height)
            .map_err(|_| overflow_error("fitted viewport height exceeds u64"))?,
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "exhaustive geometry-union bounds traversal is kept auditable in one match"
)]
fn add_operation_bounds(
    operation: &OperationData<'_>,
    scope: RenderScope<'_>,
    context: &ValidatedSvgRenderContextA1,
    transform: BoundsTransform,
    bounds: &mut BoundsAccumulator,
) -> Result<(), SvgError> {
    if let OperationData::Owned { operation, .. } = operation {
        return add_operation_bounds(operation, scope, context, transform, bounds);
    }
    if !operation_visible(operation, context, scope) {
        return Ok(());
    }
    let stroke_radius = operation_stroke_radius(operation, scope, context)?;
    match operation {
        OperationData::Owned { .. } => unreachable!(),
        OperationData::Segment { start, end, .. } => {
            bounds.point(transform.point(*start), stroke_radius);
            bounds.point(transform.point(*end), stroke_radius);
        }
        OperationData::Arc {
            start, mid, end, ..
        } => {
            if let Some((center, radius)) = circumcircle(*start, *mid, *end) {
                add_arc_bounds(
                    *start,
                    *mid,
                    *end,
                    center,
                    radius,
                    transform,
                    stroke_radius,
                    bounds,
                );
            } else {
                bounds.point(transform.point(*start), stroke_radius);
                bounds.point(transform.point(*end), stroke_radius);
            }
        }
        OperationData::Circle {
            center,
            diameter_nm,
            ..
        } => {
            nonnegative(*diameter_nm, "diameter_nm")?;
            bounds.point(
                transform.point(*center),
                (*diameter_nm as f64) / 2.0 + stroke_radius,
            );
        }
        OperationData::Rect { first, second, .. } => {
            for point in [*first, (first.0, second.1), *second, (second.0, first.1)] {
                bounds.point(transform.point(point), stroke_radius);
            }
        }
        OperationData::Poly { points, .. } => {
            for point in points {
                bounds.point(transform.point(*point), stroke_radius);
            }
        }
        OperationData::Bezier { points, .. } => {
            for point in points {
                bounds.point(transform.point(*point), stroke_radius);
            }
        }
        OperationData::Text(text) => {
            let mut cached = false;
            for point in text.cache.iter().flatten().flatten() {
                bounds.point(transform.point(*point), 0.0);
                cached = true;
            }
            for point in text.legacy_cache.iter().flatten() {
                bounds.point(transform.point(*point), 0.0);
                cached = true;
            }
            if !cached {
                bounds.unsupported_text = true;
            }
        }
        OperationData::Image {
            center,
            width_nm,
            height_nm,
            ..
        } => {
            nonnegative(*width_nm, "width_nm")?;
            nonnegative(*height_nm, "height_nm")?;
            let half_x = (*width_nm as f64) / 2.0;
            let half_y = (*height_nm as f64) / 2.0;
            for point in [
                (center.0 as f64 - half_x, center.1 as f64 - half_y),
                (center.0 as f64 + half_x, center.1 as f64 - half_y),
                (center.0 as f64 + half_x, center.1 as f64 + half_y),
                (center.0 as f64 - half_x, center.1 as f64 + half_y),
            ] {
                bounds.point(
                    transform.point((point.0.round() as i64, point.1.round() as i64)),
                    0.0,
                );
            }
        }
        OperationData::PadCircle {
            center,
            diameter_nm,
            ..
        } => {
            nonnegative(*diameter_nm, "diameter_nm")?;
            bounds.point(
                transform.point(*center),
                (*diameter_nm as f64) / 2.0 + stroke_radius,
            );
        }
        OperationData::PadOval {
            center,
            size,
            angle_deg,
            ..
        }
        | OperationData::PadRect {
            center,
            size,
            angle_deg,
            ..
        } => add_rotated_box_bounds(*center, *size, *angle_deg, transform, stroke_radius, bounds)?,
        OperationData::PadCustom {
            center,
            angle_deg,
            polygons,
            ..
        } => {
            for polygon in polygons {
                add_local_points_bounds(
                    *center,
                    *angle_deg,
                    polygon,
                    transform,
                    stroke_radius,
                    bounds,
                );
            }
        }
        OperationData::PadTrapez {
            center,
            angle_deg,
            corners,
            ..
        } => add_local_points_bounds(
            *center,
            *angle_deg,
            corners,
            transform,
            stroke_radius,
            bounds,
        ),
        OperationData::StartBlock { .. } | OperationData::EndBlock => {}
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "arc bounds receives the already-computed circle and render transform"
)]
fn add_arc_bounds(
    start: Point,
    mid: Point,
    end: Point,
    center: (f64, f64),
    radius: f64,
    transform: BoundsTransform,
    stroke_radius: f64,
    bounds: &mut BoundsAccumulator,
) {
    let angle = |point: Point| (point.1 as f64 - center.1).atan2(point.0 as f64 - center.0);
    let start_angle = angle(start);
    let mid_angle = angle(mid);
    let end_angle = angle(end);
    let ccw_total = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let ccw_mid = (mid_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let counter_clockwise = ccw_mid <= ccw_total;
    let on_sweep = |candidate: f64| {
        let candidate_span = if counter_clockwise {
            (candidate - start_angle).rem_euclid(std::f64::consts::TAU)
        } else {
            (start_angle - candidate).rem_euclid(std::f64::consts::TAU)
        };
        let total_span = if counter_clockwise {
            ccw_total
        } else {
            (start_angle - end_angle).rem_euclid(std::f64::consts::TAU)
        };
        candidate_span <= total_span + f64::EPSILON * 16.0
    };
    for point in [start, end] {
        bounds.point(transform.point(point), stroke_radius);
    }
    let transform_radians = transform.angle_deg.to_radians();
    for world_cardinal in [
        0.0,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    ] {
        let local_angle = world_cardinal - transform_radians;
        if on_sweep(local_angle) {
            let point = (
                center.0 + radius * local_angle.cos(),
                center.1 + radius * local_angle.sin(),
            );
            bounds.point(transform.point_f64(point), stroke_radius);
        }
    }
}

fn add_rotated_box_bounds(
    center: Point,
    size: Point,
    angle_deg: f64,
    transform: BoundsTransform,
    radius: f64,
    bounds: &mut BoundsAccumulator,
) -> Result<(), SvgError> {
    nonnegative(size.0, "size_x_nm")?;
    nonnegative(size.1, "size_y_nm")?;
    let half_x = size.0 as f64 / 2.0;
    let half_y = size.1 as f64 / 2.0;
    for point in [
        (-half_x, -half_y),
        (half_x, -half_y),
        (half_x, half_y),
        (-half_x, half_y),
    ] {
        let rotated = rotate_offset(point.0, point.1, -angle_deg);
        bounds.point(
            transform.point((
                (center.0 as f64 + rotated.0).round() as i64,
                (center.1 as f64 + rotated.1).round() as i64,
            )),
            radius,
        );
    }
    Ok(())
}

fn add_local_points_bounds(
    center: Point,
    angle_deg: f64,
    points: &[Point],
    transform: BoundsTransform,
    radius: f64,
    bounds: &mut BoundsAccumulator,
) {
    for point in points {
        let rotated = rotate_offset(point.0 as f64, point.1 as f64, -angle_deg);
        bounds.point(
            transform.point((
                (center.0 as f64 + rotated.0).round() as i64,
                (center.1 as f64 + rotated.1).round() as i64,
            )),
            radius,
        );
    }
}

fn circumcircle(start: Point, mid: Point, end: Point) -> Option<((f64, f64), f64)> {
    let (ax, ay) = (start.0 as f64, start.1 as f64);
    let (bx, by) = (mid.0 as f64, mid.1 as f64);
    let (cx, cy) = (end.0 as f64, end.1 as f64);
    let divisor = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if divisor == 0.0 {
        return None;
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let x = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / divisor;
    let y = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / divisor;
    Some(((x, y), (ax - x).hypot(ay - y)))
}

fn operation_stroke_radius(
    operation: &OperationData<'_>,
    scope: RenderScope<'_>,
    context: &ValidatedSvgRenderContextA1,
) -> Result<f64, SvgError> {
    if let OperationData::Owned { operation, .. } = operation {
        return operation_stroke_radius(operation, scope, context);
    }
    let (layer, layers, kind) = operation_scope(operation);
    let layer = layer.or(scope.layer);
    let layers = if layers.is_empty() {
        scope.layers
    } else {
        layers
    };
    let (mut width, filled) = match operation {
        OperationData::Segment { style, .. }
        | OperationData::Arc { style, .. }
        | OperationData::Circle { style, .. }
        | OperationData::Rect { style, .. }
        | OperationData::Poly { style, .. }
        | OperationData::Bezier { style, .. } => (style.width_nm, style.filled),
        OperationData::PadCircle { .. }
        | OperationData::PadOval { .. }
        | OperationData::PadRect { .. }
        | OperationData::PadCustom { .. }
        | OperationData::PadTrapez { .. } => (0, true),
        _ => return Ok(0.0),
    };
    if width == 0
        && let Some(value) = context.fallback_style().stroke_width_nm()
    {
        width = i64::try_from(value).map_err(|_| {
            direct_error(
                SvgErrorKind::InvalidContext,
                "SVG context stroke width exceeds i64",
            )
        })?;
    }
    let semantic = operation_semantic_role(operation, layer, layers, scope.semantic_role);
    for (pattern, style) in context.layer_styles() {
        if (pattern.matches(layer)
            || pattern_matches_represented(pattern, layers, semantic, scope.copper_stack))
            && let Some(value) = style.stroke_width_nm()
        {
            width = i64::try_from(value).map_err(|_| {
                direct_error(
                    SvgErrorKind::InvalidContext,
                    "SVG context stroke width exceeds i64",
                )
            })?;
        }
    }
    if let Some(value) = context
        .semantic_style(semantic)
        .and_then(SvgStyleOverride::stroke_width_nm)
    {
        width = i64::try_from(value).map_err(|_| {
            direct_error(
                SvgErrorKind::InvalidContext,
                "SVG context stroke width exceeds i64",
            )
        })?;
    }
    if let Some(value) = context
        .operation_style(kind)
        .and_then(SvgStyleOverride::stroke_width_nm)
    {
        width = i64::try_from(value).map_err(|_| {
            direct_error(
                SvgErrorKind::InvalidContext,
                "SVG context stroke width exceeds i64",
            )
        })?;
    }
    nonnegative(width, "width_nm")?;
    if width == 0 && filled {
        Ok(0.0)
    } else {
        Ok((if width == 0 { 152_400 } else { width }) as f64 / 2.0)
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "internal engine receives the complete validated rendering boundary"
)]
fn render_typed_document<'a, O: 'a>(
    source_kind: &'static str,
    document_id: &str,
    record_count: usize,
    records: impl Clone + Iterator<Item = RecordView<'a, O>>,
    operation: fn(&'a O) -> OperationData<'a>,
    viewport_policy: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
) -> Result<SvgArtifact, SvgError> {
    ensure(record_count, limits.max_records, "records")?;
    let mut preflight = Preflight::default();
    for record in records.clone() {
        for item in record.operations {
            account(&operation(item), &mut preflight)?;
        }
    }
    ensure(preflight.operations, limits.max_operations, "operations")?;
    ensure(preflight.points, limits.max_points, "points")?;
    ensure(preflight.text_bytes, limits.max_text_bytes, "text bytes")?;
    ensure(
        preflight.image_bytes,
        limits.max_image_encoded_bytes,
        "image bytes",
    )?;
    let preflight_work = checked_add(
        checked_add(preflight.operations, preflight.points, "render work")?,
        checked_add(preflight.text_bytes, preflight.image_bytes, "render work")?,
        "render work",
    )?;
    ensure(preflight_work, limits.max_render_work, "render work")?;
    let bounds_work = checked_add(preflight.operations, preflight.points, "bounds work")?;
    ensure(bounds_work, limits.max_bounds_work, "bounds work")?;
    let (viewport, visible_bounds, warnings) = resolve_typed_viewport(
        records.clone(),
        operation,
        source_kind,
        viewport_policy,
        context,
    )?;
    let remaining_work = limits.max_render_work - preflight_work;
    let mut sink = SvgSink::new(
        limits.max_svg_bytes,
        limits.max_svg_elements,
        remaining_work,
    );
    open_svg(&mut sink, viewport, context)?;
    let mut blocks = BlockState::new(limits.max_block_depth);
    for record in records {
        open_record(&mut sink, &record, context)?;
        for item in record.operations {
            render_operation(
                &operation(item),
                &mut sink,
                context,
                RenderScope {
                    semantic_role: Some(record_semantic_role(source_kind, record.kind)),
                    ..RenderScope::default()
                },
                &mut blocks,
            )?;
        }
        sink.raw("</g>\n")?;
    }
    if blocks.depth != 0 {
        return Err(block_error("plot document contains an unclosed block"));
    }
    sink.raw("</g>\n</svg>\n")?;
    let (svg, svg_elements, serialization_work) = sink.finish()?;
    let render_work = checked_add(preflight_work, serialization_work, "render work")?;
    let svg_bytes = svg.len();
    let result_bytes = native_result_bytes(source_kind, document_id, &svg)?;
    ensure(result_bytes, limits.max_result_bytes, "result bytes")?;
    Ok(SvgArtifact {
        source_kind,
        document_id: document_id.to_owned(),
        svg,
        occurrence_address: None,
        viewport,
        visible_bounds,
        warnings,
        max_result_bytes: limits.max_result_bytes,
        metrics: SvgMetrics {
            records: record_count,
            operations: preflight.operations,
            points: preflight.points,
            text_bytes: preflight.text_bytes,
            image_encoded_bytes: preflight.image_bytes,
            block_depth: blocks.maximum_depth,
            svg_elements,
            render_work,
            svg_bytes,
            result_bytes,
            bounds_work,
        },
    })
}

#[allow(
    clippy::too_many_lines,
    reason = "normalized heterogeneous documents retain the same bounded two-pass engine"
)]
#[allow(
    clippy::too_many_arguments,
    reason = "the internal heterogeneous renderer receives the complete validated boundary"
)]
fn render_normalized_document(
    source_kind: &'static str,
    document_id: &str,
    records: Vec<NormalizedRecord<'_>>,
    viewport_policy: ViewportPolicy,
    context: &ValidatedSvgRenderContextA1,
    limits: SvgRenderLimits,
    occurrence_address: Option<&str>,
    copper_stack: Option<&[String]>,
) -> Result<SvgArtifact, SvgError> {
    ensure(records.len(), limits.max_records, "records")?;
    let mut preflight = Preflight::default();
    for record in &records {
        for operation in &record.operations {
            account(operation, &mut preflight)?;
        }
    }
    for (actual, maximum, name) in [
        (preflight.operations, limits.max_operations, "operations"),
        (preflight.points, limits.max_points, "points"),
        (preflight.text_bytes, limits.max_text_bytes, "text bytes"),
        (
            preflight.image_bytes,
            limits.max_image_encoded_bytes,
            "image bytes",
        ),
    ] {
        ensure(actual, maximum, name)?;
    }
    let preflight_work = checked_add(
        checked_add(preflight.operations, preflight.points, "render work")?,
        checked_add(preflight.text_bytes, preflight.image_bytes, "render work")?,
        "render work",
    )?;
    ensure(preflight_work, limits.max_render_work, "render work")?;
    let bounds_work = checked_add(preflight.operations, preflight.points, "bounds work")?;
    ensure(bounds_work, limits.max_bounds_work, "bounds work")?;
    let (viewport, visible_bounds, warnings) = resolve_normalized_viewport(
        &records,
        source_kind,
        viewport_policy,
        context,
        copper_stack,
    )?;
    let mut sink = SvgSink::new(
        limits.max_svg_bytes,
        limits.max_svg_elements,
        limits.max_render_work - preflight_work,
    );
    open_svg(&mut sink, viewport, context)?;
    let mut blocks = BlockState::new(limits.max_block_depth);
    for record in &records {
        open_normalized_record(&mut sink, record, context)?;
        let record_depth = blocks.depth;
        render_operation_sequence(
            &record.operations,
            &mut sink,
            context,
            RenderScope {
                layer: record.layer,
                layers: record.layers,
                semantic_role: Some(record_semantic_role(source_kind, &record.kind)),
                pin_text: None,
                pin_number: None,
                copper_stack,
            },
            &mut blocks,
        )?;
        if blocks.depth != record_depth {
            return Err(block_error("plot record contains an unclosed block"));
        }
        sink.raw("</g>\n")?;
    }
    sink.raw("</g>\n</svg>\n")?;
    let (svg, svg_elements, serialization_work) = sink.finish()?;
    let render_work = checked_add(preflight_work, serialization_work, "render work")?;
    let svg_bytes = svg.len();
    let result_bytes = native_result_bytes(source_kind, document_id, &svg)?;
    ensure(result_bytes, limits.max_result_bytes, "result bytes")?;
    Ok(SvgArtifact {
        source_kind,
        document_id: document_id.to_owned(),
        svg,
        occurrence_address: occurrence_address.map(str::to_owned),
        viewport,
        visible_bounds,
        warnings,
        max_result_bytes: limits.max_result_bytes,
        metrics: SvgMetrics {
            records: records.len(),
            operations: preflight.operations,
            points: preflight.points,
            text_bytes: preflight.text_bytes,
            image_encoded_bytes: preflight.image_bytes,
            block_depth: blocks.maximum_depth,
            svg_elements,
            render_work,
            svg_bytes,
            result_bytes,
            bounds_work,
        },
    })
}

fn native_result_bytes(source_kind: &str, document_id: &str, svg: &str) -> Result<usize, SvgError> {
    // Exact compact JSON length of NativeSvgRenderResultA0. The digest contents
    // are immaterial to size and are always 64 ASCII hex bytes.
    let svg_bytes_decimal = svg.len().to_string();
    let mut bytes = 2usize; // braces
    for (key, value) in [
        ("document_id", document_id),
        ("engine_version", env!("CARGO_PKG_VERSION")),
        ("profile", "plotter-base-a0"),
        ("source_kind", source_kind),
        ("svg_bytes", svg_bytes_decimal.as_str()),
        (
            "svg_sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
        ("svg_utf8", svg),
        ("type", "kicad_monkey.native.svg.result"),
        ("version", "a0"),
    ] {
        if bytes != 2 {
            bytes = checked_add(bytes, 1, "result bytes")?;
        }
        bytes = checked_add(bytes, json_string_bytes(key), "result bytes")?;
        bytes = checked_add(bytes, 1, "result bytes")?;
        bytes = checked_add(bytes, json_string_bytes(value), "result bytes")?;
    }
    Ok(bytes)
}

fn json_string_bytes(value: &str) -> usize {
    2 + value
        .chars()
        .map(|character| match character {
            '\"' | '\\' | '\u{0008}' | '\u{0009}' | '\n' | '\u{000C}' | '\r' => 2,
            '\u{0000}'..='\u{001F}' => 6,
            _ => character.len_utf8(),
        })
        .sum::<usize>()
}

fn account(operation: &OperationData<'_>, counts: &mut Preflight) -> Result<(), SvgError> {
    if let OperationData::Owned { operation, .. } = operation {
        return account(operation, counts);
    }
    counts.operations = checked_add(counts.operations, 1, "operations")?;
    let (points, text, image) = match operation {
        OperationData::Owned { .. } => unreachable!("owned operations were unwrapped above"),
        OperationData::Segment { .. } | OperationData::Rect { .. } => (2, 0, 0),
        OperationData::Arc { .. } => (3, 0, 0),
        OperationData::Circle { .. }
        | OperationData::PadCircle { .. }
        | OperationData::PadOval { .. }
        | OperationData::PadRect { .. } => (1, 0, 0),
        OperationData::Poly { points, .. } => (points.len(), 0, 0),
        OperationData::Bezier { .. } | OperationData::PadTrapez { .. } => (4, 0, 0),
        OperationData::Text(text) => {
            let cache_points = text
                .cache
                .iter()
                .flatten()
                .chain(&text.legacy_cache)
                .try_fold(0usize, |sum, points| {
                    checked_add(sum, points.len(), "points")
                })?;
            (checked_add(1, cache_points, "points")?, text.text.len(), 0)
        }
        OperationData::Image { data, .. } => (1, 0, data.len()),
        OperationData::PadCustom { polygons, .. } => (
            polygons.iter().try_fold(1usize, |sum, points| {
                checked_add(sum, points.len(), "points")
            })?,
            0,
            0,
        ),
        OperationData::StartBlock { .. } | OperationData::EndBlock => (0, 0, 0),
    };
    counts.points = checked_add(counts.points, points, "points")?;
    counts.text_bytes = checked_add(counts.text_bytes, text, "text bytes")?;
    counts.image_bytes = checked_add(counts.image_bytes, image, "image bytes")?;
    Ok(())
}

fn open_svg(
    sink: &mut SvgSink,
    viewport: SvgViewport,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    sink.raw("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    sink.element()?;
    sink.raw(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">\n",
        format_mm(viewport.width_nm),
        format_mm(viewport.height_nm),
        viewport.width_nm,
        viewport.height_nm,
    ))?;
    if let SvgBackground::Opaque(color) = context.background() {
        sink.element()?;
        sink.raw("<rect x=\"0\" y=\"0\"")?;
        sink.attribute("width", &viewport.width_nm.to_string())?;
        sink.attribute("height", &viewport.height_nm.to_string())?;
        color_attribute(sink, "fill", color.as_str(), None)?;
        sink.raw("/>\n")?;
    }
    sink.element()?;
    sink.raw("<g")?;
    let x = viewport
        .min_x_nm
        .checked_neg()
        .ok_or_else(|| overflow_error("viewport X offset overflowed"))?;
    let y = viewport
        .min_y_nm
        .checked_neg()
        .ok_or_else(|| overflow_error("viewport Y offset overflowed"))?;
    sink.attribute("transform", &format!("translate({x} {y})"))?;
    sink.raw(">\n")
}

fn open_record<O>(
    sink: &mut SvgSink,
    record: &RecordView<'_, O>,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<g")?;
    if context.identity_mode() == SvgIdentityMode::Full {
        sink.id_attribute(record.uuid)?;
        sink.attribute("data-ref", record.kind)?;
        sink.attribute("data-object-id", record.object_id)?;
    }
    sink.raw(">\n")
}

fn open_normalized_record(
    sink: &mut SvgSink,
    record: &NormalizedRecord<'_>,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<g")?;
    if context.identity_mode() == SvgIdentityMode::Full {
        sink.id_attribute(record.uuid)?;
        sink.attribute("data-ref", &record.kind)?;
        sink.attribute("data-object-id", record.object_id)?;
    }
    if let Some((x, y, angle)) = record.placement {
        let mut transforms = Vec::with_capacity(2);
        if x != 0 || y != 0 {
            transforms.push(format!("translate({x} {y})"));
        }
        if angle != 0.0 {
            transforms.push(format!("rotate({})", number(-angle)));
        }
        if !transforms.is_empty() {
            sink.attribute("transform", &transforms.join(" "))?;
        }
    }
    sink.raw(">\n")
}

fn open_ownership_group(
    sink: &mut SvgSink,
    ownership: &OwnershipData<'_>,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<g")?;
    if context.identity_mode() == SvgIdentityMode::Full {
        if let Some(label) = ownership.label {
            sink.id_attribute(label)?;
        }
        for (name, value) in [
            ("data-uuid", ownership.data_uuid),
            ("data-ref", ownership.data_ref.as_deref()),
            ("data-object-id", ownership.object_id),
        ] {
            if let Some(value) = value.filter(|value| !value.is_empty()) {
                sink.attribute(name, value)?;
            }
        }
        for (key, value) in &ownership.extra_attrs {
            if !value.is_empty() {
                sink.attribute(&format!("data-{key}"), value)?;
            }
        }
    }
    sink.raw(">\n")
}

#[derive(Clone, Copy, Default)]
struct RenderScope<'a> {
    layer: Option<&'a str>,
    layers: &'a [String],
    semantic_role: Option<SvgSemanticRole>,
    pin_text: Option<PinTextKind>,
    pin_number: Option<&'a str>,
    copper_stack: Option<&'a [String]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PinTextKind {
    Name,
    Number,
}

struct BlockState {
    depth: usize,
    maximum_depth: usize,
    limit: usize,
}

impl BlockState {
    const fn new(limit: usize) -> Self {
        Self {
            depth: 0,
            maximum_depth: 0,
            limit,
        }
    }
}

fn render_operation_sequence(
    operations: &[OperationData<'_>],
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
    blocks: &mut BlockState,
) -> Result<(), SvgError> {
    let mut index = 0usize;
    while index < operations.len() {
        match &operations[index] {
            start @ OperationData::StartBlock { .. } => {
                let end = matching_block_end(operations, index)?;
                let child_scope = block_scope(scope, start);
                if sequence_has_visible(&operations[index + 1..end], context, child_scope)? {
                    emit_start_block(start, sink, context, blocks)?;
                    render_operation_sequence(
                        &operations[index + 1..end],
                        sink,
                        context,
                        child_scope,
                        blocks,
                    )?;
                    emit_end_block(sink, blocks)?;
                }
                index = end + 1;
            }
            OperationData::EndBlock => {
                return Err(block_error("plot document contains an orphan EndBlock"));
            }
            operation => {
                let operation_scope = block_child_scope(scope, operation);
                render_operation(operation, sink, context, operation_scope, blocks)?;
                index += 1;
            }
        }
    }
    Ok(())
}

fn sequence_has_visible(
    operations: &[OperationData<'_>],
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> Result<bool, SvgError> {
    let mut index = 0usize;
    while index < operations.len() {
        match &operations[index] {
            start @ OperationData::StartBlock { .. } => {
                let end = matching_block_end(operations, index)?;
                if sequence_has_visible(
                    &operations[index + 1..end],
                    context,
                    block_scope(scope, start),
                )? {
                    return Ok(true);
                }
                index = end + 1;
            }
            OperationData::EndBlock => {
                return Err(block_error("plot document contains an orphan EndBlock"));
            }
            operation => {
                if operation_visible(operation, context, block_child_scope(scope, operation)) {
                    return Ok(true);
                }
                index += 1;
            }
        }
    }
    Ok(false)
}

fn matching_block_end(operations: &[OperationData<'_>], start: usize) -> Result<usize, SvgError> {
    let mut depth = 0usize;
    for (index, operation) in operations.iter().enumerate().skip(start + 1) {
        match operation {
            OperationData::StartBlock { .. } => depth = depth.saturating_add(1),
            OperationData::EndBlock if depth == 0 => return Ok(index),
            OperationData::EndBlock => depth -= 1,
            _ => {}
        }
    }
    Err(block_error("plot document contains an unclosed block"))
}

fn block_scope<'a>(scope: RenderScope<'a>, start: &'a OperationData<'a>) -> RenderScope<'a> {
    let semantic_role = match start {
        OperationData::StartBlock { data_ref, .. } if data_ref.contains("pin") => {
            Some(SvgSemanticRole::Pin)
        }
        _ => scope.semantic_role,
    };
    RenderScope {
        semantic_role,
        pin_text: None,
        pin_number: match start {
            OperationData::StartBlock { extra_attrs, .. } => extra_attrs
                .iter()
                .find(|(key, _)| key == "pin")
                .map(|(_, value)| value.as_str()),
            _ => None,
        },
        ..scope
    }
}

fn block_child_scope<'a>(
    scope: RenderScope<'a>,
    operation: &'a OperationData<'a>,
) -> RenderScope<'a> {
    if scope.semantic_role != Some(SvgSemanticRole::Pin)
        || !matches!(operation, OperationData::Text(_))
    {
        return RenderScope {
            pin_text: None,
            ..scope
        };
    }
    let OperationData::Text(text) = operation else {
        unreachable!();
    };
    RenderScope {
        pin_text: Some(if scope.pin_number == Some(text.text) {
            PinTextKind::Number
        } else {
            PinTextKind::Name
        }),
        ..scope
    }
}

fn emit_start_block(
    operation: &OperationData<'_>,
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    blocks: &mut BlockState,
) -> Result<(), SvgError> {
    let OperationData::StartBlock {
        label,
        data_uuid,
        data_ref,
        object_id,
        extra_attrs,
    } = operation
    else {
        return Err(block_error("expected StartBlock operation"));
    };
    blocks.depth = blocks
        .depth
        .checked_add(1)
        .ok_or_else(|| overflow_error("block depth overflowed"))?;
    ensure(blocks.depth, blocks.limit, "block depth")?;
    blocks.maximum_depth = blocks.maximum_depth.max(blocks.depth);
    sink.element()?;
    sink.raw("<g")?;
    if context.identity_mode() == SvgIdentityMode::Full {
        sink.id_attribute(label)?;
        if !data_uuid.is_empty() {
            sink.attribute("data-uuid", data_uuid)?;
        }
        if !data_ref.is_empty() {
            sink.attribute("data-ref", data_ref)?;
        }
        if !object_id.is_empty() {
            sink.attribute("data-object-id", object_id)?;
        }
        for (key, value) in extra_attrs {
            if !value.is_empty() {
                sink.attribute(&format!("data-{key}"), value)?;
            }
        }
    }
    sink.raw(">\n")
}

fn emit_end_block(sink: &mut SvgSink, blocks: &mut BlockState) -> Result<(), SvgError> {
    if blocks.depth == 0 {
        return Err(block_error("plot document contains an orphan EndBlock"));
    }
    blocks.depth -= 1;
    sink.raw("</g>\n")
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "exhaustive typed operation emission remains one auditable match"
)]
fn render_operation(
    operation: &OperationData<'_>,
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
    blocks: &mut BlockState,
) -> Result<(), SvgError> {
    let inherited_layer = scope.layer;
    let inherited_layers = scope.layers;
    if !operation_visible(operation, context, scope) {
        return Ok(());
    }
    match operation {
        OperationData::Owned {
            ownership,
            operation,
        } => {
            open_ownership_group(sink, ownership, context)?;
            render_operation(operation, sink, context, scope, blocks)?;
            sink.raw("</g>\n")
        }
        OperationData::Segment { start, end, style } => {
            sink.element()?;
            sink.raw("<line")?;
            point_attributes(sink, *start, *end)?;
            emit_inherited_style(
                sink,
                style,
                PlotterOperationKind::ThickSegment,
                context,
                scope,
            )?;
            sink.raw("/>\n")
        }
        OperationData::Arc {
            start,
            mid,
            end,
            style,
        } => {
            sink.element()?;
            if let Some((radius, large, sweep)) = arc_parameters(*start, *mid, *end) {
                sink.raw(&format!(
                    "<path d=\"M {} {} A {} {} 0 {} {} {} {}\"",
                    start.0,
                    start.1,
                    number(radius),
                    number(radius),
                    u8::from(large),
                    u8::from(sweep),
                    end.0,
                    end.1,
                ))?;
            } else {
                sink.raw("<line")?;
                point_attributes(sink, *start, *end)?;
            }
            emit_inherited_style(
                sink,
                style,
                PlotterOperationKind::ArcThreePoint,
                context,
                scope,
            )?;
            sink.raw("/>\n")
        }
        OperationData::Circle {
            center,
            diameter_nm,
            style,
        } => {
            nonnegative(*diameter_nm, "diameter_nm")?;
            sink.element()?;
            sink.raw("<circle")?;
            sink.attribute("cx", &center.0.to_string())?;
            sink.attribute("cy", &center.1.to_string())?;
            sink.attribute("r", &half_number(i128::from(*diameter_nm)))?;
            emit_inherited_style(sink, style, PlotterOperationKind::Circle, context, scope)?;
            sink.raw("/>\n")
        }
        OperationData::Rect {
            first,
            second,
            corner_radius_nm,
            style,
        } => {
            sink.element()?;
            sink.raw("<rect")?;
            sink.attribute("x", &first.0.min(second.0).to_string())?;
            sink.attribute("y", &first.1.min(second.1).to_string())?;
            sink.attribute("width", &first.0.abs_diff(second.0).to_string())?;
            sink.attribute("height", &first.1.abs_diff(second.1).to_string())?;
            if *corner_radius_nm > 0 {
                sink.attribute("rx", &corner_radius_nm.to_string())?;
                sink.attribute("ry", &corner_radius_nm.to_string())?;
            }
            emit_inherited_style(sink, style, PlotterOperationKind::Rect, context, scope)?;
            sink.raw("/>\n")
        }
        OperationData::Poly { points, style } => {
            let closed = style.filled || points.first() == points.last();
            sink.element()?;
            sink.raw(if closed {
                "<polygon points=\""
            } else {
                "<polyline points=\""
            })?;
            write_points(sink, points)?;
            sink.raw("\"")?;
            emit_inherited_style(sink, style, PlotterOperationKind::PlotPoly, context, scope)?;
            sink.raw("/>\n")
        }
        OperationData::Bezier { points, style } => {
            sink.element()?;
            sink.raw(&format!(
                "<path d=\"M {} {} C {} {}, {} {}, {} {}\"",
                points[0].0,
                points[0].1,
                points[1].0,
                points[1].1,
                points[2].0,
                points[2].1,
                points[3].0,
                points[3].1,
            ))?;
            emit_inherited_style(
                sink,
                style,
                PlotterOperationKind::BezierCurve,
                context,
                scope,
            )?;
            sink.raw("/>\n")
        }
        OperationData::Text(text) => render_text(text, sink, context, scope),
        OperationData::Image {
            center,
            width_nm,
            height_nm,
            format,
            data,
        } => render_image(*center, *width_nm, *height_nm, format, data, sink, context),
        OperationData::PadCircle {
            center,
            diameter_nm,
            layers,
        } => {
            nonnegative(*diameter_nm, "diameter_nm")?;
            sink.element()?;
            sink.raw("<circle")?;
            sink.attribute("cx", &center.0.to_string())?;
            sink.attribute("cy", &center.1.to_string())?;
            sink.attribute("r", &half_number(i128::from(*diameter_nm)))?;
            emit_pad_style(
                sink,
                PlotterOperationKind::FlashPadCircle,
                context,
                RenderScope {
                    layer: inherited_layer,
                    layers: if layers.is_empty() {
                        inherited_layers
                    } else {
                        layers
                    },
                    ..scope
                },
            )?;
            sink.raw("/>\n")
        }
        OperationData::PadOval {
            center,
            size,
            angle_deg,
            layers,
        } => render_pad_oval(
            *center,
            *size,
            *angle_deg,
            sink,
            context,
            RenderScope {
                layer: inherited_layer,
                layers: if layers.is_empty() {
                    inherited_layers
                } else {
                    layers
                },
                ..scope
            },
        ),
        OperationData::PadRect {
            center,
            size,
            angle_deg,
            radius_nm,
            layers,
        } => render_pad_rect(
            *center,
            *size,
            *angle_deg,
            *radius_nm,
            sink,
            context,
            RenderScope {
                layer: inherited_layer,
                layers: if layers.is_empty() {
                    inherited_layers
                } else {
                    layers
                },
                ..scope
            },
        ),
        OperationData::PadCustom {
            center,
            angle_deg,
            polygons,
            layers,
        } => {
            for polygon in polygons {
                render_local_polygon(
                    *center,
                    *angle_deg,
                    polygon,
                    sink,
                    context,
                    PlotterOperationKind::FlashPadCustom,
                    RenderScope {
                        layer: inherited_layer,
                        layers: if layers.is_empty() {
                            inherited_layers
                        } else {
                            layers
                        },
                        ..scope
                    },
                )?;
            }
            Ok(())
        }
        OperationData::PadTrapez {
            center,
            angle_deg,
            corners,
            layers,
        } => render_local_polygon(
            *center,
            *angle_deg,
            corners,
            sink,
            context,
            PlotterOperationKind::FlashPadTrapez,
            RenderScope {
                layer: inherited_layer,
                layers: if layers.is_empty() {
                    inherited_layers
                } else {
                    layers
                },
                ..scope
            },
        ),
        OperationData::StartBlock { .. } => emit_start_block(operation, sink, context, blocks),
        OperationData::EndBlock => emit_end_block(sink, blocks),
    }
}

fn operation_visible(
    operation: &OperationData<'_>,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> bool {
    let (operation_layer, operation_layers, kind) = operation_scope(operation);
    let layer = operation_layer.or(scope.layer);
    let layers = if operation_layers.is_empty() {
        scope.layers
    } else {
        operation_layers
    };
    let preliminary_semantic =
        operation_semantic_role(operation, layer, layers, scope.semantic_role);
    let selected = context.layer_selection().is_all()
        || layer.is_some_and(|layer| context.layer_selection().matches(Some(layer)))
        || represented_layers_match(
            context.layer_selection(),
            layers,
            preliminary_semantic,
            scope.copper_stack,
        )
        || (layer.is_none() && layers.is_empty() && context.layer_selection().matches(None));
    if !selected {
        return false;
    }
    let semantic = preliminary_semantic;
    let mut visible = context.fallback_style().visible().unwrap_or(true);
    for (pattern, style) in context.layer_styles() {
        if pattern.matches(layer)
            || pattern_matches_represented(
                pattern,
                layers,
                preliminary_semantic,
                scope.copper_stack,
            )
        {
            visible = style.visible().unwrap_or(visible);
        }
    }
    if let Some(style) = context.semantic_style(semantic) {
        visible = style.visible().unwrap_or(visible);
    }
    if let Some(style) = context.operation_style(kind) {
        visible = style.visible().unwrap_or(visible);
    }
    visible
        && match scope.pin_text {
            Some(PinTextKind::Name) => context.visibility().pin_names(),
            Some(PinTextKind::Number) => context.visibility().pin_numbers(),
            None => true,
        }
}

fn represented_layers_match(
    selection: &crate::LayerSelection,
    layers: &[String],
    semantic: SvgSemanticRole,
    copper_stack: Option<&[String]>,
) -> bool {
    if layers.iter().any(|layer| selection.matches(Some(layer))) {
        return true;
    }
    let matches_candidate = |candidate: &str| selection.matches(Some(candidate));
    if layers.iter().any(|layer| layer == "*.Mask")
        && ["F.Mask", "B.Mask"].into_iter().any(matches_candidate)
    {
        return true;
    }
    let Some(copper_stack) = copper_stack else {
        return false;
    };
    if semantic == SvgSemanticRole::Drill
        && !layers
            .iter()
            .any(|layer| layer.ends_with(".Cu") || matches!(layer.as_str(), "*.Cu" | "F&B.Cu"))
        && copper_stack.iter().any(|layer| matches_candidate(layer))
    {
        return true;
    }
    if layers.iter().any(|layer| layer == "*.Cu")
        && copper_stack.iter().any(|layer| matches_candidate(layer))
    {
        return true;
    }
    if layers.iter().any(|layer| layer == "F&B.Cu") {
        let candidates = if semantic == SvgSemanticRole::Drill {
            copper_stack
        } else if copper_stack.len() > 1 {
            &copper_stack[..1]
        } else {
            copper_stack
        };
        if candidates.iter().any(|layer| matches_candidate(layer))
            || (semantic != SvgSemanticRole::Drill
                && copper_stack
                    .last()
                    .is_some_and(|layer| matches_candidate(layer)))
        {
            return true;
        }
    }
    if semantic != SvgSemanticRole::Drill {
        return false;
    }
    let mut indexes = layers
        .iter()
        .filter_map(|layer| copper_stack.iter().position(|candidate| candidate == layer));
    let Some(first) = indexes.next() else {
        return false;
    };
    let last = indexes.next_back().unwrap_or(first);
    let (start, end) = if first <= last {
        (first, last)
    } else {
        (last, first)
    };
    copper_stack[start..=end]
        .iter()
        .any(|layer| matches_candidate(layer))
}

fn pattern_matches_represented(
    pattern: &crate::LayerPattern,
    layers: &[String],
    semantic: SvgSemanticRole,
    copper_stack: Option<&[String]>,
) -> bool {
    if layers.iter().any(|layer| pattern.matches(Some(layer))) {
        return true;
    }
    if layers.iter().any(|layer| layer == "*.Mask")
        && ["F.Mask", "B.Mask"]
            .into_iter()
            .any(|layer| pattern.matches(Some(layer)))
    {
        return true;
    }
    let Some(copper_stack) = copper_stack else {
        return false;
    };
    if semantic == SvgSemanticRole::Drill
        && !layers
            .iter()
            .any(|layer| layer.ends_with(".Cu") || matches!(layer.as_str(), "*.Cu" | "F&B.Cu"))
        && copper_stack
            .iter()
            .any(|layer| pattern.matches(Some(layer)))
    {
        return true;
    }
    if layers.iter().any(|layer| layer == "*.Cu")
        && copper_stack
            .iter()
            .any(|layer| pattern.matches(Some(layer)))
    {
        return true;
    }
    if layers.iter().any(|layer| layer == "F&B.Cu") {
        if semantic == SvgSemanticRole::Drill
            && copper_stack
                .iter()
                .any(|layer| pattern.matches(Some(layer)))
        {
            return true;
        }
        if copper_stack
            .first()
            .is_some_and(|layer| pattern.matches(Some(layer)))
            || copper_stack
                .last()
                .is_some_and(|layer| pattern.matches(Some(layer)))
        {
            return true;
        }
    }
    if semantic != SvgSemanticRole::Drill {
        return false;
    }
    let indexes = layers
        .iter()
        .filter_map(|layer| copper_stack.iter().position(|candidate| candidate == layer))
        .collect::<Vec<_>>();
    let (Some(first), Some(last)) = (indexes.first(), indexes.last()) else {
        return false;
    };
    let (start, end) = if first <= last {
        (*first, *last)
    } else {
        (*last, *first)
    };
    copper_stack[start..=end]
        .iter()
        .any(|layer| pattern.matches(Some(layer)))
}

fn operation_semantic_role(
    operation: &OperationData<'_>,
    layer: Option<&str>,
    layers: &[String],
    inherited: Option<SvgSemanticRole>,
) -> SvgSemanticRole {
    match operation {
        OperationData::Owned { operation, .. } => {
            operation_semantic_role(operation, layer, layers, inherited)
        }
        OperationData::Text(_) => inherited
            .filter(|role| *role == SvgSemanticRole::Pin)
            .unwrap_or(SvgSemanticRole::Text),
        OperationData::Image { .. } => SvgSemanticRole::Image,
        OperationData::Segment { style, .. }
        | OperationData::Arc { style, .. }
        | OperationData::Circle { style, .. }
        | OperationData::Rect { style, .. }
        | OperationData::Poly { style, .. }
        | OperationData::Bezier { style, .. } => {
            let mut style = style.clone();
            style.layer = layer;
            style.layers = layers;
            semantic_role(&style, inherited)
        }
        OperationData::PadCircle { .. }
        | OperationData::PadOval { .. }
        | OperationData::PadRect { .. }
        | OperationData::PadCustom { .. }
        | OperationData::PadTrapez { .. } => {
            let style = PrimitiveStyle {
                layer,
                layers,
                role: None,
                stroke: None,
                fill: None,
                width_nm: 0,
                line_style: None,
                filled: true,
            };
            semantic_role(&style, inherited)
        }
        OperationData::StartBlock { data_ref, .. } if data_ref.contains("pin") => {
            SvgSemanticRole::Pin
        }
        OperationData::StartBlock { .. } | OperationData::EndBlock => {
            inherited.unwrap_or(SvgSemanticRole::Other)
        }
    }
}

fn operation_scope<'a>(
    operation: &'a OperationData<'a>,
) -> (Option<&'a str>, &'a [String], PlotterOperationKind) {
    match operation {
        OperationData::Owned { operation, .. } => operation_scope(operation),
        OperationData::Segment { style, .. } => (
            style.layer,
            style.layers,
            PlotterOperationKind::ThickSegment,
        ),
        OperationData::Arc { style, .. } => (
            style.layer,
            style.layers,
            PlotterOperationKind::ArcThreePoint,
        ),
        OperationData::Circle { style, .. } => {
            (style.layer, style.layers, PlotterOperationKind::Circle)
        }
        OperationData::Rect { style, .. } => {
            (style.layer, style.layers, PlotterOperationKind::Rect)
        }
        OperationData::Poly { style, .. } => {
            (style.layer, style.layers, PlotterOperationKind::PlotPoly)
        }
        OperationData::Bezier { style, .. } => {
            (style.layer, style.layers, PlotterOperationKind::BezierCurve)
        }
        OperationData::Text(text) => (text.layer, &[], PlotterOperationKind::Text),
        OperationData::Image { .. } => (None, &[], PlotterOperationKind::PlotImage),
        OperationData::PadCircle { layers, .. } => {
            (None, layers, PlotterOperationKind::FlashPadCircle)
        }
        OperationData::PadOval { layers, .. } => (None, layers, PlotterOperationKind::FlashPadOval),
        OperationData::PadRect {
            radius_nm: None,
            layers,
            ..
        } => (None, layers, PlotterOperationKind::FlashPadRect),
        OperationData::PadRect {
            radius_nm: Some(_),
            layers,
            ..
        } => (None, layers, PlotterOperationKind::FlashPadRoundRect),
        OperationData::PadCustom { layers, .. } => {
            (None, layers, PlotterOperationKind::FlashPadCustom)
        }
        OperationData::PadTrapez { layers, .. } => {
            (None, layers, PlotterOperationKind::FlashPadTrapez)
        }
        OperationData::StartBlock { .. } => (None, &[], PlotterOperationKind::StartBlock),
        OperationData::EndBlock => (None, &[], PlotterOperationKind::EndBlock),
    }
}

#[derive(Clone)]
struct EffectiveStyle {
    stroke: Option<SvgColor>,
    fill: Option<SvgColor>,
    width_nm: i64,
    line_style: Option<SvgLineStyle>,
    filled: bool,
    opacity: f64,
}

fn emit_inherited_style<'a>(
    sink: &mut SvgSink,
    source: &PrimitiveStyle<'a>,
    kind: PlotterOperationKind,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'a>,
) -> Result<(), SvgError> {
    let mut effective = source.clone();
    if effective.layer.is_none() {
        effective.layer = scope.layer;
    }
    if effective.layers.is_empty() {
        effective.layers = scope.layers;
    }
    emit_style(sink, &effective, kind, context, scope)
}

fn emit_style(
    sink: &mut SvgSink,
    source: &PrimitiveStyle<'_>,
    kind: PlotterOperationKind,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> Result<(), SvgError> {
    let mut style = EffectiveStyle {
        stroke: source.stroke.map(SvgColor::parse).transpose()?,
        fill: source.fill.map(SvgColor::parse).transpose()?,
        width_nm: source.width_nm,
        line_style: source.line_style.as_deref().and_then(source_line_style),
        filled: source.filled,
        opacity: 1.0,
    };
    apply_fallback(&mut style, context.fallback_style())?;
    style.stroke = style
        .stroke
        .as_ref()
        .map(|color| context.remap_color(color).clone());
    style.fill = style
        .fill
        .as_ref()
        .map(|color| context.remap_color(color).clone());
    let semantic = semantic_role(source, scope.semantic_role);
    for (pattern, value) in context.layer_styles() {
        if pattern.matches(source.layer)
            || pattern_matches_represented(pattern, source.layers, semantic, scope.copper_stack)
        {
            apply_override(&mut style, value)?;
        }
    }
    if let Some(value) = context.semantic_style(semantic) {
        apply_override(&mut style, value)?;
    }
    if let Some(value) = context.operation_style(kind) {
        apply_override(&mut style, value)?;
    }
    nonnegative(style.width_nm, "width_nm")?;
    if style.width_nm == 0 && style.filled {
        sink.attribute("stroke", "none")?;
    } else {
        let width = if style.width_nm == 0 {
            152_400
        } else {
            style.width_nm
        };
        let color = style.stroke.as_ref().map_or("#000000FF", SvgColor::as_str);
        color_attribute(sink, "stroke", color, Some(style.opacity))?;
        sink.attribute("stroke-width", &width.to_string())?;
        sink.attribute("stroke-linecap", "round")?;
        sink.attribute("stroke-linejoin", "round")?;
        emit_dash(sink, style.line_style, width)?;
    }
    if style.filled {
        let color = style
            .fill
            .as_ref()
            .or(style.stroke.as_ref())
            .map_or("#000000FF", SvgColor::as_str);
        color_attribute(sink, "fill", color, Some(style.opacity))?;
    } else {
        sink.attribute("fill", "none")?;
    }
    Ok(())
}

fn apply_override(style: &mut EffectiveStyle, value: &SvgStyleOverride) -> Result<(), SvgError> {
    if let Some(color) = value.stroke() {
        style.stroke = Some(color.clone());
    }
    if let Some(color) = value.fill() {
        style.fill = Some(color.clone());
    }
    if let Some(width) = value.stroke_width_nm() {
        style.width_nm = i64::try_from(width).map_err(|_| {
            direct_error(
                SvgErrorKind::InvalidContext,
                "SVG context stroke width exceeds i64",
            )
        })?;
    }
    if let Some(line_style) = value.line_style()
        && line_style != SvgLineStyle::Source
    {
        style.line_style = Some(line_style);
    }
    if let Some(fill_mode) = value.fill_mode() {
        match fill_mode {
            SvgFillMode::Source => {}
            SvgFillMode::None => style.filled = false,
            SvgFillMode::Solid => style.filled = true,
        }
    }
    if let Some(opacity) = value.opacity() {
        style.opacity *= opacity;
    }
    Ok(())
}

fn apply_fallback(style: &mut EffectiveStyle, value: &SvgStyleOverride) -> Result<(), SvgError> {
    if style.stroke.is_none() {
        style.stroke.clone_from(&value.stroke().cloned());
    }
    if style.fill.is_none() {
        style.fill.clone_from(&value.fill().cloned());
    }
    if style.width_nm == 0
        && let Some(width) = value.stroke_width_nm()
    {
        style.width_nm = i64::try_from(width).map_err(|_| {
            direct_error(
                SvgErrorKind::InvalidContext,
                "SVG context stroke width exceeds i64",
            )
        })?;
    }
    if style.line_style.is_none()
        && let Some(line_style) = value.line_style()
        && line_style != SvgLineStyle::Source
    {
        style.line_style = Some(line_style);
    }
    if let Some(fill_mode) = value.fill_mode() {
        match fill_mode {
            SvgFillMode::Source => {}
            SvgFillMode::None => style.filled = false,
            SvgFillMode::Solid => style.filled = true,
        }
    }
    if let Some(opacity) = value.opacity() {
        style.opacity *= opacity;
    }
    Ok(())
}

fn semantic_role(
    style: &PrimitiveStyle<'_>,
    inherited_role: Option<SvgSemanticRole>,
) -> SvgSemanticRole {
    if style
        .role
        .as_deref()
        .is_some_and(|role| role.contains("mask"))
    {
        SvgSemanticRole::Mask
    } else if style
        .role
        .as_deref()
        .is_some_and(|role| role.contains("drill") || role.contains("hole"))
    {
        SvgSemanticRole::Drill
    } else if style.layer.is_some_and(|layer| layer.ends_with(".Cu"))
        || style.layers.iter().any(|layer| layer.ends_with(".Cu"))
    {
        SvgSemanticRole::Copper
    } else if style.layer.is_some_and(|layer| layer.ends_with(".Mask"))
        || style.layers.iter().any(|layer| layer.ends_with(".Mask"))
    {
        SvgSemanticRole::Mask
    } else if style.layer.is_some_and(|layer| layer.ends_with(".SilkS"))
        || style.layers.iter().any(|layer| layer.ends_with(".SilkS"))
    {
        SvgSemanticRole::Silkscreen
    } else if style.layer.is_some_and(|layer| layer.ends_with(".Fab")) {
        SvgSemanticRole::Fabrication
    } else if style.layer.is_some_and(|layer| layer.contains(".CrtYd")) {
        SvgSemanticRole::Courtyard
    } else if style.layer == Some("Edge.Cuts") {
        SvgSemanticRole::BoardEdge
    } else {
        inherited_role.unwrap_or(SvgSemanticRole::Other)
    }
}

fn record_semantic_role(source_kind: &str, kind: &str) -> SvgSemanticRole {
    match (source_kind, kind) {
        ("SYM", _) => SvgSemanticRole::SymbolBody,
        ("SCH", "sheet_header") => SvgSemanticRole::Worksheet,
        ("SCH", "wire") => SvgSemanticRole::SchematicWire,
        ("SCH", "bus" | "bus_entry") => SvgSemanticRole::SchematicBus,
        ("SCH", "junction") => SvgSemanticRole::Junction,
        (
            "SCH",
            "label" | "global_label" | "hierarchical_label" | "netclass_flag" | "no_connect",
        ) => SvgSemanticRole::Label,
        ("SCH", "symbol_instance" | "symbol_overplot") => SvgSemanticRole::SymbolBody,
        ("SCH", "sheet") => SvgSemanticRole::HierarchicalSheet,
        ("SCH", "image") => SvgSemanticRole::Image,
        ("SCH", "text" | "text_box") | ("PCB", "board_text" | "board_text_box") => {
            SvgSemanticRole::Text
        }
        _ => SvgSemanticRole::Other,
    }
}

fn emit_pad_style(
    sink: &mut SvgSink,
    kind: PlotterOperationKind,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> Result<(), SvgError> {
    if !context.has_style_overrides() {
        sink.attribute("fill", "#000000")?;
        return sink.attribute("stroke", "none");
    }
    let source = PrimitiveStyle {
        layer: scope.layer,
        role: None,
        layers: scope.layers,
        stroke: None,
        fill: Some("#000000FF"),
        width_nm: 0,
        line_style: None,
        filled: true,
    };
    emit_style(sink, &source, kind, context, scope)
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "semantic text, both retained cache forms, and browser text share one emitter"
)]
fn render_text(
    text: &TextData<'_>,
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> Result<(), SvgError> {
    let (color, opacity) = resolved_text_style(
        text.color,
        text.layer.or(scope.layer),
        scope
            .semantic_role
            .filter(|role| *role == SvgSemanticRole::Pin)
            .unwrap_or(SvgSemanticRole::Text),
        context,
    )?;
    if !text.cache.is_empty() {
        for polygon in &text.cache {
            sink.element()?;
            sink.raw("<path d=\"")?;
            let mut contours = 0usize;
            for contour in polygon {
                if contour.len() >= 3 {
                    write_path(sink, contour)?;
                    contours += 1;
                }
            }
            sink.raw("\"")?;
            color_attribute(sink, "fill", color.as_str(), Some(opacity))?;
            sink.attribute("stroke", "none")?;
            if contours > 1 {
                sink.attribute("fill-rule", "evenodd")?;
            }
            sink.raw("/>\n")?;
        }
        return Ok(());
    }
    if !text.legacy_cache.is_empty() {
        for polygon in &text.legacy_cache {
            if polygon.len() < 3 {
                continue;
            }
            sink.element()?;
            sink.raw("<polygon points=\"")?;
            write_points(sink, polygon)?;
            sink.raw("\"")?;
            color_attribute(sink, "fill", color.as_str(), Some(opacity))?;
            sink.attribute("stroke", "none")?;
            sink.raw("/>\n")?;
        }
        return Ok(());
    }
    let lines = if text.multiline {
        text.text.split('\n').collect::<Vec<_>>()
    } else {
        vec![text.text]
    };
    let (first, step) = if text.multiline && text.text.contains('\n') {
        direct_multiline_positions(text, lines.len())?
    } else {
        ((text.x, text.y), (0, 0))
    };
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() && text.multiline && text.text.contains('\n') {
            continue;
        }
        let index =
            i64::try_from(index).map_err(|_| overflow_error("text line index exceeds i64"))?;
        let x = checked_line_coordinate(first.0, step.0, index)?;
        let y = checked_line_coordinate(first.1, step.1, index)?;
        sink.element()?;
        sink.raw("<text")?;
        sink.attribute("x", &x.to_string())?;
        sink.attribute("y", &y.to_string())?;
        sink.attribute("font-size", &text.size_y_nm.to_string())?;
        let face = context.font_face_override().unwrap_or(text.font_face);
        if !face.is_empty() {
            sink.attribute("font-family", face)?;
        }
        if text.bold {
            sink.attribute("font-weight", "bold")?;
        }
        if text.italic {
            sink.attribute("font-style", "italic")?;
        }
        sink.attribute(
            "text-anchor",
            if text.h_align.ends_with("CENTER") {
                "middle"
            } else if text.h_align.ends_with("RIGHT") {
                "end"
            } else {
                "start"
            },
        )?;
        sink.attribute(
            "dominant-baseline",
            if text.v_align.ends_with("TOP") {
                "hanging"
            } else if text.v_align.ends_with("CENTER") {
                "central"
            } else {
                "alphabetic"
            },
        )?;
        color_attribute(sink, "fill", color.as_str(), Some(opacity))?;
        if text.orient_deg != 0.0 {
            sink.attribute(
                "transform",
                &format!("rotate({} {x} {y})", number(-text.orient_deg)),
            )?;
        }
        sink.raw(">")?;
        sink.escaped(line)?;
        sink.raw("</text>\n")?;
    }
    Ok(())
}

fn direct_multiline_positions(
    text: &TextData<'_>,
    line_count: usize,
) -> Result<(Point, Point), SvgError> {
    let size_iu = rounded_i64((text.size_y_nm as f64) / 100.0, false)?;
    let line_step_iu = rounded_i64((size_iu as f64) * 1.68, false)?;
    let line_step = line_step_iu
        .checked_mul(100)
        .ok_or_else(|| overflow_error("text line step overflowed"))?;
    let gaps = i64::try_from(line_count.saturating_sub(1))
        .map_err(|_| overflow_error("text line count exceeds i64"))?;
    let total_step = gaps
        .checked_mul(line_step)
        .ok_or_else(|| overflow_error("text block height overflowed"))?;
    let vertical_offset = if text.v_align.ends_with("CENTER") {
        gaps.checked_mul(line_step_iu)
            .and_then(|value| (value / 2).checked_mul(-100))
            .ok_or_else(|| overflow_error("text centered offset overflowed"))?
    } else if text.v_align.ends_with("BOTTOM") {
        total_step
            .checked_neg()
            .ok_or_else(|| overflow_error("text bottom offset overflowed"))?
    } else {
        0
    };
    let first_offset = rotate_offset(0.0, vertical_offset as f64, -text.orient_deg);
    let step_offset = rotate_offset(0.0, line_step as f64, -text.orient_deg);
    Ok((
        (
            rounded_i64((text.x as f64) + first_offset.0, true)?,
            rounded_i64((text.y as f64) + first_offset.1, true)?,
        ),
        (
            rounded_i64(step_offset.0, true)?,
            rounded_i64(step_offset.1, true)?,
        ),
    ))
}

fn checked_line_coordinate(first: i64, step: i64, index: i64) -> Result<i64, SvgError> {
    first
        .checked_add(
            step.checked_mul(index)
                .ok_or_else(|| overflow_error("text line offset overflowed"))?,
        )
        .ok_or_else(|| overflow_error("text line position overflowed"))
}

fn rotate_offset(x: f64, y: f64, angle_degrees: f64) -> (f64, f64) {
    match angle_degrees.rem_euclid(360.0) {
        0.0 => (x, y),
        90.0 => (-y, x),
        180.0 => (-x, -y),
        270.0 => (y, -x),
        angle => {
            let (sine, cosine) = angle.to_radians().sin_cos();
            (x * cosine - y * sine, x * sine + y * cosine)
        }
    }
}

fn rounded_i64(value: f64, ties_even: bool) -> Result<i64, SvgError> {
    let value = if ties_even {
        value.round_ties_even()
    } else {
        value.round()
    };
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(overflow_error("text rounding exceeds i64"));
    }
    Ok(value as i64)
}

fn resolved_text_style(
    source: &str,
    layer: Option<&str>,
    semantic: SvgSemanticRole,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(SvgColor, f64), SvgError> {
    let mut color = SvgColor::parse(source)?;
    color = context.remap_color(&color).clone();
    let mut style = EffectiveStyle {
        stroke: None,
        fill: Some(color),
        width_nm: 0,
        line_style: None,
        filled: true,
        opacity: 1.0,
    };
    apply_fallback(&mut style, context.fallback_style())?;
    if let Some(layer) = layer {
        for (pattern, value) in context.layer_styles() {
            if pattern.matches(Some(layer)) {
                apply_override(&mut style, value)?;
            }
        }
    }
    if let Some(value) = context.semantic_style(semantic) {
        apply_override(&mut style, value)?;
    }
    if let Some(value) = context.operation_style(PlotterOperationKind::Text) {
        apply_override(&mut style, value)?;
    }
    Ok((
        style
            .fill
            .map_or_else(|| SvgColor::parse("#000000FF"), Ok)?,
        style.opacity,
    ))
}

fn render_image(
    center: Point,
    width: i64,
    height: i64,
    format: &str,
    data: &str,
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
) -> Result<(), SvgError> {
    nonnegative(width, "width_nm")?;
    nonnegative(height, "height_nm")?;
    sink.element()?;
    sink.raw("<image")?;
    sink.attribute("x", &centered_start(center.0, width))?;
    sink.attribute("y", &centered_start(center.1, height))?;
    sink.attribute("width", &width.to_string())?;
    sink.attribute("height", &height.to_string())?;
    sink.attribute("preserveAspectRatio", "none")?;
    let mut style = EffectiveStyle {
        stroke: None,
        fill: None,
        width_nm: 0,
        line_style: None,
        filled: false,
        opacity: 1.0,
    };
    apply_fallback(&mut style, context.fallback_style())?;
    if let Some(value) = context.semantic_style(SvgSemanticRole::Image) {
        apply_override(&mut style, value)?;
    }
    if let Some(value) = context.operation_style(PlotterOperationKind::PlotImage) {
        apply_override(&mut style, value)?;
    }
    if style.opacity < 1.0 {
        sink.attribute("opacity", &number(style.opacity))?;
    }
    let mime = match format {
        "jpeg" | "jpg" => "image/jpeg",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    sink.raw(" href=\"data:")?;
    sink.raw(mime)?;
    sink.raw(";base64,")?;
    sink.escaped(data)?;
    sink.raw("\"/>\n")
}

fn render_pad_oval(
    center: Point,
    size: Point,
    angle: f64,
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> Result<(), SvgError> {
    nonnegative(size.0, "size_x_nm")?;
    nonnegative(size.1, "size_y_nm")?;
    if size.0 == size.1 {
        sink.element()?;
        sink.raw("<circle")?;
        sink.attribute("cx", &center.0.to_string())?;
        sink.attribute("cy", &center.1.to_string())?;
        sink.attribute("r", &half_number(i128::from(size.0)))?;
        emit_pad_style(sink, PlotterOperationKind::FlashPadOval, context, scope)?;
        return sink.raw("/>\n");
    }
    let (first, second, width) = if size.0 > size.1 {
        let half = i128::from(size.0 - size.1);
        (
            (
                half_number(i128::from(center.0) * 2 - half),
                center.1.to_string(),
            ),
            (
                half_number(i128::from(center.0) * 2 + half),
                center.1.to_string(),
            ),
            size.1,
        )
    } else {
        let half = i128::from(size.1 - size.0);
        (
            (
                center.0.to_string(),
                half_number(i128::from(center.1) * 2 - half),
            ),
            (
                center.0.to_string(),
                half_number(i128::from(center.1) * 2 + half),
            ),
            size.0,
        )
    };
    sink.element()?;
    sink.raw("<line")?;
    for (name, value) in [
        ("x1", first.0),
        ("y1", first.1),
        ("x2", second.0),
        ("y2", second.1),
    ] {
        sink.attribute(name, &value)?;
    }
    rotation(sink, center, angle)?;
    if !context.has_style_overrides() {
        sink.attribute("fill", "none")?;
        sink.attribute("stroke", "#000000")?;
        sink.attribute("stroke-width", &width.to_string())?;
        sink.attribute("stroke-linecap", "round")?;
        sink.attribute("stroke-linejoin", "round")?;
        return sink.raw("/>\n");
    }
    let source = PrimitiveStyle {
        layer: scope.layer,
        role: None,
        layers: scope.layers,
        stroke: Some("#000000FF"),
        fill: None,
        width_nm: width,
        line_style: None,
        filled: false,
    };
    emit_style(
        sink,
        &source,
        PlotterOperationKind::FlashPadOval,
        context,
        scope,
    )?;
    sink.raw("/>\n")
}

fn render_pad_rect(
    center: Point,
    size: Point,
    angle: f64,
    radius: Option<i64>,
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    scope: RenderScope<'_>,
) -> Result<(), SvgError> {
    nonnegative(size.0, "size_x_nm")?;
    nonnegative(size.1, "size_y_nm")?;
    if let Some(radius) = radius {
        nonnegative(radius, "corner_radius_nm")?;
    }
    sink.element()?;
    sink.raw("<rect")?;
    sink.attribute("x", &centered_start(center.0, size.0))?;
    sink.attribute("y", &centered_start(center.1, size.1))?;
    sink.attribute("width", &size.0.to_string())?;
    sink.attribute("height", &size.1.to_string())?;
    if let Some(radius) = radius {
        sink.attribute("rx", &radius.to_string())?;
        sink.attribute("ry", &radius.to_string())?;
    }
    rotation(sink, center, angle)?;
    emit_pad_style(
        sink,
        if radius.is_some() {
            PlotterOperationKind::FlashPadRoundRect
        } else {
            PlotterOperationKind::FlashPadRect
        },
        context,
        scope,
    )?;
    sink.raw("/>\n")
}

fn render_local_polygon(
    center: Point,
    angle: f64,
    points: &[Point],
    sink: &mut SvgSink,
    context: &ValidatedSvgRenderContextA1,
    kind: PlotterOperationKind,
    scope: RenderScope<'_>,
) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<polygon points=\"")?;
    write_points(sink, points)?;
    sink.raw("\"")?;
    sink.attribute(
        "transform",
        &format!(
            "translate({} {}) rotate({})",
            center.0,
            center.1,
            number(-angle),
        ),
    )?;
    emit_pad_style(sink, kind, context, scope)?;
    sink.raw("/>\n")
}

fn point_attributes(sink: &mut SvgSink, start: Point, end: Point) -> Result<(), SvgError> {
    for (name, value) in [
        ("x1", start.0),
        ("y1", start.1),
        ("x2", end.0),
        ("y2", end.1),
    ] {
        sink.attribute(name, &value.to_string())?;
    }
    Ok(())
}

fn write_points(sink: &mut SvgSink, points: &[Point]) -> Result<(), SvgError> {
    for (index, (x, y)) in points.iter().enumerate() {
        if index > 0 {
            sink.raw(" ")?;
        }
        sink.raw(&format!("{x},{y}"))?;
    }
    Ok(())
}

fn write_path(sink: &mut SvgSink, points: &[Point]) -> Result<(), SvgError> {
    for (index, (x, y)) in points.iter().enumerate() {
        sink.raw(if index == 0 { "M " } else { " L " })?;
        sink.raw(&format!("{x} {y}"))?;
    }
    sink.raw(" Z ")
}

fn rotation(sink: &mut SvgSink, center: Point, angle: f64) -> Result<(), SvgError> {
    if angle != 0.0 {
        sink.attribute(
            "transform",
            &format!("rotate({} {} {})", number(-angle), center.0, center.1,),
        )?;
    }
    Ok(())
}

fn color_attribute(
    sink: &mut SvgSink,
    name: &str,
    rgba: &str,
    opacity: Option<f64>,
) -> Result<(), SvgError> {
    let color = SvgColor::parse(rgba)?;
    sink.attribute(name, &color.as_str()[..7])?;
    let source_alpha = if color.as_str().len() == 9 {
        u8::from_str_radix(&color.as_str()[7..], 16)
            .map_err(|_| direct_error(SvgErrorKind::Serialization, "invalid SVG alpha"))?
    } else {
        u8::MAX
    };
    let effective = f64::from(source_alpha) / 255.0 * opacity.unwrap_or(1.0);
    if effective < 1.0 {
        sink.attribute(&format!("{name}-opacity"), &number(effective))?;
    }
    Ok(())
}

fn emit_dash(sink: &mut SvgSink, style: Option<SvgLineStyle>, width: i64) -> Result<(), SvgError> {
    let scaled = |factor: i64| {
        width
            .checked_mul(factor)
            .ok_or_else(|| overflow_error("SVG dash length overflowed"))
    };
    let pattern = match style {
        Some(SvgLineStyle::Dash) => Some(format!("{} {}", scaled(4)?, scaled(2)?)),
        Some(SvgLineStyle::Dot) => Some(format!("{} {}", width, scaled(2)?)),
        Some(SvgLineStyle::DashDot) => Some(format!(
            "{} {} {} {}",
            scaled(4)?,
            scaled(2)?,
            width,
            scaled(2)?
        )),
        Some(SvgLineStyle::DashDotDot) => Some(format!(
            "{} {} {} {} {} {}",
            scaled(4)?,
            scaled(2)?,
            width,
            scaled(2)?,
            width,
            scaled(2)?
        )),
        _ => None,
    };
    if let Some(pattern) = pattern {
        sink.attribute("stroke-dasharray", &pattern)?;
    }
    Ok(())
}

fn source_line_style(value: &str) -> Option<SvgLineStyle> {
    match value {
        "DASH" => Some(SvgLineStyle::Dash),
        "DOT" => Some(SvgLineStyle::Dot),
        "DASH_DOT" => Some(SvgLineStyle::DashDot),
        "DASH_DOT_DOT" => Some(SvgLineStyle::DashDotDot),
        "SOLID" => Some(SvgLineStyle::Solid),
        _ => None,
    }
}

fn arc_parameters(start: Point, mid: Point, end: Point) -> Option<(f64, bool, bool)> {
    let (ax, ay) = (start.0 as f64, start.1 as f64);
    let (bx, by) = (mid.0 as f64, mid.1 as f64);
    let (cx, cy) = (end.0 as f64, end.1 as f64);
    let determinant = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if determinant.abs() < f64::EPSILON {
        return None;
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / determinant;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / determinant;
    let radius = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    let start_angle = (ay - uy).atan2(ax - ux);
    let mid_angle = (by - uy).atan2(bx - ux);
    let end_angle = (cy - uy).atan2(cx - ux);
    let ccw_total = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let ccw_mid = (mid_angle - start_angle).rem_euclid(std::f64::consts::TAU);
    let sweep = ccw_mid <= ccw_total;
    let span = if sweep {
        ccw_total
    } else {
        (start_angle - end_angle).rem_euclid(std::f64::consts::TAU)
    };
    Some((radius, span > std::f64::consts::PI, sweep))
}

fn centered_start(center: i64, size: i64) -> String {
    half_number(i128::from(center) * 2 - i128::from(size))
}

fn half_number(numerator: i128) -> String {
    let whole = numerator / 2;
    if numerator % 2 == 0 {
        whole.to_string()
    } else if numerator == -1 {
        "-0.5".to_owned()
    } else {
        format!("{whole}.5")
    }
}

fn number(value: f64) -> String {
    let value = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    format!("{value:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn format_mm(value_nm: u64) -> String {
    let whole = value_nm / 1_000_000;
    let fraction = value_nm % 1_000_000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:06}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn nonnegative(value: i64, label: &str) -> Result<(), SvgError> {
    if value < 0 {
        Err(SvgError::new(
            SvgErrorKind::InvalidDocument,
            format!("field {label} must be nonnegative"),
        ))
    } else {
        Ok(())
    }
}

fn direct_error(kind: SvgErrorKind, message: impl Into<String>) -> SvgError {
    SvgError::new(kind, message)
}

fn overflow_error(message: impl Into<String>) -> SvgError {
    direct_error(SvgErrorKind::ArithmeticOverflow, message)
}

fn block_error(message: impl Into<String>) -> SvgError {
    direct_error(SvgErrorKind::UnbalancedBlock, message)
}

fn checked_add(left: usize, right: usize, label: &str) -> Result<usize, SvgError> {
    left.checked_add(right).ok_or_else(|| {
        SvgError::new(
            SvgErrorKind::ArithmeticOverflow,
            format!("{label} overflowed"),
        )
    })
}

fn ensure(actual: usize, maximum: usize, label: &str) -> Result<(), SvgError> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(SvgError::new(
            SvgErrorKind::ResourceLimit,
            format!("{label} exceeds the configured limit"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SvgContextLimits, SvgRenderContextA1, SvgVisibility};

    fn filled_style() -> PrimitiveStyle<'static> {
        PrimitiveStyle {
            layer: None,
            role: None,
            layers: &[],
            stroke: Some("#000000FF"),
            fill: Some("#000000FF"),
            width_nm: 0,
            line_style: None,
            filled: true,
        }
    }

    #[test]
    fn bounds_follow_nested_pin_visibility_and_exact_transformed_arc_sweep() {
        let context = SvgRenderContextA1::builder()
            .visibility(SvgVisibility::new(false, true))
            .build()
            .validate(SvgContextLimits::default())
            .unwrap();
        let operations = vec![
            OperationData::StartBlock {
                label: "outer",
                data_uuid: "",
                data_ref: "symbol".to_owned(),
                object_id: "",
                extra_attrs: Vec::new(),
            },
            OperationData::StartBlock {
                label: "pin",
                data_uuid: "",
                data_ref: "pin".to_owned(),
                object_id: "",
                extra_attrs: vec![("pin".to_owned(), "1".to_owned())],
            },
            OperationData::Text(TextData {
                x: 0,
                y: 0,
                text: "IN",
                color: "#000000FF",
                orient_deg: 0.0,
                size_y_nm: 1,
                h_align: "LEFT".to_owned(),
                v_align: "TOP".to_owned(),
                italic: false,
                bold: false,
                multiline: false,
                font_face: "",
                layer: None,
                cache: vec![vec![vec![(1_000, 1_000), (2_000, 1_000), (2_000, 2_000)]]],
                legacy_cache: Vec::new(),
            }),
            OperationData::Segment {
                start: (0, 0),
                end: (10, 0),
                style: filled_style(),
            },
            OperationData::EndBlock,
            OperationData::EndBlock,
        ];
        let mut bounds = BoundsAccumulator::default();
        add_operation_sequence_bounds(
            &operations,
            RenderScope::default(),
            &context,
            BoundsTransform::default(),
            &mut bounds,
        )
        .unwrap();
        assert_eq!(
            bounds.finish().unwrap(),
            Some(SvgBounds {
                min_x_nm: 0,
                min_y_nm: 0,
                max_x_nm: 10,
                max_y_nm: 0,
            })
        );

        let arc = OperationData::Arc {
            start: (10, 0),
            mid: (0, 10),
            end: (-10, 0),
            style: filled_style(),
        };
        let mut arc_bounds = BoundsAccumulator::default();
        add_operation_bounds(
            &arc,
            RenderScope::default(),
            &context,
            BoundsTransform {
                translate_x: 0.0,
                translate_y: 0.0,
                angle_deg: 90.0,
            },
            &mut arc_bounds,
        )
        .unwrap();
        assert_eq!(
            arc_bounds.finish().unwrap(),
            Some(SvgBounds {
                min_x_nm: -10,
                min_y_nm: -10,
                max_x_nm: 0,
                max_y_nm: 10,
            })
        );
    }
}
