//! Board table grid/border and authored/native outline-cache emission.

use super::text::{
    BoardTextHAlign, BoardTextOperation, BoardTextVAlign, alignments, effective_line_spacing,
    ensure_retained_text_bytes, numeric_or, operation_text_bytes, text_effects, text_point_total,
};
use super::text_cache::{
    AuthoredRenderCache, attach_authored_cache, attach_native_cache, cache_is_valid_without_angle,
    parse_render_cache,
};
use super::text_native::{native_h_align, native_v_align};
use super::{
    BoardPlotLimits, BoardPlotRecord, BoardTableOperation, BoardTableRecord, BoardTextVariables,
    BudgetTracker, input_point_limit_error, layerless_segment, limit_error,
};
use crate::pcb::{PcbTable, PcbTableCell, PcbView};
use crate::plotter_ir::{child, mm_to_nm};
use crate::plotter_text_cache::{PlotterTextCacheSession, PlotterTextLayout};
use crate::plotter_types::PlotterOperation;
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Limits, Position, Sexp, parse_with_limits};
use std::collections::BTreeSet;

const DEFAULT_TABLE_STROKE_MM: f64 = 0.2;

#[derive(Clone, Copy)]
struct TableCellContext<'a> {
    variables: &'a BoardTextVariables,
    budget: &'a BudgetTracker,
    text_cache: Option<&'a PlotterTextCacheSession<'a>>,
    limits: BoardPlotLimits,
}

pub(super) fn decoded_tables(
    view: &PcbView<'_>,
    max_input_points: usize,
) -> Result<(Vec<PcbTable>, Vec<PcbTableCell>, usize), Error> {
    let tables = view.tables().collect::<Result<Vec<_>, _>>()?;
    let mut cells = Vec::new();
    let mut points = 0usize;
    for cell in view.table_cells() {
        points = points
            .checked_add(2)
            .filter(|count| *count <= max_input_points)
            .ok_or_else(input_point_limit_error)?;
        cells.push(cell?);
    }
    Ok((tables, cells, points))
}

pub(super) fn table_records(
    source: &str,
    tables: &[PcbTable],
    cells: &[PcbTableCell],
    variables: &BoardTextVariables,
    budget: &mut BudgetTracker,
    text_cache: Option<&PlotterTextCacheSession<'_>>,
    limits: BoardPlotLimits,
) -> Result<Vec<BoardPlotRecord>, Error> {
    let mut records = Vec::with_capacity(tables.len());
    let mut cell_start = 0usize;
    for (table_index, table) in tables.iter().enumerate() {
        let cell_end = cell_start
            .checked_add(table.cell_count)
            .filter(|end| *end <= cells.len())
            .ok_or_else(limit_error)?;
        let table_cells = &cells[cell_start..cell_end];
        if table_cells
            .iter()
            .any(|cell| cell.table_index != table_index)
        {
            return Err(model_error(
                "Table cell index does not match its parent table",
            ));
        }
        records.push(BoardPlotRecord::Table(table_record(
            source,
            table,
            table_cells,
            variables,
            budget,
            text_cache,
            limits,
        )?));
        cell_start = cell_end;
    }
    if cell_start != cells.len() {
        return Err(model_error("Orphaned table cell in selected PCB view"));
    }
    Ok(records)
}

fn table_record(
    source: &str,
    table: &PcbTable,
    cells: &[PcbTableCell],
    variables: &BoardTextVariables,
    budget: &mut BudgetTracker,
    text_cache: Option<&PlotterTextCacheSession<'_>>,
    limits: BoardPlotLimits,
) -> Result<BoardTableRecord, Error> {
    let form = parse_span(source, table.source_range.clone(), limits)?;
    let separator_width = nested_stroke_width(&form, "separators")?;
    let border_width = nested_stroke_width(&form, "border")?;
    let (xs, ys) = grid_coordinates(cells);
    let geometry_count = geometry_operation_count(table, cells, xs.len(), ys.len())?;
    budget.ensure_capacity(geometry_count, 0)?;

    let mut operations = Vec::with_capacity(geometry_count);
    append_geometry(
        &mut operations,
        table,
        cells,
        &xs,
        &ys,
        separator_width,
        border_width,
    )?;
    budget.charge(geometry_count, 0)?;

    for (cell_index, cell) in cells.iter().enumerate() {
        let context = TableCellContext {
            variables,
            budget,
            text_cache,
            limits,
        };
        let Some(operation) = table_cell_text_operation(source, table, cell, cell_index, context)?
        else {
            continue;
        };
        let points = text_point_total(std::slice::from_ref(&operation));
        budget.ensure_capacity(1, points)?;
        budget.charge(1, points)?;
        budget.charge_text(operation_text_bytes(&operation))?;
        operations.push(BoardTableOperation::Text(operation));
    }

    let layers = std::iter::once(table.layer.clone())
        .chain(cells.iter().map(|cell| cell.layer.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let cell_bounds_nm = cells
        .iter()
        .map(|cell| {
            Ok([
                mm_to_nm(cell.start.x.min(cell.end.x))?,
                mm_to_nm(cell.start.y.min(cell.end.y))?,
                mm_to_nm(cell.start.x.max(cell.end.x))?,
                mm_to_nm(cell.start.y.max(cell.end.y))?,
            ])
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(BoardTableRecord {
        uuid: table.uuid.clone().unwrap_or_default(),
        layers,
        cell_count: cells.len(),
        cell_bounds_nm,
        operations,
    })
}

fn grid_coordinates(cells: &[PcbTableCell]) -> (Vec<f64>, Vec<f64>) {
    let mut xs = Vec::with_capacity(cells.len().saturating_mul(2));
    let mut ys = Vec::with_capacity(cells.len().saturating_mul(2));
    for cell in cells {
        xs.extend([cell.start.x, cell.end.x]);
        ys.extend([cell.start.y, cell.end.y]);
    }
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    xs.dedup_by(|left, right| *left == *right);
    ys.dedup_by(|left, right| *left == *right);
    (xs, ys)
}

fn geometry_operation_count(
    table: &PcbTable,
    cells: &[PcbTableCell],
    x_count: usize,
    y_count: usize,
) -> Result<usize, Error> {
    if cells.is_empty() {
        return Ok(0);
    }
    let columns = if table.separator_columns {
        x_count
            .saturating_sub(2)
            .checked_mul(y_count.saturating_sub(1))
    } else {
        Some(0)
    };
    let rows = if table.separator_rows {
        y_count
            .saturating_sub(2)
            .checked_mul(x_count.saturating_sub(1))
    } else {
        Some(0)
    };
    columns
        .and_then(|count| count.checked_add(rows?))
        .and_then(|count| count.checked_add(usize::from(table.border_external) * 4))
        .ok_or_else(limit_error)
}

fn append_geometry(
    operations: &mut Vec<BoardTableOperation>,
    table: &PcbTable,
    cells: &[PcbTableCell],
    xs: &[f64],
    ys: &[f64],
    separator_width: f64,
    border_width: f64,
) -> Result<(), Error> {
    if cells.is_empty() {
        return Ok(());
    }
    if table.separator_columns {
        for &x in xs.iter().skip(1).take(xs.len().saturating_sub(2)) {
            for pair in ys.windows(2) {
                push_segment(
                    operations,
                    [x, pair[0]],
                    [x, pair[1]],
                    separator_width,
                    &table.layer,
                )?;
            }
        }
    }
    if table.separator_rows {
        for &y in ys.iter().skip(1).take(ys.len().saturating_sub(2)) {
            for pair in xs.windows(2) {
                push_segment(
                    operations,
                    [pair[1], y],
                    [pair[0], y],
                    separator_width,
                    &table.layer,
                )?;
            }
        }
    }
    if table.border_external {
        let (&min_x, &max_x, &min_y, &max_y) = (
            xs.first().ok_or_else(limit_error)?,
            xs.last().ok_or_else(limit_error)?,
            ys.first().ok_or_else(limit_error)?,
            ys.last().ok_or_else(limit_error)?,
        );
        for (start, end) in [
            ([min_x, min_y], [max_x, min_y]),
            ([max_x, min_y], [max_x, max_y]),
            ([max_x, max_y], [min_x, max_y]),
            ([min_x, max_y], [min_x, min_y]),
        ] {
            push_segment(operations, start, end, border_width, &table.layer)?;
        }
    }
    Ok(())
}

fn push_segment(
    operations: &mut Vec<BoardTableOperation>,
    start: [f64; 2],
    end: [f64; 2],
    width: f64,
    layer: &str,
) -> Result<(), Error> {
    let mut segment = match layerless_segment(
        [mm_to_nm(start[0])?, mm_to_nm(start[1])?],
        [mm_to_nm(end[0])?, mm_to_nm(end[1])?],
        mm_to_nm(width)?,
    ) {
        PlotterOperation::ThickSegment(segment) => segment,
        _ => unreachable!("layerless_segment always returns ThickSegment"),
    };
    segment.layer = Some(layer.to_owned());
    operations.push(BoardTableOperation::Segment(
        PlotterOperation::ThickSegment(segment),
    ));
    Ok(())
}

fn table_cell_text_operation(
    source: &str,
    table: &PcbTable,
    cell: &PcbTableCell,
    cell_index: usize,
    context: TableCellContext<'_>,
) -> Result<Option<BoardTextOperation>, Error> {
    let TableCellContext {
        variables,
        budget,
        text_cache,
        limits,
    } = context;
    if cell.text.is_empty() {
        return Ok(None);
    }
    let form = parse_span(source, cell.source_range.clone(), limits)?;
    if child(&form, "effects").is_none() {
        return Ok(None);
    }
    let effects = text_effects(&form)?;
    if effects.face.is_none() {
        // Python requests no text params for a cache-only, non-faced cell, so
        // `_text_op_from_render_cache_request` emits no operation.
        return Ok(None);
    }
    budget.ensure_capacity(1, 0)?;
    let resolved = resolved_cell_text(
        table,
        cell,
        cell_index,
        variables,
        budget.remaining_text_bytes()?,
    )?;
    if resolved.contains(' ') && text_cache.is_none() {
        return Err(unsupported_outline_error());
    }
    let parsed_cache = parse_render_cache(
        &form,
        budget.remaining_points()?,
        limits.max_cache_polygons,
        limits.max_cache_contours,
    )?;
    let (x, y) = draw_position(cell, &effects.justify);
    let (h_align, v_align) = alignments(&effects.justify);
    let h_align = h_align.unwrap_or(BoardTextHAlign::Left);
    let v_align = v_align.unwrap_or(BoardTextVAlign::Bottom);
    let (cache_h_align, cache_v_align) = alignments(&effects.justify);
    let base_layout = table_cell_layout(
        cell,
        &effects,
        &resolved,
        x,
        y,
        cache_h_align.unwrap_or(BoardTextHAlign::Center),
        cache_v_align.unwrap_or(BoardTextVAlign::Center),
    );
    let (wrapped, cache) = table_cell_wrapped_cache(
        &resolved,
        parsed_cache,
        base_layout,
        text_cache,
        cell_wrap_width(cell),
    )?;
    ensure_retained_text_bytes(wrapped.len(), 2, budget.remaining_text_bytes()?)?;
    let multiline = wrapped.contains('\n');
    let mut operation = BoardTextOperation {
        x: mm_to_nm(x)?,
        y: mm_to_nm(y)?,
        text: wrapped,
        color: effects.color.clone(),
        orient_deg: cell.angle,
        size_x_nm: mm_to_nm(effects.size_x)?,
        size_y_nm: mm_to_nm(effects.size_y)?,
        h_align,
        v_align,
        pen_width_nm: effects.thickness.map(mm_to_nm).transpose()?.unwrap_or(0),
        italic: effects.italic,
        bold: effects.bold,
        multiline,
        font_face: effects.face.clone().unwrap_or_default(),
        layer: Some(cell.layer.clone()),
        mirror: false,
        text_as_polygons: false,
        polyline_per_segment: false,
        knockout: false,
        render_cache_polygons: Vec::new(),
        render_cache: None,
    };
    attach_table_cell_cache(
        &mut operation,
        cache.as_ref(),
        base_layout,
        text_cache,
        budget,
        limits,
    )?;
    Ok(Some(operation))
}

fn table_cell_wrapped_cache(
    resolved: &str,
    parsed_cache: Option<AuthoredRenderCache>,
    base_layout: PlotterTextLayout<'_>,
    text_cache: Option<&PlotterTextCacheSession<'_>>,
    wrap_width: f64,
) -> Result<(String, Option<AuthoredRenderCache>), Error> {
    let wrapped = match text_cache {
        Some(resources) => resources.linebreak(base_layout, wrap_width)?,
        None => resolved.to_owned(),
    };
    let valid_cache = parsed_cache.filter(|cache| cache_is_valid_without_angle(cache, &wrapped));
    if wrapped.is_empty() || valid_cache.is_some() || text_cache.is_some() {
        Ok((wrapped, valid_cache))
    } else {
        Err(unsupported_outline_error())
    }
}

fn attach_table_cell_cache(
    operation: &mut BoardTextOperation,
    cache: Option<&AuthoredRenderCache>,
    base_layout: PlotterTextLayout<'_>,
    text_cache: Option<&PlotterTextCacheSession<'_>>,
    budget: &BudgetTracker,
    limits: BoardPlotLimits,
) -> Result<(), Error> {
    if let Some(cache) = cache {
        attach_authored_cache(operation, cache, false, budget.remaining_points()?, false)?;
    } else if !operation.text.is_empty()
        && let Some(resources) = text_cache
    {
        let mut layout = base_layout;
        layout.text = &operation.text;
        let generated = resources.generate(
            layout,
            budget.remaining_points()?,
            limits.max_cache_polygons,
            limits.max_cache_contours,
        )?;
        attach_native_cache(operation, generated, budget.remaining_points()?)?;
    }
    Ok(())
}

fn table_cell_layout<'a>(
    cell: &PcbTableCell,
    effects: &'a super::text::TextEffects,
    text: &'a str,
    x: f64,
    y: f64,
    h_align: BoardTextHAlign,
    v_align: BoardTextVAlign,
) -> PlotterTextLayout<'a> {
    PlotterTextLayout {
        text,
        face: effects.face.as_deref().unwrap_or_default(),
        bold: effects.bold,
        italic: effects.italic,
        size_x: effects.size_x,
        size_y: effects.size_y,
        position_x: x,
        position_y: y,
        angle_degrees: cell.angle,
        mirrored: effects.justify.iter().any(|value| value == "mirror"),
        horizontal_alignment: native_h_align(h_align),
        vertical_alignment: native_v_align(v_align),
        line_spacing: effective_line_spacing(effects.line_spacing),
        stroke_width: effects.effective_thickness(),
    }
}

fn resolved_cell_text(
    table: &PcbTable,
    cell: &PcbTableCell,
    cell_index: usize,
    variables: &BoardTextVariables,
    max_bytes: usize,
) -> Result<String, Error> {
    let layer_local = [
        ("LAYER", cell.layer.as_str()),
        ("layer", cell.layer.as_str()),
    ];
    if table.column_count <= 0 {
        return variables.substitute_bounded_with_local(&cell.text, &layer_local, max_bytes);
    }
    let index = i64::try_from(cell_index).map_err(|_| limit_error())?;
    let row = index / table.column_count + 1;
    let column = index % table.column_count + 1;
    let row_text = row.to_string();
    let column_text = column.to_string();
    let address = format!("{}{}", (b'A' + ((column - 1) % 26) as u8) as char, row);
    let local = [
        ("ROW", row_text.as_str()),
        ("row", row_text.as_str()),
        ("COL", column_text.as_str()),
        ("col", column_text.as_str()),
        ("ADDR", address.as_str()),
        ("addr", address.as_str()),
        layer_local[0],
        layer_local[1],
    ];
    variables.substitute_bounded_with_local(&cell.text, &local, max_bytes)
}

fn draw_position(cell: &PcbTableCell, justify: &[String]) -> (f64, f64) {
    let corners = oriented_cell_corners(cell);
    let (horizontal, vertical) = cell_text_alignment(justify);
    let anchor = aligned_cell_anchor(&corners, horizontal, vertical);
    let [offset_x, offset_y] = cell_margin_offset(cell, horizontal, vertical);
    let radians = cell.angle.to_radians();
    let rotated_x = offset_x * radians.cos() + offset_y * radians.sin();
    let rotated_y = offset_y * radians.cos() - offset_x * radians.sin();
    (anchor[0] + rotated_x, anchor[1] + rotated_y)
}

fn cell_wrap_width(cell: &PcbTableCell) -> f64 {
    let corners = oriented_cell_corners(cell);
    let width = (corners[1][0] - corners[0][0]).hypot(corners[1][1] - corners[0][1]);
    let angle = cell.angle.rem_euclid(360.0);
    let horizontal = angle.abs() <= 1e-9 || (angle - 180.0).abs() <= 1e-9;
    let margins = if horizontal {
        cell.margins[0] + cell.margins[2]
    } else {
        cell.margins[1] + cell.margins[3]
    };
    (width - margins).max(0.0)
}

fn oriented_cell_corners(cell: &PcbTableCell) -> [[f64; 2]; 4] {
    let left = cell.start.x.min(cell.end.x);
    let right = cell.start.x.max(cell.end.x);
    let top = cell.start.y.min(cell.end.y);
    let bottom = cell.start.y.max(cell.end.y);
    let angle = cell.angle.rem_euclid(360.0);
    let close = |value: f64| (angle - value).abs() <= 1e-9;
    if close(90.0) {
        [[left, bottom], [left, top], [right, top], [right, bottom]]
    } else if close(180.0) {
        [[right, bottom], [left, bottom], [left, top], [right, top]]
    } else if close(270.0) {
        [[right, top], [right, bottom], [left, bottom], [left, top]]
    } else {
        [[left, top], [right, top], [right, bottom], [left, bottom]]
    }
}

fn cell_text_alignment(justify: &[String]) -> (BoardTextHAlign, BoardTextVAlign) {
    let mut horizontal = if justify.iter().any(|value| value == "left") {
        BoardTextHAlign::Left
    } else if justify.iter().any(|value| value == "right") {
        BoardTextHAlign::Right
    } else {
        BoardTextHAlign::Center
    };
    if justify.iter().any(|value| value == "mirror") {
        horizontal = match horizontal {
            BoardTextHAlign::Left => BoardTextHAlign::Right,
            BoardTextHAlign::Right => BoardTextHAlign::Left,
            BoardTextHAlign::Center => BoardTextHAlign::Center,
        };
    }
    let vertical = if justify.iter().any(|value| value == "top") {
        BoardTextVAlign::Top
    } else if justify.iter().any(|value| value == "bottom") {
        BoardTextVAlign::Bottom
    } else {
        BoardTextVAlign::Center
    };
    (horizontal, vertical)
}

fn aligned_cell_anchor(
    corners: &[[f64; 2]; 4],
    horizontal: BoardTextHAlign,
    vertical: BoardTextVAlign,
) -> [f64; 2] {
    let midpoint =
        |left: [f64; 2], right: [f64; 2]| [(left[0] + right[0]) / 2.0, (left[1] + right[1]) / 2.0];
    let center = [
        corners.iter().map(|point| point[0]).sum::<f64>() / 4.0,
        corners.iter().map(|point| point[1]).sum::<f64>() / 4.0,
    ];
    match (horizontal, vertical) {
        (BoardTextHAlign::Left, BoardTextVAlign::Top) => corners[0],
        (BoardTextHAlign::Center, BoardTextVAlign::Top) => midpoint(corners[0], corners[1]),
        (BoardTextHAlign::Right, BoardTextVAlign::Top) => corners[1],
        (BoardTextHAlign::Left, BoardTextVAlign::Center) => midpoint(corners[0], corners[3]),
        (BoardTextHAlign::Center, BoardTextVAlign::Center) => center,
        (BoardTextHAlign::Right, BoardTextVAlign::Center) => midpoint(corners[1], corners[2]),
        (BoardTextHAlign::Left, BoardTextVAlign::Bottom) => corners[3],
        (BoardTextHAlign::Center, BoardTextVAlign::Bottom) => midpoint(corners[3], corners[2]),
        (BoardTextHAlign::Right, BoardTextVAlign::Bottom) => corners[2],
    }
}

fn cell_margin_offset(
    cell: &PcbTableCell,
    horizontal: BoardTextHAlign,
    vertical: BoardTextVAlign,
) -> [f64; 2] {
    let [margin_left, margin_top, margin_right, margin_bottom] = cell.margins;
    let offset_x = match horizontal {
        BoardTextHAlign::Left => margin_left,
        BoardTextHAlign::Right => -margin_right,
        BoardTextHAlign::Center => 0.0,
    };
    let offset_y = match vertical {
        BoardTextVAlign::Top => margin_top,
        BoardTextVAlign::Bottom => -margin_bottom,
        BoardTextVAlign::Center => 0.0,
    };
    [offset_x, offset_y]
}

fn nested_stroke_width(form: &Sexp, head: &str) -> Result<f64, Error> {
    let Some(stroke) = child(form, head).and_then(|value| child(value, "stroke")) else {
        return Ok(DEFAULT_TABLE_STROKE_MM);
    };
    numeric_or(child(stroke, "width"), 1, 0.0)
}

fn parse_span(
    source: &str,
    range: std::ops::Range<usize>,
    limits: BoardPlotLimits,
) -> Result<Sexp, Error> {
    let text = source
        .get(range)
        .ok_or_else(|| model_error("Board table span is out of range"))?;
    parse_with_limits(
        text,
        Limits {
            max_source_bytes: text.len(),
            max_depth: limits.max_depth,
            max_nodes: limits.max_parse_nodes,
            max_decoded_string_bytes: limits.max_source_bytes,
        },
    )
}

fn unsupported_outline_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::InvalidBuildValue,
        "Board table-cell outline generation requires the outline-font bridge",
        Position::START,
    )
}

fn model_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::InvalidBuildValue,
        message,
        Position::START,
    )
}
