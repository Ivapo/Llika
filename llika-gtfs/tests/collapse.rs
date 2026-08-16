//! Phase 2's exit gate, assertions 1-6. Assertion 7 — cross-process byte
//! stability, still — is `byte_stability.rs`, unchanged from Phase 1 because the
//! property is.
//!
//! What this phase claims is not that the JSON changed shape. It is that the
//! **graph `llk-001` builds from it** changed: an interchange that was several
//! degree-2 platforms is one station of degree 4, and a corridor that was
//! several singleton edges carries several lines. So the assertions that matter
//! read `llika-core/src/model.rs` rather than the file — the file is where a
//! wrong implementation looks most convincing.
//!
//! **The bundle is asserted on the graph and not on the drawn stroke**, and that
//! is not laziness. `llika-core/src/render/corridor.rs` offsets a line only
//! across a *run* — consecutive corridors carrying the same line set — and
//! collapses every offset to zero at a station of degree ≠ 2. Every multi-line
//! corridor in this fixture runs between two interchanges, so its run is one
//! corridor long and its strokes are drawn coincident by design. What Phase 2
//! delivers is the shared `LineSet`; a fixture that also showed the separation
//! would need a shared run two corridors long, and OQ-5 does not fix one.
//!
//! The fixture's engineered properties, and which assertion consumes each, are
//! documented in `common/mod.rs`.

mod common;

use common::{
    FIXTURE_LINES, FIXTURE_ROUTES_DROPPED, FIXTURE_ROUTES_KEPT, FIXTURE_ROUTES_SEEN,
    FIXTURE_STATIONS, PHASE_1_LINES, PHASE_1_STATIONS, feed_dir, import_fixture, line_stations,
    run_importer, scratch,
};
use llika_core::io::InputError;
use llika_core::{InputSchema, Network};
use llika_gtfs::{DropReason, DroppedRoute};

/// How Phase 1's counts carry into Phase 2's, checked where they are written
/// rather than inside a test — both sides are constants, so the compiler is the
/// right place to catch a literal edited in only one of them.
///
/// The **line** count carries as an exact delta: `M4` is the only route that can
/// drop. The **station** count carries only as a relation, strictly below,
/// because its exact delta is not derivable from the spec.
const _: () = assert!(FIXTURE_STATIONS < PHASE_1_STATIONS);
const _: () = assert!(FIXTURE_LINES == PHASE_1_LINES - 1);

/// The station index of an id, since `degree` and `lines_between` take indices.
fn index(network: &Network, id: &str) -> usize {
    network
        .station_index(id)
        .unwrap_or_else(|| panic!("station `{id}` is in the network"))
}

/// The line index of a route id, which is what a `LineSet` holds.
fn line_index(network: &Network, id: &str) -> usize {
    network
        .lines()
        .iter()
        .position(|line| line.id == id)
        .unwrap_or_else(|| panic!("line `{id}` is in the network"))
}

/// Assertion 1: the split-platform station emits **once**, and the two routes
/// serving its two different platforms share **one corridor**.
///
/// Read from `lines_between` and not from the JSON, because the claim is about
/// the graph. `M1` runs through `CEN_1` and `M2` through `CEN_2`, so before the
/// collapse they could not produce the same consecutive pair and the `OLD`–`CEN`
/// corridor was three separate edges carrying one, one and two lines. This is
/// `LineSet` recording more than one line on imported data for the first time,
/// which is what the bundling renderer `llk-001` Phase 5 shipped needs.
#[test]
fn the_split_platform_station_emits_once_and_its_routes_share_one_corridor() {
    let (schema, _) = import_fixture();

    assert_eq!(
        schema.stations.iter().filter(|s| s.id == "CEN").count(),
        1,
        "the parent station is emitted once, never once per platform",
    );
    for platform in ["CEN_1", "CEN_2", "OLD_1", "OLD_2", "MKT_1", "MKT_2"] {
        assert!(
            !schema.stations.iter().any(|s| s.id == platform),
            "platform `{platform}` survived as a station of its own",
        );
    }
    // It takes the parent row's name and coordinates too, not its first
    // platform's — a second, independent witness of the same rule.
    let central = schema
        .stations
        .iter()
        .find(|s| s.id == "CEN")
        .expect("CEN is emitted");
    assert_eq!(central.name, "Central");
    assert_ne!(central.name, "Central Platform 1");

    let network = Network::from_input(&schema).expect("the imported schema is a legal network");
    let corridor = network
        .lines_between(index(&network, "CEN"), index(&network, "OLD"))
        .expect("CEN and OLD are adjacent");

    for route in ["M1", "M2"] {
        assert!(
            corridor.contains(&line_index(&network, route)),
            "`{route}` does not run over the CEN–OLD corridor",
        );
    }
    // Hand-counted from the fixture: `M1`, `M2`, `M3` and `M5` all run OLD–CEN.
    // `M6` never touches CEN.
    assert_eq!(corridor.len(), 4);
}

/// Assertion 2, **both halves in one test**, because the first alone passes on a
/// feed that never had split platforms.
///
/// The "before" is Phase 1's committed literal rather than a re-run: §2.2 makes
/// the collapse neither an option nor a flag, so nothing here can produce a
/// pre-collapse import, and a flag to make the comparison runnable is exactly
/// what that section forbids.
#[test]
fn the_interchange_degree_and_the_counts_both_fall() {
    let (schema, _) = import_fixture();
    let network = Network::from_input(&schema).expect("the imported schema is a legal network");

    // Hand-counted from the fixture's corridors: CEN meets OLD, MKT, SOU and
    // FAI. Discriminating, because before the collapse `CEN_1` had degree 5 and
    // `CEN_2` degree 2 — 4 is neither, nor their sum.
    assert_eq!(network.degree(index(&network, "CEN")), 4);

    // Both counts against Phase 1's, whose relation the `const` assertions above
    // pin. Strictly below on stations, exactly one fewer line.
    assert_eq!(schema.stations.len(), FIXTURE_STATIONS);
    assert_eq!(schema.lines.len(), FIXTURE_LINES);
}

/// Assertion 3: a platform with no `parent_station` is its own station.
///
/// `WST` is on a kept route deliberately. §2.1 emits only referenced rows, so an
/// unreferenced parentless platform would emit nothing and this would fail for a
/// reason that has nothing to do with the rule under test.
#[test]
fn a_platform_with_no_parent_is_its_own_station() {
    let (schema, _) = import_fixture();

    assert!(
        schema.stations.iter().any(|s| s.id == "WST"),
        "the parentless platform did not survive as its own station",
    );
    assert_eq!(line_stations(&schema, "M1")[0], "WST");
}

/// Assertion 4: the consecutive-duplicate fold fires, and the result survives
/// `from_input`.
///
/// `M3` serves `OLD_1` then `OLD_2`, two platforms of one station in sequence.
/// The control is what makes this load-bearing rather than incidental: the same
/// schema with the fold undone is **rejected**, so the fold is doing work and
/// not merely describing what the feed already said.
#[test]
fn consecutive_platforms_of_one_station_fold_to_one() {
    let (schema, _) = import_fixture();

    assert_eq!(line_stations(&schema, "M3"), ["RIV", "OLD", "CEN", "SOU"]);
    Network::from_input(&schema).expect("the folded network is legal");

    let mut unfolded = InputSchema {
        stations: schema.stations.clone(),
        lines: schema.lines.clone(),
    };
    let m3 = unfolded
        .lines
        .iter_mut()
        .find(|line| line.id == "M3")
        .expect("M3 is in the import");
    m3.stations = ["RIV", "OLD", "OLD", "CEN", "SOU"]
        .map(str::to_string)
        .to_vec();

    let Err(rejected) = Network::from_input(&unfolded) else {
        panic!("without the fold the collapse produces a self-loop `llk-001` rejects");
    };
    assert_eq!(
        rejected,
        InputError::RepeatedStation {
            line: "M3".to_string(),
            station: "OLD".to_string(),
        },
    );
}

/// Assertion 5 and the resolution of **OQ-3**: a route collapsing to one station
/// is dropped, the drop is reported, the rest of the import succeeds, and the
/// binary **exits 0**.
///
/// The exit status is the half OQ-3 still had open, and a gate checking the drop
/// but not the status would leave the decision untested. Zero is the answer: the
/// file written is valid and drawable, and a real feed is expected to carry
/// degenerate routes, so failing the run would make a city unimportable over one
/// route in someone else's published data.
///
/// `M4` carries this assertion alone, and is a *different* route from assertion
/// 4's — that one has to survive.
#[test]
fn a_route_collapsing_to_one_station_is_dropped_reported_and_not_a_failure() {
    let (schema, report) = import_fixture();

    assert_eq!(
        report.routes_dropped,
        [DroppedRoute {
            route_id: "M4".to_string(),
            reason: DropReason::LineTooShort { stations: 1 },
        }],
    );
    assert_eq!(report.routes_dropped.len(), FIXTURE_ROUTES_DROPPED);
    assert!(
        !schema.lines.iter().any(|line| line.id == "M4"),
        "the short route survived as a line",
    );

    // The rest of the import is untouched: the drop is not a partial failure.
    assert_eq!(report.routes_seen, FIXTURE_ROUTES_SEEN);
    assert_eq!(report.routes_kept, FIXTURE_ROUTES_KEPT);
    assert_eq!(schema.lines.len(), FIXTURE_LINES);
    // `MKT` is still emitted — `M1` and `M6` reference it. A dropped route
    // contributes no stations, but it does not remove any either.
    assert!(schema.stations.iter().any(|s| s.id == "MKT"));

    let dir = scratch("llika-gtfs-dropped-route");
    let output = dir.join("city.json");
    let result = run_importer(&feed_dir(), &output);

    assert!(
        result.status.success(),
        "dropping a route is an ordinary outcome, not a failed run: {}",
        String::from_utf8_lossy(&result.stderr),
    );
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(
        stdout.contains("6 of 7 routes matched, 1 dropped"),
        "the summary must say what it dropped; it said: {stdout}",
    );
    assert!(output.exists(), "a dropped route must not stop the write");
}

/// Assertion 6: a collapsed station sits at its **parent's** `stops.txt` row, not
/// its first platform's.
///
/// It changes the map through `llk-001` §2.2's grid tie-break, which resolves two
/// stations rounding to one cell by `stations` array order, first claim wins.
///
/// **Only the order of `CEN` against `WST` discriminates.** Both readings emit
/// the same eleven ids, and `OLD` and `MKT` land in the same slot under both
/// because their rows are grouped. The fixture interleaves `CEN, WST, CEN_1` for
/// exactly this assertion.
#[test]
fn a_collapsed_station_sits_at_its_parents_row() {
    let (schema, _) = import_fixture();
    let ids: Vec<&str> = schema.stations.iter().map(|s| s.id.as_str()).collect();

    assert_eq!(&ids[..2], ["CEN", "WST"]);
    // The control: this is what emitting at the first platform's row produces.
    assert_ne!(&ids[..2], ["WST", "CEN"]);
}
