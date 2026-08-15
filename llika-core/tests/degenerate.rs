//! Phase 1 gate, assertion 5: the two degenerate inputs render rather than
//! panic.
//!
//! Both are reachable because §2.1 makes both legal — a station referenced by
//! no line is not an error, and neither are two stations at the same place — so
//! the median that `g` comes from can be taken over an empty set.

mod common;

use common::load;
use llika_core::grid::FALLBACK_GRID_SPACING_M;
use llika_core::{LayoutParams, Network, RenderParams, build_schematic_svg, run_layout};

#[test]
fn one_station_and_no_lines_renders_at_the_fallback_spacing() {
    let input = load("single_station.json");
    let network = Network::from_input(&input).expect("a lone station is legal");
    assert_eq!(network.edge_count(), 0);

    let layout = run_layout(&network, &LayoutParams::default());
    assert_eq!(layout.grid_spacing(), FALLBACK_GRID_SPACING_M);

    let svg = build_schematic_svg(&input, &LayoutParams::default(), &RenderParams::default())
        .expect("renders");
    assert!(svg.starts_with("<svg"));
    // i_max == i_min here, so the envelope is margin alone. A zero default
    // margin would make this document zero-extent.
    assert!(svg.contains(r#"viewBox="0 0 160.000 160.000""#), "{svg}");
    assert_eq!(svg.matches("<circle").count(), 1);
    assert_eq!(svg.matches("<path").count(), 0);
}

#[test]
fn two_stations_at_identical_coordinates_render_at_the_fallback_spacing() {
    let input = load("coincident_pair.json");
    let network = Network::from_input(&input).expect("coincident stations are legal");
    assert_eq!(network.edge_count(), 1);

    let layout = run_layout(&network, &LayoutParams::default());
    // The one edge is zero-length, so the non-zero set is empty and `g` falls
    // back rather than being a median of nothing — or a zero that would make
    // every cell a NaN.
    assert_eq!(layout.grid_spacing(), FALLBACK_GRID_SPACING_M);

    // And the tie-break still separates them, which is what keeps every
    // post-snap edge at least one cell long.
    assert_ne!(layout.positions()[0], layout.positions()[1]);

    let svg = build_schematic_svg(&input, &LayoutParams::default(), &RenderParams::default())
        .expect("renders");
    assert!(svg.starts_with("<svg"));
    assert_eq!(svg.matches("<circle").count(), 2);
    assert_eq!(svg.matches("<path").count(), 1);
}
