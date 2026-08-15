//! Layout: projected coordinates onto grid cells.
//!
//! Phase 1 is snap-only. The hill-climbing loop is **absent rather than
//! stubbed** — an empty `hillclimb` module would be a stub, and the point of
//! this slice is that the picture exists before any layout intelligence does.

use serde::{Deserialize, Serialize};

use crate::geometry::Point2;
use crate::grid::{GridPoint, derive_grid_spacing, snap_to_grid};
use crate::model::Network;
use crate::projection::Projector;

/// The layout's tunable surface. Phase 2 adds the five cost weights `w1`-`w5`,
/// Phase 3 the iteration count.
/// `Default` is derived: `grid_spacing: None`, which is the derived-from-the-
/// network spacing described below.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LayoutParams {
    /// Grid spacing in **metres**.
    ///
    /// `None` — the default — derives it from the network as the median
    /// projected edge length, which makes a typical edge exactly one cell for
    /// any input at any scale. `Some(m)` must be finite and positive; the flag
    /// that will supply it, and its validation, belong to Phase 6.
    pub grid_spacing: Option<f64>,
}

/// What the layout step produced.
///
/// Both vectors are indexed by station index, which is input-file order.
/// `grid_spacing` is read back from **here** and not from [`LayoutParams`],
/// because the derived default is a function of the parameters *and* the
/// network.
#[derive(Debug, Clone, PartialEq)]
pub struct SchematicLayout {
    positions: Vec<GridPoint>,
    projected: Vec<Point2>,
    grid_spacing: f64,
}

impl SchematicLayout {
    /// Grid cells, one per station, in input order.
    pub fn positions(&self) -> &[GridPoint] {
        &self.positions
    }

    /// The pre-snap plane, in metres, one per station, in input order.
    pub fn projected(&self) -> &[Point2] {
        &self.projected
    }

    /// The `g` this layout actually used, in metres.
    pub fn grid_spacing(&self) -> f64 {
        self.grid_spacing
    }
}

/// Project, derive `g`, and snap.
///
/// Infallible: every degenerate input the schema admits — no stations, no
/// edges, coincident stations — has a defined answer here rather than an error.
pub fn run_layout(network: &Network, params: &LayoutParams) -> SchematicLayout {
    let projector = Projector::from_stations(network.stations());
    let projected = projector.project_all(network.stations());

    let grid_spacing = match params.grid_spacing {
        Some(metres) => metres,
        None => derive_grid_spacing(&edge_lengths(network, &projected)),
    };

    let positions = snap_to_grid(&projected, grid_spacing);

    SchematicLayout {
        positions,
        projected,
        grid_spacing,
    }
}

/// The projected length of every corridor. Order is irrelevant — the only
/// consumer sorts.
fn edge_lengths(network: &Network, projected: &[Point2]) -> Vec<f64> {
    network
        .graph()
        .edge_indices()
        .filter_map(|edge| network.graph().edge_endpoints(edge))
        .map(|(a, b)| projected[a.index()].distance(projected[b.index()]))
        .collect()
}
