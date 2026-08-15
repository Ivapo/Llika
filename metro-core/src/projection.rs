//! Latitude/longitude to the projected plane.
//!
//! A hand-rolled equirectangular projection centred on the network's centroid.
//! At single-city scale the distortion is below what the layout step destroys
//! anyway, so a projection crate would buy accuracy this pipeline immediately
//! discards.

use crate::geometry::Point2;
use crate::model::Station;

/// Metres per degree of latitude, and of longitude at the equator.
pub const METRES_PER_DEGREE: f64 = 111_320.0;

/// The projection's origin: the centroid the plane is centred on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projector {
    pub lat_c: f64,
    pub lon_c: f64,
}

impl Projector {
    /// Centroid as the arithmetic mean of the station latitudes and
    /// longitudes, summed **in input order** so the result is bit-reproducible.
    ///
    /// An empty station list has no centroid; it takes `(0, 0)` rather than the
    /// `NaN` the mean would give. The input schema permits zero stations —
    /// §2.1's error contract does not reject it — and a `NaN` origin would
    /// propagate silently all the way to cell 0.
    pub fn from_stations(stations: &[Station]) -> Self {
        if stations.is_empty() {
            return Self {
                lat_c: 0.0,
                lon_c: 0.0,
            };
        }
        let n = stations.len() as f64;
        let lat_c = stations.iter().map(|s| s.lat).sum::<f64>() / n;
        let lon_c = stations.iter().map(|s| s.lon).sum::<f64>() / n;
        Self { lat_c, lon_c }
    }

    /// Project one coordinate. Output is **metres**, `x` east and `y` north.
    ///
    /// The `cos(lat_c)` on `x` is what makes a degree of longitude shorter than
    /// a degree of latitude away from the equator; dropping it inflates the
    /// east-west span by 27% at 38°N, which is inside every bound the gate
    /// checks and only a hand-computed distance catches.
    pub fn project(&self, lat: f64, lon: f64) -> Point2 {
        Point2::new(
            (lon - self.lon_c) * METRES_PER_DEGREE * self.lat_c.to_radians().cos(),
            (lat - self.lat_c) * METRES_PER_DEGREE,
        )
    }

    /// Project a station list, preserving order.
    pub fn project_all(&self, stations: &[Station]) -> Vec<Point2> {
        stations
            .iter()
            .map(|s| self.project(s.lat, s.lon))
            .collect()
    }
}
