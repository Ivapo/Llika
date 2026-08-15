//! Plane geometry.
//!
//! Two domains, deliberately separate. [`Point2`] is the projected metre plane,
//! and nothing here scores it. Everything else operates on [`GridPoint`] —
//! integer cells — because that is the domain the cost criteria are defined
//! over: the layout search moves stations between cells, so a criterion scored
//! in metres would be scoring a different object from the one the search moves.
//!
//! The integer domain is also what makes the criteria *exact*. Crossings come
//! out of sign tests on integer cross products, with no epsilon anywhere, and
//! `atan2` of an exact integer ratio returns an exact multiple of `FRAC_PI_4` at
//! the eight octilinear directions — which is the property the four-gonality
//! gate rests on.

use std::f64::consts::{FRAC_PI_4, PI, TAU};

use crate::grid::GridPoint;

/// A point in the projected plane: metres east and north of the network
/// centroid. `y` increases **north**, which is the opposite of SVG's `y`; the
/// flip happens once, in [`crate::render::Viewport`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2 {
    pub x: f64,
    pub y: f64,
}

impl Point2 {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Euclidean distance in metres.
    pub fn distance(self, other: Self) -> f64 {
        (other.x - self.x).hypot(other.y - self.y)
    }
}

/// The distance between two cells, in cells.
pub fn cell_distance(a: GridPoint, b: GridPoint) -> f64 {
    (((b.i - a.i) as f64).powi(2) + ((b.j - a.j) as f64).powi(2)).sqrt()
}

/// Which way the ordered triple `a → b → c` turns: `1` counter-clockwise, `-1`
/// clockwise, `0` collinear.
///
/// Computed in `i128`. Cell coordinates are bounded by the network's extent
/// divided by `g`, so `i64` would do in practice — but `g` is a parameter a
/// caller supplies, and a widened product costs nothing to rule the overflow
/// out entirely rather than argue about its reachability.
pub fn orientation(a: GridPoint, b: GridPoint, c: GridPoint) -> i8 {
    let cross = (b.i as i128 - a.i as i128) * (c.j as i128 - a.j as i128)
        - (b.j as i128 - a.j as i128) * (c.i as i128 - a.i as i128);
    match cross {
        0 => 0,
        n if n > 0 => 1,
        _ => -1,
    }
}

/// Whether `p` lies on segment `a`-`b`, **given that the three are collinear**.
fn on_collinear_segment(a: GridPoint, b: GridPoint, p: GridPoint) -> bool {
    p.i >= a.i.min(b.i) && p.i <= a.i.max(b.i) && p.j >= a.j.min(b.j) && p.j <= a.j.max(b.j)
}

/// Whether two **closed** segments intersect.
///
/// Closed is deliberate, and it is what the crossing criterion counts: an
/// endpoint lying in the other segment's interior, and a collinear overlap,
/// both read as a crossing to the eye and both are reachable. Grid occupancy
/// keeps two *stations* out of one cell, but nothing stops an edge running over
/// a cell some other station occupies.
///
/// Exact — every comparison is on integers, so there is no tolerance to tune
/// and no near-miss to classify.
pub fn segments_intersect(a1: GridPoint, a2: GridPoint, b1: GridPoint, b2: GridPoint) -> bool {
    let (o1, o2) = (orientation(a1, a2, b1), orientation(a1, a2, b2));
    let (o3, o4) = (orientation(b1, b2, a1), orientation(b1, b2, a2));

    // The general case: each segment separates the other's endpoints.
    if o1 != o2 && o3 != o4 {
        return true;
    }

    // The collinear and touching cases the general test misses.
    (o1 == 0 && on_collinear_segment(a1, a2, b1))
        || (o2 == 0 && on_collinear_segment(a1, a2, b2))
        || (o3 == 0 && on_collinear_segment(b1, b2, a1))
        || (o4 == 0 && on_collinear_segment(b1, b2, a2))
}

/// The direction of `from → to`, in radians, normalised into `[0, 2π)`.
///
/// **Normalising is load-bearing, not cosmetic.** Rust's `%` keeps the sign of
/// the dividend, so a raw `atan2` in `(-π, π]` feeds a negative remainder into
/// [`octilinear_deviation`] and yields a *negative* cost for any edge pointing
/// into the lower half-plane. Normalising here is also what makes the eight
/// octilinear directions come out as `+0.0` rather than a mix of `+0.0` and
/// `-0.0`, and it is the order [`angular_gaps`] needs anyway.
pub fn direction(from: GridPoint, to: GridPoint) -> f64 {
    let theta = ((to.j - from.j) as f64).atan2((to.i - from.i) as f64);
    if theta < 0.0 { theta + TAU } else { theta }
}

/// How far `theta` is from the nearest multiple of 45 degrees, in radians.
///
/// Exactly `0.0` at the eight octilinear directions when `theta` came from
/// [`direction`] over an integer cell offset. That exactness is a property of
/// *this* formulation: `|sin 4θ|` is an equally faithful rendering of
/// "four-gonality" and returns values of order `1e-16` at seven of the eight.
pub fn octilinear_deviation(theta: f64) -> f64 {
    let r = theta % FRAC_PI_4;
    r.min(FRAC_PI_4 - r)
}

/// The angle between two directions, folded into `[0, π]`.
///
/// This is the **interior** angle where both directions are measured outward
/// from a shared station — never the turn angle. It is exactly `π` for two
/// collinear opposite offsets at any angle, octilinear or not, which is what
/// lets the straightness criterion reach exactly zero.
pub fn interior_angle(a: f64, b: f64) -> f64 {
    let d = (a - b).abs();
    if d > PI { TAU - d } else { d }
}

/// The gaps between consecutive directions around a point, in order, summing to
/// `2π`.
///
/// `sorted` must be ascending and within `[0, 2π)`; the last gap wraps from the
/// final direction back to the first. An empty input has no gaps, and a single
/// direction has one gap of `2π`.
pub fn angular_gaps(sorted: &[f64]) -> Vec<f64> {
    let Some((&first, _)) = sorted.split_first() else {
        return Vec::new();
    };
    let last = sorted[sorted.len() - 1];
    sorted
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .chain(std::iter::once(first + TAU - last))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_3};

    use super::*;

    fn cell(i: i64, j: i64) -> GridPoint {
        GridPoint::new(i, j)
    }

    /// The eight unit cell offsets, counter-clockwise from due east.
    const OCTILINEAR: [(i64, i64); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];

    #[test]
    fn a_direction_is_normalised_into_the_full_turn() {
        let origin = cell(0, 0);
        assert_eq!(direction(origin, cell(1, 0)), 0.0);
        assert_eq!(direction(origin, cell(0, 1)), FRAC_PI_2);
        assert_eq!(direction(origin, cell(-1, 0)), PI);
        // The half-turn that a raw atan2 would return as -π/2.
        assert_eq!(direction(origin, cell(0, -1)), PI + FRAC_PI_2);
        for theta in OCTILINEAR.map(|(i, j)| direction(origin, cell(i, j))) {
            assert!((0.0..TAU).contains(&theta), "{theta} is inside [0, 2π)");
        }
    }

    /// The property the four-gonality gate rests on. An equality assertion and
    /// never a bit comparison — see [`octilinear_deviation`].
    #[test]
    fn the_octilinear_directions_deviate_by_exactly_zero() {
        let origin = cell(0, 0);
        for (i, j) in OCTILINEAR {
            let d = octilinear_deviation(direction(origin, cell(i, j)));
            assert_eq!(d, 0.0, "offset ({i}, {j}) deviates by {d}");
        }
    }

    /// And a direction the grid admits that is *not* octilinear, so the
    /// criterion is shown to be capable of returning anything at all. 22.5° is
    /// not constructible on an integer grid — `tan 22.5° = 0.414…` — and the
    /// nearest offsets the grid admits are (2,1) at 26.565° and (12,5) at
    /// 22.620°.
    #[test]
    fn an_off_angle_direction_deviates_by_more_than_nothing() {
        let d = octilinear_deviation(direction(cell(0, 0), cell(2, 1)));
        assert!(d > 0.0);
        assert!((d - 0.3217505543966422).abs() < 1e-15, "{d}");
        // Negative-going offsets are the ones an unnormalised `%` breaks.
        for (i, j) in [(2, -1), (-2, -1), (-2, 1)] {
            let d = octilinear_deviation(direction(cell(0, 0), cell(i, j)));
            assert!(d > 0.0, "offset ({i}, {j}) gave {d}");
        }
    }

    #[test]
    fn orientation_is_the_sign_of_the_turn() {
        assert_eq!(orientation(cell(0, 0), cell(1, 0), cell(1, 1)), 1);
        assert_eq!(orientation(cell(0, 0), cell(1, 0), cell(1, -1)), -1);
        assert_eq!(orientation(cell(0, 0), cell(1, 0), cell(2, 0)), 0);
    }

    #[test]
    fn segments_that_properly_cross_intersect() {
        assert!(segments_intersect(
            cell(0, 0),
            cell(2, 2),
            cell(0, 2),
            cell(2, 0)
        ));
    }

    #[test]
    fn segments_that_pass_without_meeting_do_not() {
        assert!(!segments_intersect(
            cell(0, 0),
            cell(2, 0),
            cell(0, 1),
            cell(2, 1)
        ));
        // Collinear but disjoint.
        assert!(!segments_intersect(
            cell(0, 0),
            cell(1, 0),
            cell(2, 0),
            cell(3, 0)
        ));
    }

    /// The closed test: touching counts. An endpoint in the other's interior
    /// and a collinear overlap both read as a crossing to the eye.
    #[test]
    fn touching_and_overlapping_segments_intersect() {
        // An endpoint of one lying in the other's interior.
        assert!(segments_intersect(
            cell(0, 0),
            cell(4, 0),
            cell(2, 0),
            cell(2, 3)
        ));
        // A collinear overlap.
        assert!(segments_intersect(
            cell(0, 0),
            cell(3, 0),
            cell(2, 0),
            cell(5, 0)
        ));
        // Collinear, meeting at exactly one endpoint.
        assert!(segments_intersect(
            cell(0, 0),
            cell(2, 0),
            cell(2, 0),
            cell(4, 0)
        ));
    }

    /// Exactly `π` for collinear opposite offsets at **any** angle, which is
    /// what lets the straightness criterion reach exactly zero off-axis too.
    #[test]
    fn the_interior_angle_of_a_straight_through_is_exactly_pi() {
        let origin = cell(0, 0);
        for (a, b) in [
            ((1, 0), (-1, 0)),
            ((1, 1), (-1, -1)),
            ((0, 1), (0, -1)),
            ((2, 1), (-2, -1)),
            ((1, 2), (-3, -6)),
        ] {
            let phi = interior_angle(
                direction(origin, cell(a.0, a.1)),
                direction(origin, cell(b.0, b.1)),
            );
            assert_eq!(PI - phi, 0.0, "offsets {a:?} and {b:?} gave φ = {phi}");
        }
    }

    #[test]
    fn the_interior_angle_of_a_bend_is_the_smaller_side() {
        let origin = cell(0, 0);
        let phi = |a: (i64, i64), b: (i64, i64)| {
            interior_angle(
                direction(origin, cell(a.0, a.1)),
                direction(origin, cell(b.0, b.1)),
            )
        };
        assert_eq!(phi((1, 0), (0, 1)), FRAC_PI_2);
        assert_eq!(phi((1, 0), (1, 1)), FRAC_PI_4);
        // Reflex on one side is the same angle read from the other.
        assert_eq!(phi((1, 0), (1, -1)), FRAC_PI_4);
    }

    #[test]
    fn angular_gaps_wrap_and_sum_to_a_full_turn() {
        assert!(angular_gaps(&[]).is_empty());
        assert_eq!(angular_gaps(&[0.0]), vec![TAU]);

        let origin = cell(0, 0);
        let mut cross: Vec<f64> = [(1, 0), (0, 1), (-1, 0), (0, -1)]
            .map(|(i, j)| direction(origin, cell(i, j)))
            .to_vec();
        cross.sort_by(f64::total_cmp);
        assert_eq!(angular_gaps(&cross), vec![FRAC_PI_2; 4]);
    }

    /// The one identity a degree-3 station's floor is derived from, kept here
    /// because it is a geometry fact rather than a scoring one: 135/135/90 is
    /// the best an octilinear grid allows, and its cost is bit-exactly
    /// `FRAC_PI_3` — **not** `PI / 3.0`, which is one ulp away and fails.
    #[test]
    fn the_best_degree_three_spread_costs_frac_pi_3() {
        let origin = cell(0, 0);
        let mut directions: Vec<f64> = [(1, 0), (-1, 1), (0, -1)]
            .map(|(i, j)| direction(origin, cell(i, j)))
            .to_vec();
        directions.sort_by(f64::total_cmp);

        let ideal = TAU / 3.0;
        let cost: f64 = angular_gaps(&directions)
            .iter()
            .map(|gap| (gap - ideal).abs())
            .sum();

        assert_eq!(cost, FRAC_PI_3);
        assert_ne!(
            FRAC_PI_3,
            PI / 3.0,
            "the two spellings are not the same f64"
        );
    }
}
