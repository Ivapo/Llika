//! Where a station may move, and which of those moves are legal.
//!
//! Two jobs. [`spiral_offsets`] enumerates the cells the search will try, in the
//! one order that makes "the earlier cell wins" a rule rather than an accident.
//! [`is_valid_move`] applies the three hard rejections, which exist to keep the
//! network from tearing — everything else about a move is left to the soft cost.
//!
//! **Ordinary crossings are not rejected here.** They are left entirely to the
//! `c1` penalty, which is the heaviest term in the cost function. The two wrong
//! answers are not comparable: reading the rule as a hard rejection when it is
//! soft freezes the layout early with edges it was never allowed to improve
//! through, while reading it as soft when it is hard costs a map with a crossing
//! `c1` is already pushing out. Only one of them can freeze the search.

use crate::geometry::segments_overlap;
use crate::grid::{GridOccupancy, GridPoint, ring};
use crate::model::Network;

use super::cost;

/// Every cell within `radius` rings of the origin, as offsets, in spiral order:
/// increasing Chebyshev ring, and within a ring increasing angle from due east.
///
/// The station's own cell is not among them — `ring(0)` is empty — so a move is
/// always a move. A radius below 1 offers nothing.
///
/// This *is* the order the snap's tie-break uses, which is what makes "when two
/// candidates lower `t` by the same amount, the earlier one wins" well defined
/// rather than a reference to whichever enumeration happened to reach it first.
pub(super) fn spiral_offsets(radius: u32) -> Vec<(i64, i64)> {
    (1..=radius).flat_map(|k| ring(i64::from(k))).collect()
}

/// Whether `station` may move to `to`, against all three of the hard rejections.
///
/// **Takes `positions` mutably and restores it.** Two of the three rules are
/// properties of the map *after* the move, and a search that clones the position
/// vector once per candidate would allocate tens of thousands of times on a
/// small network. The slice is bit-identical on return.
///
/// Precondition: `positions[station]` holds the station's **current** cell, since
/// the before-move edge order is read from it.
pub(super) fn is_valid_move(
    network: &Network,
    positions: &mut [GridPoint],
    occupancy: &GridOccupancy,
    station: usize,
    to: GridPoint,
) -> bool {
    if !occupancy.is_free(to) {
        return false;
    }

    let from = positions[station];
    let before = ordered_neighbours(network, positions, station);

    positions[station] = to;
    let preserved = is_rotation(&before, &ordered_neighbours(network, positions, station));
    let overlaps = overlaps_another_edge(network, positions, station);
    positions[station] = from;

    preserved && !overlaps
}

/// The station's neighbours in `c3`'s direction order — which is
/// *counter*-clockwise, since `j` increases north and the angle ascends from due
/// east. The spec words the rule as "clockwise order"; the handedness does not
/// matter to a rotation test, but the ordering is named rather than described so
/// nobody has to work that out twice.
///
/// Reused from [`cost::incident_directions`] rather than re-derived: that
/// function sorts, and the comparator *is* the tie-break rule.
fn ordered_neighbours(network: &Network, positions: &[GridPoint], station: usize) -> Vec<usize> {
    cost::incident_directions(network, positions, station)
        .into_iter()
        .map(|(_, neighbour)| neighbour)
        .collect()
}

/// Whether `after` is a cyclic rotation of `before`.
///
/// **Cyclic and not positional, deliberately.** "Clockwise order" is a property
/// of a cycle, and a station whose whole fan rotates by one position has not
/// torn anything — its edges still meet in the same rotational order, which is
/// the thing this rule protects. The positional reading rejects 4.6× as many
/// moves for no stated reason.
///
/// It follows that the rule is **vacuous below degree 3**: one neighbour has no
/// order, and with two, every sequence is a rotation of every other. That is
/// correct — you cannot flip a cycle of two — and it means the rule constrains
/// junctions only.
///
/// Neighbour indices are distinct, because two lines over one consecutive pair
/// share a single corridor, so the shift below is unambiguous.
fn is_rotation(before: &[usize], after: &[usize]) -> bool {
    if before.len() != after.len() {
        return false;
    }
    let Some(first) = after.first() else {
        return true;
    };
    let Some(shift) = before.iter().position(|n| n == first) else {
        return false;
    };
    (0..before.len()).all(|k| before[(shift + k) % before.len()] == after[k])
}

/// Whether any edge at `station` now lies **along** another edge — collinear and
/// sharing more than a single point.
///
/// **The pair set is deliberately not `c1`'s.** Every edge incident to the moved
/// station is tested against every *other* edge in the graph, including ones
/// that share an endpoint with it. `c1` excludes endpoint-sharing pairs, and
/// that exclusion is exactly what makes this a separate rule worth having: a
/// line folding back so that one edge lies along its own neighbour is invisible
/// to `c1` by construction, and it is the only case that answers to "a
/// degenerate no penalty can distinguish from a legitimate drawing".
///
/// Mirroring `c1`'s pair set would make the rule fire only where `c1` is already
/// charging its weight — leaving the fold-back it exists for unrejected. The two
/// readings are 10× apart in rejections on the sample fixture.
///
/// Two loops over the same corridor list with opposite exclusion rules sit forty
/// lines apart in this crate, and the exclusion is the entire content of each —
/// so [`cost::c1_crossings`] names this one back. Neither is the other with a
/// typo.
fn overlaps_another_edge(network: &Network, positions: &[GridPoint], station: usize) -> bool {
    let edges = cost::corridors(network);

    edges
        .iter()
        .enumerate()
        .filter(|(_, (a, b))| *a == station || *b == station)
        .any(|(n, (a, b))| {
            edges.iter().enumerate().any(|(m, (c, d))| {
                // Identity by **position in this list**, never by comparing the
                // endpoint pairs: `corridors` reports them in petgraph's
                // `(source, target)` order rather than normalised, so an edge
                // stored with the moved station second would not match itself.
                // An edge is trivially collinear with itself and overlaps
                // itself entirely, so a missed skip freezes the search outright.
                m != n
                    && segments_overlap(positions[*a], positions[*b], positions[*c], positions[*d])
            })
        })
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

    /// A three-armed junction — the lowest degree at which the order-flip rule
    /// is not vacuous.
    ///
    /// `hub` sits at the origin with neighbours due east, due north and due
    /// west, so the clockwise sequence before any move is `[east, north, west]`.
    fn junction() -> (Network, Vec<GridPoint>) {
        (
            net(
                &["hub", "east", "north", "west"],
                &[("a", &["east", "hub", "west"]), ("b", &["north", "hub"])],
            ),
            at(&[(0, 0), (2, 0), (0, 2), (-2, 0)]),
        )
    }

    /// Rejection 1 — the target cell is taken.
    #[test]
    fn a_move_onto_an_occupied_cell_is_rejected() {
        let (network, mut positions) = junction();
        let occupancy = occupied(&positions);

        // `north` is sitting there.
        assert!(!is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            0,
            GridPoint::new(0, 2)
        ));
        // The same move to the cell beside it is fine, so the rejection is the
        // occupancy and not the geometry.
        assert!(is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            0,
            GridPoint::new(1, 2)
        ));
    }

    /// Rejection 2 — the move flips the clockwise order of the station's edges.
    ///
    /// Hand-built rather than taken from the sample fixture, which also supplies
    /// flipping candidates at all four of its junctions: here the flip is
    /// legible from the coordinates. Moving `hub` north *past* its northern
    /// neighbour turns `[east, north, west]` into `[west, north, east]`, which
    /// is a reversal and not a rotation.
    #[test]
    fn an_order_flipping_move_at_a_junction_is_rejected() {
        let (network, mut positions) = junction();
        let occupancy = occupied(&positions);

        assert_eq!(ordered_neighbours(&network, &positions, 0), vec![1, 2, 3]);

        let past_the_north = GridPoint::new(0, 3);
        positions[0] = past_the_north;
        assert_eq!(
            ordered_neighbours(&network, &positions, 0),
            vec![3, 2, 1],
            "the fan reverses rather than rotating"
        );
        positions[0] = GridPoint::new(0, 0);

        assert!(!is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            0,
            past_the_north
        ));

        // A move that merely rotates the fan is allowed: shifting `hub` one
        // north gives `[north, west, east]`, a rotation of `[east, north,
        // west]`.
        let one_north = GridPoint::new(0, 1);
        positions[0] = one_north;
        assert_eq!(ordered_neighbours(&network, &positions, 0), vec![2, 3, 1]);
        positions[0] = GridPoint::new(0, 0);

        assert!(is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            0,
            one_north
        ));
    }

    /// The rule constrains junctions only, which is stated rather than left to
    /// be discovered.
    #[test]
    fn the_order_rule_is_vacuous_below_degree_three() {
        assert!(is_rotation(&[], &[]));
        assert!(is_rotation(&[4], &[4]));
        assert!(is_rotation(&[4, 9], &[9, 4]));

        assert!(is_rotation(&[1, 2, 3], &[3, 1, 2]));
        assert!(!is_rotation(&[1, 2, 3], &[3, 2, 1]));
    }

    /// Rejection 3 — the move makes one edge lie **along** another.
    ///
    /// The fold-back must share an endpoint, because that is precisely the case
    /// `c1` cannot see: it excludes endpoint-sharing pairs by construction, so
    /// this is the only thing standing between the layout and a line doubled
    /// back on itself.
    #[test]
    fn a_fold_back_onto_a_neighbouring_edge_is_rejected() {
        // A straight run `a — s — b`, with `s` free to move.
        let network = net(&["a", "s", "b"], &[("l", &["a", "s", "b"])]);
        let mut positions = at(&[(0, 0), (1, 1), (2, 2)]);
        let occupancy = occupied(&positions);

        // Moving `s` beyond `b` folds `s—b` back along `a—s`.
        assert!(!is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            1,
            GridPoint::new(3, 3)
        ));
    }

    /// **The load-bearing negative test.** A move leaving a station's two edges
    /// exactly collinear through it is a legitimate straight-through, and must
    /// not be rejected.
    ///
    /// Every positive test above passes under a `segments_intersect`
    /// implementation — that predicate is *closed*, so it counts the touching
    /// endpoint two collinear edges share. This is the one assertion that fails
    /// it deterministically rather than by luck of fixture shape, and it fails
    /// it on the move this layout most wants to make.
    #[test]
    fn a_move_into_a_collinear_straight_through_is_not_rejected() {
        // `a — s — b` with a kink at `s`.
        let network = net(&["a", "s", "b"], &[("l", &["a", "s", "b"])]);
        let mut positions = at(&[(0, 0), (1, 0), (2, 2)]);
        let occupancy = occupied(&positions);

        // Straightening it puts `a—s` and `s—b` on one line through `s`.
        let straight = GridPoint::new(1, 1);
        assert!(is_valid_move(
            &network,
            &mut positions,
            &occupancy,
            1,
            straight
        ));

        // And the reason it is not rejected: the two edges meet at exactly one
        // point, which `segments_overlap` does not count and the closed
        // intersection test does.
        assert!(!segments_overlap(
            GridPoint::new(0, 0),
            straight,
            straight,
            GridPoint::new(2, 2)
        ));
    }

    #[test]
    fn the_candidate_set_is_the_rings_in_spiral_order() {
        assert!(spiral_offsets(0).is_empty());
        assert_eq!(spiral_offsets(1), ring(1));
        // Ring 1 then ring 2, each in its own order — 8 + 16 cells.
        assert_eq!(spiral_offsets(2).len(), 24);
        assert_eq!(spiral_offsets(2)[0], (1, 0));
        assert_eq!(spiral_offsets(2)[8], (2, 0));
        // The station's own cell is never a candidate.
        assert!(!spiral_offsets(3).contains(&(0, 0)));
    }
}
