//! Phase 1 gate, assertion 10: the CLI, run twice as two separate processes on
//! the fixture with every parameter at its default, writes byte-identical
//! files.
//!
//! Two in-process calls would not do. Rust seeds its default hasher **per
//! process**, so a station map iterated directly to produce output is stable
//! within a run and varies between them — which is exactly the §2.2 violation
//! this assertion exists to catch, and the one Phase 2's byte-identical gate
//! would otherwise inherit.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root is the crate's parent")
        .join("metro-core/tests/fixtures/sample_network.json")
}

fn run_cli(output: &Path) {
    let status = Command::new(env!("CARGO_BIN_EXE_llika"))
        .arg("--input")
        .arg(fixture())
        .arg("--output")
        .arg(output)
        .status()
        .expect("the llika binary runs");
    assert!(status.success(), "llika exited with {status}");
}

#[test]
fn two_separate_processes_write_the_same_bytes() {
    let dir = std::env::temp_dir().join("llika-byte-stability");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let (first, second) = (dir.join("first.svg"), dir.join("second.svg"));

    run_cli(&first);
    run_cli(&second);

    let a = std::fs::read(&first).expect("first output");
    let b = std::fs::read(&second).expect("second output");

    assert!(!a.is_empty());
    assert_eq!(a, b, "two processes produced different bytes");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_rejected_input_fails_loudly() {
    let dir = std::env::temp_dir().join("llika-rejected-input");
    std::fs::create_dir_all(&dir).expect("scratch directory");
    let input = dir.join("broken.json");
    std::fs::write(
        &input,
        br##"{"stations": [], "lines": [
             {"id": "l", "name": "L", "color": "#000000", "stations": ["ghost", "other"]}]}"##,
    )
    .expect("write input");

    let output = Command::new(env!("CARGO_BIN_EXE_llika"))
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(dir.join("never.svg"))
        .output()
        .expect("the llika binary runs");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not defined"), "stderr was: {stderr}");
    assert!(!dir.join("never.svg").exists(), "no half-written output");

    std::fs::remove_dir_all(&dir).ok();
}
