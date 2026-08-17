//! Deterministic Rust projection from TypeSpec-generated JSON Schemas.

use anyhow::{Context, Result, bail};
use schemars::schema::RootSchema;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use typify::{TypeSpace, TypeSpaceSettings};

const SCHEMAS: [(&str, &str); 27] = [
    ("BoardPlotDocument.json", "board_plot_document.rs"),
    ("BoardPlotRequest.json", "board_plot_request.rs"),
    ("BoardPlotResult.json", "board_plot_result.rs"),
    ("BuildRequest.json", "build_request.rs"),
    ("BuildResult.json", "build_result.rs"),
    ("ScanRequest.json", "scan_request.rs"),
    ("ScanResult.json", "scan_result.rs"),
    ("FootprintEditRequest.json", "footprint_edit_request.rs"),
    ("FootprintEditResult.json", "footprint_edit_result.rs"),
    ("FootprintReadRequest.json", "footprint_read_request.rs"),
    ("FootprintReadResult.json", "footprint_read_result.rs"),
    ("FootprintPlotDocument.json", "footprint_plot_document.rs"),
    ("FootprintPlotRequest.json", "footprint_plot_request.rs"),
    ("FootprintPlotResult.json", "footprint_plot_result.rs"),
    ("SymbolPlotDocument.json", "symbol_plot_document.rs"),
    ("SymbolPlotRequest.json", "symbol_plot_request.rs"),
    ("SymbolPlotResult.json", "symbol_plot_result.rs"),
    (
        "SymbolLibraryEditRequest.json",
        "symbol_library_edit_request.rs",
    ),
    (
        "SymbolLibraryEditResult.json",
        "symbol_library_edit_result.rs",
    ),
    (
        "SymbolLibraryReadRequest.json",
        "symbol_library_read_request.rs",
    ),
    (
        "SymbolLibraryReadResult.json",
        "symbol_library_read_result.rs",
    ),
    ("CompiledSchematicGraph.json", "compiled_schematic_graph.rs"),
    ("SourceBundleManifest.json", "source_bundle_manifest.rs"),
    ("FontBundleManifest.json", "font_bundle_manifest.rs"),
    ("FontResolutionRequest.json", "font_resolution_request.rs"),
    ("ShapingRecord.json", "shaping_record.rs"),
    ("OutlineVector.json", "outline_vector.rs"),
];

const PLOTTER_OPERATION_KINDS: [(&str, &str, &str); 13] = [
    (
        "ThickSegmentOperation",
        "ThickSegment",
        "deserialize_thick_segment_kind",
    ),
    (
        "ArcThreePointOperation",
        "ArcThreePoint",
        "deserialize_arc_three_point_kind",
    ),
    ("CircleOperation", "Circle", "deserialize_circle_kind"),
    ("RectOperation", "Rect", "deserialize_rect_kind"),
    (
        "PlotPolyOperation",
        "PlotPoly",
        "deserialize_plot_poly_kind",
    ),
    (
        "BezierCurveOperation",
        "BezierCurve",
        "deserialize_bezier_curve_kind",
    ),
    ("TextOperation", "Text", "deserialize_text_kind"),
    (
        "FlashPadCircleOperation",
        "FlashPadCircle",
        "deserialize_flash_pad_circle_kind",
    ),
    (
        "FlashPadOvalOperation",
        "FlashPadOval",
        "deserialize_flash_pad_oval_kind",
    ),
    (
        "FlashPadRectOperation",
        "FlashPadRect",
        "deserialize_flash_pad_rect_kind",
    ),
    (
        "FlashPadRoundRectOperation",
        "FlashPadRoundRect",
        "deserialize_flash_pad_round_rect_kind",
    ),
    (
        "FlashPadCustomOperation",
        "FlashPadCustom",
        "deserialize_flash_pad_custom_kind",
    ),
    (
        "FlashPadTrapezOperation",
        "FlashPadTrapez",
        "deserialize_flash_pad_trapez_kind",
    ),
];

fn main() -> Result<()> {
    let check = env::args().skip(1).any(|argument| argument == "--check");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let schema_root = root.join("contracts/generated/schema");
    let output_root = root.join("src/rs/kicad-monkey-contracts/src/generated");
    let mut expected = BTreeMap::new();
    let mut modules = Vec::new();

    for (schema_name, output_name) in SCHEMAS {
        let schema_path = schema_root.join(schema_name);
        let mut schema: Value = serde_json::from_slice(
            &fs::read(&schema_path).with_context(|| format!("read {}", schema_path.display()))?,
        )?;
        validate_plotter_operation_kinds(schema_name, &schema)?;
        project_for_typify(&mut schema);
        promote_disjoint_record_unions(&mut schema);
        project_tri_state_via_drill_layers(&mut schema);
        let generated = project_generated_presence(schema_name, generate(schema)?)?;
        expected.insert(output_root.join(output_name), generated);
        modules.push(output_name.trim_end_matches(".rs"));
    }
    modules.sort_unstable();
    let module_source = format!(
        "//! TypeSpec-generated modules. Regenerate; do not edit module contents.\n\n{}\n",
        modules
            .into_iter()
            .map(|module| format!("pub mod {module};"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    expected.insert(output_root.join("mod.rs"), module_source);

    for (path, content) in expected {
        if check {
            let current = fs::read_to_string(&path)
                .with_context(|| format!("missing generated Rust file {}", path.display()))?;
            if current != content {
                bail!("stale generated Rust file {}", path.display());
            }
        } else {
            fs::write(&path, content).with_context(|| format!("write {}", path.display()))?;
        }
    }
    Ok(())
}

fn validate_plotter_operation_kinds(schema_name: &str, schema: &Value) -> Result<()> {
    if !matches!(
        schema_name,
        "FootprintPlotDocument.json" | "SymbolPlotDocument.json"
    ) {
        return Ok(());
    }
    let members = schema
        .pointer("/$defs/PlotterOperation/anyOf")
        .and_then(Value::as_array)
        .with_context(|| format!("missing {schema_name} PlotterOperation union"))?;
    let mut actual = BTreeMap::new();
    for member in members {
        let reference = member
            .get("$ref")
            .and_then(Value::as_str)
            .with_context(|| format!("{schema_name} PlotterOperation member is not a reference"))?;
        let structure = reference
            .strip_prefix("#/$defs/")
            .with_context(|| format!("{schema_name} PlotterOperation reference leaves $defs"))?;
        let kind = schema
            .pointer(&format!("/$defs/{structure}/properties/kind/const"))
            .and_then(Value::as_str)
            .with_context(|| format!("missing literal kind for {structure}"))?;
        if actual.insert(structure, kind).is_some() {
            bail!("duplicate {schema_name} PlotterOperation member {structure}");
        }
    }
    let expected = PLOTTER_OPERATION_KINDS
        .iter()
        .map(|(structure, kind, _)| (*structure, *kind))
        .collect::<BTreeMap<_, _>>();
    if actual != expected {
        bail!("{schema_name} PlotterOperation union changed; update exact-kind projection");
    }
    Ok(())
}

fn project_generated_presence(schema_name: &str, source: String) -> Result<String> {
    if !matches!(
        schema_name,
        "FootprintPlotDocument.json" | "SymbolPlotDocument.json"
    ) {
        return Ok(source);
    }
    let original = r#"    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,"#;
    let replacement = r#"    #[serde(
        default,
        deserialize_with = "crate::reject_present_render_cache_polygons",
        skip_serializing_if = "::std::vec::Vec::is_empty"
    )]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,"#;
    if !source.contains(original) {
        bail!("{schema_name} Text render-cache polygon projection changed");
    }
    let mut projected = source.replace(original, replacement);
    for (structure, _, deserializer) in PLOTTER_OPERATION_KINDS {
        projected = project_kind_deserializer(projected, structure, deserializer)?;
    }
    Ok(projected)
}

fn project_kind_deserializer(
    mut source: String,
    structure: &str,
    deserializer: &str,
) -> Result<String> {
    let structure_marker = format!("pub struct {structure} {{");
    let structure_start = source
        .find(&structure_marker)
        .with_context(|| format!("missing generated {structure}"))?;
    let structure_end = source[structure_start..]
        .find("\n}")
        .map(|offset| structure_start + offset)
        .with_context(|| format!("unterminated generated {structure}"))?;
    let field = "    pub kind: ::std::string::String,";
    let field_offset = source[structure_start..structure_end]
        .find(field)
        .map(|offset| structure_start + offset)
        .with_context(|| format!("missing generated {structure}.kind"))?;
    let replacement =
        format!("    #[serde(deserialize_with = \"crate::{deserializer}\")]\n{field}");
    source.replace_range(field_offset..field_offset + field.len(), &replacement);
    Ok(source)
}

fn generate(value: Value) -> Result<String> {
    let schema: RootSchema = serde_json::from_value(value)?;
    let mut settings = TypeSpaceSettings::default();
    settings.with_struct_builder(false);
    settings.with_replacement(
        "JavaScriptSafeInteger",
        "crate::JavaScriptSafeInteger",
        [].into_iter(),
    );
    settings.with_replacement(
        "TextSafeInteger",
        "crate::JavaScriptSafeInteger",
        [].into_iter(),
    );
    settings.with_replacement(
        "NonNegativeFiniteFloat",
        "crate::NonNegativeFiniteFloat",
        [].into_iter(),
    );
    settings.with_replacement("FiniteFloat", "crate::FiniteFloat", [].into_iter());
    settings.with_replacement("PositiveUint32", "crate::PositiveU32", [].into_iter());
    settings.with_replacement("StableTextId", "crate::StableTextId", [].into_iter());
    settings.with_replacement("NonEmptyText", "::std::string::String", [].into_iter());
    let mut type_space = TypeSpace::new(&settings);
    type_space.add_root_schema(schema)?;
    let body = type_space.to_stream().to_string();
    let source =
        format!("// Generated from TypeSpec JSON Schema through typify. Do not edit.\n\n{body}\n");
    let syntax = syn::parse_file(&source).context("parse generated Rust")?;
    rustfmt(&prettyplease::unparse(&syntax))
}

fn rustfmt(source: &str) -> Result<String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("start rustfmt for generated contract")?;
    child
        .stdin
        .take()
        .context("open rustfmt stdin")?
        .write_all(source.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!("rustfmt rejected generated contract source");
    }
    String::from_utf8(output.stdout).context("rustfmt output was not UTF-8")
}

fn promote_disjoint_record_unions(schema: &mut Value) {
    // TypeSpec emits record unions as `anyOf`. The board record variants are
    // value-disjoint on their `kind` discriminators, but typify's structural
    // exclusivity check cannot see const/enum property values (graphic and
    // track_arc records share the same required property names), so it
    // degrades the union to an unusable `subtype_N` option struct. Carrying
    // the disjointness assertion as `oneOf` in the Rust projection yields a
    // proper enum; the published schema keeps `anyOf` alongside the other
    // record unions.
    if let Some(record) = schema
        .pointer_mut("/$defs/BoardPlotRecord")
        .and_then(Value::as_object_mut)
        && let Some(members) = record.remove("anyOf")
    {
        record.insert("oneOf".to_owned(), members);
    }
}

fn project_tri_state_via_drill_layers(schema: &mut Value) {
    // The established board serializer distinguishes an absent `layers` key
    // (layerless graphic circles) from a present-but-empty `layers` array
    // (drill circles of unrouted vias). typify folds optional arrays into
    // `Vec` with `skip_serializing_if(is_empty)`, which cannot express the
    // present-but-empty state, so the Rust projection widens the board
    // circle's `layers` to a nullable array and gains `Option<Vec<String>>`.
    // The published schema keeps the plain optional array.
    if schema.pointer("/$defs/BoardPlotRecord").is_some()
        && let Some(layers) = schema
            .pointer_mut("/$defs/CircleOperation/properties/layers")
            .and_then(Value::as_object_mut)
    {
        layers.insert("type".to_owned(), serde_json::json!(["array", "null"]));
    }
}

fn project_for_typify(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("$schema");
            if matches!(
                object.get("pattern").and_then(Value::as_str),
                Some("^(0|[1-9][0-9]{0,19})$")
                    | Some("^[0-9a-f]{64}$")
                    | Some("^[ -~]{4}$")
                    | Some("^[A-Za-z0-9][A-Za-z0-9._:-]*$")
            ) {
                // JSON Schema and the promoted semantic validators retain
                // the closed string grammars. Avoid adding a regex runtime to
                // generated Rust merely for small fixed tags and hashes.
                object.remove("pattern");
            }
            if object
                .get("unevaluatedProperties")
                .is_some_and(|entry| entry == &serde_json::json!({"not": {}}))
            {
                object.remove("unevaluatedProperties");
                object.insert("additionalProperties".to_owned(), Value::Bool(false));
            }
            for child in object.values_mut() {
                project_for_typify(child);
            }
        }
        Value::Array(values) => {
            for child in values {
                project_for_typify(child);
            }
        }
        _ => {}
    }
}
