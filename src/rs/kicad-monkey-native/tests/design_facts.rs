use kicad_monkey_contracts::{
    decode_native_design_facts_result_a0, decode_native_error_a0, decode_native_handshake_a0,
};
use kicad_monkey_native::{
    ENGINE_VERSION, MAX_REQUEST_BYTES, NativeErrorKind, PROTOCOL_VERSION, execute_request_bytes,
    handshake, serialize_error,
};
use serde_json::{Value, json};
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const PROJECT: &[u8] = b"{}";
const ROOT: &[u8] = br#"(kicad_sch
  (uuid structural-root)
  (lib_symbols
    (symbol "Demo:One"
      (symbol "Demo:One_1_1"
        (pin passive line (at 0 0 0) (name "P") (number "1")))))
  (sheet (uuid child-sheet)
    (property "Sheetname" "Child")
    (property "Sheetfile" "child.kicad_sch")
    (pin "SIG" input (at 20 0 180) (uuid sheet-pin)))
  (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid root-symbol)
    (property "Reference" "R1") (property "Value" "One")))"#;
const CHILD: &[u8] = br#"(kicad_sch
  (uuid structural-child)
  (lib_symbols
    (symbol "Demo:One"
      (symbol "Demo:One_1_1"
        (pin passive line (at 0 0 0) (name "P") (number "1")))))
  (hierarchical_label "SIG" (shape input) (at 20 0 0) (uuid child-port))
  (symbol (lib_id "Demo:One") (lib_name "Demo:One") (at 0 0 0) (uuid child-symbol)
    (property "Reference" "C1") (property "Value" "One")))"#;

#[test]
fn handshake_and_valid_design_facts_are_strict_and_versioned() {
    let handshake_bytes = serde_json::to_vec(&handshake()).expect("handshake JSON");
    decode_native_handshake_a0(&handshake_bytes).expect("generated handshake contract");
    let handshake: Value = serde_json::from_slice(&handshake_bytes).expect("handshake value");
    assert_eq!(handshake["type"], "kicad_monkey.native.handshake");
    assert_eq!(handshake["version"], PROTOCOL_VERSION);
    assert_eq!(handshake["engine_version"], ENGINE_VERSION);
    assert_eq!(handshake["operations"], json!(["design-facts"]));

    let fixture = Fixture::new();
    let output =
        execute_request_bytes(&fixture.request(8 * 1024 * 1024, None)).expect("valid design facts");
    decode_native_design_facts_result_a0(&output).expect("generated result contract");
    let result: Value = serde_json::from_slice(&output).expect("result JSON");
    assert_eq!(result["type"], "kicad_monkey.native.design_facts.result");
    assert_eq!(result["version"], "a0");
    assert_eq!(result["engine_version"], ENGINE_VERSION);
    assert_eq!(
        result["compiled_schematic_graph"]["schema"],
        "kicad_monkey.compiled_schematic_graph.a0"
    );
    assert_eq!(result["kicad_netlist_version"], "E");
    assert!(
        result["kicad_netlist"]
            .as_str()
            .expect("netlist string")
            .contains("(version \"E\"")
    );
}

#[test]
fn emitted_errors_satisfy_the_generated_contract() {
    let fixture = Fixture::new();
    let mut request = fixture.request_value(8 * 1024 * 1024);
    request["limits"]["max_output_bytes"] = json!("01");
    let error = execute_request_bytes(&serde_json::to_vec(&request).expect("request JSON"))
        .expect_err("noncanonical limit");
    assert_eq!(error.kind, NativeErrorKind::Request);
    decode_native_error_a0(&serialize_error(&error)).expect("generated error contract");
}

#[test]
fn malformed_trailing_and_unknown_requests_fail_closed() {
    let fixture = Fixture::new();
    assert!(execute_request_bytes(b"{").is_err());

    let mut trailing = fixture.request(8 * 1024 * 1024, None);
    trailing.extend_from_slice(b"[]");
    assert!(execute_request_bytes(&trailing).is_err());

    let unknown = fixture.request(8 * 1024 * 1024, Some(("unknown", json!(true))));
    assert!(execute_request_bytes(&unknown).is_err());

    let wrong_version = fixture.request_value(8 * 1024 * 1024);
    let mut wrong_version = wrong_version.as_object().expect("request object").clone();
    wrong_version.insert("version".to_owned(), json!("a1"));
    assert!(
        execute_request_bytes(&serde_json::to_vec(&wrong_version).expect("request JSON")).is_err()
    );

    let mut wrong_manifest = fixture.request_value(8 * 1024 * 1024);
    wrong_manifest["manifest"]["version"] = json!("a1");
    assert!(
        execute_request_bytes(&serde_json::to_vec(&wrong_manifest).expect("request JSON")).is_err()
    );
}

#[test]
fn request_byte_limit_accepts_exact_length_and_rejects_one_over() {
    let exact = vec![b' '; MAX_REQUEST_BYTES];
    let exact_error = execute_request_bytes(&exact).expect_err("exact bytes reach JSON decoding");
    assert_eq!(exact_error.kind, NativeErrorKind::Request);

    let one_over = vec![b' '; MAX_REQUEST_BYTES + 1];
    let over_error = execute_request_bytes(&one_over).expect_err("one over fixed request ceiling");
    assert_eq!(over_error.kind, NativeErrorKind::ResourceLimit);
}

#[test]
fn path_escape_and_incomplete_slots_are_rejected() {
    let fixture = Fixture::new();
    let mut escaped = fixture.request_value(8 * 1024 * 1024);
    escaped["file_slots"][1]["path"] = json!("../root.kicad_sch");
    let error = execute_request_bytes(&serde_json::to_vec(&escaped).expect("request JSON"))
        .expect_err("path escape");
    assert!(error.message.contains("relative path") || error.message.contains("non-normal"));

    let mut incomplete = fixture.request_value(8 * 1024 * 1024);
    incomplete["file_slots"]
        .as_array_mut()
        .expect("slots")
        .pop();
    assert!(
        execute_request_bytes(&serde_json::to_vec(&incomplete).expect("request JSON")).is_err()
    );
}

#[test]
fn source_symlink_cannot_escape_the_bundle_capability() {
    let fixture = Fixture::new();
    let outside = fixture.root.with_extension("outside");
    fs::create_dir(&outside).expect("outside root");
    fs::write(outside.join("root.kicad_sch"), ROOT).expect("outside source");
    let link = fixture.root.join("escape-dir");
    create_directory_link(&outside, &link).expect("create source escape link");
    let mut escaped = fixture.request_value(8 * 1024 * 1024);
    escaped["file_slots"][1]["path"] = json!("escape-dir/root.kicad_sch");

    let error = execute_request_bytes(&serde_json::to_vec(&escaped).expect("request JSON"))
        .expect_err("symlink escape");

    assert!(matches!(
        error.kind,
        NativeErrorKind::Io | NativeErrorKind::Path
    ));
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn source_and_output_limits_accept_exact_and_reject_one_under() {
    let fixture = Fixture::new();
    let largest = [PROJECT.len(), ROOT.len(), CHILD.len()]
        .into_iter()
        .max()
        .expect("sources");
    execute_request_bytes(&fixture.request_with_source_limit(8 * 1024 * 1024, largest))
        .expect("exact source ceiling");
    assert!(
        execute_request_bytes(&fixture.request_with_source_limit(8 * 1024 * 1024, largest - 1))
            .is_err()
    );

    let first =
        execute_request_bytes(&fixture.request(8 * 1024 * 1024, None)).expect("measure output");
    let exact = execute_request_bytes(&fixture.request(first.len(), None)).expect("exact output");
    assert_eq!(exact.len(), first.len());
    assert!(execute_request_bytes(&fixture.request(first.len() - 1, None)).is_err());
}

#[test]
fn process_failure_publishes_no_partial_stdout() {
    let fixture = Fixture::new();
    let first =
        execute_request_bytes(&fixture.request(8 * 1024 * 1024, None)).expect("measure output");
    let mut child = Command::new(env!("CARGO_BIN_EXE_kicad-monkey-native"))
        .arg("design-facts")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn native transport");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(&fixture.request(first.len() - 1, None))
        .expect("write request");
    let output = child.wait_with_output().expect("native output");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("structured stderr");
    assert_eq!(error["type"], "kicad_monkey.native.error");
    assert_eq!(error["kind"], "resource_limit");
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let ordinal = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "kicad_monkey_native_{}_{}",
            std::process::id(),
            ordinal
        ));
        fs::create_dir(&root).expect("create fixture root");
        fs::write(root.join("demo.kicad_pro"), PROJECT).expect("project source");
        fs::write(root.join("root.kicad_sch"), ROOT).expect("root source");
        fs::write(root.join("child.kicad_sch"), CHILD).expect("child source");
        Self { root }
    }

    fn request(&self, output_limit: usize, extra: Option<(&str, Value)>) -> Vec<u8> {
        let mut request = self.request_value(output_limit);
        if let Some((name, value)) = extra {
            request
                .as_object_mut()
                .expect("request object")
                .insert(name.to_owned(), value);
        }
        serde_json::to_vec(&request).expect("request JSON")
    }

    fn request_with_source_limit(&self, output_limit: usize, source_limit: usize) -> Vec<u8> {
        let mut request = self.request_value(output_limit);
        request["limits"]["max_source_bytes"] = json!(source_limit.to_string());
        serde_json::to_vec(&request).expect("request JSON")
    }

    fn request_value(&self, output_limit: usize) -> Value {
        json!({
            "type": "kicad_monkey.native.design_facts.request",
            "version": "a0",
            "bundle_root": self.root_string(),
            "manifest": {
                "schema": "kicad_monkey.source_bundle_manifest.a0",
                "type": "kicad_monkey.source_bundle_manifest",
                "version": "a0",
                "root_schematic_path": "root.kicad_sch",
                "project_path": "demo.kicad_pro",
                "sources": [
                    {"path":"demo.kicad_pro", "kind":"project", "slot":0, "source_bytes":PROJECT.len().to_string()},
                    {"path":"root.kicad_sch", "kind":"schematic", "slot":1, "source_bytes":ROOT.len().to_string()},
                    {"path":"child.kicad_sch", "kind":"schematic", "slot":2, "source_bytes":CHILD.len().to_string()}
                ]
            },
            "file_slots": [
                {"slot":0, "path":"demo.kicad_pro"},
                {"slot":1, "path":"root.kicad_sch"},
                {"slot":2, "path":"child.kicad_sch"}
            ],
            "limits": {
                "max_sources": 3,
                "max_source_bytes": (64 * 1024).to_string(),
                "max_total_source_bytes": (256 * 1024).to_string(),
                "max_path_bytes": 1024,
                "max_output_bytes": output_limit.to_string()
            },
            "netlist": {"source_path":"root.kicad_sch", "date":"", "tool":"kicad-monkey-native"}
        })
    }

    fn root_string(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.root.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn create_directory_link(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_directory_link(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    let status = Command::new("cmd")
        .args(["/c", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("mklink /J failed"))
    }
}
