//! Phase 5's gate, assertions 3 and 4: the reader retains only the rows of kept
//! trips, and a row belonging to no kept trip is discarded rather than an error.
//!
//! **The assertion is a count and not a memory bound**, which is a correction
//! rather than a convenience. The reader this phase replaced held ~180 MB RSS
//! over two million rows where the streaming one holds ~3.5 MB, and nothing in
//! `cargo test` bounds memory anywhere near 180 MB — so "the test completes"
//! passes for exactly the implementation this phase exists to remove. Making a
//! whole-read actually exhaust a runner needs ~100 M rows and a multi-gigabyte
//! file, which no suite accepts. The retained count discriminates cleanly at the
//! size below: 2,000,000 against 18,000.
//!
//! The count is observable through the public API — `lib.rs` re-exports `Feed`
//! and `stop_times` is a `pub` field — so nothing here instruments the crate to
//! watch it.
//!
//! **The large feed is generated here and never committed**, which is
//! `tests/fixtures/bart.md`'s rule inverted: that file holds a publisher's bytes
//! unmodified because a snapshot is evidence of what a publisher publishes, and
//! a synthetic blob is evidence of nothing. It must not sit in `fixtures/` where
//! a reader would take it for one.

mod common;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use common::{feed_dir, scratch};
use llika_gtfs::{Feed, ImportParams};

/// Trips of the kept metro route, and the rows each one runs.
const KEPT_TRIPS: usize = 6_000;
const ROWS_PER_TRIP: usize = 3;

/// What the reader must retain: 18,000 rows.
const KEPT_ROWS: usize = KEPT_TRIPS * ROWS_PER_TRIP;

/// What the file holds. The kept rows are 0.9% of it.
const TOTAL_ROWS: usize = 2_000_000;

/// A feed of all four tables — `Feed::read` returns `MissingTable` on anything
/// less, which `byte_stability.rs:an_incomplete_feed_fails_loudly` pins — whose
/// `stop_times.txt` is mostly rows of a filtered-out bus route.
fn write_large_feed(dir: &Path) {
    std::fs::write(
        dir.join("stops.txt"),
        "stop_id,stop_name,stop_lat,stop_lon,location_type,parent_station\n\
         A,Ayllu,37.780000,-122.416590,0,\n\
         B,Bandera,37.779102,-122.392722,0,\n\
         C,Chakana,37.770000,-122.400000,0,\n",
    )
    .expect("stops.txt is written");

    std::fs::write(
        dir.join("routes.txt"),
        "route_id,route_short_name,route_long_name,route_type,route_color\n\
         M1,1,Metro Line,1,0057B8\n\
         B1,40,Bus Line,3,\n",
    )
    .expect("routes.txt is written");

    let mut trips = String::from("route_id,trip_id\n");
    for trip in 0..KEPT_TRIPS {
        writeln!(trips, "M1,M1_t{trip}").expect("a string is writable");
    }
    trips.push_str("B1,B1_t1\n");
    std::fs::write(dir.join("trips.txt"), trips).expect("trips.txt is written");

    // Buffered and flushed once: two million `write!` calls straight at a `File`
    // is a syscall each and dominates the test.
    let mut stop_times = std::io::BufWriter::new(
        std::fs::File::create(dir.join("stop_times.txt")).expect("stop_times.txt is creatable"),
    );
    writeln!(
        stop_times,
        "trip_id,arrival_time,departure_time,stop_id,stop_sequence"
    )
    .expect("the header is written");
    for trip in 0..KEPT_TRIPS {
        for (sequence, stop) in ["A", "B", "C"].iter().enumerate() {
            writeln!(stop_times, "M1_t{trip},08:00:00,08:00:00,{stop},{sequence}")
                .expect("a kept row is written");
        }
    }
    for row in 0..(TOTAL_ROWS - KEPT_ROWS) {
        writeln!(stop_times, "B1_t1,06:00:00,06:00:00,A,{row}").expect("a filler row is written");
    }
    stop_times.flush().expect("stop_times.txt is flushed");
}

/// Assertion 3. Two million rows in, eighteen thousand retained.
///
/// Costs ~3 s under a debug `cargo test` — ~0.7 s to write the ~50 MB file and
/// ~2.5 s to parse it — against `real_feed.rs`'s ~25 s radius test. Every row is
/// still *parsed*, which is unavoidable: the `trip_id` deciding a row is a field
/// of the row. What the phase removes is holding them.
#[test]
fn a_huge_table_is_retained_only_where_a_kept_trip_wants_it() {
    let dir = scratch("llika-gtfs-streaming");
    write_large_feed(&dir);

    let feed = Feed::read(&dir, &ImportParams::default()).expect("the generated feed reads");

    assert_eq!(
        feed.stop_times.len(),
        KEPT_ROWS,
        "the reader retained {} of {TOTAL_ROWS} rows, not the {KEPT_ROWS} its kept trips run",
        feed.stop_times.len(),
    );
    // The control, so the literal above cannot be read as coincidental: the
    // reader this phase replaced retained all two million.
    assert!(feed.stop_times.len() < TOTAL_ROWS / 100);
    assert!(
        feed.stop_times.iter().all(|row| row.trip_id != "B1_t1"),
        "a row of the filtered-out route survived"
    );

    // The other three tables are read whole, which is the scope of the phase.
    assert_eq!(feed.trips.len(), KEPT_TRIPS + 1);
    assert_eq!(feed.routes.len(), 2);

    // ~50 MB in the system temp directory, and `scratch` only removes a
    // directory on the *next* create. It cleans up after itself.
    std::fs::remove_dir_all(&dir).ok();
}

/// Assertion 4: a row whose `trip_id` matches no kept trip is **discarded, not
/// errored** — witnessed on the committed fixture, where three of the 48 rows
/// belong to `B1_t1`, the trip of the bus route the `route_type` filter removes.
///
/// `import.rs:the_route_type_filter_removes_the_bus_route_and_its_stop` already
/// exercises those rows end to end, so what is asserted here is that the
/// streaming reader does not turn a working case into an error — and, in the
/// literal, exactly where the three rows went.
#[test]
fn a_row_of_no_kept_trip_is_discarded_rather_than_an_error() {
    let feed = Feed::read(&feed_dir(), &ImportParams::default()).expect("the fixture feed reads");

    // Hand-counted from `tests/fixtures/feed/stop_times.txt`: 48 rows, less
    // `B1_t1`'s three.
    assert_eq!(feed.stop_times.len(), 45);
    assert!(
        feed.stop_times.iter().all(|row| row.trip_id != "B1_t1"),
        "the bus route's rows were retained"
    );
}
