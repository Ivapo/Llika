//! Phase 6 gate, assertions 1, 3, 4 and 5 — the parameter surface, driven
//! through the binary.
//!
//! Through the binary because that is where the surface is. `--params` resolves
//! a field by serde name and a flag resolves it by clap registration, and
//! nothing below the process boundary exercises both routes against each other.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the workspace root is the crate's parent")
        .join("llika-core/tests/fixtures/sample_network.json")
}

/// A scratch directory per test. The names are fixed rather than unique, so two
/// tests sharing one would race.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("llika-params-{name}"));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn llika<I: IntoIterator<Item = OsString>>(args: I) -> Output {
    Command::new(env!("CARGO_BIN_EXE_llika"))
        .args(args)
        .output()
        .expect("the llika binary runs")
}

/// `--input <fixture> --output <dir>/<name>.svg`, plus whatever else is asked
/// for, and the output path alongside.
fn invocation(input: &Path, dir: &Path, name: &str, extra: &[&str]) -> (Vec<OsString>, PathBuf) {
    let output_path = dir.join(format!("{name}.svg"));
    let mut args = vec![
        OsString::from("--input"),
        input.into(),
        OsString::from("--output"),
        output_path.clone().into(),
    ];
    args.extend(extra.iter().map(OsString::from));
    (args, output_path)
}

/// Render the sample fixture with the given extra arguments and return the
/// bytes. Panics on a non-zero exit, so a broken invocation fails here rather
/// than as a confusing comparison later.
fn render(dir: &Path, name: &str, extra: &[&str]) -> Vec<u8> {
    let (args, output_path) = invocation(&fixture(), dir, name, extra);
    let output = llika(args);
    assert!(
        output.status.success(),
        "llika failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read(&output_path).expect("the SVG was written")
}

/// Run with the given extra arguments and require a clean rejection: non-zero
/// exit, a message on stderr, and no output file left behind.
fn reject(dir: &Path, name: &str, extra: &[&str]) -> String {
    reject_input(&fixture(), dir, name, extra)
}

fn reject_input(input: &Path, dir: &Path, name: &str, extra: &[&str]) -> String {
    let (args, output_path) = invocation(input, dir, name, extra);
    let output = llika(args);
    assert!(
        !output.status.success(),
        "llika accepted {extra:?}, which the validation table forbids"
    );
    assert!(
        !output_path.exists(),
        "a rejected run left a half-written output behind"
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_params(dir: &Path, name: &str, json: &str) -> String {
    let path = dir.join(format!("{name}.json"));
    std::fs::write(&path, json).expect("params file written");
    path.to_str().expect("a UTF-8 path").to_string()
}

/// **Assertion 1**, all three clauses in one test because all three are the
/// assertion.
///
/// The two routes resolve a field differently — the file by serde name, the
/// flag by clap registration — so this is the only clause that catches one of
/// them mis-wired. A file of *defaults* would satisfy the bare comparison while
/// proving nothing, since both sides would then be the default picture, which
/// is why the file holds non-default values on a field of **each** struct and
/// why the result is asserted to differ from the all-defaults render.
///
/// `grid_spacing` and `stroke_width`, deliberately — **not** `initial_radius`,
/// which is inert at the shipped weights, and **not** `iterations`, which is
/// inert above the convergence sweep (2 of 200 on this fixture). A test keyed
/// to either would pass whether the flag were wired or not.
#[test]
fn a_params_file_and_the_equivalent_flags_agree_and_the_flags_win() {
    let dir = scratch("equivalence");
    let params = write_params(
        &dir,
        "tuned",
        r#"{"layout": {"grid_spacing": 900.0}, "render": {"stroke_width": 8.0}}"#,
    );

    let from_file = render(&dir, "from-file", &["--params", &params]);
    let from_flags = render(
        &dir,
        "from-flags",
        &["--grid-spacing", "900", "--stroke-width", "8"],
    );
    assert_eq!(
        from_file, from_flags,
        "the file route and the flag route drew different pictures"
    );

    let defaults = render(&dir, "defaults", &[]);
    assert_ne!(
        from_file, defaults,
        "the params file holds only default values, so the comparison above proves nothing"
    );

    // The override direction, which the two runs above exercise only in
    // isolation: the same file plus a conflicting flag must render what the
    // flag says.
    let overridden = render(
        &dir,
        "overridden",
        &["--params", &params, "--grid-spacing", "1500"],
    );
    let flag_only = render(
        &dir,
        "flag-only",
        &["--grid-spacing", "1500", "--stroke-width", "8"],
    );
    assert_eq!(
        overridden, flag_only,
        "the file won over the flag; individual flags override the file field by field"
    );
    assert_ne!(overridden, from_file, "the override changed nothing");

    std::fs::remove_dir_all(&dir).ok();
}

/// **Assertion 3** — each validation bound rejects at its boundary, and by both
/// routes.
///
/// The second half is the load-bearing one. A clap `value_parser` alone passes
/// the flag route and lets every `--params` file through unchecked, which is
/// exactly the failure the single validation function exists to prevent — so
/// every row is asserted twice, once as a flag and once inside a file, and the
/// two must agree.
#[test]
fn every_bound_rejects_at_its_boundary_by_both_routes() {
    let dir = scratch("bounds");

    // (flag, boundary value, the key path inside a params file)
    let rows: &[(&str, &str, &str, &str)] = &[
        ("--grid-spacing", "0", "layout", "grid_spacing"),
        // Representable, and `cooling_radius` silently clamps it to 1 — so it
        // is rejected at the boundary rather than quietly meaning something
        // else. Named by the spec specifically.
        ("--initial-radius", "0", "layout", "initial_radius"),
        ("--initial-radius", "65", "layout", "initial_radius"),
        ("--units-per-cell", "0", "render", "units_per_cell"),
        ("--stroke-width", "0", "render", "stroke_width"),
        ("--margin-cells", "-1", "render", "margin_cells"),
    ];

    for (i, (flag, value, section, key)) in rows.iter().enumerate() {
        let via_flag = reject(&dir, &format!("flag-{i}"), &[flag, value]);

        let params = write_params(
            &dir,
            &format!("file-{i}"),
            &format!(r#"{{"{section}": {{"{key}": {value}}}}}"#),
        );
        let via_file = reject(&dir, &format!("file-run-{i}"), &["--params", &params]);

        assert!(
            via_flag.contains(flag) && via_file.contains(flag),
            "{flag} {value}: the two routes reported differently — flag said {via_flag:?}, \
             file said {via_file:?}"
        );
    }

    // `iterations` has no bound at all: 0 is the supported snap-only mode, not
    // a degenerate value. Asserted so a later pass does not "tidy" it into the
    // table above.
    let snap_only = render(&dir, "snap-only", &["--iterations", "0"]);
    assert!(!snap_only.is_empty());

    // `--bundle-spacing 0` likewise: the supported disable seam, not a bound.
    let unbundled = render(&dir, "unbundled", &["--bundle-spacing", "0"]);
    assert_ne!(unbundled, snap_only);

    std::fs::remove_dir_all(&dir).ok();
}

/// **Assertion 3**, the conditional row: `--margin-cells 0` is legal on a
/// network with extent, and rejected on one that is flat on an axis.
///
/// Both halves, because the first alone would pass on an implementation that
/// rejected a zero margin outright, and the second alone on one that rejected
/// every zero margin.
#[test]
fn a_zero_margin_is_rejected_only_where_the_network_is_flat() {
    let dir = scratch("margin");

    let with_extent = render(&dir, "flat-ok", &["--margin-cells", "0"]);
    assert!(!with_extent.is_empty());

    let one_station = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("llika-core/tests/fixtures/single_station.json");
    let stderr = reject_input(&one_station, &dir, "degenerate", &["--margin-cells", "0"]);
    assert!(
        stderr.contains("--margin-cells"),
        "a one-station network with no margin is a zero-extent document, got: {stderr}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// **Assertion 4** — a misspelled key is an error, not a silent default.
///
/// Without `deny_unknown_fields` this file parses to all-defaults and renders
/// happily, discarding the one value the user was tuning. That is the worst
/// case for a knob whose whole purpose is improving a result by eye, which is
/// why this is the one place the surface chooses strictness over tolerance.
#[test]
fn a_misspelled_key_in_the_params_file_is_an_error() {
    let dir = scratch("typo");

    let typo = write_params(&dir, "typo", r#"{"layout": {"w_crosings": 9.0}}"#);
    let stderr = reject(&dir, "typo-run", &["--params", &typo]);
    assert!(
        stderr.contains("w_crosings"),
        "the error must name the key that was not understood, got: {stderr}"
    );

    // The wrapper carries the attribute too, so a misspelled *section* is
    // caught by the same rule one level up.
    let section = write_params(&dir, "section", r#"{"layout_params": {"iterations": 9}}"#);
    reject(&dir, "section-run", &["--params", &section]);

    // And the tolerance half of the contract, asserted alongside so the two are
    // not confused: a file naming only the fields someone cares about is fine.
    let partial = write_params(&dir, "partial", r#"{"layout": {"w_crossings": 9.0}}"#);
    let rendered = render(&dir, "partial-run", &["--params", &partial]);
    assert!(!rendered.is_empty(), "missing is fine, misspelled is not");

    std::fs::remove_dir_all(&dir).ok();
}

/// **Assertion 5** — a structural snapshot, carried as a **regression guard**
/// and labelled as one.
///
/// It is parameter-invariant and so cannot fail for any parameter reason. It is
/// here to catch a rewrite of `main.rs` that stops calling the pipeline
/// correctly — which `llika-core/tests/render.rs` and `bundling.rs`, asserting
/// the same shape one crate down, cannot see.
#[test]
fn the_binary_still_draws_the_whole_network() {
    let dir = scratch("structure");

    for (name, extra) in [
        ("defaults", &[][..]),
        (
            "tuned",
            &["--grid-spacing", "900", "--stroke-width", "8"][..],
        ),
    ] {
        let svg = String::from_utf8(render(&dir, name, extra)).expect("the SVG is UTF-8");
        assert!(svg.starts_with("<svg"), "{name}: not an SVG document");
        assert_eq!(svg.matches("<path").count(), 3, "{name}: one path per line");
        assert_eq!(
            svg.matches("<circle").count(),
            17,
            "{name}: one marker per station"
        );
    }

    std::fs::remove_dir_all(&dir).ok();
}
