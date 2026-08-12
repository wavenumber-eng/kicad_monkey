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

const SCHEMAS: [(&str, &str); 4] = [
    ("BuildRequest.json", "build_request.rs"),
    ("BuildResult.json", "build_result.rs"),
    ("ScanRequest.json", "scan_request.rs"),
    ("ScanResult.json", "scan_result.rs"),
];

fn main() -> Result<()> {
    let check = env::args().skip(1).any(|argument| argument == "--check");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let schema_root = root.join("contracts/generated/schema");
    let output_root = root.join("src/rs/kicad-monkey-contracts/src/generated");
    let mut expected = BTreeMap::new();

    for (schema_name, output_name) in SCHEMAS {
        let schema_path = schema_root.join(schema_name);
        let mut schema: Value = serde_json::from_slice(
            &fs::read(&schema_path).with_context(|| format!("read {}", schema_path.display()))?,
        )?;
        project_for_typify(&mut schema);
        let generated = generate(schema)?;
        expected.insert(output_root.join(output_name), generated);
    }

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

fn generate(value: Value) -> Result<String> {
    let schema: RootSchema = serde_json::from_value(value)?;
    let mut settings = TypeSpaceSettings::default();
    settings.with_struct_builder(false);
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

fn project_for_typify(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.remove("$schema");
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
