use super::{
    DEFAULT_KICAD_VERSION_TEXT, Paper, PlotBudget, SchematicPlotContext, SchematicPlotLimits,
    SchematicPlotOperation, SchematicTitleBlock, color_hex, limit_error, model_error,
};
use crate::plotter_ir::mm_to_nm;
use crate::plotter_types::{
    PlotterFill, PlotterImage, PlotterOperation, PlotterPoly, PlotterRect, PlotterText,
    PlotterTextHAlign, PlotterTextVAlign,
};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Limits, Position, parse_with_limits};
use crate::worksheet::{
    WorksheetBitmap, WorksheetCorner, WorksheetItem, WorksheetLimits, WorksheetPoint,
    WorksheetRepeat, WorksheetSetup, WorksheetText, WorksheetView,
};
use std::collections::BTreeMap;

const DRAWING_SHEET_COLOR: &str = "#840000FF";
const BACKGROUND_COLOR: &str = "#F5F4EFFF";
const DRAWING_SHEET_MIN_WIDTH_NM: i64 = 152_400;
const BITMAP_DEFAULT_DPI: f64 = 300.0;
const MAX_EXPANSION_DEPTH: usize = 10;

const DEFAULT_WORKSHEET: &str = concat!(
    "(kicad_wks (version 20210606) (generator pl_editor)\n",
    "(setup (textsize 1.5 1.5)(linewidth 0.15)(textlinewidth 0.15)\n",
    "(left_margin 10)(right_margin 10)(top_margin 10)(bottom_margin 10))\n",
    "(rect (name \"\") (start 110 34) (end 2 2) (comment \"rect around the title block\"))\n",
    "(rect (name \"\") (start 0 0 ltcorner) (end 0 0) (repeat 2) (incrx 2) (incry 2))\n",
    "(line (name \"\") (start 50 2 ltcorner) (end 50 0 ltcorner) (repeat 30) (incrx 50))\n",
    "(tbtext \"1\" (name \"\") (pos 25 1 ltcorner) (font (size 1.3 1.3)) (repeat 100) (incrx 50))\n",
    "(line (name \"\") (start 50 2 lbcorner) (end 50 0 lbcorner) (repeat 30) (incrx 50))\n",
    "(tbtext \"1\" (name \"\") (pos 25 1 lbcorner) (font (size 1.3 1.3)) (repeat 100) (incrx 50))\n",
    "(line (name \"\") (start 0 50 ltcorner) (end 2 50 ltcorner) (repeat 30) (incry 50))\n",
    "(tbtext \"A\" (name \"\") (pos 1 25 ltcorner) (font (size 1.3 1.3)) (justify center) (repeat 100) (incry 50))\n",
    "(line (name \"\") (start 0 50 rtcorner) (end 2 50 rtcorner) (repeat 30) (incry 50))\n",
    "(tbtext \"A\" (name \"\") (pos 1 25 rtcorner) (font (size 1.3 1.3)) (justify center) (repeat 100) (incry 50))\n",
    "(tbtext \"Date: ${ISSUE_DATE}\" (name \"\") (pos 87 6.9))\n",
    "(line (name \"\") (start 110 5.5) (end 2 5.5))\n",
    "(tbtext \"${KICAD_VERSION}\" (name \"\") (pos 109 4.1) (comment \"Kicad version\"))\n",
    "(line (name \"\") (start 110 8.5) (end 2 8.5))\n",
    "(tbtext \"Rev: ${REVISION}\" (name \"\") (pos 24 6.9) (font bold))\n",
    "(tbtext \"Size: ${PAPER}\" (name \"\") (pos 109 6.9) (comment \"Paper format name\"))\n",
    "(tbtext \"Id: ${#}/${##}\" (name \"\") (pos 24 4.1) (comment \"Sheet id\"))\n",
    "(line (name \"\") (start 110 12.5) (end 2 12.5))\n",
    "(tbtext \"Title: ${TITLE}\" (name \"\") (pos 109 10.7) (font (size 2 2) bold italic))\n",
    "(tbtext \"File: ${FILENAME}\" (name \"\") (pos 109 14.3))\n",
    "(line (name \"\") (start 110 18.5) (end 2 18.5))\n",
    "(tbtext \"Sheet: ${SHEETPATH}\" (name \"\") (pos 109 17))\n",
    "(tbtext \"${COMPANY}\" (name \"\") (pos 109 20) (font bold) (comment \"Company name\"))\n",
    "(tbtext \"${COMMENT1}\" (name \"\") (pos 109 23) (comment \"Comment 0\"))\n",
    "(tbtext \"${COMMENT2}\" (name \"\") (pos 109 26) (comment \"Comment 1\"))\n",
    "(tbtext \"${COMMENT3}\" (name \"\") (pos 109 29) (comment \"Comment 2\"))\n",
    "(tbtext \"${COMMENT4}\" (name \"\") (pos 109 32) (comment \"Comment 3\"))\n",
    "(line (name \"\") (start 90 8.5) (end 90 5.5))\n",
    "(line (name \"\") (start 26 8.5) (end 26 2))\n",
    ")\n",
);

pub(super) fn drawing_sheet_operations(
    paper: &Paper,
    title_block: Option<&SchematicTitleBlock>,
    width_nm: i64,
    height_nm: i64,
    context: &SchematicPlotContext,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
) -> Result<Vec<SchematicPlotOperation>, Error> {
    let worksheet = worksheet_text(context, limits)?;
    // WorksheetView decodes items lazily. This aggregate preflight makes the
    // public parse-node limit cover the entire explicit/default sidecar.
    parse_with_limits(
        worksheet,
        Limits {
            max_source_bytes: limits.max_worksheet_bytes,
            max_depth: limits.max_depth,
            max_nodes: limits.max_parse_nodes,
            max_decoded_string_bytes: limits.max_worksheet_bytes,
        },
    )?;
    let worksheet_limits = WorksheetLimits {
        max_source_bytes: limits.max_worksheet_bytes,
        max_output_bytes: limits.max_worksheet_bytes,
        max_depth: limits.max_depth,
        max_top_level_forms: limits.max_worksheet_items.saturating_add(16),
        max_items: limits.max_worksheet_items,
        max_nodes_per_item: limits.max_parse_nodes,
        max_decoded_string_bytes: limits
            .max_text_bytes
            .max(limits.max_worksheet_bitmap_encoded_bytes),
        max_point_sets_per_polygon: limits.max_worksheet_point_sets,
        max_points_per_polygon: limits.max_worksheet_points,
        max_justify_tokens: 16,
        max_bitmap_data_parts: limits.max_worksheet_bitmap_data_parts,
        max_bitmap_data_bytes: limits.max_worksheet_bitmap_encoded_bytes,
    };
    let view = WorksheetView::parse(worksheet, worksheet_limits)?;
    let setup = view.setup()?;
    let mut operations = Vec::new();
    budget.charge(0, 1, 0)?;
    operations.push(
        PlotterOperation::Rect(PlotterRect {
            x1: 0,
            y1: 0,
            x2: width_nm,
            y2: height_nm,
            fill: PlotterFill::FilledShape,
            width_nm: 100,
            corner_radius_nm: 0,
            layer: None,
            stroke_color: Some(BACKGROUND_COLOR.to_owned()),
            fill_color: Some(BACKGROUND_COLOR.to_owned()),
            line_style: None,
        })
        .into(),
    );
    let page_w_mm = width_nm as f64 / 1_000_000.0;
    let page_h_mm = height_nm as f64 / 1_000_000.0;
    let mut repeat_total = 0usize;
    let mut image_budget = ImageBudget::default();
    let mut worksheet_point_sets = 0usize;
    let mut worksheet_points = 0usize;
    let mut worksheet_data_parts = 0usize;
    for item in view.items() {
        let item = item?;
        match &item {
            WorksheetItem::Polygon(value) => {
                worksheet_point_sets = checked(
                    worksheet_point_sets,
                    value.point_sets.len(),
                    limits.max_worksheet_point_sets,
                )?;
                let points = value
                    .point_sets
                    .iter()
                    .try_fold(0usize, |total, values| total.checked_add(values.len()))
                    .ok_or_else(limit_error)?;
                worksheet_points = checked(worksheet_points, points, limits.max_worksheet_points)?;
            }
            WorksheetItem::Bitmap(value) => {
                worksheet_data_parts = checked(
                    worksheet_data_parts,
                    value.data_parts.len(),
                    limits.max_worksheet_bitmap_data_parts,
                )?;
            }
            _ => {}
        }
        let (option, repeat) = item_option_repeat(&item);
        let count = repeat_count(repeat);
        repeat_total = repeat_total.checked_add(count).ok_or_else(limit_error)?;
        if repeat_total > limits.max_worksheet_repeats {
            return Err(limit_error());
        }
        if (option == "page1only" && context.sheet_index != 1)
            || (option == "notonpage1" && context.sheet_index == 1)
        {
            continue;
        }
        match item {
            WorksheetItem::Line(item) => line_operations(
                &mut operations,
                &item.start,
                &item.end,
                item.line_width.unwrap_or(setup.line_width),
                item.repeat,
                setup,
                page_w_mm,
                page_h_mm,
                budget,
            )?,
            WorksheetItem::Rect(item) => rect_operations(
                &mut operations,
                &item.start,
                &item.end,
                item.line_width.unwrap_or(setup.line_width),
                item.repeat,
                setup,
                page_w_mm,
                page_h_mm,
                budget,
            )?,
            WorksheetItem::Text(item) => text_operations(
                &mut operations,
                &item,
                paper,
                title_block,
                context,
                setup,
                page_w_mm,
                page_h_mm,
                budget,
                limits,
            )?,
            WorksheetItem::Bitmap(item) => bitmap_operations(
                &mut operations,
                &item,
                setup,
                page_w_mm,
                page_h_mm,
                budget,
                limits,
                &mut image_budget,
            )?,
            WorksheetItem::Polygon(_) => {}
        }
    }
    Ok(operations)
}

fn worksheet_text<'a>(
    context: &'a SchematicPlotContext,
    limits: SchematicPlotLimits,
) -> Result<&'a str, Error> {
    let Some(bytes) = context.worksheet_source.as_deref() else {
        return Ok(DEFAULT_WORKSHEET);
    };
    if bytes.len() > limits.max_worksheet_bytes {
        return Err(limit_error());
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    std::str::from_utf8(bytes).map_err(|_| {
        Error::at(
            ErrorPhase::Lex,
            ErrorKind::InvalidUtf8,
            "worksheet sidecar is not UTF-8",
            Position::START,
        )
    })
}

fn item_option_repeat(item: &WorksheetItem) -> (&str, WorksheetRepeat) {
    match item {
        WorksheetItem::Line(value) => (&value.option, value.repeat),
        WorksheetItem::Rect(value) => (&value.option, value.repeat),
        WorksheetItem::Polygon(value) => (&value.option, value.repeat),
        WorksheetItem::Text(value) => (&value.option, value.repeat),
        WorksheetItem::Bitmap(value) => (&value.option, value.repeat),
    }
}

fn repeat_count(repeat: WorksheetRepeat) -> usize {
    usize::try_from(repeat.count.max(1)).unwrap_or(usize::MAX)
}

fn line_operations(
    operations: &mut Vec<SchematicPlotOperation>,
    start: &WorksheetPoint,
    end: &WorksheetPoint,
    width_mm: f64,
    repeat: WorksheetRepeat,
    setup: WorksheetSetup,
    page_w: f64,
    page_h: f64,
    budget: &mut PlotBudget,
) -> Result<(), Error> {
    for index in 0..repeat_count(repeat) {
        let delta = (
            repeat.increment_x * index as f64,
            repeat.increment_y * index as f64,
        );
        let a = resolve_point(*start, page_w, page_h, setup, delta);
        let b = resolve_point(*end, page_w, page_h, setup, delta);
        if index > 0 && !(inside(a, page_w, page_h, setup) && inside(b, page_w, page_h, setup)) {
            continue;
        }
        budget.charge(0, 1, 2)?;
        operations.push(
            PlotterOperation::PlotPoly(PlotterPoly {
                points: vec![
                    [mm_to_nm(a.0)?, mm_to_nm(a.1)?],
                    [mm_to_nm(b.0)?, mm_to_nm(b.1)?],
                ],
                fill: PlotterFill::NoFill,
                width_nm: drawing_width(width_mm)?,
                layer: None,
                stroke_color: Some(DRAWING_SHEET_COLOR.to_owned()),
                fill_color: None,
                line_style: None,
            })
            .into(),
        );
    }
    Ok(())
}

fn rect_operations(
    operations: &mut Vec<SchematicPlotOperation>,
    start: &WorksheetPoint,
    end: &WorksheetPoint,
    width_mm: f64,
    repeat: WorksheetRepeat,
    setup: WorksheetSetup,
    page_w: f64,
    page_h: f64,
    budget: &mut PlotBudget,
) -> Result<(), Error> {
    for index in 0..repeat_count(repeat) {
        let delta = (
            repeat.increment_x * index as f64,
            repeat.increment_y * index as f64,
        );
        let a = resolve_point(*start, page_w, page_h, setup, delta);
        let b = resolve_point(*end, page_w, page_h, setup, delta);
        if index > 0 && !(inside(a, page_w, page_h, setup) && inside(b, page_w, page_h, setup)) {
            continue;
        }
        budget.charge(0, 1, 0)?;
        operations.push(
            PlotterOperation::Rect(PlotterRect {
                x1: mm_to_nm(a.0)?,
                y1: mm_to_nm(a.1)?,
                x2: mm_to_nm(b.0)?,
                y2: mm_to_nm(b.1)?,
                fill: PlotterFill::NoFill,
                width_nm: drawing_width(width_mm)?,
                corner_radius_nm: 0,
                layer: None,
                stroke_color: Some(DRAWING_SHEET_COLOR.to_owned()),
                fill_color: None,
                line_style: None,
            })
            .into(),
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn text_operations(
    operations: &mut Vec<SchematicPlotOperation>,
    item: &WorksheetText,
    paper: &Paper,
    title: Option<&SchematicTitleBlock>,
    context: &SchematicPlotContext,
    setup: WorksheetSetup,
    page_w: f64,
    page_h: f64,
    budget: &mut PlotBudget,
    limits: SchematicPlotLimits,
) -> Result<(), Error> {
    let count = repeat_count(item.repeat);
    let increment =
        item.repeat.increment_label + i64::from(item.repeat.increment_label == 0 && count > 1);
    let size_x = if item.font.size_x == 0.0 {
        setup.text_size_x
    } else {
        item.font.size_x
    };
    let size_y = if item.font.size_y == 0.0 {
        setup.text_size_x
    } else {
        item.font.size_y
    };
    let size_x_nm = mm_to_nm(size_x)?;
    let size_y_nm = mm_to_nm(size_y)?;
    let width_mm = item.font.line_width.unwrap_or(setup.text_line_width);
    let pen_width = if item.font.bold {
        ((size_x_nm.abs().min(size_y_nm.abs()) as f64 / 5.0).round_ties_even() as i64)
            .max(DRAWING_SHEET_MIN_WIDTH_NM)
    } else {
        drawing_width(width_mm)?
    };
    let (h_align, v_align) = alignments(&item.justify);
    for index in 0..count {
        let point = resolve_point(
            item.position,
            page_w,
            page_h,
            setup,
            (
                item.repeat.increment_x * index as f64,
                item.repeat.increment_y * index as f64,
            ),
        );
        if index > 0 && !inside(point, page_w, page_h, setup) {
            continue;
        }
        let raw = if index > 0 && increment != 0 {
            increment_label(&item.text, (index as i64).saturating_mul(increment))
        } else {
            item.text.clone()
        };
        let mut body = expand_text(&raw, paper, title, context, limits.max_text_bytes)?;
        if body.ends_with("\r\n") {
            body.truncate(body.len() - 2)
        } else if body.ends_with('\r') || body.ends_with('\n') {
            body.pop();
        }
        budget.charge_text(body.len())?;
        let font_face = if item.font.face.is_empty() {
            "Arial"
        } else {
            item.font.face.as_str()
        };
        budget.charge_metadata(font_face.len())?;
        budget.charge(0, 1, 0)?;
        let color = item
            .font
            .color
            .and_then(|c| color_hex(c.red, c.green, c.blue, c.alpha))
            .unwrap_or_else(|| DRAWING_SHEET_COLOR.to_owned());
        let multiline = body.contains('\n');
        operations.push(
            PlotterOperation::Text(PlotterText {
                x: mm_to_nm(point.0)?,
                y: mm_to_nm(point.1)?,
                text: body,
                color,
                orient_deg: item.rotate,
                size_x_nm,
                size_y_nm,
                h_align,
                v_align,
                pen_width_nm: pen_width,
                italic: item.font.italic,
                bold: item.font.bold,
                multiline,
                font_face: font_face.to_owned(),
                layer: None,
            })
            .into(),
        );
    }
    Ok(())
}

#[derive(Default)]
struct ImageBudget {
    encoded: usize,
    decoded: usize,
    pixels: usize,
    work: usize,
}

#[allow(clippy::too_many_arguments)]
fn bitmap_operations(
    operations: &mut Vec<SchematicPlotOperation>,
    item: &WorksheetBitmap,
    setup: WorksheetSetup,
    page_w: f64,
    page_h: f64,
    budget: &mut PlotBudget,
    limits: SchematicPlotLimits,
    images: &mut ImageBudget,
) -> Result<(), Error> {
    if !item.scale.is_finite() || item.scale <= 0.0 {
        return Err(model_error(
            "Worksheet bitmap scale must be finite and positive",
        ));
    }
    let encoded = item.data_parts.concat();
    images.encoded = checked(
        images.encoded,
        encoded.len(),
        limits.max_worksheet_bitmap_encoded_bytes,
    )?;
    let decoded = decode_base64(
        &encoded,
        limits
            .max_worksheet_bitmap_decoded_bytes
            .saturating_sub(images.decoded),
    )?;
    images.decoded = checked(
        images.decoded,
        decoded.len(),
        limits.max_worksheet_bitmap_decoded_bytes,
    )?;
    images.work = checked(
        images.work,
        encoded.len().saturating_add(decoded.len()),
        limits.max_worksheet_bitmap_decode_work,
    )?;
    let (width, height, ppm_x, ppm_y, metadata_work) = png_metadata(
        &decoded,
        limits
            .max_worksheet_bitmap_decode_work
            .saturating_sub(images.work),
    )?;
    images.work = checked(
        images.work,
        metadata_work,
        limits.max_worksheet_bitmap_decode_work,
    )?;
    if width as usize > limits.max_worksheet_bitmap_width_px
        || height as usize > limits.max_worksheet_bitmap_height_px
    {
        return Err(limit_error());
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(limit_error)?;
    images.pixels = checked(images.pixels, pixels, limits.max_worksheet_bitmap_pixels)?;
    let width_nm = bitmap_extent(width, item.scale, ppm_x)?;
    let height_nm = bitmap_extent(height, item.scale, ppm_y)?;
    for index in 0..repeat_count(item.repeat) {
        let point = resolve_point(
            item.position,
            page_w,
            page_h,
            setup,
            (
                item.repeat.increment_x * index as f64,
                item.repeat.increment_y * index as f64,
            ),
        );
        if index > 0 && !inside(point, page_w, page_h, setup) {
            continue;
        }
        budget.charge_metadata(encoded.len())?;
        budget.charge(0, 1, 0)?;
        operations.push(SchematicPlotOperation::PlotImage(PlotterImage {
            x: mm_to_nm(point.0)?,
            y: mm_to_nm(point.1)?,
            width_nm,
            height_nm,
            scale: item.scale,
            image_data_b64: encoded.clone(),
            image_format: "png".to_owned(),
            stroke_color: Some(DRAWING_SHEET_COLOR.to_owned()),
        }));
    }
    Ok(())
}

fn resolve_point(
    point: WorksheetPoint,
    page_w: f64,
    page_h: f64,
    setup: WorksheetSetup,
    delta: (f64, f64),
) -> (f64, f64) {
    let (l, r, t, b) = (
        setup.left_margin,
        setup.right_margin,
        setup.top_margin,
        setup.bottom_margin,
    );
    let (origin_x, origin_y, sx, sy) = match point.corner {
        WorksheetCorner::LeftTop => (l, t, 1.0, 1.0),
        WorksheetCorner::RightTop => (page_w - r, t, -1.0, 1.0),
        WorksheetCorner::LeftBottom => (l, page_h - b, 1.0, -1.0),
        WorksheetCorner::RightBottom | WorksheetCorner::None => {
            (page_w - r, page_h - b, -1.0, -1.0)
        }
    };
    (
        origin_x + sx * (point.x + delta.0),
        origin_y + sy * (point.y + delta.1),
    )
}
fn inside(point: (f64, f64), page_w: f64, page_h: f64, setup: WorksheetSetup) -> bool {
    point.0 >= setup.left_margin
        && point.0 <= page_w - setup.right_margin
        && point.1 >= setup.top_margin
        && point.1 <= page_h - setup.bottom_margin
}
fn drawing_width(width: f64) -> Result<i64, Error> {
    Ok(mm_to_nm(width)?.max(DRAWING_SHEET_MIN_WIDTH_NM))
}
fn alignments(values: &[String]) -> (PlotterTextHAlign, PlotterTextVAlign) {
    let (mut h, mut v) = (PlotterTextHAlign::Left, PlotterTextVAlign::Center);
    for value in values {
        match value.as_str() {
            "left" => h = PlotterTextHAlign::Left,
            "center" => h = PlotterTextHAlign::Center,
            "right" => h = PlotterTextHAlign::Right,
            "top" => v = PlotterTextVAlign::Top,
            "bottom" => v = PlotterTextVAlign::Bottom,
            _ => {}
        }
    }
    (h, v)
}

fn increment_label(value: &str, increment: i64) -> String {
    let Some(last) = value.chars().last() else {
        return value.to_owned();
    };
    if last.is_ascii_digit() {
        let split = value.trim_end_matches(|c: char| c.is_ascii_digit()).len();
        return value[split..]
            .parse::<i64>()
            .ok()
            .and_then(|number| number.checked_add(increment))
            .map_or_else(
                || value.to_owned(),
                |number| format!("{}{number}", &value[..split]),
            );
    }
    if last.is_alphabetic() {
        let prefix = &value[..value.len() - last.len_utf8()];
        return u32::try_from(i64::from(last as u32).saturating_add(increment))
            .ok()
            .and_then(char::from_u32)
            .map_or_else(|| value.to_owned(), |next| format!("{prefix}{next}"));
    }
    value.to_owned()
}

fn expand_text(
    value: &str,
    paper: &Paper,
    title: Option<&SchematicTitleBlock>,
    context: &SchematicPlotContext,
    maximum: usize,
) -> Result<String, Error> {
    let mut builtins: BTreeMap<String, String> = BTreeMap::new();
    let title = title.cloned().unwrap_or_default();
    builtins.insert("TITLE".to_owned(), title.title);
    builtins.insert("ISSUE_DATE".to_owned(), title.date);
    builtins.insert("REVISION".to_owned(), title.revision);
    builtins.insert("REV".to_owned(), builtins["REVISION"].clone());
    builtins.insert("COMPANY".to_owned(), title.company);
    builtins.insert("PAPER".to_owned(), paper.size.clone());
    builtins.insert(
        "FILENAME".to_owned(),
        context
            .source_path
            .as_deref()
            .and_then(|path| path.rsplit(['/', '\\']).next())
            .unwrap_or("")
            .to_owned(),
    );
    builtins.insert("SHEETPATH".to_owned(), context.sheet_path.clone());
    builtins.insert(
        "SHEETNAME".to_owned(),
        if context.sheet_path == "/" {
            String::new()
        } else {
            context.sheet_name.clone()
        },
    );
    builtins.insert(
        "KICAD_VERSION".to_owned(),
        DEFAULT_KICAD_VERSION_TEXT.to_owned(),
    );
    builtins.insert("#".to_owned(), context.sheet_index.to_string());
    builtins.insert("##".to_owned(), context.sheet_count.to_string());
    builtins.insert("SHEETNUMBER".to_owned(), context.sheet_index.to_string());
    builtins.insert("SHEETCOUNT".to_owned(), context.sheet_count.to_string());
    builtins.insert("VARIANT".to_owned(), String::new());
    for index in 1..10 {
        builtins.insert(
            format!("COMMENT{index}"),
            title
                .comments
                .get(&(index as i64))
                .cloned()
                .unwrap_or_default(),
        );
    }
    let mut out = value.to_owned();
    for _ in 0..MAX_EXPANSION_DEPTH {
        let next = expand_modern_once(&out, &builtins, context, maximum)?;
        if next == out {
            break;
        }
        out = next;
    }
    expand_legacy(&out, &builtins, maximum)
}

fn expand_modern_once(
    value: &str,
    builtins: &BTreeMap<String, String>,
    context: &SchematicPlotContext,
    maximum: usize,
) -> Result<String, Error> {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let Some(end) = tail.find('}') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let name = tail[..end].trim();
        if name.is_empty() {
            out.push_str("${");
            out.push_str(&tail[..=end]);
        } else if let Some(value) = builtins.get(name) {
            let value = value.replace("\\n", "\n");
            let self_reference = value.len() == name.len().saturating_add(3)
                && value.starts_with("${")
                && value.ends_with('}')
                && &value[2..value.len() - 1] == name;
            if self_reference {
                out.push_str(context.project_variables.get(name).unwrap_or(&value));
            } else {
                out.push_str(&value);
            }
        } else if let Some(value) = context.project_variables.get(name) {
            out.push_str(value);
        } else {
            out.push_str("${");
            out.push_str(&tail[..=end]);
        }
        if out.len() > maximum {
            return Err(limit_error());
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    if out.len() > maximum {
        return Err(limit_error());
    }
    Ok(out)
}

fn expand_legacy(
    value: &str,
    builtins: &BTreeMap<String, String>,
    maximum: usize,
) -> Result<String, Error> {
    let mut out = String::new();
    let chars = value.chars().collect::<Vec<_>>();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] != '%' || index + 1 >= chars.len() {
            out.push(chars[index]);
            index += 1;
            continue;
        }
        let code = chars[index + 1];
        if code == '%' {
            out.push('%');
            index += 2;
            continue;
        }
        if code == 'C' && index + 2 < chars.len() && chars[index + 2].is_ascii_digit() {
            let slot = chars[index + 2].to_digit(10).unwrap() + 1;
            let key = format!("COMMENT{slot}");
            out.push_str(builtins.get(key.as_str()).map_or("", String::as_str));
            index += 3;
            continue;
        }
        let key = match code {
            'K' => Some("KICAD_VERSION"),
            'Z' => Some("PAPER"),
            'Y' => Some("COMPANY"),
            'D' => Some("ISSUE_DATE"),
            'R' => Some("REVISION"),
            'S' => Some("#"),
            'N' => Some("##"),
            'F' => Some("FILENAME"),
            'P' => Some("SHEETPATH"),
            'T' => Some("TITLE"),
            _ => None,
        };
        if let Some(key) = key {
            out.push_str(builtins.get(key).map_or("", String::as_str));
            index += 2;
        } else {
            out.push('%');
            index += 1;
        }
        if out.len() > maximum {
            return Err(limit_error());
        }
    }
    Ok(out)
}

fn decode_base64(value: &str, maximum: usize) -> Result<Vec<u8>, Error> {
    let bytes = value.as_bytes();
    if bytes.len() % 4 != 0 {
        return Err(model_error("Invalid worksheet bitmap base64 length"));
    }
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return Err(model_error("Invalid worksheet bitmap base64 padding"));
    }
    let decoded_len = (bytes.len() / 4)
        .checked_mul(3)
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(limit_error)?;
    if decoded_len > maximum {
        return Err(limit_error());
    }
    let mut output = Vec::with_capacity(decoded_len);
    for (block_index, encoded) in bytes.chunks_exact(4).enumerate() {
        let mut block = [0u8; 4];
        for (index, byte) in encoded.iter().copied().enumerate() {
            block[index] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                b'=' => 64,
                _ => return Err(model_error("Invalid worksheet bitmap base64")),
            }
        }
        let last = block_index + 1 == bytes.len() / 4;
        if block[0] == 64
            || block[1] == 64
            || (!last && (block[2] == 64 || block[3] == 64))
            || (block[2] == 64 && block[3] != 64)
            || (block[2] == 64 && block[1] & 0x0f != 0)
            || (block[3] == 64 && block[2] != 64 && block[2] & 0x03 != 0)
        {
            return Err(model_error("Invalid worksheet bitmap base64 padding"));
        }
        output.push((block[0] << 2) | (block[1] >> 4));
        if block[2] != 64 {
            output.push((block[1] << 4) | (block[2] >> 2));
        }
        if block[3] != 64 {
            output.push((block[2] << 6) | block[3]);
        }
    }
    debug_assert_eq!(output.len(), decoded_len);
    Ok(output)
}

fn png_metadata(
    data: &[u8],
    maximum_work: usize,
) -> Result<(u32, u32, Option<u32>, Option<u32>, usize), Error> {
    if data.len() < 33 || !data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(model_error("Worksheet bitmap is not PNG"));
    }
    let first_length = u32::from_be_bytes(data[8..12].try_into().unwrap()) as usize;
    if first_length != 13 || &data[12..16] != b"IHDR" {
        return Err(model_error(
            "Worksheet PNG must begin with a canonical IHDR",
        ));
    }
    let (mut width, mut height, mut ppm_x, mut ppm_y) = (0, 0, None, None);
    let (mut offset, mut work) = (8usize, 0usize);
    while offset + 8 <= data.len() {
        let length = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|v| v.checked_add(length))
            .ok_or_else(limit_error)?;
        if end > data.len() {
            return Err(model_error("Malformed worksheet PNG"));
        }
        work = checked(work, length.saturating_add(12), maximum_work)?;
        let kind = &data[offset + 4..offset + 8];
        let chunk = &data[offset + 8..offset + 8 + length];
        if kind == b"IHDR" && offset == 8 {
            width = u32::from_be_bytes(chunk[0..4].try_into().unwrap());
            height = u32::from_be_bytes(chunk[4..8].try_into().unwrap());
        } else if kind == b"pHYs" && length >= 9 && chunk[8] == 1 {
            ppm_x = Some(u32::from_be_bytes(chunk[0..4].try_into().unwrap())).filter(|v| *v > 0);
            ppm_y = Some(u32::from_be_bytes(chunk[4..8].try_into().unwrap())).filter(|v| *v > 0);
        }
        offset = end;
        if kind == b"IEND" {
            break;
        }
    }
    if width == 0 || height == 0 {
        return Err(model_error("Worksheet PNG dimensions must be positive"));
    }
    Ok((width, height, ppm_x, ppm_y, work))
}
fn bitmap_extent(size: u32, scale: f64, ppm: Option<u32>) -> Result<i64, Error> {
    if !scale.is_finite() {
        return Err(model_error("Worksheet bitmap scale must be finite"));
    }
    let ppi = ppm
        .map(|value| (value as f64 * 0.0254).round_ties_even() as i64)
        .filter(|value| *value > 0);
    let mm = if let Some(ppi) = ppi {
        size as f64 * scale * 25.4 / ppi as f64
    } else {
        size as f64 * scale * 25.4 / BITMAP_DEFAULT_DPI
    };
    mm_to_nm(mm)
}
fn checked(current: usize, additional: usize, maximum: usize) -> Result<usize, Error> {
    current
        .checked_add(additional)
        .filter(|value| *value <= maximum)
        .ok_or_else(limit_error)
}
