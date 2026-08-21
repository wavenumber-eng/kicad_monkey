use super::{SchematicPinShape, parse_iu};
use serde::Deserialize;

#[derive(Deserialize)]
struct CoordinateVectors {
    cases: Vec<CoordinateCase>,
}

#[derive(Deserialize)]
struct CoordinateCase {
    name: String,
    millimetres: String,
    expected_iu: Option<String>,
}

#[test]
fn exact_decimal_coordinates_match_shared_ties_even_and_range_vectors() {
    let vectors: CoordinateVectors = serde_json::from_str(include_str!(
        "../../../../../tests/parity/schematic_coordinate_iu_vectors.json"
    ))
    .expect("coordinate vectors");
    for case in vectors.cases {
        let actual = parse_iu(&case.millimetres, "vector");
        match case.expected_iu {
            Some(expected) => assert_eq!(
                actual.expect(&case.name).to_string(),
                expected,
                "{}",
                case.name
            ),
            None => assert!(actual.is_err(), "{} unexpectedly decoded", case.name),
        }
    }
}

#[test]
fn sheet_pin_shapes_cover_the_complete_python_label_shape_vocabulary() {
    let cases = [
        ("input", SchematicPinShape::Input),
        ("output", SchematicPinShape::Output),
        ("bidirectional", SchematicPinShape::Bidirectional),
        ("tri_state", SchematicPinShape::TriState),
        ("passive", SchematicPinShape::Passive),
        ("dot", SchematicPinShape::Dot),
        ("round", SchematicPinShape::Round),
        ("diamond", SchematicPinShape::Diamond),
        ("rectangle", SchematicPinShape::Rectangle),
    ];
    for (source, expected) in cases {
        assert_eq!(SchematicPinShape::from_source(Some(source)), expected);
        assert_eq!(expected.as_str(), source);
    }
    assert_eq!(
        SchematicPinShape::from_source(None),
        SchematicPinShape::Input
    );
    assert_eq!(
        SchematicPinShape::from_source(Some("bogus")),
        SchematicPinShape::Input
    );
}
