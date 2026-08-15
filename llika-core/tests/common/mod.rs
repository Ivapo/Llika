//! Fixture loading and the literals the Phase 1 gate is keyed to.
//!
//! Every number here was derived from the fixture by an **independent**
//! implementation of the spec's §2.2 — a throwaway Python script — and not by
//! the code under test. A test that recomputes its expected value with the code
//! it is testing asserts only that the code agrees with itself.

#![allow(dead_code)]

use std::path::PathBuf;

use llika_core::{InputSchema, LayoutParams, parse_input};

/// Hand-counted from the JSON: Red contributes 7 consecutive pairs, Green 8 and
/// Blue 6 — 21 in all — and the four trunk pairs (`riverside`-`oldtown`,
/// `oldtown`-`eastbank`, `eastbank`-`central`, `central`-`market`) appear in
/// both Red and Green, so 21 - 4 = 17 corridors.
pub const SAMPLE_EDGES: usize = 17;
pub const SAMPLE_STATIONS: usize = 17;
pub const SAMPLE_LINES: usize = 3;

/// The derived `g` for the fixture, in metres: the 9th of its 17 sorted edge
/// lengths, which is the `riverside` → `hillcrest` corridor.
pub const SAMPLE_GRID_SPACING_M: f64 = 2269.9117477523614;

/// `crossing.json`'s crossings at the snap, hand-counted from the cells its
/// coordinates were designed to land on.
///
/// Coast runs along `j = 0` through `(-1,0) (0,0) (1,0) (2,0)`. Ridge dips under
/// it: `northpoint (-1,1)` → `hollow (0,-1)` → `summit (1,1)`. So
/// `northpoint`-`hollow` meets `j = 0` at `i = -0.5`, strictly inside the coast
/// edge `westport`-`harbour`, and `hollow`-`summit` meets it at `i = 0.5`,
/// strictly inside `harbour`-`midtown`. No other pair meets at all, and the two
/// lines share no station, so `c1` counts both.
pub const CROSSING_C1_AT_SNAP: f64 = 2.0;

/// The share of `crossing.json`'s corridors within 5° of a multiple of 45 at the
/// snap: the three coast edges run due east, and ridge's two run at
/// `atan2(∓2, ±1)` — 63.4° off axis, 18.4° from the nearest diagonal. 3 of 5.
///
/// Unlike the sample fixture, whose corridors are all octilinear before the
/// search touches them, this one leaves the octilinearity claim something to
/// prove.
pub const CROSSING_OCTILINEAR_AT_SNAP: f64 = 0.6;

/// Parameters that project and snap and then stop.
///
/// Phase 1's assertions are about the projection, the snap and the viewport,
/// and the search would perturb them without saying anything about what they
/// assert — a snap-order tie-break checked against a layout the search has since
/// moved is a coincidence rather than a test. This is also the baseline the
/// search's own gate measures against.
pub fn snap_only() -> LayoutParams {
    LayoutParams {
        iterations: 0,
        ..LayoutParams::default()
    }
}

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

pub fn load(name: &str) -> InputSchema {
    let text = std::fs::read_to_string(fixture_path(name))
        .unwrap_or_else(|e| panic!("fixture {name} is readable: {e}"));
    parse_input(&text).unwrap_or_else(|e| panic!("fixture {name} parses: {e}"))
}

pub fn sample() -> InputSchema {
    load("sample_network.json")
}

/// Relative comparison, for values that come out of `cos` and `sqrt` and will
/// not survive exact `f64` equality against a decimal computed elsewhere.
pub fn close(actual: f64, expected: f64, relative_tolerance: f64) -> bool {
    (actual - expected).abs() <= relative_tolerance * expected.abs()
}
