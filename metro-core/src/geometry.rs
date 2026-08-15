//! Plane geometry.
//!
//! Phase 1 carries `Point2` and nothing else. Segment intersection and angle
//! math arrive with Phase 2, where `c1` and `c5` are their first consumers and
//! their first tests.

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
