//! The five cost criteria, and the weighted total the search minimises.
//!
//! Lower is better, and `t = w1*c1 + w2*c2 + w3*c3 + w4*c4 + w5*c5`. Each
//! criterion is separately public because the five weights become the sliders
//! of the roadmap's UI, so each has to be independently computable and
//! independently meaningful — a fused score could not be steered one axis at a
//! time.
//!
//! **The domain is the grid**, not the projected metre plane: the search moves
//! stations between cells, so a criterion scored in metres would be scoring a
//! different object from the one the search moves. Every function here takes
//! positions as `&[GridPoint]` indexed by station index rather than a
//! [`SchematicLayout`](super::SchematicLayout), because Phase 3 scores
//! *candidate* position sets that no layout owns yet.
//!
//! These are **this spec's operational definitions**, not quotations from the
//! 2011 paper. They implement the criteria it names, but the functional forms
//! are chosen here. OQ-1's re-read of the paper is the occasion to reconcile
//! them, and a difference found then is a recorded change to these formulas,
//! not a defect in the phase that shipped them.

use std::f64::consts::{PI, TAU};

use petgraph::visit::EdgeRef;

use crate::geometry::{
    angular_gaps, cell_distance, direction, interior_angle, octilinear_deviation,
    segments_intersect,
};
use crate::grid::GridPoint;
use crate::model::Network;

use super::LayoutParams;

/// The five criteria, scored separately. Weighted into `t` by [`Cost::total`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cost {
    /// Edge crossings.
    pub c1: f64,
    /// Edge length against the target.
    pub c2: f64,
    /// Angular resolution at a station.
    pub c3: f64,
    /// Line straightness.
    pub c4: f64,
    /// Four-gonality.
    pub c5: f64,
}

impl Cost {
    /// The weighted total `t`.
    pub fn total(&self, params: &LayoutParams) -> f64 {
        params.w_crossings * self.c1
            + params.w_edge_length * self.c2
            + params.w_angular_resolution * self.c3
            + params.w_straightness * self.c4
            + params.w_octilinearity * self.c5
    }
}

/// Score all five in one pass over the network.
pub fn evaluate(network: &Network, positions: &[GridPoint], target_edge_cells: f64) -> Cost {
    Cost {
        c1: c1_crossings(network, positions),
        c2: c2_edge_length(network, positions, target_edge_cells),
        c3: c3_angular_resolution(network, positions),
        c4: c4_straightness(network, positions),
        c5: c5_octilinearity(network, positions),
    }
}

/// `t` directly, for a caller that wants the number and not the breakdown.
pub fn total_cost(
    network: &Network,
    positions: &[GridPoint],
    target_edge_cells: f64,
    params: &LayoutParams,
) -> f64 {
    evaluate(network, positions, target_edge_cells).total(params)
}

/// The corridors, as pairs of station indices, in input order.
///
/// Visible to the rest of `layout` so the search's overlap rejection walks the
/// same edge list the criteria score, rather than a second spelling of it.
pub(super) fn corridors(network: &Network) -> Vec<(usize, usize)> {
    let graph = network.graph();
    graph
        .edge_indices()
        .filter_map(|edge| graph.edge_endpoints(edge))
        .map(|(a, b)| (a.index(), b.index()))
        .collect()
}

/// `c1` — **edge crossings**. One point for every unordered pair of edges that
/// share no endpoint station and whose closed segments intersect.
///
/// Sharing no endpoint is what makes a path graph score zero: consecutive edges
/// of a path meet at their common station by construction, which is not a
/// crossing. The value is always integral; it is an `f64` only so the weighted
/// sum needs no conversion at the call site.
///
/// **That exclusion is exactly what the search's overlap rejection does not
/// do.** `super::candidate` walks this same corridor list *including*
/// endpoint-sharing pairs, because the case it exists for — a line folded back
/// along its own neighbour — is invisible here by construction. The two loops
/// look alike and are not: neither is the other with a typo.
pub fn c1_crossings(network: &Network, positions: &[GridPoint]) -> f64 {
    let edges = corridors(network);
    let mut crossings = 0usize;

    for (n, &(a1, a2)) in edges.iter().enumerate() {
        for &(b1, b2) in &edges[n + 1..] {
            let shares_endpoint = a1 == b1 || a1 == b2 || a2 == b1 || a2 == b2;
            if shares_endpoint {
                continue;
            }
            if segments_intersect(positions[a1], positions[a2], positions[b1], positions[b2]) {
                crossings += 1;
            }
        }
    }

    crossings as f64
}

/// `c2` — **edge length**. `Σ (|e| / L − 1)²`, with `|e|` in cells and `L` the
/// target [`super::SchematicLayout::target_edge_cells`] carries.
///
/// Zero exactly when every edge is `L` cells long. Squared rather than absolute
/// so one badly wrong edge costs more than several slightly wrong ones, which
/// is what stops the criterion from being satisfied by a uniformly stretched
/// map.
pub fn c2_edge_length(network: &Network, positions: &[GridPoint], target: f64) -> f64 {
    corridors(network)
        .into_iter()
        .map(|(a, b)| {
            let ratio = cell_distance(positions[a], positions[b]) / target;
            (ratio - 1.0).powi(2)
        })
        .sum()
}

/// `c3` — **angular resolution**. For each station of degree `d ≥ 2`, the sum
/// over its `d` angular gaps of `|θ_k − 2π/d|`, in radians.
///
/// This is what keeps a multi-line interchange legible: it wants the edges
/// leaving a station spread evenly rather than fanned into one quadrant.
/// Degrees 0 and 1 contribute nothing — there is no spacing to be uneven.
///
/// **It has a positive floor at some degrees, by construction.** On an
/// octilinear grid every gap is a multiple of `π/4`, so an even spread needs
/// `8/d` integral — reachable at exactly degree 1, 2, 4 and 8, and unreachable
/// at 3, 5, 6 and 7. A degree-3 station's best is `135°/135°/90°`, costing
/// `FRAC_PI_3`. That is what a soft penalty does where the grid forbids its
/// optimum, not a defect.
pub fn c3_angular_resolution(network: &Network, positions: &[GridPoint]) -> f64 {
    (0..network.stations().len())
        .map(|station| {
            let incident = incident_directions(network, positions, station);
            if incident.len() < 2 {
                return 0.0;
            }

            let directions: Vec<f64> = incident.iter().map(|(theta, _)| *theta).collect();
            let ideal = TAU / directions.len() as f64;
            angular_gaps(&directions)
                .into_iter()
                .map(|gap| (gap - ideal).abs())
                .sum::<f64>()
        })
        .sum()
}

/// The direction of every corridor leaving `station`, paired with the station at
/// its far end, **ordered**: ascending direction, ties broken by neighbour
/// index.
///
/// The sort belongs here rather than at either call site, because that ordering
/// is a rule two things depend on: `c3` needs it to make its gap sequence
/// deterministic, and the search's order-flip rejection is pinned to it by name.
/// A comparator written out twice is one that can drift.
pub(super) fn incident_directions(
    network: &Network,
    positions: &[GridPoint],
    station: usize,
) -> Vec<(f64, usize)> {
    let graph = network.graph();
    let node = network.node(station);
    let mut incident: Vec<(f64, usize)> = graph
        .edges(node)
        .map(|edge| {
            let neighbour = if edge.source() == node {
                edge.target()
            } else {
                edge.source()
            }
            .index();
            (
                direction(positions[station], positions[neighbour]),
                neighbour,
            )
        })
        .collect();

    // Two incident edges can share a direction, so the ordering breaks ties by
    // neighbour station index, which keeps the sequence deterministic rather
    // than dependent on the graph's edge enumeration.
    incident.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    incident
}

/// `c4` — **line straightness**. For each line, at every interior station of
/// its walk whose **degree is 2**, `π − φ` where `φ` is the interior angle at
/// that station — both incident directions measured outward from it, never the
/// turn angle.
///
/// Zero when the line runs straight through, rising to `π` at a full reversal.
/// Summed over lines rather than over stations, so two lines sharing a corridor
/// each pay their own bend.
///
/// Degree-2 only, deliberately: a bend at a junction is legitimate, and this
/// criterion exists to stop a line kinking where nothing forces it to.
pub fn c4_straightness(network: &Network, positions: &[GridPoint]) -> f64 {
    network
        .lines()
        .iter()
        .flat_map(|line| line.stations.windows(3))
        .map(|walk| {
            let [before, at, after] = [&walk[0], &walk[1], &walk[2]].map(|id| {
                network
                    .station_index(id)
                    .expect("every line's stations are resolved by Network::from_input")
            });

            if network.degree(at) != 2 {
                return 0.0;
            }

            let phi = interior_angle(
                direction(positions[at], positions[before]),
                direction(positions[at], positions[after]),
            );
            PI - phi
        })
        .sum()
}

/// `c5` — **four-gonality**. `Σ` over edges of the distance from the edge's
/// direction to the nearest multiple of 45 degrees, in radians.
///
/// This is the octilinear-snap criterion and the reason the output looks like a
/// transit poster rather than a plot. Exactly zero at the eight octilinear
/// directions — see [`octilinear_deviation`] for why that exactness is a
/// property of this formulation and not of every faithful-looking one.
pub fn c5_octilinearity(network: &Network, positions: &[GridPoint]) -> f64 {
    corridors(network)
        .into_iter()
        .map(|(a, b)| octilinear_deviation(direction(positions[a], positions[b])))
        .sum()
}

#[cfg(test)]
pub(super) mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3, FRAC_PI_4};

    use super::*;
    use crate::io::InputSchema;
    use crate::model::{Line, Station};

    /// A network from ids alone. Coordinates are irrelevant here — every
    /// criterion is scored over the `&[GridPoint]` passed alongside — so they
    /// are all zero, and the graph is built through the real validating
    /// constructor rather than by hand.
    pub(in crate::layout) fn net(stations: &[&str], lines: &[(&str, &[&str])]) -> Network {
        let input = InputSchema {
            stations: stations
                .iter()
                .map(|id| Station {
                    id: (*id).to_string(),
                    name: (*id).to_string(),
                    lat: 0.0,
                    lon: 0.0,
                })
                .collect(),
            lines: lines
                .iter()
                .map(|(id, walk)| Line {
                    id: (*id).to_string(),
                    name: (*id).to_string(),
                    color: "#000000".to_string(),
                    stations: walk.iter().map(|s| (*s).to_string()).collect(),
                })
                .collect(),
        };
        Network::from_input(&input).expect("the hand-built network is valid")
    }

    pub(in crate::layout) fn at(cells: &[(i64, i64)]) -> Vec<GridPoint> {
        cells.iter().map(|(i, j)| GridPoint::new(*i, *j)).collect()
    }

    /// A single edge from the origin to `offset`, for the one-edge criteria.
    fn one_edge(offset: (i64, i64)) -> (Network, Vec<GridPoint>) {
        (
            net(&["a", "b"], &[("l", &["a", "b"])]),
            at(&[(0, 0), offset]),
        )
    }

    // ---- assertion 1: c5 ----

    /// Exactly `0.0` at each of the eight unit cell offsets. An equality
    /// assertion and never `to_bits`: three of the eight return negative zero
    /// under an unnormalised direction, and `assert_eq!` passes on `-0.0` where
    /// a bit comparison does not.
    #[test]
    fn c5_is_exactly_zero_at_the_eight_octilinear_offsets() {
        for offset in [
            (1, 0),
            (1, 1),
            (0, 1),
            (-1, 1),
            (-1, 0),
            (-1, -1),
            (0, -1),
            (1, -1),
        ] {
            let (network, positions) = one_edge(offset);
            let c5 = c5_octilinearity(&network, &positions);
            assert_eq!(c5, 0.0, "offset {offset:?} scored {c5}");
        }
    }

    /// And strictly positive off those eight. The offset is `(2,1)`, 26.565°:
    /// 22.5° is not constructible on an integer grid — `tan 22.5° = 0.414…` —
    /// and the nearest offsets the grid admits are `(2,1)` and `(12,5)` at
    /// 22.620°.
    #[test]
    fn c5_is_positive_at_an_off_angle_the_grid_admits() {
        let (network, positions) = one_edge((2, 1));
        assert!(c5_octilinearity(&network, &positions) > 0.0);
    }

    // ---- assertion 2: c1 ----

    /// A path graph scores zero: consecutive edges meet at their shared
    /// station by construction, and pairs sharing an endpoint are excluded.
    #[test]
    fn c1_is_zero_for_a_path_graph() {
        let network = net(&["a", "b", "c", "d"], &[("l", &["a", "b", "c", "d"])]);
        let positions = at(&[(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(c1_crossings(&network, &positions), 0.0);

        // Including where the path doubles back over its own corner without
        // any two disjoint edges meeting.
        let positions = at(&[(0, 0), (1, 0), (1, 1), (0, 1)]);
        assert_eq!(c1_crossings(&network, &positions), 0.0);
    }

    /// Two disjoint edges through each other: exactly one crossing.
    #[test]
    fn c1_counts_a_proper_crossing_once() {
        let network = net(
            &["a", "b", "c", "d"],
            &[("l", &["a", "b"]), ("m", &["c", "d"])],
        );
        let positions = at(&[(0, 0), (2, 2), (0, 2), (2, 0)]);
        assert_eq!(c1_crossings(&network, &positions), 1.0);
    }

    /// And a pair that merely touch — an endpoint of one lying in the other's
    /// interior — which the closed-segment test counts. Occupancy keeps two
    /// *stations* out of one cell, but nothing stops an edge running over a
    /// cell some other station occupies.
    #[test]
    fn c1_counts_a_touching_pair_once() {
        let network = net(
            &["a", "b", "c", "d"],
            &[("l", &["a", "b"]), ("m", &["c", "d"])],
        );
        // `c` sits on the interior of a-b, and its edge leaves northward.
        let positions = at(&[(0, 0), (4, 0), (2, 0), (2, 3)]);
        assert_eq!(c1_crossings(&network, &positions), 1.0);
    }

    // ---- assertion 3: c3 ----

    /// A degree-4 cross reaches the criterion's optimum exactly. Four gaps of
    /// `π/2` against an ideal of `2π/4`, all bit-identical.
    #[test]
    fn c3_is_exactly_zero_for_an_even_degree_four_cross() {
        let network = net(
            &["hub", "e", "n", "w", "s"],
            &[("l", &["e", "hub", "w"]), ("m", &["n", "hub", "s"])],
        );
        let positions = at(&[(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)]);
        assert_eq!(c3_angular_resolution(&network, &positions), 0.0);
    }

    /// The same station with its edges fanned to one side is strictly worse.
    #[test]
    fn c3_is_positive_for_an_uneven_degree_four_spread() {
        let network = net(
            &["hub", "e", "n", "w", "s"],
            &[("l", &["e", "hub", "w"]), ("m", &["n", "hub", "s"])],
        );
        let positions = at(&[(0, 0), (1, 0), (1, 1), (0, 1), (-1, 0)]);
        assert!(c3_angular_resolution(&network, &positions) > 0.0);
    }

    /// The degree-3 floor. `135°/135°/90°` is the best of the ten partitions of
    /// 360° into three multiples of 45°, and it costs `FRAC_PI_3`.
    ///
    /// Spelled `FRAC_PI_3` and **not** `PI / 3.0`: the two are different
    /// doubles one ulp apart — `3ff0c152382d7366` against `3ff0c152382d7365` —
    /// and the computed floor is bit-exactly the former, so the natural
    /// spelling fails on a correct implementation.
    #[test]
    fn c3_bottoms_out_at_frac_pi_3_for_the_best_degree_three_station() {
        let network = net(
            &["hub", "e", "nw", "s"],
            &[("l", &["e", "hub", "nw"]), ("m", &["hub", "s"])],
        );
        // 0°, 135°, 270° — gaps of 135/135/90.
        let positions = at(&[(0, 0), (1, 0), (-1, 1), (0, -1)]);
        assert_eq!(c3_angular_resolution(&network, &positions), FRAC_PI_3);
    }

    /// Degree 0 and 1 have no spacing to be uneven and contribute nothing —
    /// otherwise every terminus on the map would carry a constant penalty.
    #[test]
    fn c3_ignores_stations_below_degree_two() {
        let network = net(&["a", "b"], &[("l", &["a", "b"])]);
        let positions = at(&[(0, 0), (3, 1)]);
        assert_eq!(c3_angular_resolution(&network, &positions), 0.0);
    }

    // ---- assertion 4: c4 ----

    /// A line running straight through a degree-2 station pays nothing, and
    /// that holds off-axis too: the interior angle is exactly `π` for any
    /// collinear opposite pair.
    #[test]
    fn c4_is_zero_where_a_line_runs_straight_through() {
        let network = net(&["a", "b", "c"], &[("l", &["a", "b", "c"])]);
        assert_eq!(
            c4_straightness(&network, &at(&[(0, 0), (1, 0), (2, 0)])),
            0.0
        );
        // Non-octilinear, and still exactly zero.
        assert_eq!(
            c4_straightness(&network, &at(&[(0, 0), (2, 1), (4, 2)])),
            0.0
        );
    }

    /// A bend at a degree-2 station costs `π − φ`.
    #[test]
    fn c4_is_positive_at_a_bend_through_a_degree_two_station() {
        let network = net(&["a", "b", "c"], &[("l", &["a", "b", "c"])]);
        let cost = c4_straightness(&network, &at(&[(0, 0), (1, 0), (1, 1)]));
        assert_eq!(cost, FRAC_PI_2);

        let gentler = c4_straightness(&network, &at(&[(0, 0), (1, 0), (2, 1)]));
        assert_eq!(gentler, FRAC_PI_4);
        assert!(gentler < cost, "a gentler bend costs less");
    }

    /// A bend at a **degree-3** station is free. A bend at a junction is
    /// legitimate; this criterion exists to stop a line kinking where nothing
    /// forces it to.
    #[test]
    fn c4_is_unchanged_by_a_bend_at_a_degree_three_station() {
        let straight = net(&["a", "b", "c"], &[("l", &["a", "b", "c"])]);
        let bent = at(&[(0, 0), (1, 0), (1, 1)]);
        assert!(c4_straightness(&straight, &bent) > 0.0);

        // The same bend, with a spur making the middle station degree 3.
        let junction = net(
            &["a", "b", "c", "spur"],
            &[("l", &["a", "b", "c"]), ("m", &["b", "spur"])],
        );
        let with_spur = at(&[(0, 0), (1, 0), (1, 1), (2, 0)]);
        assert_eq!(c4_straightness(&junction, &with_spur), 0.0);
    }

    /// Two lines sharing one corridor each pay their own bend, because the sum
    /// runs over lines and not over stations.
    #[test]
    fn c4_charges_every_line_over_a_shared_bend() {
        let one = net(&["a", "b", "c"], &[("l", &["a", "b", "c"])]);
        let two = net(
            &["a", "b", "c"],
            &[("l", &["a", "b", "c"]), ("m", &["a", "b", "c"])],
        );
        let bent = at(&[(0, 0), (1, 0), (1, 1)]);
        assert_eq!(
            c4_straightness(&two, &bent),
            2.0 * c4_straightness(&one, &bent)
        );
    }

    // ---- assertion 5: c2 ----

    /// Zero exactly when every edge is the target, positive otherwise, and at
    /// any target — not just one cell.
    #[test]
    fn c2_is_zero_at_the_target_length_and_positive_away_from_it() {
        let network = net(&["a", "b", "c"], &[("l", &["a", "b", "c"])]);

        let unit = at(&[(0, 0), (1, 0), (2, 0)]);
        assert_eq!(c2_edge_length(&network, &unit, 1.0), 0.0);
        assert!(c2_edge_length(&network, &unit, 3.0) > 0.0);

        let triple = at(&[(0, 0), (3, 0), (6, 0)]);
        assert_eq!(c2_edge_length(&network, &triple, 3.0), 0.0);
        assert!(c2_edge_length(&network, &triple, 1.0) > 0.0);

        // Two edges each off the target by one cell: (2/1 − 1)² twice.
        let stretched = at(&[(0, 0), (2, 0), (4, 0)]);
        assert_eq!(c2_edge_length(&network, &stretched, 1.0), 2.0);
    }

    // ---- assertion 6: the weighted total ----

    /// One weight at 1 and the other four at 0 makes `t` that criterion
    /// exactly, for each of the five in turn.
    #[test]
    fn the_total_is_the_weighted_sum_of_the_five() {
        let network = net(
            &["a", "b", "c", "d", "e"],
            &[("l", &["a", "b", "c"]), ("m", &["d", "e"])],
        );
        let positions = at(&[(0, 0), (2, 1), (3, 3), (1, -1), (0, 3)]);
        let cost = evaluate(&network, &positions, 1.0);

        let off = LayoutParams {
            w_crossings: 0.0,
            w_edge_length: 0.0,
            w_angular_resolution: 0.0,
            w_straightness: 0.0,
            w_octilinearity: 0.0,
            ..LayoutParams::default()
        };

        let one_hot = [
            (
                LayoutParams {
                    w_crossings: 1.0,
                    ..off.clone()
                },
                cost.c1,
            ),
            (
                LayoutParams {
                    w_edge_length: 1.0,
                    ..off.clone()
                },
                cost.c2,
            ),
            (
                LayoutParams {
                    w_angular_resolution: 1.0,
                    ..off.clone()
                },
                cost.c3,
            ),
            (
                LayoutParams {
                    w_straightness: 1.0,
                    ..off.clone()
                },
                cost.c4,
            ),
            (
                LayoutParams {
                    w_octilinearity: 1.0,
                    ..off.clone()
                },
                cost.c5,
            ),
        ];
        for (params, expected) in one_hot {
            assert_eq!(cost.total(&params), expected);
        }

        // Every weight off scores nothing at all — the shape a derived
        // `Default` would have given, and the reason this one is written out.
        assert_eq!(cost.total(&off), 0.0);

        // The criteria are non-trivial on this graph, so the identities above
        // are not five readings of zero.
        assert!(cost.c2 > 0.0 && cost.c3 > 0.0 && cost.c4 > 0.0 && cost.c5 > 0.0);

        // And `total_cost` is the same number by the shorter road.
        assert_eq!(
            total_cost(&network, &positions, 1.0, &LayoutParams::default()),
            cost.total(&LayoutParams::default())
        );
    }

    /// A scorer whose weights are all zero scores nothing, and would sail
    /// through Phase 3's cost-decrease gate. `Default` is written out for this
    /// reason and the gate checks it.
    #[test]
    fn every_default_weight_is_non_zero() {
        let params = LayoutParams::default();
        for (name, weight) in [
            ("w_crossings", params.w_crossings),
            ("w_edge_length", params.w_edge_length),
            ("w_angular_resolution", params.w_angular_resolution),
            ("w_straightness", params.w_straightness),
            ("w_octilinearity", params.w_octilinearity),
        ] {
            assert!(weight != 0.0, "{name} defaults to zero");
            assert!(weight.is_finite(), "{name} is not finite");
        }
    }
}
