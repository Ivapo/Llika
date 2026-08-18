//! `llika` — read a network file, write a schematic SVG.
//!
//! Every `LayoutParams` and `RenderParams` field has a flag here, plus
//! `--params <file>` for the whole pair at once. Three rules hold the surface
//! together and each is load-bearing:
//!
//! 1. **A flag is its field name kebab-cased, with no exceptions and no table.**
//!    `w_crossings` gives `--w-crossings`. One hand-written exception would
//!    force the enumeration test below to consult the same mapping this file
//!    does, which makes it assert that the code agrees with itself.
//! 2. **The file loses to the flags, field by field.** A field named in neither
//!    takes its `Default`, which is the same thing `#[serde(default)]` means one
//!    level down.
//! 3. **Validation runs once, over the merged pair.** A clap `value_parser`
//!    would leave `--params` entirely unchecked, so the bounds live in
//!    [`validate`] and both routes go through it. They live here rather than in
//!    core because `run_layout` is infallible by design — every degenerate input
//!    the schema admits has a defined answer there rather than an error.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use llika_core::projection::Projector;
use llika_core::{
    LayoutParams, Network, RenderParams, parse_input, render_to_string, run_layout, total_cost,
};
use serde::{Deserialize, Serialize};

/// The upper bound on `--initial-radius`.
///
/// `candidate::spiral_offsets` materialises every cell of rings `1..=r` and
/// rebuilds it every sweep. Σ8k over `1..=64` is 16 640 tuples; over `1..=10 000`
/// it is 400 040 000, which is not a slow run but an unresponsive machine.
const MAX_INITIAL_RADIUS: u32 = 64;

/// `allow_negative_numbers` because several of these fields legitimately take
/// one and the validation table below sets no lower bound on them — a negative
/// weight inverts a criterion, and a negative `bundle_spacing` mirrors a bundle.
/// Without it clap reads `--w-crossings -1` as a missing value followed by an
/// unknown flag, so a value the surface accepts could not be typed at all.
#[derive(Parser)]
#[command(
    name = "llika",
    about = "Draw a transit network as a schematic map.",
    allow_negative_numbers = true
)]
struct Args {
    /// Network JSON file to read.
    #[arg(long)]
    input: PathBuf,

    /// SVG file to write.
    #[arg(long)]
    output: PathBuf,

    /// JSON file holding `{"layout": {...}, "render": {...}}`. Both keys are
    /// optional; individual flags override it field by field.
    #[arg(long)]
    params: Option<PathBuf>,

    // ---- LayoutParams, one flag per field ------------------------------
    //
    // Every one is `Option<T>` with **no** `default_value`, so `None` is
    // "absent" and `Some(v)` is "given". Defaults on the args here would make
    // `--params` inert: every field would arrive already set, and the file
    // would have nothing left to say.
    /// Grid cell size in metres. Default: the network's median edge length.
    #[arg(long)]
    grid_spacing: Option<f64>,

    /// Ceiling on search iterations. The search stops at the first one that
    /// moves nothing, so raising this cannot change the map.
    #[arg(long)]
    iterations: Option<u32>,

    /// Movement radius at the first sweep, in Chebyshev rings (1-64).
    ///
    /// Saturated above two at the shipped weights: on BART, 2, 3, 5 and 8 give
    /// bit-identical positions, so raising it only costs O(r²) candidates a
    /// sweep. It is not inert — `r_0 = 1` is a different map there, differing in
    /// 33 of 50 stations. On the 17-station sample fixture all five agree, which
    /// is why this was once claimed of every network.
    #[arg(long)]
    initial_radius: Option<u32>,

    /// Whether to translate whole bridge-side clusters as well [default: true].
    ///
    /// Switching it off reproduces the single-station search exactly, which is
    /// the baseline every measurement of this step is taken against.
    //
    // An explicit value rather than a `--no-` pair: the enumeration test derives
    // exactly one flag per field, and a pair is two names for one field. clap's
    // `SetTrue` could not turn this `true` default off in any case.
    #[arg(long, action = clap::ArgAction::Set)]
    cluster_moves: Option<bool>,

    /// Weight on edge crossings.
    #[arg(long)]
    w_crossings: Option<f64>,

    /// Weight on edge length against the target.
    #[arg(long)]
    w_edge_length: Option<f64>,

    /// Weight on angular resolution at a station.
    #[arg(long)]
    w_angular_resolution: Option<f64>,

    /// Weight on a line bending where nothing forces it to.
    #[arg(long)]
    w_straightness: Option<f64>,

    /// Weight on distance from the nearest multiple of 45 degrees.
    #[arg(long)]
    w_octilinearity: Option<f64>,

    // ---- RenderParams --------------------------------------------------
    /// SVG user units per grid cell.
    #[arg(long)]
    units_per_cell: Option<f64>,

    /// Blank cells around the network on every side.
    #[arg(long)]
    margin_cells: Option<f64>,

    /// Stroke width of a line, in user units.
    #[arg(long)]
    stroke_width: Option<f64>,

    /// Distance between adjacent lines of a bundle, in user units. Default: the
    /// stroke width. Zero disables bundling.
    #[arg(long)]
    bundle_spacing: Option<f64>,
}

/// The `--params` file format: one object, two optional keys.
///
/// It lives here and not in core because it is a CLI file format rather than a
/// library type — the argument for the two structs being plain `Serialize` is
/// about what a Tauri command passes, which is the structs and not a file.
///
/// `deny_unknown_fields` on the wrapper as well as on both structs, so
/// `{"layout_params": {...}}` is an error rather than a file that parses to
/// nothing.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ParamsFile {
    layout: LayoutParams,
    render: RenderParams,
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
///
/// Nothing is written until every check has passed, so a rejected run leaves no
/// half-written output.
fn run(args: &Args) -> Result<String, Box<dyn std::error::Error>> {
    let (layout_params, render_params) = merge(args)?;

    let text = std::fs::read_to_string(&args.input)?;
    let input = parse_input(&text)?;
    let network = Network::from_input(&input)?;

    validate(&layout_params, &render_params, Extent::of(&network))?;

    // The cost pair the summary reports. The "before" is this same run with the
    // search switched off — the reproducible baseline every measurement of the
    // search in this project is taken against, and two public calls.
    let baseline = run_layout(
        &network,
        &LayoutParams {
            iterations: 0,
            ..layout_params.clone()
        },
    );
    let cost_before = total_cost(
        &network,
        baseline.positions(),
        baseline.target_edge_cells(),
        &layout_params,
    );

    let layout = run_layout(&network, &layout_params);
    let cost_after = total_cost(
        &network,
        layout.positions(),
        layout.target_edge_cells(),
        &layout_params,
    );

    let svg = render_to_string(&network, &layout, &render_params);
    std::fs::write(&args.output, &svg)?;

    Ok(format!(
        "wrote {} — {} stations, {} lines, grid {:.0}m, cost {:.6} → {:.6} over {} iterations",
        args.output.display(),
        network.stations().len(),
        network.lines().len(),
        // Both of these are read back from the layout: each is a function of
        // the parameters *and* the network, so `LayoutParams` does not know
        // either. Printing the *requested* iteration count would be a lie the
        // moment the early exit fires, which on the sample fixture is at once.
        layout.grid_spacing(),
        cost_before,
        cost_after,
        layout.executed_iterations(),
    ))
}

/// The file first, then the flags over the top of it, field by field.
///
/// A field named in neither ends up at its `Default`, which is what
/// `#[serde(default)]` gives the fields inside the file and what
/// `LayoutParams::default()` gives the rest.
fn merge(args: &Args) -> Result<(LayoutParams, RenderParams), Box<dyn std::error::Error>> {
    let file = match &args.params {
        Some(path) => read_params_file(path)?,
        None => ParamsFile::default(),
    };

    let ParamsFile {
        layout: mut l,
        render: mut r,
    } = file;

    if let Some(v) = args.grid_spacing {
        l.grid_spacing = Some(v);
    }
    if let Some(v) = args.iterations {
        l.iterations = v;
    }
    if let Some(v) = args.initial_radius {
        l.initial_radius = v;
    }
    if let Some(v) = args.cluster_moves {
        l.cluster_moves = v;
    }
    if let Some(v) = args.w_crossings {
        l.w_crossings = v;
    }
    if let Some(v) = args.w_edge_length {
        l.w_edge_length = v;
    }
    if let Some(v) = args.w_angular_resolution {
        l.w_angular_resolution = v;
    }
    if let Some(v) = args.w_straightness {
        l.w_straightness = v;
    }
    if let Some(v) = args.w_octilinearity {
        l.w_octilinearity = v;
    }

    if let Some(v) = args.units_per_cell {
        r.units_per_cell = v;
    }
    if let Some(v) = args.margin_cells {
        r.margin_cells = v;
    }
    if let Some(v) = args.stroke_width {
        r.stroke_width = v;
    }
    if let Some(v) = args.bundle_spacing {
        r.bundle_spacing = Some(v);
    }

    Ok((l, r))
}

fn read_params_file(path: &Path) -> Result<ParamsFile, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read params file {}: {e}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|e| format!("cannot parse params file {}: {e}", path.display()).into())
}

/// How far the network reaches on each axis of the projected plane, in metres.
///
/// Only one bound needs it — `margin_cells` may be zero except where the drawn
/// network is degenerate on an axis, and the parameter pair cannot know that.
///
/// Taken from the **projected plane** rather than from raw lat/lon, so a network
/// straddling a pole (where `cos(lat_c)` collapses the east-west axis whatever
/// the longitudes say) is judged on what actually gets drawn.
#[derive(Debug, Clone, Copy)]
struct Extent {
    east_west: f64,
    north_south: f64,
}

impl Extent {
    fn of(network: &Network) -> Self {
        let projector = Projector::from_stations(network.stations());
        let points = projector.project_all(network.stations());

        let mut extent = Self {
            east_west: 0.0,
            north_south: 0.0,
        };
        if let Some(first) = points.first() {
            let (mut x0, mut x1) = (first.x, first.x);
            let (mut y0, mut y1) = (first.y, first.y);
            for p in &points[1..] {
                x0 = x0.min(p.x);
                x1 = x1.max(p.x);
                y0 = y0.min(p.y);
                y1 = y1.max(p.y);
            }
            extent.east_west = x1 - x0;
            extent.north_south = y1 - y0;
        }
        extent
    }

    /// Flat on at least one axis, so the document envelope on that axis is the
    /// margin alone.
    fn is_flat(&self) -> bool {
        self.east_west == 0.0 || self.north_south == 0.0
    }
}

/// Every bound, in one place, over the merged pair.
///
/// One function rather than a set of `value_parser`s because `--params` would
/// bypass those entirely, and it lands in the same phase as the flags. The five
/// weights carry no bound: any real number is a meaningful weight, including
/// zero, which is how a criterion is switched off.
fn validate(layout: &LayoutParams, render: &RenderParams, extent: Extent) -> Result<(), String> {
    if let Some(g) = layout.grid_spacing {
        // `round(x / g)` is a NaN at zero, and a NaN casts silently to cell 0.
        if !g.is_finite() || g <= 0.0 {
            return Err(format!(
                "--grid-spacing must be finite and above zero, got {g}"
            ));
        }
    }

    // Rejected at zero rather than accepted as one: `hillclimb::cooling_radius`
    // clamps it to 1, so zero is representable and silently means something
    // else. `iterations` has no bound at all — 0 is the supported snap-only
    // mode, not a degenerate value.
    if layout.initial_radius < 1 || layout.initial_radius > MAX_INITIAL_RADIUS {
        return Err(format!(
            "--initial-radius must be between 1 and {MAX_INITIAL_RADIUS}, got {}",
            layout.initial_radius
        ));
    }

    for (flag, value) in [
        ("--units-per-cell", render.units_per_cell),
        ("--stroke-width", render.stroke_width),
    ] {
        // A zero or negative scale draws nothing, or draws it inside out.
        if !value.is_finite() || value <= 0.0 {
            return Err(format!("{flag} must be finite and above zero, got {value}"));
        }
    }

    if !render.margin_cells.is_finite() || render.margin_cells < 0.0 {
        return Err(format!(
            "--margin-cells must be finite and not negative, got {}",
            render.margin_cells
        ));
    }
    if render.margin_cells == 0.0 && extent.is_flat() {
        // The envelope is `(extent + 2 * margin) * units_per_cell` on each
        // axis, so a flat network with no margin is a zero-extent document —
        // a file that renders and shows nothing.
        return Err(
            "--margin-cells must be above zero: this network is flat on one axis, so a \
             zero margin would give a document with no extent"
                .to_string(),
        );
    }

    if let Some(s) = render.bundle_spacing {
        // Zero is the supported disable seam, so only finiteness is required.
        if !s.is_finite() {
            return Err(format!("--bundle-spacing must be finite, got {s}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The exit gate's assertion 2: every serde-visible field of both structs
    /// has a registered flag, so the surface cannot silently fall behind the
    /// structs as fields are added.
    ///
    /// It **derives** the expected flag by kebab-casing the field name rather
    /// than consulting a table this file also consults — which is exactly what
    /// the no-exceptions naming rule buys, and the reason `--w-crossing` was
    /// not worth keeping. A table on both sides would assert only that the code
    /// agrees with itself.
    ///
    /// It lives inside `main.rs` because `llika-cli` has no lib target, so
    /// nothing under `tests/` can name [`Args`]. Shelling out to `--help` and
    /// grepping is the alternative and is worse: it tests the help renderer,
    /// not the registration.
    #[test]
    fn every_parameter_field_has_a_flag() {
        let command = Args::command();
        let registered: Vec<String> = command
            .get_arguments()
            .filter_map(|arg| arg.get_long().map(str::to_string))
            .collect();

        let mut checked = 0;
        for defaults in [
            serde_json::to_value(LayoutParams::default()).expect("LayoutParams serializes"),
            serde_json::to_value(RenderParams::default()).expect("RenderParams serializes"),
        ] {
            let fields = defaults.as_object().expect("both params are JSON objects");
            for field in fields.keys() {
                let flag = field.replace('_', "-");
                assert!(
                    registered.contains(&flag),
                    "field `{field}` has no `--{flag}`; registered flags are {registered:?}"
                );
                checked += 1;
            }
        }

        // Nine layout fields and four render ones. A guard on the guard: a
        // struct that serialized to an empty object would pass the loop above
        // vacuously.
        assert_eq!(
            checked, 13,
            "expected 13 parameter fields, walked {checked}"
        );
    }

    /// `--cluster-moves` takes an explicit value rather than being a bare flag.
    ///
    /// The field defaults to `true`, and clap's `SetTrue` cannot turn a `true`
    /// default off — which is the whole reason the spec pinned an explicit
    /// value here. This is the assertion that the `ArgAction::Set` above did not
    /// quietly get dropped.
    #[test]
    fn cluster_moves_takes_a_value() {
        let args = Args::try_parse_from([
            "llika",
            "--input",
            "in.json",
            "--output",
            "out.svg",
            "--cluster-moves",
            "false",
        ])
        .expect("`--cluster-moves false` parses");
        assert_eq!(args.cluster_moves, Some(false));

        assert!(
            Args::try_parse_from(["llika", "--input", "i", "--output", "o", "--cluster-moves"])
                .is_err(),
            "a bare `--cluster-moves` must not be accepted as a flag"
        );
    }

    /// The merge order, at the unit level: absent flags leave the file alone,
    /// and a given one wins.
    ///
    /// The gate asserts this end to end through two renders; this is the
    /// cheaper statement of the same rule, and it is what makes a failure there
    /// readable.
    #[test]
    fn a_flag_overrides_the_file_and_an_absent_one_does_not() {
        let args = Args::try_parse_from([
            "llika",
            "--input",
            "in.json",
            "--output",
            "out.svg",
            "--grid-spacing",
            "900",
        ])
        .expect("parses");

        // No `--params`, so the file half is `Default`.
        let (layout, render) = merge(&args).expect("merges");
        assert_eq!(layout.grid_spacing, Some(900.0));
        assert_eq!(layout.iterations, LayoutParams::default().iterations);
        assert_eq!(render, RenderParams::default());
    }
}
