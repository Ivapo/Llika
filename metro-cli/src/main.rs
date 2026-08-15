//! `llika` — read a network file, write a schematic SVG.
//!
//! Phase 1 carries `--input` and `--output` only. The full parameter surface —
//! every `LayoutParams` and `RenderParams` field, plus `--params <file>` — is
//! Phase 6.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use metro_core::{LayoutParams, Network, RenderParams, parse_input, render_to_string, run_layout};

#[derive(Parser)]
#[command(name = "llika", about = "Draw a metro network as a schematic map.")]
struct Args {
    /// Network JSON file to read.
    #[arg(long)]
    input: PathBuf,

    /// SVG file to write.
    #[arg(long)]
    output: PathBuf,
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

/// The three steps taken separately rather than through
/// `build_schematic_svg`: it is the same pipeline, and it is what the desktop
/// app will do, so the CLI is the standing proof that the split works.
fn run(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(&args.input)?;
    let input = parse_input(&text)?;

    let network = Network::from_input(&input)?;
    let layout = run_layout(&network, &LayoutParams::default());
    let svg = render_to_string(&network, &layout, &RenderParams::default());
    std::fs::write(&args.output, &svg)?;

    Ok(format!(
        "wrote {} — {} stations, {} lines, grid {:.0}m",
        args.output.display(),
        network.stations().len(),
        network.lines().len(),
        // Read back from the layout: the default `g` is a function of the
        // parameters *and* the network, so LayoutParams does not know it.
        layout.grid_spacing(),
    ))
}
