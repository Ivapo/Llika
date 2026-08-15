//! Layout: projected coordinates onto grid cells, and what it costs.
//!
//! Three steps, in order. [`Projector`] puts stations on a metre plane;
//! [`snap_to_grid`] rounds them onto integer cells, one station per cell; and
//! [`hillclimb`] moves them from there, one station at a time, against the five
//! separable criteria in [`cost`]. [`candidate`] says where a station may go and
//! which of those moves would tear the network.

use serde::{Deserialize, Serialize};

use crate::geometry::Point2;
use crate::grid::{GridPoint, derive_grid_spacing, snap_to_grid};
use crate::model::Network;
use crate::projection::Projector;

pub mod cost;

mod candidate;
mod cluster;
mod hillclimb;

/// The layout's tunable surface.
///
/// The five weights are the reason the cost function is five separable terms
/// rather than one fused score: they become the sliders of the roadmap's UI, so
/// each criterion has to be independently computable and independently
/// meaningful.
///
/// Named for what they weigh rather than `w1`-`w5`, which is how the spec words
/// them: these names are serde-visible and Phase 6 derives a flag from each
/// field, and §1's own end-state invocation types `--w-crossing`, not `--w1`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutParams {
    /// Grid spacing in **metres**.
    ///
    /// `None` — the default — derives it from the network as the median
    /// projected edge length, which makes a typical edge exactly one cell for
    /// any input at any scale. `Some(m)` must be finite and positive; the flag
    /// that will supply it, and its validation, belong to Phase 6.
    pub grid_spacing: Option<f64>,

    /// How many sweeps the search makes over the stations.
    ///
    /// A **bound, not a target**: the search stops improving as soon as no
    /// station has an improving move left, and nothing detects that or exits
    /// early. **Zero means snap and stop**, which is both the reproducible
    /// baseline every measurement of the search is taken against and the way to
    /// ask for projection and snapping alone.
    pub iterations: u32,

    /// The movement radius at the first sweep, in Chebyshev rings — **not** a
    /// Euclidean distance. It shrinks linearly to one cell over the run, so the
    /// search ranges widely early and settles late.
    pub initial_radius: u32,

    /// Whether the search also translates whole bridge-side clusters, after each
    /// per-station sweep.
    ///
    /// **On by default**, because it removes a class of local minimum the
    /// per-station sweep cannot see out of: a tight group hanging off the map by
    /// one long edge cannot shorten that edge by moving any single member.
    /// Switching it off reproduces the single-station search exactly, which is
    /// the baseline every measurement of this step is taken against.
    pub cluster_moves: bool,

    /// `w1` — edge crossings.
    pub w_crossings: f64,
    /// `w2` — edge length against the target.
    pub w_edge_length: f64,
    /// `w3` — angular resolution at a station.
    pub w_angular_resolution: f64,
    /// `w4` — a line bending where nothing forces it to.
    pub w_straightness: f64,
    /// `w5` — distance from the nearest multiple of 45 degrees.
    pub w_octilinearity: f64,
}

/// **Explicit, never derived.** `#[derive(Default)]` gives every weight `0.0`,
/// which makes `t ≡ 0` — a scorer that scores nothing, which no per-criterion
/// test would catch and which would then sail through Phase 3's cost-decrease
/// gate.
///
/// The five values are **provisional** (OQ-2). The source paper gives none, the
/// criteria have different natural scales, and Phase 3's visual gate is the
/// first place they can be judged. Crossings and four-gonality lead: a crossing
/// is the most visually damaging thing on the map, and `c5` carries the whole
/// octilinear look while being small in magnitude — at most `π/8` per edge.
impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            grid_spacing: None,
            iterations: 200,
            initial_radius: 3,
            cluster_moves: true,
            w_crossings: 5.0,
            w_edge_length: 1.0,
            w_angular_resolution: 1.0,
            w_straightness: 2.0,
            w_octilinearity: 5.0,
        }
    }
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
    target_edge_cells: f64,
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

    /// The length, **in cells**, that `c2` wants every edge to be — OQ-6's
    /// answer. Read it back from here for the same reason as `grid_spacing`:
    /// it is a function of the parameters *and* the network.
    pub fn target_edge_cells(&self) -> f64 {
        self.target_edge_cells
    }
}

/// Project, derive `g`, snap, and hill-climb.
///
/// Infallible: every degenerate input the schema admits — no stations, no
/// edges, coincident stations — has a defined answer here rather than an error.
/// The search inherits that: a network with nothing to improve simply finds no
/// improving move.
pub fn run_layout(network: &Network, params: &LayoutParams) -> SchematicLayout {
    let projector = Projector::from_stations(network.stations());
    let projected = projector.project_all(network.stations());

    let lengths = edge_lengths(network, &projected);
    let typical = derive_grid_spacing(&lengths);

    let grid_spacing = match params.grid_spacing {
        Some(metres) => metres,
        None => typical,
    };

    let (mut positions, mut occupancy) = snap_to_grid(&projected, grid_spacing);
    let target_edge_cells = target_edge_cells(typical, grid_spacing);

    // The occupancy comes from the snap rather than being rebuilt: it is the
    // one structure that answers "which cells are taken", and the search moves
    // stations between them.
    hillclimb::run(
        network,
        &mut positions,
        &mut occupancy,
        target_edge_cells,
        params,
    );

    SchematicLayout {
        positions,
        projected,
        grid_spacing,
        target_edge_cells,
    }
}

/// OQ-6's answer: `c2`'s target edge length, in cells.
///
/// The network's own typical edge — the same median `g` derives from — measured
/// in whatever cells are actually in use. Under the default `g` the numerator
/// *is* `g`, so this is exactly `1.0` and §2.2's argument holds verbatim: a
/// typical edge is one cell, which is the target `c2` is trying to reach, so
/// the layout starts near the criterion's optimum for any network at any scale.
///
/// The case OQ-6 says the resolution has to cover is a user-supplied
/// `--grid-spacing`, and this is what covers it. A fixed target of one cell
/// would fuse two unrelated jobs into one knob — halving `g` to get finer
/// movement resolution would also halve the map's target edge length, and
/// contract the whole drawing. Here `g` sets the resolution and the network
/// sets the target.
///
/// **Clamped at one cell** because §2.2's occupancy invariant puts every
/// post-snap edge at least one cell apart, so a target below one is
/// unreachable. Left unclamped it induces the same ranking — shorter is better
/// — but with a magnitude that grows without bound as `g` does, which would
/// silently re-weight `c2` against the other four.
fn target_edge_cells(typical_edge_metres: f64, grid_spacing: f64) -> f64 {
    (typical_edge_metres / grid_spacing).max(1.0)
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
