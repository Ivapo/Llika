//! Phase 5 gate, assertions 2 and 3: the bundle is real, and it collapses
//! where it should.
//!
//! Both read the **emitted document** rather than the renderer's internals,
//! because the observable this phase produces is the picture. Coordinates come
//! back through `Viewport::project` rather than a re-derived transform, on the
//! rule `render.rs` already follows.

mod common;

use common::sample;
use llika_core::render::Viewport;
use llika_core::{LayoutParams, Network, RenderParams, render_to_string, run_layout};

const RED: &str = "#E4002B";
const GREEN: &str = "#00843D";

/// `num()` writes three decimals, so a value the renderer computed exactly is
/// read back with up to 5e-4 of rounding error. Anything asserted about a
/// *distance* between two such values therefore gets a tolerance; anything
/// asserted about two values being the **same** point does not, since identical
/// text parses identically.
const ROUNDING: f64 = 1e-3;

/// Every vertex of the path drawn in the given colour, in the line's own
/// station-list order.
fn path_points(svg: &str, stroke: &str) -> Vec<(f64, f64)> {
    let document = roxmltree::Document::parse(svg).expect("the SVG is well-formed XML");
    let data = document
        .descendants()
        .filter(|node| node.has_tag_name("path"))
        .find(|node| node.attribute("stroke") == Some(stroke))
        .and_then(|node| node.attribute("d"))
        .unwrap_or_else(|| panic!("a path is drawn in {stroke}"))
        .to_owned();

    // "M x y L x y L x y …" — commands and coordinates alternate, so dropping
    // every token that does not parse as a number leaves the vertices.
    let numbers: Vec<f64> = data
        .split_whitespace()
        .filter_map(|token| token.parse().ok())
        .collect();
    assert_eq!(numbers.len() % 2, 0, "coordinates come in pairs");
    numbers.chunks(2).map(|pair| (pair[0], pair[1])).collect()
}

struct Drawn {
    red: Vec<(f64, f64)>,
    green: Vec<(f64, f64)>,
    centre: Box<dyn Fn(&str) -> (f64, f64)>,
    spacing: f64,
}

fn drawn() -> Drawn {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &LayoutParams::default());
    let params = RenderParams::default();
    let svg = render_to_string(&network, &layout, &params);
    let viewport = Viewport::new(&layout, &params);

    let positions: Vec<_> = layout.positions().to_vec();
    let indices: Vec<(String, usize)> = network
        .stations()
        .iter()
        .enumerate()
        .map(|(at, station)| (station.id.clone(), at))
        .collect();

    Drawn {
        red: path_points(&svg, RED),
        green: path_points(&svg, GREEN),
        // The bare cell centre — where the marker sits and where an unbundled
        // renderer would have put both strokes.
        centre: Box::new(move |id: &str| {
            let at = indices
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, at)| *at)
                .unwrap_or_else(|| panic!("{id} is a station of the fixture"));
            viewport.project(positions[at])
        }),
        // The default: `None` derives the spacing as `stroke_width`.
        spacing: params.stroke_width,
    }
}

/// Assertion 2 — **the only clause that discriminates.** Two of the three
/// assertions this phase originally carried are satisfied by the *Phase 1*
/// renderer: it already emitted one path per line, and unbundled paths already
/// coincided at every interchange.
///
/// `oldtown`→`eastbank` is the fixture's single genuinely parallel pair, which
/// is what OQ-5 lengthened the trunk to guarantee: both are degree 2 and both
/// carry exactly {Red, Green}, so both are interior to the same run.
#[test]
fn the_shared_trunk_draws_as_two_parallel_strokes_a_bundle_spacing_apart() {
    let drawn = drawn();

    // Red: westgate riverside [oldtown eastbank] central market quayside seacliff
    // Green: northgate hillcrest riverside [oldtown eastbank] central market fairview millpond
    let (red_old, red_east) = (drawn.red[2], drawn.red[3]);
    let (green_old, green_east) = (drawn.green[3], drawn.green[4]);

    // Parallel: the two segments' directions have zero cross product.
    let red_run = (red_east.0 - red_old.0, red_east.1 - red_old.1);
    let green_run = (green_east.0 - green_old.0, green_east.1 - green_old.1);
    let cross = red_run.0 * green_run.1 - red_run.1 * green_run.0;
    assert!(
        cross.abs() < ROUNDING,
        "the two strokes are not parallel: cross product {cross}"
    );

    for (red, green, id) in [
        (red_old, green_old, "oldtown"),
        (red_east, green_east, "eastbank"),
    ] {
        // Separated by exactly `bundle_spacing`, measured perpendicular to the
        // corridor — which here is the whole separation, since the two vertices
        // sit on a common perpendicular.
        let gap = (red.0 - green.0).hypot(red.1 - green.1);
        assert!(
            (gap - drawn.spacing).abs() < ROUNDING,
            "at {id} the strokes are {gap} apart, not {}",
            drawn.spacing
        );

        let along = red_run.0 * (red.0 - green.0) + red_run.1 * (red.1 - green.1);
        assert!(
            along.abs() < ROUNDING,
            "at {id} the separation is not perpendicular to the corridor: {along}"
        );

        // Symmetric about the corridor centreline: the two vertices straddle
        // the bare cell centre rather than sitting to one side of it.
        let midpoint = ((red.0 + green.0) / 2.0, (red.1 + green.1) / 2.0);
        let centre = (drawn.centre)(id);
        assert!(
            (midpoint.0 - centre.0).abs() < ROUNDING && (midpoint.1 - centre.1).abs() < ROUNDING,
            "at {id} the bundle straddles {midpoint:?} rather than the centreline {centre:?}"
        );
    }
}

/// Assertion 3 — both halves in one test. The first alone passes vacuously on a
/// renderer that does no bundling at all, which is precisely the state this
/// phase started from.
///
/// `central` is degree 4 and `market` degree 3, so both are collapse stations
/// and every offset there is zero. `oldtown` and `eastbank` are degree 2 with a
/// constant line set, so both carry a full offset.
#[test]
fn the_bundle_converges_at_the_interchanges_and_only_there() {
    let drawn = drawn();

    for (red, green, id) in [
        (drawn.red[4], drawn.green[5], "central"),
        (drawn.red[5], drawn.green[6], "market"),
    ] {
        assert_eq!(
            red, green,
            "at the interchange {id} both lines must pass through the identical point"
        );
        assert_eq!(red, (drawn.centre)(id), "and that point is the cell centre");
    }

    for (red, green, id) in [
        (drawn.red[2], drawn.green[3], "oldtown"),
        (drawn.red[3], drawn.green[4], "eastbank"),
    ] {
        assert_ne!(
            red, green,
            "along the trunk at {id} the two lines must stay separate"
        );
    }
}

/// Not gate-required, and it exists because the gap is real: `render.rs`'s
/// bounds check reads `layout.positions()` and so only ever sees *unoffset*
/// cell centres. Offsets are applied after `Viewport::project`, so a path
/// vertex can leave the document without any existing assertion noticing —
/// which is the failure §2.5 names when it calls the mitre clamp load-bearing.
#[test]
fn every_drawn_vertex_is_inside_the_document() {
    let network = Network::from_input(&sample()).expect("the fixture is valid");
    let layout = run_layout(&network, &LayoutParams::default());
    let params = RenderParams::default();
    let svg = render_to_string(&network, &layout, &params);
    let viewport = Viewport::new(&layout, &params);

    for line in network.lines() {
        for (x, y) in path_points(&svg, &line.color) {
            assert!(
                (0.0..=viewport.width()).contains(&x) && (0.0..=viewport.height()).contains(&y),
                "{} draws a vertex at ({x}, {y}), outside the viewBox",
                line.id
            );
        }
    }
}
