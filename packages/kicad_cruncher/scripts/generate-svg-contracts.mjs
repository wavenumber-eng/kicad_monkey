// Cruncher-owned TypeSpec -> JSON Schema / TypeScript / Rust. No Monkey catalog edits.
import { readFile, mkdir, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { compile } from "json-schema-to-typescript";

const pkg = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const root = path.resolve(pkg, "../..");
const check = process.argv.includes("--check");
execFileSync(process.execPath, [
  path.join(root, "node_modules/@typespec/compiler/cmd/tsp.js"),
  "compile", path.join(pkg, "src/tsp/kicad_cruncher/main.tsp"),
  "--config", path.join(pkg, "tspconfig.yaml"),
], { cwd: root, stdio: "inherit" });
const emitted = path.join(pkg, "temp/contracts/PcbSvgConfig.json");
const schemaText = await readFile(emitted, "utf8");
await emit("docs/contracts/pcb_svg_config.a0.schema.json", schemaText);
await emit("src/ts/contracts/pcb-svg-config.ts", await compile(
  JSON.parse(schemaText), "PcbSvgConfig", {
    bannerComment: "/* Generated from Cruncher TypeSpec. Do not edit. */",
    cwd: path.dirname(emitted),
  },
));
execFileSync("cargo", ["run", "--locked", "-p", "kicad-monkey-codegen", "--",
  "--standalone", emitted,
  path.join(pkg, "src/rs/kicad-cruncher-cli/src/pcb_svg/generated.rs"),
  ...(check ? ["--check"] : []),
], { cwd: root, stdio: "inherit" });

async function emit(relative, text) {
  const output = path.join(pkg, relative);
  if (check) {
    if (await readFile(output, "utf8") !== text) throw new Error(`Stale contract: ${relative}`);
  } else {
    await mkdir(path.dirname(output), { recursive: true });
    await writeFile(output, text, "utf8");
  }
}
