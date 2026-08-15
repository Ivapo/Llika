//! Phase 1 gate, assertion 3: each of the five §2.1 conditions is rejected.
//!
//! The inputs are inline rather than fixture files so each one visibly violates
//! exactly one condition — a test cannot quietly pass on the wrong error.

use metro_core::{InputError, Network, parse_input};

fn reject(json: &str) -> InputError {
    let input = parse_input(json).expect("the JSON itself is well-formed");
    Network::from_input(&input).expect_err("this input must be rejected")
}

#[test]
fn a_line_referencing_an_undefined_station_is_rejected() {
    let error = reject(
        r##"{
          "stations": [
            { "id": "a", "name": "A", "lat": 37.78, "lon": -122.42 },
            { "id": "b", "name": "B", "lat": 37.79, "lon": -122.41 }
          ],
          "lines": [
            { "id": "l1", "name": "L1", "color": "#000000",
              "stations": ["a", "ghost"] }
          ]
        }"##,
    );
    assert_eq!(
        error,
        InputError::UnknownStation {
            line: "l1".into(),
            station: "ghost".into(),
        }
    );
}

#[test]
fn a_duplicate_station_id_is_rejected() {
    let error = reject(
        r##"{
          "stations": [
            { "id": "a", "name": "A", "lat": 37.78, "lon": -122.42 },
            { "id": "a", "name": "A again", "lat": 37.79, "lon": -122.41 }
          ],
          "lines": []
        }"##,
    );
    assert_eq!(error, InputError::DuplicateStation { id: "a".into() });
}

#[test]
fn a_duplicate_line_id_is_rejected() {
    let error = reject(
        r##"{
          "stations": [
            { "id": "a", "name": "A", "lat": 37.78, "lon": -122.42 },
            { "id": "b", "name": "B", "lat": 37.79, "lon": -122.41 }
          ],
          "lines": [
            { "id": "l1", "name": "L1", "color": "#000000", "stations": ["a", "b"] },
            { "id": "l1", "name": "L1 again", "color": "#111111", "stations": ["b", "a"] }
          ]
        }"##,
    );
    assert_eq!(error, InputError::DuplicateLine { id: "l1".into() });
}

#[test]
fn a_line_with_fewer_than_two_stations_is_rejected() {
    let error = reject(
        r##"{
          "stations": [
            { "id": "a", "name": "A", "lat": 37.78, "lon": -122.42 }
          ],
          "lines": [
            { "id": "l1", "name": "L1", "color": "#000000", "stations": ["a"] }
          ]
        }"##,
    );
    assert_eq!(
        error,
        InputError::LineTooShort {
            id: "l1".into(),
            count: 1,
        }
    );
}

#[test]
fn the_same_station_twice_consecutively_is_rejected() {
    let error = reject(
        r##"{
          "stations": [
            { "id": "a", "name": "A", "lat": 37.78, "lon": -122.42 },
            { "id": "b", "name": "B", "lat": 37.79, "lon": -122.41 }
          ],
          "lines": [
            { "id": "l1", "name": "L1", "color": "#000000",
              "stations": ["a", "a", "b"] }
          ]
        }"##,
    );
    assert_eq!(
        error,
        InputError::RepeatedStation {
            line: "l1".into(),
            station: "a".into(),
        }
    );
}

/// The condition that is deliberately **not** an error: an isolated node is a
/// degenerate but drawable map, and rejecting it would fail a valid
/// single-station input.
#[test]
fn a_station_no_line_references_is_accepted() {
    let input = parse_input(
        r##"{
          "stations": [
            { "id": "a", "name": "A", "lat": 37.78, "lon": -122.42 },
            { "id": "b", "name": "B", "lat": 37.79, "lon": -122.41 },
            { "id": "lonely", "name": "Lonely", "lat": 37.80, "lon": -122.40 }
          ],
          "lines": [
            { "id": "l1", "name": "L1", "color": "#000000", "stations": ["a", "b"] }
          ]
        }"##,
    )
    .expect("well-formed");
    let network = Network::from_input(&input).expect("an isolated node is legal");

    let lonely = network.station_index("lonely").expect("lonely exists");
    assert_eq!(network.degree(lonely), 0);
    assert_eq!(network.edge_count(), 1);
}
