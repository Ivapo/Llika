//! Phase 4 gate: rigid moves of bridge-side clusters.
//!
//! **Everything here is measured against Phase 3's shipped layout**, which
//! `common::PHASE3_POSITIONS` and `common::PHASE3_TOTAL_COST` pin. That is the
//! role `iterations = 0` played for Phase 3: assertion 2 has no meaning without
//! a baseline that cannot drift under it.
//!
//! The group-form rejections are unit-tested in `layout/cluster.rs`, where the
//! predicate they turn on is reachable.

mod common;

use common::{PHASE3_POSITIONS, PHASE3_TOTAL_COST, close, sample};
use llika_core::grid::GridPoint;
use llika_core::{LayoutParams, Network, SchematicLayout, run_layout, total_cost};

/// Assertion 1 — the baseline, bit-for-bit.
#[test]
fn the_baseline_reproduces_phase_threes_shipped_layout() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let params = baseline();
    let layout = run_layout(&network, &params);

    assert_eq!(
        layout.positions(),
        cells(&PHASE3_POSITIONS).as_slice(),
        "the baseline layout moved"
    );
    assert!(
        close(
            total_of(&network, &layout, &params),
            PHASE3_TOTAL_COST,
            1e-6
        ),
        "the baseline cost moved"
    );
}

/// Assertion 2 — the cluster pass improves the map.
///
/// Keyed to the committed 17-station fixture rather than to a purpose-built one,
/// because OQ-7's resolution measured a real improving cluster move on it: a
/// rigid move of `parkview`/`lakeside`, the side of the `university`–`parkview`
/// bridge, takes `t` from the baseline to 16.222682 on its own.
///
/// **The direction is not free.** Hill-climbing is path-dependent, so an accepted
/// cluster move can steer the run to a worse fixed point than the single-station
/// search reaches. No literal is keyed to the final value for the same reason.
#[test]
fn cluster_moves_strictly_lower_the_total_cost() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let params = LayoutParams::default();

    let before = total_of(&network, &run_layout(&network, &baseline()), &params);
    let after = total_of(&network, &run_layout(&network, &params), &params);

    assert!(
        after < before,
        "cluster moves left the cost at {after}, up from {before}"
    );
}

/// Assertion 5's other half — that the new path is *reached* at the defaults.
///
/// Determinism across processes is delegated to
/// `llika-cli/tests/byte_stability.rs`, which runs the binary twice on those same
/// defaults; two in-process runs cannot see a per-process hasher seed. That test
/// only covers the cluster pass if the cluster pass fires, which is what this
/// says — confirmed rather than assumed.
#[test]
fn the_default_parameters_actually_move_a_cluster() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    let before = run_layout(&network, &baseline());
    let after = run_layout(&network, &LayoutParams::default());

    assert_ne!(
        before.positions(),
        after.positions(),
        "the defaults produced the single-station layout"
    );
}

/// The single-station search, with the cluster pass switched off. `run_layout` is
/// the only public entry, so this flag is the seam every comparison above needs.
fn baseline() -> LayoutParams {
    LayoutParams {
        cluster_moves: false,
        ..LayoutParams::default()
    }
}

fn cells(pairs: &[(i64, i64)]) -> Vec<GridPoint> {
    pairs.iter().map(|(i, j)| GridPoint::new(*i, *j)).collect()
}

fn total_of(network: &Network, layout: &SchematicLayout, params: &LayoutParams) -> f64 {
    total_cost(
        network,
        layout.positions(),
        layout.target_edge_cells(),
        params,
    )
}
