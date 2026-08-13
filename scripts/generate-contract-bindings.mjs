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
      const tag = target?.properties?.kind?.const;
      if (typeof name === "string" && typeof tag === "string") {
        taggedStructs.set(name, { field: "kind", value: tag });
      }
    }
  }

  const lines = [
    '"""Generated strict msgspec transport bindings. Do not edit."""',
    "",
    "from __future__ import annotations",
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
    } else {
      lines.push(`${functionName} = msgspec.json.Decoder(${typeName}).decode`);
    }
  }
  const exported = [
    ...definitions.keys(),
    ...roots.map(([, typeName]) => typeName),
    ...roots.map(([, typeName]) => `decode_${snakeCase(typeName.replace(/^SExpression/u, "sexpr_"))}`),
    "validate_footprint_plot_document_a0",
  ];
  lines.push("", "", "__all__ = (", ...exported.map((name) => `    ${pythonLiteral(name)},`), ")", "");
  return lines.join("\n");
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
    "    total_operations = 0",
    "    for record_index, record in enumerate(value.records):",
    "        if record.operation_count != len(record.operations):",
    "            raise msgspec.ValidationError(",
    '                f"operation_count_mismatch at $.records[{record_index}].operation_count"',
    "            )",
    "        total_operations += len(record.operations)",
    "        for operation_index, operation in enumerate(record.operations):",
    '            path = f"$.records[{record_index}].operations[{operation_index}]"',
    "            if isinstance(operation, (ThickSegmentOperation, CircleOperation)):",
    "                _validate_shared_graphic_or_drill(operation, path)",
    "            elif isinstance(operation, (",
    "                FlashPadCircleOperation,",
    "                FlashPadOvalOperation,",
    "                FlashPadRectOperation,",
    "                FlashPadRoundRectOperation,",
    "                FlashPadCustomOperation,",
    "                FlashPadTrapezOperation,",
    "            )) and not operation.layers:",
    '                raise msgspec.ValidationError(f"missing_layers at {path}")',
    "            if isinstance(operation, FlashPadCustomOperation):",
    "                widths = operation.polygon_widths_nm",
    "                if widths is not UNSET and widths and len(widths) != len(operation.polygons):",
    '                    raise msgspec.ValidationError(f"polygon_width_count_mismatch at {path}.polygon_widths_nm")',
    "    if value.total_operations != total_operations:",
    '        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")',
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
    const pythonName = ["type", "float"].includes(property) ? `${property}_` : property;
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
  if (schema.type === "string") return "str";
  if (schema.type === "number") return "float";
  if (schema.type === "integer") return "int";
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

function snakeCase(value) {
  return value.replace(/([a-z0-9])([A-Z])/gu, "$1_$2").replace(/[^a-zA-Z0-9]+/gu, "_").toLowerCase();
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function fail(message) {
  throw new Error(message);
}
