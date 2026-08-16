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
const FEED_FILES: [&str; 6] = [
    "agency.txt",
    "calendar.txt",
    "stops.txt",
    "routes.txt",
    "trips.txt",
    "stop_times.txt",
];

fn build_zip(at: &Path) -> PathBuf {
    let path = at.join("feed.zip");
    let mut writer = zip::ZipWriter::new(BufWriter::new(
        File::create(&path).expect("the archive is creatable"),
    ));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for name in FEED_FILES {
        let bytes = std::fs::read(feed_dir().join(name)).expect("the fixture table is readable");
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
    let zip = build_zip(&archive_dir);
    let from_archive = import_twice(&archive_dir, &zip);

    assert_eq!(
        from_directory, from_archive,
        "the .zip and the unpacked directory disagree",
    );
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
