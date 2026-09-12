#[path = "support/source_authoring/board_readback.rs"]
mod board_readback;
#[path = "support/source_authoring/fixtures.rs"]
mod fixtures;
#[path = "support/source_authoring/limits.rs"]
mod limits;
#[path = "support/source_authoring/standalone_readback.rs"]
mod standalone_readback;
use board_readback::*;
use fixtures::*;
use limits::*;

use kicad_monkey_core::{
    AuthoredCustomPadClearance, AuthoredDrill, AuthoredEmbeddedFile, AuthoredFootprint,
    AuthoredFootprintOccurrence, AuthoredFootprintProperty, AuthoredFootprintText, AuthoredGraphic,
    AuthoredGraphicGeometry, AuthoredLayer, AuthoredModel, AuthoredNet, AuthoredNetRef,
    AuthoredPad, AuthoredPadAnchor, AuthoredPadKind, AuthoredPadPolygonPoint, AuthoredPadPrimitive,
    AuthoredPadPrimitiveFill, AuthoredPadPrimitiveGeometry, AuthoredPadShape, AuthoredPcb,
    AuthoredPoint, AuthoredResourceData, AuthoredRoutingArc, AuthoredSegment, AuthoredSetup,
    AuthoredStackup, AuthoredStackupLayer, AuthoredStandaloneFootprint, AuthoredTextEffects,
    AuthoredZoneConnection, EmbeddedDecodeLimits, EmbeddedFileOwner, ErrorKind, FootprintDocument,
    FootprintLimits, PcbAuthoringLimits, PcbFootprintMemberOwner, PcbPadPolygonPoint,
    PcbPadPrimitiveGeometry, PcbProfileOwner,
};
use std::io::{Cursor, Write};

#[test]
fn fresh_typed_board_emits_and_reads_required_first_spike_semantics() {
    let document = authored_board()
        .to_document(PcbAuthoringLimits::default())
        .expect("fresh board document");
    publish_for_independent_oracle("native-authored.kicad_pcb", document.source());
    let view = document.view().expect("fresh board view");
    assert_board_metadata_and_profile(&view);
    assert_board_occurrences(&view);
    assert_board_surface_policy(&view);
    assert_board_local_artwork(&view);
    assert_board_resources(&view);
    assert_board_routes(&view);
    let mut bytes = Vec::new();
    document.write_to(&mut bytes).expect("board write");
    assert_eq!(bytes, document.source().as_bytes());
}

#[test]
fn fresh_typed_standalone_footprint_round_trips_members_model_and_resource_bytes() {
    let document = standalone_footprint()
        .to_document(PcbAuthoringLimits::default())
        .expect("fresh footprint document");
    publish_for_independent_oracle("Demo_Standalone.kicad_mod", document.source());
    let view = document.view().expect("footprint view");
    standalone_readback::assert_metadata_and_artwork(&view);
    let pad = standalone_readback::assert_custom_pad(&view);
    standalone_readback::assert_placed_custom_pad(&pad);
    standalone_readback::assert_cut_model_and_resource(&view);
}

#[test]
fn authored_writer_rejects_unsupported_loss_identity_and_resource_limits() {
    assert_custom_compatibility_and_rejections();
    assert_metadata_loss_rejections();
    let board = authored_board();
    let text = board
        .canonical_text(PcbAuthoringLimits::default())
        .expect("baseline");
    let exact = PcbAuthoringLimits {
        max_output_bytes: text.len(),
        ..PcbAuthoringLimits::default()
    };
    assert_eq!(
        board.canonical_text(exact).expect("exact output").len(),
        text.len()
    );
    assert_eq!(
        board
            .canonical_text(PcbAuthoringLimits {
                max_output_bytes: text.len() - 1,
                ..exact
            })
            .expect_err("one-under output")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut unsupported = board.clone();
    unsupported.version = 20_260_101;
    let error = unsupported
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unsupported version");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("supported"));

    let mut duplicate = board.clone();
    duplicate.footprints[1].uuid = duplicate.footprints[0].uuid.clone();
    let error = duplicate
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("duplicate source identity");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("duplicate KiCad UUID"));

    assert_occurrence_metadata_losses(&board);

    assert_case_insensitive_identity(&board);

    let mut unknown_net = board;
    unknown_net.segments[0].net_code = 99;
    let error = unknown_net
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("unknown net");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("unknown authored net"));

    let error = standalone_footprint()
        .canonical_text(PcbAuthoringLimits {
            max_resource_encoded_bytes: 0,
            ..PcbAuthoringLimits::default()
        })
        .expect_err("encoded resource limit");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn board_resource_namespace_and_embedded_model_links_are_loss_aware() {
    let board = authored_board();
    let mut shadowed_payload = board.clone();
    shadowed_payload.footprints[0].footprint.embedded_files[0].name = "board.step".to_owned();
    shadowed_payload.footprints[0].footprint.models[0].path = "kicad-embed://board.step".to_owned();
    let error = shadowed_payload
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("footprint payload shadowed by board resource");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("conflicting payloads"));
    assert_eq!(
        shadowed_payload
            .canonical_text(PcbAuthoringLimits {
                max_objects: 0,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("bounded traversal precedes namespace allocation")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut board_owned_reference = board.clone();
    board_owned_reference.footprints[0].footprint.embedded_files[0].name = "board.step".to_owned();
    board_owned_reference.footprints[0].footprint.embedded_files[0].data =
        AuthoredResourceData::DeclarationOnly;
    board_owned_reference.footprints[0].footprint.models[0].path =
        "kicad-embed://board.step".to_owned();
    board_owned_reference
        .canonical_text(PcbAuthoringLimits::default())
        .expect("footprint declaration references board-owned payload");

    let mut direct_board_reference = board.clone();
    direct_board_reference.footprints[0]
        .footprint
        .embedded_files
        .clear();
    direct_board_reference.footprints[0].footprint.models[0].path =
        "kicad-embed://board.step".to_owned();
    direct_board_reference
        .canonical_text(PcbAuthoringLimits::default())
        .expect("occurrence model directly references board declaration");

    let mut incompatible_reference = board_owned_reference;
    incompatible_reference.footprints[0]
        .footprint
        .embedded_files[0]
        .file_type = "font".to_owned();
    let error = incompatible_reference
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("incompatible resource declaration type");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("conflicting types"));

    let mut conflicting_occurrence_payloads = board.clone();
    conflicting_occurrence_payloads.footprints[1]
        .footprint
        .embedded_files
        .push(AuthoredEmbeddedFile {
            name: "footprint.step".to_owned(),
            file_type: "model".to_owned(),
            data: AuthoredResourceData::Bytes(b"different payload".to_vec()),
        });
    let error = conflicting_occurrence_payloads
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("conflicting occurrence payloads");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert!(error.message.contains("conflicting payloads"));

    let mut repeated_identical_payload = board.clone();
    repeated_identical_payload.footprints[1]
        .footprint
        .embedded_files
        .push(board.footprints[0].footprint.embedded_files[0].clone());
    repeated_identical_payload
        .canonical_text(PcbAuthoringLimits::default())
        .expect("identical occurrence payloads can be hoisted once");

    let mut missing_local = board.clone();
    missing_local.footprints[0].footprint.embedded_files.clear();
    let error = missing_local
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("missing local embedded model declaration");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    assert_eq!(
        missing_local
            .canonical_text(PcbAuthoringLimits {
                max_resource_input_bytes: 0,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("resource traversal is bounded before model lookup")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut empty_name = board;
    empty_name.footprints[0].footprint.models[0].path = "kicad-embed://".to_owned();
    let error = empty_name
        .canonical_text(PcbAuthoringLimits::default())
        .expect_err("empty embedded model resource name");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
}

#[test]
fn standalone_embedded_models_require_local_model_declarations() {
    let mut missing = standalone_footprint();
    missing.footprint.embedded_files.clear();
    assert_eq!(
        missing
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("missing standalone embedded model resource")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut deferred_missing = standalone_footprint();
    deferred_missing.footprint.models[0].path = "kicad-embed://missing.step".to_owned();
    assert_eq!(
        deferred_missing
            .canonical_text(PcbAuthoringLimits {
                max_resource_input_bytes: 0,
                ..PcbAuthoringLimits::default()
            })
            .expect_err("standalone resource traversal is bounded before model lookup")
            .kind,
        ErrorKind::ResourceLimit
    );

    let mut wrong_type = standalone_footprint();
    wrong_type.footprint.embedded_files[0].file_type = "other".to_owned();
    assert_eq!(
        wrong_type
            .canonical_text(PcbAuthoringLimits::default())
            .expect_err("wrong standalone embedded model resource type")
            .kind,
        ErrorKind::InvalidBuildValue
    );

    let mut declaration_only = standalone_footprint();
    declaration_only.footprint.embedded_files[0].data = AuthoredResourceData::DeclarationOnly;
    declaration_only
        .canonical_text(PcbAuthoringLimits::default())
        .expect("host-supplied standalone embedded model declaration");
}

#[test]
fn authored_validation_limits_and_loss_boundaries_are_explicit() {
    let minimal = AuthoredStandaloneFootprint::new("A", "F.Cu");
    let with_points = assert_object_string_and_point_limits(&minimal);
    assert_resource_work_limits();
    assert_setup_policy_losses();
    assert_member_policy_losses(&minimal, with_points);
    assert_document_scope_losses();
}

#[test]
fn owned_footprint_document_io_obeys_exact_limits_and_reports_errors() {
    let source = standalone_footprint()
        .canonical_text(PcbAuthoringLimits::default())
        .expect("standalone source");
    let exact = FootprintLimits {
        max_source_bytes: source.len(),
        max_output_bytes: source.len(),
        ..FootprintLimits::default()
    };
    let document = FootprintDocument::from_reader(Cursor::new(source.as_bytes()), exact)
        .expect("exact footprint read");
    let mut output = Vec::new();
    document
        .write_to(&mut output)
        .expect("exact footprint write");
    assert_eq!(output, source.as_bytes());

    assert_eq!(
        FootprintDocument::from_reader(
            Cursor::new(source.as_bytes()),
            FootprintLimits {
                max_source_bytes: source.len() - 1,
                ..FootprintLimits::default()
            },
        )
        .expect_err("source ceiling")
        .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(
        FootprintDocument::from_reader(
            Cursor::new([b'(', 0xff, b')']),
            FootprintLimits::default(),
        )
        .expect_err("invalid UTF-8")
        .kind,
        ErrorKind::InvalidUtf8
    );

    let strict = FootprintDocument::parse(
        source,
        FootprintLimits {
            max_output_bytes: output.len() - 1,
            ..FootprintLimits::default()
        },
    )
    .expect("parse with strict output limit");
    let mut untouched = Vec::new();
    assert_eq!(
        strict
            .write_to(&mut untouched)
            .expect_err("output ceiling")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert!(untouched.is_empty());
    assert_eq!(
        FootprintDocument::from_reader(FailingReader, FootprintLimits::default())
            .expect_err("read error")
            .kind,
        ErrorKind::Io
    );
    assert_eq!(
        document
            .write_to(FailingWriter)
            .expect_err("write error")
            .kind,
        ErrorKind::Io
    );
}

struct FailingReader;

impl std::io::Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("read failure"))
    }
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("write failure"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
