//! Source-selected library-symbol geometry and text plotter producer.

use crate::plotter_ir::{
    ArcThreePoint, BezierCurve, PlotterCircle, PlotterFill, PlotterLineStyle, PlotterOperation,
    PlotterPoly, PlotterRect,
};
use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Limits, Position, Sexp, TokenKind, decode_quoted,
    parse_with_limits,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::symbol_pin::pin_operations;
use crate::symbol_text::{
    SymbolTextBudget, SymbolTextSettings, SymbolTextVariables, body_text_operation,
    pin_text_operations,
};
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_BODY_WIDTH_NM: i64 = 152_400;
const DEFAULT_POLYLINE_WIDTH_NM: i64 = 152_400;
const MIN_PLOT_WIDTH_NM: i64 = 84_700;
const DEVICE_COLOR: &str = "#840000FF";
const DEVICE_BACKGROUND_COLOR: &str = "#FFFFC2FF";
const JS_SAFE_MAX: f64 = 9_007_199_254_740_991.0;

/// Resource ceilings for one selected library-symbol conversion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolPlotLimits {
    pub max_source_bytes: usize,
    pub max_depth: usize,
    pub max_symbols: usize,
    pub max_subsymbols: usize,
    pub max_operations: usize,
    pub max_points: usize,
    pub max_text_carriers: usize,
    pub max_text_bytes: usize,
}

impl Default for SymbolPlotLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_symbols: 100_000,
            max_subsymbols: 1_024,
            max_operations: 100_000,
            max_points: 1_000_000,
            max_text_carriers: 100_000,
            max_text_bytes: 16 * 1024 * 1024,
        }
    }
}

/// One selected unit/style body from a library symbol.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolPlotRecord {
    pub name: String,
    pub unit: u32,
    pub style: u32,
    pub operations: Vec<PlotterOperation>,
}

/// Typed facts needed to serialize the symbol geometry and text plotter subset.
#[derive(Clone, Debug, PartialEq)]
pub struct SymbolPlotDocument {
    pub name: String,
    pub extends: Option<String>,
    pub unit: Option<u32>,
    pub style: u32,
    pub in_bom: bool,
    pub on_board: bool,
    pub power: bool,
    pub records: Vec<SymbolPlotRecord>,
}

#[derive(Clone, Copy, Debug)]
enum SymbolSpanLookup {
    Unique(usize),
    Duplicate(Position),
}

/// Select and convert one symbol without materializing the complete library.
pub fn symbol_plot_document(
    source: &str,
    symbol_name: &str,
    unit: Option<u32>,
    style: u32,
    limits: SymbolPlotLimits,
) -> Result<SymbolPlotDocument, Error> {
    symbol_plot_document_with_text_variables(
        source,
        symbol_name,
        unit,
        style,
        limits,
        &SymbolTextVariables::default(),
    )
}

/// Select and convert one symbol with bounded caller-supplied project text variables.
pub fn symbol_plot_document_with_text_variables(
    source: &str,
    symbol_name: &str,
    unit: Option<u32>,
    style: u32,
    limits: SymbolPlotLimits,
    text_variables: &SymbolTextVariables,
) -> Result<SymbolPlotDocument, Error> {
    validate_limits(source, symbol_name, limits)?;
    validate_library_root(source, limits)?;
    let spans = select_symbol_spans(source, limits)?;
    let index = index_symbol_spans(source, &spans)?;
    let span = find_symbol_span(&spans, &index, symbol_name)?
        .ok_or_else(|| model_error("Requested library symbol was not found", Position::START))?;
    let form = parse_selected_symbol(source, span, limits)?;
    let inherited = inherited_geometry(source, &spans, &index, &form, symbol_name, limits)?;
    let geometry = inherited.as_ref().unwrap_or(&form);
    build_document(
        &form,
        geometry,
        limits,
        SymbolBuildContext {
            symbol_name,
            unit,
            style,
            position: span.start,
            text_variables,
        },
    )
}

fn validate_limits(source: &str, symbol_name: &str, limits: SymbolPlotLimits) -> Result<(), Error> {
    if source.len() > limits.max_source_bytes
        || limits.max_symbols == 0
        || limits.max_subsymbols == 0
    {
        return Err(limit_error(
            "Symbol plot resource limit is invalid or exceeded",
        ));
    }
    if symbol_name.is_empty() {
        return Err(model_error(
            "symbol_name must not be empty",
            Position::START,
        ));
    }
    Ok(())
}

fn validate_library_root(source: &str, limits: SymbolPlotLimits) -> Result<(), Error> {
    let roots = scan_form_spans_with_limits(
        source,
        &Selector {
            min_depth: Some(0),
            max_depth: Some(0),
            ..Selector::default()
        },
        projection_limits(limits, 2),
    )?;
    if roots.len() != 1 || roots[0].head.as_deref() != Some("kicad_symbol_lib") {
        return Err(model_error(
            "Expected exactly one kicad_symbol_lib root",
            roots.first().map_or(Position::START, |root| root.start),
        ));
    }
    Ok(())
}

fn select_symbol_spans(source: &str, limits: SymbolPlotLimits) -> Result<Vec<FormSpan>, Error> {
    scan_form_spans_with_limits(
        source,
        &Selector {
            paths: Some(
                [vec!["kicad_symbol_lib".to_owned(), "symbol".to_owned()]]
                    .into_iter()
                    .collect(),
            ),
            min_depth: Some(1),
            max_depth: Some(1),
            ..Selector::default()
        },
        projection_limits(limits, limits.max_symbols),
    )
}

fn index_symbol_spans(
    source: &str,
    spans: &[FormSpan],
) -> Result<BTreeMap<String, SymbolSpanLookup>, Error> {
    let mut index = BTreeMap::new();
    for (span_index, span) in spans.iter().enumerate() {
        let name = selected_name(source, span)?;
        index
            .entry(name)
            .and_modify(|entry| *entry = SymbolSpanLookup::Duplicate(span.start))
            .or_insert(SymbolSpanLookup::Unique(span_index));
    }
    Ok(index)
}

fn find_symbol_span<'a>(
    spans: &'a [FormSpan],
    index: &BTreeMap<String, SymbolSpanLookup>,
    symbol_name: &str,
) -> Result<Option<&'a FormSpan>, Error> {
    match index.get(symbol_name) {
        Some(SymbolSpanLookup::Unique(span_index)) => Ok(spans.get(*span_index)),
        Some(SymbolSpanLookup::Duplicate(position)) => {
            Err(model_error("Duplicate library symbol name", *position))
        }
        None => Ok(None),
    }
}

fn inherited_geometry(
    source: &str,
    spans: &[FormSpan],
    index: &BTreeMap<String, SymbolSpanLookup>,
    target: &Sexp,
    target_name: &str,
    limits: SymbolPlotLimits,
) -> Result<Option<Sexp>, Error> {
    if children(target, "symbol").next().is_some() {
        return Ok(None);
    }
    let mut base_name = extends_name(target);
    let mut seen = BTreeSet::from([target_name.to_owned()]);
    while let Some(name) = base_name {
        if !seen.insert(name.clone()) {
            return Ok(None);
        }
        let Some(span) = find_symbol_span(spans, index, &name)? else {
            return Ok(None);
        };
        let form = parse_selected_symbol(source, span, limits)?;
        if children(&form, "symbol").next().is_some() {
            return Ok(Some(form));
        }
        base_name = extends_name(&form);
    }
    Ok(None)
}

fn extends_name(form: &Sexp) -> Option<String> {
    child(form, "extends")
        .and_then(|value| value_at(value, 1))
        .map(str::to_owned)
}

fn projection_limits(limits: SymbolPlotLimits, max_selected_forms: usize) -> ProjectionLimits {
    ProjectionLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_selected_forms,
        ..ProjectionLimits::default()
    }
}

fn selected_name(source: &str, span: &FormSpan) -> Result<String, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        let _left = lexer.next().transpose()?;
        let _head = lexer.next().transpose()?;
        let token = lexer
            .next()
            .transpose()?
            .ok_or_else(|| model_error("Library symbol name is missing", Position::START))?;
        match token.kind {
            TokenKind::QuotedString => Ok(decode_quoted(token.lexeme)),
            TokenKind::Atom => Ok(token.lexeme.to_owned()),
            _ => Err(model_error(
                "Library symbol name is invalid",
                token.position,
            )),
        }
    })()
    .map_err(|error| rebase_error(error, span))
}

fn parse_selected_symbol(
    source: &str,
    span: &FormSpan,
    limits: SymbolPlotLimits,
) -> Result<Sexp, Error> {
    let max_nodes = span.range.len().saturating_add(1);
    parse_with_limits(
        span.text(source)?,
        Limits {
            max_source_bytes: span.range.len(),
            max_depth: limits.max_depth,
            max_nodes,
            max_decoded_string_bytes: span.range.len(),
        },
    )
    .map_err(|error| rebase_error(error, span))
}

#[derive(Clone, Copy)]
struct SymbolBuildContext<'a> {
    symbol_name: &'a str,
    unit: Option<u32>,
    style: u32,
    position: Position,
    text_variables: &'a SymbolTextVariables,
}

fn build_document(
    header: &Sexp,
    geometry: &Sexp,
    limits: SymbolPlotLimits,
    context: SymbolBuildContext<'_>,
) -> Result<SymbolPlotDocument, Error> {
    let SymbolBuildContext {
        symbol_name,
        unit,
        style,
        position,
        text_variables,
    } = context;
    let forms = list(header).ok_or_else(|| model_error("Expected symbol list", position))?;
    let mut point_count = 0usize;
    let mut records = Vec::new();
    let mut subsymbol_count = 0usize;
    let mut operation_count = 0usize;
    let settings = SymbolTextSettings::from_header(geometry, position)?;
    let mut text_budget = SymbolTextBudget::new(limits);
    for subsymbol in children(geometry, "symbol") {
        subsymbol_count = subsymbol_count.saturating_add(1);
        if subsymbol_count > limits.max_subsymbols {
            return Err(limit_error("Symbol subsymbol limit exceeded"));
        }
        let name = value_at(subsymbol, 1)
            .ok_or_else(|| model_error("Subsymbol name is missing", position))?;
        let (sub_unit, sub_style) = subsymbol_identity(name);
        if !selected_subsymbol(sub_unit, sub_style, unit, style) {
            continue;
        }
        let remaining_operations = limits.max_operations.saturating_sub(operation_count);
        let operations = convert_subsymbol(
            subsymbol,
            SubsymbolContext {
                max_operations: remaining_operations,
                max_points: limits.max_points,
                position,
                settings,
                text_variables,
            },
            &mut point_count,
            &mut text_budget,
        )?;
        operation_count = operation_count.saturating_add(operations.len());
        records.push(SymbolPlotRecord {
            name: name.to_owned(),
            unit: sub_unit,
            style: sub_style,
            operations,
        });
    }
    let _ = forms;
    Ok(SymbolPlotDocument {
        name: symbol_name.to_owned(),
        extends: child(header, "extends")
            .and_then(|value| value_at(value, 1))
            .map(str::to_owned),
        unit,
        style,
        in_bom: yes_flag(header, "in_bom", true),
        on_board: yes_flag(header, "on_board", true),
        power: child(header, "power").is_some() || has_atom(header, "power"),
        records,
    })
}

pub(crate) fn selected_subsymbol(
    sub_unit: u32,
    sub_style: u32,
    unit: Option<u32>,
    style: u32,
) -> bool {
    let style_matches = sub_style == 0 || sub_style == style || (style == 0 && sub_style == 1);
    let unit_matches = unit.is_none_or(|requested| sub_unit == 0 || sub_unit == requested);
    style_matches && unit_matches
}

pub(crate) fn subsymbol_identity(name: &str) -> (u32, u32) {
    let mut suffix = name.rsplitn(3, '_');
    let style = suffix.next().and_then(|value| value.parse().ok());
    let unit = suffix.next().and_then(|value| value.parse().ok());
    match (unit, style) {
        (Some(unit), Some(style)) => (unit, style),
        _ => (1, 0),
    }
}

#[derive(Clone, Copy)]
struct SubsymbolContext<'a> {
    max_operations: usize,
    max_points: usize,
    position: Position,
    settings: SymbolTextSettings,
    text_variables: &'a SymbolTextVariables,
}

fn convert_subsymbol(
    form: &Sexp,
    context: SubsymbolContext<'_>,
    point_count: &mut usize,
    text_budget: &mut SymbolTextBudget,
) -> Result<Vec<PlotterOperation>, Error> {
    let SubsymbolContext {
        max_operations,
        max_points,
        position,
        settings,
        text_variables,
    } = context;
    let mut fills = Vec::new();
    let mut outlines = Vec::new();
    for head in ["rectangle", "circle", "arc", "polyline", "bezier"] {
        for shape in children(form, head) {
            let operation = convert_shape(shape, head, max_points, point_count, position)?;
            let Some(operation) = operation else { continue };
            let (fill, outline) = split_filled_outline(operation);
            push_operation(&mut fills, fill, max_operations)?;
            if let Some(outline) = outline {
                push_operation(&mut outlines, outline, max_operations)?;
            }
        }
    }
    for text in children(form, "text") {
        if let Some(operation) = body_text_operation(text, text_variables, text_budget, position)? {
            push_operation(&mut fills, operation, max_operations)?;
        }
    }
    for pin in children(form, "pin") {
        for operation in pin_operations(pin, max_points, point_count, position)? {
            push_operation(&mut fills, operation, max_operations)?;
        }
        for operation in pin_text_operations(pin, settings, None, text_budget, position)? {
            push_operation(&mut fills, operation, max_operations)?;
        }
    }
    if fills.len().saturating_add(outlines.len()) > max_operations {
        return Err(limit_error("Symbol operation limit exceeded"));
    }
    fills.extend(outlines);
    Ok(fills)
}

pub(crate) fn convert_shape(
    form: &Sexp,
    head: &str,
    max_points: usize,
    point_count: &mut usize,
    position: Position,
) -> Result<Option<PlotterOperation>, Error> {
    let stroke = stroke_spec(form, head == "polyline", position)?;
    let mut fill = fill_spec(form, position)?;
    if fill.kind == PlotterFill::FilledShape && fill.color.is_none() {
        fill.color = Some(stroke.color.clone());
    }
    let operation = match head {
        "rectangle" => Some(rectangle(form, stroke, fill, position)?),
        "circle" => Some(circle(form, stroke, fill, position)?),
        "arc" => Some(arc(form, stroke, fill, position)?),
        "polyline" => Some(polyline(
            form,
            stroke,
            fill,
            max_points,
            point_count,
            position,
        )?),
        "bezier" => bezier(form, stroke, fill, max_points, point_count, position)?,
        _ => None,
    };
    Ok(operation)
}

#[derive(Clone, Debug)]
struct StrokeSpec {
    width_nm: i64,
    color: String,
    style: PlotterLineStyle,
}

#[derive(Clone, Debug)]
struct FillSpec {
    kind: PlotterFill,
    color: Option<String>,
}

fn stroke_spec(form: &Sexp, polyline: bool, position: Position) -> Result<StrokeSpec, Error> {
    let stroke = child(form, "stroke");
    let width = stroke
        .and_then(|value| child(value, "width"))
        .map_or(Ok(0.0), |value| numeric_at(value, 1, position))?;
    let default_width = if polyline {
        DEFAULT_POLYLINE_WIDTH_NM
    } else {
        DEFAULT_BODY_WIDTH_NM
    };
    let width_nm = if width < 0.0 {
        0
    } else if width == 0.0 {
        default_width.max(MIN_PLOT_WIDTH_NM)
    } else {
        mm_to_nm(width)?.max(MIN_PLOT_WIDTH_NM)
    };
    let style = match stroke
        .and_then(|value| child(value, "type"))
        .and_then(|value| value_at(value, 1))
        .unwrap_or("default")
    {
        "default" => PlotterLineStyle::Default,
        "solid" => PlotterLineStyle::Solid,
        "dash" => PlotterLineStyle::Dash,
        "dot" => PlotterLineStyle::Dot,
        "dash_dot" => PlotterLineStyle::DashDot,
        "dash_dot_dot" => PlotterLineStyle::DashDotDot,
        _ => return Err(model_error("Unsupported symbol stroke type", position)),
    };
    Ok(StrokeSpec {
        width_nm,
        color: color(stroke.and_then(|value| child(value, "color")), position)?
            .unwrap_or_else(|| DEVICE_COLOR.to_owned()),
        style,
    })
}

fn fill_spec(form: &Sexp, position: Position) -> Result<FillSpec, Error> {
    let fill = child(form, "fill");
    let kind = match fill
        .and_then(|value| child(value, "type"))
        .and_then(|value| value_at(value, 1))
        .unwrap_or("none")
    {
        "outline" => PlotterFill::FilledShape,
        "background" => PlotterFill::FilledWithBackgroundBodyColor,
        "color" => PlotterFill::FilledWithColor,
        "hatch" => PlotterFill::Hatch,
        "reverse_hatch" => PlotterFill::ReverseHatch,
        "cross_hatch" => PlotterFill::CrossHatch,
        _ => PlotterFill::NoFill,
    };
    let explicit = color(fill.and_then(|value| child(value, "color")), position)?;
    let color = explicit.or_else(|| match kind {
        PlotterFill::FilledWithBackgroundBodyColor => Some(DEVICE_BACKGROUND_COLOR.to_owned()),
        PlotterFill::FilledWithColor => Some(DEVICE_COLOR.to_owned()),
        _ => None,
    });
    Ok(FillSpec { kind, color })
}

fn rectangle(
    form: &Sexp,
    stroke: StrokeSpec,
    fill: FillSpec,
    position: Position,
) -> Result<PlotterOperation, Error> {
    let start = required_child(form, "start", position)?;
    let end = required_child(form, "end", position)?;
    Ok(PlotterOperation::Rect(PlotterRect {
        x1: coordinate(start, 1, false, position)?,
        y1: coordinate(start, 2, true, position)?,
        x2: coordinate(end, 1, false, position)?,
        y2: coordinate(end, 2, true, position)?,
        fill: fill.kind,
        width_nm: stroke.width_nm,
        corner_radius_nm: 0,
        layer: None,
        stroke_color: Some(stroke.color),
        fill_color: fill.color,
        line_style: Some(stroke.style),
    }))
}

fn circle(
    form: &Sexp,
    stroke: StrokeSpec,
    fill: FillSpec,
    position: Position,
) -> Result<PlotterOperation, Error> {
    let center = required_child(form, "center", position)?;
    let radius = required_child(form, "radius", position)?;
    Ok(PlotterOperation::Circle(PlotterCircle {
        cx: coordinate(center, 1, false, position)?,
        cy: coordinate(center, 2, true, position)?,
        diameter_nm: mm_to_nm(numeric_at(radius, 1, position)? * 2.0)?,
        fill: fill.kind,
        width_nm: stroke.width_nm,
        layer: None,
        role: None,
        layers: Vec::new(),
        mask_margin_nm: None,
        pad_size_x_nm: None,
        pad_size_y_nm: None,
        stroke_color: Some(stroke.color),
        fill_color: fill.color,
        line_style: Some(stroke.style),
    }))
}

fn arc(
    form: &Sexp,
    stroke: StrokeSpec,
    fill: FillSpec,
    position: Position,
) -> Result<PlotterOperation, Error> {
    let start = required_child(form, "start", position)?;
    let mid = required_child(form, "mid", position)?;
    let end = required_child(form, "end", position)?;
    Ok(PlotterOperation::ArcThreePoint(ArcThreePoint {
        start_x: coordinate(start, 1, false, position)?,
        start_y: coordinate(start, 2, true, position)?,
        mid_x: coordinate(mid, 1, false, position)?,
        mid_y: coordinate(mid, 2, true, position)?,
        end_x: coordinate(end, 1, false, position)?,
        end_y: coordinate(end, 2, true, position)?,
        fill: fill.kind,
        width_nm: stroke.width_nm,
        layer: None,
        stroke_color: Some(stroke.color),
        fill_color: fill.color,
        line_style: Some(stroke.style),
    }))
}

fn polyline(
    form: &Sexp,
    stroke: StrokeSpec,
    fill: FillSpec,
    max_points: usize,
    point_count: &mut usize,
    position: Position,
) -> Result<PlotterOperation, Error> {
    let points = points(form, max_points, point_count, position)?;
    Ok(PlotterOperation::PlotPoly(PlotterPoly {
        points,
        fill: fill.kind,
        width_nm: stroke.width_nm,
        layer: None,
        stroke_color: Some(stroke.color),
        fill_color: fill.color,
        line_style: Some(stroke.style),
    }))
}

fn bezier(
    form: &Sexp,
    stroke: StrokeSpec,
    fill: FillSpec,
    max_points: usize,
    point_count: &mut usize,
    position: Position,
) -> Result<Option<PlotterOperation>, Error> {
    let points = points(form, max_points, point_count, position)?;
    if let [start, ctrl1, ctrl2, end] = points.as_slice() {
        return Ok(Some(PlotterOperation::BezierCurve(BezierCurve {
            start_x: start[0],
            start_y: start[1],
            ctrl1_x: ctrl1[0],
            ctrl1_y: ctrl1[1],
            ctrl2_x: ctrl2[0],
            ctrl2_y: ctrl2[1],
            end_x: end[0],
            end_y: end[1],
            width_nm: stroke.width_nm,
            tolerance_nm: 0,
            layer: None,
            stroke_color: Some(stroke.color),
            line_style: Some(stroke.style),
        })));
    }
    if points.len() < 2 {
        return Ok(None);
    }
    Ok(Some(PlotterOperation::PlotPoly(PlotterPoly {
        points,
        fill: fill.kind,
        width_nm: stroke.width_nm,
        layer: None,
        stroke_color: Some(stroke.color),
        fill_color: fill.color,
        line_style: Some(stroke.style),
    })))
}

fn points(
    form: &Sexp,
    max_points: usize,
    point_count: &mut usize,
    position: Position,
) -> Result<Vec<[i64; 2]>, Error> {
    let pts = required_child(form, "pts", position)?;
    let mut output = Vec::new();
    for point in children(pts, "xy") {
        *point_count = point_count.saturating_add(1);
        if *point_count > max_points {
            return Err(limit_error("Symbol geometry point limit exceeded"));
        }
        output.push([
            coordinate(point, 1, false, position)?,
            coordinate(point, 2, true, position)?,
        ]);
    }
    Ok(output)
}

pub(crate) fn split_filled_outline(
    mut operation: PlotterOperation,
) -> (PlotterOperation, Option<PlotterOperation>) {
    let fill = operation_fill(&operation);
    if matches!(
        fill,
        None | Some(PlotterFill::NoFill | PlotterFill::FilledShape)
    ) {
        return (operation, None);
    }
    let mut outline = operation.clone();
    set_fill_pass(&mut operation);
    set_outline_pass(&mut outline);
    (operation, Some(outline))
}

fn operation_fill(operation: &PlotterOperation) -> Option<PlotterFill> {
    match operation {
        PlotterOperation::ArcThreePoint(value) => Some(value.fill),
        PlotterOperation::Circle(value) => Some(value.fill),
        PlotterOperation::Rect(value) => Some(value.fill),
        PlotterOperation::PlotPoly(value) => Some(value.fill),
        _ => None,
    }
}

fn set_fill_pass(operation: &mut PlotterOperation) {
    macro_rules! apply {
        ($value:ident) => {{
            $value.width_nm = 0;
            if let Some(color) = $value
                .fill_color
                .clone()
                .or_else(|| $value.stroke_color.clone())
            {
                $value.stroke_color = Some(color.clone());
                $value.fill_color = Some(color);
            }
        }};
    }
    match operation {
        PlotterOperation::ArcThreePoint(value) => apply!(value),
        PlotterOperation::Circle(value) => apply!(value),
        PlotterOperation::Rect(value) => apply!(value),
        PlotterOperation::PlotPoly(value) => apply!(value),
        _ => {}
    }
}

fn set_outline_pass(operation: &mut PlotterOperation) {
    macro_rules! apply {
        ($value:ident) => {{
            $value.fill = PlotterFill::NoFill;
            $value.fill_color = None;
        }};
    }
    match operation {
        PlotterOperation::ArcThreePoint(value) => apply!(value),
        PlotterOperation::Circle(value) => apply!(value),
        PlotterOperation::Rect(value) => apply!(value),
        PlotterOperation::PlotPoly(value) => apply!(value),
        _ => {}
    }
}

fn push_operation(
    operations: &mut Vec<PlotterOperation>,
    operation: PlotterOperation,
    max_operations: usize,
) -> Result<(), Error> {
    if operations.len() >= max_operations {
        return Err(limit_error("Symbol operation limit exceeded"));
    }
    operations.push(operation);
    Ok(())
}

fn color(form: Option<&Sexp>, position: Position) -> Result<Option<String>, Error> {
    let Some(form) = form else { return Ok(None) };
    let r = color_channel(form, 1, position)?;
    let g = color_channel(form, 2, position)?;
    let b = color_channel(form, 3, position)?;
    let alpha = numeric_at(form, 4, position)?;
    if alpha <= 0.0 {
        return Ok(None);
    }
    let scaled = if alpha <= 1.0 { alpha * 255.0 } else { alpha };
    let a = scaled.clamp(0.0, 255.0).round_ties_even() as u8;
    Ok(Some(format!("#{r:02X}{g:02X}{b:02X}{a:02X}")))
}

fn color_channel(form: &Sexp, index: usize, position: Position) -> Result<u8, Error> {
    Ok(numeric_at(form, index, position)?.clamp(0.0, 255.0) as u8)
}

fn coordinate(form: &Sexp, index: usize, flip_y: bool, position: Position) -> Result<i64, Error> {
    let value = numeric_at(form, index, position)?;
    mm_to_nm(if flip_y { -value } else { value })
}

fn mm_to_nm(value: f64) -> Result<i64, Error> {
    let scaled = value * 1_000_000.0;
    if !scaled.is_finite() || !(-JS_SAFE_MAX..=JS_SAFE_MAX).contains(&scaled) {
        return Err(model_error(
            "Symbol coordinate exceeds JavaScript safe-integer range",
            Position::START,
        ));
    }
    Ok(scaled.round_ties_even() as i64)
}

fn yes_flag(form: &Sexp, head: &str, default: bool) -> bool {
    child(form, head)
        .and_then(|value| value_at(value, 1))
        .map_or(default, |value| value == "yes")
}

fn has_atom(form: &Sexp, expected: &str) -> bool {
    list(form).is_some_and(|values| values.iter().any(|value| text(value) == Some(expected)))
}

fn required_child<'a>(form: &'a Sexp, head: &str, position: Position) -> Result<&'a Sexp, Error> {
    child(form, head)
        .ok_or_else(|| model_error("Required symbol geometry form is missing", position))
}

fn children<'a>(form: &'a Sexp, head: &'a str) -> impl Iterator<Item = &'a Sexp> + 'a {
    list(form).into_iter().flatten().filter(move |candidate| {
        list(candidate)
            .and_then(|values| values.first())
            .and_then(text)
            == Some(head)
    })
}

fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
    list(form)?.iter().find(|candidate| {
        list(candidate)
            .and_then(|values| values.first())
            .and_then(text)
            == Some(head)
    })
}

fn list(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn value_at(form: &Sexp, index: usize) -> Option<&str> {
    list(form)?.get(index).and_then(text)
}

fn numeric_at(form: &Sexp, index: usize, position: Position) -> Result<f64, Error> {
    let value = list(form)
        .and_then(|values| values.get(index))
        .ok_or_else(|| model_error("Expected numeric symbol value", position))?;
    let number = match value {
        Sexp::Integer(value) => *value as f64,
        Sexp::Float(value) => *value,
        Sexp::Atom(value) => value
            .parse()
            .map_err(|_| model_error("Expected numeric symbol value", position))?,
        _ => return Err(model_error("Expected numeric symbol value", position)),
    };
    if !number.is_finite() {
        return Err(model_error("Symbol numeric value must be finite", position));
    }
    Ok(number)
}

fn rebase_error(mut error: Error, span: &FormSpan) -> Error {
    if let Some(position) = error.position {
        error.position = Some(Position {
            offset: span.start.offset.saturating_add(position.offset),
            line: span
                .start
                .line
                .saturating_add(position.line.saturating_sub(1)),
            column: if position.line == 1 {
                span.start
                    .column
                    .saturating_add(position.column.saturating_sub(1))
            } else {
                position.column
            },
        });
    }
    error
}

fn model_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}
