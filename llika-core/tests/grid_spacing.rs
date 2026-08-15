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

use common::{SAMPLE_GRID_SPACING_M, close, sample};
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
    let layout = run_layout(&network, &LayoutParams::default());

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
    let layout = run_layout(
        &network,
        &LayoutParams {
            grid_spacing: Some(900.0),
        },
    );

    assert_eq!(layout.grid_spacing(), 900.0);
}
