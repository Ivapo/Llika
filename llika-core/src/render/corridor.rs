//! Line bundling: two lines sharing a corridor draw as parallel offset
//! strokes that converge to a single point at a real interchange.
//!
//! This is the spec's §2.5, and it is original — the source paper covers
//! layout and not rendering.
//!
//! Three ideas carry the whole file:
//!
//! 1. **A collapse station is one where the line set changes, or where degree
//!    is not 2.** At one, every offset is zero. That is stated on the
//!    *station*, not on a position within a run, and keeping it that way is
//!    what makes the two degenerate run shapes below fall out with no special
//!    case.
//! 2. **A run is a maximal path of consecutive corridors carrying the same
//!    line set, broken at every collapse station.** A line keeps one fixed
//!    perpendicular offset across a run, so it never visually swaps sides
//!    mid-run.
//! 3. **The side a positive offset lies on is a property of the run**, not of
//!    any line's traversal. Two lines walking a shared corridor in opposite
//!    list order would otherwise compute opposite normals and land on the same
//!    side.
//!
//! **Nothing here may iterate a `HashMap`.** Grouping corridors into runs and
//! keying offsets by line id is exactly where one is the reflex structure, and
//! §2.2's input-order rule is what byte-stability across processes rests on.
//! Every structure below is a `Vec` indexed by station or line index, or the
//! `BTreeSet` the graph edge already carries.

use crate::layout::SchematicLayout;
use crate::model::Network;

use super::{RenderParams, Viewport};

/// The largest the mitre may scale an offset, in multiples of the offset
/// itself.
///
/// **Load-bearing rather than defensive.** `1/cos(θ/2)` *increases* with the
/// turn and diverges as a corridor folds back on itself, and nothing makes an
/// octilinear layout an invariant — §1.1 says in terms that this design leaves
/// some edges off-angle, and the search's overlap rejection is a move filter
/// that `iterations = 0` never runs. On the integer grid, neighbours at
/// `(5,0)` and `(5,1)` already give a factor of 10.15, and at `(20,0)` and
/// `(20,1)`, 40.04. `Viewport::new` sizes the document from *station* extents,
/// so an unclamped mitre puts a path vertex outside the `viewBox` unseen.
///
/// The number is **borrowed** from SVG's `stroke-miterlimit` default rather
/// than derived from it — SVG bevels past its limit where this clamps, and its
/// ratio is against `stroke-width` where this one is against `bundle_spacing`.
/// What justifies 4 here on its own terms: at `4 · s` the outer stroke of a
/// two-line bundle sits `2 · s` off the centreline, which is bounded and still
/// reads as a corner, and beyond it the join reads as a spike.
///
/// It is a named constant and deliberately **not** a [`RenderParams`] field:
/// the tunable surface is what a slider binds to, and a degeneracy guard is not
/// a thing anyone tunes. The same call `FALLBACK_GRID_SPACING_M` is.
const MITRE_SCALE_CLAMP: f64 = 4.0;

/// Every line's offset from the bare cell centre, in SVG user units.
///
/// Indexed `[line index][position in that line's station list]` — a
/// **(line, position)** pair and not a (line, station) one. §2.1 rejects only
/// *consecutive* repeats, so a line may legally visit one station twice; this
/// shape is right whatever that costs downstream, and it is also what
/// `line_path_data` walks.
pub(super) struct Bundling {
    offsets: Vec<Vec<(f64, f64)>>,
}

impl Bundling {
    pub(super) fn new(
        network: &Network,
        layout: &SchematicLayout,
        viewport: &Viewport,
        params: &RenderParams,
    ) -> Self {
        // `None` derives the spacing as `stroke_width`, which puts two strokes
        // exactly adjacent — touching without a gap or an overlap. A fixed
        // default would be wrong the moment `stroke_width` changes, and a
        // bundle spaced at half its stroke width draws as one thick smear.
        let spacing = params.bundle_spacing.unwrap_or(params.stroke_width);

        let corridors = corridors(network);
        let adjacency = adjacency(network, &corridors);
        let frames = station_frames(network, layout, viewport, &corridors, &adjacency);

        let offsets = network
            .lines()
            .iter()
            .enumerate()
            .map(|(line, spec)| {
                spec.stations
                    .iter()
                    .map(|id| {
                        let station = network
                            .station_index(id)
                            .expect("every line's stations are resolved by Network::from_input");
                        match &frames[station] {
                            None => (0.0, 0.0),
                            Some(frame) => {
                                let magnitude = frame.signed_offset(network, line, spacing);
                                (
                                    frame.direction.0 * frame.scale * magnitude,
                                    frame.direction.1 * frame.scale * magnitude,
                                )
                            }
                        }
                    })
                    .collect()
            })
            .collect();

        Self { offsets }
    }

    pub(super) fn offset(&self, line: usize, nth: usize) -> (f64, f64) {
        self.offsets[line][nth]
    }
}

/// Where a station's offsets point, and how the lines through it are ordered.
///
/// Held per station rather than per (run, position). A station carrying a
/// frame is a non-collapse station, so its degree is 2 and it belongs to
/// **exactly one** run — which is what makes one entry per station well
/// defined rather than accidentally consistent.
struct Frame {
    /// The mitre's unit direction, in SVG user space.
    direction: (f64, f64),
    /// `1/cos(θ/2)`, clamped. See [`MITRE_SCALE_CLAMP`].
    scale: f64,
    /// The run's lines, sorted by line **`id`** — the string, which is why
    /// §2.1 rejects a duplicate one. Deliberately *not* input order, which is
    /// the rule everywhere else: the structure an implementer has in hand,
    /// `LineSet`, is a set of line *indices* and yields the other order.
    lines_by_id: Vec<usize>,
}

impl Frame {
    /// The signed distance from the corridor centreline, so the bundle
    /// straddles it symmetrically and `n = 1` falls out as zero with no
    /// special case.
    fn signed_offset(&self, network: &Network, line: usize, spacing: f64) -> f64 {
        let n = self.lines_by_id.len();
        let k = self
            .lines_by_id
            .iter()
            .position(|&candidate| candidate == line)
            .unwrap_or_else(|| {
                panic!(
                    "line {} passes through a station of a run it is not in",
                    network.lines()[line].id
                )
            });
        (k as f64 - (n as f64 - 1.0) / 2.0) * spacing
    }
}

/// The corridors, as pairs of station indices with the lower first.
///
/// petgraph's edge indices are insertion order, which is input order by
/// construction, so this needs no sort to be deterministic. The endpoints do
/// need normalising: petgraph reports them as it stored them.
fn corridors(network: &Network) -> Vec<(usize, usize)> {
    let graph = network.graph();
    graph
        .edge_indices()
        .filter_map(|edge| graph.edge_endpoints(edge))
        .map(|(a, b)| {
            let (a, b) = (a.index(), b.index());
            (a.min(b), a.max(b))
        })
        .collect()
}

/// Each station's `(neighbour, corridor index)` pairs, sorted by neighbour.
fn adjacency(network: &Network, corridors: &[(usize, usize)]) -> Vec<Vec<(usize, usize)>> {
    let mut adjacency = vec![Vec::new(); network.stations().len()];
    for (corridor, &(a, b)) in corridors.iter().enumerate() {
        adjacency[a].push((b, corridor));
        adjacency[b].push((a, corridor));
    }
    for neighbours in &mut adjacency {
        neighbours.sort_unstable();
    }
    adjacency
}

/// A station where every offset collapses to zero: the line set changes, or
/// the degree is not 2.
///
/// **`!= 2` and not `> 2`.** A *terminus* shared by two lines is degree 1, and
/// a bundle that ran to the end of the track without converging would draw two
/// separate stub ends at one stop.
fn is_collapse(network: &Network, station: usize, adjacency: &[Vec<(usize, usize)>]) -> bool {
    let neighbours = &adjacency[station];
    if neighbours.len() != 2 {
        return true;
    }
    network.lines_between(neighbours[0].0, station)
        != network.lines_between(station, neighbours[1].0)
}

/// Every station's offset frame, or `None` where it is a collapse station.
fn station_frames(
    network: &Network,
    layout: &SchematicLayout,
    viewport: &Viewport,
    corridors: &[(usize, usize)],
    adjacency: &[Vec<(usize, usize)>],
) -> Vec<Option<Frame>> {
    let mut frames: Vec<Option<Frame>> = (0..network.stations().len()).map(|_| None).collect();
    let mut claimed = vec![false; corridors.len()];

    for seed in 0..corridors.len() {
        if claimed[seed] {
            continue;
        }
        let path = walk_run(network, corridors, adjacency, &mut claimed, seed);

        // The run's line set is any of its corridors' — they are equal by
        // construction, which is what "the same line set" in the run
        // definition means. Read off the graph edge rather than re-derived
        // from the station lists: two structures answering one question is how
        // they come to disagree.
        let mut lines_by_id: Vec<usize> = network
            .lines_between(path[0], path[1])
            .expect("a run's stations are consecutive")
            .iter()
            .copied()
            .collect();
        lines_by_id.sort_by(|&a, &b| network.lines()[a].id.cmp(&network.lines()[b].id));

        // A closed run repeats its first station as its last, so a plain
        // `windows(3)` would never put that station in the middle. Appending
        // the second station wraps the sequence round to it.
        let mut walk = path.clone();
        if path[0] == path[path.len() - 1] {
            walk.push(path[1]);
        }

        // A run endpoint is a collapse station by construction, so testing the
        // station is the same rule as "zero at the endpoints, mitre in the
        // interior" — and unlike that phrasing it stays true for a run with no
        // endpoints, or two that are the same station.
        for window in walk.windows(3) {
            let (previous, station, next) = (window[0], window[1], window[2]);
            if is_collapse(network, station, adjacency) {
                continue;
            }
            let incoming = towards(layout, viewport, previous, station);
            let outgoing = towards(layout, viewport, station, next);
            let (direction, scale) = mitre(incoming, outgoing);
            frames[station] = Some(Frame {
                direction,
                scale,
                lines_by_id: lines_by_id.clone(),
            });
        }
    }

    frames
}

/// One run's stations, in the run's own direction.
///
/// The path a closed run returns repeats its first station as its last, so
/// `windows(3)` reaches every one of its stations exactly once.
fn walk_run(
    network: &Network,
    corridors: &[(usize, usize)],
    adjacency: &[Vec<(usize, usize)>],
    claimed: &mut [bool],
    seed: usize,
) -> Vec<usize> {
    let (a, b) = corridors[seed];
    claimed[seed] = true;

    let mut path = vec![a, b];
    extend(network, adjacency, claimed, &mut path);

    // A run can close on either extension, and which one depends on where the
    // seed corridor sat in it — so the test is made after each. A pure cycle
    // closes on the first; `[X, a, b, X, c]` seeded at `a`-`b` closes on the
    // second, since both walks run into `X`.
    if path[0] != path[path.len() - 1] {
        path.reverse();
        extend(network, adjacency, claimed, &mut path);
    }

    if path[0] == path[path.len() - 1] {
        return direct_closed_run(path);
    }

    // Direct the run from its lower-indexed endpoint station to the other.
    if path[0] > path[path.len() - 1] {
        path.reverse();
    }
    path
}

/// Grow a path from its tail while the tail is not a collapse station.
fn extend(
    network: &Network,
    adjacency: &[Vec<(usize, usize)>],
    claimed: &mut [bool],
    path: &mut Vec<usize>,
) {
    loop {
        let tail = path[path.len() - 1];
        if tail == path[0] && path.len() > 1 {
            // The walk came back to where it started: the run has closed.
            return;
        }
        if is_collapse(network, tail, adjacency) {
            return;
        }
        // Degree 2, and the corridor we arrived on is already claimed, so the
        // unclaimed one is the way onward.
        let Some(&(next, corridor)) = adjacency[tail]
            .iter()
            .find(|&&(_, corridor)| !claimed[corridor])
        else {
            return;
        };
        claimed[corridor] = true;
        path.push(next);
    }
}

/// A run with no endpoints, or two that are the same station, still needs a
/// direction. Both take the same fallback, which is why it is written once.
///
/// **Direct it from its lowest-indexed station towards whichever neighbour
/// along the run has the lower index.** Either choice draws a valid mirror of
/// the other; what matters is that one of them is named. The rule needs the
/// lowest-indexed station to have two *distinct* run-neighbours, and both
/// shapes supply them: a closed cycle gives every station two by construction,
/// distinct because `from_input` dedupes a 2-cycle into one corridor so the
/// shortest cycle is length 3; and in `[X, a, b, X, c]` every station of the
/// loop has two distinct run-neighbours whichever is lowest.
///
/// Takes and returns the closed form — first station repeated as last.
fn direct_closed_run(path: Vec<usize>) -> Vec<usize> {
    let cycle = &path[..path.len() - 1];
    let start = cycle
        .iter()
        .enumerate()
        .min_by_key(|&(_, station)| *station)
        .map(|(at, _)| at)
        .expect("a run has at least two stations");

    let len = cycle.len();
    let rotated: Vec<usize> = (0..len).map(|k| cycle[(start + k) % len]).collect();
    let mut directed = if rotated[len - 1] < rotated[1] {
        // The lower-indexed neighbour lies the other way round the cycle.
        let mut reversed = vec![rotated[0]];
        reversed.extend(rotated[1..].iter().rev().copied());
        reversed
    } else {
        rotated
    };
    directed.push(directed[0]);
    directed
}

/// The unit direction from one station to another, in SVG user space.
///
/// User space and not cell space: the offset magnitude is in user units
/// because it derives from `stroke_width`, and the viewport's y-flip means the
/// two frames disagree about which side is left.
fn towards(layout: &SchematicLayout, viewport: &Viewport, from: usize, to: usize) -> (f64, f64) {
    let (x1, y1) = viewport.project(layout.positions()[from]);
    let (x2, y2) = viewport.project(layout.positions()[to]);
    normalize((x2 - x1, y2 - y1))
}

/// The left normal of a direction in SVG user space.
fn left_normal(direction: (f64, f64)) -> (f64, f64) {
    normalize((direction.1, -direction.0))
}

fn normalize(vector: (f64, f64)) -> (f64, f64) {
    let length = vector.0.hypot(vector.1);
    if length == 0.0 {
        // Unreachable post-snap — §2.2's occupancy invariant puts every edge
        // at least one cell long — but a zero vector would poison every
        // downstream coordinate rather than draw badly.
        return (0.0, 0.0);
    }
    (vector.0 / length, vector.1 / length)
}

/// Where the offset points at a station, and how far.
///
/// At an interior station of a run the corridor can bend, and the offset point
/// is the **mitre** — the intersection of the two offset lines, which is what
/// keeps the parallel distance exactly `s` on both sides of the corner rather
/// than pinching it. `incoming` and `outgoing` are the two corridor
/// directions, both taken in the run's direction.
///
/// The direction is `normalize(n₁ + n₂)` and the scale `1/cos(θ/2)`, `θ` being
/// the turn angle. `|n₁ + n₂| = 2cos(θ/2)`, so the scale is `2/|n₁ + n₂|` and
/// no trig call is needed — and that spelling shares its guard with the fused
/// form, whose denominator vanishes in the same place.
///
/// A straight-through gives `n₁ = n₂`, a bisector of `n₁` and a scale of
/// exactly 1. Where `n₁ + n₂` is zero the direction is undefined; take `n₁`.
/// The clamp then **subsumes** that anti-parallel case instead of
/// special-casing it — everywhere else it makes the function bounded and
/// continuous approaching that point, which a discontinuous special case at
/// exactly 180° would not, so the guard returns the clamp and not 1.
fn mitre(incoming: (f64, f64), outgoing: (f64, f64)) -> ((f64, f64), f64) {
    let n1 = left_normal(incoming);
    let n2 = left_normal(outgoing);
    let sum = (n1.0 + n2.0, n1.1 + n2.1);
    let length = sum.0.hypot(sum.1);
    if length == 0.0 {
        return (n1, MITRE_SCALE_CLAMP);
    }
    (
        (sum.0 / length, sum.1 / length),
        (2.0 / length).min(MITRE_SCALE_CLAMP),
    )
}

#[cfg(test)]
mod tests {
    //! Phase 5 gate, assertion 4: the mitre's four branches.
    //!
    //! **Neither committed fixture can carry this**, which is the whole reason
    //! it is a separate assertion. On `sample_network.json` the only two
    //! bundled run-interior stations are `oldtown` and `eastbank`, both
    //! straight-through at scale exactly 1; the only bend at a run-interior
    //! station is `southgate`, which carries one line and so multiplies the
    //! mitre by zero; `crossing.json` bundles nothing at all, since its two
    //! lines share no station. The clamp is **structurally unreachable** from
    //! any octilinear fixture — the sharpest such corner is 2.61 against a
    //! limit of 4.
    //!
    //! So of the four branches only straight-through is reached with a nonzero
    //! offset, and an implementation writing `1/cos(θ)` for `1/cos(θ/2)`, or
    //! omitting the clamp, or dividing by zero at anti-parallel, would pass
    //! every other assertion in this phase and the whole suite besides.
    //!
    //! It needs no fixture and no `Network` at all — a direction pair is
    //! enough — and it lives here because `corridor.rs` is a private module
    //! nothing in `llika-core/tests/` can reach into.

    use super::*;

    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-12
    }

    #[test]
    fn a_straight_through_scales_by_exactly_one() {
        let ((dx, dy), scale) = mitre((1.0, 0.0), (1.0, 0.0));
        assert_eq!(scale, 1.0);
        // The bisector of two equal normals is that normal.
        assert!(close(dx, 0.0) && close(dy, -1.0), "direction ({dx}, {dy})");
    }

    #[test]
    fn a_ninety_degree_turn_scales_by_root_two() {
        let ((dx, dy), scale) = mitre((1.0, 0.0), (0.0, 1.0));
        assert!(close(scale, std::f64::consts::SQRT_2), "scale {scale}");

        // Checked analytically as well as numerically: the two offset lines at
        // distance `s` are `y = -s` and `x = s`, which meet at `s * (1, -1)`.
        let root_half = std::f64::consts::FRAC_1_SQRT_2;
        assert!(
            close(dx, root_half) && close(dy, -root_half),
            "direction ({dx}, {dy})"
        );
    }

    /// The near-anti-parallel neighbourhood the clamp exists for: neighbours at
    /// cell offsets `(5,0)` and `(5,1)` put the turn at 168.690°, a raw factor
    /// of 10.148. The clamp threshold is θ > 151.045°, so this shape clamps
    /// with room rather than marginally.
    #[test]
    fn a_near_fold_back_is_clamped_rather_than_diverging() {
        let incoming = normalize((-5.0, 0.0));
        let outgoing = normalize((5.0, 1.0));

        let unclamped = {
            let n1 = left_normal(incoming);
            let n2 = left_normal(outgoing);
            2.0 / (n1.0 + n2.0).hypot(n1.1 + n2.1)
        };
        assert!(
            (unclamped - 10.148).abs() < 1e-3,
            "the premise: the raw factor is 10.148, got {unclamped}"
        );

        let (_, scale) = mitre(incoming, outgoing);
        assert_eq!(scale, MITRE_SCALE_CLAMP);
    }

    /// The magnitude is asserted as well as the direction. §2.5 reaches the
    /// anti-parallel case **through** the clamp rather than through a special
    /// case, and an implementer who reads the guard as a whole special case
    /// would return `1 · s`, which a direction-only assertion accepts.
    #[test]
    fn an_exact_reversal_takes_the_first_normal_at_the_clamp() {
        let ((dx, dy), scale) = mitre((1.0, 0.0), (-1.0, 0.0));
        assert_eq!(scale, MITRE_SCALE_CLAMP);
        assert!(close(dx, 0.0) && close(dy, -1.0), "direction ({dx}, {dy})");
    }

    /// Not gate-required. A run with no endpoints — a closed cycle of degree-2
    /// stations carrying one constant line set — is legal input that neither
    /// committed fixture contains, so without this the fallback ships with no
    /// coverage at all.
    #[test]
    fn a_closed_run_is_directed_from_its_lowest_station_towards_its_lower_neighbour() {
        // The cycle 4 → 7 → 2 → 9 → 4, seeded anywhere. Its lowest station is
        // 2, whose run-neighbours are 7 and 9, so the run runs 2 → 7.
        let directed = direct_closed_run(vec![7, 2, 9, 4, 7]);
        assert_eq!(directed, vec![2, 7, 4, 9, 2]);

        // And the mirror: seeded so the rotation already faces the lower
        // neighbour, it is left alone rather than flipped twice.
        let directed = direct_closed_run(vec![2, 7, 4, 9, 2]);
        assert_eq!(directed, vec![2, 7, 4, 9, 2]);
    }
}
