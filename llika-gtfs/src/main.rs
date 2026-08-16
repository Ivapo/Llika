//! `llika-gtfs` — read a GTFS feed, write a Llika network file.
//!
//! The two steps stay two steps. This binary writes `bart.json`; `llika` draws
//! it. The file between them is the point: it is an ordinary `llk-001` input
//! file — readable, hand-editable, and accepted by
//! `llika-core/src/io.rs:Network::from_input` unchanged — and it is where a
//! person deletes the depot spur and fixes the shouting-caps station name before
//! the map is drawn.
//!
//! The flag surface follows the rule `llika` holds itself to: **a flag is its
//! field name kebab-cased, with no exceptions and no table.** `route_types`
//! gives `--route-types`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use llika_gtfs::{ImportParams, ImportReport, import, to_json};

#[derive(Parser)]
#[command(
    name = "llika-gtfs",
    about = "Turn a published GTFS feed into a Llika network file."
)]
struct Args {
    /// GTFS feed to read: a `.zip` or an unpacked directory.
    #[arg(long)]
    input: PathBuf,

    /// Network JSON file to write.
    #[arg(long)]
    output: PathBuf,

    /// Comma-separated `route_type` values to keep [default: 0,1 — tram, light
    /// rail and metro].
    //
    // `Option<Vec<u16>>` with no `default_value`, so `None` stays "absent"
    // rather than arriving already set. `llika`'s parameter flags are shaped the
    // same way for the same reason, and it is what leaves room for a `--params`
    // file to speak later.
    #[arg(long, value_delimiter = ',')]
    route_types: Option<Vec<u16>>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Nothing is written until the whole import has succeeded, so a rejected feed
/// leaves no half-written output.
fn run(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    let params = match &args.route_types {
        Some(route_types) => ImportParams {
            route_types: route_types.clone(),
        },
        None => ImportParams::default(),
    };

    let (schema, report) = import(&args.input, &params)?;
    std::fs::write(&args.output, to_json(&schema)?)?;

    Ok(summary(&args.output.display().to_string(), &schema, &report))
}

/// The summary line §2.6 asks for.
///
/// It carries no dropped-route count yet, and that is deliberate: nothing in
/// this phase can drop a route. `LineTooShort` is the one condition OQ-3 is
/// still about, it is reachable only after platforms collapse, and Phase 2 is
/// where both the drop and this clause arrive. A permanent `0 dropped` would be
/// format stability bought with a number that cannot move.
fn summary(output: &str, schema: &llika_core::InputSchema, report: &ImportReport) -> String {
    format!(
        "wrote {output} — {} stations, {} lines, {} of {} routes matched",
        schema.stations.len(),
        schema.lines.len(),
        report.routes_kept,
        report.routes_seen,
    )
}
