//! Phase 1 gate, assertions 8 and 9: the document, and which way up it is.

mod common;

use common::{SAMPLE_LINES, SAMPLE_STATIONS, sample};
use llika_core::render::Viewport;
use llika_core::{LayoutParams, Network, RenderParams, render_to_string, run_layout};

/// Assertion 8 — the y-flip. Latitude increases north and SVG `y` increases
/// down, so a renderer that omits the flip draws the map upside down and passes
/// every count-based assertion in this file.
#[test]
fn the_more_northerly_station_has_the_smaller_svg_y() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &LayoutParams::default());
    let params = RenderParams::default();
    let viewport = Viewport::new(&layout, &params);

    let northgate = network
        .station_index("northgate")
        .expect("northgate exists");
    let seacliff = network.station_index("seacliff").expect("seacliff exists");

    // The premise: these two really are at different latitudes, north first.
    assert!(network.stations()[northgate].lat > network.stations()[seacliff].lat);

    let (_, north_y) = viewport.project(layout.positions()[northgate]);
    let (_, south_y) = viewport.project(layout.positions()[seacliff]);
    assert!(
        north_y < south_y,
        "northgate y {north_y} must be above seacliff y {south_y}"
    );

    // And every drawn point is inside the document.
    for cell in layout.positions() {
        let (x, y) = viewport.project(*cell);
        assert!((0.0..=viewport.width()).contains(&x));
        assert!((0.0..=viewport.height()).contains(&y));
    }
}

/// Assertion 9 — well-formed XML, one path per line, one marker per station.
#[test]
fn the_document_is_well_formed_with_one_path_per_line_and_a_marker_per_station() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &LayoutParams::default());
    let svg = render_to_string(&network, &layout, &RenderParams::default());

    let document = roxmltree::Document::parse(&svg).expect("the SVG is well-formed XML");
    let count = |tag: &str| {
        document
            .descendants()
            .filter(|node| node.has_tag_name(tag))
            .count()
    };

    assert_eq!(count("path"), SAMPLE_LINES);
    assert_eq!(count("circle"), SAMPLE_STATIONS);

    // Each path carries its line's colour, and is a stroke rather than a fill.
    let strokes: Vec<&str> = document
        .descendants()
        .filter(|node| node.has_tag_name("path"))
        .filter_map(|node| node.attribute("stroke"))
        .collect();
    for line in network.lines() {
        assert!(strokes.contains(&line.color.as_str()), "{} drawn", line.id);
    }
}

/// The same input and parameters give the same bytes. This is the in-process
/// half; the cross-process half — which is the one that can see a per-process
/// hasher seed — is `llika-cli/tests/byte_stability.rs`.
#[test]
fn rendering_twice_in_one_process_gives_the_same_bytes() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let params = RenderParams::default();

    let first = render_to_string(
        &network,
        &run_layout(&network, &LayoutParams::default()),
        &params,
    );
    let second = render_to_string(
        &network,
        &run_layout(&network, &LayoutParams::default()),
        &params,
    );

    assert_eq!(first, second);
}
