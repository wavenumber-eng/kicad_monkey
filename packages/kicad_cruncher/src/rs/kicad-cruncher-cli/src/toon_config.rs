//! Editable Toon preset loading over the Cruncher PCB SVG config contract.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::ToonTheme;
use crate::pcb_svg::generated::PcbSvgConfig;
use crate::pcb_svg::preview::ToonStyleSettings;

pub const TOON_CONFIG_FILENAME: &str = "toon.config";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedToonColors {
    pub soldermask: String,
    pub silkscreen: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedToonView {
    pub name: String,
    pub layers: Vec<String>,
    pub colors: ResolvedToonColors,
    pub style: ToonStyleSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedToonViews {
    pub top: ResolvedToonView,
    pub bottom: ResolvedToonView,
}

pub fn default_toon_config() -> Value {
    json!({
        "schema": "kicad_cruncher.pcb_svg.config.a0",
        "global": {
            "include_metadata": true,
            "styles": {
                "board_substrate": {"enabled": true, "color": "#B6A26B", "opacity": 1.0},
                "board_outline": {"enabled": true, "color": "#000000", "line_width_mm": 0.25},
                "soldermask_film": {"enabled": true, "color": "auto", "opacity": 0.75},
                "board_cutouts": {
                    "enabled": true,
                    "color": "#555555",
                    "outline_width_mm": 0.24,
                    "hatch": true,
                    "hatch_spacing_mm": 1.0,
                    "hatch_angle_deg": 45.0,
                    "hatch_line_width_mm": 0.12,
                    "hatch_color": "#777777",
                    "hatch_opacity": 0.18,
                    "outline_opacity": 0.25
                },
                "smd_pads": {"enabled": true, "color": "#DFC951"},
                "through_hole_pads": {"enabled": true, "color": "#DFC951"},
                "copper_traces": {"enabled": true, "color": "#DFC951"},
                "vias": {"enabled": true, "color": "#DFC951"},
                "copper_polygons": {"enabled": true, "color": "#DFC951"},
                "drills": {
                    "enabled": true,
                    "plated_color": "#D3D3D3",
                    "non_plated_color": "#D3D3D3",
                    "opacity": 1.0,
                    "outline": false,
                    "outline_width_mm": 0.1,
                    "respect_tenting": true
                },
                "slots": {
                    "enabled": true,
                    "plated_color": "#D3D3D3",
                    "non_plated_color": "#D3D3D3",
                    "opacity": 1.0,
                    "outline": false,
                    "outline_width_mm": 0.1,
                    "respect_tenting": true
                },
                "silkscreen_component_graphics": {"enabled": true, "color": "#F5F5F5"},
                "silkscreen_designators": {"enabled": true, "color": "#F5F5F5"},
                "silkscreen_board_graphics": {"enabled": true, "color": "#F5F5F5"},
                "illustration": {
                    "enabled": true,
                    "opacity": 1.0,
                    "outline_width_mm": 0.025,
                    "detail_width_mm": 0.01375
                },
                "assembly_designators": {
                    "enabled": true,
                    "color": "#FF0000",
                    "box_fill_ratio": 0.8,
                    "max_font_size_mm": 2.5,
                    "min_font_size_mm": 0.35,
                    "font_family": "Arial, sans-serif",
                    "font_weight": "700",
                    "opacity": 1.0,
                    "stroke_color": "#FFFFFF",
                    "stroke_width_mm": 0.0
                }
            }
        },
        "layer_outputs": {"enabled": false},
        "views": [
            {
                "name": "top",
                "enabled": true,
                "mirror": false,
                "group_id": "toon-top",
                "output_svg": "{board}__{view}.svg",
                "layers": [
                    "BOARD_SUBSTRATE", "F.Cu", "SOLDERMASK_FILM_TOP", "F.SilkS",
                    "BOARD_CUTOUTS", "DRILLS", "SLOTS", "BOARD_OUTLINE", "ILLUSTRATION_TOP"
                ]
            },
            {
                "name": "bottom",
                "enabled": true,
                "mirror": true,
                "group_id": "toon-bottom",
                "output_svg": "{board}__{view}.svg",
                "layers": [
                    "BOARD_SUBSTRATE", "B.Cu", "SOLDERMASK_FILM_BOTTOM", "B.SilkS",
                    "BOARD_CUTOUTS", "DRILLS", "SLOTS", "BOARD_OUTLINE", "ILLUSTRATION_BOTTOM"
                ]
            }
        ],
        "assembly": {
            "default_projection": "detail",
            "dnp_projection": "bounding_box",
            "designator_color": "#FF0000",
            "dnp_designator_color": "#808080"
        }
    })
}

pub fn resolve_toon_config(
    config_path: Option<&Path>,
    theme: Option<ToonTheme>,
    custom_soldermask: Option<&str>,
) -> Result<Value, String> {
    let mut resolved = default_toon_config();
    if let Some(path) = config_path.filter(|path| path.exists()) {
        let authored = load_jsonc(path)?;
        overlay(&mut resolved, authored);
    }
    if let Some(theme) = theme {
        let (mask, silk) = theme.colors();
        apply_colors(&mut resolved, mask, silk)?;
    } else if let Some(mask) = custom_soldermask {
        apply_colors(&mut resolved, mask, contrast_for_custom(mask))?;
    }
    serde_json::from_value::<PcbSvgConfig>(resolved.clone())
        .map_err(|error| format!("invalid PCB SVG config: {error}"))?;
    resolved_views(&resolved)?;
    Ok(resolved)
}

pub fn resolved_views(config: &Value) -> Result<ResolvedToonViews, String> {
    Ok(ResolvedToonViews {
        top: resolved_view(config, "top")?,
        bottom: resolved_view(config, "bottom")?,
    })
}

fn resolved_view(config: &Value, side: &str) -> Result<ResolvedToonView, String> {
    let views = config
        .get("views")
        .and_then(Value::as_array)
        .ok_or_else(|| "Toon config views must be an array".to_owned())?;
    let enabled = views
        .iter()
        .filter(|view| view.get("enabled").and_then(Value::as_bool) != Some(false))
        .collect::<Vec<_>>();
    let named = enabled
        .iter()
        .copied()
        .filter(|view| {
            view.get("name")
                .and_then(Value::as_str)
                .is_some_and(|name| name.eq_ignore_ascii_case(side))
        })
        .collect::<Vec<_>>();
    let matching = if named.is_empty() {
        enabled
            .into_iter()
            .filter(|view| view_layers_identify_side(view, side))
            .collect::<Vec<_>>()
    } else {
        named
    };
    let view = match matching.as_slice() {
        [view] => *view,
        [] => {
            return Err(format!("Toon config requires one enabled {side} view"));
        }
        _ => {
            return Err(format!("Toon config has more than one enabled {side} view"));
        }
    };
    let name = view
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Toon {side} view name must be a string"))?
        .to_owned();
    let layers = view
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Toon view {name:?} layers must be an array"))?
        .iter()
        .map(|layer| {
            layer
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| format!("Toon view {name:?} layer names must be strings"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut effective = config.clone();
    let mut styles = config
        .pointer("/global/styles")
        .cloned()
        .ok_or_else(|| "Toon config global.styles must exist".to_owned())?;
    if let Some(overrides) = view.get("styles") {
        if !overrides.is_object() {
            return Err(format!("Toon view {name:?} styles must be an object"));
        }
        overlay(&mut styles, overrides.clone());
    }
    *effective
        .pointer_mut("/global/styles")
        .ok_or_else(|| "Toon config global.styles must exist".to_owned())? = styles;
    Ok(ResolvedToonView {
        name,
        layers,
        colors: resolved_colors(&effective)?,
        style: resolved_style(&effective)?,
    })
}

fn view_layers_identify_side(view: &Value, side: &str) -> bool {
    let side_tokens: &[&str] = match side {
        "top" => &[
            "F.Cu",
            "TOP",
            "SOLDERMASK_FILM_TOP",
            "F.SilkS",
            "TOPOVERLAY",
            "ILLUSTRATION_TOP",
        ],
        "bottom" => &[
            "B.Cu",
            "BOTTOM",
            "SOLDERMASK_FILM_BOTTOM",
            "B.SilkS",
            "BOTTOMOVERLAY",
            "ILLUSTRATION_BOTTOM",
        ],
        _ => return false,
    };
    view.get("layers")
        .and_then(Value::as_array)
        .is_some_and(|layers| {
            layers.iter().any(|layer| {
                layer
                    .as_str()
                    .is_some_and(|layer| side_tokens.contains(&layer))
            })
        })
}

pub fn resolved_colors(config: &Value) -> Result<ResolvedToonColors, String> {
    let styles = config
        .pointer("/global/styles")
        .and_then(Value::as_object)
        .ok_or_else(|| "Toon config global.styles must be an object".to_owned())?;
    let soldermask = style_color(styles, "soldermask_film")?;
    let silkscreen = style_color(styles, "silkscreen_board_graphics")?;
    for style in ["silkscreen_component_graphics", "silkscreen_designators"] {
        let color = style_color(styles, style)?;
        if color != silkscreen {
            return Err(format!(
                "Toon currently requires all silkscreen style colors to match; {style} is {color} but silkscreen_board_graphics is {silkscreen}"
            ));
        }
    }
    Ok(ResolvedToonColors {
        soldermask,
        silkscreen,
    })
}

pub fn resolved_style(config: &Value) -> Result<ToonStyleSettings, String> {
    Ok(ToonStyleSettings {
        substrate_color: string_at(config, "/global/styles/board_substrate/color")?,
        substrate_opacity: number_at(config, "/global/styles/board_substrate/opacity")?,
        soldermask_opacity: number_at(config, "/global/styles/soldermask_film/opacity")?,
        copper_color: unified_copper_color(config)?,
        board_outline_color: string_at(config, "/global/styles/board_outline/color")?,
        board_outline_width_mm: number_at(config, "/global/styles/board_outline/line_width_mm")?,
        cutout_outline_color: string_at(config, "/global/styles/board_cutouts/color")?,
        cutout_outline_width_mm: number_at(
            config,
            "/global/styles/board_cutouts/outline_width_mm",
        )?,
        cutout_outline_opacity: number_at(config, "/global/styles/board_cutouts/outline_opacity")?,
        cutout_hatch_color: string_at(config, "/global/styles/board_cutouts/hatch_color")?,
        cutout_hatch_spacing_mm: number_at(
            config,
            "/global/styles/board_cutouts/hatch_spacing_mm",
        )?,
        cutout_hatch_angle_deg: number_at(config, "/global/styles/board_cutouts/hatch_angle_deg")?,
        cutout_hatch_line_width_mm: number_at(
            config,
            "/global/styles/board_cutouts/hatch_line_width_mm",
        )?,
        cutout_hatch_opacity: number_at(config, "/global/styles/board_cutouts/hatch_opacity")?,
        plated_drill_color: string_at(config, "/global/styles/drills/plated_color")?,
        non_plated_drill_color: string_at(config, "/global/styles/drills/non_plated_color")?,
        drill_opacity: number_at(config, "/global/styles/drills/opacity")?,
        drill_outline: bool_at(config, "/global/styles/drills/outline")?,
        drill_outline_width_mm: number_at(config, "/global/styles/drills/outline_width_mm")?,
        drill_respect_tenting: bool_at(config, "/global/styles/drills/respect_tenting")?,
        plated_slot_color: string_at(config, "/global/styles/slots/plated_color")?,
        non_plated_slot_color: string_at(config, "/global/styles/slots/non_plated_color")?,
        slot_opacity: number_at(config, "/global/styles/slots/opacity")?,
        slot_outline: bool_at(config, "/global/styles/slots/outline")?,
        slot_outline_width_mm: number_at(config, "/global/styles/slots/outline_width_mm")?,
        slot_respect_tenting: bool_at(config, "/global/styles/slots/respect_tenting")?,
        illustration_opacity: number_at(config, "/global/styles/illustration/opacity")?,
        illustration_outline_width_mm: number_at(
            config,
            "/global/styles/illustration/outline_width_mm",
        )?,
        illustration_detail_width_mm: number_at(
            config,
            "/global/styles/illustration/detail_width_mm",
        )?,
        assembly_designator_color: string_at(config, "/global/styles/assembly_designators/color")?,
        assembly_designator_fill_ratio: config
            .pointer("/global/styles/assembly_designators/fill_ratio")
            .and_then(Value::as_f64)
            .map(Ok)
            .unwrap_or_else(|| {
                number_at(config, "/global/styles/assembly_designators/box_fill_ratio")
            })?,
        assembly_designator_max_font_size_mm: number_at(
            config,
            "/global/styles/assembly_designators/max_font_size_mm",
        )?,
        assembly_designator_min_font_size_mm: number_at(
            config,
            "/global/styles/assembly_designators/min_font_size_mm",
        )?,
        assembly_designator_font_family: string_at(
            config,
            "/global/styles/assembly_designators/font_family",
        )?,
        assembly_designator_font_weight: string_or_number_at(
            config,
            "/global/styles/assembly_designators/font_weight",
        )?,
        assembly_designator_opacity: number_at(
            config,
            "/global/styles/assembly_designators/opacity",
        )?,
        assembly_designator_stroke_color: string_at(
            config,
            "/global/styles/assembly_designators/stroke_color",
        )?,
        assembly_designator_stroke_width_mm: number_at(
            config,
            "/global/styles/assembly_designators/stroke_width_mm",
        )?,
        assembly_hidden_designators: hidden_designators(config)?,
    })
}

fn hidden_designators(config: &Value) -> Result<BTreeSet<String>, String> {
    let Some(components) = config.get("components") else {
        return Ok(BTreeSet::new());
    };
    let components = components
        .as_object()
        .ok_or_else(|| "Toon config components must be an object".to_owned())?;
    Ok(components
        .iter()
        .filter(|(_, override_value)| {
            override_value
                .get("show_designator")
                .and_then(Value::as_bool)
                == Some(false)
        })
        .map(|(reference, _)| reference.clone())
        .collect())
}

pub fn config_text(config: &Value) -> Result<String, String> {
    let body = serde_json::to_string_pretty(config)
        .map_err(|error| format!("could not serialize Toon config: {error}"))?;
    Ok(format!(
        "// Editable KiCad Cruncher Toon preset. JSONC comments and trailing commas are accepted.\n// Explicit --theme or --soldermask-color overrides the saved style colors.\n{body}\n"
    ))
}

pub fn write_config(path: &Path, config: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("config path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create config directory: {error}"))?;
    fs::write(path, config_text(config)?)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn style_color(styles: &Map<String, Value>, name: &str) -> Result<String, String> {
    styles
        .get(name)
        .and_then(Value::as_object)
        .and_then(|style| style.get("color"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("Toon config global.styles.{name}.color must be a string"))
}

fn unified_copper_color(config: &Value) -> Result<String, String> {
    let paths = [
        "/global/styles/smd_pads/color",
        "/global/styles/through_hole_pads/color",
        "/global/styles/copper_traces/color",
        "/global/styles/vias/color",
        "/global/styles/copper_polygons/color",
    ];
    let color = string_at(config, paths[0])?;
    for path in &paths[1..] {
        let candidate = string_at(config, path)?;
        if candidate != color {
            return Err(format!(
                "Toon currently requires all copper style colors to match; {path} is {candidate} but {} is {color}",
                paths[0]
            ));
        }
    }
    Ok(color)
}

fn string_at(config: &Value, pointer: &str) -> Result<String, String> {
    config
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("Toon config {pointer} must be a string"))
}

fn number_at(config: &Value, pointer: &str) -> Result<f64, String> {
    config
        .pointer(pointer)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("Toon config {pointer} must be a number"))
}

fn string_or_number_at(config: &Value, pointer: &str) -> Result<String, String> {
    let value = config
        .pointer(pointer)
        .ok_or_else(|| format!("Toon config {pointer} must exist"))?;
    match value {
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) => Ok(value.to_string()),
        _ => Err(format!("Toon config {pointer} must be a string or number")),
    }
}

fn bool_at(config: &Value, pointer: &str) -> Result<bool, String> {
    config
        .pointer(pointer)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Toon config {pointer} must be a boolean"))
}

fn apply_colors(config: &mut Value, mask: &str, silk: &str) -> Result<(), String> {
    {
        let styles = config
            .pointer_mut("/global/styles")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| "Toon config global.styles must be an object".to_owned())?;
        set_palette_colors(styles, mask, silk, false)?;
    }
    let views = config
        .get_mut("views")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "Toon config views must be an array".to_owned())?;
    for view in views {
        let view = view
            .as_object_mut()
            .ok_or_else(|| "Toon config view entries must be objects".to_owned())?;
        let styles = view
            .entry("styles")
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .ok_or_else(|| "Toon config view styles must be an object".to_owned())?;
        set_palette_colors(styles, mask, silk, true)?;
    }
    Ok(())
}

fn set_palette_colors(
    styles: &mut Map<String, Value>,
    mask: &str,
    silk: &str,
    create_missing: bool,
) -> Result<(), String> {
    set_style_color(styles, "soldermask_film", mask, create_missing)?;
    for name in [
        "silkscreen_component_graphics",
        "silkscreen_designators",
        "silkscreen_board_graphics",
    ] {
        set_style_color(styles, name, silk, create_missing)?;
    }
    Ok(())
}

fn set_style_color(
    styles: &mut Map<String, Value>,
    name: &str,
    color: &str,
    create_missing: bool,
) -> Result<(), String> {
    let style = if create_missing {
        styles
            .entry(name.to_owned())
            .or_insert_with(|| Value::Object(Map::new()))
    } else {
        styles
            .get_mut(name)
            .ok_or_else(|| format!("Toon config global.styles.{name} must exist"))?
    };
    style
        .as_object_mut()
        .ok_or_else(|| format!("Toon config global.styles.{name} must be an object"))?
        .insert("color".to_owned(), Value::String(color.to_owned()));
    Ok(())
}

fn contrast_for_custom(mask: &str) -> &'static str {
    if mask.eq_ignore_ascii_case("#EEEEEE") {
        "#000000"
    } else {
        "#F5F5F5"
    }
}

fn overlay(base: &mut Value, override_value: Value) {
    match (base, override_value) {
        (Value::Object(base), Value::Object(override_object)) => {
            for (key, value) in override_object {
                match base.get_mut(&key) {
                    Some(existing) => overlay(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, value) => *base = value,
    }
}

fn load_jsonc(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let stripped = strip_jsonc(&text)?;
    let value: Value = serde_json::from_str(&stripped)
        .map_err(|error| format!("could not parse {} as JSON/JSONC: {error}", path.display()))?;
    if !value.is_object() {
        return Err(format!("{} must contain a JSON object", path.display()));
    }
    Ok(value)
}

#[allow(
    clippy::cognitive_complexity,
    reason = "the two small byte-state passes preserve strings while removing comments and trailing commas"
)]
fn strip_jsonc(text: &str) -> Result<String, String> {
    let chars = text.as_bytes();
    let mut output = Vec::with_capacity(text.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < chars.len() {
        let byte = chars[index];
        if in_string {
            output.push(byte);
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if byte == b'"' {
            in_string = true;
            output.push(b'"');
            index += 1;
        } else if byte == b'/' && chars.get(index + 1) == Some(&b'/') {
            index += 2;
            while index < chars.len() && !matches!(chars[index], b'\r' | b'\n') {
                index += 1;
            }
        } else if byte == b'/' && chars.get(index + 1) == Some(&b'*') {
            let start = index;
            index += 2;
            while index + 1 < chars.len() && !(chars[index] == b'*' && chars[index + 1] == b'/') {
                index += 1;
            }
            if index + 1 >= chars.len() {
                return Err(format!("unterminated JSONC block comment at byte {start}"));
            }
            index += 2;
        } else {
            output.push(byte);
            index += 1;
        }
    }
    if in_string {
        return Err("unterminated JSON string".to_owned());
    }
    let mut without_trailing_commas = Vec::with_capacity(output.len());
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < output.len() {
        let byte = output[index];
        if in_string {
            without_trailing_commas.push(byte);
            index += 1;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        if byte == b'"' {
            in_string = true;
        } else if byte == b',' {
            let mut lookahead = index + 1;
            while lookahead < output.len() && output[lookahead].is_ascii_whitespace() {
                lookahead += 1;
            }
            if matches!(output.get(lookahead), Some(b'}' | b']')) {
                index += 1;
                continue;
            }
        }
        without_trailing_commas.push(byte);
        index += 1;
    }
    String::from_utf8(without_trailing_commas)
        .map_err(|error| format!("invalid UTF-8 after JSONC parsing: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc_overlay_then_explicit_theme_matches_altium_precedence() {
        let mut config = default_toon_config();
        overlay(
            &mut config,
            serde_json::from_str(
                r##"{"global":{"styles":{"soldermask_film":{"color":"#123456"}}}}"##,
            )
            .unwrap(),
        );
        assert_eq!(resolved_colors(&config).unwrap().soldermask, "#123456");
        apply_colors(&mut config, "#EEEEEE", "#000000").unwrap();
        assert_eq!(
            resolved_colors(&config).unwrap(),
            ResolvedToonColors {
                soldermask: "#EEEEEE".to_owned(),
                silkscreen: "#000000".to_owned(),
            }
        );
    }

    #[test]
    fn jsonc_strip_preserves_comment_markers_in_strings_and_trailing_commas() {
        let text = r##"{
          // line
          "url": "https://example.test/a/*b*/",
          "styles": {"color": "#123456",}, /* block */
        }"##;
        let parsed: Value = serde_json::from_str(&strip_jsonc(text).unwrap()).unwrap();
        assert_eq!(parsed["url"], "https://example.test/a/*b*/");
        assert_eq!(parsed["styles"]["color"], "#123456");
    }

    #[test]
    fn view_styles_override_global_styles_independently() {
        let mut config = default_toon_config();
        config["views"][0]["styles"] = json!({
            "soldermask_film": {"color": "#123456", "opacity": 0.5},
            "illustration": {"opacity": 0.4}
        });

        let views = resolved_views(&config).expect("resolve view styles");
        assert_eq!(views.top.colors.soldermask, "#123456");
        assert_eq!(views.top.style.soldermask_opacity, 0.5);
        assert_eq!(views.top.style.illustration_opacity, 0.4);
        assert_eq!(views.bottom.colors.soldermask, "auto");
        assert_eq!(views.bottom.style.soldermask_opacity, 0.75);
        assert_eq!(views.bottom.style.illustration_opacity, 1.0);
    }

    #[test]
    fn explicit_theme_overrides_colors_in_every_view() {
        let mut config = default_toon_config();
        config["views"][0]["styles"] = json!({
            "soldermask_film": {"color": "#123456"}
        });
        config["views"][1]["styles"] = json!({
            "soldermask_film": {"color": "#654321"}
        });

        apply_colors(&mut config, "#000000", "#F5F5F5").unwrap();
        let views = resolved_views(&config).expect("resolve themed views");
        assert_eq!(views.top.colors.soldermask, "#000000");
        assert_eq!(views.bottom.colors.soldermask, "#000000");
    }

    #[test]
    fn named_views_may_omit_the_illustration_layer() {
        let mut config = default_toon_config();
        config["views"][0]["layers"] = json!(["BOARD_SUBSTRATE", "BOARD_OUTLINE"]);

        let views = resolved_views(&config).expect("resolve named view without models");
        assert_eq!(views.top.layers, vec!["BOARD_SUBSTRATE", "BOARD_OUTLINE"]);
    }

    #[test]
    fn component_override_can_hide_an_assembly_designator() {
        let mut config = default_toon_config();
        config["components"] = json!({
            "J1": {"show_designator": false},
            "J2": {"show_designator": true}
        });

        let views = resolved_views(&config).expect("resolve component overrides");
        assert!(views.top.style.assembly_hidden_designators.contains("J1"));
        assert!(!views.top.style.assembly_hidden_designators.contains("J2"));
        assert_eq!(
            views.top.style.assembly_hidden_designators,
            views.bottom.style.assembly_hidden_designators
        );
    }
}
