//! Phase 1's exit gate, assertions 1-6 and 9. Assertions 7 and 8 — cross-process
//! byte stability, and the `.zip` against the directory — need two processes and
//! live in `byte_stability.rs`.
//!
//! The fixture's engineered properties, and which assertion consumes each, are
//! documented in `common/mod.rs`.

mod common;

use common::{
    FIXTURE_LINES, FIXTURE_ROUTES_KEPT, FIXTURE_ROUTES_SEEN, FIXTURE_STATIONS, FIXTURE_STOPS_SEEN,
    import_fixture, line_color, line_stations,
};
use llika_core::{LayoutParams, Network, RenderParams, build_schematic_svg};

/// Assertion 1: the whole point of the format. The file this crate writes is an
/// ordinary `llk-001` input file, and every later phase's map depends on it.
#[test]
fn the_imported_feed_is_accepted_by_from_input() {
    let (schema, _) = import_fixture();
    Network::from_input(&schema).expect("the imported schema is a legal network");
}

/// Assertion 2: literals a person counted from the fixture.
///
/// The ordered list reads `M1`, which has **exactly one trip** and so one
/// pattern. Keyed to `M5` — whose longest trip is deliberately not its modal
/// one — it would break at Phase 3 by design, and churning a literal across
/// phases is what the fixture exists to avoid.
#[test]
fn the_fixture_imports_to_the_counted_stations_lines_and_order() {
    let (schema, report) = import_fixture();

    assert_eq!(schema.stations.len(), FIXTURE_STATIONS);
    assert_eq!(schema.lines.len(), FIXTURE_LINES);
    assert_eq!(report.stations_emitted, FIXTURE_STATIONS);
    assert_eq!(report.stops_seen, FIXTURE_STOPS_SEEN);

    assert_eq!(
        line_stations(&schema, "M1"),
        ["WST", "OLD", "CEN", "MKT", "EAST"],
    );

    // Stations come out in `stops.txt` row order and lines in `routes.txt` row
    // order — §2.5, and the property `llk-001` §2.2's grid tie-break reads.
    let ids: Vec<&str> = schema.stations.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "CEN", "WST", "OLD", "MKT", "EAST", "SOU", "NOR", "HIL", "RIV", "QUA", "FAI",
        ],
    );
    let line_ids: Vec<&str> = schema.lines.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(line_ids, ["M1", "M2", "M3", "M5", "M6"]);
}

/// Assertion 3: the filter **removes** something, rather than passing vacuously.
///
/// `DEP` is the second half and the sharper one: it is served only by `B1`, so a
/// filter that dropped the route but left its stops behind would put a stray
/// isolated marker on the poster.
#[test]
fn the_route_type_filter_removes_the_bus_route_and_its_stop() {
    let (schema, report) = import_fixture();

    assert_eq!(report.routes_seen, FIXTURE_ROUTES_SEEN);
    assert_eq!(report.routes_kept, FIXTURE_ROUTES_KEPT);
    assert!(
        !schema.lines.iter().any(|line| line.id == "B1"),
        "the bus route survived the filter",
    );
    assert!(
        !schema.stations.iter().any(|s| s.id == "DEP"),
        "a stop only the filtered-out route serves was emitted",
    );
}

/// Assertion 4: the trip whose rows are written **out of** `stop_sequence` order
/// reads in `stop_sequence` order.
///
/// This is the only assertion in the gate that separates a reader that sorts
/// from one that does not. The naive version of it — non-consecutive values in
/// ascending row order — passes on both, which is why `M2_t1` is written 30, 10,
/// 20 rather than merely 10, 20, 30.
#[test]
fn a_trip_written_out_of_sequence_reads_in_sequence() {
    let (schema, _) = import_fixture();

    assert_eq!(line_stations(&schema, "M2"), ["SOU", "CEN", "OLD"]);
    // The control: this is what a reader ignoring `stop_sequence` produces.
    assert_ne!(line_stations(&schema, "M2"), ["OLD", "SOU", "CEN"]);
}

/// Assertion 5, both halves — and the **first** is the one that discriminates.
///
/// A parser typed off the reflex reading of §2.1 dies on `CEN_E1`'s empty
/// coordinates before the second half is ever reached. The second holds under
/// the emit rule whether the `location_type` filter exists or not, since no line
/// references an entrance.
#[test]
fn an_entrance_row_with_no_coordinates_neither_fails_the_parse_nor_becomes_a_station() {
    let (schema, _) = import_fixture();

    assert!(
        !schema.stations.iter().any(|s| s.id == "CEN_E1"),
        "the entrance was emitted as a station",
    );
}

/// Assertion 6, both halves, because §2.4 treats them as different cases and the
/// fallback is where an implementer would collapse them.
///
/// `B1` also states no colour and sits **above** `M3` in `routes.txt`. It is
/// filtered out, so it is not a kept route and must not consume a palette index
/// — an implementation counting all routes gives `M3` the second colour.
#[test]
fn a_colourless_route_takes_the_first_palette_colour_and_an_explicit_white_stays_white() {
    let (schema, _) = import_fixture();

    assert_eq!(line_color(&schema, "M3"), llika_gtfs::FALLBACK_PALETTE[0]);
    assert_eq!(line_color(&schema, "M3"), "#E4002B");
    assert_eq!(line_color(&schema, "M5"), "#FFFFFF");

    // Every other surviving route states its own colour, so exactly one fallback
    // fired and the index above is not an accident of a wider sweep. `M4` stated
    // one too and is dropped, so this fixture cannot say whether a dropped route
    // consumes an index; the code decides it by checking the drop first.
    assert_eq!(line_color(&schema, "M1"), "#0057B8");
    assert_eq!(line_color(&schema, "M2"), "#FF8200");
    assert_eq!(line_color(&schema, "M6"), "#00A3E0");
}

/// §2.4's three-rung name ladder, all three rungs on one fixture.
///
/// The `route_id` rung is stricter than GTFS requires — the standard says at
/// least one of the two name columns must be defined — and exists so a feed
/// breaking that rule still imports.
#[test]
fn a_line_name_falls_from_short_to_long_to_the_route_id() {
    let (schema, _) = import_fixture();
    let name = |id: &str| {
        schema
            .lines
            .iter()
            .find(|line| line.id == id)
            .map(|line| line.name.clone())
            .expect("the line is in the import")
    };

    assert_eq!(name("M1"), "1", "route_short_name");
    assert_eq!(name("M6"), "Bay Loop", "route_long_name");
    assert_eq!(name("M5"), "M5", "route_id");
}

/// The naive representative trip this phase ships, pinned so Phase 3's change is
/// visible as a change rather than a coincidence.
///
/// `M5`'s longest trip is a six-stop special and its modal one is the four-stop
/// pattern three trips run; OQ-1 says the longest is the wrong answer and Phase 3
/// replaces it. `M6`'s four trips all have three stops, so the tie-break decides
/// — the earlier `trips.txt` row, which is `M6_R1`.
#[test]
fn the_representative_trip_is_the_longest_with_the_earlier_trip_winning_a_tie() {
    let (schema, _) = import_fixture();

    assert_eq!(
        line_stations(&schema, "M5"),
        ["NOR", "HIL", "RIV", "OLD", "CEN", "FAI"],
    );
    // The modal pattern, which Phase 3 will switch to.
    assert_ne!(line_stations(&schema, "M5"), ["NOR", "HIL", "RIV", "OLD"]);

    assert_eq!(line_stations(&schema, "M6"), ["MKT", "QUA", "FAI"]);
    assert_ne!(line_stations(&schema, "M6"), ["FAI", "QUA", "SOU"]);
}

/// Assertion 9: `llika` draws the imported file without error.
///
/// **Not through the binary.** `CARGO_BIN_EXE_llika` is only set for integration
/// tests of the package declaring that binary, and §2.5 points this crate's
/// dependency at `llika-core` and not at `llika-cli`. So this calls
/// `build_schematic_svg`, which is the same pipeline and keeps the dependency
/// direction the crate layout pins.
#[test]
fn llika_draws_the_imported_network() {
    let (schema, _) = import_fixture();

    let svg = build_schematic_svg(&schema, &LayoutParams::default(), &RenderParams::default())
        .expect("the imported network draws");

    assert!(svg.starts_with("<svg"), "not an SVG document: {svg:.60}");
    // One stroke per line and one marker per station. Since the collapse `CEN`,
    // `OLD` and `MKT` draw once each rather than twice, which is the difference
    // between a transit diagram and a duplicated one.
    assert_eq!(svg.matches("<path").count(), FIXTURE_LINES);
    assert_eq!(svg.matches("<circle").count(), FIXTURE_STATIONS);
}
