//! Phase 3 gate: the search lowers the cost, and keeps everything the snap
//! established while doing it.
//!
//! **Every measurement here is against `iterations = 0`**, which reproduces the
//! snap-only layout. That baseline is what replaces the Phase 1 golden file
//! retired in this phase: there is otherwise no committed artifact of "the map
//! before the search" left to compare against, and a golden would fail on every
//! legitimate improvement anyway.
//!
//! The rejection rules are unit-tested in `layout/candidate.rs`, where the
//! predicate they turn on is reachable.

mod common;

use common::{CROSSING_C1_AT_SNAP, CROSSING_OCTILINEAR_AT_SNAP, load, sample, snap_only};
use llika_core::geometry::{direction, octilinear_deviation};
use llika_core::grid::{GridPoint, snap_to_grid};
use llika_core::layout::cost::c1_crossings;
use llika_core::{LayoutParams, Network, SchematicLayout, run_layout, total_cost};

/// Assertion 1 — the reproducible baseline. Assertion 2 has no meaning without
/// it, and it is a property worth having in its own right: it is how a caller
/// asks for projection and snapping alone.
#[test]
fn zero_iterations_reproduces_the_snap_only_layout() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &snap_only());

    let (snapped, _) = snap_to_grid(layout.projected(), layout.grid_spacing());

    assert_eq!(
        layout.positions(),
        snapped.as_slice(),
        "zero iterations must leave the snap untouched"
    );
}

/// Assertion 2 — the search improves the map.
///
/// Before-versus-after the search, and explicitly **not** "final iteration
/// versus first". The search reaches a fixed point inside the first sweep on
/// this fixture, so those two are bit-identical and that reading would fail on a
/// correct implementation.
#[test]
fn the_search_strictly_lowers_the_total_cost() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let params = LayoutParams::default();

    let before = total_of(&network, &run_layout(&network, &snap_only()), &params);
    let after = total_of(&network, &run_layout(&network, &params), &params);

    assert!(
        after < before,
        "the search left the cost at {after}, up from {before}"
    );
}

/// Assertion 3 — the map does not become *less* octilinear.
///
/// Non-decreasing and never "strictly greater": the snapped fixture is already
/// fully octilinear by accident of its own station spacing, so the fraction
/// starts at 1.0 and nothing can exceed it (OQ-8). The strict-improvement burden
/// lives on `t`, one assertion up.
#[test]
fn the_search_does_not_make_the_map_less_octilinear() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    let before = octilinear_fraction(&network, run_layout(&network, &snap_only()).positions());
    let after = octilinear_fraction(
        &network,
        run_layout(&network, &LayoutParams::default()).positions(),
    );

    assert!(
        after >= before,
        "octilinearity fell from {before} to {after}"
    );
}

/// Assertion 5 — the crossing penalty actually drives the search.
///
/// The 17-station fixture scores `c1 = 0` before **and** after, so without a
/// second fixture the phase's headline decision — that ordinary crossings are
/// left to the soft penalty rather than rejected outright — would ship with zero
/// coverage. `crossing.json` is two lines sharing no station, one dipping under
/// the other and back, crossing it twice.
///
/// **It is built to catch the wrong tool as well as the missing rule.** The
/// dipping line's middle station is degree 2, so its two edges share an
/// endpoint, and the rule must not count that shared endpoint. Under `segments_intersect` with the pinned pair
/// set, every station in this fixture becomes immovable and the search freezes
/// solid: `c1` stays at 2 and nothing moves at all. A crossing fixture whose
/// stations are all degree 1 has no endpoint-sharing pair and so cannot see
/// that, which is the luck of fixture shape this one is designed out of.
#[test]
fn a_deliberate_crossing_is_searched_away() {
    let input = load("crossing.json");
    let network = Network::from_input(&input).expect("the crossing fixture is valid");

    let before = run_layout(&network, &snap_only());
    let after = run_layout(&network, &LayoutParams::default());

    assert_eq!(
        c1_crossings(&network, before.positions()),
        CROSSING_C1_AT_SNAP,
        "the fixture must start with the crossings it was built to have"
    );
    assert!(
        c1_crossings(&network, after.positions()) < CROSSING_C1_AT_SNAP,
        "the search left the crossings in place"
    );
}

/// Assertion 3 again, where it has something to prove.
///
/// On the 17-station fixture the octilinearity assertion passes as `1.0 >= 1.0`
/// and could not have failed. Here three of the five corridors are octilinear at
/// the snap and the other two are 18° off the nearest diagonal, so a search that
/// ignored `c5` would show it.
#[test]
fn the_search_straightens_the_crossing_fixture_onto_the_grid() {
    let network = Network::from_input(&load("crossing.json")).expect("the fixture is valid");

    let before = octilinear_fraction(&network, run_layout(&network, &snap_only()).positions());
    let after = octilinear_fraction(
        &network,
        run_layout(&network, &LayoutParams::default()).positions(),
    );

    assert_eq!(before, CROSSING_OCTILINEAR_AT_SNAP);
    assert!(after > before, "octilinearity stayed at {before}");
}

/// Assertion 6's other half — that the loop is *reached* at the defaults.
///
/// Determinism across processes is delegated to
/// `llika-cli/tests/byte_stability.rs`, which runs the binary twice on those
/// same defaults; two in-process runs cannot see a per-process hasher seed. That
/// test only covers the search if the search runs, which is what this says.
#[test]
fn the_default_parameters_actually_move_stations() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    let before = run_layout(&network, &snap_only());
    let after = run_layout(&network, &LayoutParams::default());

    let moved = before
        .positions()
        .iter()
        .zip(after.positions())
        .filter(|(a, b)| a != b)
        .count();

    assert!(moved > 0, "the search moved nothing at the defaults");
}

/// `llk-001` Phase 7's gate, the fixture half: `--initial-radius` changes nothing
/// **here**.
///
/// This is the measurement the old `--help` text generalised from, and it still
/// holds — which is what makes that text explicable rather than merely wrong. The
/// half that does not generalise lives in
/// `llika-gtfs/tests/real_feed.rs:the_initial_radius_saturates_above_two_on_bart`,
/// where `r_0 = 1` draws a different map. Kept here because a shipped `--help`
/// string asserts both, and an unasserted sentence in one is how the other came
/// to overstate.
#[test]
fn the_initial_radius_changes_nothing_on_the_fixture() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    let at = |r| {
        run_layout(
            &network,
            &LayoutParams {
                initial_radius: r,
                ..LayoutParams::default()
            },
        )
        .positions()
        .to_vec()
    };

    let one = at(1);
    for r in [2, 3, 5, 8] {
        assert_eq!(
            at(r),
            one,
            "r_0 = {r} is not r_0 = 1's layout on the fixture"
        );
    }
}

fn total_of(network: &Network, layout: &SchematicLayout, params: &LayoutParams) -> f64 {
    total_cost(
        network,
        layout.positions(),
        layout.target_edge_cells(),
        params,
    )
}

/// The share of corridors within 5 degrees of a multiple of 45.
fn octilinear_fraction(network: &Network, positions: &[GridPoint]) -> f64 {
    let graph = network.graph();
    let edges: Vec<(usize, usize)> = graph
        .edge_indices()
        .filter_map(|edge| graph.edge_endpoints(edge))
        .map(|(a, b)| (a.index(), b.index()))
        .collect();

    let tolerance = 5.0_f64.to_radians();
    let within = edges
        .iter()
        .filter(|(a, b)| octilinear_deviation(direction(positions[*a], positions[*b])) <= tolerance)
        .count();

    within as f64 / edges.len() as f64
}
