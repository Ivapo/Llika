//! Phase 1 gate, assertion 10: the CLI, run twice as two separate processes on
//! the fixture with every parameter at its default, writes byte-identical
//! files.
//!
//! Two in-process calls would not do. Rust seeds its default hasher **per
//! process**, so a station map iterated directly to produce output is stable
//! within a run and varies between them — which is exactly the §2.2 violation
//! this assertion exists to catch, and the one Phase 2's byte-identical gate
//! would otherwise inherit.
//!
//! Every phase since has delegated its own determinism clause here rather than
//! re-asserting it weakly in-process. Phase 6 is the first since Phase 1 to
//! rewrite the binary this file invokes, so it adds the `--params` run below:
//! the file route reaches the same pipeline by a different door, and a new door
//! is where a new hash-map walk would enter.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root is the crate's parent")
        .join("llika-core/tests/fixtures/sample_network.json")
}

fn run_cli(output: &Path, extra: &[&Path]) {
    let status = Command::new(env!("CARGO_BIN_EXE_llika"))
        .arg("--input")
        .arg(fixture())
        .arg("--output")
        .arg(output)
        .args(extra)
        .status()
        .expect("the llika binary runs");
    assert!(status.success(), "llika exited with {status}");
}

fn assert_two_processes_agree(dir: &Path, extra: &[&Path]) -> Vec<u8> {
    let (first, second) = (dir.join("first.svg"), dir.join("second.svg"));

    run_cli(&first, extra);
    run_cli(&second, extra);

    let a = std::fs::read(&first).expect("first output");
    let b = std::fs::read(&second).expect("second output");

    assert!(!a.is_empty());
    assert_eq!(a, b, "two processes produced different bytes");
    a
}

#[test]
fn two_separate_processes_write_the_same_bytes() {
    let dir = std::env::temp_dir().join("llika-byte-stability");
    std::fs::create_dir_all(&dir).expect("scratch directory");

    assert_two_processes_agree(&dir, &[]);

    std::fs::remove_dir_all(&dir).ok();
}

/// The same, through `--params`.
///
/// The file holds a non-default value, so this is not the default picture
/// repeated: it is the merge path, run twice, and the two runs must agree with
/// each other **and** differ from the defaults — otherwise the parameters never
/// reached the pipeline and this covers nothing new.
#[test]
fn two_processes_agree_through_a_params_file() {
    let dir = std::env::temp_dir().join("llika-byte-stability-params");
    std::fs::create_dir_all(&dir).expect("scratch directory");

    let params = dir.join("params.json");
    std::fs::write(
        &params,
        r#"{"layout": {"grid_spacing": 900.0}, "render": {"stroke_width": 8.0}}"#,
    )
    .expect("params file written");

    let tuned = assert_two_processes_agree(&dir, &[Path::new("--params"), &params]);

    let defaults_dir = dir.join("defaults");
    std::fs::create_dir_all(&defaults_dir).expect("scratch directory");
    let defaults = assert_two_processes_agree(&defaults_dir, &[]);

    assert_ne!(
        tuned, defaults,
        "the params file changed nothing, so this run covers no new path"
    );

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
