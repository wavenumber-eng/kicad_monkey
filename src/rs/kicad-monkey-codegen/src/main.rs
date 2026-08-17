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

const SCHEMAS: [(&str, &str); 30] = [
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
    ("SchematicPlotDocument.json", "schematic_plot_document.rs"),
    ("SchematicPlotRequest.json", "schematic_plot_request.rs"),
    ("SchematicPlotResult.json", "schematic_plot_result.rs"),
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

const PLOTTER_OPERATION_KINDS: [(&str, &str, &str); 14] = [
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
        "PlotImageOperation",
        "PlotImage",
        "deserialize_plot_image_kind",
    ),
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

const BOARD_FOOTPRINT_OPERATION_KINDS: [(&str, &str, &str); 15] = [
    (
        "BoardFootprintThickSegmentOperation",
        "ThickSegment",
        "deserialize_thick_segment_kind",
    ),
    (
        "BoardFootprintArcThreePointOperation",
        "ArcThreePoint",
        "deserialize_arc_three_point_kind",
    ),
    (
        "BoardFootprintCircleOperation",
        "Circle",
        "deserialize_circle_kind",
    ),
    (
        "BoardFootprintRectOperation",
        "Rect",
        "deserialize_rect_kind",
    ),
    (
        "BoardFootprintPlotPolyOperation",
        "PlotPoly",
        "deserialize_plot_poly_kind",
    ),
    (
        "BoardFootprintBezierCurveOperation",
        "BezierCurve",
        "deserialize_bezier_curve_kind",
    ),
    (
        "BoardFootprintTextOperation",
        "Text",
        "deserialize_text_kind",
    ),
    (
        "BoardFootprintFlashPadCircleOperation",
        "FlashPadCircle",
        "deserialize_flash_pad_circle_kind",
    ),
    (
        "BoardFootprintFlashPadOvalOperation",
        "FlashPadOval",
        "deserialize_flash_pad_oval_kind",
    ),
    (
        "BoardFootprintFlashPadRectOperation",
        "FlashPadRect",
        "deserialize_flash_pad_rect_kind",
    ),
    (
        "BoardFootprintFlashPadRoundRectOperation",
        "FlashPadRoundRect",
        "deserialize_flash_pad_round_rect_kind",
    ),
    (
        "BoardFootprintFlashPadCustomOperation",
        "FlashPadCustom",
        "deserialize_flash_pad_custom_kind",
    ),
    (
        "BoardFootprintFlashPadTrapezOperation",
        "FlashPadTrapez",
        "deserialize_flash_pad_trapez_kind",
    ),
    (
        "BoardFootprintStartBlockOperation",
        "StartBlock",
        "deserialize_start_block_kind",
    ),
    (
        "BoardFootprintEndBlockOperation",
        "EndBlock",
        "deserialize_end_block_kind",
    ),
];

const SCHEMATIC_RECORD_KINDS: [(&str, &str, &str); 22] = [
    (
        "SchematicSheetHeaderPlotRecord",
        "sheet_header",
        "deserialize_sheet_header_kind",
    ),
    (
        "SchematicWirePlotRecord",
        "wire",
        "deserialize_wire_record_kind",
    ),
    (
        "SchematicBusPlotRecord",
        "bus",
        "deserialize_bus_record_kind",
    ),
    (
        "SchematicBusEntryPlotRecord",
        "bus_entry",
        "deserialize_bus_entry_record_kind",
    ),
    (
        "SchematicJunctionPlotRecord",
        "junction",
        "deserialize_junction_record_kind",
    ),
    (
        "SchematicNoConnectPlotRecord",
        "no_connect",
        "deserialize_no_connect_record_kind",
    ),
    (
        "SchematicLabelPlotRecord",
        "label",
        "deserialize_label_record_kind",
    ),
    (
        "SchematicGlobalLabelPlotRecord",
        "global_label",
        "deserialize_global_label_record_kind",
    ),
    (
        "SchematicHierarchicalLabelPlotRecord",
        "hierarchical_label",
        "deserialize_hierarchical_label_record_kind",
    ),
    (
        "SchematicNetclassFlagPlotRecord",
        "netclass_flag",
        "deserialize_netclass_flag_record_kind",
    ),
    (
        "SchematicTextPlotRecord",
        "text",
        "deserialize_text_record_kind",
    ),
    (
        "SchematicTextBoxPlotRecord",
        "text_box",
        "deserialize_text_box_record_kind",
    ),
    (
        "SchematicGraphicPolylinePlotRecord",
        "graphic_polyline",
        "deserialize_graphic_polyline_record_kind",
    ),
    (
        "SchematicGraphicArcPlotRecord",
        "graphic_arc",
        "deserialize_graphic_arc_record_kind",
    ),
    (
        "SchematicGraphicCirclePlotRecord",
        "graphic_circle",
        "deserialize_graphic_circle_record_kind",
    ),
    (
        "SchematicGraphicRectanglePlotRecord",
        "graphic_rectangle",
        "deserialize_graphic_rectangle_record_kind",
    ),
    (
        "SchematicGraphicBezierPlotRecord",
        "graphic_bezier",
        "deserialize_graphic_bezier_record_kind",
    ),
    (
        "SchematicRuleAreaPlotRecord",
        "rule_area",
        "deserialize_rule_area_record_kind",
    ),
    (
        "SchematicImagePlotRecord",
        "image",
        "deserialize_image_record_kind",
    ),
    (
        "SchematicTablePlotRecord",
        "table",
        "deserialize_table_record_kind",
    ),
    (
        "SchematicSymbolInstancePlotRecord",
        "symbol_instance",
        "deserialize_symbol_instance_record_kind",
    ),
    (
        "SchematicSymbolOverplotPlotRecord",
        "symbol_overplot",
        "deserialize_symbol_overplot_record_kind",
    ),
];

const SCHEMATIC_SYMBOL_OPERATION_KINDS: [(&str, &str, &str); 16] = [
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
        "PlotImageOperation",
        "PlotImage",
        "deserialize_plot_image_kind",
    ),
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
    (
        "SchematicSymbolStartBlockOperation",
        "StartBlock",
        "deserialize_start_block_kind",
    ),
    (
        "SchematicSymbolEndBlockOperation",
        "EndBlock",
        "deserialize_end_block_kind",
    ),
];

const SCHEMATIC_REQUEST_U64_FIELDS: [&str; 15] = [
    "max_source_bytes",
    "max_worksheet_bytes",
    "max_output_bytes",
    "max_text_bytes",
    "max_metadata_bytes",
    "max_image_encoded_bytes",
    "max_image_decoded_bytes",
    "max_image_pixels",
    "max_image_decode_work",
    "max_symbol_overlap_checks",
    "max_text_variable_bytes",
    "max_worksheet_bitmap_encoded_bytes",
    "max_worksheet_bitmap_decoded_bytes",
    "max_worksheet_bitmap_pixels",
    "max_worksheet_bitmap_decode_work",
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
        flatten_board_footprint_operation_extensions(schema_name, &mut schema)?;
        project_schematic_request_fields(schema_name, &mut schema)?;
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

fn flatten_board_footprint_operation_extensions(
    schema_name: &str,
    schema: &mut Value,
) -> Result<()> {
    if schema_name != "BoardPlotDocument.json" {
        return Ok(());
    }
    for (structure, _, _) in BOARD_FOOTPRINT_OPERATION_KINDS.iter().take(13) {
        if schema
            .pointer(&format!("/$defs/{structure}/allOf"))
            .is_none()
        {
            continue;
        }
        let base_reference = schema
            .pointer(&format!("/$defs/{structure}/allOf/0/$ref"))
            .and_then(Value::as_str)
            .with_context(|| format!("missing generated {structure} base reference"))?;
        let base = base_reference
            .strip_prefix("#/$defs/")
            .with_context(|| format!("{structure} base reference leaves $defs"))?;
        let base_properties = schema
            .pointer(&format!("/$defs/{base}/properties"))
            .and_then(Value::as_object)
            .with_context(|| format!("missing generated {base} properties"))?
            .clone();
        let base_required = schema.pointer(&format!("/$defs/{base}/required")).cloned();
        let extension = schema
            .pointer_mut(&format!("/$defs/{structure}"))
            .and_then(Value::as_object_mut)
            .with_context(|| format!("missing generated {structure}"))?;
        extension.remove("allOf");
        let properties = extension
            .get_mut("properties")
            .and_then(Value::as_object_mut)
            .with_context(|| format!("missing generated {structure} properties"))?;
        for (name, property) in base_properties {
            if properties.insert(name.clone(), property).is_some() {
                bail!("{structure} unexpectedly redeclared base property {name}");
            }
        }
        if let Some(required) = base_required {
            extension.insert("required".to_owned(), required);
        }
    }
    Ok(())
}

fn validate_plotter_operation_kinds(schema_name: &str, schema: &Value) -> Result<()> {
    if !matches!(
        schema_name,
        "BoardPlotDocument.json"
            | "FootprintPlotDocument.json"
            | "SchematicPlotDocument.json"
            | "SymbolPlotDocument.json"
    ) {
        return Ok(());
    }
    validate_operation_union(
        schema_name,
        schema,
        "PlotterOperation",
        &PLOTTER_OPERATION_KINDS,
    )?;
    if schema_name == "BoardPlotDocument.json" {
        validate_operation_union(
            schema_name,
            schema,
            "BoardFootprintOperation",
            &BOARD_FOOTPRINT_OPERATION_KINDS,
        )?;
    }
    if schema_name == "SchematicPlotDocument.json" {
        validate_operation_union(
            schema_name,
            schema,
            "SchematicPlotRecord",
            &SCHEMATIC_RECORD_KINDS,
        )?;
        validate_operation_union(
            schema_name,
            schema,
            "SchematicSymbolOperation",
            &SCHEMATIC_SYMBOL_OPERATION_KINDS,
        )?;
    }
    Ok(())
}

fn validate_operation_union(
    schema_name: &str,
    schema: &Value,
    union: &str,
    expected_kinds: &[(&str, &str, &str)],
) -> Result<()> {
    let members = schema
        .pointer(&format!("/$defs/{union}/anyOf"))
        .and_then(Value::as_array)
        .with_context(|| format!("missing {schema_name} {union} union"))?;
    let mut actual = BTreeMap::new();
    for member in members {
        let reference = member
            .get("$ref")
            .and_then(Value::as_str)
            .with_context(|| format!("{schema_name} {union} member is not a reference"))?;
        let structure = reference
            .strip_prefix("#/$defs/")
            .with_context(|| format!("{schema_name} {union} reference leaves $defs"))?;
        let kind = literal_kind_for_structure(schema, structure)?;
        if actual.insert(structure, kind).is_some() {
            bail!("duplicate {schema_name} {union} member {structure}");
        }
    }
    let expected = expected_kinds
        .iter()
        .map(|(structure, kind, _)| (*structure, *kind))
        .collect::<BTreeMap<_, _>>();
    if actual != expected {
        bail!("{schema_name} {union} union changed; update exact-kind projection");
    }
    Ok(())
}

fn literal_kind_for_structure<'a>(schema: &'a Value, structure: &str) -> Result<&'a str> {
    if let Some(kind) = schema
        .pointer(&format!("/$defs/{structure}/properties/kind/const"))
        .and_then(Value::as_str)
    {
        return Ok(kind);
    }
    let reference = schema
        .pointer(&format!("/$defs/{structure}/allOf/0/$ref"))
        .and_then(Value::as_str)
        .with_context(|| format!("missing literal kind or base operation for {structure}"))?;
    let base = reference
        .strip_prefix("#/$defs/")
        .with_context(|| format!("{structure} base operation reference leaves $defs"))?;
    schema
        .pointer(&format!("/$defs/{base}/properties/kind/const"))
        .and_then(Value::as_str)
        .with_context(|| format!("missing literal kind for {structure} base {base}"))
}

fn project_generated_presence(schema_name: &str, source: String) -> Result<String> {
    if !matches!(
        schema_name,
        "BoardPlotDocument.json"
            | "FootprintPlotDocument.json"
            | "SchematicPlotDocument.json"
            | "SchematicPlotRequest.json"
            | "SymbolPlotDocument.json"
    ) {
        return Ok(source);
    }
    let mut projected = source;
    if schema_name == "SchematicPlotRequest.json" {
        return project_schematic_request_u64_strings(projected);
    }
    if matches!(
        schema_name,
        "FootprintPlotDocument.json" | "SchematicPlotDocument.json" | "SymbolPlotDocument.json"
    ) {
        let original = r#"    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,"#;
        let replacement = r#"    #[serde(
        default,
        deserialize_with = "crate::reject_present_render_cache_polygons",
        skip_serializing_if = "::std::vec::Vec::is_empty"
    )]
    pub render_cache_polygons: ::std::vec::Vec<::std::vec::Vec<PlotterPoint>>,"#;
        if !projected.contains(original) {
            bail!("{schema_name} Text render-cache polygon projection changed");
        }
        projected = projected.replace(original, replacement);
    }
    for (structure, _, deserializer) in PLOTTER_OPERATION_KINDS {
        projected = project_kind_deserializer(projected, structure, deserializer)?;
    }
    if schema_name == "BoardPlotDocument.json" {
        for (structure, _, deserializer) in BOARD_FOOTPRINT_OPERATION_KINDS {
            projected = project_kind_deserializer(projected, structure, deserializer)?;
        }
        projected = project_dimension_text_presence(projected)?;
    }
    if schema_name == "SchematicPlotDocument.json" {
        for (structure, _, deserializer) in SCHEMATIC_RECORD_KINDS {
            projected = project_kind_deserializer(projected, structure, deserializer)?;
        }
        for (structure, _, deserializer) in SCHEMATIC_SYMBOL_OPERATION_KINDS
            .into_iter()
            .filter(|(structure, _, _)| structure.starts_with("SchematicSymbol"))
        {
            projected = project_kind_deserializer(projected, structure, deserializer)?;
        }
        projected = project_schematic_record_string(projected)?;
        projected = project_schematic_junction_color(projected)?;
        // The deterministic-map substitution can cross rustfmt's line-width
        // boundary, so normalize the fully projected source as the final step.
        projected = rustfmt(&projected)?;
    }
    Ok(projected)
}

fn project_schematic_request_u64_strings(mut source: String) -> Result<String> {
    for field_name in SCHEMATIC_REQUEST_U64_FIELDS {
        let field = format!("    pub {field_name}: ::std::string::String,");
        if source.matches(&field).count() != 1 {
            bail!("SchematicPlotRequest.json {field_name} uint64 projection changed");
        }
        let replacement =
            format!("    #[serde(deserialize_with = \"crate::deserialize_u64_string\")]\n{field}");
        source = source.replacen(&field, &replacement, 1);
    }
    rustfmt(&source)
}

fn project_schematic_junction_color(mut source: String) -> Result<String> {
    let structure = "SchematicJunctionPlotRecord";
    let structure_marker = format!("pub struct {structure} {{");
    let structure_start = source
        .find(&structure_marker)
        .with_context(|| format!("missing generated {structure}"))?;
    let structure_end = source[structure_start..]
        .find("\n}")
        .map(|offset| structure_start + offset)
        .with_context(|| format!("unterminated generated {structure}"))?;
    let field = r#"    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub color: ::std::option::Option<::std::string::String>,"#;
    let field_offset = source[structure_start..structure_end]
        .find(field)
        .map(|offset| structure_start + offset)
        .with_context(|| format!("missing generated {structure}.color"))?;
    let replacement = r#"    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nullable_string",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub color: ::std::option::Option<::std::option::Option<::std::string::String>>,"#;
    source.replace_range(field_offset..field_offset + field.len(), replacement);
    Ok(source)
}

fn project_schematic_record_string(source: String) -> Result<String> {
    let hash_map = "::std::collections::HashMap<::std::string::String, ::std::string::String>";
    if !source.contains(hash_map) {
        bail!("SchematicPlotDocument.json RecordString map projection changed");
    }
    Ok(source.replace(
        hash_map,
        "::std::collections::BTreeMap<::std::string::String, ::std::string::String>",
    ))
}

fn project_dimension_text_presence(mut source: String) -> Result<String> {
    let structure = "DimensionPlotRecord";
    let structure_marker = format!("pub struct {structure} {{");
    let structure_start = source
        .find(&structure_marker)
        .with_context(|| format!("missing generated {structure}"))?;
    let structure_end = source[structure_start..]
        .find("\n}")
        .map(|offset| structure_start + offset)
        .with_context(|| format!("unterminated generated {structure}"))?;
    let field = r#"    #[serde(default, skip_serializing_if = "::std::option::Option::is_none")]
    pub text: ::std::option::Option<::std::string::String>,"#;
    let field_offset = source[structure_start..structure_end]
        .find(field)
        .map(|offset| structure_start + offset)
        .with_context(|| format!("missing generated {structure}.text"))?;
    let replacement = r#"    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_optional_string",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub text: ::std::option::Option<::std::string::String>,"#;
    source.replace_range(field_offset..field_offset + field.len(), replacement);
    Ok(source)
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
    settings.with_replacement(
        "SchematicPositiveUint32",
        "::std::num::NonZeroU32",
        [].into_iter(),
    );
    settings.with_replacement(
        "SchematicDefaultLineWidthNm",
        "crate::SchematicDefaultLineWidthNm",
        [].into_iter(),
    );
    settings.with_replacement(
        "SchematicTextOffsetRatio",
        "crate::NonNegativeFiniteFloat",
        [].into_iter(),
    );
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
    for pointer in [
        "/$defs/BoardPlotRecord",
        "/$defs/BoardFootprintOperation",
        "/$defs/SchematicPlotRecord",
    ] {
        if let Some(record) = schema.pointer_mut(pointer).and_then(Value::as_object_mut)
            && let Some(members) = record.remove("anyOf")
        {
            record.insert("oneOf".to_owned(), members);
        }
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

fn project_schematic_request_fields(schema_name: &str, schema: &mut Value) -> Result<()> {
    if schema_name != "SchematicPlotRequest.json" {
        return Ok(());
    }
    let bounded = serde_json::json!({
        "type": "integer",
        "minimum": 1,
        "maximum": 4_294_967_295_u64,
    });
    for field in ["sheet_index", "sheet_count"] {
        let pointer = format!("/properties/{field}");
        let property = schema
            .pointer_mut(&pointer)
            .with_context(|| format!("missing SchematicPlotRequest.{field}"))?;
        if property != &bounded {
            bail!("SchematicPlotRequest.{field} positive-u32 schema changed");
        }
        *property = serde_json::json!({"$ref": "#/$defs/SchematicPositiveUint32"});
    }
    let definitions = schema
        .pointer_mut("/$defs")
        .and_then(Value::as_object_mut)
        .context("missing SchematicPlotRequest $defs")?;
    definitions.insert("SchematicPositiveUint32".to_owned(), bounded);

    let default_width = serde_json::json!({
        "type": "integer",
        "minimum": 84_700,
        "maximum": 9_007_199_254_740_991_i64,
    });
    let default_width_property = schema
        .pointer("/properties/default_line_width_nm")
        .context("missing SchematicPlotRequest.default_line_width_nm")?;
    let mut default_width_definition = schema
        .pointer("/$defs/SchematicDefaultLineWidthNm")
        .cloned()
        .context("missing SchematicDefaultLineWidthNm definition")?;
    default_width_definition
        .as_object_mut()
        .context("SchematicDefaultLineWidthNm must be an object")?
        .remove("description");
    if default_width_property != &serde_json::json!({"$ref": "#/$defs/SchematicDefaultLineWidthNm"})
        || default_width_definition != default_width
    {
        bail!("SchematicPlotRequest.default_line_width_nm schema changed");
    }

    let ratio = serde_json::json!({
        "type": "number",
        "minimum": 0,
        "maximum": 1.7976931348623157e308_f64,
    });
    let ratio_property = schema
        .pointer("/properties/text_offset_ratio")
        .context("missing SchematicPlotRequest.text_offset_ratio")?;
    let mut ratio_definition = schema
        .pointer("/$defs/SchematicTextOffsetRatio")
        .cloned()
        .context("missing SchematicTextOffsetRatio definition")?;
    ratio_definition
        .as_object_mut()
        .context("SchematicTextOffsetRatio must be an object")?
        .remove("description");
    if ratio_property != &serde_json::json!({"$ref": "#/$defs/SchematicTextOffsetRatio"})
        || ratio_definition != ratio
    {
        bail!("SchematicPlotRequest.text_offset_ratio schema changed");
    }
    Ok(())
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
            if let Some(entry) = object.remove("unevaluatedProperties") {
                let projected = if entry == serde_json::json!({"not": {}}) {
                    Value::Bool(false)
                } else {
                    entry
                };
                object
                    .entry("additionalProperties".to_owned())
                    .or_insert(projected);
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
