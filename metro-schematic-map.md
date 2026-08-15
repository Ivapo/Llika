# Llika — Metro Schematic Map Generator

## Idea

A tool that takes a real metro network and produces a schematic map. A
schematic map is the abstract diagram style used on real transit posters,
for example the London Underground map. The tool does not just plot
stations on a real map. It transforms real station positions into the
straight-line, mostly-45-degree style of a real transit diagram.

The user can tweak layout parameters after the tool creates a first
result. The first automatic result must already look good on its own.

## Design decisions

- **Name:** Llika. This is a Quechua word for "net" or "web" — a mesh
  made from woven threads. A metro schematic map is a mesh of stations
  and lines. The line-bundling renderer draws parallel lines as
  separate threads. These threads converge to one point at each real
  interchange.
- **Input method (decide later):** the user did not lock in how a station
  gets placed yet. Two candidate input methods stay open: click a point
  on an interactive map, or import real station and line data from
  OpenStreetMap or a GTFS feed. Build the core layout algorithm first,
  on hand-authored test data, and add an input method afterward.
- **Preferred tech stack:** Tauri, TypeScript, and Rust.
- **Output platform:** v1 outputs one static SVG file from a command-line
  tool. The final product is a Tauri desktop app (Rust backend, TypeScript
  frontend), where the user tweaks parameters and sees the diagram update
  live. The core algorithm code is written once, in Rust, so it serves
  both the v1 CLI and the later Tauri app with no rewrite.
- **Algorithm scope for v1:** a fast heuristic, not a research-grade
  constraint solver. Chosen method: Stott & Rodgers multicriteria
  hill-climbing (see below), not a mixed-integer-programming solver.

## Prior art check

No existing Rust project does this. The closest existing tool, LOOM (a
C++ suite from the University of Freiburg), is GPL-3.0 licensed. Its
published algorithm papers are free to read and reimplement. Its C++
source code is not free to copy into a Tauri app under a different
license, so do not vendor LOOM code.

## Algorithm: Stott & Rodgers multicriteria hill-climbing

Source: Stott, J., Rodgers, P., "Automatic Metro Map Design Techniques,"
ICC 2005. Journal version: Stott, Rodgers, Martinez-Ovando, Walker, IEEE
TVCG 17(1):101-114, 2011 (DOI 10.1109/TVCG.2010.24).

1. **Initial layout.** Project each station's lat/lon to a flat plane
   (equirectangular, centered on the network's centroid). Snap each
   projected point to the nearest cell of a square integer grid with
   spacing `g`.
2. **Cost function.** A weighted sum of five criteria (lower is better):
   - `c1` edge crossings.
   - `c2` edge length — penalizes edges not exactly one grid cell long.
   - `c3` angular resolution — penalizes uneven edge spacing around a
     station. This keeps multi-line interchange stations legible.
   - `c4` line straightness — penalizes a bend where one line passes
     straight through a non-interchange station.
   - `c5` "four-gonality" — zero when an edge's angle is a multiple of
     45 degrees. This is the octilinear-snap criterion.
   - Total cost `t = w1*c1 + w2*c2 + w3*c3 + w4*c4 + w5*c5`. The five
     weights become the tunable parameters in the future UI.
3. **Hill-climbing loop**, a fixed number of iterations, no randomness:
   for each station, test every free grid point within a movement
   radius `r` of its current spot, and move to whichever point lowers
   `t` the most. Reject a move that lands on an occupied grid point or
   that flips the clockwise order of a station's connected edges. `r`
   starts large and shrinks to 1 grid cell over the run.
4. **Cluster moves.** Group stations connected by very short edges
   (under `2g`) and move the group as one rigid unit, using the same
   move-and-score rule. This step escapes a dead end: a tight cluster
   attached to the rest of the map by one long edge cannot shorten that
   edge by moving any single station in the cluster.
5. Label placement (station name text) is out of scope for v1.

## Architecture (v1)

A Rust workspace with two crates:

- `metro-core` — library crate. Holds the data model, the projection
  step, the layout algorithm, and the SVG renderer.
- `metro-cli` — thin binary crate. Reads a network file, calls into
  `metro-core`, writes an SVG file to disk.

Dependencies: `petgraph` (station graph), `svg` (SVG output),
`serde`/`serde_json` (input file and parameter structs), `clap` (CLI
flags), `thiserror` (errors). No projection library and no
force-directed-layout crate are needed — a hand-rolled equirectangular
projection is enough at single-city scale, and this algorithm is a grid
search, not a physics simulation.

### Module layout (`metro-core/src/`)

```
lib.rs                // build_schematic_svg(): the one convenience fn
model.rs               // Station, Line, Network (wraps a petgraph graph)
io.rs                   // InputSchema (serde), Network::from_input
projection.rs            // Projector: lat/lon -> flat plane
geometry.rs                // Point2, segment intersection, angle math
grid.rs                      // GridPoint, snap_to_grid, GridOccupancy
layout/
  mod.rs                      // LayoutParams, SchematicLayout, run_layout()
  cost.rs                       // the five cost criteria, independently testable
  candidate.rs                    // candidate grid points, move validity checks
  cluster.rs                        // short-edge cluster detection and moves
  hillclimb.rs                        // the iteration loop and cooling schedule
render/
  mod.rs                              // RenderParams, render_to_string()
  corridor.rs                           // line-bundling (see below)
```

### Input file schema

A JSON file with stations and lines. A line is an ordered list of
station ids — the edges of the graph come from consecutive pairs in
that list.

```json
{
  "stations": [
    { "id": "central", "name": "Central", "lat": 37.7749, "lon": -122.4194 },
    { "id": "market",  "name": "Market",  "lat": 37.7756, "lon": -122.4020 }
  ],
  "lines": [
    { "id": "red", "name": "Red Line", "color": "#E4002B",
      "stations": ["westgate", "riverside", "oldtown", "central", "market"] }
  ]
}
```

### API shape for future reuse

`Network::from_input`, `layout::run_layout`, and `render::render_to_string`
are each public functions on their own, not only bundled inside
`build_schematic_svg`. A future Tauri command parses and projects a
loaded file once, keeps the resulting `Network` in memory, and on every
slider change calls only `run_layout` and `render_to_string` again with
new parameters — skipping the file-parse step each time. `LayoutParams`
and `RenderParams` are plain `Serialize`/`Deserialize` structs with a
`Default` impl, so the same types pass straight through a Tauri command
with no translation layer, and through CLI flags in v1
(`--grid-spacing`, `--iterations`, `--w-crossing`, and so on, with a
`--params <file>` option for the full struct).

### Line-bundling (parallel-offset) rendering

Original design — the source paper does not cover rendering. Real
transit maps draw lines that share a corridor as parallel offset
strokes, converging to one point at true interchanges.

- Two lines "share a corridor" when they cross the same graph edge (the
  same consecutive station pair appears in both lines' station lists).
  This is a topology check, not a geometric one — it will not catch an
  express line that skips stations along a shared corridor. Accepted
  v1 limit.
- For a run of consecutive edges with the same set of lines, assign
  each line a fixed perpendicular offset for that whole run (sorted by
  line id, for now), so a line does not visually swap sides mid-run.
- At any station where the set of lines changes, or where more than
  two edges meet, every line's offset collapses to zero at that
  station — so bundled lines visually merge into a single point
  exactly at real interchanges, and stay visually separate along a
  shared trunk.
- Each line renders as one SVG path across its full station list (not
  one path segment per edge), so corner rendering is free via
  `stroke-linejoin="round"`.

## Test fixture

A 17-station, 3-line JSON fixture, hand-authored with realistic but not
literal real-city coordinates. It exercises every part of the pipeline
in one file:
- Red and Green share a 3-edge trunk (`riverside` → `oldtown` →
  `central` → `market`) — exercises line-bundling.
- `central` has degree 4 across 3 distinct lines — a true interchange.
- `market` is a 3-way split point where the line set changes.
- Station coordinates are not grid-aligned, so hill-climbing has real
  work to do.

Full JSON fixture and research citations are saved at
`/Users/ivapo/.claude/plans/i-have-an-idea-fizzy-deer.md` and
`/Users/ivapo/.claude/plans/i-have-an-idea-fizzy-deer-agent-a307ab6a4dbbfe42e.md`.

## Build order

1. Empty workspace, both crates, dependencies added. `cargo build` green.
2. `model.rs`, `io.rs`, `projection.rs`. Unit test: load the fixture,
   check station/edge counts and a sane projected spread.
3. A naive end-to-end path: grid-snap with zero hill-climbing
   iterations, and a bare renderer with no line-bundling. Wire up
   `metro-cli`. Milestone: a real, viewable, if ugly, SVG comes out of
   the CLI before any hill-climbing code exists.
4. The five cost functions in `layout/cost.rs`, unit-tested against
   small hand-built graphs with known expected costs.
5. Single-node hill-climbing (`candidate.rs`, `hillclimb.rs`, cluster
   step still a no-op). Validate against the fixture: cost must not
   increase across iterations.
6. The cluster-move step, with a small dedicated close-station test
   case added to exercise it.
7. The line-bundling renderer (`render/corridor.rs`). Re-render the
   fixture and confirm the Red/Green trunk shows as two parallel
   strokes that converge to single points at `central` and `market`.
8. Full CLI flag surface, `--params` file support, a couple of
   structural (not pixel-exact) snapshot tests.

## Open questions to resolve during implementation

- Whether "no crossing another edge" is a hard move constraint, or only
  a hard rule against exact overlap, with ordinary crossings left to
  the soft `c1` penalty — needs a direct re-read of the source paper.
- Starting weight values for `w1`-`w5` have no canonical value in the
  paper — treat the first defaults as a starting point to tune by eye,
  not as settled numbers.
- Deterministic tie-break for two stations that snap to the same grid
  cell before hill-climbing starts (proposed: spiral search to the
  nearest free cell).

## Roadmap beyond v1

- Wrap `metro-core` as a Tauri backend; add a TypeScript frontend
  with sliders bound to `LayoutParams`/`RenderParams`, re-rendering
  live.
- Add real input methods: click-to-place stations on an interactive
  map inside the app, and/or import real systems via OpenStreetMap or
  a GTFS feed.
- Consider the Bast/Brosi/Storandt octilinear grid-graph algorithm
  (2020, the method behind LOOM) as an optional second layout mode. It
  guarantees exact octilinear angles instead of approaching them via a
  soft penalty, at the cost of more implementation work.

## Status

Idea stage — design researched and drafted, no code written yet.
