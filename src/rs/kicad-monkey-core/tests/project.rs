use kicad_monkey_core::{ProjectDocument, ProjectErrorKind, ProjectLimits, ProjectView};
use serde_json::json;
use std::io::{Cursor, Write};

const PROJECT: &str = r##"{
  "meta": {
    "filename": "demo.kicad_pro"
  },
  "text_variables": {
    "TITLE": "Demo",
    "REV": "A"
  },
  "schematic": {
    "subpart_first_id": 65,
    "subpart_id_separator": 46,
    "variants": [
      {
        "name": "Production",
        "description": "Loaded"
      },
      {
        "name": "No RF"
      }
    ]
  },
  "net_settings": {
    "classes": [
      {
        "name": "Default",
        "track_width": 0.25,
        "clearance": 0.2,
        "diff_pair_gap": 0.3,
        "diff_pair_width": 0.2,
        "diff_pair_via_gap": 0.4,
        "via_diameter": 0.8,
        "via_drill": 0.4,
        "microvia_diameter": 0.3,
        "microvia_drill": 0.1,
        "bus_width": 12.0,
        "wire_width": 6.0,
        "pcb_color": "#ff0000",
        "schematic_color": "#00ff00",
        "line_style": 2,
        "priority": 1,
        "tuning_profile": "fast"
      }
    ],
    "netclass_assignments": {
      "GND": [
        "Power"
      ]
    },
    "netclass_patterns": [
      {
        "pattern": "USB*",
        "netclass": "Diff"
      }
    ],
    "net_colors": {
      "GND": "#000000"
    }
  },
  "board": {
    "design_settings": {
      "diff_pair_dimensions": [
        {
          "width": 0.2,
          "gap": 0.25,
          "via_gap": 0.3
        }
      ],
      "tuning_pattern_settings": {
        "diff_pair_defaults": {
          "spacing": 0.6,
          "min_amplitude": 0.2,
          "max_amplitude": 1.0,
          "corner_style": 1,
          "corner_radius_percentage": 80,
          "single_sided": true
        },
        "diff_pair_skew_defaults": {},
        "single_track_defaults": {}
      }
    }
  }
}
"##;

#[test]
fn typed_project_fields_cover_compiler_and_board_settings() {
    let document =
        ProjectDocument::parse(PROJECT.to_owned(), ProjectLimits::default()).expect("project");
    let view = document.view();
    assert_project_metadata(view);
    assert_net_settings(view);
    assert_board_settings(view);
}

fn assert_project_metadata(view: ProjectView<'_>) {
    assert_eq!(
        view.text_variables().expect("variables"),
        [
            ("TITLE".to_owned(), "Demo".to_owned()),
            ("REV".to_owned(), "A".to_owned())
        ]
    );
    let variants = view.variants().expect("variants");
    assert_eq!(variants[0].name, "Production");
    assert_eq!(variants[0].description.as_deref(), Some("Loaded"));
    assert_eq!(variants[1].description, None);
    assert_eq!(
        view.get_path("meta.filename"),
        Some(&json!("demo.kicad_pro"))
    );
}

fn assert_net_settings(view: ProjectView<'_>) {
    let nets = view.net_settings().expect("net settings");
    assert_eq!(nets.classes.len(), 1);
    assert_eq!(nets.classes[0].name, "Default");
    assert_eq!(nets.classes[0].track_width, Some(0.25));
    assert_eq!(nets.classes[0].line_style, Some(2));
    assert_eq!(
        nets.assignments,
        [("GND".to_owned(), vec!["Power".to_owned()])]
    );
    assert_eq!(nets.patterns[0].pattern, "USB*");
    assert_eq!(nets.colors, [("GND".to_owned(), "#000000".to_owned())]);
}

fn assert_board_settings(view: ProjectView<'_>) {
    let board = view.board_design_settings().expect("board settings");
    assert_eq!(board.diff_pair_dimensions[0].via_gap, Some(0.3));
    assert_eq!(board.tuning.diff_pair_defaults.spacing, Some(0.6));
    assert_eq!(board.tuning.diff_pair_defaults.single_sided, Some(true));
}

#[test]
fn exact_and_canonical_writes_are_stable_and_ordered() {
    let document =
        ProjectDocument::from_reader(Cursor::new(PROJECT.as_bytes()), ProjectLimits::default())
            .expect("project");
    let mut output = Vec::new();
    document.write_to(&mut output).expect("exact write");
    assert_eq!(output, PROJECT.as_bytes());
    assert_eq!(document.canonical_text().expect("canonical"), PROJECT);
    let reparsed = ProjectDocument::parse(
        document.canonical_text().expect("canonical"),
        ProjectLimits::default(),
    )
    .expect("reparse");
    assert_eq!(reparsed.canonical_text().expect("second"), PROJECT);
}

#[test]
fn exact_promoted_collection_limits_are_accepted() {
    let document = ProjectDocument::parse(
        PROJECT.to_owned(),
        ProjectLimits {
            max_source_bytes: PROJECT.len(),
            max_output_bytes: PROJECT.len(),
            max_text_variables: 2,
            max_variants: 2,
            max_net_classes: 1,
            max_netclass_assignments: 2,
            max_netclass_patterns: 1,
            max_net_colors: 1,
            max_diff_pair_dimensions: 1,
            ..ProjectLimits::default()
        },
    )
    .expect("exact limits");
    assert_eq!(
        document.view().text_variables().expect("variables").len(),
        2
    );
    assert_eq!(document.view().variants().expect("variants").len(), 2);
    assert_eq!(
        document.view().net_settings().expect("nets").classes.len(),
        1
    );
    assert_eq!(
        document
            .view()
            .board_design_settings()
            .expect("board")
            .diff_pair_dimensions
            .len(),
        1
    );
    assert_eq!(
        document.canonical_text().expect("output").len(),
        PROJECT.len()
    );
}

#[test]
fn null_variant_description_matches_python_none() {
    let document = ProjectDocument::parse(
        r#"{"schematic":{"variants":[{"name":"Null","description":null}]}}"#.to_owned(),
        ProjectLimits::default(),
    )
    .expect("project");
    assert_eq!(
        document.view().variants().expect("variants"),
        [kicad_monkey_core::ProjectVariant {
            name: "Null".to_owned(),
            description: None,
        }]
    );
}

#[test]
fn text_variable_variant_and_path_mutations_reparse_and_stabilize() {
    let mut document =
        ProjectDocument::parse(PROJECT.to_owned(), ProjectLimits::default()).expect("project");
    mutate_variables(&mut document);
    mutate_variants(&mut document);
    assert!(document.set_path("meta.marker", json!(42)).expect("path"));
    assert!(
        !document
            .set_path("meta.marker", json!(42))
            .expect("stable path")
    );
    assert_eq!(document.view().get_path("meta.marker"), Some(&json!(42)));
    assert_eq!(
        document.view().text_variables().expect("variables"),
        [("TITLE".to_owned(), "Changed".to_owned())]
    );
    assert_eq!(document.view().variants().expect("variants").len(), 2);
    let stable = document.canonical_text().expect("canonical");
    assert_eq!(stable, document.source());
    assert_eq!(
        ProjectDocument::parse(stable.clone(), ProjectLimits::default())
            .expect("reparse")
            .canonical_text()
            .expect("second"),
        stable
    );
}

fn mutate_variables(document: &mut ProjectDocument) {
    assert!(!document.set_text_variable("TITLE", "Demo").expect("no-op"));
    assert!(document.set_text_variable("TITLE", "Changed").expect("set"));
    assert!(document.remove_text_variable("REV").expect("remove"));
    assert!(!document.remove_text_variable("missing").expect("absent"));
}

fn mutate_variants(document: &mut ProjectDocument) {
    let added = document.add_variant("Test", None).expect("add");
    assert_eq!(added.name, "Test");
    assert!(document.rename_variant("Test", "Renamed").expect("rename"));
    assert!(
        !document
            .rename_variant("missing", "Unused")
            .expect("absent")
    );
    assert_eq!(
        document.remove_variant("Renamed").expect("remove"),
        Some(kicad_monkey_core::ProjectVariant {
            name: "Renamed".to_owned(),
            description: None,
        })
    );
}

#[test]
fn mutations_reject_conflicts_and_leave_source_unchanged() {
    let mut document =
        ProjectDocument::parse(PROJECT.to_owned(), ProjectLimits::default()).expect("project");
    for error in [
        document.add_variant("", None).expect_err("empty").kind,
        document
            .add_variant("Production", None)
            .expect_err("duplicate")
            .kind,
        document
            .rename_variant("No RF", "Production")
            .expect_err("rename duplicate")
            .kind,
        document.set_path("", json!(1)).expect_err("path").kind,
        document
            .set_path("meta.filename.child", json!(1))
            .expect_err("non-object")
            .kind,
    ] {
        assert!(matches!(
            error,
            ProjectErrorKind::Conflict | ProjectErrorKind::InvalidPath
        ));
        assert_eq!(document.source(), PROJECT);
    }
}

#[test]
fn source_output_and_typed_collection_limits_fail_closed() {
    assert_eq!(
        ProjectDocument::from_reader(
            Cursor::new(PROJECT.as_bytes()),
            ProjectLimits {
                max_source_bytes: PROJECT.len() - 1,
                ..ProjectLimits::default()
            },
        )
        .expect_err("source")
        .kind,
        ProjectErrorKind::ResourceLimit
    );
    let mut output = Vec::new();
    let mut limited = ProjectDocument::parse(
        PROJECT.to_owned(),
        ProjectLimits {
            max_output_bytes: PROJECT.len() - 1,
            ..ProjectLimits::default()
        },
    )
    .expect("read");
    assert_eq!(
        limited.write_to(&mut output).expect_err("output").kind,
        ProjectErrorKind::ResourceLimit
    );
    assert!(output.is_empty());
    assert_eq!(
        limited
            .set_text_variable("X", "Y")
            .expect_err("mutation output")
            .kind,
        ProjectErrorKind::ResourceLimit
    );
    assert_eq!(limited.source(), PROJECT);

    let limits = [
        ProjectLimits {
            max_text_variables: 1,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_variants: 1,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_net_classes: 0,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_netclass_assignments: 0,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_netclass_patterns: 0,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_net_colors: 0,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_diff_pair_dimensions: 0,
            ..ProjectLimits::default()
        },
        ProjectLimits {
            max_typed_string_bytes: 1,
            ..ProjectLimits::default()
        },
    ];
    for (index, limits) in limits.into_iter().enumerate() {
        let document = ProjectDocument::parse(PROJECT.to_owned(), limits).expect("source");
        let result = match index {
            0 => document.view().text_variables().map(|_| ()),
            1 | 7 => document.view().variants().map(|_| ()),
            2..=5 => document.view().net_settings().map(|_| ()),
            _ => document.view().board_design_settings().map(|_| ()),
        };
        assert_eq!(
            result.expect_err("typed limit").kind,
            ProjectErrorKind::ResourceLimit
        );
    }
}

#[test]
fn invalid_input_and_io_failures_are_structured() {
    assert_eq!(
        ProjectDocument::from_reader(Cursor::new([0xff]), ProjectLimits::default())
            .expect_err("UTF-8")
            .kind,
        ProjectErrorKind::InvalidUtf8
    );
    assert_eq!(
        ProjectDocument::parse("{".to_owned(), ProjectLimits::default())
            .expect_err("JSON")
            .kind,
        ProjectErrorKind::InvalidJson
    );
    assert_eq!(
        ProjectDocument::parse("[]".to_owned(), ProjectLimits::default())
            .expect_err("root")
            .kind,
        ProjectErrorKind::RootNotObject
    );
    let document =
        ProjectDocument::parse(PROJECT.to_owned(), ProjectLimits::default()).expect("project");
    assert_eq!(
        document.write_to(FailingWriter).expect_err("I/O").kind,
        ProjectErrorKind::Io
    );
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
