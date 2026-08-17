use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    BoardPlotLimits, BoardPlotRecord, BoardTableOperation, BoardTextBoxOperation,
    BoardTextVariables, PlotterTextCacheLimits, PlotterTextCacheResources, PlotterTextFont,
    TextContour, TextPoint, TextRenderCache, TextRenderCacheLimits, TextRenderCachePolygon,
    board_plot_document_with_text_cache_sidecar, write_text_render_cache_a0,
};
use serde::Deserialize;
use std::{env, fs, process::ExitCode};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GateFont {
    face: String,
    bold: bool,
    italic: bool,
    #[serde(default)]
    fake_bold: bool,
    #[serde(default)]
    fake_italic: bool,
    shaping: ShapingInput,
}

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            print!("{}", String::from_utf8(output).expect("writer emits UTF-8"));
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<Vec<u8>, String> {
    let mut arguments = env::args_os().skip(1);
    let font_path = arguments.next().ok_or_else(usage)?;
    let request_path = arguments.next().ok_or_else(usage)?;
    let board_path = arguments.next().ok_or_else(usage)?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    let font_bytes = fs::read(font_path).map_err(|error| format!("read font: {error}"))?;
    let request_bytes = fs::read(request_path).map_err(|error| format!("read request: {error}"))?;
    let board = fs::read_to_string(board_path).map_err(|error| format!("read board: {error}"))?;
    let request: GateFont = serde_json::from_slice(&request_bytes)
        .map_err(|error| format!("parse request: {error}"))?;
    let fonts = [PlotterTextFont {
        face: &request.face,
        bold: request.bold,
        italic: request.italic,
        font_bytes: &font_bytes,
        shaping: request.shaping,
        fake_bold: request.fake_bold,
        fake_italic: request.fake_italic,
    }];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let document = board_plot_document_with_text_cache_sidecar(
        &board,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .map_err(|error| format!("plot board: {error}"))?;
    let cache = document
        .records
        .iter()
        .find_map(|record| match record {
            BoardPlotRecord::Text(record) => record
                .operations
                .first()
                .and_then(|operation| operation.render_cache.as_ref()),
            BoardPlotRecord::TextBox(record) => {
                record
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        BoardTextBoxOperation::Text(text) => text.render_cache.as_ref(),
                        BoardTextBoxOperation::Border(_) => None,
                    })
            }
            BoardPlotRecord::Table(record) => {
                record
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        BoardTableOperation::Text(text) => text.render_cache.as_ref(),
                        BoardTableOperation::Segment(_) => None,
                    })
            }
            _ => None,
        })
        .ok_or_else(|| "board produced no text cache".to_owned())?;
    let cache = TextRenderCache {
        text: cache.text.clone(),
        angle_degrees: cache.angle,
        polygons: cache
            .polygons
            .iter()
            .map(|contours| TextRenderCachePolygon {
                contours: contours
                    .iter()
                    .map(|points| TextContour {
                        points: points
                            .iter()
                            .map(|[x, y]| TextPoint {
                                x: *x as f64 / 1_000_000.0,
                                y: *y as f64 / 1_000_000.0,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    };
    write_text_render_cache_a0(&cache, TextRenderCacheLimits::default())
        .map_err(|error| format!("write cache: {error}"))
}

fn usage() -> String {
    "usage: board_plot_text_cache_gate FONT REQUEST_JSON BOARD".to_owned()
}
