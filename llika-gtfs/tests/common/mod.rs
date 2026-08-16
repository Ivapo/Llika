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
//! emit the identical array, so the assertion on §2.5's emission position would
//! pass for an implementation doing the opposite. Interleaved, the two readings
//! give `[WST, CEN, …]` against `[CEN, WST, …]`.
//!
//! Note what that costs a gate written against it: `OLD` and `MKT` **are** laid
//! out grouped, so both readings put them in the same slot. The two arrays
//! differ only at indices 0 and 1, and only an assertion on the order of `CEN`
//! against `WST` discriminates — membership, the station count, and the position
//! of any other station all pass under the wrong reading.
//!
//! Two rows exist only to be **absent** from the output: `DEP`, which only the
//! filtered-out `B1` serves, and `CEN_E1`, the entrance.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use llika_gtfs::{ImportParams, ImportReport};
use llika_core::InputSchema;

/// The unpacked feed. `.zip` handling is asserted in `byte_stability.rs`, which
/// builds the archive from these same files.
pub fn feed_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/feed")
}

/// A scratch directory per test. The names are fixed rather than unique, so two
/// tests sharing one would race; each is removed before it is created so a run
/// never inherits the last one's files.
pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

/// Run the real binary and hand back everything it did — status, stdout and
/// stderr — so a caller can assert on the one it is about.
///
/// Raw rather than success-asserting because two gates want different halves of
/// it: byte stability wants the bytes on disk, and OQ-3's answer is an exit
/// status that a helper unwrapping it would have hidden.
pub fn run_importer(input: &Path, output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_llika-gtfs"))
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .output()
        .expect("the llika-gtfs binary runs")
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

/// Hand-counted from `stops.txt`: the eleven stations some surviving line
/// references, once platforms carry their parent's identity.
///
/// The eight rows that are not here are `CEN_1`, `CEN_2`, `OLD_1`, `OLD_2`,
/// `MKT_1` and `MKT_2` (platforms, whose identity is now the parent's),
/// `CEN_E1` (an entrance) and `DEP` (served only by the filtered-out `B1`).
///
/// **Phase 3 does not move it, and that is counted rather than assumed.** `M5`
/// stops drawing `CEN` and `FAI` when it switches to its modal pattern, but
/// `M1`, `M2` and `M3` reference `CEN` and `M6` references `FAI`, so every one
/// of the eleven is still referenced by something. What Phase 3 moves is the
/// *graph*: `collapse.rs` counts one corridor and one degree fewer at `CEN`.
pub const FIXTURE_STATIONS: usize = 11;

/// Hand-counted from `routes.txt`: seven routes, less `B1` the bus and less
/// `M4`, whose two platforms are one station after the collapse.
pub const FIXTURE_LINES: usize = 5;

/// The routes that **matched the `route_type` filter** — every route but `B1`.
///
/// Not the same as [`FIXTURE_LINES`] since the collapse: a matched route can
/// still fail to become a line, and `M4` does.
pub const FIXTURE_ROUTES_KEPT: usize = 6;

/// The matched routes that did not become lines: `M4` alone.
pub const FIXTURE_ROUTES_DROPPED: usize = 1;

pub const FIXTURE_ROUTES_SEEN: usize = 7;
pub const FIXTURE_STOPS_SEEN: usize = 19;

/// Phase 1's committed counts, kept as the "before" its successor is measured
/// against.
///
/// **They carry forward as literals rather than as a re-run**, and they have to:
/// §2.2 makes the collapse neither an option nor a flag, so after Phase 2
/// nothing in this crate can produce a pre-collapse import, and adding a flag to
/// make the comparison runnable is exactly what that section forbids.
///
/// The two carry differently, which is deliberate. The **line** count is an
/// exact delta — `M4` is the only route that can drop, so it is `FIXTURE_LINES +
/// 1`. The **station** count carries only as a relation, strictly above, because
/// its exact delta is not derivable from the spec.
pub const PHASE_1_STATIONS: usize = 14;
pub const PHASE_1_LINES: usize = 6;
