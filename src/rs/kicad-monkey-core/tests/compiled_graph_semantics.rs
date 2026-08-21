use kicad_monkey_contracts::decode_compiled_schematic_graph_a0;
use kicad_monkey_contracts::generated::compiled_schematic_graph::{
    GraphicalTargetType, TerminalRole,
};
use kicad_monkey_core::{
    CompiledGraphIdentityAllocator, IdentityMapping, compiled_schematic_graph_design_scope,
    validate_compiled_schematic_graph,
};
use serde_json::Value;

fn vectors() -> Value {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/compiled_schematic_graph_a0_vectors.json"
    )))
    .expect("compiled graph vectors")
}

fn mapping(value: &Value) -> IdentityMapping {
    value
        .as_object()
        .expect("identity mapping")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn allocate(
    allocator: &mut CompiledGraphIdentityAllocator,
    allocation: &Value,
) -> Result<String, kicad_monkey_core::CompiledGraphIdentityError> {
    let object_type = allocation["object_type"].as_str().expect("object type");
    if allocation["mode"] == "source" {
        let owner_refs = allocation["owner_refs"]
            .as_array()
            .expect("owner refs")
            .iter()
            .map(|value| value.as_str().expect("owner ref").to_owned())
            .collect::<Vec<_>>();
        allocator.allocate_source(
            object_type,
            &mapping(&allocation["source_identity"]),
            &owner_refs,
        )
    } else {
        allocator.allocate_derived(object_type, &mapping(&allocation["identity"]))
    }
}

#[test]
fn native_allocator_matches_every_shared_identity_vector() {
    let vectors = vectors();
    let identity = &vectors["identity"];
    let scope = compiled_schematic_graph_design_scope("KiCad", r"Board\Main.kicad_pro")
        .expect("design scope");
    assert_eq!(
        serde_json::to_value(&scope).expect("scope JSON"),
        identity["normalized_scope"]
    );
    let mut allocator = CompiledGraphIdentityAllocator::new(&scope).expect("allocator");
    for group in [
        "allocations",
        "supporting_allocations",
        "canonical_allocations",
    ] {
        for allocation in identity[group].as_array().expect(group) {
            assert_eq!(
                allocate(&mut allocator, allocation).expect("native allocation"),
                allocation["expected"]
            );
        }
    }
}

#[test]
fn native_allocator_matches_normalization_precedence_and_failures() {
    let identity = &vectors()["identity"];
    let scope = mapping(&identity["normalized_scope"]);
    for case in identity["selector_equivalence"]
        .as_array()
        .expect("selector cases")
    {
        let mut left = CompiledGraphIdentityAllocator::new(&scope).expect("left allocator");
        let mut right = CompiledGraphIdentityAllocator::new(&scope).expect("right allocator");
        let mut left_case = case["left"].clone();
        left_case["mode"] = "source".into();
        let mut right_case = case["right"].clone();
        right_case["mode"] = "source".into();
        assert_eq!(
            allocate(&mut left, &left_case).expect("left"),
            case["expected"]
        );
        assert_eq!(
            allocate(&mut right, &right_case).expect("right"),
            case["expected"]
        );
    }
    for case in identity["scope_cases"].as_array().expect("scope cases") {
        let project = case["project"].as_object().expect("project");
        let filename = project
            .get("filename")
            .or_else(|| project.get("name"))
            .and_then(Value::as_str)
            .expect("project filename or name");
        let actual = compiled_schematic_graph_design_scope(
            case["source_cad"].as_str().expect("source CAD"),
            filename,
        )
        .expect("normalized scope");
        assert_eq!(
            serde_json::to_value(actual).expect("scope JSON"),
            case["expected"]
        );
    }
    assert_identity_failures(identity, &scope);
}

fn assert_identity_failures(identity: &Value, scope: &IdentityMapping) {
    for case in identity["failures"].as_array().expect("failure cases") {
        let expected = case["error_match"].as_str().expect("error match");
        if case["mode"] == "scope" {
            let error = compiled_schematic_graph_design_scope(
                case["source_cad"].as_str().expect("source CAD"),
                "",
            )
            .expect_err("scope failure");
            assert!(error.to_string().contains(expected));
            continue;
        }
        let mut allocator = CompiledGraphIdentityAllocator::new(scope).expect("allocator");
        let mut allocation = case.clone();
        if case["mode"] == "duplicate_source" {
            allocation["mode"] = "source".into();
            allocate(&mut allocator, &allocation).expect("first allocation");
        }
        let error = allocate(&mut allocator, &allocation).expect_err("identity failure");
        assert!(error.to_string().contains(expected), "{}", case["id"]);
    }
}

#[test]
fn native_semantic_validator_accepts_the_complete_shared_graph() {
    let vectors = vectors();
    let bytes = serde_json::to_vec(&vectors["graph"]).expect("graph JSON");
    let graph = decode_compiled_schematic_graph_a0(&bytes).expect("strict graph");
    validate_compiled_schematic_graph(&graph).expect("semantic graph");
}

#[test]
fn native_semantic_validator_rejects_reference_ownership_and_cycle_failures() {
    let vectors = vectors();
    let bytes = serde_json::to_vec(&vectors["graph"]).expect("graph JSON");
    let graph = decode_compiled_schematic_graph_a0(&bytes).expect("strict graph");

    let mut wrong_type = graph.clone();
    wrong_type.page_definitions[0].unit_definition_ref = wrong_type.page_definitions[0].id.clone();
    assert_eq!(
        validate_compiled_schematic_graph(&wrong_type)
            .expect_err("wrong reference type")
            .code,
        "wrong_reference_type"
    );

    let mut missing_inverse = graph.clone();
    missing_inverse.unit_occurrences[0]
        .page_occurrence_refs
        .clear();
    assert!(
        validate_compiled_schematic_graph(&missing_inverse)
            .expect_err("missing inverse")
            .message
            .contains("not listed")
    );

    let mut cycle = graph.clone();
    let child_unit = cycle.hierarchy_occurrences[0]
        .child_unit_occurrence_ref
        .clone();
    cycle.hierarchy_occurrences[0].parent_unit_occurrence_ref = child_unit.clone();
    cycle.hierarchy_occurrences[0].parent_page_occurrence_ref =
        cycle.page_occurrences[1].id.clone();
    cycle.hierarchy_occurrences[0].child_unit_occurrence_ref = child_unit;
    assert_eq!(
        validate_compiled_schematic_graph(&cycle)
            .expect_err("hierarchy cycle")
            .code,
        "hierarchy_cycle"
    );
}

#[test]
fn native_semantic_validator_rejects_ids_bindings_and_graphical_targets() {
    let vectors = vectors();
    let bytes = serde_json::to_vec(&vectors["graph"]).expect("graph JSON");
    let graph = decode_compiled_schematic_graph_a0(&bytes).expect("strict graph");

    let mut duplicate = graph.clone();
    duplicate.component_occurrences[0].id = duplicate.unit_definitions[0].id.clone();
    assert_eq!(
        validate_compiled_schematic_graph(&duplicate)
            .expect_err("duplicate ID")
            .code,
        "duplicate_row_id"
    );

    let mut wrong_binding = graph.clone();
    wrong_binding.terminal_occurrences[1].role = TerminalRole::PowerPort;
    assert!(
        validate_compiled_schematic_graph(&wrong_binding)
            .expect_err("wrong binding roles")
            .message
            .contains("sheet_entry to a port")
    );

    let mut wrong_target = graph;
    wrong_target.graphical_artifact_links[0].target_type =
        GraphicalTargetType::SchTerminalOccurrence;
    assert_eq!(
        validate_compiled_schematic_graph(&wrong_target)
            .expect_err("wrong target type")
            .code,
        "wrong_reference_type"
    );
}

#[test]
fn shared_inverse_ownership_failures_are_rejected_by_rust() {
    let vectors = vectors();
    for case in vectors["semantic_failures"]
        .as_array()
        .expect("semantic failures")
    {
        let graph = semantic_failure_graph(
            vectors["graph"].clone(),
            case["kind"].as_str().expect("failure kind"),
        );
        let bytes = serde_json::to_vec(&graph).expect("failure graph JSON");
        let graph = decode_compiled_schematic_graph_a0(&bytes).expect("strict failure graph");
        let error = validate_compiled_schematic_graph(&graph).expect_err("semantic failure");
        assert!(
            error
                .message
                .contains(case["error_match"].as_str().expect("error match")),
            "{}",
            case["id"]
        );
    }
}

fn semantic_failure_graph(mut graph: Value, kind: &str) -> Value {
    match kind {
        "definition_extra_cross_owner" => add_cross_owner_definition(&mut graph),
        "occurrence_extra_cross_owner" => add_cross_owner_occurrence(&mut graph),
        "hierarchy_wrong_parent_inverse" => {
            graph["unit_occurrences"][0]["parent_hierarchy_occurrence_ref"] =
                graph["hierarchy_occurrences"][0]["id"].clone();
        }
        _ => panic!("unknown semantic failure kind {kind}"),
    }
    graph
}

fn add_cross_owner_definition(graph: &mut Value) {
    let mut unit = graph["unit_definitions"][0].clone();
    let mut page = graph["page_definitions"][0].clone();
    unit["id"] = "aaaaaaaa-aaaa-7aaa-8aaa-aaaaaaaaaaaa".into();
    unit["page_definition_refs"] = serde_json::json!(["bbbbbbbb-bbbb-7bbb-8bbb-bbbbbbbbbbbb"]);
    page["id"] = "bbbbbbbb-bbbb-7bbb-8bbb-bbbbbbbbbbbb".into();
    page["unit_definition_ref"] = unit["id"].clone();
    graph["unit_definitions"]
        .as_array_mut()
        .expect("unit definitions")
        .push(unit);
    graph["page_definitions"]
        .as_array_mut()
        .expect("page definitions")
        .push(page.clone());
    graph["unit_definitions"][0]["page_definition_refs"]
        .as_array_mut()
        .expect("page refs")
        .push(page["id"].clone());
}

fn add_cross_owner_occurrence(graph: &mut Value) {
    let mut unit = graph["unit_occurrences"][0].clone();
    let mut page = graph["page_occurrences"][0].clone();
    unit["id"] = "cccccccc-cccc-7ccc-8ccc-cccccccccccc".into();
    unit["page_occurrence_refs"] = serde_json::json!(["dddddddd-dddd-7ddd-8ddd-dddddddddddd"]);
    unit.as_object_mut()
        .expect("unit occurrence")
        .remove("parent_hierarchy_occurrence_ref");
    page["id"] = "dddddddd-dddd-7ddd-8ddd-dddddddddddd".into();
    page["unit_occurrence_ref"] = unit["id"].clone();
    graph["unit_occurrences"]
        .as_array_mut()
        .expect("unit occurrences")
        .push(unit);
    graph["page_occurrences"]
        .as_array_mut()
        .expect("page occurrences")
        .push(page.clone());
    graph["unit_occurrences"][0]["page_occurrence_refs"]
        .as_array_mut()
        .expect("page refs")
        .push(page["id"].clone());
}

#[test]
fn wide_definition_owner_validates_with_one_prebuilt_inverse_membership_set() {
    let vectors = vectors();
    let bytes = serde_json::to_vec(&vectors["graph"]).expect("graph JSON");
    let mut graph = decode_compiled_schematic_graph_a0(&bytes).expect("strict graph");
    let unit_ref = graph.unit_definitions[0].id.clone();
    let template = graph.page_definitions[0].clone();
    for index in 0_u64..2_048 {
        let mut page = template.clone();
        page.id = format!("00000000-0000-7{:03x}-8000-{index:012x}", index & 0xfff);
        page.unit_definition_ref.clone_from(&unit_ref);
        graph.unit_definitions[0]
            .page_definition_refs
            .push(page.id.clone());
        graph.page_definitions.push(page);
    }
    validate_compiled_schematic_graph(&graph).expect("wide exact inverse");
    graph.unit_definitions[0]
        .page_definition_refs
        .push(graph.page_definitions[1].id.clone());
    assert!(
        validate_compiled_schematic_graph(&graph)
            .expect_err("duplicate inverse")
            .message
            .contains("inverse is inconsistent")
    );
}
