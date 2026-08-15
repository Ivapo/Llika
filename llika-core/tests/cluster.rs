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
        close(total_of(&network, &layout, &params), PHASE3_TOTAL_COST, 1e-6),
        "the baseline cost moved"
    );
}

fn baseline() -> LayoutParams {
    LayoutParams::default()
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
