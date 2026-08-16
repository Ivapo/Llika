//! Phase 4's gate: a feed nobody in this repo authored.
//!
//! Every other gate in this crate reads `tests/fixtures/feed/`, nineteen stops
//! engineered to exercise a rule. This one reads BART's published archive, whose
//! author never saw this spec — so it is the only test here that can find out
//! that an assumption in §2.1 was wrong.
//!
//! **Every literal below is hand-counted from `tests/fixtures/bart.zip` and
//! moves when that file is refreshed.** The snapshot expires 2026-08-30;
//! `tests/fixtures/bart.md` says so, and says that refreshing it is a deliberate
//! act which names the moved literals in its commit message.

use std::path::PathBuf;

use llika_core::{InputSchema, LayoutParams, Line, Network, RenderParams, run_layout};
use llika_gtfs::{ImportParams, ImportReport, MergedRoute, import};

fn bart() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("bart.zip")
}

/// The default params, deliberately. BART carries no `route_type = 0`, so `0,1`
/// and `1` select the same twelve routes — which means this also pins that
/// OQ-2's default reads a real metro feed without being told to.
fn import_bart() -> (InputSchema, ImportReport) {
    import(&bart(), &ImportParams::default()).expect("BART's feed imports")
}

#[test]
fn the_bart_feed_imports_to_the_counted_stations_lines_and_routes() {
    let (schema, report) = import_bart();

    assert_eq!(schema.stations.len(), 50);
    assert_eq!(schema.lines.len(), 6);

    assert_eq!(
        report.routes_seen, 14,
        "twelve metro routes and two bus bridges"
    );
    assert_eq!(report.routes_kept, 12);
    assert_eq!(report.stops_seen, 287);
    assert_eq!(report.stations_emitted, 50);

    // Nothing degenerate: no route falls under two stations, which is the one
    // live member of `llk-001`'s five conditions.
    assert_eq!(report.routes_dropped, []);
}

/// The phase's real subject. BART publishes each line twice, one `route_id` per
/// direction with the same colour, so an importer that took the feed at its word
/// would draw twelve lines where a poster wants six.
#[test]
fn each_direction_pair_merges_into_the_earlier_route() {
    let (schema, report) = import_bart();

    let merged: Vec<(&str, &str)> = report
        .routes_merged
        .iter()
        .map(|MergedRoute { route_id, into }| (route_id.as_str(), into.as_str()))
        .collect();
    assert_eq!(
        merged,
        [
            ("2", "1"),   // Yellow-N into Yellow-S
            ("4", "3"),   // Orange-S into Orange-N
            ("6", "5"),   // Green-N into Green-S
            ("8", "7"),   // Red-N into Red-S
            ("12", "11"), // Blue-N into Blue-S
            ("20", "19"), // Grey-S into Grey-N
        ],
        "in routes.txt row order, each absorbed into the earlier row"
    );

    let survivors: Vec<&str> = schema.lines.iter().map(|line| line.id.as_str()).collect();
    assert_eq!(survivors, ["1", "3", "5", "7", "11", "19"]);

    // The colour a merged pair shares is stated by the feed, so nothing here
    // falls back to the palette. Six distinct BART colours out the far end.
    let colors: Vec<&str> = schema
        .lines
        .iter()
        .map(|line| line.color.as_str())
        .collect();
    assert_eq!(
        colors,
        [
            "#FFFF33", "#FF9933", "#339933", "#FF0000", "#0099CC", "#B0BEC7"
        ]
    );
}

/// One structural literal, so a silent change in collapse, trip selection or
/// merging is caught rather than merely counted. The endpoints are the half that
/// matters: a merge keeping the wrong direction leaves the length alone.
#[test]
fn the_yellow_line_runs_antioch_to_the_airport() {
    let (schema, _) = import_bart();

    let yellow = schema
        .lines
        .iter()
        .find(|line| line.id == "1")
        .expect("route 1 is Yellow-S");

    assert_eq!(yellow.stations.len(), 27);
    assert_eq!(yellow.stations.first().map(String::as_str), Some("ANTC"));
    assert_eq!(yellow.stations.last().map(String::as_str), Some("SFIA"));

    // The station ids are BART's parent-station rows, and the names ride along
    // unused — §1.2 still defers label placement.
    let named = |id: &str| {
        schema
            .stations
            .iter()
            .find(|station| station.id == id)
            .map(|station| station.name.clone())
            .expect("every referenced station is emitted")
    };
    assert_eq!(named("ANTC"), "Antioch");
    assert_eq!(named("SFIA"), "San Francisco International Airport");
}

#[test]
fn llika_draws_the_bart_network() {
    let (schema, _) = import_bart();

    // `Network::from_input` accepting it is the assertion the whole format
    // exists for, and `build_schematic_svg` is the same pipeline `llika` runs.
    // Called in-process rather than through the binary: `CARGO_BIN_EXE_llika` is
    // set only for the package declaring it, and §2.5 points this crate's
    // dependency at `llika-core`.
    let svg = llika_core::build_schematic_svg(
        &schema,
        &LayoutParams::default(),
        &RenderParams::default(),
    )
    .expect("BART's imported network is a valid one");

    assert!(svg.starts_with("<svg"));
    assert_eq!(svg.matches("<circle").count(), schema.stations.len());
    assert_eq!(svg.matches("<path").count(), schema.lines.len());
}

/// **The merge is not cosmetic, and this is where that is written down.**
///
/// The graph is identical either way — `llika-core/src/model.rs:LineSet` keys a
/// corridor on an unordered pair, so a reversed duplicate adds no edge. But
/// `llika-core/src/layout/cost.rs:c4_straightness` sums over **lines**, so an
/// unmerged pair pays every bend twice and the search weights a route's
/// straightness by how many times its operator published it. Fifty stations land
/// somewhere else for it.
#[test]
fn re_adding_the_absorbed_directions_moves_the_layout() {
    let (merged, report) = import_bart();

    let mut doubled = merged.clone();
    for line in &merged.lines {
        let mut stations = line.stations.clone();
        stations.reverse();
        doubled.lines.push(Line {
            id: format!("{}-reversed", line.id),
            name: line.name.clone(),
            color: line.color.clone(),
            stations,
        });
    }
    assert_eq!(
        doubled.lines.len(),
        merged.lines.len() + report.routes_merged.len()
    );

    let network = |schema| Network::from_input(schema).expect("both networks are valid");
    let layout = |schema| run_layout(&network(schema), &LayoutParams::default());

    let (before, after) = (layout(&merged), layout(&doubled));
    assert_eq!(
        before.grid_spacing(),
        after.grid_spacing(),
        "the grid is derived from the stations, which the merge does not touch"
    );
    assert_ne!(
        before.positions(),
        after.positions(),
        "if these ever agree, c4 has stopped summing over lines and the merge \
         has become the tidiness change it is documented not to be"
    );
}
