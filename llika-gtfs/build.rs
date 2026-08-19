//! Emit `mbta_feed` when the second city's archive is on disk.
//!
//! `llk-002`'s OQ-8 keeps that feed out of this repository — 18.6 MB, and the
//! network it imports to is committed in its place — so the two gates that read
//! it cannot run from a fresh clone and must say so rather than fail.
//!
//! **The reflex implementation is worse than no gate**, which is why this file
//! exists at all. An early `return` after a `println!` inside the test reports
//! `1 passed` and prints nothing, because libtest captures the output of passing
//! tests. `#[cfg_attr(not(mbta_feed), ignore = "…")]` prints `ignored` with the
//! reason, which is the outcome the phase asked for.
//!
//! `rustc-check-cfg` is not decoration: CI runs
//! `cargo clippy --workspace --all-targets -- -D warnings`, and without that line
//! a custom cfg is an `unexpected_cfgs` warning and the job is red.

fn main() {
    // Declared unconditionally — the lint fires whether or not the cfg is set.
    println!("cargo::rustc-check-cfg=cfg(mbta_feed)");
    // So fetching the feed re-triggers the build rather than leaving a stale cfg.
    println!("cargo::rerun-if-changed=tests/fixtures/mbta.zip");

    if std::path::Path::new("tests/fixtures/mbta.zip").exists() {
        println!("cargo::rustc-cfg=mbta_feed");
    }
}
