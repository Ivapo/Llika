//! Rigid moves of whole groups of stations, for the dead end a single-station
//! search cannot see out of.
//!
//! A tight cluster attached to the rest of the map by one long edge cannot
//! shorten that edge by moving any one of its stations, so the per-station sweep
//! sits at a local minimum. Translating the whole group moves it.
//!
//! **A cluster is one side of a bridge**, and specifically the smaller side. That
//! is structural and parameter-free, which is what a length threshold could not
//! be: after snapping, edge lengths are compressed against the target by
//! construction — the sample fixture's seventeen corridors measure 1.0 or 1.41
//! cells and nothing else — so no cut of that distribution separates a cluster
//! from the rest of the map. A whole connected component is excluded by
//! definition rather than by tuning, which matters because every criterion is
//! translation-invariant: translating a whole component leaves `t` bit-identical,
//! and a search that accepts only strict improvements could never fire.
//!
//! **Clusters nest; they are not a partition.** On the sample fixture the seven
//! sides stand in seven containment relations, so one station is translated by
//! several clusters over a single pass. An implementation that dedupes or
//! partitions to tidy that up silently drops the clusters that would have fired.

use std::cmp::Ordering;

use crate::geometry::segments_overlap;
use crate::grid::{GridOccupancy, GridPoint};
use crate::model::Network;

use super::LayoutParams;
use super::candidate::{is_rotation, ordered_neighbours};
use super::cost::{self, total_cost};

/// One side of one bridge: the stations that move together, and the edge that
/// joins them to everything else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Cluster {
    /// Index into [`cost::corridors`], not a station pair. The corridor list is
    /// unnormalised — petgraph reports `(source, target)` in insertion order — so
    /// an edge is identified by its position in that list and never by comparing
    /// endpoints.
    pub bridge: usize,
    /// Station indices, **ascending**. The order is what makes the membership
    /// test a binary search and the tie rule below a first-element comparison.
    pub members: Vec<usize>,
}

/// Every bridge's smaller side, in corridor order, sides of fewer than two
/// stations dropped.
///
/// **Computed once, before the first sweep, and never recomputed.** Bridges are a
/// property of the graph alone, not of the layout, so nothing a move does can
/// change this answer.
///
/// This is the spec's definition transliterated rather than a low-link algorithm
/// re-derived: for each corridor, walk the graph *without* it from each endpoint.
/// The corridor is a bridge exactly when the two walks do not meet, and when they
/// do not, those two reach-sets **are** the two sides — so one pass answers both
/// questions, and the disconnected input the schema admits needs no special case,
/// since other components appear in neither set. The cost is `O(E·(V+E))`, which
/// is roughly 200k operations on a London-sized network and is paid once.
///
/// Sides of one station are dropped because the per-station sweep already moves
/// them: a cluster move over a single station is the same move made more
/// expensively. On the sample fixture that drops exactly the five degree-1
/// termini, leaving seven.
///
/// **Nothing here iterates a `HashMap`.** The adjacency is a `Vec` indexed by
/// station and the reach-sets are collected in ascending station order, so the
/// result is a function of the input file alone — which is what byte-stability
/// across processes rests on.
pub(super) fn find(network: &Network) -> Vec<Cluster> {
    let corridors = cost::corridors(network);
    let stations = network.stations().len();

    let mut adjacency: Vec<Vec<(usize, usize)>> = vec![Vec::new(); stations];
    for (edge, &(a, b)) in corridors.iter().enumerate() {
        adjacency[a].push((b, edge));
        adjacency[b].push((a, edge));
    }

    corridors
        .iter()
        .enumerate()
        .filter_map(|(edge, &(a, b))| {
            let from_a = reach(&adjacency, stations, a, edge);
            // They meet, so removing the corridor left the component whole.
            if from_a.binary_search(&b).is_ok() {
                return None;
            }
            let from_b = reach(&adjacency, stations, b, edge);
            let members = smaller_side(from_a, from_b);
            (members.len() >= 2).then_some(Cluster {
                bridge: edge,
                members,
            })
        })
        .collect()
}

/// The stations reachable from `start` with `without` removed, ascending.
///
/// Ascending by construction rather than in visit order: the frontier's order is
/// an implementation detail and the caller compares these sets, so pinning the
/// output order here means no later rewrite of the walk can change what `find`
/// returns.
fn reach(
    adjacency: &[Vec<(usize, usize)>],
    stations: usize,
    start: usize,
    without: usize,
) -> Vec<usize> {
    let mut seen = vec![false; stations];
    let mut frontier = vec![start];
    seen[start] = true;

    while let Some(station) = frontier.pop() {
        for &(neighbour, edge) in &adjacency[station] {
            if edge != without && !seen[neighbour] {
                seen[neighbour] = true;
                frontier.push(neighbour);
            }
        }
    }

    (0..stations).filter(|station| seen[*station]).collect()
}

/// The smaller of the two sides, ties broken by the lower minimum station index.
///
/// The sides of a bridge are disjoint and both non-empty — each holds at least
/// its own endpoint — so their minimum indices always differ and the rule is
/// total. Both are ascending, so the minimum is the first element.
fn smaller_side(a: Vec<usize>, b: Vec<usize>) -> Vec<usize> {
    match a.len().cmp(&b.len()) {
        Ordering::Less => a,
        Ordering::Greater => b,
        Ordering::Equal => {
            if a[0] < b[0] {
                a
            } else {
                b
            }
        }
    }
}

/// One sweep over the clusters, in corridor order, after the per-station sweep
/// and inside the same iteration.
///
/// Each cluster is offered the same candidate offsets and the same cooling radius
/// a single station gets, accepted under the same strict-improvement rule, so the
/// incumbent holds on a tie and the earlier cell in spiral order wins.
pub(super) fn pass(
    network: &Network,
    positions: &mut [GridPoint],
    occupancy: &mut GridOccupancy,
    clusters: &[Cluster],
    offsets: &[(i64, i64)],
    target_edge_cells: f64,
    params: &LayoutParams,
) {
    for cluster in clusters {
        let mut best = (
            (0, 0),
            total_cost(network, positions, target_edge_cells, params),
        );

        for &offset in offsets {
            if !is_valid_move(network, positions, occupancy, cluster, offset) {
                continue;
            }

            translate(positions, &cluster.members, offset);
            let cost = total_cost(network, positions, target_edge_cells, params);
            translate(positions, &cluster.members, back(offset));

            if cost < best.1 {
                best = (offset, cost);
            }
        }

        if best.0 != (0, 0) {
            apply(positions, occupancy, cluster, best.0);
        }
    }
}

/// Whether the whole cluster may translate by `offset`, against all three of the
/// hard rejections in their group forms.
///
/// **Takes `positions` mutably and restores it**, as the single-station predicate
/// does and for the same reason: two of the three rules are properties of the map
/// *after* the move, and cloning the position vector once per candidate would
/// allocate tens of thousands of times on a small network.
///
/// **Exactly one edge changes geometry** under a rigid translation — the bridge —
/// because it is the only edge joining the two sides and every edge inside a side
/// is carried along unchanged. That is a complete argument for two of the three
/// narrowings below. It is *not* the argument for the third; see there.
pub(super) fn is_valid_move(
    network: &Network,
    positions: &mut [GridPoint],
    occupancy: &GridOccupancy,
    cluster: &Cluster,
    offset: (i64, i64),
) -> bool {
    if !targets_are_free(positions, occupancy, cluster, offset) {
        return false;
    }

    let corridors = cost::corridors(network);
    let (near, far) = corridors[cluster.bridge];

    // Rejection 2 is evaluated at the bridge's two endpoints **only**: every
    // other station's fan is rigidly translated and therefore unchanged.
    let before = (
        ordered_neighbours(network, positions, near),
        ordered_neighbours(network, positions, far),
    );

    translate(positions, &cluster.members, offset);
    let preserved = is_rotation(&before.0, &ordered_neighbours(network, positions, near))
        && is_rotation(&before.1, &ordered_neighbours(network, positions, far));
    let overlaps = bridge_overlaps(&corridors, positions, cluster.bridge);
    translate(positions, &cluster.members, back(offset));

    preserved && !overlaps
}

/// Rejection 1 — **occupancy, read with the cluster's own cells vacated**.
///
/// A target cell counts as free when it is free *or* when a member of this same
/// cluster currently holds it. A translation is injective, so members cannot
/// collide with one another and that is exactly the remaining condition.
///
/// Without the second clause a translation by one cell is rejected by the
/// cluster's own body and **no cluster move can ever fire** — which is the
/// failure that leaves every aggregate assertion about the search still passing.
fn targets_are_free(
    positions: &[GridPoint],
    occupancy: &GridOccupancy,
    cluster: &Cluster,
    offset: (i64, i64),
) -> bool {
    cluster.members.iter().all(|member| {
        let to = positions[*member].offset(offset.0, offset.1);
        match occupancy.station_at(to) {
            None => true,
            Some(holder) => cluster.members.binary_search(&holder).is_ok(),
        }
    })
}

/// Rejection 3 — the bridge may not come to lie **along** another edge: collinear
/// and sharing more than a single point.
///
/// The pair set is `super::candidate`'s and deliberately not `c1`'s — every other
/// edge in the graph, **including** the intra-cluster edge that shares the
/// bridge's near endpoint, because a fold-back along one's own neighbour is
/// invisible to `c1` by construction.
///
/// **The reason the other pairs may be dropped is not "only one edge changes
/// geometry".** Overlap is a pairwise *positional* property, so an intra-cluster
/// edge does move relative to the rest of the map even though its own shape is
/// preserved. What licenses dropping those pairs is that an intra-cluster edge has
/// both endpoints inside the side and an external edge has both outside, so such a
/// pair can never share an endpoint — which makes the dropped pairs exactly `c1`'s
/// pair set, where the closed test already charges a collinear overlap at the
/// heaviest weight in the cost function. That is the soft/hard split one level up,
/// and it leaves a stated asymmetry: a configuration hard-rejected when a single
/// station creates it is only *priced* when a cluster move does.
fn bridge_overlaps(corridors: &[(usize, usize)], positions: &[GridPoint], bridge: usize) -> bool {
    let (a, b) = corridors[bridge];
    corridors.iter().enumerate().any(|(other, (c, d))| {
        other != bridge
            && segments_overlap(positions[a], positions[b], positions[*c], positions[*d])
    })
}

/// Move every member by `offset`, in place.
fn translate(positions: &mut [GridPoint], members: &[usize], offset: (i64, i64)) {
    for member in members {
        positions[*member] = positions[*member].offset(offset.0, offset.1);
    }
}

const fn back((di, dj): (i64, i64)) -> (i64, i64) {
    (-di, -dj)
}

/// Commit an accepted translation to both the positions and the occupancy.
///
/// **The order is load-bearing and it needs no new `grid.rs` API.**
/// [`GridOccupancy`] has no bulk or release method and `relocate` debug-asserts a
/// free target, so relocating members in arbitrary order trips its own
/// precondition. Members move in **decreasing projection along the offset**: the
/// front of the cluster goes first, into cells [`targets_are_free`] has already
/// proved free, and each later member lands on a cell its predecessor has just
/// vacated. A member whose target is held by a sibling is held by one whose
/// projection is greater by `|offset|²`, which is strictly positive, so that
/// sibling has already moved.
///
/// Ties in projection cannot collide with each other — two members at the same
/// projection are at different cells, and a translation is injective — so the
/// sort's stability leaves them in ascending station order and nothing depends on
/// which goes first.
fn apply(
    positions: &mut [GridPoint],
    occupancy: &mut GridOccupancy,
    cluster: &Cluster,
    offset: (i64, i64),
) {
    let mut order = cluster.members.clone();
    order.sort_by(|a, b| projection(positions[*b], offset).cmp(&projection(positions[*a], offset)));

    for member in order {
        let from = positions[member];
        let to = from.offset(offset.0, offset.1);
        occupancy.relocate(member, from, to);
        positions[member] = to;
    }
}

fn projection(cell: GridPoint, (di, dj): (i64, i64)) -> i64 {
    cell.i * di + cell.j * dj
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cost::tests::{at, net};

    /// An occupancy holding every station where the positions say it is.
    fn occupied(positions: &[GridPoint]) -> GridOccupancy {
        let mut occupancy = GridOccupancy::new();
        for (station, cell) in positions.iter().enumerate() {
            occupancy.claim(station, *cell);
        }
        occupancy
    }

    fn ids(network: &Network, cluster: &Cluster) -> Vec<String> {
        cluster
            .members
            .iter()
            .map(|m| network.stations()[*m].id.clone())
            .collect()
    }

    /// The committed 17-station fixture, which the phase's gate is keyed to.
    fn sample() -> Network {
        let input =
            crate::io::parse_input(include_str!("../../tests/fixtures/sample_network.json"))
                .expect("the fixture parses");
        Network::from_input(&input).expect("the fixture is valid")
    }

    /// Assertion 3 — the cluster set is the one the spec defines.
    ///
    /// The expected sides were derived **by hand from the JSON**, not from this
    /// code: the fixture's only cycle is `central → market → quayside →
    /// brookside → southgate → central`, so the other twelve corridors are
    /// bridges, and five of them have a single degree-1 terminus on their
    /// smaller side. What remains is these seven, in corridor order.
    ///
    /// **They nest — seven containment relations over seven sides.** An
    /// implementation that dedupes or partitions to tidy that up passes every
    /// aggregate assertion while silently dropping the clusters that would have
    /// fired.
    #[test]
    fn the_fixtures_clusters_are_the_seven_bridge_sides() {
        let network = sample();
        let clusters = find(&network);

        let expected: [&[&str]; 7] = [
            &["westgate", "riverside", "northgate", "hillcrest"],
            &["westgate", "riverside", "oldtown", "northgate", "hillcrest"],
            &[
                "westgate",
                "riverside",
                "oldtown",
                "eastbank",
                "northgate",
                "hillcrest",
            ],
            &["northgate", "hillcrest"],
            &["fairview", "millpond"],
            &["lakeside", "parkview"],
            &["lakeside", "parkview", "university"],
        ];

        let found: Vec<Vec<String>> = clusters.iter().map(|c| ids(&network, c)).collect();
        assert_eq!(found, expected.map(|side| side.to_vec()).to_vec());

        // Sizes 4, 5, 6, 2, 2, 2, 3 — and none is the whole component, which
        // the bridge rule guarantees by definition rather than by tuning.
        assert!(
            clusters
                .iter()
                .all(|c| c.members.len() < network.stations().len())
        );
    }

    /// Assertion 3's other half, which the fixture cannot supply.
    ///
    /// Two triangles joined by one edge. A fixture-only check cannot tell a
    /// correct bridge finder from one that returns every edge — here that
    /// difference is 1 against 7. And the join splits the graph **evenly**, so
    /// the lower-minimum-index tie rule decides; no tie arises anywhere on the
    /// fixture, whose sides are 4, 5, 6, 2, 2, 2 and 3 over 17 stations.
    #[test]
    fn a_bridge_between_two_cycles_splits_evenly_and_the_lower_index_side_wins() {
        let network = net(
            &["s0", "s1", "s2", "s3", "s4", "s5"],
            &[
                ("a", &["s0", "s1", "s2", "s0"]),
                ("b", &["s2", "s3"]),
                ("c", &["s3", "s4", "s5", "s3"]),
            ],
        );

        let clusters = find(&network);

        assert_eq!(clusters.len(), 1, "only the joining edge is a bridge");
        assert_eq!(clusters[0].bridge, 3, "`s2`-`s3` is the fourth corridor");
        assert_eq!(
            clusters[0].members,
            vec![0, 1, 2],
            "the tie went to the side with the higher minimum index"
        );
    }

    /// A spur `a — b` hanging off `c — d` by the bridge `b — c`, plus an
    /// isolated blocker due east of `b`.
    ///
    /// The bridge splits it 2/2, so the tie rule picks `{a, b}` — the side whose
    /// minimum station index is lower. `b` and `c` are both degree 2, which
    /// makes the order-flip rule vacuous here and leaves the other two
    /// rejections to be tested one at a time.
    fn spur() -> (Network, Vec<GridPoint>, Cluster) {
        let network = net(
            &["a", "b", "c", "d", "blocker"],
            &[("l", &["a", "b", "c", "d"])],
        );
        let positions = at(&[(1, -1), (1, 0), (4, 0), (5, 0), (2, 0)]);
        let cluster = Cluster {
            bridge: 1,
            members: vec![0, 1],
        };
        assert_eq!(find(&network), vec![cluster.clone()]);
        (network, positions, cluster)
    }

    /// Rejection 1 — occupancy, and **the load-bearing negative**.
    ///
    /// A translation onto a cell the cluster itself is vacating must be
    /// accepted. The reflex reading — `is_free` alone — rejects it, because a
    /// one-cell translation always lands on the cluster's own body, and that
    /// makes the whole feature inert while every aggregate assertion about the
    /// search still passes.
    #[test]
    fn a_cluster_may_translate_onto_its_own_vacated_cells() {
        let (network, mut positions, cluster) = spur();
        let occupancy = occupied(&positions);

        // `a` moves onto the cell `b` is leaving.
        assert!(is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            &cluster,
            (0, 1)
        ));

        // And a cell held by a **non**-member still blocks: `blocker` sits due
        // east of `b`, where this translation would put it.
        assert!(!is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            &cluster,
            (1, 0)
        ));
    }

    /// Rejection 3 — the bridge may not come to lie **along** another edge.
    ///
    /// The fold-back is onto the cluster's own internal edge, which shares the
    /// bridge's near endpoint — precisely the case `c1` cannot see, since it
    /// excludes endpoint-sharing pairs by construction. Translating `{a, b}` by
    /// `(3, 3)` puts `b` due north of `c` with `a` between them, so `a — b` lies
    /// inside `b — c`.
    #[test]
    fn a_bridge_folded_back_along_the_clusters_own_edge_is_rejected() {
        let (network, mut positions, cluster) = spur();
        let occupancy = occupied(&positions);

        assert!(!is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            &cluster,
            (3, 3)
        ));
    }

    /// **The load-bearing negative for rejection 3.** Two edges meeting at
    /// exactly one point are a legitimate straight-through, not an overlap.
    ///
    /// This is the assertion that fails a `segments_intersect` implementation
    /// deterministically rather than by luck of fixture shape — that predicate is
    /// *closed*, so it counts the touching endpoint — and it fails it on the move
    /// this layout most wants to make.
    #[test]
    fn a_cluster_sliding_into_a_collinear_straight_through_is_not_rejected() {
        let network = net(&["a", "b", "c", "d"], &[("l", &["a", "b", "c", "d"])]);
        let mut positions = at(&[(0, 0), (1, 0), (4, 0), (5, 0)]);
        let occupancy = occupied(&positions);
        let cluster = Cluster {
            bridge: 1,
            members: vec![0, 1],
        };
        assert_eq!(find(&network), vec![cluster.clone()]);

        // `a — b` and the bridge `b — c` end up on one line, touching at `b`.
        assert!(is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            &cluster,
            (2, 0)
        ));
    }

    /// Rejection 2 — the translation flips the fan at the bridge's **outside**
    /// endpoint, which is the only other station whose fan changes.
    ///
    /// `v` is a three-armed junction, the lowest degree at which the rule is not
    /// vacuous: neighbours due east, due west, and `u` due north. Dragging the
    /// cluster from north of `v` to south of it turns `[p, u, q]` into
    /// `[p, q, u]`, which is not a rotation. `w` sits east of `u` rather than
    /// beyond it, so the fold-back rule is not what fires.
    #[test]
    fn a_translation_that_flips_the_outside_endpoints_fan_is_rejected() {
        let network = net(
            &["v", "p", "q", "u", "w"],
            &[("a", &["p", "v", "q"]), ("b", &["v", "u", "w"])],
        );
        let mut positions = at(&[(0, 0), (2, 0), (-2, 0), (0, 2), (1, 2)]);
        let occupancy = occupied(&positions);
        let cluster = Cluster {
            bridge: 2,
            members: vec![3, 4],
        };
        assert_eq!(find(&network), vec![cluster.clone()]);

        assert_eq!(ordered_neighbours(&network, &positions, 0), vec![1, 3, 2]);
        assert!(!is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            &cluster,
            (0, -4)
        ));

        // A translation that merely rotates that fan is allowed: swinging `u`
        // round to the north-east leaves the sequence untouched.
        assert!(is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            &cluster,
            (1, 0)
        ));
    }

    /// Applying an accepted move keeps the occupancy consistent.
    ///
    /// The order is what buys that. `a`'s target is the cell `b` currently
    /// holds, so relocating in station order asks `GridOccupancy::relocate` to
    /// move `a` onto an occupied cell and trips its debug assertion. Decreasing
    /// projection along the offset sends `b` first.
    #[test]
    fn an_accepted_move_relocates_the_front_of_the_cluster_first() {
        let (_, mut positions, cluster) = spur();
        let mut occupancy = occupied(&positions);

        apply(&mut positions, &mut occupancy, &cluster, (0, 1));

        assert_eq!(positions[0], GridPoint::new(1, 0));
        assert_eq!(positions[1], GridPoint::new(1, 1));
        assert_eq!(occupancy.station_at(GridPoint::new(1, 0)), Some(0));
        assert_eq!(occupancy.station_at(GridPoint::new(1, 1)), Some(1));
        assert!(occupancy.is_free(GridPoint::new(1, -1)), "the tail vacated");
        assert_eq!(occupancy.len(), 5, "a relocation is not a claim");
    }
}
