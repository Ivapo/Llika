//! Layout: projected coordinates onto grid cells, and what it costs.
//!
//! Three steps, in order. [`Projector`] puts stations on a metre plane;
//! [`snap_to_grid`] rounds them onto integer cells, one station per cell; and
//! [`hillclimb`] moves them from there against the five separable criteria in
//! [`cost`], stopping as soon as an iteration changes nothing.
//!
//! That last step is two passes, not one. Each iteration sweeps the stations
//! individually — [`candidate`] says where one may go and which moves would tear
//! the network — and then translates whole [`cluster`]s rigidly, which is the
//! only way out of a dead end no single station can see: a group hanging off the
//! map by one long edge cannot shorten that edge by moving any one of its
//! members.

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
/// them: these names are serde-visible and the CLI derives a flag from each field
/// by kebab-casing it, with no exceptions — `w_crossings` gives `--w-crossings`.
///
/// **`default` and `deny_unknown_fields`, together and deliberately.** A params
/// file naming only the weights someone cares about is the common case, so a
/// missing field falls back to [`Default`] rather than failing with `missing
/// field iterations`. A *misspelled* one is the opposite case: without the second
/// attribute `{"w_crosings": 9.0}` parses to all-defaults and silently discards
/// the one value the user was tuning, which is the worst failure for a knob whose
/// whole purpose is improving a result by eye. Missing is fine, misspelled is not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutParams {
    /// Grid spacing in **metres**.
    ///
    /// `None` — the default — derives it from the network as the median
    /// projected edge length, which makes a typical edge exactly one cell for
    /// any input at any scale. `Some(m)` must be finite and positive — a bound
    /// `llika-cli` enforces, since `run_layout` is infallible by design.
    pub grid_spacing: Option<f64>,

    /// The **most** iterations the search will make. Each is a sweep over every
    /// station followed by a sweep over every cluster.
    ///
    /// A **bound, not a target**: the search stops at the first iteration that
    /// moves nothing in either pass, because every iteration after that one is
    /// provably a no-op. Raising this cannot change the map, only the ceiling —
    /// on the sample fixture a run executes 2 of the 200 asked for.
    ///
    /// **Zero means snap and stop**, which is both the reproducible baseline
    /// every measurement of the search is taken against and the way to ask for
    /// projection and snapping alone.
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
/// The five values are **still provisional** (OQ-2), and they are the *second*
/// set: `5 / 1 / 1 / 2 / 5` shipped from Phase 3 to Phase 7, judged twice by eye
/// against the 17-station fixture and left standing both times. BART falsified
/// them — 176 of 324 settings in a coarse grid dominate them on all five
/// unweighted criteria at once, by a mechanism that is named rather than
/// guessed. `c4` and `c5` both price what an edge's *angle* does, and at
/// `w5:w4` = 5:2 `c4` outbids `c5`: the search buys a straight line through a
/// station by paying for an off-angle edge. Lowering `w4` and raising `w5` takes
/// BART from 48 of 50 corridors octilinear to 50 of 50, at `c5` exactly zero and
/// crossings still zero.
///
/// **Four-gonality now leads alone, and a crossing is still the most expensive
/// single thing on the map.** The two are not commensurable: `c1` counts
/// crossings, so one costs `5.0`, while `c5` sums a deviation of at most `π/8`
/// per edge, so the most expensive single off-angle edge costs
/// `10 × π/8 = 3.926991`.
///
/// **`w4 = 0.25` is a design call and no measurement here argues for it.** At
/// this weighting `w4 = 0` and `w4 = 0.25` draw byte-identical BART layouts. `c4`
/// keeps a low price rather than being switched off because it is one of the five
/// criteria with its own zero-set, `--w-straightness` is a flag someone can
/// raise, and a network whose lines bend where BART's do not is the case it
/// exists for — deleting a criterion because one city cannot see it is
/// overfitting to that city.
///
/// **Provisional, because this is one network and not a calibration.** These are
/// a *round* setting inside the region that dominates the old one, picked to be
/// explainable rather than the grid's argmax; a second real network should be
/// expected to move them again.
impl Default for LayoutParams {
    fn default() -> Self {
        Self {
            grid_spacing: None,
            iterations: 200,
            initial_radius: 3,
            cluster_moves: true,
            w_crossings: 5.0,
            w_edge_length: 1.0,
            w_angular_resolution: 0.5,
            w_straightness: 0.25,
            w_octilinearity: 10.0,
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
    executed_iterations: u32,
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

    /// How many iterations the search **actually executed**, which is at most
    /// [`LayoutParams::iterations`] and usually far below it — the search stops
    /// at the first one that moves nothing.
    ///
    /// Here for the same reason as the two scalars above: it is a function of
    /// the parameters *and* the network, so a caller cannot compute it from
    /// [`LayoutParams`]. Reporting the requested count instead would be a lie
    /// the moment the early exit fires, which on the sample fixture is
    /// immediately — 2 executed against 200 asked for.
    pub fn executed_iterations(&self) -> u32 {
        self.executed_iterations
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
    let executed_iterations = hillclimb::run(
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
        executed_iterations,
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
