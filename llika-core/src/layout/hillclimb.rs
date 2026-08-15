//! The search: a fixed number of sweeps, each station moved to whichever free
//! cell nearby lowers the cost most.
//!
//! **No randomness anywhere**, so a given input and parameter set always produce
//! the same map. Stations are swept in input-file order, candidates are
//! enumerated in the grid's spiral order, and ties go to the earlier candidate —
//! three rules that together make determinism a property rather than an
//! intention. Equal-cost candidates are common rather than exotic: a symmetric
//! neighbourhood produces them constantly.
//!
//! **Reaching a fixed point early is correct behaviour, not a stall**, and the
//! search stops when it does. `iterations` is a bound, not a target.
//!
//! The exit is **output-identical**, not merely output-similar, which is what
//! makes it a saving rather than a trade. If an iteration moves nothing in
//! *either* pass, positions and occupancy are unchanged entering the next; the
//! cooling radius is non-increasing, so that iteration's candidate set is a
//! subset of the one just exhausted over an identical layout; and clusters are a
//! function of the graph rather than of the positions, so the cluster pass sees
//! the same set. It therefore also moves nothing, and by induction so does every
//! iteration after it. On the sample fixture the search converges inside sweep 1
//! and 199 of the 200 sweeps were provably no-ops.
//!
//! **The test is the whole iteration's and not the station sweep's.** A cluster
//! can have an improving move at a layout where no single station does — that is
//! the dead end [`super::cluster`] exists for — so an exit keyed to the station
//! sweep alone would stop one pass short of the fixed point and produce a
//! different map.
//!
//! One consequence worth stating: **an isolated station never moves.** With no
//! incident edges it contributes to no criterion, so every candidate scores
//! identically and none is *strictly* lower. It stays where the snap put it,
//! which is right — nothing constrains it, so one cell is as good as another.

use crate::grid::{GridOccupancy, GridPoint};
use crate::model::Network;

use super::LayoutParams;
use super::candidate::{is_valid_move, spiral_offsets};
use super::cluster;
use super::cost::total_cost;

/// Hill-climb `positions` in place, keeping `occupancy` in step with them.
///
/// Returns the number of iterations actually executed, which is what makes the
/// early exit observable rather than inferred from a layout that would look the
/// same either way.
pub(super) fn run(
    network: &Network,
    positions: &mut [GridPoint],
    occupancy: &mut GridOccupancy,
    target_edge_cells: f64,
    params: &LayoutParams,
) -> u32 {
    // Bridges are a property of the graph and not of the layout, so the cluster
    // set is built **once** here and never recomputed as stations move.
    let clusters = if params.cluster_moves {
        cluster::find(network)
    } else {
        Vec::new()
    };

    let mut executed = 0;

    for k in 0..params.iterations {
        // `r` is fixed across a sweep, so the candidate offsets are built once
        // per iteration rather than once per station.
        let offsets = spiral_offsets(cooling_radius(k, params));
        let mut moved = false;

        for station in 0..network.stations().len() {
            let from = positions[station];
            let mut best = (
                from,
                total_cost(network, positions, target_edge_cells, params),
            );

            for (di, dj) in &offsets {
                let to = from.offset(*di, *dj);
                if !is_valid_move(network, positions, occupancy, station, to) {
                    continue;
                }

                positions[station] = to;
                let cost = total_cost(network, positions, target_edge_cells, params);
                positions[station] = from;

                // Strictly lower, so the incumbent holds on a tie — and since
                // the incumbent starts as the station's own cell and candidates
                // arrive in spiral order, that means "stay put unless something
                // improves, and prefer the earlier cell when two do equally".
                if cost < best.1 {
                    best = (to, cost);
                }
            }

            if best.0 != from {
                occupancy.relocate(station, from, best.0);
                positions[station] = best.0;
                moved = true;
            }
        }

        // **After** the per-station sweep and inside the same iteration, so a
        // cluster is offered the map the stations have just settled into. `|=`
        // and not `||`: the pass runs whether or not a station moved.
        moved |= cluster::pass(
            network,
            positions,
            occupancy,
            &clusters,
            &offsets,
            target_edge_cells,
            params,
        );

        executed += 1;
        if !moved {
            break;
        }
    }

    executed
}

/// The movement radius at sweep `k`: linear, integral, and never below one cell.
///
/// `r_k = max(1, round(r_0 * (1 - k / iterations)))`. The `max` is load-bearing
/// at the far end, where `round(3 * 0.005)` is already `0` at `k = 199`; a
/// zero radius would offer no candidates at all for the last half of the run.
///
/// The `0/0` at `iterations = 0` is unreachable, because the caller's loop is
/// then empty — which is what makes zero iterations mean "snap and stop".
///
/// Both parameters are `u32` so `f64::from` is lossless and total, which leaves
/// the whole schedule free of a numeric cast except the final rounding — and
/// that one is of a value the arithmetic above keeps in `0..=r_0`. `k` is
/// strictly below `iterations`, so `progress` is in `(0, 1]` and never negative.
fn cooling_radius(k: u32, params: &LayoutParams) -> u32 {
    let progress = 1.0 - f64::from(k) / f64::from(params.iterations);
    let radius = (f64::from(params.initial_radius) * progress).round() as u32;
    radius.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::cost::tests::{at, net};

    fn params(initial_radius: u32, iterations: u32) -> LayoutParams {
        LayoutParams {
            initial_radius,
            iterations,
            ..LayoutParams::default()
        }
    }

    #[test]
    fn the_radius_shrinks_linearly_to_one_cell() {
        let p = params(3, 200);

        assert_eq!(cooling_radius(0, &p), 3);
        assert_eq!(cooling_radius(100, &p), 2); // round(3 * 0.5) = 2
        assert_eq!(cooling_radius(199, &p), 1); // round(3 * 0.005) = 0, clamped
    }

    /// The radius never reaches zero, which would leave the last sweeps with no
    /// candidates to consider.
    #[test]
    fn the_radius_never_falls_below_one() {
        let p = params(1, 10);
        for k in 0..10 {
            assert!(cooling_radius(k, &p) >= 1);
        }
        // A radius of zero is representable and means one, which is the
        // clamp's other job; a negative one is not representable at all, so the
        // flag Phase 6 derives rejects it at the boundary rather than silently
        // meaning something.
        assert_eq!(cooling_radius(0, &params(0, 10)), 1);
    }

    /// **The spiral tie-break**, which §2.4 pins as a determinism rule: where two
    /// candidates lower `t` by the same amount, the *earlier* cell in spiral
    /// order wins.
    ///
    /// Untested, the rule is free to invert unnoticed — flipping the comparison
    /// in `run` to `<=`, so the last equal-best candidate wins instead, leaves
    /// every other test in the workspace green while silently changing the map.
    ///
    /// The tie is built rather than found. `blocker` sits due east of `b`, on the
    /// single cell that would otherwise win outright, which leaves `(1, 1)` and
    /// `(1, -1)` level: both are one diagonal from `a` and both octilinear, so
    /// their costs agree bit-for-bit. `(1, 1)` is index 1 of ring 1 and `(1, -1)`
    /// is index 7.
    #[test]
    fn the_earlier_cell_in_spiral_order_wins_a_tie() {
        let network = net(&["b", "blocker", "a"], &[("l", &["b", "a"])]);
        let mut positions = at(&[(0, 0), (1, 0), (2, 0)]);

        let mut occupancy = GridOccupancy::new();
        for (station, cell) in positions.iter().enumerate() {
            occupancy.claim(station, *cell);
        }

        run(&network, &mut positions, &mut occupancy, 1.0, &params(1, 1));

        assert_eq!(
            positions[0],
            GridPoint::new(1, 1),
            "the tie went to the later cell in spiral order"
        );

        // And an isolated station never moves: with no incident edges it appears
        // in no criterion, so every candidate scores identically and none is
        // *strictly* lower.
        assert_eq!(positions[1], GridPoint::new(1, 0));
    }

    /// The early exit is **output-identical**, which is the whole claim.
    ///
    /// The fixture converges inside sweep 1, so sweep 2 is the one that finds
    /// nothing and stops — and asking for 200 sweeps yields the same map as
    /// asking for 3, because the induction says every sweep after the first
    /// no-op is itself a no-op.
    #[test]
    fn the_search_stops_once_an_iteration_moves_nothing() {
        let network = crate::layout::cost::tests::sample();

        let long = crate::run_layout(&network, &LayoutParams::default());
        let short = crate::run_layout(&network, &params(3, 3));

        assert_eq!(
            long.positions(),
            short.positions(),
            "the sweeps after convergence changed the map"
        );
        assert_eq!(executed_sweeps(&network, &LayoutParams::default()), 2);
    }

    /// **The natural wrong version, and the only thing that catches it.**
    ///
    /// Exiting when the *station* sweep moved nothing — ignoring the cluster
    /// pass — is invisible in every measurement above: with both passes on, the
    /// fixture is bit-identical at 1, 2, 3, 10 and 200 iterations, so the wrong
    /// rule and the right one stop at the same map.
    ///
    /// Re-entering the search from the layout the single-station search settles
    /// at separates them. There, by construction, sweep 1's station pass moves
    /// nothing — that layout is its fixed point at this radius — while the
    /// cluster pass moves `parkview`/`lakeside`. The wrong rule executes one
    /// sweep; the right one keeps going.
    #[test]
    fn the_exit_test_is_the_whole_iterations_and_not_the_station_sweeps() {
        let network = crate::layout::cost::tests::sample();
        let single_station = crate::run_layout(
            &network,
            &LayoutParams {
                cluster_moves: false,
                ..LayoutParams::default()
            },
        );

        let mut positions = single_station.positions().to_vec();
        let mut occupancy = GridOccupancy::new();
        for (station, cell) in positions.iter().enumerate() {
            occupancy.claim(station, *cell);
        }

        let executed = run(
            &network,
            &mut positions,
            &mut occupancy,
            single_station.target_edge_cells(),
            &LayoutParams::default(),
        );

        assert_ne!(
            positions.as_slice(),
            single_station.positions(),
            "the cluster pass found nothing to do, so this proves nothing"
        );
        assert!(
            executed > 1,
            "the search exited after one sweep, on the strength of a station \
             pass that moved nothing while the cluster pass moved something"
        );
    }

    /// How many sweeps `run_layout` really takes on a network.
    fn executed_sweeps(network: &crate::model::Network, params: &LayoutParams) -> u32 {
        let layout = crate::run_layout(
            network,
            &LayoutParams {
                iterations: 0,
                ..params.clone()
            },
        );
        let mut positions = layout.positions().to_vec();
        let mut occupancy = GridOccupancy::new();
        for (station, cell) in positions.iter().enumerate() {
            occupancy.claim(station, *cell);
        }
        run(
            network,
            &mut positions,
            &mut occupancy,
            layout.target_edge_cells(),
            params,
        )
    }
}
