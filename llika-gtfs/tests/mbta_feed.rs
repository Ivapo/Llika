//! Phase 6's gate: a second real city, of a different shape.
//!
//! `real_feed.rs` reads BART and is the only test in this crate whose author
//! never saw this spec. This one reads a second such feed — MBTA's — for a
//! reason BART cannot supply: every judgement of `llk-001`'s five criterion
//! weights so far rests on one city plus a 17-station fixture, and its **OQ-2**
//! has asked three times for the same measurement, a criteria vector on a
//! network that is not BART.
//!
//! ## Two kinds of test live here, and the split is OQ-8's argument
//!
//! **The archive is not in this repository.** OQ-8 decided that the feed which
//! earns a place in the tree is the one that serves as the *example*, and BART
//! already does — so what is committed is the imported **network**,
//! `tests/fixtures/golden/mbta.json`, with its provenance in
//! `tests/fixtures/mbta.md`.
//!
//! So the test that reads `tests/fixtures/mbta.zip` is `#[ignore]`d when that
//! file is absent, via the `mbta_feed` cfg `build.rs` emits, and the two that
//! read the committed network run everywhere — including on a fresh clone and in
//! CI. **The layout half is the half a second city exists for, and it is the half
//! that always runs.**
//!
//! **Absent skips; stale fails.** `https://cdn.mbta.com/MBTA_GTFS.zip` always
//! serves the current build, so a second person fetching later gets a *different
//! feed* and the literals below fail. That is `bart.md`'s discipline one feed
//! over: `mbta.md` carries the sha256 and says the same, so the failure is
//! diagnosable from the file this test points at. There is no runtime skip in
//! libtest that could have made the stale case quiet, and the compile-time one
//! cannot see a hash.

use std::path::{Path, PathBuf};

use llika_core::{
    InputSchema, LayoutParams, Network, RenderParams, evaluate, parse_input, run_layout,
};
use llika_gtfs::{ImportParams, ImportReport, import};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The fetched archive, stated here the way `real_feed.rs:bart` states its own.
/// `.gitignore` keeps it out of the tree and `build.rs` watches this same path,
/// so the gate and the cfg cannot disagree about where the feed is.
fn mbta() -> PathBuf {
    fixtures().join("mbta.zip")
}

/// The default params, deliberately — and here that is a stronger statement than
/// it was for BART. MBTA publishes 400 routes, of which the eight rapid-transit
/// ones are `route_type` 0 and 1: the default selects them without being told,
/// and the 369 bus, 14 commuter-rail and 9 ferry routes are filtered out.
fn import_mbta() -> (InputSchema, ImportReport) {
    import(&mbta(), &ImportParams::default()).expect("MBTA's feed imports")
}

/// The committed network — this phase's product, and what the layout gates read.
fn committed_network() -> InputSchema {
    let path = fixtures().join("golden/mbta.json");
    let json = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the committed network at {} is readable: {e}",
            path.display()
        )
    });
    parse_input(&json).expect("the committed network parses")
}

/// Gate 1. **Measured, not hand-counted** — `stops_seen` is a raw `stops.txt`
/// row count and runs to five figures here, which Phase 4's "hand-counted"
/// wording does not survive at city scale.
#[test]
#[cfg_attr(
    not(mbta_feed),
    ignore = "fetch https://cdn.mbta.com/MBTA_GTFS.zip to llika-gtfs/tests/fixtures/mbta.zip"
)]
fn the_mbta_feed_imports_to_the_measured_stations_lines_and_routes() {
    let (schema, report) = import_mbta();

    assert_eq!(schema.stations.len(), 119);
    assert_eq!(schema.lines.len(), 8);

    assert_eq!(
        report.routes_seen, 400,
        "eight rapid-transit routes among 369 bus, 14 commuter-rail and 9 ferry"
    );
    assert_eq!(report.routes_kept, 8);
    assert_eq!(report.stops_seen, 10_298);
    assert_eq!(report.stations_emitted, 119);

    // Nothing degenerate, and nothing to merge: MBTA publishes no per-direction
    // route split, so the hazard BART's feed exists here to exercise is absent
    // from this one. Asserted rather than left unstated — an empty
    // `routes_merged` is a fact about the feed, not an oversight in the gate.
    assert_eq!(report.routes_dropped, []);
    assert_eq!(report.routes_merged, []);
}

/// Gate 2. Reads the committed network, so it runs on a fresh clone.
#[test]
fn llika_draws_the_mbta_network() {
    let schema = committed_network();

    let svg = llika_core::build_schematic_svg(
        &schema,
        &LayoutParams::default(),
        &RenderParams::default(),
    )
    .expect("MBTA's imported network is a valid one");

    assert!(svg.starts_with("<svg"));
    assert_eq!(svg.matches("<circle").count(), schema.stations.len());
    assert_eq!(svg.matches("<path").count(), schema.lines.len());
}

/// One weighting, named for what it is rather than by its numbers.
struct Weighting {
    name: &'static str,
    params: LayoutParams,
    /// `c1..c5`, measured at this phase's implementation.
    expected: [f64; 5],
}

fn weights(w1: f64, w2: f64, w3: f64, w4: f64, w5: f64) -> LayoutParams {
    LayoutParams {
        w_crossings: w1,
        w_edge_length: w2,
        w_angular_resolution: w3,
        w_straightness: w4,
        w_octilinearity: w5,
        ..LayoutParams::default()
    }
}

/// **Four, and the fourth is why this is an experiment rather than a formality.**
/// The first three all carry `w1 = 5`, so comparing them to each other cannot say
/// anything about `w_crossings` — and `w_crossings` is the one weight nothing in
/// this tree has ever constrained, because until MBTA no network here crossed at
/// all under a surviving weighting.
fn weightings() -> Vec<Weighting> {
    vec![
        Weighting {
            name: "shipped 5/1/0.5/0.25/10",
            params: weights(5.0, 1.0, 0.5, 0.25, 10.0),
            expected: [1.0, 6.77460326, 64.40264940, 19.63495408, 0.0],
        },
        Weighting {
            name: "Phase 7 runner-up 5/1/1/0.5/10",
            params: weights(5.0, 1.0, 1.0, 0.5, 10.0),
            expected: [1.0, 8.94617613, 59.69026042, 17.27875959, 0.0],
        },
        Weighting {
            name: "pre-Phase-7 5/1/1/2/5",
            params: weights(5.0, 1.0, 1.0, 2.0, 5.0),
            expected: [2.0, 6.60303038, 62.83185307, 18.84955592, 0.0],
        },
        Weighting {
            name: "w1 raised 100/1/0.5/0.25/10",
            params: weights(100.0, 1.0, 0.5, 0.25, 10.0),
            expected: [0.0, 5.94617613, 63.35545185, 21.20575041, 0.0],
        },
    ]
}

/// The five unweighted criteria, which is the only comparable thing across
/// weightings.
type Cost5 = [f64; 5];

fn score(schema: &InputSchema, params: &LayoutParams) -> Cost5 {
    let network = Network::from_input(schema).expect("MBTA's imported network is a valid one");
    let layout = run_layout(&network, params);
    let cost = evaluate(&network, layout.positions(), layout.target_edge_cells());
    [cost.c1, cost.c2, cost.c3, cost.c4, cost.c5]
}

/// Pareto dominance over the **unweighted** criteria: `≤` on all five and `<` on
/// at least one, with a `1e-9` guard.
///
/// Restated here rather than inherited by pointer, because it is the whole method
/// of gate 4. It has to be `≤`-and-one-`<` rather than strict improvement on all
/// five: a point already at `c1 = 0` cannot strictly improve it, and a reading
/// that demanded so would score nothing and conclude the opposite.
///
/// **Never by `t`.** The weighted total is *defined* by the weights, so it is not
/// comparable across them — ranking four weightings by `t` ranks four different
/// questions.
fn dominates(a: Cost5, b: Cost5) -> bool {
    const GUARD: f64 = 1e-9;
    a.iter().zip(b).all(|(x, y)| *x <= y + GUARD) && a.iter().zip(b).any(|(x, y)| *x < y - GUARD)
}

/// Gate 4, and the assertion the whole phase exists to keep running. Reads the
/// committed network, so it runs on a fresh clone with no archive.
///
/// **A committed test, not a one-off measurement**, so the numbers cannot rot
/// silently — which is the difference between this and the throwaway crate that
/// first produced them during Phase 6's review. Every literal above was
/// re-derived here rather than copied from that round; all sixteen agree with it
/// to the digit, which is corroboration and not the source.
///
/// **Exact for `c1` and `c5`, tolerant for the rest**, as
/// `real_feed.rs:bart_draws_to_the_shipped_criteria_vector` argues: `c1` is an
/// integer count, and a sum over octilinear edges is exactly `+0.0`, so a
/// tolerance there would only hide a near-miss. The bound is **absolute** and
/// written inline, for that test's reason — a relative `1e-6` against a
/// six-decimal literal is only sound for values `≥ 0.5`.
///
/// **It costs ≈63 s under a debug `cargo test`** — four layouts at 119 stations
/// and 120 corridors, against `real_feed.rs`'s ≈25 s radius test, which is four
/// layouts at 50 and 50. Stated so the price is chosen rather than discovered.
/// The four weightings are the experiment; drop the runner-up before the
/// `w1`-raised one, which is the only member that can say anything about `w1`.
///
/// **This pins a property of the layout to a third-party snapshot that expires.**
/// `tests/fixtures/mbta.md` says so.
#[test]
fn mbta_draws_to_the_measured_criteria_vectors() {
    let schema = committed_network();

    let measured: Vec<(&str, Cost5)> = weightings()
        .into_iter()
        .map(|w| {
            let got = score(&schema, &w.params);
            for (i, (actual, expected)) in got.iter().zip(w.expected).enumerate() {
                let exact = i == 0 || i == 4;
                let ok = if exact {
                    *actual == expected
                } else {
                    (actual - expected).abs() < 1e-6
                };
                assert!(
                    ok,
                    "{}: c{} is {actual}, expected {expected}",
                    w.name,
                    i + 1
                );
            }
            (w.name, got)
        })
        .collect();

    // **Whether any weighting yields `c1 > 0`, asserted explicitly** rather than
    // left to be read off the literals. Three of the four do, which is what makes
    // MBTA the first network in this tree that constrains `w_crossings` at all:
    // `llk-001` Phase 7 recorded `c1 = 0` for every surviving weighting on both
    // fixtures *and* BART.
    let crossing: Vec<&str> = measured
        .iter()
        .filter(|(_, c)| c[0] > 0.0)
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        crossing,
        [
            "shipped 5/1/0.5/0.25/10",
            "Phase 7 runner-up 5/1/1/0.5/10",
            "pre-Phase-7 5/1/1/2/5",
        ],
        "which weightings leave MBTA a crossing"
    );

    // **The verdict, and it is a null one: no weighting dominates any other.**
    // Not a failure of the experiment — it is the experiment's answer, and the
    // reason this phase produces the comparison without moving a default. Raising
    // `w1` to 100 removes the crossing and improves `c2` and `c3`, and pays for it
    // in `c4`: 19.634954 → 21.205750, which is exactly `π/2`, one right angle's
    // worth of added bend. A trade, not a strict improvement.
    for (a_name, a) in &measured {
        for (b_name, b) in &measured {
            assert!(
                !dominates(*a, *b),
                "{a_name} dominates {b_name}, and Phase 6 recorded that none does"
            );
        }
    }
}
