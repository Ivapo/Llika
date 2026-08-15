//! The five criteria against the 17-station fixture.
//!
//! Not part of the Phase 2 gate, which is per-criterion unit tests over
//! hand-built graphs with hand-computed expected values. This is the one place
//! the criteria meet real data before Phase 3 wires them into the search, and
//! it exists to catch what a hand-built two-edge graph cannot: a panic on a
//! degree-1 terminus, a degree-3 junction or a shared trunk, and a `NaN`
//! leaking out of a division nothing else divides by.
//!
//! It asserts shape and finiteness, never a magnitude — the weights are
//! provisional (OQ-2), so any number pinned here would be pinning an artefact
//! of something still expected to change.
//!
//! **Scored on the snap alone.** What the search does to these numbers is its
//! own gate's business, in `hillclimb.rs`; here they are the input the search
//! starts from.

mod common;

use common::{sample, snap_only};
use llika_core::layout::cost::{
    c1_crossings, c2_edge_length, c3_angular_resolution, c4_straightness, c5_octilinearity,
};
use llika_core::{LayoutParams, Network, evaluate, run_layout};

#[test]
fn every_criterion_scores_the_snapped_fixture_without_panicking() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &snap_only());
    let positions = layout.positions();

    // The fixture spans the degrees that matter: a degree-1 terminus, degree-2
    // trunk stations, `market` at degree 3 — the one with a positive floor —
    // and `central` at degree 4.
    let degrees: Vec<usize> = (0..network.stations().len())
        .map(|s| network.degree(s))
        .collect();
    for expected in [1, 2, 3, 4] {
        assert!(
            degrees.contains(&expected),
            "the fixture has a degree-{expected} station"
        );
    }

    let cost = evaluate(&network, positions, layout.target_edge_cells());
    for (name, value) in [
        ("c1", cost.c1),
        ("c2", cost.c2),
        ("c3", cost.c3),
        ("c4", cost.c4),
        ("c5", cost.c5),
    ] {
        assert!(value.is_finite(), "{name} was {value}");
        assert!(value >= 0.0, "{name} was negative: {value}");
    }

    // `evaluate` is the five functions and nothing else.
    assert_eq!(cost.c1, c1_crossings(&network, positions));
    assert_eq!(
        cost.c2,
        c2_edge_length(&network, positions, layout.target_edge_cells())
    );
    assert_eq!(cost.c3, c3_angular_resolution(&network, positions));
    assert_eq!(cost.c4, c4_straightness(&network, positions));
    assert_eq!(cost.c5, c5_octilinearity(&network, positions));

    // The criteria can see something to improve, so `t` is not already at a
    // floor the search would have nothing to do about.
    let total = cost.total(&LayoutParams::default());
    assert!(total.is_finite() && total > 0.0, "t was {total}");
    assert!(cost.c3 > 0.0 && cost.c4 > 0.0 && cost.c2 > 0.0);

    // **`c1` and `c5` are both exactly zero here, and that is a fact about the
    // fixture rather than about the scorer** — recorded because Phase 3's gate
    // is keyed to it and cannot be met as written. See OQ-8.
    //
    // The fixture's 17 stations were authored roughly 2.3 km apart, and `g`
    // derives as their median edge, so every station snaps onto a 7x5 patch of
    // unit cells and all 17 corridors land on octilinear directions. The
    // fraction of edges within 5 degrees of a multiple of 45 is therefore
    // already 1.0 before any hill-climbing runs.
    assert_eq!(cost.c5, 0.0, "the snapped fixture is already octilinear");
    assert_eq!(cost.c1, 0.0, "the snapped fixture has no crossings");
}
