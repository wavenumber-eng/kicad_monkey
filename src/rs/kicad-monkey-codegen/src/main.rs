//! Deterministic Rust projection from TypeSpec-generated JSON Schemas.

use anyhow::{Context, Result, bail};
use schemars::schema::RootSchema;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use typify::{TypeSpace, TypeSpaceSettings};

const SCHEMAS: [(&str, &str); 37] = [
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
    ("NativeHandshake.json", "native_handshake.rs"),
    ("NativeHandshakeA1.json", "native_handshake_a1.rs"),
    (
        "NativeDesignFactsRequest.json",
        "native_design_facts_request.rs",
    ),
    (
        "NativeDesignFactsResult.json",
        "native_design_facts_result.rs",
    ),
    (
        "NativeSvgRenderRequest.json",
        "native_svg_render_request.rs",
    ),
    ("NativeSvgRenderResult.json", "native_svg_render_result.rs"),
    ("NativeError.json", "native_error.rs"),
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

const SCHEMATIC_RECORD_KINDS: [(&str, &str, &str); 23] = [
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
    (
        "SchematicSheetPlotRecord",
        "sheet",
        "deserialize_sheet_record_kind",
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

const SCHEMATIC_SHEET_OPERATION_KINDS: [(&str, &str, &str); 6] = [
    (
        "ThickSegmentOperation",
        "ThickSegment",
        "deserialize_thick_segment_kind",
    ),
    ("RectOperation", "Rect", "deserialize_rect_kind"),
    (
        "PlotPolyOperation",
        "PlotPoly",
        "deserialize_plot_poly_kind",
    ),
    ("TextOperation", "Text", "deserialize_text_kind"),
    (
        "SchematicSheetStartBlockOperation",
        "StartBlock",
        "deserialize_start_block_kind",
    ),
    (
        "SchematicSheetEndBlockOperation",
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
        let published_schema = schema.clone();
        validate_plotter_operation_kinds(schema_name, &schema)?;
        flatten_board_footprint_operation_extensions(schema_name, &mut schema)?;
        project_schematic_request_fields(schema_name, &mut schema)?;
        project_native_external_references(schema_name, &mut schema)?;
        project_native_handshake_tuple(schema_name, &mut schema)?;
        project_for_typify(&mut schema);
        promote_disjoint_record_unions(&mut schema);
        project_tri_state_via_drill_layers(&mut schema);
        let generated = project_generated_presence(
            schema_name,
            &published_schema,
            generate(schema_name, schema)?,
        )?;
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
        validate_operation_union(
            schema_name,
            schema,
            "SchematicSheetOperation",
            &SCHEMATIC_SHEET_OPERATION_KINDS,
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

fn project_generated_presence(
    schema_name: &str,
    published_schema: &Value,
    source: String,
) -> Result<String> {
    if !uses_strict_presence_projection(schema_name) {
        return Ok(source);
    }
    let mut projected = source;
    if schema_name == "SchematicPlotRequest.json" {
        projected = project_schematic_request_u64_strings(projected)?;
    }
    let is_document = matches!(
        schema_name,
        "BoardPlotDocument.json"
            | "FootprintPlotDocument.json"
            | "SchematicPlotDocument.json"
            | "SymbolPlotDocument.json"
    );
    if is_document
        && matches!(
            schema_name,
            "FootprintPlotDocument.json" | "SchematicPlotDocument.json" | "SymbolPlotDocument.json"
        )
    {
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
    if is_document {
        for (structure, _, deserializer) in PLOTTER_OPERATION_KINDS {
            projected = project_kind_deserializer(projected, structure, deserializer)?;
        }
    }
    if schema_name == "BoardPlotDocument.json" {
        for (structure, _, deserializer) in BOARD_FOOTPRINT_OPERATION_KINDS {
            projected = project_kind_deserializer(projected, structure, deserializer)?;
        }
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
        for (structure, _, deserializer) in SCHEMATIC_SHEET_OPERATION_KINDS
            .into_iter()
            .filter(|(structure, _, _)| structure.starts_with("SchematicSheet"))
        {
            projected = project_kind_deserializer(projected, structure, deserializer)?;
        }
        projected = project_schematic_record_string(projected)?;
        projected = project_schematic_junction_color(projected)?;
        projected = project_schematic_segment_layers(projected)?;
    }
    projected = project_schema_presence(schema_name, published_schema, projected)?;
    // Presence and deterministic-map substitutions can cross rustfmt's line-width
    // boundary, so normalize the fully projected source as the final step.
    rustfmt(&projected)
}

fn project_schematic_segment_layers(mut source: String) -> Result<String> {
    let marker = "pub struct ThickSegmentOperation {";
    let start = source
        .find(marker)
        .context("missing generated ThickSegmentOperation")?;
    let end = source[start..]
        .find("\n}")
        .map(|offset| start + offset)
        .context("unterminated generated ThickSegmentOperation")?;
    let field = r#"    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub layers: ::std::vec::Vec<::std::string::String>,"#;
    let offset = source[start..end]
        .find(field)
        .map(|offset| start + offset)
        .context("missing generated ThickSegmentOperation.layers")?;
    let replacement = r#"    #[serde(
        default,
        deserialize_with = "crate::reject_present_schematic_segment_layers",
        skip_serializing_if = "::std::vec::Vec::is_empty"
    )]
    pub layers: ::std::vec::Vec<::std::string::String>,"#;
    source.replace_range(offset..offset + field.len(), replacement);
    Ok(source)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FieldPresence {
    OptionalNonnullable,
    OptionalNullable,
    RequiredNullable,
}

fn uses_strict_presence_projection(schema_name: &str) -> bool {
    matches!(
        schema_name,
        "CompiledSchematicGraph.json"
            | "SourceBundleManifest.json"
            | "BoardPlotDocument.json"
            | "BoardPlotRequest.json"
            | "BoardPlotResult.json"
            | "FootprintPlotDocument.json"
            | "FootprintPlotRequest.json"
            | "FootprintPlotResult.json"
            | "SymbolPlotDocument.json"
            | "SymbolPlotRequest.json"
            | "SymbolPlotResult.json"
            | "SchematicPlotDocument.json"
            | "SchematicPlotRequest.json"
            | "SchematicPlotResult.json"
    )
}

fn project_schema_presence(schema_name: &str, schema: &Value, source: String) -> Result<String> {
    let rules = schema_presence_rules(schema_name, schema)?;
    let mut syntax =
        syn::parse_file(&source).context("parse generated Rust for presence projection")?;
    let mut seen = BTreeSet::new();
    let mut projected_optionals = 0usize;
    let mut required_nullables = 0usize;

    for item in &mut syntax.items {
        let syn::Item::Struct(structure) = item else {
            continue;
        };
        let syn::Fields::Named(fields) = &mut structure.fields else {
            continue;
        };
        let structure_name = structure.ident.to_string();
        for field in &mut fields.named {
            let Some(field_ident) = &field.ident else {
                continue;
            };
            let rust_name = field_ident.to_string();
            let wire_name = serde_field_name(field)?.unwrap_or_else(|| rust_name.clone());
            let key = (structure_name.clone(), wire_name.clone());
            let Some(presence) = rules.get(&key).copied() else {
                continue;
            };
            seen.insert(key.clone());
            if !is_option_type(&field.ty) {
                // Vec/map/defaulted scalar projections already reject JSON null through
                // their native deserializers; only Option<T> needs a presence projection.
                continue;
            }
            let (optional_count, required_count) =
                project_presence_field(schema_name, field, presence, &wire_name, &rust_name, &key)?;
            projected_optionals += optional_count;
            required_nullables += required_count;
        }
    }

    let missing = rules
        .keys()
        .filter(|key| !seen.contains(*key))
        .map(|(structure, field)| format!("{structure}.{field}"))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "{schema_name} generated presence fields are missing: {}",
            missing.join(", ")
        );
    }
    let expected_required_nullables = rules
        .values()
        .filter(|presence| **presence == FieldPresence::RequiredNullable)
        .count();
    if required_nullables != expected_required_nullables {
        bail!(
            "{schema_name} required-nullable projection changed: {required_nullables} != {expected_required_nullables}"
        );
    }
    if rules
        .values()
        .any(|presence| *presence == FieldPresence::OptionalNonnullable)
        && projected_optionals == 0
    {
        bail!("{schema_name} has no projected optional nonnullable Option<T> fields");
    }
    Ok(prettyplease::unparse(&syntax))
}

fn project_presence_field(
    schema_name: &str,
    field: &mut syn::Field,
    presence: FieldPresence,
    wire_name: &str,
    rust_name: &str,
    key: &(String, String),
) -> Result<(usize, usize)> {
    let span = field
        .ident
        .as_ref()
        .context("generated field has no name")?
        .span();
    match presence {
        FieldPresence::OptionalNonnullable => {
            let replacement = if wire_name == rust_name {
                syn::parse_quote!(#[serde(
                    default,
                    deserialize_with = "crate::deserialize_present_nonnull",
                    skip_serializing_if = "::std::option::Option::is_none"
                )])
            } else {
                let wire_name = syn::LitStr::new(wire_name, span);
                syn::parse_quote!(#[serde(
                    rename = #wire_name,
                    default,
                    deserialize_with = "crate::deserialize_present_nonnull",
                    skip_serializing_if = "::std::option::Option::is_none"
                )])
            };
            replace_serde_presence_attr(field, replacement)?;
            Ok((1, 0))
        }
        FieldPresence::RequiredNullable => {
            let replacement = if wire_name == rust_name {
                syn::parse_quote!(#[serde(
                    deserialize_with = "crate::deserialize_required_nullable"
                )])
            } else {
                let wire_name = syn::LitStr::new(wire_name, span);
                syn::parse_quote!(#[serde(
                    rename = #wire_name,
                    deserialize_with = "crate::deserialize_required_nullable"
                )])
            };
            replace_serde_presence_attr(field, replacement)?;
            Ok((0, 1))
        }
        FieldPresence::OptionalNullable => {
            if key != &("SchematicJunctionPlotRecord".to_owned(), "color".to_owned())
                || !is_nested_option_type(&field.ty)
                || !has_deserializer(field, "deserialize_present_nullable_string")
            {
                bail!(
                    "{schema_name} optional-nullable projection changed at {}.{}",
                    key.0,
                    key.1
                );
            }
            Ok((0, 0))
        }
    }
}

fn serde_field_name(field: &syn::Field) -> Result<Option<String>> {
    let mut rename = None;
    for attribute in field
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("serde"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value = meta.value()?;
                rename = Some(value.parse::<syn::LitStr>()?.value());
            } else if meta.input.peek(syn::Token![=]) {
                let _ = meta.value()?.parse::<syn::Expr>()?;
            }
            Ok(())
        })?;
    }
    Ok(rename)
}

fn replace_serde_presence_attr(field: &mut syn::Field, replacement: syn::Attribute) -> Result<()> {
    let serde_attributes = field
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("serde"))
        .count();
    if serde_attributes > 1 {
        bail!("generated optional field has multiple serde attributes");
    }
    field
        .attrs
        .retain(|attribute| !attribute.path().is_ident("serde"));
    field.attrs.push(replacement);
    Ok(())
}

fn has_deserializer(field: &syn::Field, needle: &str) -> bool {
    field.attrs.iter().any(|attribute| {
        attribute.path().is_ident("serde")
            && matches!(&attribute.meta, syn::Meta::List(list) if list.tokens.to_string().contains(needle))
    })
}

fn is_option_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Option")
}

fn is_nested_option_type(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    arguments.args.iter().any(
        |argument| matches!(argument, syn::GenericArgument::Type(inner) if is_option_type(inner)),
    )
}

fn schema_presence_rules(
    schema_name: &str,
    schema: &Value,
) -> Result<BTreeMap<(String, String), FieldPresence>> {
    let mut objects = BTreeMap::new();
    objects.insert(
        schema_name.trim_end_matches(".json").to_owned() + "A0",
        schema,
    );
    if let Some(definitions) = schema.get("$defs").and_then(Value::as_object) {
        objects.extend(
            definitions
                .iter()
                .map(|(name, value)| (name.clone(), value)),
        );
    }

    let mut rules = BTreeMap::new();
    for (structure, object) in objects {
        let mut properties = BTreeMap::new();
        let mut required = BTreeSet::new();
        collect_object_shape(schema, object, &mut properties, &mut required)?;
        for (field, property) in properties {
            let nullable = schema_allows_null(property);
            let presence = match (required.contains(&field), nullable) {
                (false, false) => Some(FieldPresence::OptionalNonnullable),
                (false, true) => Some(FieldPresence::OptionalNullable),
                (true, true) => Some(FieldPresence::RequiredNullable),
                (true, false) => None,
            };
            if let Some(presence) = presence {
                rules.insert((structure.clone(), field.clone()), presence);
            }
        }
    }
    Ok(rules)
}

fn collect_object_shape<'a>(
    root: &'a Value,
    object: &'a Value,
    properties: &mut BTreeMap<String, &'a Value>,
    required: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        let name = reference
            .strip_prefix("#/$defs/")
            .with_context(|| format!("presence reference leaves $defs: {reference}"))?;
        let target = root
            .pointer(&format!("/$defs/{name}"))
            .with_context(|| format!("missing presence reference {reference}"))?;
        collect_object_shape(root, target, properties, required)?;
    }
    if let Some(parts) = object.get("allOf").and_then(Value::as_array) {
        for part in parts {
            collect_object_shape(root, part, properties, required)?;
        }
    }
    if let Some(fields) = object.get("properties").and_then(Value::as_object) {
        properties.extend(fields.iter().map(|(name, value)| (name.clone(), value)));
    }
    if let Some(names) = object.get("required").and_then(Value::as_array) {
        for name in names.iter().filter_map(Value::as_str) {
            if !properties.contains_key(name) {
                bail!("required presence field {name} has no property");
            }
            required.insert(name.to_owned());
        }
    }
    Ok(())
}

fn schema_allows_null(schema: &Value) -> bool {
    match schema.get("type") {
        Some(Value::String(kind)) if kind == "null" => return true,
        Some(Value::Array(kinds))
            if kinds
                .iter()
                .any(|kind| kind.as_str().is_some_and(|kind| kind == "null")) =>
        {
            return true;
        }
        _ => {}
    }
    ["anyOf", "oneOf"].into_iter().any(|key| {
        schema
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|members| members.iter().any(schema_allows_null))
    })
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

fn generate(schema_name: &str, value: Value) -> Result<String> {
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
    if schema_name == "NativeDesignFactsRequest.json" {
        settings.with_replacement(
            "NativeSourceBundleManifestProjection",
            "crate::generated::source_bundle_manifest::SourceBundleManifestA0",
            [].into_iter(),
        );
    }
    if schema_name == "NativeDesignFactsResult.json" {
        settings.with_replacement(
            "NativeCompiledSchematicGraphProjection",
            "crate::generated::compiled_schematic_graph::CompiledSchematicGraphA0",
            [].into_iter(),
        );
    }
    if schema_name == "NativeSvgRenderRequest.json" {
        settings.with_replacement(
            "NativeFootprintPlotDocumentProjection",
            "::serde_json::Value",
            [].into_iter(),
        );
        settings.with_replacement(
            "NativeSymbolPlotDocumentProjection",
            "::serde_json::Value",
            [].into_iter(),
        );
        settings.with_replacement(
            "NativeBoardPlotDocumentProjection",
            "::serde_json::Value",
            [].into_iter(),
        );
        settings.with_replacement(
            "NativeSchematicPlotDocumentProjection",
            "::serde_json::Value",
            [].into_iter(),
        );
    }
    let mut type_space = TypeSpace::new(&settings);
    type_space.add_root_schema(schema)?;
    let body = type_space.to_stream().to_string();
    let source =
        format!("// Generated from TypeSpec JSON Schema through typify. Do not edit.\n\n{body}\n");
    let syntax = syn::parse_file(&source).context("parse generated Rust")?;
    rustfmt(&prettyplease::unparse(&syntax))
}

fn project_native_handshake_tuple(schema_name: &str, schema: &mut Value) -> Result<()> {
    if schema_name != "NativeHandshakeA1.json" {
        return Ok(());
    }
    let operations = schema
        .pointer_mut("/properties/operations")
        .and_then(Value::as_object_mut)
        .context("NativeHandshakeA1.json missing operations schema")?;
    if operations.get("minItems") != Some(&Value::from(2))
        || operations.get("maxItems") != Some(&Value::from(2))
    {
        bail!("NativeHandshakeA1.json operations tuple length changed");
    }
    let prefix_items = operations
        .remove("prefixItems")
        .and_then(|value| value.as_array().cloned())
        .context("NativeHandshakeA1.json operations tuple projection changed")?;
    let expected = ["design-facts", "render-svg"];
    if prefix_items.len() != expected.len()
        || prefix_items
            .iter()
            .zip(expected)
            .any(|(item, expected)| item.get("const").and_then(Value::as_str) != Some(expected))
    {
        bail!("NativeHandshakeA1.json operations tuple order changed");
    }
    operations.insert(
        "items".to_owned(),
        serde_json::json!({ "anyOf": prefix_items }),
    );
    Ok(())
}

fn project_native_external_references(schema_name: &str, schema: &mut Value) -> Result<()> {
    if schema_name == "NativeSvgRenderRequest.json" {
        let references = [
            (
                "/$defs/NativeFootprintSvgDocument/properties/value/$ref",
                "urn:wavenumber:schema:kicad_monkey.footprint_plot.document:a0",
                "NativeFootprintPlotDocumentProjection",
            ),
            (
                "/$defs/NativeSymbolSvgDocument/properties/value/$ref",
                "urn:wavenumber:schema:kicad_monkey.symbol_plot.document:a0",
                "NativeSymbolPlotDocumentProjection",
            ),
            (
                "/$defs/NativeBoardSvgDocument/properties/value/$ref",
                "urn:wavenumber:schema:kicad_monkey.board_plot.document:a0",
                "NativeBoardPlotDocumentProjection",
            ),
            (
                "/$defs/NativeSchematicSvgDocument/properties/value/$ref",
                "urn:wavenumber:schema:kicad_monkey.schematic_plot.document:a0",
                "NativeSchematicPlotDocumentProjection",
            ),
        ];
        for (pointer, external, projection) in references {
            project_native_reference(schema_name, schema, pointer, external, projection)?;
        }
        return Ok(());
    }
    let (pointer, external, projection) = match schema_name {
        "NativeDesignFactsRequest.json" => (
            "/properties/manifest/$ref",
            "urn:wavenumber:schema:kicad_monkey.source_bundle_manifest:a0",
            "NativeSourceBundleManifestProjection",
        ),
        "NativeDesignFactsResult.json" => (
            "/properties/compiled_schematic_graph/$ref",
            "urn:wavenumber:schema:kicad_monkey.compiled_schematic_graph:a0",
            "NativeCompiledSchematicGraphProjection",
        ),
        _ => return Ok(()),
    };
    project_native_reference(schema_name, schema, pointer, external, projection)
}

fn project_native_reference(
    schema_name: &str,
    schema: &mut Value,
    pointer: &str,
    external: &str,
    projection: &str,
) -> Result<()> {
    let reference = schema
        .pointer_mut(pointer)
        .with_context(|| format!("missing {schema_name} external contract reference"))?;
    if reference.as_str() != Some(external) {
        bail!("{schema_name} external contract reference changed");
    }
    *reference = Value::String(format!("#/$defs/{projection}"));
    let definitions = schema
        .as_object_mut()
        .context("native transport schema root must be an object")?
        .entry("$defs")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .with_context(|| format!("invalid {schema_name} definitions"))?;
    definitions.insert(projection.to_owned(), serde_json::json!({"type": "object"}));
    Ok(())
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
