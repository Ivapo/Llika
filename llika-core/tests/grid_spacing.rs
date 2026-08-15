//! Phase 1 gate, assertion 4: the derived `g`.
//!
//! Without this, the one piece of new critical-path machinery ships unverified.
//! A collision still occurs under mean-instead-of-median, the bounding-box
//! check is pre-grid, the flip and count checks are scale-invariant, and
//! byte-stability asserts stability rather than correctness — so `(a + b) / 2`,
//! the reflex implementation, passes everything else, and Phase 2's
//! byte-identity gate would then freeze it.
//!
//! The other half of this assertion — the lower-middle rule on an even count —
//! is a unit test on `grid::median_lower`, where the input can be hand-built.

mod common;

use common::{SAMPLE_GRID_SPACING_M, close, sample, snap_only};
use llika_core::{LayoutParams, Network, run_layout};

/// The literal is the 9th of the fixture's 17 sorted projected edge lengths —
/// the `riverside` → `hillcrest` corridor — computed by an independent
/// implementation of §2.2, not by the code under test.
///
/// Relative rather than exact: `g` comes out of `cos` and `sqrt`, so exact
/// `f64` equality against a decimal computed elsewhere will not hold.
#[test]
fn the_default_grid_spacing_is_the_median_projected_edge_length() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &snap_only());

    let g = layout.grid_spacing();
    assert!(
        close(g, SAMPLE_GRID_SPACING_M, 1e-6),
        "derived g was {g}, expected {SAMPLE_GRID_SPACING_M}"
    );

    // A median is a member of its set, which an average is not. The fixture has
    // an odd edge count, so the lower-middle rule is invisible here — that is
    // what `grid::median_lower`'s even-count unit test covers — but "g is one
    // of the edge lengths" still fails for any implementation that averages.
    let lengths = projected_edge_lengths(&network, &layout);
    assert!(
        lengths.iter().any(|len| close(*len, g, 1e-12)),
        "g is one of the fixture's own edge lengths"
    );
    assert_eq!(lengths.len(), common::SAMPLE_EDGES);
}

fn projected_edge_lengths(network: &Network, layout: &llika_core::SchematicLayout) -> Vec<f64> {
    let points = layout.projected();
    network
        .graph()
        .edge_indices()
        .filter_map(|edge| network.graph().edge_endpoints(edge))
        .map(|(a, b)| points[a.index()].distance(points[b.index()]))
        .collect()
}

/// An explicit spacing is used as given, and is not the derived one.
#[test]
fn an_explicit_grid_spacing_overrides_the_derivation() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &explicit_spacing(900.0));

    assert_eq!(layout.grid_spacing(), 900.0);
}

/// Phase 2 gate, assertion 5's other half — OQ-6's target length `L`.
///
/// Under the default `g` the target is **exactly** `1.0`, because the numerator
/// is the same median `g` itself comes from. That is §2.2's argument made
/// checkable: a typical edge is one cell, which is the length `c2` is trying to
/// reach, so the layout starts near the criterion's optimum at any scale.
#[test]
fn the_target_edge_length_is_exactly_one_cell_under_the_derived_spacing() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &snap_only());

    assert_eq!(layout.target_edge_cells(), 1.0);
}

/// And it self-scales when a user supplies `g` — the case OQ-6 says the
/// resolution has to cover. At 300 m on a network whose typical edge is
/// `SAMPLE_GRID_SPACING_M`, `c2` asks for edges of about 7.6 cells rather than
/// contracting the whole map to 300 m.
#[test]
fn the_target_edge_length_self_scales_against_an_explicit_spacing() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");

    let fine = run_layout(&network, &explicit_spacing(300.0));
    let expected = SAMPLE_GRID_SPACING_M / 300.0;
    assert!(
        close(fine.target_edge_cells(), expected, 1e-6),
        "target was {} cells, expected {expected}",
        fine.target_edge_cells()
    );

    // Clamped at one cell: §2.2's occupancy invariant puts every post-snap edge
    // at least one cell apart, so a target below one is unreachable.
    let coarse = run_layout(&network, &explicit_spacing(4.0 * SAMPLE_GRID_SPACING_M));
    assert_eq!(coarse.target_edge_cells(), 1.0);
}

/// Snap-only, like every other layout in this file: `g` and the target length
/// are both fixed before the first sweep, so running the search would only cost
/// time.
fn explicit_spacing(metres: f64) -> LayoutParams {
    LayoutParams {
        grid_spacing: Some(metres),
        ..snap_only()
    }
}
