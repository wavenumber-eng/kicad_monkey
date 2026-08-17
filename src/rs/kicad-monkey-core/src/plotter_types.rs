//! Producer-neutral plotter operation vocabulary.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThickSegment {
    pub start_x: i64,
    pub start_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub width_nm: i64,
    pub layer: Option<String>,
    pub role: Option<String>,
    pub layers: Vec<String>,
    pub mask_margin_nm: Option<i64>,
    pub pad_size_x_nm: Option<i64>,
    pub pad_size_y_nm: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotterFill {
    NoFill,
    FilledShape,
    FilledWithBackgroundBodyColor,
    FilledWithColor,
    Hatch,
    ReverseHatch,
    CrossHatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotterLineStyle {
    Default,
    Solid,
    Dash,
    Dot,
    DashDot,
    DashDotDot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotterTextHAlign {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlotterTextVAlign {
    Top,
    Center,
    Bottom,
}

/// Cache-free shared Text payload used by standalone footprint and symbol
/// producers. Board carriers retain their richer authored/native cache type.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotterText {
    pub x: i64,
    pub y: i64,
    pub text: String,
    pub color: String,
    pub orient_deg: f64,
    pub size_x_nm: i64,
    pub size_y_nm: i64,
    pub h_align: PlotterTextHAlign,
    pub v_align: PlotterTextVAlign,
    pub pen_width_nm: i64,
    pub italic: bool,
    pub bold: bool,
    pub multiline: bool,
    pub font_face: String,
    pub layer: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcThreePoint {
    pub start_x: i64,
    pub start_y: i64,
    pub mid_x: i64,
    pub mid_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub layer: Option<String>,
    pub stroke_color: Option<String>,
    pub fill_color: Option<String>,
    pub line_style: Option<PlotterLineStyle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotterCircle {
    pub cx: i64,
    pub cy: i64,
    pub diameter_nm: i64,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub layer: Option<String>,
    pub role: Option<String>,
    pub layers: Vec<String>,
    pub mask_margin_nm: Option<i64>,
    pub pad_size_x_nm: Option<i64>,
    pub pad_size_y_nm: Option<i64>,
    pub stroke_color: Option<String>,
    pub fill_color: Option<String>,
    pub line_style: Option<PlotterLineStyle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotterRect {
    pub x1: i64,
    pub y1: i64,
    pub x2: i64,
    pub y2: i64,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub corner_radius_nm: i64,
    pub layer: Option<String>,
    pub stroke_color: Option<String>,
    pub fill_color: Option<String>,
    pub line_style: Option<PlotterLineStyle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlotterPoly {
    pub points: Vec<[i64; 2]>,
    pub fill: PlotterFill,
    pub width_nm: i64,
    pub layer: Option<String>,
    pub stroke_color: Option<String>,
    pub fill_color: Option<String>,
    pub line_style: Option<PlotterLineStyle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BezierCurve {
    pub start_x: i64,
    pub start_y: i64,
    pub ctrl1_x: i64,
    pub ctrl1_y: i64,
    pub ctrl2_x: i64,
    pub ctrl2_y: i64,
    pub end_x: i64,
    pub end_y: i64,
    pub width_nm: i64,
    pub tolerance_nm: i64,
    pub layer: Option<String>,
    pub stroke_color: Option<String>,
    pub line_style: Option<PlotterLineStyle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlashPadCircle {
    pub x: i64,
    pub y: i64,
    pub diameter_nm: i64,
    pub layers: Vec<String>,
    pub mask_margin_nm: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashPadOval {
    pub x: i64,
    pub y: i64,
    pub size_x_nm: i64,
    pub size_y_nm: i64,
    pub orient_deg: f64,
    pub layers: Vec<String>,
    pub mask_margin_nm: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashPadRect {
    pub x: i64,
    pub y: i64,
    pub size_x_nm: i64,
    pub size_y_nm: i64,
    pub orient_deg: f64,
    pub layers: Vec<String>,
    pub mask_margin_nm: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashPadRoundRect {
    pub x: i64,
    pub y: i64,
    pub size_x_nm: i64,
    pub size_y_nm: i64,
    pub corner_radius_nm: i64,
    pub orient_deg: f64,
    pub layers: Vec<String>,
    pub mask_margin_nm: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashPadCustom {
    pub x: i64,
    pub y: i64,
    pub size_x_nm: i64,
    pub size_y_nm: i64,
    pub orient_deg: f64,
    pub polygons: Vec<Vec<[i64; 2]>>,
    pub polygon_widths_nm: Option<Vec<i64>>,
    pub anchor_shape: Option<String>,
    pub layers: Vec<String>,
    pub mask_margin_nm: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlashPadTrapez {
    pub x: i64,
    pub y: i64,
    pub corners: [[i64; 2]; 4],
    pub orient_deg: f64,
    pub layers: Vec<String>,
    pub mask_margin_nm: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlotterOperation {
    ThickSegment(ThickSegment),
    ArcThreePoint(ArcThreePoint),
    Circle(PlotterCircle),
    Rect(PlotterRect),
    PlotPoly(PlotterPoly),
    BezierCurve(BezierCurve),
    Text(PlotterText),
    FlashPadCircle(FlashPadCircle),
    FlashPadOval(FlashPadOval),
    FlashPadRect(FlashPadRect),
    FlashPadRoundRect(FlashPadRoundRect),
    FlashPadCustom(FlashPadCustom),
    FlashPadTrapez(FlashPadTrapez),
}
