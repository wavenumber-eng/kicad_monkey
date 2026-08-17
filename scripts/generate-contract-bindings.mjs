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
    assert(!/(?:[:<]\s*any\b|\bany\[\])/u.test(source), `${outputName}: forbidden any`);
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
      const encoded = JSON.stringify(definition);
      if (definitions.has(name)) {
        assert(definitions.get(name).encoded === encoded, `${name}: conflicting definitions`);
      } else {
        definitions.set(name, { encoded, schema: definition });
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
    } else if (typeName === "SymbolPlotDocumentA0") {
      lines.push(...renderPythonSymbolPlotterValidation(functionName, typeName));
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
    "resolve_font_selection_a0",
    "validate_font_bundle_manifest_a0",
    "validate_outline_vector_a0",
    "validate_shaping_record_a0",
    "validate_symbol_plot_document_a0",
  ];
  lines.push("", "", "__all__ = (", ...exported.map((name) => `    ${pythonLiteral(name)},`), ")", "");
  return lines.join("\n");
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
    "    if not value.records or not isinstance(value.records[0], SymbolHeaderPlotRecord):",
    '        raise msgspec.ValidationError("missing_symbol_header at $.records[0]")',
    "    total_operations = 0",
    "    for record_index, record in enumerate(value.records):",
    "        if isinstance(record, SymbolHeaderPlotRecord):",
    "            if record_index != 0 or record.operation_count != 0 or record.operations:",
    '                raise msgspec.ValidationError(f"invalid_symbol_header at $.records[{record_index}]")',
    "        elif record.operation_count != len(record.operations):",
    '            raise msgspec.ValidationError(f"operation_count_mismatch at $.records[{record_index}].operation_count")',
    "        total_operations += len(record.operations)",
    "        for operation_index, operation in enumerate(record.operations):",
    '            path = f"$.records[{record_index}].operations[{operation_index}]"',
    "            allowed = isinstance(operation, (ArcThreePointOperation, CircleOperation, RectOperation, PlotPolyOperation, BezierCurveOperation))",
    "            layer = None if not hasattr(operation, 'layer') or operation.layer is UNSET else operation.layer",
    "            if not allowed or layer is not None:",
    '                raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")',
    "            if isinstance(operation, CircleOperation):",
    "                role = None if operation.role is UNSET else operation.role",
    "                layers = [] if operation.layers is UNSET else operation.layers",
    "                if role is not None or layers or operation.mask_margin_nm is not UNSET or operation.pad_size_x_nm is not UNSET or operation.pad_size_y_nm is not UNSET:",
    '                    raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")',
    "    if value.total_operations != total_operations:",
    '        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")',
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
  if (Array.isArray(schema.anyOf) && Object.keys(schema).length === 1) {
    const nullArm = schema.anyOf.findIndex((arm) => arm?.type === "null");
    if (nullArm !== -1 && schema.anyOf.length === 2) {
      return `${pythonType(schema.anyOf[1 - nullArm])} | None`;
    }
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
  if (isFalseSchema(value.unevaluatedProperties)) {
    value.additionalProperties = false;
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
