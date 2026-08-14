use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    TextBlockLayoutLimits, TextBlockLayoutRequest, TextHorizontalAlignment, TextRenderCacheLimits,
    TextVerticalAlignment, generate_text_render_cache_block_hinted_a0, write_text_render_cache_a0,
};
use serde::Deserialize;
use std::{env, fs, process::ExitCode};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GateRequest {
    shaping: ShapingInput,
    size_x: f64,
    size_y: f64,
    position_x: f64,
    position_y: f64,
    angle_degrees: f64,
    mirrored: bool,
    horizontal_alignment: String,
    vertical_alignment: String,
    #[serde(default = "default_line_spacing")]
    line_spacing: f64,
    max_error: f64,
}

fn default_line_spacing() -> f64 {
    1.0
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
    let font_path = arguments
        .next()
        .ok_or_else(|| "usage: text_render_cache_oracle_gate FONT REQUEST_JSON".to_owned())?;
    let request_path = arguments
        .next()
        .ok_or_else(|| "usage: text_render_cache_oracle_gate FONT REQUEST_JSON".to_owned())?;
    if arguments.next().is_some() {
        return Err("usage: text_render_cache_oracle_gate FONT REQUEST_JSON".to_owned());
    }
    let font_bytes = fs::read(font_path).map_err(|error| format!("read font: {error}"))?;
    let request_bytes = fs::read(request_path).map_err(|error| format!("read request: {error}"))?;
    let request: GateRequest = serde_json::from_slice(&request_bytes)
        .map_err(|error| format!("parse request: {error}"))?;
    let horizontal_alignment = match request.horizontal_alignment.as_str() {
        "left" => TextHorizontalAlignment::Left,
        "center" => TextHorizontalAlignment::Center,
        "right" => TextHorizontalAlignment::Right,
        value => return Err(format!("unknown horizontal alignment: {value}")),
    };
    let vertical_alignment = match request.vertical_alignment.as_str() {
        "top" => TextVerticalAlignment::Top,
        "center" => TextVerticalAlignment::Center,
        "bottom" => TextVerticalAlignment::Bottom,
        value => return Err(format!("unknown vertical alignment: {value}")),
    };
    let cache = generate_text_render_cache_block_hinted_a0(
        &font_bytes,
        TextBlockLayoutRequest {
            shaping: &request.shaping,
            size_x: request.size_x,
            size_y: request.size_y,
            position_x: request.position_x,
            position_y: request.position_y,
            angle_degrees: request.angle_degrees,
            mirrored: request.mirrored,
            horizontal_alignment,
            vertical_alignment,
            line_spacing: request.line_spacing,
            max_error: request.max_error,
        },
        TextBlockLayoutLimits::default(),
        TextRenderCacheLimits::default(),
    )
    .map_err(|error| format!("generate cache: {error}"))?;
    write_text_render_cache_a0(&cache, TextRenderCacheLimits::default())
        .map_err(|error| format!("write cache: {error}"))
}
