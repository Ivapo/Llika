//! Phase 1's exit gate, assertions 7 and 8: the same feed imported twice as two
//! separate processes writes byte-identical files, and the `.zip` and the
//! unpacked directory agree.
//!
//! Two in-process calls would not do — `llika-cli/tests/byte_stability.rs` has
//! made the argument since `llk-001` Phase 1 and it holds one crate over. Rust
//! seeds its default hasher **per process**, so a stop map iterated directly to
//! produce the `stations` array is stable within a run and varies between them.
//! This is the step upstream of every determinism guarantee `llk-001` proved, so
//! a leak here would evaporate all of them at once.
//!
//! The archive is **built from the committed feed directory** rather than
//! committed beside it: a binary blob in the tree can drift from the CSVs a
//! person edits, and assertion 8 would then fail for a reason that has nothing
//! to do with the reader. Entries go in at the archive root, Deflated — a zip
//! nested under a top-level directory is a named Phase 4 hazard, not this
//! phase's subject.
//!
//! **Phase 5's gate, assertion 2, extends the second of those to an archive that
//! is actually large.** That phase rewrote the `Source::Archive` branch from a
//! `read_to_end` into a stream, and every case above it is 48 rows — one deflate
//! block, decompressed in a single call, which is the one size at which a
//! streaming bug cannot show. So the same comparison runs again over a feed
//! whose `stop_times.txt` crosses many block boundaries and the window they are
//! written against.
//!
//! **Phase 6's gate, assertion 5, adds the property on a third real feed.** Every
//! case above reads either the hand-authored fixture or an archive generated from
//! it, so the determinism guarantee has never been stated against a table shape
//! nobody in this repository chose. MBTA's is fetched rather than committed —
//! `llk-002` OQ-8 — so that case is `#[ignore]`d without it, via the `mbta_feed`
//! cfg `build.rs` emits.

mod common;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{feed_dir, run_importer, scratch};
use zip::write::SimpleFileOptions;

/// The four tables §2.1 reads, plus the two it never opens — `agency.txt` and
/// `calendar.txt` ride along so the archive is a plausible feed and extra
/// members are shown to be harmless.
/// Rows of filler on the large case's `stop_times.txt` — ~3.4 MB, against the
/// 32 KiB window deflate is written against.
const FILLER_ROWS: usize = 150_000;

const FEED_FILES: [&str; 6] = [
    "agency.txt",
    "calendar.txt",
    "stops.txt",
    "routes.txt",
    "trips.txt",
    "stop_times.txt",
];

/// Zip the six files of `from` into `at/feed.zip`.
///
/// `from` is a parameter rather than always `feed_dir()` because assertion 2's
/// large case builds its own feed directory first: the archive and the directory
/// have to hold the same bytes for the comparison to be about the reader.
fn build_zip(at: &Path, from: &Path) -> PathBuf {
    let path = at.join("feed.zip");
    let mut writer = zip::ZipWriter::new(BufWriter::new(
        File::create(&path).expect("the archive is creatable"),
    ));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for name in FEED_FILES {
        let bytes = std::fs::read(from.join(name)).expect("the fixture table is readable");
        writer.start_file(name, options).expect("archive entry");
        writer.write_all(&bytes).expect("archive entry written");
    }
    writer.finish().expect("the archive closes");
    path
}

fn run_ok(input: &Path, output: &Path) {
    let result = run_importer(input, output);
    assert!(
        result.status.success(),
        "llika-gtfs exited with {}: {}",
        result.status,
        String::from_utf8_lossy(&result.stderr),
    );
}

fn import_twice(dir: &Path, input: &Path) -> Vec<u8> {
    let (first, second) = (dir.join("first.json"), dir.join("second.json"));
    run_ok(input, &first);
    run_ok(input, &second);

    let a = std::fs::read(&first).expect("first output");
    let b = std::fs::read(&second).expect("second output");

    assert!(!a.is_empty());
    assert_eq!(a, b, "two processes produced different bytes");
    a
}

#[test]
fn two_separate_processes_write_the_same_bytes() {
    let dir = scratch("llika-gtfs-byte-stability");
    import_twice(&dir, &feed_dir());
}

/// Assertion 8. Both input forms reach the same reader by different doors, and a
/// door is where a divergence would enter.
#[test]
fn the_archive_and_the_directory_produce_identical_output() {
    let dir = scratch("llika-gtfs-zip-vs-directory");

    let from_directory = import_twice(&dir, &feed_dir());

    let archive_dir = dir.join("archive");
    std::fs::create_dir_all(&archive_dir).expect("scratch directory");
    let zip = build_zip(&archive_dir, &feed_dir());
    let from_archive = import_twice(&archive_dir, &zip);

    assert_eq!(
        from_directory, from_archive,
        "the .zip and the unpacked directory disagree",
    );
}

/// Phase 5's gate, assertion 2: the two doors still agree on an archive big
/// enough for the streaming reader to be doing something.
///
/// The feed is the committed one with `stop_times.txt` extended by 150,000 rows
/// on a `trip_id` no trip declares — ~3.4 MB, which is a hundred times the
/// 32 KiB window deflate is written against and many blocks past the single one
/// the 48-row case fits in. **The filler is discarded rather than kept**, which
/// makes this the large-scale statement of assertion 4 as well: the output must
/// still be the committed fixture's, byte for byte, from both doors.
///
/// It costs ~1.4 s under a debug `cargo test` — four binary runs over the large
/// feed — against `real_feed.rs`'s ~25 s radius test.
#[test]
fn the_archive_and_the_directory_agree_across_deflate_block_boundaries() {
    let dir = scratch("llika-gtfs-large-zip-vs-directory");

    let feed = dir.join("feed");
    std::fs::create_dir_all(&feed).expect("scratch directory");
    for name in FEED_FILES {
        std::fs::copy(feed_dir().join(name), feed.join(name)).expect("a fixture table is copied");
    }

    let mut stop_times = std::fs::read_to_string(feed.join("stop_times.txt"))
        .expect("the fixture's stop_times is readable");
    for row in 0..FILLER_ROWS {
        stop_times.push_str(&format!(
            "FILLER_t1,07:00:00,07:00:00,NOR,{row}
"
        ));
    }
    std::fs::write(feed.join("stop_times.txt"), &stop_times).expect("the large table is written");
    assert!(
        stop_times.len() > 3_000_000,
        "the table is too small to cross a deflate block boundary"
    );

    let from_directory = import_twice(&dir, &feed);

    let archive_dir = dir.join("archive");
    std::fs::create_dir_all(&archive_dir).expect("scratch directory");
    let zip = build_zip(&archive_dir, &feed);
    let from_archive = import_twice(&archive_dir, &zip);

    assert_eq!(
        from_directory, from_archive,
        "the .zip and the unpacked directory disagree on a large feed",
    );
    // The filler belongs to no trip, so it changes nothing: this is the
    // committed fixture's own output, which `golden.rs` pins.
    assert_eq!(
        from_directory,
        std::fs::read(feed_dir().parent().unwrap().join("golden/fixture.json"))
            .expect("the golden is readable"),
        "150,000 rows of no trip's stop times moved the output",
    );

    // ~3.4 MB in the system temp directory, and `scratch` only removes a
    // directory on the *next* create. It cleans up after itself.
    std::fs::remove_dir_all(&dir).ok();
}

/// A feed missing one of the four tables fails loudly and leaves nothing behind.
///
/// `llika-cli` holds the same line for a rejected input file: a half-written
/// output is worse than none, because the next step in the pipeline accepts it.
#[test]
fn an_incomplete_feed_fails_loudly() {
    let dir = scratch("llika-gtfs-incomplete-feed");
    let feed = dir.join("feed");
    std::fs::create_dir_all(&feed).expect("scratch directory");
    std::fs::copy(feed_dir().join("stops.txt"), feed.join("stops.txt")).expect("one table copied");

    let output = Command::new(env!("CARGO_BIN_EXE_llika-gtfs"))
        .arg("--input")
        .arg(&feed)
        .arg("--output")
        .arg(dir.join("never.json"))
        .output()
        .expect("the llika-gtfs binary runs");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("routes.txt"), "stderr was: {stderr}");
    assert!(!dir.join("never.json").exists(), "no half-written output");
}

/// Phase 6's gate 5: determinism across processes on a **third** real feed.
///
/// **Stated as an extension because this file read only `common::feed_dir()`
/// until now** — BART was never added to it, so "delegated as every phase has"
/// would have left this green without a line of new-feed work.
///
/// **A skip here costs least of the phase's two feed-reading gates**: the
/// property itself is already held by `two_separate_processes_write_the_same_bytes`
/// on the fixture and by the large-zip case above on a generated archive. What
/// this case adds is the property on a third real feed, at a scale and a column
/// shape nobody here authored, rather than the property.
///
/// **It costs ≈13 s under a debug `cargo test`** — `import_twice` shells out to
/// the binary twice, and the debug binary is ≈6.25 s on this archive — against
/// `real_feed.rs`'s ≈25 s radius test and the ≈1.4 s large-zip case above.
/// Stated so the price is chosen rather than discovered.
#[test]
#[cfg_attr(
    not(mbta_feed),
    ignore = "fetch https://cdn.mbta.com/MBTA_GTFS.zip to llika-gtfs/tests/fixtures/mbta.zip"
)]
fn two_processes_write_the_same_bytes_on_the_mbta_feed() {
    let dir = scratch("llika-gtfs-mbta-byte-stability");
    let feed = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mbta.zip");
    import_twice(&dir, &feed);
    std::fs::remove_dir_all(&dir).ok();
}
