import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { compile } from "json-schema-to-typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const schemaRoot = path.join(root, "contracts/generated/schema");
const check = process.argv.includes("--check");
const generatePython = process.argv.includes("--python");
const generateTypeScript = process.argv.includes("--typescript");
assert(generatePython || generateTypeScript, "select --python and/or --typescript");

const roots = [
  ["BuildRequest.json", "SExpressionBuildRequestA0", "build-request.ts"],
  ["BuildResult.json", "SExpressionBuildResultA0", "build-result.ts"],
  ["ScanRequest.json", "SExpressionScanRequestA0", "scan-request.ts"],
  ["ScanResult.json", "SExpressionScanResultA0", "scan-result.ts"],
  ["FootprintEditRequest.json", "FootprintEditRequestA0", "footprint-edit-request.ts"],
  ["FootprintEditResult.json", "FootprintEditResultA0", "footprint-edit-result.ts"],
  ["FootprintReadRequest.json", "FootprintReadRequestA0", "footprint-read-request.ts"],
  ["FootprintReadResult.json", "FootprintReadResultA0", "footprint-read-result.ts"],
  ["FootprintPlotDocument.json", "FootprintPlotDocumentA0", "footprint-plot-document.ts"],
  ["FootprintPlotRequest.json", "FootprintPlotRequestA0", "footprint-plot-request.ts"],
  ["FootprintPlotResult.json", "FootprintPlotResultA0", "footprint-plot-result.ts"],
  ["BoardPlotDocument.json", "BoardPlotDocumentA0", "board-plot-document.ts"],
  ["BoardPlotRequest.json", "BoardPlotRequestA0", "board-plot-request.ts"],
  ["BoardPlotResult.json", "BoardPlotResultA0", "board-plot-result.ts"],
  ["SymbolPlotDocument.json", "SymbolPlotDocumentA0", "symbol-plot-document.ts"],
  ["SymbolPlotRequest.json", "SymbolPlotRequestA0", "symbol-plot-request.ts"],
  ["SymbolPlotResult.json", "SymbolPlotResultA0", "symbol-plot-result.ts"],
  ["SchematicPlotDocument.json", "SchematicPlotDocumentA0", "schematic-plot-document.ts"],
  ["SchematicPlotRequest.json", "SchematicPlotRequestA0", "schematic-plot-request.ts"],
  ["SchematicPlotResult.json", "SchematicPlotResultA0", "schematic-plot-result.ts"],
  ["SymbolLibraryEditRequest.json", "SymbolLibraryEditRequestA0", "symbol-library-edit-request.ts"],
  ["SymbolLibraryEditResult.json", "SymbolLibraryEditResultA0", "symbol-library-edit-result.ts"],
  ["SymbolLibraryReadRequest.json", "SymbolLibraryReadRequestA0", "symbol-library-read-request.ts"],
  ["SymbolLibraryReadResult.json", "SymbolLibraryReadResultA0", "symbol-library-read-result.ts"],
  ["CompiledSchematicGraph.json", "CompiledSchematicGraphA0", "compiled-schematic-graph.ts"],
  ["SourceBundleManifest.json", "SourceBundleManifestA0", "source-bundle-manifest.ts"],
  ["FontBundleManifest.json", "FontBundleManifestA0", "font-bundle-manifest.ts"],
  ["FontResolutionRequest.json", "FontResolutionRequestA0", "font-resolution-request.ts"],
  ["ShapingRecord.json", "ShapingRecordA0", "shaping-record.ts"],
  ["OutlineVector.json", "OutlineVectorA0", "outline-vector.ts"],
];
const schemas = new Map();
for (const [file] of roots) {
  const document = JSON.parse(await readFile(path.join(schemaRoot, file), "utf8"));
  assert(document.$schema === "https://json-schema.org/draft/2020-12/schema", `${file}: draft`);
  schemas.set(file, document);
}

if (generatePython) {
  const output = renderPython();
  await emit(path.join(root, "src/py/kicad_monkey/contracts/generated.py"), output);
  await emit(
    path.join(root, "src/py/kicad_monkey/contracts/__init__.py"),
    [
      '"""TypeSpec-generated KiCad Monkey transport contracts."""',
      "",
      "from .generated import *  # noqa: F403",
      "from .generated import __all__ as __all__",
      "",
    ].join("\n"),
  );
}

if (generateTypeScript) {
  const outputRoot = path.join(root, "src/ts/kicad_monkey/contracts/generated");
  const exports = [];
  for (const [file, typeName, outputName] of roots) {
    const projected = projectSchema(structuredClone(schemas.get(file)));
    const source = await compile(projected, typeName, {
      bannerComment: "/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */",
      format: true,
      unknownAny: false,
    });
    const forbiddenAny = source.match(/.{0,80}(?:[:<]\s*any\b|\bany\[\]).{0,80}/u)?.[0];
    assert(!forbiddenAny, `${outputName}: forbidden any near ${JSON.stringify(forbiddenAny)}`);
    await emit(path.join(outputRoot, outputName), source);
    exports.push(
      `export type { ${typeName} } from "./${outputName.replace(/\.ts$/u, ".js")}";`,
    );
  }
  await emit(path.join(outputRoot, "index.ts"), `${exports.join("\n")}\n`);
}

function renderPython() {
  const definitions = new Map();
  for (const [file] of roots) {
    const schema = schemas.get(file);
    for (const [name, definition] of Object.entries(schema.$defs ?? {})) {
      const projected = flattenPythonObjectExtension(definition, schema.$defs ?? {});
      const encoded = JSON.stringify(projected);
      if (definitions.has(name)) {
        assert(definitions.get(name).encoded === encoded, `${name}: conflicting definitions`);
      } else {
        definitions.set(name, { encoded, schema: projected });
      }
    }
  }
  const taggedStructs = new Map();
  for (const { schema } of definitions.values()) {
    if (!Array.isArray(schema.anyOf)) continue;
    for (const variant of schema.anyOf) {
      const name = variant.$ref?.split("/").at(-1);
      const target = definitions.get(name)?.schema;
      const tagField = ["kind", "mode"].find(
        (field) => typeof target?.properties?.[field]?.const === "string",
      );
      const tag = tagField === undefined ? undefined : target.properties[tagField].const;
      if (typeof name === "string" && typeof tagField === "string" && typeof tag === "string") {
        taggedStructs.set(name, { field: tagField, value: tag });
        continue;
      }
      // msgspec cannot give one struct several tag values, so union members
      // whose discriminator is an enum expand to one tagged struct per value
      // beneath a Union alias that keeps the published member name.
      const enumField = ["kind", "mode"].find(
        (field) => typeof target?.properties?.[field]?.$ref === "string",
      );
      const enumName = target?.properties?.[enumField]?.$ref?.split("/").at(-1);
      const values = definitions.get(enumName)?.schema?.enum;
      if (
        typeof name === "string" &&
        typeof enumField === "string" &&
        Array.isArray(values) &&
        values.every((value) => typeof value === "string")
      ) {
        taggedStructs.set(name, { field: enumField, values });
      }
    }
  }

  const lines = [
    '"""Generated strict msgspec transport bindings. Do not edit."""',
    "",
    "from __future__ import annotations",
    "",
    "import hashlib",
    "import math",
    "from dataclasses import dataclass",
    "",
    "from typing import Annotated, Literal, Union",
    "",
    "import msgspec",
    "from msgspec import UNSET, Meta, Struct, UnsetType, field",
  ];
  for (const [name, value] of definitions) {
    lines.push("", "", ...renderPythonDeclaration(name, value.schema, taggedStructs.get(name)));
  }
  for (const [file, typeName] of roots) {
    lines.push("", "", ...renderPythonDeclaration(typeName, schemas.get(file)));
  }
  lines.push("", "");
  for (const [, typeName] of roots) {
    const functionName = `decode_${snakeCase(typeName.replace(/^SExpression/u, "sexpr_"))}`;
    if (typeName === "FootprintPlotDocumentA0") {
      lines.push(...renderPythonPlotterValidation(functionName, typeName));
    } else if (typeName === "BoardPlotDocumentA0") {
      lines.push(...renderPythonBoardPlotterValidation(functionName, typeName));
    } else if (typeName === "SymbolPlotDocumentA0") {
      lines.push(...renderPythonSymbolPlotterValidation(functionName, typeName));
    } else if (typeName === "SchematicPlotDocumentA0") {
      lines.push(...renderPythonSchematicPlotterValidation(functionName, typeName));
    } else if (typeName === "SourceBundleManifestA0") {
      lines.push(...renderPythonSourceBundleValidation(functionName, typeName));
    } else if (typeName === "FontBundleManifestA0") {
      lines.push(...renderPythonFontBundleValidation(functionName, typeName));
    } else if (typeName === "ShapingRecordA0") {
      lines.push(...renderPythonShapingRecordValidation(functionName, typeName));
    } else if (typeName === "OutlineVectorA0") {
      lines.push(...renderPythonOutlineVectorValidation(functionName, typeName));
    } else {
      lines.push(`${functionName} = msgspec.json.Decoder(${typeName}).decode`);
    }
  }
  const exported = [
    ...definitions.keys(),
    ...roots.map(([, typeName]) => typeName),
    ...roots.map(([, typeName]) => `decode_${snakeCase(typeName.replace(/^SExpression/u, "sexpr_"))}`),
    "validate_footprint_plot_document_a0",
    "validate_board_plot_document_a0",
    "resolve_font_selection_a0",
    "validate_font_bundle_manifest_a0",
    "validate_outline_vector_a0",
    "validate_shaping_record_a0",
    "validate_symbol_plot_document_a0",
    "validate_schematic_plot_document_a0",
  ];
  lines.push("", "", "__all__ = (", ...exported.map((name) => `    ${pythonLiteral(name)},`), ")", "");
  return lines.join("\n");
}

function flattenPythonObjectExtension(definition, definitions) {
  const projected = structuredClone(definition);
  if (!Array.isArray(projected.allOf) || projected.allOf.length !== 1) return projected;
  const reference = projected.allOf[0]?.$ref;
  const baseName = typeof reference === "string" ? reference.split("/").at(-1) : undefined;
  const base = definitions[baseName];
  assert(base?.type === "object", `${baseName}: unsupported Python object extension base`);
  projected.properties = { ...(base.properties ?? {}), ...(projected.properties ?? {}) };
  projected.required = [...new Set([...(base.required ?? []), ...(projected.required ?? [])])];
  delete projected.allOf;
  return projected;
}

function renderPythonBoardPlotterValidation(functionName, typeName) {
  return [
    `_board_plot_document_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _board_plot_document_a0_decoder.decode(data)",
    "    validate_board_plot_document_a0(value)",
    "    return value",
    "",
    "",
    `def validate_board_plot_document_a0(value: ${typeName}) -> None:`,
    '    if value.schema != "kicad.plotter_ir.a0" or value.source_kind != "PCB" or value.coordinate_space.unit != "nm" or value.coordinate_space.y_axis != "down":',
    '        raise msgspec.ValidationError("invalid_board_document at $")',
    "    total_operations = 0",
    "    saw_footprint = False",
    "    for record_index, record in enumerate(value.records):",
    "        path = f'$.records[{record_index}]'",
    "        if any(isinstance(operation, PlotImageOperation) for operation in record.operations):",
    '            raise msgspec.ValidationError(f"invalid_board_operation at {path}.operations")',
    "        if isinstance(record, BoardFootprintPlotRecord):",
    "            saw_footprint = True",
    "            _validate_board_footprint_plot_record(record, path)",
    "        elif saw_footprint:",
    '            raise msgspec.ValidationError(f"invalid_board_record_order at {path}")',
    "        if record.operation_count != len(record.operations):",
    '            raise msgspec.ValidationError(f"operation_count_mismatch at {path}.operation_count")',
    "        total_operations += len(record.operations)",
    "        if isinstance(record, DimensionPlotRecord):",
    "            _validate_dimension_plot_record(record, path)",
    "    if value.total_operations != total_operations:",
    '        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")',
    "",
    "",
    "def _validate_dimension_plot_record(record: DimensionPlotRecord, path: str) -> None:",
    "    if not record.layers or record.layers != sorted(set(record.layers)):",
    '        raise msgspec.ValidationError(f"invalid_dimension at {path}.layers")',
    "    saw_text = False",
    "    marker_count = 0",
    "    for operation_index, operation in enumerate(record.operations):",
    "        operation_path = f'{path}.operations[{operation_index}]'",
    "        if operation.index != operation_index:",
    '            raise msgspec.ValidationError(f"operation_index_mismatch at {operation_path}.index")',
    "        if isinstance(operation, TextOperation):",
    "            if operation_index != 0 or saw_text:",
    '                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")',
    "            saw_text = True",
    "            layer = None if operation.layer is UNSET else operation.layer",
    "            if not operation.font_face or layer not in record.layers:",
    '                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")',
    "            _validate_board_text_payload(operation, operation_path)",
    "        elif isinstance(operation, ThickSegmentOperation):",
    "            layer = None if operation.layer is UNSET else operation.layer",
    "            layers = [] if operation.layers is UNSET else operation.layers",
    "            forbidden = (operation.role is not UNSET, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET)",
    "            if layer not in record.layers or any(forbidden):",
    '                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")',
    "        elif isinstance(operation, CircleOperation):",
    "            marker_count += 1",
    "            layer = None if operation.layer is UNSET else operation.layer",
    "            layers = [] if operation.layers is UNSET else operation.layers",
    "            forbidden = (operation.role is not UNSET, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET, operation.stroke_color is not UNSET, operation.fill_color is not UNSET, operation.line_style is not UNSET)",
    '            if record.dimension_type != "orthogonal" or marker_count > 1 or layer not in record.layers or operation.fill != "FILLED_SHAPE" or operation.diameter_nm != 200_000 or operation.width_nm != 0 or any(forbidden):',
    '                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")',
    "        else:",
    '            raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")',
    "",
    "",
    "def _validate_board_text_payload(operation: TextOperation, path: str) -> None:",
    "    markers = (operation.mirror, operation.text_as_polygons, operation.polyline_per_segment, operation.knockout)",
    "    if any(marker is not UNSET and marker is not True for marker in markers):",
    '        raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    "    if (operation.text_as_polygons is not UNSET) != (not operation.font_face):",
    '        raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    "    has_cache = operation.render_cache is not UNSET",
    "    polygons = [] if operation.render_cache_polygons is UNSET else operation.render_cache_polygons",
    "    if has_cache != (operation.render_cache_source is not UNSET) or has_cache != (operation.render_cache_exact is not UNSET) or has_cache == (not polygons):",
    '        raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    "    if not has_cache:",
    "        if operation.knockout is not UNSET:",
    '            raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    "        return",
    "    cache = operation.render_cache",
    '    if cache.schema != "kicad.render_cache.v1" or cache.unit != "nm" or cache.coordinate_space != "board" or cache.source != operation.render_cache_source or cache.text != operation.text or cache.angle != operation.orient_deg or cache.exact != operation.render_cache_exact or cache.knockout != operation.knockout:',
    '        raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    "    if len(cache.polygons) != len(polygons):",
    '        raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    "    for polygon, exterior in zip(cache.polygons, polygons):",
    "        if not polygon.contours or any(len(contour) < 3 for contour in polygon.contours) or polygon.contours[0] != exterior:",
    '            raise msgspec.ValidationError(f"invalid_board_text at {path}")',
    ...renderPythonBoardFootprintValidation(),
  ];
}

function renderPythonBoardFootprintValidation() {
  return [
    "",
    "",
    "def _validate_board_footprint_plot_record(record: BoardFootprintPlotRecord, path: str) -> None:",
    '    if record.object_id != record.library_link or not math.isfinite(record.placement.angle_deg):',
    '        raise msgspec.ValidationError(f"invalid_board_footprint at {path}")',
    "    operation_index = 0",
    "    pad_phase = False",
    "    last_key = None",
    "    while operation_index < len(record.operations):",
    "        operation = record.operations[operation_index]",
    "        operation_path = f'{path}.operations[{operation_index}]'",
    "        if isinstance(operation, BoardFootprintStartBlockOperation):",
    "            pad_phase = True",
    "            if operation_index + 2 >= len(record.operations) or not isinstance(record.operations[operation_index + 2], BoardFootprintEndBlockOperation):",
    '                raise msgspec.ValidationError(f"invalid_board_footprint at {operation_path}")',
    "            inner = record.operations[operation_index + 1]",
    "            end = record.operations[operation_index + 2]",
    "            _validate_board_footprint_header(operation, operation_index, 'StartBlock', operation_path)",
    "            _validate_board_footprint_header(inner, operation_index + 1, _board_footprint_expected_kind(inner), f'{path}.operations[{operation_index + 1}]')",
    "            _validate_board_footprint_header(end, operation_index + 2, 'EndBlock', f'{path}.operations[{operation_index + 2}]')",
    "            _validate_board_footprint_pad_block(record, operation, inner, operation_path)",
    "            operation_index += 3",
    "            continue",
    "        if pad_phase or isinstance(operation, BoardFootprintEndBlockOperation):",
    '            raise msgspec.ValidationError(f"invalid_board_footprint at {operation_path}")',
    "        key = _validate_board_footprint_child(record, operation, operation_index, operation_path)",
    "        if last_key is not None and last_key >= key:",
    '            raise msgspec.ValidationError(f"invalid_board_footprint_order at {operation_path}")',
    "        last_key = key",
    "        operation_index += 1",
    "",
    "",
    "def _validate_board_footprint_header(operation: object, index: int, kind: str, path: str) -> None:",
    "    if operation.index != index:",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_header at {path}")',
    "",
    "",
    "def _board_footprint_expected_kind(operation: object) -> str:",
    "    kinds = ((BoardFootprintThickSegmentOperation, 'ThickSegment'), (BoardFootprintArcThreePointOperation, 'ArcThreePoint'), (BoardFootprintCircleOperation, 'Circle'), (BoardFootprintRectOperation, 'Rect'), (BoardFootprintPlotPolyOperation, 'PlotPoly'), (BoardFootprintBezierCurveOperation, 'BezierCurve'), (BoardFootprintTextOperation, 'Text'), (BoardFootprintFlashPadCircleOperation, 'FlashPadCircle'), (BoardFootprintFlashPadOvalOperation, 'FlashPadOval'), (BoardFootprintFlashPadRectOperation, 'FlashPadRect'), (BoardFootprintFlashPadRoundRectOperation, 'FlashPadRoundRect'), (BoardFootprintFlashPadCustomOperation, 'FlashPadCustom'), (BoardFootprintFlashPadTrapezOperation, 'FlashPadTrapez'), (BoardFootprintStartBlockOperation, 'StartBlock'), (BoardFootprintEndBlockOperation, 'EndBlock'))",
    "    for operation_type, kind in kinds:",
    "        if isinstance(operation, operation_type):",
    "            return kind",
    '    raise msgspec.ValidationError("invalid_board_footprint_operation")',
    "",
    "",
    "def _validate_board_footprint_child(record: BoardFootprintPlotRecord, operation: object, index: int, path: str) -> tuple[int, int, int]:",
    "    allowed = (BoardFootprintThickSegmentOperation, BoardFootprintArcThreePointOperation, BoardFootprintCircleOperation, BoardFootprintRectOperation, BoardFootprintPlotPolyOperation, BoardFootprintTextOperation)",
    "    if not isinstance(operation, allowed):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_child at {path}")',
    "    _validate_board_footprint_header(operation, index, _board_footprint_expected_kind(operation), path)",
    "    metadata = (operation.label, operation.data_uuid, operation.data_ref, operation.object_id, operation.extra_attrs)",
    "    if any(value is UNSET for value in metadata):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_metadata at {path}")',
    "    attrs = operation.extra_attrs",
    "    layer = None if operation.layer is UNSET else operation.layer",
    "    layer_name = None if attrs.layer_name is UNSET else attrs.layer_name",
    "    if not operation.label or not operation.data_uuid or not operation.object_id or operation.data_ref != attrs.footprint_primitive or attrs.component != record.reference or attrs.component_uid != record.uuid or attrs.component_uuid != record.uuid or attrs.footprint != record.library_link or layer_name != layer or (attrs.layer_name is UNSET) != (attrs.layer_role is UNSET) or (layer is not None and attrs.layer_role != _board_footprint_layer_role(layer)):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_metadata at {path}")',
    "    _validate_board_footprint_child_shape(operation, attrs, path)",
    "    phases = {'property': 0, 'fp_text': 1, 'fp_text_box': 2, 'fp_line': 3, 'fp_arc': 4, 'fp_circle': 5, 'fp_rect': 6, 'fp_poly': 7}",
    "    sub_index = 0 if attrs.footprint_subop_index is UNSET else attrs.footprint_subop_index",
    "    return (phases[operation.data_ref], attrs.footprint_object_index, sub_index)",
    "",
    "",
    "def _validate_board_footprint_child_shape(operation: object, attrs: BoardFootprintChildAttrs, path: str) -> None:",
    "    data_ref = operation.data_ref",
    "    if isinstance(operation, BoardFootprintTextOperation):",
    "        valid_ref = data_ref in ('property', 'fp_text', 'fp_text_box')",
    "        valid_attrs = attrs.primitive == 'footprint-text' and attrs.footprint_text_role is not UNSET and attrs.footprint_graphic_kind is UNSET and ((data_ref == 'property') == (attrs.property_name is not UNSET)) and ((data_ref == 'fp_text') == (attrs.fp_text_type is not UNSET))",
    "        _validate_board_footprint_text(operation, path)",
    "    else:",
    "        expected = None",
    "        if isinstance(operation, BoardFootprintThickSegmentOperation): expected = 'text-box-border' if data_ref == 'fp_text_box' else 'line'",
    "        elif isinstance(operation, BoardFootprintArcThreePointOperation): expected = 'arc'",
    "        elif isinstance(operation, BoardFootprintCircleOperation): expected = 'circle'",
    "        elif isinstance(operation, BoardFootprintRectOperation): expected = 'text-box-border' if data_ref == 'fp_text_box' else 'rect'",
    "        elif isinstance(operation, BoardFootprintPlotPolyOperation): expected = 'poly'",
    "        valid_refs = {BoardFootprintThickSegmentOperation: ('fp_text_box', 'fp_line'), BoardFootprintArcThreePointOperation: ('fp_arc',), BoardFootprintCircleOperation: ('fp_circle',), BoardFootprintRectOperation: ('fp_text_box', 'fp_rect'), BoardFootprintPlotPolyOperation: ('fp_poly',)}",
    "        valid_ref = data_ref in valid_refs[type(operation)]",
    "        valid_attrs = attrs.primitive == 'footprint-graphic' and attrs.footprint_text_role is UNSET and attrs.property_name is UNSET and attrs.fp_text_type is UNSET and attrs.footprint_graphic_kind == expected",
    "    subop_required = data_ref in ('fp_text_box', 'fp_line', 'fp_arc')",
    "    if not valid_ref or not valid_attrs or ((attrs.footprint_subop_index is not UNSET) != subop_required):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_shape at {path}")',
    "",
    "",
    "def _board_footprint_layer_role(layer: str) -> str:",
    "    if layer.endswith('.Cu') or layer in ('*.Cu', 'F&B.Cu'): return 'copper'",
    "    if layer.endswith('.SilkS'): return 'silkscreen'",
    "    if layer.endswith('.Mask') or layer == '*.Mask': return 'soldermask'",
    "    if layer.endswith('.Paste'): return 'paste'",
    "    if layer.endswith('.Fab'): return 'fab'",
    "    if layer.endswith('.Courtyard'): return 'courtyard'",
    "    if layer == 'Edge.Cuts': return 'board-outline'",
    "    if layer == 'DRILLS': return 'drill'",
    "    if layer.endswith('.User') or layer.startswith('User.'): return 'user'",
    "    return 'other'",
    "",
    "",
    "def _validate_board_footprint_text(operation: BoardFootprintTextOperation, path: str) -> None:",
    "    if not math.isfinite(operation.orient_deg) or operation.mirror is not UNSET or operation.text_as_polygons is not UNSET or operation.polyline_per_segment is not UNSET or operation.knockout is False:",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")',
    "    has_cache = operation.render_cache is not UNSET",
    "    polygons = [] if operation.render_cache_polygons is UNSET else operation.render_cache_polygons",
    "    if has_cache != (operation.render_cache_source is not UNSET) or has_cache != (operation.render_cache_exact is not UNSET) or has_cache == (not polygons):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")',
    "    if not has_cache:",
    "        if operation.knockout is not UNSET: raise msgspec.ValidationError(f'invalid_board_footprint_cache at {path}')",
    "        return",
    "    cache = operation.render_cache",
    "    if cache.schema != 'kicad.render_cache.v1' or cache.unit != 'nm' or cache.coordinate_space != 'footprint_local' or cache.source != operation.render_cache_source or cache.text != operation.text or not math.isfinite(cache.angle) or cache.exact != operation.render_cache_exact or cache.knockout != operation.knockout or len(cache.polygons) != len(polygons):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")',
    "    for polygon, exterior in zip(cache.polygons, polygons):",
    "        if not polygon.contours or any(len(contour) < 3 for contour in polygon.contours) or polygon.contours[0] != exterior:",
    '            raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")',
    "",
    "",
    "def _validate_board_footprint_pad_block(record: BoardFootprintPlotRecord, start: BoardFootprintStartBlockOperation, inner: object, path: str) -> None:",
    "    attrs = start.extra_attrs",
    "    expected_component = record.reference if record.reference else UNSET",
    "    expected_uuid = record.uuid if record.uuid else UNSET",
    "    expected_footprint = record.library_link if record.library_link else UNSET",
    "    pad_number_valid = (attrs.pad_number == start.object_id) if attrs.pad_number is not UNSET else start.object_id == 'pad'",
    "    expected_designator = UNSET if attrs.pad_number is UNSET else (f'{record.reference}-{attrs.pad_number}' if record.reference else attrs.pad_number)",
    "    inner_layers_value = getattr(inner, 'layers', UNSET)",
    "    inner_layers = [] if inner_layers_value is UNSET else inner_layers_value",
    "    expected_layer_names = ','.join(inner_layers) if inner_layers else UNSET",
    "    common = attrs.component == expected_component and attrs.component_uid == expected_uuid and attrs.component_uuid == expected_uuid and attrs.footprint == expected_footprint and pad_number_valid and attrs.pad_designator == expected_designator and (attrs.pad_type is UNSET or bool(attrs.pad_type)) and (attrs.pad_shape is UNSET or bool(attrs.pad_shape)) and attrs.layer_names == expected_layer_names and start.label == start.data_uuid",
    "    metadata = tuple(getattr(inner, name, UNSET) for name in ('label', 'data_uuid', 'data_ref', 'object_id', 'extra_attrs'))",
    "    if not common or any(value is not UNSET for value in metadata):",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_pad at {path}")',
    "    if start.data_ref == 'pad':",
    "        hole_names = ('hole_owner', 'hole_kind', 'hole_plating', 'hole_render', 'hole_width_mm', 'hole_height_mm', 'hole_diameter_mm')",
    "        layers = [] if start.layers is UNSET else start.layers",
    "        valid = attrs.primitive == 'pad' and all(getattr(attrs, name) is UNSET for name in hole_names) and bool(layers) and isinstance(inner, (BoardFootprintFlashPadCircleOperation, BoardFootprintFlashPadOvalOperation, BoardFootprintFlashPadRectOperation, BoardFootprintFlashPadRoundRectOperation, BoardFootprintFlashPadCustomOperation, BoardFootprintFlashPadTrapezOperation)) and inner.layers == layers",
    "        if isinstance(inner, BoardFootprintFlashPadCircleOperation): valid = valid and inner.mask_margin_nm is not UNSET and inner.role is UNSET",
    "        if isinstance(inner, BoardFootprintFlashPadCustomOperation): valid = valid and (inner.polygon_widths_nm is UNSET or not inner.polygon_widths_nm or len(inner.polygon_widths_nm) == len(inner.polygons))",
    "    else:",
    "        round_hole = attrs.hole_kind == 'round' and attrs.hole_diameter_mm is not UNSET and attrs.hole_width_mm is UNSET and attrs.hole_height_mm is UNSET",
    "        slot_hole = attrs.hole_kind == 'slot' and attrs.hole_diameter_mm is UNSET and attrs.hole_width_mm is not UNSET and attrs.hole_height_mm is not UNSET",
    "        valid = attrs.primitive == 'pad-hole' and start.label.endswith(':hole') and attrs.hole_owner == start.label[:-5] and attrs.hole_plating in ('plated', 'non_plated') and attrs.hole_render == 'drill' and (round_hole or slot_hole) and isinstance(inner, (BoardFootprintCircleOperation, BoardFootprintThickSegmentOperation)) and inner.layer is UNSET and bool(inner.layers)",
    "        if valid and attrs.hole_plating == 'plated': valid = inner.role == 'pad_drill' and inner.mask_margin_nm is UNSET and inner.pad_size_x_nm is UNSET and inner.pad_size_y_nm is UNSET",
    "        elif valid: valid = inner.role == 'npth_hole' and inner.mask_margin_nm is not UNSET and inner.pad_size_x_nm is not UNSET and inner.pad_size_y_nm is not UNSET",
    "    if not valid:",
    '        raise msgspec.ValidationError(f"invalid_board_footprint_pad at {path}")',
  ];
}

function renderPythonShapingRecordValidation(functionName, typeName) {
  return [
    `_shaping_record_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _shaping_record_a0_decoder.decode(data)",
    "    validate_shaping_record_a0(value)",
    "    return value",
    "",
    "",
    `def validate_shaping_record_a0(value: ${typeName}) -> None:`,
    '    if value.schema != "kicad_monkey.shaping_record.a0" or value.type_ != "kicad_monkey.shaping_record" or value.version != "a0":',
    '        raise msgspec.ValidationError("unsupported_contract at $")',
    "    if not isinstance(value.comparison, ExactComparisonPolicy):",
    '        raise msgspec.ValidationError("invalid_comparison at $.comparison")',
    '    if value.input.text_index_unit != "utf8_byte_offset":',
    '        raise msgspec.ValidationError("invalid_text_index at $.input.text_index_unit")',
    "    _validate_font_text_identity(value.case_id, '$.case_id')",
    "    _validate_font_text_identity(value.input.font_id, '$.input.font_id')",
    "    _validate_font_hash(value.input.font_sha256, '$.input.font_sha256')",
    "    _validate_font_variations(value.input.variations, '$.input.variations')",
    "    if value.input.script is not UNSET and not _font_tag_valid(value.input.script):",
    '        raise msgspec.ValidationError("invalid_tag at $.input.script")',
    "    if value.input.language is not UNSET and not value.input.language:",
    '        raise msgspec.ValidationError("invalid_language at $.input.language")',
    "    char_starts: set[int] = set()",
    "    offset = 0",
    "    for char in value.input.text:",
    "        char_starts.add(offset)",
    "        offset += _font_utf8_len(char)",
    "    feature_endpoints = {*char_starts, offset}",
    "    feature_tags: set[str] = set()",
    "    for index, feature in enumerate(value.input.features):",
    "        if not _font_tag_valid(feature.tag):",
    '            raise msgspec.ValidationError(f"invalid_tag at $.input.features[{index}].tag")',
    "        if feature.tag in feature_tags:",
    '            raise msgspec.ValidationError(f"duplicate_feature_tag at $.input.features[{index}].tag")',
    "        feature_tags.add(feature.tag)",
    "        global_range = feature.start == 0 and feature.end == 4_294_967_295",
    "        bounded = feature.start <= feature.end and feature.start in feature_endpoints and feature.end in feature_endpoints",
    "        if not global_range and not bounded:",
    '            raise msgspec.ValidationError(f"invalid_text_index at $.input.features[{index}]")',
    "    for index, glyph in enumerate(value.glyphs):",
    "        if glyph.cluster not in char_starts:",
    '            raise msgspec.ValidationError(f"invalid_text_index at $.glyphs[{index}].cluster")',
  ];
}

function renderPythonOutlineVectorValidation(functionName, typeName) {
  return [
    `_outline_vector_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _outline_vector_a0_decoder.decode(data)",
    "    validate_outline_vector_a0(value)",
    "    return value",
    "",
    "",
    `def validate_outline_vector_a0(value: ${typeName}) -> None:`,
    '    if value.schema != "kicad_monkey.outline_vector.a0" or value.type_ != "kicad_monkey.outline_vector" or value.version != "a0":',
    '        raise msgspec.ValidationError("unsupported_contract at $")',
    '    if value.coordinate_format != "font_design_units_f64":',
    '        raise msgspec.ValidationError("unsupported_contract at $.coordinate_format")',
    "    _validate_font_text_identity(value.case_id, '$.case_id')",
    "    _validate_font_text_identity(value.font_id, '$.font_id')",
    "    _validate_font_hash(value.font_sha256, '$.font_sha256')",
    "    _validate_font_variations(value.variations, '$.variations')",
    "    if value.units_per_em <= 0:",
    '        raise msgspec.ValidationError("invalid_units_per_em at $.units_per_em")',
    "    comparison = value.coordinate_comparison",
    "    if isinstance(comparison, AbsoluteToleranceComparisonPolicy):",
    "        if not math.isfinite(comparison.absolute_tolerance) or comparison.absolute_tolerance < 0:",
    '            raise msgspec.ValidationError("invalid_comparison at $.coordinate_comparison")',
    "    elif not isinstance(comparison, ExactComparisonPolicy):",
    '        raise msgspec.ValidationError("invalid_comparison at $.coordinate_comparison")',
    "    for index, command in enumerate(value.commands):",
    "        if isinstance(command, (OutlineMoveTo, OutlineLineTo)):",
    "            coordinates = (command.x, command.y)",
    "        elif isinstance(command, OutlineQuadTo):",
    "            coordinates = (command.control_x, command.control_y, command.x, command.y)",
    "        elif isinstance(command, OutlineCurveTo):",
    "            coordinates = (command.control1_x, command.control1_y, command.control2_x, command.control2_y, command.x, command.y)",
    "        else:",
    "            coordinates = ()",
    "        if any(not math.isfinite(coordinate) for coordinate in coordinates):",
    '            raise msgspec.ValidationError(f"invalid_coordinate at $.commands[{index}]")',
  ];
}

function renderPythonFontBundleValidation(functionName, typeName) {
  return [
    "@dataclass(frozen=True, slots=True)",
    "class _ValidatedFontBundleA0:",
    `    manifest: ${typeName}`,
    "    id_index: dict[str, int]",
    "    alias_index: dict[str, int | None]",
    "",
    "",
    `_font_bundle_manifest_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    return _font_bundle_manifest_a0_decoder.decode(data)",
    "",
    "",
    `def validate_font_bundle_manifest_a0(`,
    `    value: ${typeName},`,
    "    buffers: list[bytes] | tuple[bytes, ...],",
    "    *,",
    "    max_fonts: int = 4_096,",
    "    max_font_bytes: int = 256 * 1024 * 1024,",
    "    max_total_font_bytes: int = 1024 * 1024 * 1024,",
    "    max_aliases_per_font: int = 4_096,",
    "    max_variations_per_font: int = 4_096,",
    "    max_metadata_string_bytes: int = 64 * 1024 * 1024,",
    ") -> _ValidatedFontBundleA0:",
    '    if value.schema != "kicad_monkey.font_bundle.a0" or value.type_ != "kicad_monkey.font_bundle" or value.version != "a0":',
    '        raise msgspec.ValidationError("unsupported_contract at $")',
    "    limits = (max_fonts, max_font_bytes, max_total_font_bytes, max_aliases_per_font, max_variations_per_font, max_metadata_string_bytes)",
    "    if any(limit < 0 for limit in limits):",
    '        raise msgspec.ValidationError("invalid_limit at $")',
    "    if len(value.fonts) > max_fonts:",
    '        raise msgspec.ValidationError("resource_limit at $.fonts")',
    "    if len(value.fonts) != len(buffers):",
    '        raise msgspec.ValidationError("buffer_count_mismatch at $.fonts")',
    "    ids: set[str] = set()",
    "    slots: set[int] = set()",
    "    id_index: dict[str, int] = {}",
    "    alias_index: dict[str, int | None] = {}",
    "    total_bytes = 0",
    "    metadata_string_bytes = 0",
    "    for index, font in enumerate(value.fonts):",
    '        path = f"$.fonts[{index}]"',
    "        if not font.id or font.id in ids:",
    '            raise msgspec.ValidationError(f"duplicate_font_id at {path}.id")',
    "        _validate_font_text_identity(font.id, f'{path}.id')",
    "        ids.add(font.id)",
    "        id_index[font.id] = index",
    "        if font.slot in slots:",
    '            raise msgspec.ValidationError(f"duplicate_font_slot at {path}.slot")',
    "        slots.add(font.slot)",
    "        if font.slot >= len(buffers):",
    '            raise msgspec.ValidationError(f"invalid_slot at {path}.slot")',
    "        if len(font.sha256) != 64 or any(char not in '0123456789abcdef' for char in font.sha256):",
    '            raise msgspec.ValidationError(f"invalid_hash at {path}.sha256")',
    "        if len(font.aliases) > max_aliases_per_font or len(font.variations) > max_variations_per_font:",
    '            raise msgspec.ValidationError(f"resource_limit at {path}")',
    "        if any(not alias for alias in font.aliases) or len(set(font.aliases)) != len(font.aliases):",
    '            raise msgspec.ValidationError(f"invalid_alias at {path}.aliases")',
    "        axes: set[str] = set()",
    "        for variation_index, variation in enumerate(font.variations):",
    "            axis = variation.axis",
    "            if len(axis) != 4 or any(ord(char) < 32 or ord(char) > 126 for char in axis) or not math.isfinite(variation.value) or axis in axes:",
    '                raise msgspec.ValidationError(f"invalid_variation at {path}.variations[{variation_index}]")',
    "            axes.add(axis)",
    "        strings = [font.id, font.sha256, *font.aliases, *(variation.axis for variation in font.variations)]",
    "        strings.extend(value for value in (font.family, font.style, font.postscript_name) if value is not UNSET)",
    "        metadata_string_bytes += sum(_font_utf8_len(value) for value in strings)",
    "        if metadata_string_bytes > max_metadata_string_bytes:",
    '            raise msgspec.ValidationError("resource_limit at $.fonts")',
    "        for alias in font.aliases:",
    "            if alias in alias_index and alias_index[alias] != index:",
    "                alias_index[alias] = None",
    "            else:",
    "                alias_index[alias] = index",
    "        buffer = buffers[font.slot]",
    "        if len(buffer) > max_font_bytes:",
    '            raise msgspec.ValidationError(f"resource_limit at {path}.slot")',
    "        total_bytes += len(buffer)",
    "        if total_bytes > max_total_font_bytes:",
    '            raise msgspec.ValidationError("resource_limit at $.fonts")',
    "    for index, font in enumerate(value.fonts):",
    "        if hashlib.sha256(buffers[font.slot]).hexdigest() != font.sha256:",
    '            path = f"$.fonts[{index}]"',
    '            raise msgspec.ValidationError(f"hash_mismatch at {path}.sha256")',
    "    return _ValidatedFontBundleA0(value, id_index, alias_index)",
    "",
    "",
    "def resolve_font_selection_a0(",
    "    bundle: _ValidatedFontBundleA0,",
    "    request: FontResolutionRequestA0,",
    "    *,",
    "    max_request_aliases: int = 4_096,",
    "    max_request_string_bytes: int = 16 * 1024 * 1024,",
    ") -> FontBundleEntry:",
    '    if request.schema != "kicad_monkey.font_resolution_request.a0" or request.type_ != "kicad_monkey.font_resolution_request" or request.version != "a0":',
    '        raise msgspec.ValidationError("unsupported_contract at $")',
    "    if max_request_aliases < 0 or max_request_string_bytes < 0:",
    '        raise msgspec.ValidationError("invalid_limit at $.selection")',
    "    if len(request.selection.aliases) > max_request_aliases:",
    '        raise msgspec.ValidationError("resource_limit at $.selection.aliases")',
    "    font_id = None if request.selection.font_id is UNSET else request.selection.font_id",
    "    request_strings = [*request.selection.aliases]",
    "    if font_id is not None:",
    "        _validate_font_text_identity(font_id, '$.selection.font_id')",
    "        request_strings.append(font_id)",
    "    if sum(_font_utf8_len(value) for value in request_strings) > max_request_string_bytes:",
    '        raise msgspec.ValidationError("resource_limit at $.selection")',
    "    if font_id == '':",
    '        raise msgspec.ValidationError("invalid_selection at $.selection.font_id")',
    "    if any(not alias for alias in request.selection.aliases) or len(set(request.selection.aliases)) != len(request.selection.aliases):",
    '        raise msgspec.ValidationError("invalid_selection at $.selection.aliases")',
    "    if font_id is not None:",
    "        if font_id in bundle.id_index:",
    "            return bundle.manifest.fonts[bundle.id_index[font_id]]",
    '        raise msgspec.ValidationError("missing_font at $.selection.font_id")',
    "    matched: int | None = None",
    "    for alias in request.selection.aliases:",
    "        if alias not in bundle.alias_index:",
    "            continue",
    "        target = bundle.alias_index[alias]",
    "        if target is None or (matched is not None and matched != target):",
    '            raise msgspec.ValidationError("ambiguous_font at $.selection.aliases")',
    "        matched = target",
    "    if matched is None:",
    '        raise msgspec.ValidationError("missing_font at $.selection")',
    "    return bundle.manifest.fonts[matched]",
    "",
    "",
    "def _font_utf8_len(value: str) -> int:",
    "    total = 0",
    "    for char in value:",
    "        codepoint = ord(char)",
    "        total += 1 if codepoint < 0x80 else 2 if codepoint < 0x800 else 3 if codepoint < 0x10000 else 4",
    "    return total",
    "",
    "",
    "def _validate_font_text_identity(value: str, path: str) -> None:",
    "    if not value or not value[0].isascii() or not value[0].isalnum() or any(",
    "        not char.isascii() or (not char.isalnum() and char not in '._:-') for char in value[1:]",
    "    ):",
    '        raise msgspec.ValidationError(f"invalid_text_id at {path}")',
    "",
    "",
    "def _font_tag_valid(value: str) -> bool:",
    "    return len(value) == 4 and all(char.isascii() and ' ' <= char <= '~' for char in value)",
    "",
    "",
    "def _validate_font_hash(value: str, path: str) -> None:",
    "    if len(value) != 64 or any(char not in '0123456789abcdef' for char in value):",
    '        raise msgspec.ValidationError(f"invalid_hash at {path}")',
    "",
    "",
    "def _validate_font_variations(value: list[FontVariationCoordinate], path: str) -> None:",
    "    axes: set[str] = set()",
    "    for index, variation in enumerate(value):",
    "        if not _font_tag_valid(variation.axis) or not math.isfinite(variation.value) or variation.axis in axes:",
    '            raise msgspec.ValidationError(f"invalid_variation at {path}[{index}]")',
    "        axes.add(variation.axis)",
  ];
}

function renderPythonSourceBundleValidation(functionName, typeName) {
  return [
    `_source_bundle_manifest_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _source_bundle_manifest_a0_decoder.decode(data)",
    "    for source in value.sources:",
    "        if int(source.source_bytes) > 18_446_744_073_709_551_615:",
    '            raise msgspec.ValidationError("source_bytes exceeds uint64")',
    "    return value",
  ];
}

function renderPythonPlotterValidation(functionName, typeName) {
  return [
    `_footprint_plot_document_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _footprint_plot_document_a0_decoder.decode(data)",
    "    validate_footprint_plot_document_a0(value)",
    "    return value",
    "",
    "",
    `def validate_footprint_plot_document_a0(value: ${typeName}) -> None:`,
    "    if len(value.records) != 1:",
    '        raise msgspec.ValidationError("invalid_footprint_document at $.records")',
    "    total_operations = 0",
    "    for record_index, record in enumerate(value.records):",
    "        if record.object_id != record.name:",
    '            raise msgspec.ValidationError(f"invalid_footprint_record at $.records[{record_index}]")',
    "        if record.operation_count != len(record.operations):",
    "            raise msgspec.ValidationError(",
    '                f"operation_count_mismatch at $.records[{record_index}].operation_count"',
    "            )",
    "        total_operations += len(record.operations)",
    "        for operation_index, operation in enumerate(record.operations):",
    '            path = f"$.records[{record_index}].operations[{operation_index}]"',
    "            if operation.index != operation_index:",
    '                raise msgspec.ValidationError(f"operation_index_mismatch at {path}.index")',
    "            if isinstance(operation, (ThickSegmentOperation, CircleOperation)):",
    "                _validate_shared_graphic_or_drill(operation, path)",
    "            elif isinstance(operation, TextOperation):",
    "                _validate_footprint_text(operation, path)",
    "            elif isinstance(operation, (ArcThreePointOperation, RectOperation, PlotPolyOperation, BezierCurveOperation)):",
    "                if operation.layer is UNSET or not operation.layer:",
    '                    raise msgspec.ValidationError(f"missing_layer at {path}")',
    "            elif isinstance(operation, (",
    "                FlashPadCircleOperation,",
    "                FlashPadOvalOperation,",
    "                FlashPadRectOperation,",
    "                FlashPadRoundRectOperation,",
    "                FlashPadCustomOperation,",
    "                FlashPadTrapezOperation,",
    "            )):",
    "                if not operation.layers:",
    '                    raise msgspec.ValidationError(f"missing_layers at {path}")',
    "                if isinstance(operation, FlashPadCircleOperation) and (",
    "                    operation.mask_margin_nm is UNSET or operation.role is not UNSET",
    "                ):",
    '                    raise msgspec.ValidationError(f"invalid_pad_operation at {path}")',
    "            else:",
    '                raise msgspec.ValidationError(f"invalid_footprint_operation at {path}")',
    "            if isinstance(operation, FlashPadCustomOperation):",
    "                widths = operation.polygon_widths_nm",
    "                if widths is not UNSET and widths and len(widths) != len(operation.polygons):",
    '                    raise msgspec.ValidationError(f"polygon_width_count_mismatch at {path}.polygon_widths_nm")',
    "    if value.total_operations != total_operations:",
    '        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")',
    "",
    "",
    "def _validate_footprint_text(operation: TextOperation, path: str) -> None:",
    "    forbidden = (",
    "        operation.mirror is not UNSET,",
    "        operation.text_as_polygons is not UNSET,",
    "        operation.polyline_per_segment is not UNSET,",
    "        operation.knockout is not UNSET,",
    "        operation.render_cache_polygons is not UNSET,",
    "        operation.render_cache is not UNSET,",
    "        operation.render_cache_source is not UNSET,",
    "        operation.render_cache_exact is not UNSET,",
    "    )",
    "    if operation.layer is UNSET or not operation.layer or any(forbidden):",
    '        raise msgspec.ValidationError(f"invalid_footprint_text at {path}")',
    "",
    "",
    "def _validate_shared_graphic_or_drill(operation: ThickSegmentOperation | CircleOperation, path: str) -> None:",
    "    layer = None if operation.layer is UNSET else operation.layer",
    "    role = None if operation.role is UNSET else operation.role",
    "    layers = [] if operation.layers is UNSET else operation.layers",
    "    has_mask = operation.mask_margin_nm is not UNSET",
    "    has_size_x = operation.pad_size_x_nm is not UNSET",
    "    has_size_y = operation.pad_size_y_nm is not UNSET",
    "    graphic = (",
    "        role is None and layer is not None and not layers",
    "        and not has_mask and not has_size_x and not has_size_y",
    "    )",
    "    pad_drill = (",
    '        role == "pad_drill" and layer is None and bool(layers)',
    "        and not has_mask and not has_size_x and not has_size_y",
    "    )",
    "    npth_hole = (",
    '        role == "npth_hole" and layer is None and bool(layers)',
    "        and has_mask and has_size_x and has_size_y",
    "    )",
    "    if not (graphic or pad_drill or npth_hole):",
    '        raise msgspec.ValidationError(f"conflicting_plotter_fields at {path}")',
  ];
}

function renderPythonSymbolPlotterValidation(functionName, typeName) {
  return [
    `_symbol_plot_document_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _symbol_plot_document_a0_decoder.decode(data)",
    "    validate_symbol_plot_document_a0(value)",
    "    return value",
    "",
    "",
    `def validate_symbol_plot_document_a0(value: ${typeName}) -> None:`,
    '    if value.schema != "kicad.plotter_ir.a0" or value.source_kind != "SYM" or value.coordinate_space.unit != "nm" or value.coordinate_space.y_axis != "down":',
    '        raise msgspec.ValidationError("invalid_symbol_document at $")',
    "    if not value.records or not isinstance(value.records[0], SymbolHeaderPlotRecord):",
    '        raise msgspec.ValidationError("missing_symbol_header at $.records[0]")',
    "    total_operations = 0",
    "    for record_index, record in enumerate(value.records):",
    "        if isinstance(record, SymbolHeaderPlotRecord):",
    "            if record_index != 0 or record.object_id != record.name or record.operation_count != 0 or record.operations:",
    '                raise msgspec.ValidationError(f"invalid_symbol_header at $.records[{record_index}]")',
    "        elif not record.object_id:",
    '            raise msgspec.ValidationError(f"invalid_symbol_record at $.records[{record_index}]")',
    "        if record.operation_count != len(record.operations):",
    '            raise msgspec.ValidationError(f"operation_count_mismatch at $.records[{record_index}].operation_count")',
    "        total_operations += len(record.operations)",
    "        for operation_index, operation in enumerate(record.operations):",
    '            path = f"$.records[{record_index}].operations[{operation_index}]"',
    "            if operation.index != total_operations - len(record.operations) + operation_index:",
    '                raise msgspec.ValidationError(f"operation_index_mismatch at {path}.index")',
    "            allowed = isinstance(operation, (ArcThreePointOperation, CircleOperation, RectOperation, PlotPolyOperation, BezierCurveOperation, TextOperation))",
    "            layer = None if not hasattr(operation, 'layer') or operation.layer is UNSET else operation.layer",
    "            if not allowed or (not isinstance(operation, TextOperation) and layer is not None):",
    '                raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")',
    "            if isinstance(operation, CircleOperation):",
    "                role = None if operation.role is UNSET else operation.role",
    "                layers = [] if operation.layers is UNSET else operation.layers",
    "                if role is not None or layers or operation.mask_margin_nm is not UNSET or operation.pad_size_x_nm is not UNSET or operation.pad_size_y_nm is not UNSET:",
    '                    raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")',
    "            if isinstance(operation, TextOperation):",
    "                forbidden = (",
    "                    layer is not None,",
    "                    operation.mirror is not UNSET,",
    "                    operation.text_as_polygons is not UNSET,",
    "                    operation.polyline_per_segment is not UNSET,",
    "                    operation.knockout is not UNSET,",
    "                    operation.render_cache_polygons is not UNSET,",
    "                    operation.render_cache is not UNSET,",
    "                    operation.render_cache_source is not UNSET,",
    "                    operation.render_cache_exact is not UNSET,",
    "                )",
    "                if any(forbidden):",
    '                    raise msgspec.ValidationError(f"invalid_symbol_text at {path}")',
    "    if value.total_operations != total_operations:",
    '        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")',
  ];
}

function renderPythonSchematicPlotterValidation(functionName, typeName) {
  return [
    `_schematic_plot_document_a0_decoder = msgspec.json.Decoder(${typeName})`,
    "",
    "",
    `def ${functionName}(data: bytes) -> ${typeName}:`,
    "    value = _schematic_plot_document_a0_decoder.decode(data)",
    "    validate_schematic_plot_document_a0(value)",
    "    return value",
    "",
    "",
    `def validate_schematic_plot_document_a0(value: ${typeName}) -> None:`,
    '    if value.schema != "kicad.plotter_ir.a0" or value.source_kind != "SCH" or value.coordinate_space.unit != "nm" or value.coordinate_space.y_axis != "down":',
    '        raise msgspec.ValidationError("invalid_schematic_document at $")',
    "    if not value.records or not isinstance(value.records[0], SchematicSheetHeaderPlotRecord):",
    '        raise msgspec.ValidationError("missing_sheet_header at $.records[0]")',
    "    phases = {SchematicSheetHeaderPlotRecord: 0, SchematicWirePlotRecord: 1, SchematicBusPlotRecord: 2, SchematicBusEntryPlotRecord: 3, SchematicJunctionPlotRecord: 4, SchematicNoConnectPlotRecord: 5}",
    "    previous_phase = -1",
    "    total_operations = 0",
    "    for record_index, record in enumerate(value.records):",
    "        path = f'$.records[{record_index}]'",
    "        phase = phases[type(record)]",
    "        if phase < previous_phase or (phase == 0 and record_index != 0):",
    '            raise msgspec.ValidationError(f"invalid_schematic_record_order at {path}")',
    "        previous_phase = phase",
    "        if record.object_id != record.uuid:",
    '            raise msgspec.ValidationError(f"invalid_schematic_record_identity at {path}")',
    "        if record.operation_count != len(record.operations):",
    '            raise msgspec.ValidationError(f"operation_count_mismatch at {path}.operation_count")',
    "        for operation_index, operation in enumerate(record.operations):",
    "            if operation.index != operation_index:",
    '                raise msgspec.ValidationError(f"operation_index_mismatch at {path}.operations[{operation_index}].index")',
    "        if isinstance(record, SchematicSheetHeaderPlotRecord):",
    "            _validate_schematic_sheet_header(value, record, path)",
    "        elif isinstance(record, (SchematicWirePlotRecord, SchematicBusPlotRecord, SchematicBusEntryPlotRecord)):",
    "            _validate_schematic_polyline_record(record, path)",
    "        elif isinstance(record, SchematicJunctionPlotRecord):",
    "            _validate_schematic_junction_record(record, path)",
    "        else:",
    "            _validate_schematic_no_connect_record(record, path)",
    "        total_operations += len(record.operations)",
    "    if value.total_operations != total_operations:",
    '        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")',
    "",
    "",
    "def _validate_schematic_sheet_header(value: SchematicPlotDocumentA0, record: SchematicSheetHeaderPlotRecord, path: str) -> None:",
    "    if value.canvas.width_nm != record.sheet_width_nm or value.canvas.height_nm != record.sheet_height_nm or record.sheet_width_nm <= 0 or record.sheet_height_nm <= 0:",
    '        raise msgspec.ValidationError(f"invalid_sheet_header at {path}")',
    "    if not record.operations or not isinstance(record.operations[0], RectOperation):",
    '        raise msgspec.ValidationError(f"invalid_sheet_background at {path}.operations[0]")',
    "    background = record.operations[0]",
    "    background_layer = None if background.layer is UNSET else background.layer",
    "    if (background.x1, background.y1, background.x2, background.y2) != (0, 0, record.sheet_width_nm, record.sheet_height_nm) or background.fill != 'FILLED_SHAPE' or background.width_nm != 100 or background.corner_radius_nm != 0 or background_layer is not None or background.stroke_color != '#F5F4EFFF' or background.fill_color != '#F5F4EFFF':",
    '        raise msgspec.ValidationError(f"invalid_sheet_background at {path}.operations[0]")',
    "    for operation_index, operation in enumerate(record.operations[1:], start=1):",
    "        operation_path = f'{path}.operations[{operation_index}]'",
    "        if not isinstance(operation, (RectOperation, PlotPolyOperation, TextOperation, PlotImageOperation)):",
    '            raise msgspec.ValidationError(f"invalid_worksheet_operation at {operation_path}")',
    "        layer = None if not hasattr(operation, 'layer') or operation.layer is UNSET else operation.layer",
    "        if layer is not None:",
    '            raise msgspec.ValidationError(f"invalid_worksheet_operation at {operation_path}")',
    "        if isinstance(operation, RectOperation) and (operation.fill != 'NO_FILL' or operation.width_nm < 152_400 or operation.corner_radius_nm != 0 or operation.stroke_color != '#840000FF' or operation.fill_color is not UNSET or operation.line_style is not UNSET):",
    '            raise msgspec.ValidationError(f"invalid_worksheet_rect at {operation_path}")',
    "        if isinstance(operation, PlotPolyOperation) and (len(operation.points) != 2 or operation.fill != 'NO_FILL' or operation.width_nm < 152_400 or operation.stroke_color != '#840000FF' or operation.fill_color is not UNSET or operation.line_style is not UNSET):",
    '            raise msgspec.ValidationError(f"invalid_worksheet_polyline at {operation_path}")',
    "        if isinstance(operation, TextOperation):",
    "            forbidden = (operation.mirror is not UNSET, operation.text_as_polygons is not UNSET, operation.polyline_per_segment is not UNSET, operation.knockout is not UNSET, operation.render_cache_polygons is not UNSET, operation.render_cache is not UNSET, operation.render_cache_source is not UNSET, operation.render_cache_exact is not UNSET)",
    "            if any(forbidden) or not math.isfinite(operation.orient_deg):",
    '                raise msgspec.ValidationError(f"invalid_worksheet_text at {operation_path}")',
    "        if isinstance(operation, PlotImageOperation) and (operation.image_format != 'png' or not math.isfinite(operation.scale) or operation.scale <= 0 or operation.width_nm < 0 or operation.height_nm < 0 or operation.stroke_color != '#840000FF' or not _valid_schematic_png_base64(operation.image_data_b64)):",
    '            raise msgspec.ValidationError(f"invalid_worksheet_image at {operation_path}")',
    "",
    "",
    "def _valid_schematic_png_base64(value: str) -> bool:",
    "    prefix = bytearray()",
    "    quartet: list[int] = []",
    "    ended = False",
    "    for character in value:",
    "        if character in ' \\t\\r\\n\\v\\f':",
    "            return False",
    "        if ended:",
    "            return False",
    "        code = ord(character)",
    "        if 65 <= code <= 90: sextet = code - 65",
    "        elif 97 <= code <= 122: sextet = code - 97 + 26",
    "        elif 48 <= code <= 57: sextet = code - 48 + 52",
    "        elif character == '+': sextet = 62",
    "        elif character == '/': sextet = 63",
    "        elif character == '=': sextet = 64",
    "        else: return False",
    "        quartet.append(sextet)",
    "        if len(quartet) != 4:",
    "            continue",
    "        if quartet[0] >= 64 or quartet[1] >= 64:",
    "            return False",
    "        if quartet[2] == 64:",
    "            if quartet[3] != 64 or quartet[1] & 0x0F:",
    "                return False",
    "            decoded_len = 1",
    "            ended = True",
    "        elif quartet[3] == 64:",
    "            if quartet[2] & 0x03:",
    "                return False",
    "            decoded_len = 2",
    "            ended = True",
    "        else:",
    "            decoded_len = 3",
    "        decoded = ((quartet[0] << 2) | (quartet[1] >> 4), ((quartet[1] << 4) | (quartet[2] >> 2)) & 0xFF, ((quartet[2] << 6) | quartet[3]) & 0xFF)",
    "        prefix.extend(decoded[:min(decoded_len, 33 - len(prefix))])",
    "        quartet.clear()",
    "    if quartet or len(prefix) < 33:",
    "        return False",
    "    width = int.from_bytes(prefix[16:20], 'big')",
    "    height = int.from_bytes(prefix[20:24], 'big')",
    "    return prefix[:8] == b'\\x89PNG\\r\\n\\x1a\\n' and prefix[8:12] == b'\\x00\\x00\\x00\\r' and prefix[12:16] == b'IHDR' and width > 0 and height > 0",
    "",
    "",
    "def _validate_schematic_polyline_record(record: SchematicWirePlotRecord | SchematicBusPlotRecord | SchematicBusEntryPlotRecord, path: str) -> None:",
    "    if len(record.operations) != 1 or not isinstance(record.operations[0], PlotPolyOperation):",
    '        raise msgspec.ValidationError(f"invalid_connectivity_record at {path}")',
    "    operation = record.operations[0]",
    "    layer = None if operation.layer is UNSET else operation.layer",
    "    if layer is not None or operation.fill != 'NO_FILL' or operation.width_nm < 0 or operation.stroke_color is UNSET or not operation.stroke_color or operation.line_style is UNSET or not operation.points:",
    '        raise msgspec.ValidationError(f"invalid_connectivity_polyline at {path}.operations[0]")',
    "    if isinstance(record, SchematicBusEntryPlotRecord) and len(operation.points) != 2:",
    '        raise msgspec.ValidationError(f"invalid_bus_entry at {path}.operations[0].points")',
    "",
    "",
    "def _validate_schematic_junction_record(record: SchematicJunctionPlotRecord, path: str) -> None:",
    "    if len(record.operations) != 1 or not isinstance(record.operations[0], CircleOperation):",
    '        raise msgspec.ValidationError(f"invalid_junction at {path}")',
    "    operation = record.operations[0]",
    "    layer = None if operation.layer is UNSET else operation.layer",
    "    role = None if operation.role is UNSET else operation.role",
    "    layers = [] if operation.layers is UNSET else operation.layers",
    "    forbidden = (role is not None, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET)",
    "    if layer is not None or any(forbidden) or operation.fill != 'FILLED_SHAPE' or operation.width_nm != 0 or operation.diameter_nm <= 0 or operation.stroke_color is UNSET or operation.fill_color is UNSET or operation.stroke_color != operation.fill_color:",
    '        raise msgspec.ValidationError(f"invalid_junction at {path}.operations[0]")',
    "    expected_color = '#009600FF' if record.color is UNSET or record.color is None else record.color",
    "    if expected_color != operation.stroke_color:",
    '        raise msgspec.ValidationError(f"invalid_junction_color at {path}.color")',
    "",
    "",
    "def _validate_schematic_no_connect_record(record: SchematicNoConnectPlotRecord, path: str) -> None:",
    "    if len(record.operations) != 2 or not all(isinstance(operation, PlotPolyOperation) for operation in record.operations):",
    '        raise msgspec.ValidationError(f"invalid_no_connect at {path}")',
    "    first, second = record.operations",
    "    for operation_index, operation in enumerate((first, second)):",
    "        layer = None if operation.layer is UNSET else operation.layer",
    "        if layer is not None or operation.fill != 'NO_FILL' or operation.width_nm <= 0 or operation.stroke_color != '#000084FF' or operation.line_style is not UNSET or len(operation.points) != 2:",
    '            raise msgspec.ValidationError(f"invalid_no_connect at {path}.operations[{operation_index}]")',
    "    if first.width_nm != second.width_nm or first.points[0][0] != second.points[0][0] or first.points[1][0] != second.points[1][0] or first.points[0][1] != second.points[1][1] or first.points[1][1] != second.points[0][1]:",
    '        raise msgspec.ValidationError(f"invalid_no_connect_geometry at {path}.operations")',
  ];
}

function renderPythonDeclaration(name, schema, tag = undefined) {
  if (Array.isArray(schema.enum)) {
    return [`${name} = Literal[${schema.enum.map(pythonLiteral).join(", ")}]`];
  }
  if (Array.isArray(schema.anyOf)) {
    return [`${name} = Union[${schema.anyOf.map(pythonForwardType).join(", ")}]`];
  }
  if (schema.type === "integer") {
    const constraints = [];
    if (Number.isSafeInteger(schema.minimum)) constraints.push(`ge=${schema.minimum}`);
    if (Number.isSafeInteger(schema.maximum)) constraints.push(`le=${schema.maximum}`);
    assert(constraints.length > 0, `${name}: unconstrained integer alias`);
    return [`${name} = Annotated[int, Meta(${constraints.join(", ")})]`];
  }
  if (schema.type === "number") {
    const constraints = [];
    if (Number.isFinite(schema.minimum)) constraints.push(`ge=${schema.minimum}`);
    if (Number.isFinite(schema.maximum)) constraints.push(`le=${schema.maximum}`);
    assert(constraints.length > 0, `${name}: unconstrained number alias`);
    return [`${name} = Annotated[float, Meta(${constraints.join(", ")})]`];
  }
  if (schema.type === "string") {
    const constraints = [];
    if (typeof schema.pattern === "string") {
      constraints.push(`pattern=${pythonLiteral(schema.pattern)}`);
    }
    if (Number.isSafeInteger(schema.minLength)) constraints.push(`min_length=${schema.minLength}`);
    if (Number.isSafeInteger(schema.maxLength)) constraints.push(`max_length=${schema.maxLength}`);
    assert(constraints.length > 0, `${name}: unconstrained string alias`);
    return [`${name} = Annotated[str, Meta(${constraints.join(", ")})]`];
  }
  if (schema.type === "array") {
    const itemType = pythonType(schema.items);
    const constraints = [];
    if (Number.isSafeInteger(schema.minItems)) constraints.push(`min_length=${schema.minItems}`);
    if (Number.isSafeInteger(schema.maxItems)) constraints.push(`max_length=${schema.maxItems}`);
    const listType = `list[${itemType}]`;
    return [
      constraints.length > 0
        ? `${name} = Annotated[${listType}, Meta(${constraints.join(", ")})]`
        : `${name} = ${listType}`,
    ];
  }
  const mapValues = schema.additionalProperties ?? schema.unevaluatedProperties;
  if (
    schema.type === "object"
    && mapValues !== undefined
    && mapValues !== false
    && !isFalseSchema(mapValues)
    && Object.keys(schema.properties ?? {}).length === 0
  ) {
    return [`${name} = dict[str, ${pythonType(mapValues)}]`];
  }
  assert(schema.type === "object", `${name}: expected object or enum`);
  if (Array.isArray(tag?.values)) {
    const lines = [];
    const members = [];
    for (const value of tag.values) {
      const member = `${name}${pascalCase(value)}`;
      members.push(member);
      if (lines.length > 0) lines.push("", "");
      lines.push(...renderPythonDeclaration(member, schema, { field: tag.field, value }));
    }
    return [...lines, "", "", `${name} = Union[${members.join(", ")}]`];
  }
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {}).filter(
    ([property]) => property !== tag?.field,
  );
  const ordered = [
    ...properties.filter(([property]) => required.has(property)),
    ...properties.filter(([property]) => !required.has(property)),
  ];
  const tagOptions = tag
    ? `, tag=${pythonLiteral(tag.value)}, tag_field=${pythonLiteral(tag.field)}`
    : "";
  const lines = [`class ${name}(Struct, forbid_unknown_fields=True, frozen=True${tagOptions}):`];
  if (ordered.length === 0) return [...lines, "    pass"];
  for (const [property, propertySchema] of ordered) {
    const sanitized = property.replace(/[^A-Za-z0-9_]/gu, "_");
    const identifier = /^[A-Za-z_]/u.test(sanitized) ? sanitized : `_${sanitized}`;
    const pythonName = ["type", "float", "extends"].includes(identifier)
      ? `${identifier}_`
      : identifier;
    const annotation = pythonType(propertySchema);
    const rename = pythonName === property ? "" : `, name=${pythonLiteral(property)}`;
    if (required.has(property)) {
      lines.push(
        rename
          ? `    ${pythonName}: ${annotation} = field(${rename.slice(2)})`
          : `    ${pythonName}: ${annotation}`,
      );
    } else {
      lines.push(`    ${pythonName}: ${annotation} | UnsetType = field(default=UNSET${rename})`);
    }
  }
  return lines;
}

function pythonType(schema) {
  if (typeof schema.$ref === "string") return schema.$ref.split("/").at(-1);
  if ("const" in schema) return `Literal[${pythonLiteral(schema.const)}]`;
  if (
    Array.isArray(schema.anyOf)
    && Object.keys(schema).every((key) => key === "anyOf" || key === "description")
  ) {
    const nullArm = schema.anyOf.findIndex((arm) => arm?.type === "null");
    if (nullArm !== -1 && schema.anyOf.length === 2) {
      return `${pythonType(schema.anyOf[1 - nullArm])} | None`;
    }
    return `Union[${schema.anyOf.map(pythonType).join(", ")}]`;
  }
  if (schema.type === "string") {
    const constraints = [];
    if (typeof schema.pattern === "string") {
      constraints.push(`pattern=${pythonLiteral(schema.pattern)}`);
    }
    if (Number.isSafeInteger(schema.minLength)) constraints.push(`min_length=${schema.minLength}`);
    if (Number.isSafeInteger(schema.maxLength)) constraints.push(`max_length=${schema.maxLength}`);
    return constraints.length > 0
      ? `Annotated[str, Meta(${constraints.join(", ")})]`
      : "str";
  }
  if (schema.type === "number") {
    const constraints = [];
    if (Number.isFinite(schema.minimum)) constraints.push(`ge=${schema.minimum}`);
    if (Number.isFinite(schema.maximum)) constraints.push(`le=${schema.maximum}`);
    return constraints.length > 0
      ? `Annotated[float, Meta(${constraints.join(", ")})]`
      : "float";
  }
  if (schema.type === "integer") {
    const constraints = [];
    if (Number.isSafeInteger(schema.minimum)) constraints.push(`ge=${schema.minimum}`);
    if (Number.isSafeInteger(schema.maximum)) constraints.push(`le=${schema.maximum}`);
    return constraints.length > 0
      ? `Annotated[int, Meta(${constraints.join(", ")})]`
      : "int";
  }
  if (schema.type === "boolean") return "bool";
  if (schema.type === "array" && Array.isArray(schema.prefixItems)) {
    return `tuple[${schema.prefixItems.map(pythonType).join(", ")}]`;
  }
  if (schema.type === "array") return `list[${pythonType(schema.items)}]`;
  const mapValues = schema.additionalProperties ?? schema.unevaluatedProperties;
  if (schema.type === "object" && mapValues && !isFalseSchema(mapValues)) {
    return `dict[str, ${pythonType(mapValues)}]`;
  }
  fail(`unsupported Python schema: ${JSON.stringify(schema)}`);
}

function pythonForwardType(schema) {
  if (typeof schema.$ref === "string") return pythonLiteral(schema.$ref.split("/").at(-1));
  return pythonType(schema);
}

function projectSchema(value) {
  if (Array.isArray(value)) return value.map(projectSchema);
  if (value === null || typeof value !== "object") return value;
  for (const [key, child] of Object.entries(value)) value[key] = projectSchema(child);
  if (value.unevaluatedProperties !== undefined) {
    value.additionalProperties = isFalseSchema(value.unevaluatedProperties)
      ? false
      : value.unevaluatedProperties;
    delete value.unevaluatedProperties;
  }
  return value;
}

function isFalseSchema(value) {
  return value === false || (value?.not && Object.keys(value).length === 1 && Object.keys(value.not).length === 0);
}

async function emit(outputPath, content) {
  if (check) {
    const current = await readFile(outputPath, "utf8").catch(() => undefined);
    assert(current === content, `stale generated binding: ${path.relative(root, outputPath)}`);
  } else {
    await mkdir(path.dirname(outputPath), { recursive: true });
    await writeFile(outputPath, content, "utf8");
  }
}

function pythonLiteral(value) {
  if (typeof value === "string") return JSON.stringify(value);
  if (value === null) return "None";
  return String(value);
}

function pascalCase(value) {
  return value
    .split(/[^a-zA-Z0-9]+/u)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join("");
}

function snakeCase(value) {
  return value.replace(/([a-z0-9])([A-Z])/gu, "$1_$2").replace(/[^a-zA-Z0-9]+/gu, "_").toLowerCase();
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function fail(message) {
  throw new Error(message);
}
