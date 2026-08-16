//! The fixture feed, and what a person counted from it.
//!
//! Every literal here was counted by hand from the six CSVs and not recomputed
//! by the code under test — a test that derives its expected value from the
//! implementation asserts only that the code agrees with itself.
//!
//! ## What the fixture is engineered to hold
//!
//! The spec's OQ-5 fixes the feed's properties **before** it is written, at
//! Phase 1, including the ones Phases 2 and 3 need. That is the whole point of
//! the question: extending the feed later moves every literal keyed to it, and
//! `llk-001`'s own OQ-5 had to re-author a fixture at Phase 5 for exactly that
//! reason. So the feed already carries:
//!
//! | property | rows | the phase that cannot be gated without it |
//! |---|---|---|
//! | split platforms under one parent, on two routes | `CEN` + `CEN_1`/`CEN_2` | 2 |
//! | an **emitted** station between a parent row and its first platform | `CEN`, `WST`, `CEN_1` | 2 |
//! | a platform with no `parent_station`, on a kept route | `WST` | 2 |
//! | a route collapsing to fewer than two stations | `M4` over `MKT_1`/`MKT_2` | 2 |
//! | a trip serving two platforms of one station consecutively | `M3` over `OLD_1`/`OLD_2` | 2 |
//! | a route whose longest trip is not its modal one | `M5`: 3×P (4 stops) vs 1×Q (6) | 3 |
//! | a route with exactly one trip | `M1` | 3 |
//! | two patterns of a route tied on trip count | `M6`: 2×R, 2×S | 3 |
//! | a route of a filtered-out `route_type` | `B1`, type 3 | 1 |
//! | `stop_times` rows out of `stop_sequence` order, values 10/20/30 | `M2_t1` | 1 |
//! | a `location_type = 2` row with no coordinates | `CEN_E1` | 1 |
//! | exactly one kept route with no `route_color`, and one stating `FFFFFF` | `M3`, `M5` | 1 |
//!
//! The interleave is the subtle one. Under the natural grouped layout
//! `CEN, CEN_1, CEN_2, WST` the parent-row rule and the first-platform-row rule
//! emit the identical array, so Phase 2's assertion on §2.5's emission position
//! would pass for an implementation doing the opposite. Interleaved, the two
//! readings give `[WST, CEN, …]` against `[CEN, WST, …]`.
//!
//! Two rows exist only to be **absent** from the output: `DEP`, which only the
//! filtered-out `B1` serves, and `CEN_E1`, the entrance.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use llika_gtfs::{ImportParams, ImportReport};
use llika_core::InputSchema;

/// The unpacked feed. `.zip` handling is asserted in `byte_stability.rs`, which
/// builds the archive from these same files.
pub fn feed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/feed")
}

pub fn import_fixture() -> (InputSchema, ImportReport) {
    llika_gtfs::import(&feed_dir(), &ImportParams::default())
        .unwrap_or_else(|e| panic!("the fixture feed imports: {e}"))
}

/// The station ids of one line, by route id.
pub fn line_stations<'a>(schema: &'a InputSchema, id: &str) -> &'a [String] {
    &schema
        .lines
        .iter()
        .find(|line| line.id == id)
        .unwrap_or_else(|| panic!("line `{id}` is in the import"))
        .stations
}

pub fn line_color<'a>(schema: &'a InputSchema, id: &str) -> &'a str {
    &schema
        .lines
        .iter()
        .find(|line| line.id == id)
        .unwrap_or_else(|| panic!("line `{id}` is in the import"))
        .color
}

/// Hand-counted from `stops.txt`: the fourteen rows some kept line references.
/// The five that are not here are `CEN`, `OLD` and `MKT` (parents, which emit
/// nothing of their own account), `CEN_E1` (an entrance) and `DEP` (served only
/// by the filtered-out `B1`).
///
/// **Phase 2's gate 2 consumes this number**, which is why it is written down
/// here rather than derived there: after the collapse nothing can produce a
/// pre-collapse import, so the "before" has to carry forward.
pub const FIXTURE_STATIONS: usize = 14;

/// Hand-counted from `routes.txt`: seven routes, of which `B1` is a bus.
///
/// **Also consumed by Phase 2's gate 2** — it drops to five there, because
/// `M4`'s two platforms are one station after the collapse and the route is no
/// longer a line.
pub const FIXTURE_LINES: usize = 6;
pub const FIXTURE_ROUTES_SEEN: usize = 7;
pub const FIXTURE_STOPS_SEEN: usize = 19;
