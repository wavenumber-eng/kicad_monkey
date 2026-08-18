use kicad_monkey_contracts::{
    decode_native_design_facts_result_a0, decode_native_design_facts_result_a1,
    decode_native_error_a0, decode_native_handshake_a0, decode_native_handshake_a2,
};
use kicad_monkey_native::{
    DESIGN_FACTS_A1_MAX_WILDCARD_MATCH_WORK, DESIGN_FACTS_RESOURCE_PROFILE, ENGINE_VERSION,
    MAX_DESIGN_REQUEST_DEPTH, MAX_DESIGN_REQUEST_NODES, MAX_REQUEST_BYTES, NativeErrorKind,
    PROTOCOL_VERSION, execute_request_a1_bytes, execute_request_bytes, handshake, handshake_a2,
    serialize_error,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
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
fn a2_handshake_and_a1_facts_are_strict_integral_and_deterministic() {
    let handshake_bytes = serde_json::to_vec(&handshake_a2()).expect("handshake a2 JSON");
    decode_native_handshake_a2(&handshake_bytes).expect("generated handshake a2 contract");
    let handshake: Value = serde_json::from_slice(&handshake_bytes).expect("handshake a2 value");
    assert_eq!(handshake["version"], "a2");
    assert_eq!(
        handshake["operations"],
        json!(["design-facts", "render-svg", "design-facts-a1"])
    );

    let fixture = Fixture::new();
    let request = fixture.request_a1(8 * 1024 * 1024);
    let first = execute_request_a1_bytes(&request).expect("valid a1 design facts");
    let second = execute_request_a1_bytes(&request).expect("repeat a1 design facts");
    assert_eq!(second, first);
    decode_native_design_facts_result_a1(&first).expect("generated a1 result contract");
    let result: Value = serde_json::from_slice(&first).expect("a1 result JSON");
    assert_eq!(result["version"], "a1");
    assert_eq!(result["resource_profile"], DESIGN_FACTS_RESOURCE_PROFILE);
    let netlist = result["kicad_netlist"].as_str().expect("netlist text");
    assert_eq!(result["kicad_netlist_bytes"], netlist.len().to_string());
    assert_eq!(
        result["kicad_netlist_sha256"],
        hex_digest(&Sha256::digest(netlist.as_bytes()))
    );
}

#[test]
fn a1_snapshot_uses_actual_bytes_and_is_manifest_order_independent() {
    let fixture = Fixture::new();
    let ordered =
        execute_request_a1_bytes(&fixture.request_a1(8 * 1024 * 1024)).expect("ordered snapshot");
    let ordered: Value = serde_json::from_slice(&ordered).expect("ordered result");

    let mut reordered_request = fixture.request_value_a1(8 * 1024 * 1024);
    reordered_request["manifest"]["sources"]
        .as_array_mut()
        .expect("manifest sources")
        .reverse();
    let reordered = execute_request_a1_bytes(
        &serde_json::to_vec(&reordered_request).expect("reordered request"),
    )
    .expect("reordered snapshot");
    let reordered: Value = serde_json::from_slice(&reordered).expect("reordered result");
    assert_eq!(
        reordered["source_snapshot_sha256"],
        ordered["source_snapshot_sha256"]
    );

    let mut changed = ROOT.to_vec();
    changed.push(b'\n');
    fs::write(fixture.root.join("root.kicad_sch"), changed).expect("change staged bytes");
    let mut changed_request = fixture.request_value_a1(8 * 1024 * 1024);
    changed_request["manifest"]["sources"][1]["source_bytes"] = json!((ROOT.len() + 1).to_string());
    let changed =
        execute_request_a1_bytes(&serde_json::to_vec(&changed_request).expect("changed request"))
            .expect("changed snapshot");
    let changed: Value = serde_json::from_slice(&changed).expect("changed result");
    assert_ne!(
        changed["source_snapshot_sha256"],
        ordered["source_snapshot_sha256"]
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
    for execute in [execute_request_bytes, execute_request_a1_bytes] {
        let exact = vec![b' '; MAX_REQUEST_BYTES];
        let exact_error = execute(&exact).expect_err("exact bytes reach JSON decoding");
        assert_eq!(exact_error.kind, NativeErrorKind::Request);

        let one_over = vec![b' '; MAX_REQUEST_BYTES + 1];
        let over_error = execute(&one_over).expect_err("one over fixed request ceiling");
        assert_eq!(over_error.kind, NativeErrorKind::ResourceLimit);
    }
}

#[test]
fn both_design_versions_enforce_exact_structural_node_and_depth_limits() {
    let node_elements = MAX_DESIGN_REQUEST_NODES - 3;
    let exact_nodes = serde_json::to_vec(&json!({
        "items": vec![Value::Null; node_elements]
    }))
    .expect("exact-node JSON");
    let one_over_nodes = serde_json::to_vec(&json!({
        "items": vec![Value::Null; node_elements + 1]
    }))
    .expect("one-over-node JSON");
    let exact_depth = nested_json(MAX_DESIGN_REQUEST_DEPTH - 1);
    let one_over_depth = nested_json(MAX_DESIGN_REQUEST_DEPTH);

    for execute in [execute_request_bytes, execute_request_a1_bytes] {
        assert_eq!(
            execute(&exact_nodes)
                .expect_err("exact nodes reach typed decoding")
                .kind,
            NativeErrorKind::Request
        );
        let node_error = execute(&one_over_nodes).expect_err("one node over");
        assert_eq!(node_error.kind, NativeErrorKind::ResourceLimit);
        assert!(node_error.message.contains("node"));

        assert_eq!(
            execute(exact_depth.as_bytes())
                .expect_err("exact depth reaches typed decoding")
                .kind,
            NativeErrorKind::Request
        );
        let depth_error = execute(one_over_depth.as_bytes()).expect_err("one depth over");
        assert_eq!(depth_error.kind, NativeErrorKind::ResourceLimit);
        assert!(depth_error.message.contains("depth"));
    }
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

#[test]
fn a1_process_failure_publishes_no_partial_stdout() {
    let fixture = Fixture::new();
    let first =
        execute_request_a1_bytes(&fixture.request_a1(8 * 1024 * 1024)).expect("measure a1 output");
    let mut request = fixture.request_value_a1(first.len() - 1);
    request["limits"]["max_output_bytes"] = json!((first.len() - 1).to_string());
    let mut child = Command::new(env!("CARGO_BIN_EXE_kicad-monkey-native"))
        .arg("design-facts-a1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn a1 native transport");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(&serde_json::to_vec(&request).expect("a1 request"))
        .expect("write a1 request");
    let output = child.wait_with_output().expect("native a1 output");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("structured stderr");
    assert_eq!(error["kind"], "resource_limit");
}

#[test]
fn a1_wildcard_profile_is_exact_preserves_a0_and_publishes_no_partial_overflow() {
    // A literal pattern charges `(tokens + 1) * name_chars + 2 * tokens`.
    let exact_name_bytes = 200;
    let pattern_bytes =
        (DESIGN_FACTS_A1_MAX_WILDCARD_MATCH_WORK - exact_name_bytes) / (exact_name_bytes + 2);
    assert_eq!(pattern_bytes, 9_900);
    assert_eq!(
        (pattern_bytes + 1) * exact_name_bytes + 2 * pattern_bytes,
        DESIGN_FACTS_A1_MAX_WILDCARD_MATCH_WORK
    );
    let exact = WildcardFixture::new(exact_name_bytes, pattern_bytes);
    execute_request_a1_bytes(&exact.request("a1")).expect("exact A1 wildcard-work profile ceiling");
    let a0_error = execute_request_bytes(&exact.request("a0"))
        .expect_err("A0 retains its 250,000-unit wildcard ceiling");
    assert_eq!(a0_error.kind, NativeErrorKind::ResourceLimit);
    assert!(a0_error.message.contains("wildcard match work"));

    let one_over = WildcardFixture::new(exact_name_bytes + 1, pattern_bytes);
    let overflow = execute_request_a1_bytes(&one_over.request("a1"))
        .expect_err("first representable workload above the A1 ceiling");
    assert_eq!(overflow.kind, NativeErrorKind::ResourceLimit);
    assert!(overflow.message.contains("wildcard match work"));

    let mut child = Command::new(env!("CARGO_BIN_EXE_kicad-monkey-native"))
        .arg("design-facts-a1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn A1 wildcard-overflow transport");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(&one_over.request("a1"))
        .expect("write A1 wildcard-overflow request");
    let output = child
        .wait_with_output()
        .expect("native A1 wildcard overflow");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error: Value = serde_json::from_slice(&output.stderr).expect("structured stderr");
    assert_eq!(error["kind"], "resource_limit");
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

struct WildcardFixture {
    root: PathBuf,
    project_source_bytes: usize,
    root_source_bytes: usize,
}

impl WildcardFixture {
    fn new(net_name_bytes: usize, pattern_bytes: usize) -> Self {
        let ordinal = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "kicad_monkey_native_wildcard_{}_{}",
            std::process::id(),
            ordinal
        ));
        fs::create_dir(&root).expect("create wildcard fixture root");
        let mut source = br#"(kicad_sch
          (uuid wildcard-root)
          (lib_symbols
            (symbol "Demo:One"
              (symbol "Demo:One_1_1"
                (pin passive line (at 0 0 0) (name "P") (number "1")))))
          (global_label ""#
            .to_vec();
        source.resize(source.len() + net_name_bytes, b'A');
        source.extend_from_slice(
            br#"" (shape input) (at 0 0 0) (uuid wildcard-label))
          (symbol (lib_id "Demo:One") (lib_name "Demo:One")
            (at 0 0 0) (uuid wildcard-symbol)
            (property "Reference" "U1") (property "Value" "One")))"#,
        );
        let mut project = br#"{
          "net_settings": {
            "classes": [{"name":"Default"}],
            "netclass_patterns": [{"pattern":""#
            .to_vec();
        project.resize(project.len() + pattern_bytes, b'B');
        project.extend_from_slice(
            br#"", "netclass":"Default"}]
          }
        }"#,
        );
        fs::write(root.join("demo.kicad_pro"), &project).expect("wildcard project");
        fs::write(root.join("root.kicad_sch"), &source).expect("wildcard schematic");
        Self {
            root,
            project_source_bytes: project.len(),
            root_source_bytes: source.len(),
        }
    }

    fn request(&self, version: &str) -> Vec<u8> {
        let total_source_bytes = self.project_source_bytes + self.root_source_bytes;
        let max_source_bytes = self.project_source_bytes.max(self.root_source_bytes);
        let mut request = json!({
            "type": "kicad_monkey.native.design_facts.request",
            "version": version,
            "bundle_root": self.root.to_string_lossy(),
            "manifest": {
                "schema": "kicad_monkey.source_bundle_manifest.a0",
                "type": "kicad_monkey.source_bundle_manifest",
                "version": "a0",
                "root_schematic_path": "root.kicad_sch",
                "project_path": "demo.kicad_pro",
                "sources": [
                    {"path":"demo.kicad_pro", "kind":"project", "slot":0, "source_bytes":self.project_source_bytes.to_string()},
                    {"path":"root.kicad_sch", "kind":"schematic", "slot":1, "source_bytes":self.root_source_bytes.to_string()}
                ]
            },
            "file_slots": [
                {"slot":0, "path":"demo.kicad_pro"},
                {"slot":1, "path":"root.kicad_sch"}
            ],
            "limits": {
                "max_sources": 2,
                "max_source_bytes": max_source_bytes.to_string(),
                "max_total_source_bytes": total_source_bytes.to_string(),
                "max_path_bytes": 1024,
                "max_output_bytes": (32 * 1024 * 1024).to_string()
            },
            "netlist": {"source_path":"root.kicad_sch", "date":"", "tool":"kicad-monkey-native"}
        });
        if version == "a1" {
            request.as_object_mut().expect("request object").insert(
                "resource_profile".to_owned(),
                json!(DESIGN_FACTS_RESOURCE_PROFILE),
            );
        }
        serde_json::to_vec(&request).expect("wildcard request JSON")
    }
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

    fn request_a1(&self, output_limit: usize) -> Vec<u8> {
        serde_json::to_vec(&self.request_value_a1(output_limit)).expect("a1 request JSON")
    }

    fn request_value_a1(&self, output_limit: usize) -> Value {
        let mut request = self.request_value(output_limit);
        request["version"] = json!("a1");
        request.as_object_mut().expect("a1 request object").insert(
            "resource_profile".to_owned(),
            json!(DESIGN_FACTS_RESOURCE_PROFILE),
        );
        request
    }

    fn root_string(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
}

fn nested_json(array_count: usize) -> String {
    format!("{}null{}", "[".repeat(array_count), "]".repeat(array_count))
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(self.root.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Drop for WildcardFixture {
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
