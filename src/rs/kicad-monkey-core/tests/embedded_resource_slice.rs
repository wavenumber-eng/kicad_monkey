use kicad_monkey_core::{
    EmbeddedDataPresence, EmbeddedFileOwner, PcbFamily, PcbLimits, PcbSelection, PcbView,
};
#[cfg(feature = "embedded-resource-zstd")]
use kicad_monkey_core::{EmbeddedDecodeLimits, FootprintLimits, FootprintView};
#[cfg(feature = "embedded-resource-zstd")]
use sha2::{Digest, Sha256};

#[cfg(feature = "embedded-resource-zstd")]
#[test]
#[allow(
    clippy::cognitive_complexity,
    reason = "one scenario verifies owner ordering, payloads, and compatibility iterators"
)]
fn board_and_embedded_footprint_resources_keep_owner_and_decode_lazily() {
    let board_payload = b"board-owned bytes";
    let footprint_payload = b"footprint-owned bytes";
    let source = format!(
        r#"(kicad_pcb
  (footprint "Demo:Part"
    (embedded_files
      (file (name "shared.step") (type model) (data |{}|) (checksum "{}"))
      (file (name "external.step") (type model) (checksum "external-checksum"))))
  (footprint "Demo:Second"
    (embedded_files
      (file (name "second.bin") (type other) (data |{}|) (checksum "{}"))))
  (embedded_files
    (file (name "shared.step") (type model) (data |{}|) (checksum "41B3C1140F84B2B746E8E9CACA2E64FD"))
    (file (name "broken.bin") (type other) (data |not-base64|))))"#,
        encoded(footprint_payload),
        checksum(footprint_payload),
        encoded(b"second footprint"),
        checksum(b"second footprint"),
        encoded(board_payload),
    );
    let view = PcbView::parse(&source, PcbLimits::default()).expect("board");
    let files = view
        .all_embedded_files()
        .collect::<Result<Vec<_>, _>>()
        .expect("resource metadata");

    assert_eq!(files.len(), 5);
    assert_eq!(view.counts().embedded_files, 2);
    assert_eq!(view.counts().footprint_embedded_files, 3);
    assert_eq!(files[0].name, "shared.step");
    assert_eq!(
        files[0].owner,
        EmbeddedFileOwner::EmbeddedFootprint { footprint_index: 0 }
    );
    assert_eq!(files[0].data_presence, EmbeddedDataPresence::Encoded);
    assert_eq!(
        view.decode_embedded_file(&files[0], EmbeddedDecodeLimits::default())
            .expect("footprint payload")
            .expect("present payload"),
        footprint_payload
    );
    assert_eq!(files[1].data_presence, EmbeddedDataPresence::Absent);
    assert_eq!(
        view.decode_embedded_file(&files[1], EmbeddedDecodeLimits::default())
            .expect("metadata-only declaration"),
        None
    );

    assert_eq!(
        files[2].owner,
        EmbeddedFileOwner::EmbeddedFootprint { footprint_index: 1 }
    );
    assert_eq!(
        view.decode_embedded_file(&files[3], EmbeddedDecodeLimits::default())
            .expect("board payload")
            .expect("present payload"),
        board_payload
    );
    let error = view
        .decode_embedded_file(&files[4], EmbeddedDecodeLimits::default())
        .expect_err("malformed nonempty payload");
    assert!(error.message.contains("broken.bin"));
    assert_eq!(error.token.as_deref(), Some("broken.bin"));

    assert_eq!(
        view.embedded_files()
            .collect::<Result<Vec<_>, _>>()
            .expect("board resources")
            .len(),
        2
    );
    assert_eq!(
        view.footprint_embedded_files()
            .collect::<Result<Vec<_>, _>>()
            .expect("footprint resources")
            .len(),
        3
    );
}

#[cfg(feature = "embedded-resource-zstd")]
#[test]
fn standalone_resources_distinguish_absent_empty_and_encoded_payloads() {
    let payload = b"standalone bytes";
    let source = format!(
        r#"(footprint "Standalone"
  (model "kicad-embed://asset.bin")
  (embedded_files
    (file (name "asset.bin") (type other) (data |{}|) (checksum "{}"))
    (file (name "empty.bin") (type other) (data) (checksum "{}"))
    (file (name "external.bin") (type other))))"#,
        encoded(payload),
        checksum(payload),
        checksum(b""),
    );
    let view = FootprintView::parse(&source, FootprintLimits::default()).expect("footprint");
    let files = view
        .embedded_files()
        .collect::<Result<Vec<_>, _>>()
        .expect("resource metadata");

    assert_eq!(files.len(), 3);
    assert!(
        files
            .iter()
            .all(|file| file.owner == EmbeddedFileOwner::StandaloneFootprint)
    );
    assert_eq!(files[0].data_presence, EmbeddedDataPresence::Encoded);
    assert_eq!(files[1].data_presence, EmbeddedDataPresence::Empty);
    assert_eq!(files[2].data_presence, EmbeddedDataPresence::Absent);
    let model = view.models().next().expect("model").expect("model record");
    let lookup_name = model
        .path
        .strip_prefix("kicad-embed://")
        .expect("embedded model link");
    assert_eq!(lookup_name, files[0].name);
    assert_eq!(
        view.embedded_file_encoded_data(&files[1], 0)
            .expect("explicit empty encoded access"),
        Some(String::new())
    );
    assert_eq!(
        view.embedded_file_encoded_data(&files[2], 0)
            .expect("absent encoded access"),
        None
    );
    assert_eq!(
        view.decode_embedded_file(&files[0], EmbeddedDecodeLimits::default())
            .expect("decode")
            .expect("present"),
        payload
    );
    assert_eq!(
        view.decode_embedded_file(&files[1], EmbeddedDecodeLimits::default())
            .expect("empty payload"),
        Some(Vec::new())
    );
    assert_eq!(
        view.decode_embedded_file(&files[2], EmbeddedDecodeLimits::default())
            .expect("absent payload"),
        None
    );
}

#[cfg(feature = "embedded-resource-zstd")]
#[test]
fn explicit_payload_access_enforces_the_caller_limit() {
    let payload = b"bounded";
    let encoded = encoded(payload);
    let source = format!(
        r#"(footprint "Bounded" (embedded_files
          (file (name "bounded.bin") (data |{encoded}|))))"#
    );
    let view = FootprintView::parse(&source, FootprintLimits::default()).expect("footprint");
    let file = view
        .embedded_files()
        .next()
        .expect("file")
        .expect("metadata");
    assert_eq!(file.encoded_data_bytes, encoded.len());
    assert!(
        view.embedded_file_encoded_data(&file, encoded.len())
            .expect("exact limit")
            .is_some()
    );
    let error = view
        .embedded_file_encoded_data(&file, encoded.len() - 1)
        .expect_err("one byte under");
    assert!(error.message.contains("bounded.bin"));

    let error = view
        .decode_embedded_file(
            &file,
            EmbeddedDecodeLimits {
                max_compressed_bytes: 0,
                ..EmbeddedDecodeLimits::default()
            },
        )
        .expect_err("compressed payload limit");
    assert_eq!(error.kind, kicad_monkey_core::ErrorKind::ResourceLimit);
    assert!(error.message.contains("bounded.bin"));

    let mut forged = file.clone();
    forged.checksum = Some(checksum(payload));
    let error = view
        .embedded_file_encoded_data(&forged, encoded.len())
        .expect_err("metadata must remain source-owned");
    assert_eq!(error.kind, kicad_monkey_core::ErrorKind::InvalidSpan);
    assert!(error.message.contains("bounded.bin"));
}

#[cfg(feature = "embedded-resource-zstd")]
#[test]
fn malformed_zstd_checksum_and_decoded_limits_are_resource_specific() {
    let payload = b"decoded-limit";
    let source = format!(
        r#"(kicad_pcb (embedded_files
          (file (name "bad-zstd.bin") (data |{}|))
          (file (name "bad-checksum.bin") (data |{}|)
            (checksum "00000000000000000000000000000000"))
          (file (name "too-large.bin") (data |{}|) (checksum "{}"))))"#,
        base64(b"not a zstd frame"),
        encoded(payload),
        encoded(payload),
        checksum(payload),
    );
    let view = PcbView::parse(&source, PcbLimits::default()).expect("board");
    let files = view
        .embedded_files()
        .collect::<Result<Vec<_>, _>>()
        .expect("metadata");

    for (index, expected_name) in [(0, "bad-zstd.bin"), (1, "bad-checksum.bin")] {
        let error = view
            .decode_embedded_file(&files[index], EmbeddedDecodeLimits::default())
            .expect_err("malformed payload");
        assert!(error.message.contains(expected_name));
        assert_eq!(error.token.as_deref(), Some(expected_name));
    }

    let error = view
        .decode_embedded_file(
            &files[2],
            EmbeddedDecodeLimits {
                max_decoded_bytes: payload.len() - 1,
                ..EmbeddedDecodeLimits::default()
            },
        )
        .expect_err("decoded limit");
    assert_eq!(error.kind, kicad_monkey_core::ErrorKind::ResourceLimit);
    assert!(error.message.contains("too-large.bin"));
}

#[cfg(feature = "embedded-resource-zstd")]
#[test]
fn nonempty_payload_requires_a_source_checksum() {
    let source = format!(
        r#"(footprint "Checksum" (embedded_files
          (file (name "missing-checksum.bin") (data |{}|))))"#,
        encoded(b"valid compressed bytes")
    );
    let view = FootprintView::parse(&source, FootprintLimits::default()).expect("footprint");
    let file = view
        .embedded_files()
        .next()
        .expect("file")
        .expect("metadata");
    let error = view
        .decode_embedded_file(&file, EmbeddedDecodeLimits::default())
        .expect_err("nonempty payload without integrity evidence");
    assert_eq!(error.kind, kicad_monkey_core::ErrorKind::UnexpectedToken);
    assert_eq!(error.token.as_deref(), Some("missing-checksum.bin"));
    assert!(error.message.contains("missing its checksum"));
}

#[test]
fn metadata_and_encoded_access_work_without_the_decoder_feature() {
    let source = r#"(kicad_pcb
      (footprint "One" (embedded_files (file (name "local.bin") (data))))
      (embedded_files (file (name "board.bin") (data |AAAA|))))"#;
    let view = PcbView::parse_selected(
        source,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::EmbeddedFiles),
    )
    .expect("board-only resources");
    let board = view
        .embedded_files()
        .next()
        .expect("board declaration")
        .expect("metadata");
    assert_eq!(board.owner, EmbeddedFileOwner::Board);
    assert_eq!(
        view.embedded_file_encoded_data(&board, 4)
            .expect("encoded payload"),
        Some("AAAA".to_owned())
    );
    assert_eq!(view.counts().footprints, 0);
    assert_eq!(view.counts().footprint_embedded_files, 0);

    let nested = PcbView::parse_selected(
        source,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::FootprintEmbeddedFiles),
    )
    .expect("footprint resources");
    let local = nested
        .footprint_embedded_files()
        .next()
        .expect("local declaration")
        .expect("metadata");
    assert_eq!(
        local.owner,
        EmbeddedFileOwner::EmbeddedFootprint { footprint_index: 0 }
    );
    assert_eq!(local.data_presence, EmbeddedDataPresence::Empty);
    assert_eq!(nested.counts().embedded_files, 0);
    assert_eq!(nested.counts().footprint_embedded_files, 1);
}

#[cfg(feature = "embedded-resource-zstd")]
fn checksum(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(feature = "embedded-resource-zstd")]
fn encoded(bytes: &[u8]) -> String {
    base64(&zstd::stream::encode_all(bytes, 0).expect("zstd encode"))
}

#[cfg(feature = "embedded-resource-zstd")]
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        encoded.push(char::from(ALPHABET[(first >> 2) as usize]));
        encoded.push(char::from(
            ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize],
        ));
        encoded.push(if chunk.len() > 1 {
            char::from(ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize])
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            char::from(ALPHABET[(third & 0x3f) as usize])
        } else {
            '='
        });
    }
    encoded
}
