//! SVG rendering.
//!
//! Phase 1 is the bare renderer: one path per line, one marker per station, no
//! line-bundling and no labels. Bundling arrives in Phase 5 as
//! `render/corridor.rs`.

use serde::{Deserialize, Serialize};
use svg::Document;
use svg::node::element::{Circle, Path, Rectangle};

use crate::grid::GridPoint;
use crate::layout::SchematicLayout;
use crate::model::{Line, Network};

/// The renderer's tunable surface. Phase 6 completes it and gives every field a
/// flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderParams {
    /// SVG user units per grid cell.
    pub units_per_cell: f64,
    /// Blank cells around the network on every side.
    ///
    /// Must default **above zero**: a one-station network has `i_max == i_min`,
    /// so the document envelope reduces to `2 * margin_cells * units_per_cell`
    /// and a zero margin would yield a zero-extent document for an input the
    /// gate requires to render.
    pub margin_cells: f64,
    /// Stroke width of a line, in user units.
    pub stroke_width: f64,
}

impl Default for RenderParams {
    fn default() -> Self {
        Self {
            units_per_cell: 40.0,
            margin_cells: 2.0,
            stroke_width: 6.0,
        }
    }
}

/// The grid-cell to SVG-user-space transform, and the document envelope.
///
/// This owns the **y-flip**: latitude increases north and SVG `y` increases
/// down, so a renderer that omits the flip draws the map upside down and still
/// satisfies every count-based assertion. Tests read positions back through
/// this type rather than re-deriving the transform, so what they check is what
/// the document contains.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    i_min: i64,
    j_max: i64,
    units_per_cell: f64,
    margin_cells: f64,
    width: f64,
    height: f64,
}

impl Viewport {
    /// The envelope of a layout. An empty layout takes an origin of `(0, 0)`,
    /// leaving a document that is margin on every side.
    pub fn new(layout: &SchematicLayout, params: &RenderParams) -> Self {
        let (i_min, i_max, j_min, j_max) = bounds(layout.positions());
        Self {
            i_min,
            j_max,
            units_per_cell: params.units_per_cell,
            margin_cells: params.margin_cells,
            width: ((i_max - i_min) as f64 + 2.0 * params.margin_cells) * params.units_per_cell,
            height: ((j_max - j_min) as f64 + 2.0 * params.margin_cells) * params.units_per_cell,
        }
    }

    /// A grid cell in SVG user space, `y` pointing down.
    pub fn project(&self, cell: GridPoint) -> (f64, f64) {
        (
            ((cell.i - self.i_min) as f64 + self.margin_cells) * self.units_per_cell,
            ((self.j_max - cell.j) as f64 + self.margin_cells) * self.units_per_cell,
        )
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }
}

fn bounds(positions: &[GridPoint]) -> (i64, i64, i64, i64) {
    let mut iter = positions.iter();
    let Some(first) = iter.next() else {
        return (0, 0, 0, 0);
    };
    iter.fold(
        (first.i, first.i, first.j, first.j),
        |(i_min, i_max, j_min, j_max), cell| {
            (
                i_min.min(cell.i),
                i_max.max(cell.i),
                j_min.min(cell.j),
                j_max.max(cell.j),
            )
        },
    )
}

/// Render a laid-out network as one SVG document.
///
/// Deterministic: lines and stations are walked in input order, and the `svg`
/// crate sorts attributes by name, so the same input and parameters give the
/// same bytes in any process.
pub fn render_to_string(
    network: &Network,
    layout: &SchematicLayout,
    params: &RenderParams,
) -> String {
    let viewport = Viewport::new(layout, params);
    let (width, height) = (viewport.width(), viewport.height());

    let mut document = Document::new()
        .set("width", num(width))
        .set("height", num(height))
        .set("viewBox", format!("0 0 {} {}", num(width), num(height)))
        .add(
            // Without a painted background the map reads as strokes on
            // whatever the viewer's page happens to be.
            Rectangle::new()
                .set("x", "0")
                .set("y", "0")
                .set("width", num(width))
                .set("height", num(height))
                .set("fill", "#ffffff"),
        );

    for line in network.lines() {
        document = document.add(
            Path::new()
                .set("d", line_path_data(network, layout, &viewport, line))
                .set("fill", "none")
                .set("stroke", line.color.as_str())
                .set("stroke-width", num(params.stroke_width))
                .set("stroke-linejoin", "round")
                .set("stroke-linecap", "round"),
        );
    }

    for cell in layout.positions() {
        let (x, y) = viewport.project(*cell);
        document = document.add(
            Circle::new()
                .set("cx", num(x))
                .set("cy", num(y))
                .set("r", num(params.stroke_width * 0.9))
                .set("fill", "#ffffff")
                .set("stroke", "#1a1a1a")
                .set("stroke-width", num(params.stroke_width * 0.3)),
        );
    }

    document.to_string()
}

/// One path across a line's whole station list, so corner rounding comes free
/// from `stroke-linejoin`.
fn line_path_data(
    network: &Network,
    layout: &SchematicLayout,
    viewport: &Viewport,
    line: &Line,
) -> String {
    let mut data = String::new();
    for (nth, station_id) in line.stations.iter().enumerate() {
        let station = network
            .station_index(station_id)
            .expect("every line's stations are resolved by Network::from_input");
        let (x, y) = viewport.project(layout.positions()[station]);
        let command = if nth == 0 { 'M' } else { 'L' };
        if nth > 0 {
            data.push(' ');
        }
        data.push_str(&format!("{command} {} {}", num(x), num(y)));
    }
    data
}

/// Fixed-precision so the document is stable and readable rather than carrying
/// float tails.
fn num(value: f64) -> String {
    format!("{value:.3}")
}
