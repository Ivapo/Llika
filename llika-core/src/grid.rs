//! The integer grid: deriving its spacing, and snapping stations onto it.
//!
//! The grid holds **at most one station per cell**, or the occupancy checks the
//! layout search will lean on mean nothing. Two stations can round to the same
//! cell, so the collision has a deterministic rule: stations claim cells in the
//! order they appear in the input file, first claim wins, and a station that
//! finds its cell taken spirals out to the first free one.

use std::collections::HashMap;

use crate::geometry::Point2;

/// `g` where the network has no non-degenerate edge to derive it from.
///
/// This constant escapes the argument against constant grid spacings — that no
/// single spacing suits every network's typical edge — because a network with
/// no non-degenerate edge has no typical edge for a constant to be wrong about.
/// The only thing `g` still does there is keep isolated stations in distinct
/// cells, and any positive value does that.
pub const FALLBACK_GRID_SPACING_M: f64 = 500.0;

/// A cell of the integer grid. `j` increases **north**, like the projected
/// plane and unlike SVG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GridPoint {
    pub i: i64,
    pub j: i64,
}

impl GridPoint {
    pub const fn new(i: i64, j: i64) -> Self {
        Self { i, j }
    }

    /// The cell offset by `(di, dj)`.
    pub const fn offset(self, di: i64, dj: i64) -> Self {
        Self {
            i: self.i + di,
            j: self.j + dj,
        }
    }
}

/// The cell a projected point rounds to, **before** any collision is resolved.
///
/// Public because the difference between this and the post-tie-break position
/// is the only observable proof that a collision happened at all.
pub fn raw_cell(p: Point2, g: f64) -> GridPoint {
    GridPoint::new((p.x / g).round() as i64, (p.y / g).round() as i64)
}

/// The lower-middle median: the lower of the two middle values on an even
/// count, so no float averaging enters a value everything else is keyed to.
pub fn median_lower(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let index = if n.is_multiple_of(2) {
        n / 2 - 1
    } else {
        n / 2
    };
    Some(sorted[index])
}

/// The default `g`: the lower-middle median of the **non-zero-length** edges,
/// or [`FALLBACK_GRID_SPACING_M`] where that set is empty.
///
/// Zero-length edges are excluded and the empty case has an answer because both
/// are reachable: a station referenced by no line is legal, so a network can
/// have no edges at all, and two stations at identical coordinates give a
/// degenerate one. The median of an empty set is undefined and a zero median
/// makes `round(x / g)` a `NaN` that casts silently to cell 0.
pub fn derive_grid_spacing(edge_lengths: &[f64]) -> f64 {
    let non_zero: Vec<f64> = edge_lengths
        .iter()
        .copied()
        .filter(|len| *len > 0.0)
        .collect();
    median_lower(&non_zero).unwrap_or(FALLBACK_GRID_SPACING_M)
}

/// The cells at Chebyshev distance exactly `k` from the origin, ordered by
/// increasing angle from due east.
///
/// No two cells of one ring lie on the same ray — two proportional offsets with
/// the same Chebyshev norm are the same offset — so the angle is a total order
/// and the ring needs no further tie rule.
pub fn ring(k: i64) -> Vec<(i64, i64)> {
    if k <= 0 {
        return Vec::new();
    }
    let mut cells: Vec<(i64, i64)> = Vec::new();
    for di in -k..=k {
        for dj in -k..=k {
            if di.abs().max(dj.abs()) == k {
                cells.push((di, dj));
            }
        }
    }
    cells.sort_by(|a, b| angle_from_east(*a).total_cmp(&angle_from_east(*b)));
    cells
}

/// `atan2` normalised into `[0, 2π)`, so due east sorts first and the order runs
/// counter-clockwise from there.
fn angle_from_east((di, dj): (i64, i64)) -> f64 {
    let angle = (dj as f64).atan2(di as f64);
    if angle < 0.0 {
        angle + std::f64::consts::TAU
    } else {
        angle
    }
}

/// Which station holds which cell.
///
/// **`by_cell` is queried by key and never iterated**, which is what lets it be
/// a `HashMap` without making the output vary between processes: Rust seeds its
/// default hasher per process. The search carries one of these from the snap all
/// the way through, so a `for (cell, station) in &self.by_cell` added later
/// would be a determinism bug rather than a style choice.
#[derive(Debug, Clone, Default)]
pub struct GridOccupancy {
    by_cell: HashMap<GridPoint, usize>,
}

impl GridOccupancy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_free(&self, cell: GridPoint) -> bool {
        !self.by_cell.contains_key(&cell)
    }

    pub fn station_at(&self, cell: GridPoint) -> Option<usize> {
        self.by_cell.get(&cell).copied()
    }

    pub fn len(&self) -> usize {
        self.by_cell.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_cell.is_empty()
    }

    /// Move `station` from one cell to another, which is what the layout
    /// search does on every accepted move.
    ///
    /// The counterpart to [`claim`](Self::claim), and the reason this type
    /// survives the snap rather than being built and dropped inside
    /// [`snap_to_grid`]: a search that kept its own free-cell index would be a
    /// second structure answering "which cells are taken", which is how the two
    /// come to disagree.
    ///
    /// Both preconditions are the caller's to hold — the search checks
    /// [`is_free`](Self::is_free) before it scores a candidate at all — so they
    /// are debug assertions rather than a `Result` nothing would ever handle.
    pub fn relocate(&mut self, station: usize, from: GridPoint, to: GridPoint) {
        debug_assert_eq!(
            self.by_cell.get(&from),
            Some(&station),
            "relocating station {station} from a cell it does not hold"
        );
        debug_assert!(
            to == from || self.is_free(to),
            "relocating station {station} onto an occupied cell"
        );
        self.by_cell.remove(&from);
        self.by_cell.insert(to, station);
    }

    /// Claim `preferred` for `station`, or the first free cell in spiral order
    /// around it. Returns the cell actually claimed.
    ///
    /// Terminates: rings grow without bound and occupancy is finite, so some
    /// ring always contains a free cell.
    pub fn claim(&mut self, station: usize, preferred: GridPoint) -> GridPoint {
        if self.is_free(preferred) {
            self.by_cell.insert(preferred, station);
            return preferred;
        }
        let mut k = 1;
        loop {
            for (di, dj) in ring(k) {
                let candidate = preferred.offset(di, dj);
                if self.is_free(candidate) {
                    self.by_cell.insert(candidate, station);
                    return candidate;
                }
            }
            k += 1;
        }
    }
}

/// Snap projected points onto the grid, in input order, one station per cell.
///
/// Returns the occupancy alongside the positions rather than dropping it: the
/// layout search moves stations between cells and needs the same structure that
/// decided where they started. See [`GridOccupancy::relocate`].
pub fn snap_to_grid(points: &[Point2], g: f64) -> (Vec<GridPoint>, GridOccupancy) {
    let mut occupancy = GridOccupancy::new();
    let positions = points
        .iter()
        .enumerate()
        .map(|(station, p)| occupancy.claim(station, raw_cell(*p, g)))
        .collect();
    (positions, occupancy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_an_even_count_is_the_lower_middle_value() {
        // Lower middle is 3.0; the mean of the two middles would be 3.5, and
        // that reflex implementation passes every other Phase 1 assertion.
        let lengths = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        assert_eq!(median_lower(&lengths), Some(3.0));

        // Unsorted input, and a pair whose mean is not either value.
        let lengths = [900.0, 100.0, 400.0, 200.0];
        assert_eq!(median_lower(&lengths), Some(200.0));
    }

    #[test]
    fn median_of_an_odd_count_is_the_middle_value() {
        assert_eq!(median_lower(&[5.0, 1.0, 3.0]), Some(3.0));
    }

    #[test]
    fn median_of_nothing_is_nothing() {
        assert_eq!(median_lower(&[]), None);
    }

    #[test]
    fn spacing_ignores_zero_length_edges_and_falls_back_when_none_remain() {
        assert_eq!(derive_grid_spacing(&[0.0, 0.0]), FALLBACK_GRID_SPACING_M);
        assert_eq!(derive_grid_spacing(&[]), FALLBACK_GRID_SPACING_M);
        // 0.0 dropped, leaving [10, 20, 30]: the median is 20, not 10.
        assert_eq!(derive_grid_spacing(&[30.0, 0.0, 10.0, 20.0]), 20.0);
    }

    #[test]
    fn the_first_ring_starts_due_east_and_runs_counter_clockwise() {
        assert_eq!(
            ring(1),
            vec![
                (1, 0),
                (1, 1),
                (0, 1),
                (-1, 1),
                (-1, 0),
                (-1, -1),
                (0, -1),
                (1, -1),
            ]
        );
        assert_eq!(ring(2).len(), 16);
        assert_eq!(ring(2)[0], (2, 0));
        assert!(ring(0).is_empty());
    }

    #[test]
    fn a_displaced_station_takes_the_first_free_cell_in_spiral_order() {
        let mut occupancy = GridOccupancy::new();
        let cell = GridPoint::new(4, 7);
        assert_eq!(occupancy.claim(0, cell), cell);
        // Due east is the first cell of ring 1.
        assert_eq!(occupancy.claim(1, cell), GridPoint::new(5, 7));
        // Then north-east.
        assert_eq!(occupancy.claim(2, cell), GridPoint::new(5, 8));
    }

    #[test]
    fn relocating_frees_the_old_cell_and_takes_the_new_one() {
        let mut occupancy = GridOccupancy::new();
        let (from, to) = (GridPoint::new(0, 0), GridPoint::new(3, -2));
        occupancy.claim(7, from);

        occupancy.relocate(7, from, to);

        assert!(occupancy.is_free(from), "the vacated cell is free again");
        assert_eq!(occupancy.station_at(to), Some(7));
        // One station in, one station out — a relocation is not a claim.
        assert_eq!(occupancy.len(), 1);
    }
}
