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
  ["FootprintPlotDocument.json", "FootprintPlotterIRDocumentSliceA0", "footprint-plot-document.ts"],
  ["FootprintPlotRequest.json", "FootprintPlotterIRRequestA0", "footprint-plot-request.ts"],
  ["FootprintPlotResult.json", "FootprintPlotterIRResultA0", "footprint-plot-result.ts"],
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

  const lines = [
    '"""Generated strict msgspec transport bindings. Do not edit."""',
    "",
    "from __future__ import annotations",
    "",
    "from typing import Literal",
    "",
    "import msgspec",
    "from msgspec import UNSET, Struct, UnsetType, field",
  ];
  for (const [name, value] of definitions) {
    lines.push("", "", ...renderPythonDeclaration(name, value.schema));
  }
  for (const [file, typeName] of roots) {
    lines.push("", "", ...renderPythonDeclaration(typeName, schemas.get(file)));
  }
  lines.push("", "");
  for (const [, typeName] of roots) {
    const functionName = `decode_${snakeCase(typeName.replace(/^SExpression/u, "sexpr_"))}`;
    lines.push(`${functionName} = msgspec.json.Decoder(${typeName}).decode`);
  }
  const exported = [
    ...definitions.keys(),
    ...roots.map(([, typeName]) => typeName),
    ...roots.map(([, typeName]) => `decode_${snakeCase(typeName.replace(/^SExpression/u, "sexpr_"))}`),
  ];
  lines.push("", "", "__all__ = (", ...exported.map((name) => `    ${pythonLiteral(name)},`), ")", "");
  return lines.join("\n");
}

function renderPythonDeclaration(name, schema) {
  if (Array.isArray(schema.enum)) {
    return [`${name} = Literal[${schema.enum.map(pythonLiteral).join(", ")}]`];
  }
  assert(schema.type === "object", `${name}: expected object or enum`);
  const required = new Set(schema.required ?? []);
  const properties = Object.entries(schema.properties ?? {});
  const ordered = [
    ...properties.filter(([property]) => required.has(property)),
    ...properties.filter(([property]) => !required.has(property)),
  ];
  const lines = [`class ${name}(Struct, forbid_unknown_fields=True, frozen=True):`];
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
  if (schema.type === "array") return `list[${pythonType(schema.items)}]`;
  fail(`unsupported Python schema: ${JSON.stringify(schema)}`);
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
