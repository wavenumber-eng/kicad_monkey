use super::parse_iu;
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
