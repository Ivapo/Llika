---
id: llk-001
title: schematic-map-pipeline
note: >
  The v1 pipeline turning a JSON metro network into one static octilinear SVG
  schematic map — projection, grid snap, Stott-Rodgers hill-climbing layout and a
  line-bundling renderer, behind a CLI.
status: accepted
last_updated: 2026-08-14

phases:
  - name: "Phase 1 — thin end-to-end slice: JSON in, SVG out"
    reviewed: 2026-08-14
    shipped: 2026-08-14
    cut: null
    by: null
  - name: "Phase 2 — the five cost criteria"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 3 — single-station hill-climbing"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 4 — cluster moves"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 5 — line-bundling renderer"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 6 — full parameter surface"
    reviewed: null
    shipped: null
    cut: null
    by: null

extends: null
supersedes: null
superseded_by: null
related: []
reference: >
  Stott, Rodgers, Martinez-Ovando, Walker, "Automatic Metro Map Layout Using
  Multicriteria Optimization", IEEE TVCG 17(1):101-114, 2011, DOI
  10.1109/TVCG.2010.24; earlier form Stott & Rodgers, ICC 2005. Out of scope from
  it: label placement, and the mixed-integer and constraint-solver formulations it
  compares itself against. LOOM (Bast, Brosi, Storandt; Univ. Freiburg) is prior
  art whose papers are readable and whose GPL-3.0 C++ source must not be vendored.
---

# Schematic map pipeline

## 1. Goal

Turn a real metro network — stations with real latitudes and longitudes, and lines
as ordered station lists — into a schematic map: the abstract diagram style of a
transit poster, where track runs in straight strokes at multiples of 45 degrees and
geographic accuracy is traded away for legibility.

**The observable is the drawn map**: the network rendered as an octilinear,
straight-line, mostly-45-degree transit diagram, delivered in v1 as one static SVG
file a person opens and reads as a transit poster. Not the layout coordinates, not
the graph, not the parameter structs — the picture.

The end state of v1, concretely:

```console
$ llika --input tests/fixtures/sample_network.json --output bayside.svg \
        --grid-spacing 900 --iterations 200 --w-crossing 5.0
wrote bayside.svg — 17 stations, 3 lines, grid 900m, cost 4820.3 → 611.7 over 200 iterations
```

`bayside.svg` opens in any browser and reads as a transit diagram: every edge at or
near a multiple of 45 degrees, a marker at every stop, and the Red and Green lines
drawn as two parallel strokes along the trunk they share, converging to a single
point where they actually interchange.

The first automatic result must already look good with no tuning. The flags exist to
improve a good result, not to rescue a bad default.

### 1.1 Non-goals

- **Label placement.** No station-name text in v1. It is a hard sub-problem of its
  own in the source paper and it is deferred whole.
- **A GUI.** v1 is a library plus a CLI. The Tauri desktop app is the roadmap target
  and §2.6 is the constraint that keeps it cheap, but no UI code is written here.
- **Real network import.** No OpenStreetMap and no GTFS. Input is a hand-authored
  JSON file against the schema in §2.1. See OQ-4.
- **Guaranteed octilinearity.** This design *approaches* 45-degree angles through a
  soft penalty and will leave some edges off-angle. An algorithm that guarantees
  them exactly is a different layout mode — reserved in §2.7, not built here.
- **Vendoring LOOM.** Its papers may be read and reimplemented; its GPL-3.0 C++ must
  not be copied into this tree under any circumstances.

## 2. Design

### 2.1 Data model and input schema

Input is one JSON file. Stations carry an id, a display name and a real lat/lon.
A line carries an id, a name, a colour and an **ordered** station list; the graph's
edges are the consecutive pairs of that list, which is what makes a shared corridor
a topological fact rather than a geometric guess (§2.5).

```json
{
  "stations": [
    { "id": "central", "name": "Central", "lat": 37.7749, "lon": -122.4194 },
    { "id": "market",  "name": "Market",  "lat": 37.7756, "lon": -122.4020 }
  ],
  "lines": [
    { "id": "red", "name": "Red Line", "color": "#E4002B",
      "stations": ["westgate", "riverside", "oldtown", "eastbank", "central", "market"] }
  ]
}
```

*(That block is a **shape excerpt, not a valid input**: its Red line names four
stations the `stations` array does not define. A real file defines every id any line
references and the parser rejects one that does not, so do not use it as a parse
fixture. Its trunk carries `eastbank` and so matches OQ-5's four-edge trunk rather
than the three-edge one both source planning documents describe — that deviation is
deliberate and argued in OQ-5.)*

`Network` wraps a `petgraph` graph: stations are nodes, consecutive pairs are edges,
and an edge records the set of lines crossing it. Two lines crossing the same pair
share one edge — the edge is the corridor.

**`Network::from_input` error contract.** All five are hard errors, as `thiserror`
variants — none is a warning and none is silently repaired, because every one of
them makes a downstream invariant unenforceable:

| condition | reason it cannot be tolerated |
|---|---|
| a line references an undefined station id | there is no node to attach the edge to |
| duplicate station id | node identity stops being a key |
| duplicate line id | line identity is the render-offset sort key (§2.5) |
| a line with fewer than 2 stations | contributes no edge; silently vanishes from the map |
| the same id twice consecutively in a line | a self-loop, which has no angle and breaks `c3`/`c5` |

A station defined but referenced by no line is **legal** — an isolated node is a
degenerate but drawable map, and rejecting it would fail a valid single-station
input.

### 2.2 Projection, the grid, and SVG user space

Three coordinate systems, and the whole numeric chain is pinned here because every
phase downstream is gated on properties that only exist once it is.

**1. Projected plane — metres.** Each lat/lon is projected by a hand-rolled
equirectangular projection centred on the network's centroid:

```
x = (lon - lon_c) * 111_320 * cos(lat_c)      // metres east of centroid
y = (lat - lat_c) * 111_320                    // metres north of centroid
```

Output is **metres**, `x` increasing east and `y` increasing **north**. At
single-city scale the distortion is below what the layout step destroys anyway, so a
projection crate would buy accuracy this pipeline immediately discards.

**2. Grid — integer cells of `g` metres.** `i = round(x / g)`, `j = round(y / g)`, so
`j` also increases north. `g` is `LayoutParams::grid_spacing`, typed
`Option<f64>` in metres:

- `Some(m)` — use `m` metres, which is what `--grid-spacing` sets.
- **`None`, the default** — derive `g` from the network as the **median projected
  length of the graph's non-zero-length edges**, taking the **lower** of the two
  middle values when that count is even, so no float averaging enters a value
  everything else is keyed to.
- **The degenerate case has a stated answer**, because §2.1 makes it reachable: a
  station referenced by no line is legal, so `{"stations":[one],"lines":[]}` parses
  to a graph with **no edges**, and two stations at identical coordinates give a
  **zero-length** one. Where the non-zero-length edge set is empty — no edges at all,
  or every edge degenerate — `g` falls back to a constant **500 metres**. The median
  of an empty set is otherwise undefined and the natural implementation indexes out
  of bounds on an input this spec explicitly accepts; a zero median would make
  `round(x / g)` a NaN that casts silently to cell 0.

The fallback constant is not subject to the argument against constants in the
decision below: that argument is that no single spacing suits every network's
*typical edge*, and a network with no non-degenerate edge has no typical edge for a
constant to be wrong about. The only thing `g` still does there is keep isolated
stations in distinct cells, and any positive value does that.

**Degeneracy is confined to this derivation.** After snapping, the tie-break
guarantees one station per cell, so every *post-snap* edge is at least one cell long
and no zero-length edge ever reaches `c3` or `c5`. That is why §2.1 does not need a
sixth error condition rejecting coincident stations, and why it would be wrong to add
one — the numeric hazard lives entirely in the pre-snap plane.

#### Why the default grid spacing is derived, not a constant (decision, recorded)

`LayoutParams::default()` cannot know the network, so a constant would have to be
right for every city at once — and it cannot be. A metre value tuned to a network
whose stations sit 1.5 km apart puts a denser system entirely inside one cell and a
sparser one 50 cells apart, and §1 promises the first automatic result already looks
good **with no tuning**. Deriving `g` as the median edge length makes a typical edge
exactly one cell, which is the target `c2` (§2.3) is trying to reach, so the default
starts the layout near the criterion's optimum for any input at any scale. The cost
is that `g` is not a pure function of the parameters — it is a function of the
parameters *and* the network — so anything reporting it must read it back from the
layout rather than from `LayoutParams`.

**3. SVG user space.** With `units_per_cell` and `margin_cells` from `RenderParams`:

```
svg_x = (i - i_min + margin_cells) * units_per_cell
svg_y = (j_max - j + margin_cells) * units_per_cell      // note: flipped

width  = (i_max - i_min + 2 * margin_cells) * units_per_cell
height = (j_max - j_min + 2 * margin_cells) * units_per_cell
viewBox = "0 0 {width} {height}"
```

The flip is not incidental. Latitude increases north and SVG `y` increases **down**,
so an implementation that omits it renders the map upside down and still satisfies
every count-based assertion — which is why Phase 1's gate checks an explicit
north/south pair rather than trusting the eye.

**Grid occupancy and the tie-break.** The grid holds at most one station per cell,
or the occupancy checks in §2.4 mean nothing. Two stations can round to the same
cell, so the collision needs a deterministic rule (OQ-3, resolved):

- Stations are claimed **in the order they appear in the input file's `stations`
  array**. First claim wins; a later station that finds its cell taken spirals out.
- The spiral visits cells in increasing Chebyshev ring `k = 1, 2, 3…`, and within a
  ring in increasing `atan2` order starting due east, taking the first free cell.

**Input order is the iteration order everywhere it is observable**, not just here.
A `HashMap` keyed by station id is the natural structure for lookup and its
iteration order is randomised per process; anything that walks stations to produce
output — the snap, the search in §2.4, the render — walks the input-order sequence
instead. This is what makes §2.4's no-randomness claim true rather than merely
intended, and Phase 1 gates it.

### 2.3 The cost function

Five criteria, lower is better, combined as
`t = w1*c1 + w2*c2 + w3*c3 + w4*c4 + w5*c5`:

| | criterion | penalizes |
|---|---|---|
| `c1` | edge crossings | pairs of edges that cross |
| `c2` | edge length | edges that are not the target length |
| `c3` | angular resolution | uneven edge spacing around a station — this is what keeps a multi-line interchange legible |
| `c4` | line straightness | a bend where one line passes straight through a non-interchange station |
| `c5` | four-gonality | any edge angle that is not a multiple of 45 degrees; zero when it is |

`c5` is the octilinear-snap criterion and the reason the output looks like a transit
poster rather than a plot.

The five weights are the tunable surface. They are the reason the cost function is
built as five separable terms rather than one fused score: **`w1`-`w5` become the
sliders of the roadmap's UI**, so each criterion has to be independently computable
and independently meaningful.

The five criteria have different natural scales and the source paper gives no
canonical weights — OQ-2.

### 2.4 Hill-climbing

A fixed iteration count and **no randomness**, so a given input and parameter set
always produce the same map. Per iteration, for each station **in input order**
(§2.2): test every free grid point within a movement radius `r`, and move to
whichever lowers `t` most. `r` starts large and shrinks to one cell over the run.

Determinism needs one more rule than "no randomness" gives it: when two candidate
cells lower `t` by the same amount, **the earlier cell in §2.2's spiral order wins**.
Equal-cost candidates are common rather than exotic — a symmetric neighbourhood
produces them constantly — so leaving the tie to whichever the enumeration happened
to reach first makes the output depend on iteration order, which is the thing §2.2
went to trouble to fix.

Two move rejections keep the network from tearing:

- the target cell is occupied;
- the move flips the clockwise order of the station's connected edges.

Whether crossing another edge is a *third* hard rejection or is left entirely to the
soft `c1` penalty is OQ-1, and it is the one open question that changes what gets
built rather than when.

**Cluster moves.** Stations joined by edges shorter than `2g` are grouped and moved
as one rigid unit under the same score-and-move rule. This exists for a specific dead
end: a tight cluster attached to the rest of the map by a single long edge cannot
shorten that edge by moving any one of its stations, so single-station hill-climbing
is stuck at a local minimum it cannot see out of.

The `2g` threshold was written when `g` was an externally chosen constant and a
typical edge spanned many cells. §2.2's derived default changes what it selects —
OQ-7.

#### Why hill-climbing and not a solver (decision, recorded)

The alternative is a mixed-integer or constraint formulation that guarantees exact
octilinear angles. It is rejected for v1 on three grounds: it needs an external
solver dependency; its runtime is not bounded in a way that survives a UI slider
being dragged; and its five criteria do not decompose into five independent weights,
which is the property §2.3 is built around. The guaranteeing algorithm is reserved as
a second mode in §2.7 rather than discarded.

### 2.5 Line-bundling renderer

Original design — the source paper covers layout and not rendering.

Real transit maps draw lines sharing a corridor as parallel offset strokes that
converge to one point at a true interchange. The rules:

- Two lines **share a corridor** when the same consecutive station pair appears in
  both station lists. This is a topology check. It will not catch an express line
  that skips stations along a corridor it visually shares — an accepted v1 limit,
  stated here so a reviewer does not report it as a bug.
- For a run of consecutive edges carrying the same line set, each line keeps one
  fixed perpendicular offset across the whole run, ordered by line id, so a line
  never visually swaps sides mid-run.
- At any station where the line set changes, or where more than two edges meet,
  every offset collapses to zero. Bundled lines therefore merge to a single point
  exactly at real interchanges and stay separate along a shared trunk.
- Each line renders as **one** SVG path across its full station list, not one path
  per edge, so corner rounding comes free from `stroke-linejoin="round"`.

### 2.6 API shape for reuse (decision, recorded)

`Network::from_input`, `run_layout` and `render_to_string` are each public in their
own right, not only reachable through the `build_schematic_svg` convenience
function. `LayoutParams` and `RenderParams` are plain `Serialize`/`Deserialize`
structs with a `Default` impl.

The reason is a future the code must not have to be rewritten for: a Tauri command
parses and projects a loaded file **once**, holds the `Network` in memory, and on
every slider drag calls only `run_layout` and `render_to_string` with new
parameters. If parse and layout are fused, every slider drag re-reads the file. The
same two structs pass straight through a Tauri command and through v1's CLI flags
with no translation layer.

The algorithm is written once, in Rust, and serves both the v1 CLI and the later app.

### 2.7 Reserved: layout modes

The layout step is one *kind* of a broader thing. A second mode — the
Bast/Brosi/Storandt octilinear grid-graph algorithm (2020), which guarantees exact
octilinear angles instead of approaching them through `c5` — is **reserved, not
designed**. When it is written it is a new spec carrying `extends: llk-001`, not a
phase appended here, because it is a new subject under the framework this spec sets
up rather than more of this one.

Nothing else in this spec is a reserved namespace. Projection, rendering and input
format have exactly one kind each in v1.

### 2.8 Crate and module layout

A Cargo workspace, two crates:

- `metro-core` — library. Data model, projection, layout, renderer.
- `metro-cli` — thin binary. Reads a network file, calls `metro-core`, writes SVG.

```
metro-core/src/
  lib.rs          build_schematic_svg()
  model.rs        Station, Line, Network
  io.rs           InputSchema, Network::from_input
  projection.rs   lat/lon -> plane
  geometry.rs     Point2, segment intersection, angle math
  grid.rs         GridPoint, snap_to_grid, GridOccupancy
  layout/
    mod.rs        LayoutParams, SchematicLayout, run_layout
    cost.rs       the five criteria, independently testable
    candidate.rs  candidate points, move validity
    cluster.rs    short-edge clusters and rigid moves
    hillclimb.rs  iteration loop, cooling schedule
  render/
    mod.rs        RenderParams, render_to_string
    corridor.rs   line-bundling
```

Dependencies: `petgraph`, `svg`, `serde`/`serde_json`, `clap`, `thiserror`. No
projection crate and no force-directed-layout crate — §2.2 gives the reason for the
first, and this algorithm is a bounded grid search rather than a physics simulation.

*(No `file:symbol` citations appear in this spec: no code exists yet. Phases that
land code are what put citable symbols in the tree, and later revisions cite them.)*

## 3. Open questions

- **OQ-1** — Is "does not cross another edge" a hard move rejection, or only a hard
  rule against exact overlap with ordinary crossings left to the soft `c1` penalty?
  *(needs-input — a direct re-read of the 2011 TVCG paper.)* **Blocks Phase 3**; it
  changes what `candidate.rs` implements. This is the one open question that can
  produce a wrong implementation rather than a late one: build the hard-rejection
  reading when the paper means the soft one and the layout freezes early with edges
  it was never allowed to improve through.
- **OQ-2** — Starting values for `w1`-`w5`. The paper gives none, and the five
  criteria have different natural scales. *(design call.)* Blocks nothing
  structurally; the first defaults are a starting point to tune by eye against the
  fixture, and Phase 3's visual gate is the first place they are judged. Recorded so
  a later pass does not mistake the first numbers for settled ones.
- **OQ-3** — ~~Deterministic tie-break when two stations snap to the same grid cell
  before hill-climbing starts. Proposed: spiral search outward to the nearest free
  cell, in a fixed order so the result is reproducible. *(design call.)* **Blocks
  Phase 1** — `GridOccupancy` cannot hold its one-station-per-cell invariant without
  an answer.~~ **RESOLVED 2026-08-14, in §2.2.** Claim order is the input file's
  `stations` array order, first claim wins, and a displaced station spirals out by
  increasing Chebyshev ring then increasing `atan2` from due east. Review round 1
  found the proposal as written fixed the *spiral's* order but never said **which**
  of two colliding stations keeps the cell — a gap that produces a different map
  either way, so the answer is now the pair of rules rather than one. The same
  ordering resolves equal-cost move candidates in §2.4, and Phase 1 gates it.
- **OQ-4** — Which input method follows v1: click-to-place on an interactive map, or
  import from OpenStreetMap / a GTFS feed. *(deferred — explicitly out of v1 scope
  per §1.1.)* Deliberately left open so the core algorithm is built against
  hand-authored data first and neither input path is designed around prematurely.
- **OQ-5** — ~~**The 17-station fixture does not exist.** The pointer chain dead-ends:
  the seed document points at two planning files, the plan file points at the agent
  research file, and the agent research file contains no JSON and no station ids at
  all. Nothing points back. *(answerable now — it must be authored.)* **Blocks Phase
  1**, whose scope includes writing it and whose exit gate depends on it.

  The described shape — 17 stations, 3 lines, a Red/Green trunk `riverside` →
  `oldtown` → `central` → `market`, `central` at degree 4 across 3 distinct lines,
  `market` a 3-way split where the line set changes, coordinates deliberately
  off-grid — is satisfiable, but **under-constrained for the gates keyed to it**.
  Round 1 found two properties the fixture must also have, neither implied by the
  above and both discovered only by working forward to later phases:

  - **At least one pair of stations lands in the same grid cell at the default `g`.**
    Off-grid is not colliding: coordinates that merely miss cell centres never
    exercise OQ-3's tie-break, so the one phase that builds the tie-break would ship
    it with zero gate coverage. A realistically close pair — two stops a few hundred
    metres apart, well under a median-length edge — supplies it naturally.
  - **The trunk carries two *consecutive* interior stations of degree 2 with the
    same line set** — and this deliberately lengthens the trunk the source documents
    describe. §2.5 collapses every offset to zero wherever degree exceeds 2 or the
    line set changes. On the sources' 3-edge trunk `riverside` → `oldtown` →
    `central` → `market`, offsets collapse at `central` (degree 4 by construction)
    and at `riverside` (the line set changes — Red arrives there from `westgate`),
    leaving `oldtown` as the *only* interior station. Two paths pinched at both ends
    and separated at one point in the middle are a lens, not the "two distinct,
    **constant**, parallel offsets" Phase 5's gate asserts. So the fixture's trunk is
    `riverside` → `oldtown` → `eastbank` → `central` → `market` — four edges, with
    `oldtown` and `eastbank` both degree 2 and both carrying exactly {Red, Green},
    giving one genuinely parallel segment between them.

    Recorded as a deviation rather than folded in silently: the sources say a 3-edge
    trunk, this spec says four, and the reason is that §2.5's own collapse rule makes
    the 3-edge version unable to satisfy Phase 5. Settled here because Phase 1 is
    where the fixture is authored, and discovering it at Phase 5 means re-authoring
    the fixture every later gate is keyed to.~~

  **RESOLVED 2026-08-14 by Phase 1, which authored it** at
  `metro-core/tests/fixtures/sample_network.json`. 17 stations, 3 lines and **17**
  deduped corridors — the literal assertion 1 is keyed to, hand-counted as
  7 + 8 + 6 = 21 consecutive pairs less the 4 shared trunk pairs. Red and Green run
  the four-edge trunk `riverside` → `oldtown` → `eastbank` → `central` → `market`;
  `central` is degree 4 across 3 lines, `market` the 3-way split, and `oldtown` and
  `eastbank` are both degree 2 carrying exactly {Red, Green}, which is the parallel
  segment Phase 5 needs. Blue closes a cycle `central` → `market` → `quayside` →
  `brookside` → `southgate` → `central`, so the graph is not a tree and Phase 2's
  `c1` has crossings to find.

  Two facts settled by authoring it rather than by specifying it. **The collision
  pair is `northgate` and `lakeside`** — two northern termini on different lines,
  403 m apart, both degree 1, so the displacement the tie-break causes distorts
  nothing structural; `northgate` is earlier in the `stations` array and keeps the
  cell, `lakeside` takes the cell due east. And **the derived `g` is
  2269.9117477523614 m**, the 9th of the 17 sorted edge lengths — the
  `riverside` → `hillcrest` corridor.

  One thing the fixture cannot cover, recorded so a later phase does not assume it:
  17 is an **odd** edge count, so the lower-middle median rule is invisible in `g`'s
  literal. That half of assertion 4 is a unit test on `grid::median_lower` over a
  hand-built even-count set, which is where it belongs.
- **OQ-6** — `c2` as described penalizes edges that are not *exactly one grid cell*
  long, which makes the target a network of uniform unit edges. Is the target length
  one cell, or the mean edge length, or a per-edge ideal? *(design call.)* **Blocks
  Phase 2.** The uniform-unit reading fights `c5` on any network whose station
  spacing varies, which is every real one.

  §2.2's derived default weakens the objection without settling it: when `g` is the
  median edge length, "exactly one cell" *is* the network's own typical spacing, so
  the target is no longer an arbitrary constant. It re-opens the moment a user passes
  `--grid-spacing`, which is the case the resolution has to cover.

- **OQ-7** — §2.4's cluster threshold is `2g`, chosen when `g` was an externally
  supplied constant. Under §2.2's derived default `g` is the median edge length, so
  **at least half of all edges are `≤ g`** and therefore under `2g` by construction:
  the step meant to group "stations joined by very short edges" would group most of
  the network into one rigid unit and the cluster move would degenerate into
  translating the whole map. *(design call.)* **Blocks Phase 4.** Candidate answers:
  scale the threshold to a fraction of the median rather than a multiple; define it
  against the *shortest* edges by percentile; or key it to absolute metres
  independent of `g`. Recorded rather than resolved because Phase 4 has its own
  fixture and gate, and the right answer is measurable there and guesswork here.

## 4. Implementation phases

Strictly sequential — each depends on the one before. Each is one plan-mode pass and
each carries the two standing plan steps (a commit plan, a reconciliation step).

### Phase 1 — thin end-to-end slice: JSON in, SVG out
*Produces the observable: **yes**, deliberately. Grid-snap with zero hill-climbing
iterations and a bare renderer produce a real, viewable, ugly SVG before any layout
intelligence exists. This phase is large because every smaller cut of it produces
nothing anyone can see: a workspace produces no map, and a projection module
produces no map. The point of taking the whole slice at once is that everything after
it is an improvement to a picture that already exists.*

- **Scope:** the Cargo workspace and both crates with the §2.8 dependencies.
  `model.rs`; `io.rs` implementing §2.1's five-condition error contract;
  `projection.rs` and `grid.rs` implementing §2.2's full chain, including the OQ-3
  claim order and spiral. `geometry.rs` gets **`Point2` only** — segment intersection
  and angle math move to Phase 2, whose `c1` and `c5` are their first consumers and
  first tests. `LayoutParams` with `grid_spacing: Option<f64>` and a `Default`
  (Phase 2 adds `w1`-`w5`); `RenderParams` with `units_per_cell` = 40,
  **`margin_cells` = 2** and `stroke_width` = 6 as its `Default` (Phase 6 completes
  the surface). The margin must default **above zero**: a one-station network has
  `i_max = i_min`, so §2.2's envelope reduces to `2 * margin_cells * units_per_cell`
  and a zero margin yields a zero-extent document for the very input assertion 5
  requires to render. `run_layout` doing
  snap-only, with the iteration loop **absent rather than stubbed**.
  `render_to_string` drawing one `<path>` per line and one marker per station, no
  bundling. `build_schematic_svg`. `metro-cli` with `--input` and `--output`.
  **Author the fixture** at `metro-core/tests/fixtures/sample_network.json` to the
  full shape OQ-5 describes, including the collision pair and the four-edge trunk —
  it does not exist and cannot be copied. Plus the two degenerate inputs assertion 5
  needs.
- **Exit gate:** `cargo test` green. Ten assertions, each reproducible by a second
  person from the fixture and this section alone:
  1. the fixture parses to **17** stations and **3** lines, and to a deduped edge
     count matching a **literal hand-counted from the JSON by a person** and written
     into the test. Not recomputed by the dedup code under test — a test that derives
     the expected value from the implementation asserts only that the code agrees
     with itself. OQ-5's constraints do not fix this number; it exists only once the
     fixture is authored.
  2. `central` has degree 4 spanning 3 distinct lines; `oldtown` and `eastbank` are
     both degree 2 and both carry exactly {Red, Green}.
  3. each of the five §2.1 error conditions is rejected, one test each.
  4. **the derived `g` equals a literal hand-computed from the fixture's edge
     lengths**, to a relative tolerance of `1e-6` — `g` comes out of `cos` and
     `sqrt`, so exact `f64` equality against a hand-computed decimal will not hold —
     plus a unit test over a hand-built even-count length set asserting the
     lower-middle value and *not* the mean of the two middles. Without this the
     one piece of new critical-path machinery ships unverified: a collision still
     occurs under mean-instead-of-median, the bbox check is pre-grid, the flip and
     count checks are scale-invariant, and byte-stability asserts stability rather
     than correctness — so `(a+b)/2`, the reflex implementation, passes everything
     else and Phase 2's byte-identity gate then freezes it.
  5. **the two degenerate inputs render rather than panic**: a network with one
     station and no lines, and a network whose only edge joins two stations at
     identical coordinates. Both produce a valid SVG with `g` = 500 m. §2.1 makes
     both legal, so both are reachable.
  6. at the default derived `g`, **every station occupies a distinct cell**, and the
     OQ-5 collision pair is shown to have genuinely collided: their raw rounded cells
     are equal and their post-tie-break cells differ. Both halves are needed — the
     first alone passes vacuously on a fixture where nothing ever collided.
  7. the projected bounding box is non-degenerate in both axes, and **its larger axis
     extent** is between 1 km and 100 km while the smaller is strictly positive. The
     axis is named because an elongated fixture passes one reading and fails another.
     The range is what catches a degrees-for-metres error, which no count or shape
     assertion can see; it does **not** catch `cos(lat_c)` omitted from `x`, which
     inflates the span by 27% at this latitude and stays inside the bounds. Only a
     hand-computed distance between two named stations would, and that is what
     assertion 4's literal effectively supplies.
  8. of two stations at different latitudes, the more northerly has the strictly
     **smaller** SVG `y` — the §2.2 flip, which every count-based assertion passes
     upside down.
  9. the output SVG parses as well-formed XML, carries exactly one `<path>` element
     per line (3) and 17 station markers.
  10. **byte-stability across processes**: the CLI binary, invoked on the fixture
     with `--input` and `--output` alone so every parameter takes its default, run
     twice as two separate processes, produces byte-identical files. Two in-process
     calls would not do — Rust's default hasher is seeded per process, so a station
     map iterated directly is stable within a run and varies between them, which is
     exactly the §2.2 violation this assertion exists to catch and the one Phase 2's
     byte-identical gate would otherwise inherit.

  Then the human half, named separately because it is **not** reproducible and does
  not carry the gate: open the SVG in a browser and confirm it shows a recognizable
  network. The ten assertions above are what pass or fail Phase 1.
- **Close-out:** seeds `rules/data-model.md`, `rules/projection-grid.md` and
  `rules/rendering.md`. Commit the workspace, the modules and the fixture.

### Phase 2 — the five cost criteria
*Produces the observable: **no**, argued. These are pure scoring functions with no
path to the renderer; the SVG is unchanged. They are a phase of their own because
this is the one point in the pipeline where expected values can be computed by hand
on small graphs, and folding them into Phase 3 would mean debugging the scorer and
the search together, with only a picture to tell them apart.*

- **Scope:** `layout/cost.rs` — `c1` through `c5` and the weighted total.
  `LayoutParams` gains `w1`-`w5` with a `Default` impl. `geometry.rs` gains segment
  intersection and angle math, moved here from Phase 1 because `c1` and `c5` are
  their first consumers and this is where they first get tested. Resolve OQ-6 and
  implement the chosen `c2` target. Nothing calls these yet.
- **Exit gate:** per-criterion unit tests over hand-built graphs with hand-computed
  expected values, each criterion having both a zero case and a known-nonzero case.
  Specifically: `c5` is exactly zero for an edge at each of the eight octilinear
  angles and strictly positive at 22.5 degrees; `c1` is zero for a path graph and
  exactly 1 for a hand-built single crossing; `c4` is zero where a line runs straight
  through a degree-2 station and positive at a bend. The Phase 1 SVG for the fixture
  is **byte-identical** before and after this phase.
- **Close-out:** seeds `rules/layout-cost.md`. Records the OQ-6 resolution in §3.

### Phase 3 — single-station hill-climbing
*Produces the observable: **yes** — the first map that actually looks schematic.*

- **Scope:** `candidate.rs` (candidate enumeration within `r`, move validity) and
  `hillclimb.rs` (iteration loop, cooling `r` from large to one cell). Both move
  rejections from §2.4. Cluster step remains absent. Resolve OQ-1 first — it decides
  what `candidate.rs` rejects.
- **Exit gate:** on the fixture, total cost at the final iteration is strictly lower
  than at the first, asserted in a test rather than eyeballed in a log. Determinism:
  two runs with identical input and parameters produce byte-identical SVG. A test
  that a move onto an occupied cell is rejected, and one that an order-flipping move
  is rejected. The fraction of edges within 5 degrees of a multiple of 45 is measured
  and asserted to be strictly greater than the same measurement on the Phase 1
  output.
- **Close-out:** updates `rules/layout-cost.md`, seeds `rules/layout-search.md`.
  Records the OQ-1 resolution in §3.

### Phase 4 — cluster moves
*Produces the observable: **yes** — the same map with a class of local minimum
removed.*

- **Scope:** `cluster.rs` — grouping stations joined by short edges, and moving a
  group as one rigid unit under the same score-and-move rule. **Resolve OQ-7 first**:
  the `2g` threshold as written selects the majority of edges under §2.2's derived
  `g`, which would make the cluster move a whole-map translation.
- **Exit gate:** a second, small fixture built specifically so a tight cluster hangs
  off the network by one long edge — a case where single-station moves provably
  cannot shorten it. On that fixture, final cost with cluster moves enabled is
  strictly lower than with them disabled. The 17-station fixture's cost is still
  non-increasing and still deterministic across runs.
- **Close-out:** updates `rules/layout-search.md`.

### Phase 5 — line-bundling renderer
*Produces the observable: **yes** — and this is the phase that makes the output
read as a transit poster rather than as a graph drawing.*

- **Scope:** `render/corridor.rs` implementing every rule in §2.5 — shared-corridor
  detection by shared consecutive pair, one fixed offset per line per run ordered by
  line id, offsets collapsing to zero where the line set changes or degree exceeds 2,
  one SVG path per line with `stroke-linejoin="round"`.
- **Exit gate:** structural assertions on the fixture's SVG — along the
  `oldtown` → `eastbank` segment, the one place OQ-5 guarantees two consecutive
  interior degree-2 stations carrying {Red, Green}, the two lines' paths hold two
  distinct, constant, parallel offsets; at `central` and at `market` both paths pass
  through the identical point; the document holds exactly one path element per line.
  Confirmed visually in a browser: the trunk is two parallel strokes converging to
  single points at the two interchanges.
- **Close-out:** updates `rules/rendering.md`.

### Phase 6 — full parameter surface
*Produces the observable: **yes** — the map, made tunable. This is the phase that
delivers §1's promise that a user can improve a good first result, and it is the
last thing the roadmap's UI needs from the core.*

- **Scope:** `clap` flags covering every `LayoutParams` and `RenderParams` field
  (`--grid-spacing`, `--iterations`, `--w-crossing` and the rest), plus `--params
  <file>` taking the whole struct as JSON. `Serialize`/`Deserialize`/`Default` on
  both structs. Structural snapshot tests.
- **Exit gate:** a `--params` file and the equivalent individual flags produce
  byte-identical SVG. A test enumerating both structs' fields against the registered
  flags fails if a field has no flag, so the surface cannot silently fall behind the
  structs. At least two structural snapshot tests — element and path counts,
  interchange coincidence — that are explicitly not pixel-exact. `--help` lists every
  flag.
- **Close-out:** seeds `rules/cli.md`, updates the `CLAUDE.md` observable line if the
  invocation in §1 has drifted from the real one.
