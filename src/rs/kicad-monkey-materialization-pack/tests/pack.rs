//! Pack round trip and rejection paths.

use kicad_monkey_contracts::generated::materialization_document::MaterializationDocumentA0;
use kicad_monkey_materialization_pack::{
    COLUMN_ALIGNMENT, PACK_MAGIC, PackError, PackLimits, PackReader, encode_pack,
};
use serde_json::json;

fn document() -> MaterializationDocumentA0 {
    serde_json::from_value(json!({
        "schema": "kicad_monkey.materialization.a0",
        "type": "kicad_monkey.materialization.document",
        "version": "a0",
        "producer": { "name": "pack-test", "version": "0" },
        "source": { "sha256": "0".repeat(64), "source_bytes": "0" },
        "coordinate_system": { "unit": "nm", "x_axis": "board_right", "y_axis": "board_down" },
        "scope": { "products": ["copper"], "tile_size_nm": 10_000_000, "complete": false },
        "stack": { "slices": [ { "key": "copper.front", "role": "conductor", "order": 0 } ],
                   "regions": [ { "key": "rigid", "kind": "rigid", "bend_lines": [],
                                  "slice_keys": ["copper.front"] } ] },
        "nets": [ { "key": "GND", "name": "GND" } ],
        "net_classes": [], "diff_pairs": [], "components": [],
        "operations": [ {
            "kind": "surface", "index": 0, "action": "add", "material": "copper",
            "slice_key": "copper.front", "net_key": "GND",
            "shape": { "kind": "disk_2d", "center_nm": [0, 0], "radius_nm": 100 },
            "affine": { "m11": 1.0, "m12": 0.0, "m21": 0.0, "m22": 1.0, "tx_nm": 0, "ty_nm": 0 },
            "bounds": { "min_x_nm": -100, "min_y_nm": -100, "max_x_nm": 100, "max_y_nm": 100 }
        } ],
        "binding": [],
        "chunks": [ { "slice_key": "copper.front", "tile_x": 0, "tile_y": 0,
                      "bounds": { "min_x_nm": -100, "min_y_nm": -100, "max_x_nm": 100, "max_y_nm": 100 },
                      "net_keys": ["GND"], "first_operation_index": 0, "operation_count": 1 } ],
        "diagnostics": []
    }))
    .expect("document decodes")
}

fn open(bytes: &[u8]) -> Result<MaterializationDocumentA0, PackError> {
    PackReader::open(bytes, PackLimits::default())?.document()
}

#[test]
fn a_pack_round_trips_exactly_and_deterministically() {
    let original = document();
    let bytes = encode_pack(&original).expect("encode");
    assert_eq!(&bytes[..8], &PACK_MAGIC);
    assert_eq!(bytes.len() % COLUMN_ALIGNMENT, 0);
    assert_eq!(encode_pack(&original).expect("again"), bytes);
    let reader = PackReader::open(&bytes, PackLimits::default()).expect("open");
    assert!(reader.header().operations.is_empty());
    assert_eq!(reader.header().chunks.len(), 1);
    assert_eq!(reader.chunk(0).expect("chunk").len(), 1);
    assert_eq!(
        serde_json::to_string(&reader.document().expect("document")).expect("json"),
        serde_json::to_string(&original).expect("json")
    );
}

#[test]
fn the_reader_rejects_bad_preambles_truncation_and_oversize() {
    let bytes = encode_pack(&document()).expect("encode");
    let mut wrong_magic = bytes.clone();
    wrong_magic[0] = b'X';
    assert!(matches!(open(&wrong_magic), Err(PackError::BadMagic)));
    let mut wrong_version = bytes.clone();
    wrong_version[8] = 9;
    assert!(matches!(
        open(&wrong_version),
        Err(PackError::UnsupportedVersion(9))
    ));
    assert!(matches!(open(&bytes[..20]), Err(PackError::Truncated)));
    let tight = PackLimits {
        max_header_bytes: 8,
    };
    assert!(matches!(
        PackReader::open(&bytes, tight).map(|_| ()),
        Err(PackError::HeaderTooLarge(_))
    ));
}

#[test]
fn the_writer_rejects_an_invalid_document() {
    let mut invalid = document();
    invalid.chunks[0].slice_key = "copper.back".to_owned();
    assert!(matches!(encode_pack(&invalid), Err(PackError::Invalid(_))));
}

#[test]
fn corrupt_or_truncated_packs_fail_closed() {
    let bytes = encode_pack(&document()).expect("encode");
    for cut in 0..bytes.len() {
        assert!(open(&bytes[..cut]).is_err(), "truncated at {cut}");
    }
    for position in 16..bytes.len() {
        for flip in [0x01u8, 0x80, 0xff] {
            let mut damaged = bytes.clone();
            damaged[position] ^= flip;
            // Any outcome but a panic is acceptable; a decode that succeeds
            // must still be a valid document.
            if let Ok(document) = open(&damaged) {
                kicad_monkey_contracts::materialization_contract::validate_materialization_document(
                    &document,
                )
                .expect("a successful decode is valid");
            }
        }
    }
}
