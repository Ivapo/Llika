//! Phase 1 gate, assertions 1, 2, 6 and 7: what the fixture is, and what the
//! grid does with it.

mod common;

use std::collections::BTreeSet;

use common::{SAMPLE_EDGES, SAMPLE_LINES, SAMPLE_STATIONS, sample};
use llika_core::grid::raw_cell;
use llika_core::{LayoutParams, Network, run_layout};

/// Assertion 1 — the fixture is the network the spec's OQ-5 describes.
#[test]
fn the_fixture_parses_to_seventeen_stations_three_lines_and_seventeen_corridors() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    assert_eq!(network.stations().len(), SAMPLE_STATIONS);
    assert_eq!(network.lines().len(), SAMPLE_LINES);
    assert_eq!(network.edge_count(), SAMPLE_EDGES);
}

/// Assertion 2 — the degrees the later phases are keyed to. `central` is the
/// true interchange; `oldtown` and `eastbank` are the two consecutive interior
/// trunk stations that give Phase 5 a parallel segment rather than a lens.
#[test]
fn the_interchange_and_the_trunk_have_the_shape_the_later_phases_need() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    let central = network.station_index("central").expect("central exists");
    assert_eq!(network.degree(central), 4);
    assert_eq!(network.lines_at_station(central).len(), 3);

    for id in ["oldtown", "eastbank"] {
        let station = network.station_index(id).expect("trunk station exists");
        assert_eq!(network.degree(station), 2, "{id} is an interior station");
        assert_eq!(
            line_ids(&network, station),
            BTreeSet::from(["green".to_string(), "red".to_string()]),
            "{id} carries exactly Red and Green"
        );
    }
}

/// Assertion 6 — one station per cell, and the tie-break was actually
/// exercised. Both halves are needed: the first alone passes vacuously on a
/// fixture where nothing ever collided.
#[test]
fn every_station_holds_a_distinct_cell_and_the_collision_pair_really_collided() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &LayoutParams::default());

    let distinct: BTreeSet<_> = layout.positions().iter().collect();
    assert_eq!(
        distinct.len(),
        SAMPLE_STATIONS,
        "the grid holds at most one station per cell"
    );

    // Northgate and Lakeside are two northern termini on different lines, 403 m
    // apart — well under a median-length edge.
    let northgate = network
        .station_index("northgate")
        .expect("northgate exists");
    let lakeside = network.station_index("lakeside").expect("lakeside exists");
    let g = layout.grid_spacing();

    assert_eq!(
        raw_cell(layout.projected()[northgate], g),
        raw_cell(layout.projected()[lakeside], g),
        "the pair rounds to one cell before the tie-break"
    );
    assert_ne!(
        layout.positions()[northgate],
        layout.positions()[lakeside],
        "the tie-break moved one of them out"
    );

    // First claim wins, and claims run in the input file's station order, where
    // northgate (index 8) precedes lakeside (index 12).
    assert_eq!(
        layout.positions()[northgate],
        raw_cell(layout.projected()[northgate], g),
        "the earlier station in the input keeps the cell"
    );
    // Ring 1 starts due east, so that is where the displaced station lands.
    assert_eq!(
        layout.positions()[lakeside],
        raw_cell(layout.projected()[lakeside], g).offset(1, 0),
    );
}

/// Assertion 7 — the projected plane is in metres and spans a city.
///
/// The range catches a degrees-for-metres error by four orders of magnitude. It
/// does **not** catch `cos(lat_c)` omitted from `x`, which inflates the span by
/// 27% at this latitude and stays inside the bounds; the hand-computed `g` in
/// `grid_spacing.rs` is what covers that.
#[test]
fn the_projected_bounding_box_spans_a_plausible_city_in_metres() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &LayoutParams::default());

    let xs: Vec<f64> = layout.projected().iter().map(|p| p.x).collect();
    let ys: Vec<f64> = layout.projected().iter().map(|p| p.y).collect();
    let span = |vs: &[f64]| {
        let min = vs.iter().copied().fold(f64::INFINITY, f64::min);
        let max = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        max - min
    };
    let (x_span, y_span) = (span(&xs), span(&ys));

    assert!(x_span > 0.0 && y_span > 0.0, "non-degenerate in both axes");

    let larger = x_span.max(y_span);
    let smaller = x_span.min(y_span);
    assert!(
        (1_000.0..=100_000.0).contains(&larger),
        "larger axis {larger} m is between 1 km and 100 km"
    );
    assert!(smaller > 0.0);

    // And the grid is derived from that same plane, so it is city-scaled too.
    // Its exact value is assertion 4's business, in `grid_spacing.rs`.
    assert!(layout.grid_spacing() > 0.0);
    assert!(layout.grid_spacing() < larger);
}

fn line_ids(network: &Network, station: usize) -> BTreeSet<String> {
    network
        .lines_at_station(station)
        .iter()
        .map(|line| network.lines()[*line].id.clone())
        .collect()
}
