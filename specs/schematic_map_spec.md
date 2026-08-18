---
id: llk-001
title: schematic-map-pipeline
note: >
  The v1 pipeline turning a JSON metro network into one static octilinear SVG
  schematic map — projection, grid snap, Stott-Rodgers hill-climbing layout and a
  line-bundling renderer, behind a CLI.
status: accepted
last_updated: 2026-08-18

phases:
  - name: "Phase 1 — thin end-to-end slice: JSON in, SVG out"
    reviewed: 2026-08-14
    shipped: 2026-08-14
    cut: null
    by: null
  - name: "Phase 2 — the five cost criteria"
    reviewed: 2026-08-14
    shipped: 2026-08-14
    cut: null
    by: null
  - name: "Phase 3 — single-station hill-climbing"
    reviewed: 2026-08-15
    shipped: 2026-08-15
    cut: null
    by: null
  - name: "Phase 4 — cluster moves"
    reviewed: 2026-08-15
    shipped: 2026-08-15
    cut: null
    by: null
  - name: "Phase 5 — line-bundling renderer"
    reviewed: 2026-08-15
    shipped: 2026-08-15
    cut: null
    by: null
  - name: "Phase 6 — full parameter surface"
    reviewed: 2026-08-15
    shipped: 2026-08-15
    cut: null
    by: null
  - name: "Phase 7 — the weights, corrected against a real network"
    reviewed: 2026-08-17
    shipped: 2026-08-17
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

**CORRECTED 2026-08-15, at Phase 6's close-out.** The block above is what this goal
was written to promise before anything was built; two of its three lines are now
wrong, and the shipped binary is not going to be bent to match them. What `llika`
actually prints, at those same flags:

```console
$ llika --input llika-core/tests/fixtures/sample_network.json --output bayside.svg \
        --grid-spacing 900 --iterations 200 --w-crossings 5.0
wrote bayside.svg — 17 stations, 3 lines, grid 900m, cost 54.817110 → 12.747375 over 3 iterations
```

Three differences, each with its own reason. **`--w-crossings`, plural** — Phase 6
decided every flag is its field name kebab-cased with no exceptions, because one
irregular flag would force its field-to-flag gate to consult the same mapping the
implementation does; §1 is the side that changes. **The cost pair was invented** and
is wrong by two orders of magnitude; the real figures were measured independently at
Phase 6's review round and reproduced by the shipped binary. **`over 3 iterations`,
not 200** — §2.4's early exit means `iterations` is a ceiling, and printing the
requested count would be a lie. At the defaults the same line reads
`grid 2270m, cost 37.166633 → 11.338720 over 2 iterations`.

Nothing else in the block moved: 17 stations, 3 lines and `grid 900m` were right.

**CORRECTED AGAIN 2026-08-17, at Phase 7's close-out**, beneath the note above rather
than inside it — the 2026-08-15 figures were right when they were measured, and what
moved is the thing they were measured against. Phase 7 reweighted
`LayoutParams::default()` from 5/1/1/2/5 to 5/1/0.5/0.25/10, and `t` is *defined* by the
weights, so both cost pairs above are now wrong by construction and neither layout
changed. At §1's flags the line reads
`cost 48.328948 → 5.357277 over 3 iterations`; at the defaults,
`grid 2270m, cost 13.539238 → 4.662836 over 2 iterations`. **The fixture's picture is
bit-identical across the reweight** — `llika-core/tests/golden.rs` is what says so — so
this correction is arithmetic and nothing about `bayside.svg` moved.

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

#### The operational definitions (decision, recorded)

The table above is vocabulary, not a specification: five one-line descriptions leave
an implementer to invent five formulas, and a criterion nobody wrote down is a
criterion the gate cannot check. So each is pinned here.

**Domain: the grid.** Every criterion is evaluated over integer `GridPoint`
coordinates, not over the projected metre plane. §2.4 moves stations between cells,
so a criterion scored in metres would be scoring a different object from the one the
search moves. An edge's direction is `atan2(Δj, Δi)` over the integer cell offset.

- **`c1` — crossings.** For every unordered pair of edges **sharing no endpoint
  station**, add 1 when their **closed** segments intersect. `c1` is therefore an
  integer count. Sharing no endpoint is what makes a path graph score zero. The
  closed test is deliberate: an endpoint lying in another edge's interior, and a
  collinear overlap, both read as a crossing to the eye and both are reachable —
  occupancy keeps two *stations* out of one cell, but nothing stops an edge running
  over a cell some other station occupies.
- **`c2` — edge length.** `c2 = Σ_e (|e| / L − 1)²`, over edges, with `|e|` the
  Euclidean distance in cells and `L` the target length **OQ-6 settles**. Zero iff
  every edge is exactly `L`. The functional form is fixed here; only `L` is open.

  *(OQ-6 resolved 2026-08-14, at Phase 2: `L = max(1, m / g)` where `m` is the same
  median non-zero projected edge length `g` derives from. Under the default `g` it is
  exactly `1.0`, since the numerator is `g` itself. It is carried on
  `SchematicLayout` beside `grid_spacing`, for the same reason — it is a function of
  the parameters *and* the network.)*
- **`c3` — angular resolution.** For each station of degree `d ≥ 2`: order its
  incident edges by direction, take the `d` consecutive angular gaps `θ_1…θ_d`
  (which sum to `2π`), and let the ideal gap be `2π/d`. Then
  `c3 = Σ_v Σ_k |θ_{v,k} − 2π/deg(v)|`, in radians. Degree 0 and 1 contribute
  nothing. Two incident edges can share a direction, so ties in that ordering break
  by neighbour station index, which keeps the gap sequence deterministic.

  **`c3` has a positive floor at some degrees, by construction.** On an octilinear
  grid every gap is a multiple of `π/4`, so an even spread needs `8/d` integral and
  is reachable at exactly degree 1, 2, 4 and 8 — unreachable at 3, 5, 6 and 7. A
  degree-3 station's best of the ten possible shapes is `135°/135°/90°`, scoring
  `2·|3π/4 − 2π/3| + |π/2 − 2π/3| = π/3`. That is what a soft penalty does where the
  grid forbids its optimum, not a defect — but it is stated because a gate demanding
  `c3 = 0` at every degree would be unsatisfiable.

  **Spell that floor `FRAC_PI_3`, not `PI / 3.0`.** They are different doubles, one
  ulp apart — `1.04719755119659785336` against `1.04719755119659763132` — and the
  computed floor is bit-exactly the former. A test written `assert_eq!(c3, PI / 3.0)`,
  which is the natural spelling given the derivation above is written in π, fails on
  a correct implementation.
- **`c4` — line straightness.** For each line, walk its station list; at every
  **interior** station of that walk whose **degree is 2**, let `φ` be the **interior
  angle at the station** — both incident directions measured outward from it, never
  the turn angle — and add `π − φ` — zero when the
  line runs straight through, rising to `π` at a full reversal. Sum over all lines,
  so two lines sharing a corridor each pay their own bend. Degree-2 only: a bend at a
  junction is legitimate, and this criterion exists to stop a line kinking where
  nothing forces it to.
- **`c5` — four-gonality.** For each edge of direction `θ`: `r = θ mod π/4`,
  `d = min(r, π/4 − r)`, and `c5 = Σ_e d`, in radians. Zero exactly at the eight
  octilinear directions.

  **That exactness is a property of this formulation over integer offsets, and the
  gate depends on it.** `atan2` of an exact integer ratio returns an exact multiple
  of `FRAC_PI_4` at the eight axis and diagonal directions, and `PI` is bit-for-bit
  `4.0 * FRAC_PI_4`, so `d` is exactly `0.0` there. An equally faithful-looking
  `|sin 4θ|` instead returns values of order `1e-16` at seven of the eight, and an
  edge built from `cos`/`sin` rather than from an integer offset breaks exactness for
  every formulation. The formula above is the specified one, and the gate's eight
  edges are **unit cell offsets**. Measured on this machine, all eight give exactly
  zero — but three of them give **negative** zero, from exactly the three offsets with
  `Δj < 0`. `assert_eq!(d, 0.0)` passes on `-0.0` and a bit comparison does not, so
  the gate is an equality assertion, never `to_bits`. That also covers the
  implementation that normalises `θ` into `[0, 2π)` first, as `c3` needs anyway,
  which yields `+0.0` at all eight.

  One caveat, recorded rather than guarded: exactness at the eight directions rests
  on the platform's `atan2` being correctly rounded at exact integer ratios. It holds
  here and on glibc. It is the one thing in this phase that could behave differently
  on another target, and a failure there is a gate to re-key, not a bug to chase.

*(These five are this spec's operational definitions, not quotations from the 2011
paper. They implement the criteria that paper names — the table is its vocabulary —
but the exact functional forms are chosen here, and Phase 2's gate checks the
properties each one must have rather than fidelity to a source nobody in this repo
has yet re-read. OQ-1's paper re-read, which Phase 3 needs anyway, is the occasion to
reconcile them; a difference found then is a change to these formulas, recorded as
one, not a defect in the phase that shipped them.)*

### 2.4 Hill-climbing

A fixed iteration count and **no randomness**, so a given input and parameter set
always produce the same map. Per iteration, for each station **in input order**
(§2.2): test every free grid point within a movement radius `r`, and move to
whichever lowers `t` most. `r` starts large and shrinks to one cell over the run.

**`r` is a Chebyshev ring count, not a Euclidean radius**, and the candidate set at
radius `r` is every cell in rings `1..=r` of §2.2's spiral — which is what makes
"the earlier cell in the spiral order" below a well-defined tie-break rather than a
reference to a different enumeration. The cooling law is **linear and integral**:
`r_k = max(1, round(r_0 * (1 - k / iterations)))` at iteration `k`, from
`r_0 = LayoutParams::initial_radius` (default **3**) over
`LayoutParams::iterations` (default **200**). Both are stated here rather than left
to the implementer because both are serde-visible and Phase 6 derives a flag from
each.

**Reaching a fixed point early is correct behaviour, not a stall**, ~~and nothing
detects it and nothing stops early~~ **— and the search stops when it reaches one.**
A hill-climb that can find no improving move for any station has converged, and on a
network already near its optimum that can happen inside the first sweep. The iteration
count is a bound, not a target, and a run that does nothing after iteration 1 has still
produced the right map. The reversal is OQ-9's, argued below.

Determinism needs one more rule than "no randomness" gives it: when two candidate
cells lower `t` by the same amount, **the earlier cell in §2.2's spiral order wins**.
Equal-cost candidates are common rather than exotic — a symmetric neighbourhood
produces them constantly — so leaving the tie to whichever the enumeration happened
to reach first makes the output depend on iteration order, which is the thing §2.2
went to trouble to fix.

Three move rejections keep the network from tearing:

- the target cell is occupied;
- the move flips the clockwise order of the station's connected edges;
- the move makes one of the station's edges **exactly overlap** another edge —
  collinear and sharing more than a single point.

The third is a rejection in its own right and not a narrowing of either other, which
is why it is counted here rather than folded into OQ-1's paragraph below. It has its
own geometry and its own pair set, both pinned below.

**The order-flip predicate, pinned.** Let the station's neighbours be ordered by
`geometry::direction` — the same `[0, 2π)` normalisation `c3` uses, ties broken by
neighbour station index, so the sequence is the one §2.3 already made deterministic.
A move is rejected when the resulting sequence is **not a rotation of** the sequence
before it. Cyclic and not positional: "clockwise order" is a property of a cycle, and
a station whose whole fan rotates by one position has not torn anything — its edges
still meet in the same rotational order, which is the thing the rule protects. The
positional reading rejects 4.6× as many moves for no stated reason.

Two consequences follow and are stated rather than left to be discovered:

- **The rule is vacuous below degree 3.** One neighbour has no order, and with two,
  every sequence is a rotation of every other. That is correct — you cannot flip a
  cycle of two — and it means the rule constrains only junctions. On the OQ-5 fixture
  that is 4 of 17 stations.
- **It is evaluated on the station's own incident edges only**, not on the whole map.

**OQ-1 resolved (2026-08-14): an ordinary crossing is *not* a hard rejection.** Only
exact overlap is — the third bullet above. Ordinary crossings are left entirely to
the soft `c1` penalty.

The argument is the asymmetry this spec already recorded: build the hard-rejection
reading when the source means the soft one, and the layout freezes early with edges
it was never allowed to improve through — whereas building the soft reading when the
source means the hard one costs a map with a crossing that `c1`, weighted 5.0 and the
heaviest term in §2.3, is already pushing out. The failure modes are not comparable,
so the reading that cannot freeze the search is the one to build. See OQ-1 in §3 for
what remains open about it.

*(**CORRECTED 2026-08-17, at Phase 7.** "the heaviest term in §2.3" was true when it was
written and is not now: Phase 7 raised `w_octilinearity` to 10.0 against `w_crossings`
5.0. **The argument is untouched**, because it was always about what one crossing costs,
and the two criteria are not commensurable — `c1` counts, so a crossing costs 5.0 flat,
while `c5` sums a deviation of at most `π/8` an edge, so the most expensive single
off-angle edge costs `10 × π/8 = 3.926991`. A crossing is still the dearer defect. Read
the sentence as "weighted 5.0, and dearer per occurrence than anything else in §2.3".)*

**The overlap predicate's pair set, pinned — and it is deliberately *not* `c1`'s.**
Test every edge incident to the moved station against **every other edge in the
graph, including edges that share an endpoint with it**. `c1` excludes
endpoint-sharing pairs (§2.3), and that exclusion is exactly what makes this a
separate rule worth having: a line folding back so that one edge lies along its own
neighbour is invisible to `c1` by construction, and it is the only case that answers
to "a degenerate no penalty can distinguish from a legitimate drawing".

Mirroring `c1`'s pair set instead would make the rule fire only where `c1` is already
charging 5.0 — leaving the fold-back it exists for unrejected, and making the
rationale above false. The two readings are 10× apart in rejections on the OQ-5
fixture at the pinned defaults (3351 against 312), a wider gap than the one that
justified pinning the order-flip predicate two paragraphs up, so this is not a
detail an implementer can be left to settle.

**`geometry::segments_intersect` is the wrong tool here, and reaching for it breaks
two things at once.** §2.3 built it as a *closed* test that deliberately counts
touching — its own test asserts true for two collinear segments meeting at exactly
one endpoint — so it returns true for every legitimate straight-through, forbidding
the collinear moves this layout most wants, and true for ordinary crossings, silently
reinstating the hard-crossing rejection OQ-1 just decided against. The predicate here
is collinearity via `geometry::orientation` plus a **strict** interval overlap. That
is exact `i128` arithmetic already in the tree, with no epsilon to tune.

**Cluster moves.** This exists for a specific dead end: a tight cluster attached to
the rest of the map by a single long edge cannot shorten that edge by moving any one
of its stations, so single-station hill-climbing is stuck at a local minimum it cannot
see out of.

A **cluster** is one side of a **bridge** — an edge whose removal **increases the
number of connected components**, which is the reading a Tarjan-style finder gives and
which stays well defined on the disconnected input §2.1 admits — and specifically the
**smaller** side, ties broken by the lower minimum station index. The two sides of a
bridge are disjoint and non-empty, so their minimum indices always differ and that tie
rule is total. Sides of fewer than two stations are dropped: a single station is what
the per-station sweep already moves, and a cluster move over one is the same move made
more expensively. Bridges are a property of the **graph alone**, not of the layout, so
the cluster set is computed **once, before the first sweep, and never recomputed** as
stations move. A graph with no bridges — a pure cycle — yields no clusters and an inert
pass, which is correct.

**Clusters nest; they are not a partition**, and that is worth stating because the rule
this replaced *was* one. On the fixture the seven sides stand in seven containment
relations — `{northgate, hillcrest}` inside the four-station side inside the five
inside the six — so a single station is translated by several clusters over one pass.
An implementation that dedupes or partitions to "tidy that up" silently drops the
clusters that would have fired.

A cluster is translated rigidly — every member by the same offset — over the same
candidate offsets and the same cooling radius a single station gets, accepted under
the same strict-improvement rule and the same spiral tie-break. **The pass runs after
the per-station sweep, inside the same iteration**, over clusters in the order their
bridges appear in the corridor list, which is input order (§2.2).

**Exactly one edge changes geometry under a cluster move**, because a bridge is the
only edge joining its two sides and a rigid translation preserves every edge inside a
side. That is what makes the three rejections cheap to state for the group case, and
each needs stating, because all three are written above for a single station:

- **Occupancy is read with the cluster's own cells vacated.** A target cell counts as
  free when it is free *or* when a member of the same cluster currently holds it. A
  translation is injective, so members cannot collide with one another and this is
  exactly the remaining condition. Without the clause, a translation by one cell is
  rejected by the cluster's own body and no cluster move can ever fire.

  **Applying an accepted move needs an order, and there is one that needs no new API.**
  `GridOccupancy` has no bulk or release method — `relocate` debug-asserts a free
  target — so relocating members in arbitrary order trips its own precondition. Move
  them in **decreasing projection along the offset**: the front of the cluster goes
  first, into cells the occupancy check has already proved free, and each later member
  lands on a cell its predecessor has just vacated. That keeps `grid.rs` out of this
  phase entirely.
- **The order-flip rule is evaluated at the bridge's two endpoints only.** Every other
  station's fan is rigidly translated and therefore unchanged.
- **The overlap rule is evaluated on the bridge edge alone**, against every other edge
  in the graph, with the pair set pinned above — and **the reason is not the one above
  it**. Overlap is a pairwise *positional* property, so an intra-cluster edge does move
  relative to the rest of the map even though its own shape is preserved; "only one
  edge changes geometry" does not by itself license dropping those pairs. What licenses
  it is that an intra-cluster edge has both endpoints inside the side and an external
  edge has both outside, so **such a pair can never share an endpoint** — which makes
  the dropped pairs exactly `c1`'s pair set, where `c1`'s closed test already charges a
  collinear overlap at weight 5.0. That is OQ-1's soft/hard split applied one level up,
  and it leaves a stated asymmetry: a configuration hard-rejected when a single station
  creates it is only *priced* when a cluster move does.

#### Why a bridge and not "edges shorter than `2g`" (decision, recorded)

The original rule was "stations joined by edges shorter than `2g`", written when `g`
was an externally chosen constant and a typical edge spanned many cells. §2.2's derived
default destroyed it, and not by a margin worth re-tuning — see OQ-7, which carries the
measurements. The short version: under the derived `g` **every** edge of the fixture is
under `2g`, so the whole network is one cluster; all five criteria are
translation-invariant, so translating a whole connected component leaves `t` bit-identical;
and the search accepts only strict improvements. The step could therefore never fire.

No threshold repairs that, because the quantity is wrong rather than the constant is:
after snapping, edge lengths cluster tightly around `L` by construction — the fixture's
seventeen corridors measure 1.0 or 1.41 cells and nothing else — so no cut of that
distribution separates a cluster from the rest of the map. A bridge is **structural**,
parameter-free, and cannot degenerate to a whole component by definition, which is
exactly the guarantee a length threshold could not give.

#### Why the search stops early (OQ-9, decided 2026-08-15 at Phase 4)

The struck clause above was written to say that early convergence is **correct**, and
that much is untouched. What it also asserted — that the remaining sweeps must actually
execute — the induction here makes pure cost, so the search exits instead.

If an iteration moves nothing **in either pass**, positions and occupancy are unchanged
entering the next; the cooling radius is non-increasing, so that iteration's candidate
set is a *subset* of the one just exhausted over an identical layout; and clusters are a
function of the graph rather than of the positions, so the cluster pass sees the same
set. It therefore also moves nothing, and by induction so does every iteration after it.
**The exit is output-identical, not merely output-similar.** Measured: on the fixture,
`iterations` of 1, 2, 3, 10 and 200 give bit-identical positions at `t = 11.338720`, and
the run executes 2 iterations rather than 200.

**The test is the whole iteration's and not the per-station sweep's**, and that
distinction needs an assertion of its own rather than a sentence. A cluster can have an
improving move at a layout where no single station does — that is the dead end cluster
moves exist for — so an exit keyed to the station sweep alone stops one pass short of the
fixed point and produces a different map. It is invisible on both committed fixtures,
where the two passes converge inside the same sweep, and every other test in the
workspace stays green under the wrong reading. What separates them is re-entering the
search from the layout the single-station search settles at: there the station sweep
moves nothing by construction while the cluster pass moves `parkview`/`lakeside`.

**The other half of OQ-9 is not answered here** — the whole-network rescore per candidate
stands, and §3 carries both the measurement and the reason the shallow fixes were left.

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
  both station lists. This is a topology check, and it is **already computed** —
  `LineSet` on the graph edge is exactly this, built at parse time. The renderer
  reads it rather than re-deriving shared pairs from the station lists, on the same
  grounds Phase 3 gave for occupancy: two structures answering one question is how
  they come to disagree. It will not catch an express line that skips stations along
  a corridor it visually shares — an accepted v1 limit, stated here so a reviewer
  does not report it as a bug.
- **A collapse station is one where the line set changes, or where degree ≠ 2.** At
  one, every offset is zero. Bundled lines therefore merge to a single point exactly
  at real interchanges and stay separate along a shared trunk.

  **`≠ 2` and not `> 2`**, which is wider than this section first said and deliberately
  so: a *terminus* shared by two lines is degree 1, and a bundle that ran to the end of
  the track without converging would draw two separate stub ends at one stop. Invisible
  on the fixture, where every degree-1 station carries a single line.
- **A run is a maximal path of consecutive edges carrying the same line set, and it
  breaks at every collapse station.** Both halves are needed: without the second, the
  fixture's whole trunk is one run whose *interior* contains `central`, and "each line
  keeps one fixed offset across the whole run" then contradicts the collapse rule two
  bullets up. Its interior stations are not collapse stations.

  Its **endpoints** are, with two exceptions that are legal input rather than corner
  cases, both reachable because §2.1 rejects only *consecutive* repeats. A closed cycle
  of degree-2 stations carrying one constant line set has no collapse station to break
  it, so the run has **no** endpoints. And a line that revisits a station — `[X, a, b,
  X, c]` — drives `X` to degree 3, making it a collapse station, so the run `X–a–b–X`
  has **two endpoints that are the same station**. The rule below that needs a direction
  says what to do in both; each carries `n = 1` in any realistic instance, so what is at
  stake is totality rather than a picture.
- Within a run each line keeps **one fixed perpendicular offset**, so a line never
  visually swaps sides mid-run. For a run carrying `n` lines, the line at index `k`
  — 0-based, in the run's sort order — takes signed offset `(k − (n−1)/2) · s`, so the
  bundle straddles the corridor's centreline symmetrically and `n = 1` falls out as
  zero with no special case.
- **The sort key is the line `id`, the string**, which is why §2.1 rejects a duplicate
  one — that error's stated reason is this sort being total. Note it is *not* input
  order, which §2.2 makes the rule everywhere else: on the fixture the ids sort
  `blue < green < red` where input order is `red, green, blue`, so the two readings
  mirror the trunk. Both are deterministic; this one is named because the structure an
  implementer has in hand, `LineSet`, is a set of line *indices* and yields the other.
- **The side a positive offset lies on is a property of the run, not of any line's
  traversal.** Direct the **whole run** from its lower-indexed endpoint station to its
  other endpoint; every corridor in it inherits that direction, and the offset is
  along the left normal `(dy, −dx)/|(dx, dy)|` of `(dx, dy)` in SVG user space.

  Both exceptions above take the same fallback, which is why it is stated once: where a
  run has no endpoints, or two that are the same station, direct it from its
  **lowest-indexed station towards whichever neighbour along the run has the lower
  index**. Either choice draws a valid mirror of the other; what matters is that one of
  them is named.

  Two things need this, and the second is why it is the **run** rather than the
  corridor. Two lines that walk a shared corridor in opposite list order — legal, and
  unremarkable on a real network — would otherwise compute opposite normals and land on
  the same side. And the mitre below needs the two corridors at a bend expressed in one
  frame; a per-corridor rule gives adjacent corridors independently-signed normals, and
  the bisector of those is meaningless.
- **An offset belongs to a (line, position-in-that-line's-list) pair, not to a
  (line, station) pair.** §2.1 rejects only *consecutive* repeats, so a line may
  legally visit one station twice, at two different offsets. Not reachable on the
  committed fixture; cheap to get right and expensive to retrofit.
- Each line renders as **one** SVG path across its full station list, not one path
  per edge, so corner rounding comes free from `stroke-linejoin="round"`.
- **Markers do not move.** They stay circles at the bare cell centre. At the default
  spacing a two-line bundle spans `±stroke_width` and the marker radius is
  `0.9 · stroke_width`, so the marker sits inside the pair, centred on the seam, and
  still reads as one stop serving both. At `n ≥ 3` it no longer covers the outer
  strokes — an accepted v1 limit, unreachable on the fixture, and a per-line marker is
  a different design rather than a tweak to this one.

#### Where the offset points at a bend, and how big it is (decision, recorded)

Two numbers the rules above leave open, and an implementer cannot draw a single
stroke without both.

**The magnitude is `RenderParams::bundle_spacing`, an `Option<f64>` in SVG user
units** — the perpendicular distance between *adjacent* lines of a bundle. `Some(u)`
uses `u`; **`None`, the default, derives it as `stroke_width`**, which puts two
strokes exactly adjacent, touching without a gap or an overlap. It is `Option` rather
than a constant for §2.2's reason one subsystem over: a fixed default is wrong the
moment `stroke_width` changes, and a bundle whose spacing is half its stroke width
draws as one thick smear. `Some(0.0)` disables bundling and is the seam the exit gate
measures against, the role `iterations = 0` played for Phase 3 and `cluster_moves:
false` for Phase 4. It is serde-visible, so Phase 6 derives a flag from it like every
other field (§2.6).

**At an interior station of a run the corridor can bend, and the offset point is the
mitre** — the intersection of the two offset lines, which is what keeps the parallel
distance exactly `s` on both sides of the corner rather than pinching it. With `n₁` and
`n₂` the two corridors' left normals **in the run's direction** (the bullet above is
what makes them comparable), the offset direction is `normalize(n₁ + n₂)` and the scale
is `1 / cos(θ/2)`, `θ` being the turn angle. A straight-through gives `n₁ = n₂`, the
bisector is `n₁` and the scale is exactly 1.

**The scale is clamped at 4, and the clamp is load-bearing rather than defensive.** An
earlier draft of this section argued the factor was bounded because the layout is
octilinear, and that argument was wrong in three separate ways — recorded here because
the wrong version is the one a reader is likely to reconstruct:

- `1/cos(θ/2)` **increases** with `θ`, so a *lower* bound on the turn bounds it from
  below, not above. The figure `1/cos(67.5°) ≈ 2.61` is θ = 135°, the sharpest
  non-reversing octilinear corner, not θ = 45°.
- **Octilinearity is not an invariant.** §1.1 says in terms that this design leaves some
  edges off-angle, so "interior angle ≥ 45°" is not a property of a shipped layout. The
  hazard is the near-anti-parallel neighbourhood, where no exact-reversal special case
  fires and the factor diverges: on the integer grid, neighbours at `(5,0)` and `(5,1)`
  give 11.31° and a factor of **10.15**; at `(20,0)` and `(20,1)`, 2.86° and **40.04**.
  `c3`, `c4` and `c5` price those shapes. Nothing forbids them.
- **§2.4's overlap rejection is a move filter, not a layout invariant.** It runs only
  when a candidate move is evaluated. `grid.rs`'s snap claims cells and checks nothing
  else, and `iterations = 0` — a supported mode, and Phase 3's own baseline — evaluates
  no candidate at all, so a fold-back present at the snap is never removed.

So the bound has to be imposed rather than inherited. **Clamp the scale at 4.**

The number is **borrowed** from SVG's `stroke-miterlimit` default rather than derived
from it, and the difference is worth one sentence because the looser claim was written
first: SVG *bevels* past its limit where this clamps, and its ratio is against
`stroke-width` where this one is against `bundle_spacing` — the same quantity only while
`bundle_spacing` is `None`. So 4 is a sanity anchor from a renderer that faced the same
trade, not a value this rule inherits. What justifies it here on its own terms: at `4 · s`
the outer stroke of a two-line bundle sits `2 · s` off the centreline, which is bounded
and still reads as a corner, and beyond it the join reads as a spike.

It is a named constant and deliberately **not** a `RenderParams` field: the tunable
surface is what §2.6's sliders bind to, and a degeneracy guard is not a thing anyone
tunes — the same call `FALLBACK_GRID_SPACING_M` is.

The clamp also **subsumes the anti-parallel case instead of special-casing it**. Where
`n₁ + n₂` is zero the direction is undefined; take `n₁`. Everywhere else the clamp makes
the function bounded and continuous approaching that point, which a discontinuous
special case at exactly 180° would not.

The consequence of getting this wrong is concrete and not cosmetic: `Viewport::new`
sizes the document from the *station* extents plus the margin, and the existing render
test bounds-checks station points only — so an unclamped mitre puts a path vertex
outside the `viewBox` and nothing in the suite sees it.

**One consequence of the collapse rule, measured on the fixture and stated rather than
discovered at the gate: a corridor whose *both* endpoints are collapse stations draws
with zero offset at both ends, so its lines overprint exactly as they do today.** On
`sample_network.json` the four trunk corridors split 1 / 2 / 1: `oldtown`–`eastbank`
is genuinely parallel, `riverside`–`oldtown` and `eastbank`–`central` splay from zero
to full offset, and **`central`–`market` overprints entirely**, because `central` is
degree 4 and `market` degree 3. That is the same lens OQ-5 lengthened the trunk to
avoid, one corridor over, and it is an accepted v1 limit: separating it needs offsets
that taper into a station rather than collapsing at it, which is a different renderer.

### 2.6 API shape for reuse (decision, recorded)

`Network::from_input`, `run_layout` and `render_to_string` are each public in their
own right, not only reachable through the `build_schematic_svg` convenience
function. `LayoutParams` and `RenderParams` are plain `Serialize`/`Deserialize`
structs with a `Default` impl.

*(Recorded 2026-08-14, at Phase 2. The five cost weights are **named for what they
weigh** — `w_crossings`, `w_edge_length`, `w_angular_resolution`, `w_straightness`,
`w_octilinearity` — rather than `w1`-`w5`, which is how §2.3 and Phase 2's scope word
them. The names are serde-visible, so they are the key names in Phase 6's `--params`
file, and Phase 6 derives a flag from every field: `w1` would give `--w1`, while §1's
own end-state invocation types `--w-crossing`. The `w1`-`w5` spelling stays in the
prose, where it is the shorter name for a criterion rather than an identifier.)*

*(That tension is **resolved at Phase 6**, in its favour of the field name: every flag
is its field kebab-cased, with no exceptions, and §1's block is corrected instead. The
argument is that one irregular flag forces Phase 6's field-to-flag gate to consult the
same mapping the implementation does, which makes it assert that the code agrees with
itself. Recorded here because this is the section that raised it and left it open.)*

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

- `llika-core` — library. Data model, projection, layout, renderer.
- `llika-cli` — thin binary. Reads a network file, calls `llika-core`, writes SVG.
  Its binary is named `llika`, which is §1's invocation.

*(Corrected 2026-08-14, at Phase 1's close-out. This section and both source
documents said `metro-core` / `metro-cli`; the crates are named for the project
instead, since nothing in the pipeline is specific to a metro — a tram or bus
network is stations and ordered line lists too. The correction is recorded rather
than made silently because the names were a stated design decision, and `rules/`
cites these paths as `file:symbol`. It applies **document-wide**: every path
elsewhere in this spec was updated with it, including in phases already shipped,
because a path that no longer resolves is a broken fact rather than a superseded
decision.)*

```
llika-core/src/
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
    cluster.rs    bridge-side clusters and rigid moves
    hillclimb.rs  iteration loop, cooling schedule
  render/
    mod.rs        RenderParams, render_to_string
    corridor.rs   line-bundling
```

Dependencies: `petgraph`, `svg`, `serde`/`serde_json`, `clap`, `thiserror`. No
projection crate and no force-directed-layout crate — §2.2 gives the reason for the
first, and this algorithm is a bounded grid search rather than a physics simulation.

*(This spec still carries no `file:symbol` citations, though Phase 1 has shipped and
the tree now holds citable symbols. That is deliberate: the spec records decisions
and does not track the code, so `rules/` is what cites symbols — all three files
seeded at Phase 1's close-out do. A citation added here would rot in exactly the way
§8.1 of the methodology describes, with nothing regenerating against it.)*

## 3. Open questions

- **OQ-1** — ~~Is "does not cross another edge" a hard move rejection, or only a hard
  rule against exact overlap with ordinary crossings left to the soft `c1` penalty?
  *(needs-input — a direct re-read of the 2011 TVCG paper.)* **Blocks Phase 3**; it
  changes what `candidate.rs` implements. This is the one open question that can
  produce a wrong implementation rather than a late one: build the hard-rejection
  reading when the paper means the soft one and the layout freezes early with edges
  it was never allowed to improve through.~~

  **DECIDED 2026-08-15, in §2.4, as this spec's own call rather than as the paper's.**
  Exact overlap is a hard rejection; ordinary crossings are left to `c1`. The
  reasoning is in §2.4 and turns on the asymmetry the struck text above states: the
  two wrong answers have very different costs, and only one of them can freeze the
  search.

  **What is closed and what is not.** The *implementation* is closed — Phase 3 shipped
  the soft reading on 2026-08-15, in `layout/candidate.rs`. The *reconciliation* is
  still open: nobody in this repo has read the 2011 paper, so this is an operational
  decision in exactly the sense §2.3's five formulas are, and the paper's actual rule
  may differ. Reading it remains worth doing, and a difference found then is a recorded
  change to §2.4, not a defect in the phase that shipped it.

  The precedent is deliberate. Phase 2's round 1 found four criteria with no formula
  anywhere and closed them the same way rather than waiting on the same unread paper;
  leaving this one `needs-input` would block the project's central phase on a
  dependency nothing in the tree can discharge.
- **OQ-2** — Starting values for `w1`-`w5`. The paper gives none, and the five
  criteria have different natural scales. *(design call.)* Blocks nothing
  structurally; the first defaults are a starting point to tune by eye against the
  fixture, and Phase 3's visual gate is the first place they are judged. Recorded so
  a later pass does not mistake the first numbers for settled ones.

  **Judged once, 2026-08-15, at Phase 3's visual gate, and left unchanged.** The map
  the provisional weights produce reads as a transit diagram: `riverside` reaches
  `c3`'s degree-3 floor exactly, the Blue line's fold at `quayside` straightens into
  an axis run, and no line kinks where nothing forces it. There was nothing to correct
  by eye, so 5.0 / 1.0 / 1.0 / 2.0 / 5.0 stand — **still provisional, not settled**.
  Phase 5 redraws the same layout with bundling and is the next honest occasion; a
  weight judged against an unbundled picture is judged against half of it.

  **One measured consequence of these weights, which Phase 6 needs before it designs a
  flag around it: `initial_radius` is inert.** Found by a code review after Phase 3
  shipped and reproduced independently — `r_0` of 1, 2, 3, 5 and 8 all give
  **bit-identical positions** on the fixture, at `t = 22.505867` with the same 5
  movers, while costing `O(r²)` candidates each. The cause is `c2`: at `L = 1.0` cell,
  `(|e|/L − 1)²` prices every ring-≥2 move out of contention before any other criterion
  is consulted. Zeroing `w_edge_length` makes the radius matter again, which is what
  identifies `c2` as the cause rather than the search.

  This is a **weights** finding and not a search one, which is why it is recorded here
  rather than in OQ-9. It leaves Phase 6 about to expose `--initial-radius` as a knob
  that, at the shipped defaults, does nothing but multiply the runtime by ~9. Either
  the weights change, or the flag ships with that stated — but not neither.

  **Phase 4's review round re-measured this and it is unchanged by cluster moves**: with
  them on, `r_0` of 1, 2, 3, 5 and 8 still give bit-identical positions. Worth recording
  because a cluster translation prices only one edge under `c2` and might plausibly have
  revived the knob; it does not.

  **Re-judged 2026-08-15 at Phase 5's visual gate — the redraw this question nominated —
  and they stand a second time.** 5.0 / 1.0 / 1.0 / 2.0 / 5.0, unchanged. This is the
  occasion the entry above called "the next honest occasion", on the grounds that a weight
  judged against an unbundled picture is judged against half of it; the bundled half is now
  drawn and it does not change the reading. The junctions still fan evenly — `central`
  reaches `c3`'s optimum outright at 0/90/180/270 across four corridors — and the only bend
  at a degree-2 station is Blue's at `southgate`, which the geometry forces rather than the
  weights permitting.

  **What the bundled picture newly shows is not a weights matter**, which is the finding
  worth recording rather than the verdict. The trunk reads as a lens: `oldtown`–`eastbank`
  is genuinely parallel, while `riverside`–`oldtown` and `eastbank`–`central` splay from
  zero to full offset. §2.5's last decision predicted exactly that split and priced it as an
  accepted v1 limit, and no setting of `w1`-`w5` touches it — the shape comes from the
  collapse rule's geometry, not from what the search optimised. Re-weighting to chase it
  would be tuning the wrong subsystem.

  **Still provisional, and now for a stated reason rather than for want of an occasion.**
  Two judgements, both by eye and both by the same party, are not a calibration; the
  measurement that would settle these numbers is a second network with a different shape,
  which §1.1 puts out of v1 scope. Phase 6 exposes them as flags, which is what lets someone
  else disagree cheaply — and that is the honest end state for a value nobody has yet had a
  way to falsify.

  **JUDGED A THIRD TIME 2026-08-17, against BART, and this time the numbers did not
  stand. 5.0 / 1.0 / 1.0 / 2.0 / 5.0 are dominated and should change.** The entry above
  named the missing measurement as "a second network with a different shape, which §1.1
  puts out of v1 scope". `llk-002` shipped and that bar is gone: BART is committed, it is
  50 stations against the fixture's 17, and it is the first network in this repo capable of
  falsifying these values. It did.

  **The comparison is by criteria vector, never by `t`.** `t` is *defined* by the weights,
  so it is not comparable across settings; the only weight-free comparison available is
  Pareto dominance over the five unweighted `c1..c5`, and every number below is that. All
  were measured against the shipped `layout::cost::evaluate` at
  `llika-gtfs/tests/fixtures/bart.zip --route-types 1`.

  | | `c1` | `c2` | `c3` | `c4` | `c5` | octilinear |
  |---|---|---|---|---|---|---|
  | snap only (`iterations = 0`) | 12 | 47.888 | 124.486 | 131.686 | 3.141 | 40/50 |
  | **shipped 5/1/1/2/5** | 0 | 7.576 | 46.316 | 27.489 | 0.644 | **48/50** |
  | 5/1/1/0.5/10 | 0 | 1.373 | 29.322 | 18.850 | **0.000** | **50/50** |
  | 5/1/0.5/0.25/10 | 0 | 0.515 | 24.609 | 15.708 | **0.000** | **50/50** |

  **176 of 324 settings in a coarse five-dimensional grid dominate the shipped defaults on
  all five criteria at once.** Not a marginal miss — over half of an unselective grid beats
  them outright, which is what makes this a correction rather than a preference.

  **The grid, because a number nobody can rebuild is not evidence.** Round 1 could not
  reproduce this figure and said so; two independent reconstructions gave 184 and 175, and
  a third 103 of 180 — consistent in magnitude, but not the same experiment. It is
  `w1 ∈ {2.5, 5, 10} × w2 ∈ {0.5, 1, 2} × w3 ∈ {0.5, 1, 2} × w4 ∈ {0.25, 0.5, 1, 2} ×
  w5 ∈ {5, 10, 20}` — 3·3·3·4·3 = 324 — each cell run through `run_layout` on BART and
  scored by `layout::cost::evaluate` at the resulting positions.

  **"Dominate" is `≤` on all five and `<` on at least one**, with a `1e-9` guard. It has
  to be: the shipped point already has `c1 = 0`, so strict improvement on all five is
  unattainable by construction, and a reading that demanded it would score zero cells and
  conclude the opposite.

  **The mechanism is the `w5:w4` ratio, and it is a threshold rather than a gradient.**
  Holding `w5 = 5` and walking `w4`, BART reaches `c5 = 0.000` at `w4 ≤ 0.5` and sits at
  `c5 = 0.644` for every `w4 ≥ 0.75` — the shipped 5:2 = 2.5 is on the wrong side of a cliff
  at roughly 10. `c4` and `c5` are the two criteria that both price what an edge's *angle*
  does, and at the shipped ratio `c4` outbids `c5`: the search buys a straight line through
  a station by paying for an off-angle edge, and two corridors never recover. Raising `w5`
  with `w4` held reaches `c5 = 0` too, but only at `w5 ≥ 15`, and every such layout carries
  `c1 = 1` — a crossing, the most visually damaging thing on the map. Lowering `w4` gets
  there with `c1 = 0`, so **that is the direction to take**.

  **Why neither prior judgement could have caught it, which is the finding that matters
  most: it is OQ-8's fixture property, striking a second time.** OQ-8 records that
  `sample_network.json` snaps to `c5 = 0.0` *exactly* — all 17 corridors already octilinear
  before any search runs. A fixture whose `c5` is zero at the baseline cannot exhibit a
  weight that under-prices `c5`; there is no error left for the criterion to fail to
  correct. Both earlier judgements were taken on the one network in the tree structurally
  incapable of showing this defect, and the Phase 5 entry's "the bundled half is now drawn
  and it does not change the reading" was true and still blind for the same reason. **The
  lesson generalises past the weights: a gate keyed to this fixture is weaker than it
  looks**, and OQ-8's cheap resolution — assert non-decreasing octilinearity — bought less
  coverage than the expensive one would have.

  **Two candidates, both verified by eye at `gallery/`-equivalent renders.** `5/1/1/0.5/10`
  is the minimal change — it moves only the ratio the mechanism names, leaves `w1`, `w2`
  and `w3` alone, and is the conservative pick. `5/1/0.5/0.25/10` is better on every
  criterion and drew the best picture of anything tested: the doubled-back Red line inside
  the western trunk resolves, one 45° connector survives so the map does not read as a
  circuit board, and it is fully octilinear with no crossings. Both leave
  `sample_network.json` **byte-identical** — 0 of 17 stations move — and both leave the
  `llika-gtfs` fixture feed byte-identical too.

  **That byte-identity is luck and must not be sold as structure.** `5/1/1/0.25/10` — one
  step away from the first candidate — moves 10 of the 11 stations in the GTFS fixture, and
  `5/1/0.25/0.5/10` moves 5 of them. The search is chaotic in the weights at this scale, so
  "no gate literal changes" is a property of the two settings named and not of the
  direction. Any other value needs the same check run again.

  **A correction to this question's own `initial_radius` finding, which was over-general.**
  The entry above records `r_0` of 1, 2, 3, 5 and 8 giving bit-identical positions, and
  concludes the knob is inert. On the fixture that reproduces exactly. On BART it is false:
  `r_0 = 1` differs from `r_0 ≥ 2` in all 50 stations. So what the fixture
  measurement actually established is a saturation, not an inertness, and the two got
  conflated because the fixture's live step happens to be a no-op there.

  **A first draft of this correction then over-generalised in exactly the way it was
  correcting**, saying the knob is "saturated above [2] on both networks and at every
  weighting tested". Round 1 falsified that too: on BART at the *shipped* weights, `r_0` of
  3, 5 and 8 each differ from `r_0 = 2` in **8 of 50 stations** while reaching an identical
  `t = 112.087766`. That is cost-saturation, not position-saturation. Position-saturation
  above 2 holds at both candidate weightings and on the fixture at all three — so it is a
  property the reweight *creates*, and any rule or help text asserting it is describing the
  post-change tree. Recorded because two successive attempts to state this knob's behaviour
  both reached further than the measurement did.

  Worse for the shipped values, and better for the candidates: **at 5/1/1/2/5 the default
  `initial_radius: 3` is the wrong side of that step.** `r_0 = 1` reaches `t = 84.332220`
  where `r_0 = 3` reaches `112.087766` — the default explores more and lands worse. At both
  candidate weightings the ordering is the intended one (`r_0 ≥ 2` strictly better:
  `16.746281` against `36.517509` at `5/1/0.5/0.25/10`), so the same change that fixes the
  weights also makes the radius default behave as its own doc-comment claims.

  **Unresolved, and deliberately so.** This is one more network, not a calibration — the
  third judgement by the same party, and the first that could fail. What is settled is
  *negative* and needs no further evidence: 5/1/1/2/5 is dominated, by a mechanism that is
  identified and reproducible. Which replacement is right is a weaker claim resting on one
  city, and picking the grid's argmax would be exactly the overfitting this question has
  warned about twice. The change belongs in a phase of its own with its own review round,
  because it moves shipped defaults, the `--initial-radius` help text and three `rules/`
  files together. **That phase is Phase 7**, and it does not resolve this question either —
  §4 says so in terms.

  **What would resolve it, and what it is attached to.** A second real network of a
  different shape, imported and drawn, with the criteria vector compared across at least
  three weightings — the measurement this entry has now demanded three times. That is
  `llk-002` Phase 4's shape done once more (a feed, a licence check, a committed fixture),
  and it is named here rather than left floating because the methodology's §4 is explicit
  that an open question with no phase attached is one nothing will ever force. Until such a
  phase exists this question stays open with a *known* answer to its negative half and an
  unforced one to its positive half.

  **Phase 7 shipped `5 / 1 / 0.5 / 0.25 / 10` on 2026-08-17**, this entry's second
  candidate, and left this question open exactly as the paragraph above says. Every figure
  the entry carries was reproduced against the shipped tree at that phase's
  implementation. One was not: the note that "one 45° connector survives" is a visual
  reading, and the layout in fact carries **three** diagonal corridors against 47 on an
  axis — which is the same point about it not reading as a circuit board, counted.

  **That phase now exists: `llk-002` Phase 6, drafted 2026-08-18** — a second city, blocked
  on that spec's OQ-8, which names the feed. *(It was drafted as Phase 5 against Chicago
  specifically; its round-1 review measured the CTA archive at 99.6 MB against BART's
  892 KB and split the work in two — `llk-002` Phase 5 now makes a city feed readable at
  all, and the feed choice went back to an open question. The shape argument that picked
  Chicago was also found wrong in part; `llk-002` Phase 6 records how.)* Its gate produces
  this entry's fourth measurement: the criteria vector at the three weightings the
  paragraph above asks for, compared by Pareto dominance and never by `t`. **It does not
  change the weights** — if the vector says they should move, that is a Phase 8 of *this*
  spec, which is the split Phase 7 used in the other direction. The pointer is what §4
  asks for and what this entry has lacked since it was raised: until now nothing in either
  spec would ever have forced this measurement.

  One thing that phase is expected to settle, which is Phase 7's leftover rather than this
  entry's: **whether `w_crossings` is pinnable at all.** Every weighting that survives
  Phase 7's gate reaches `c1 = 0` on both fixtures and on BART, so nothing in this tree
  constrains `w1`. A ring core can cross where a tree of spurs cannot — and a second
  network that also never crosses is itself the answer, in the negative.

  **ANSWERED 2026-08-18, and in a third way neither branch above anticipated: `w1` is
  unpinnable on a network that *does* cross.** Measured locally on Chicago's CTA rail
  network — 141 stations, 146 corridors, imported through the shipped binary at
  `ImportParams::default()`, nothing committed, the provenance in `llk-002`'s OQ-8. It is
  the first network in this tree that crosses at all: the snapped layout carries **25**
  crossings and the search takes it to **4**, all four inside the Loop.

  Then the knob does nothing. `--w-crossings` at **5, 25, 100 and 400** — eighty times the
  shipped default — leaves the count at **4** every time, and so does every structural
  lever: `--initial-radius 8`, `--iterations 200`, `--grid-spacing 600`, and zeroing
  `w_straightness` and `w_edge_length` together. Only the changes that make it *worse*
  move it — `--cluster-moves false` gives 6 and `--initial-radius 1` gives 17. The run
  converges in 14 sweeps and `--iterations 200` is bit-identical to it, so this is a local
  minimum rather than a budget.

  **So the negative half of this entry is now known for a second reason, and the stronger
  one.** It was "no network here crosses, so nothing constrains `w1`". It is now "a network
  here crosses, and `w1` still constrains nothing, because the search saturates before the
  weight can matter". A weighting cannot be calibrated against an outcome the search cannot
  produce at any weight — which means **`llk-002` Phase 6's gate 4 will report `c1 > 0` and
  still not pin `w1`**, and that phase should expect it rather than read it as a finding.

  *(Recorded against this entry rather than §2.3 because it changes no decision. What it
  does change is where the problem lives, and that is **OQ-10** below: this is a move-set
  limitation wearing a weighting question's clothes, and the two would have been confused
  for one more phase without the measurement.)*
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
- **OQ-4** — ~~Which input method follows v1: click-to-place on an interactive map, or
  import from OpenStreetMap / a GTFS feed. *(deferred — explicitly out of v1 scope
  per §1.1.)* Deliberately left open so the core algorithm is built against
  hand-authored data first and neither input path is designed around prematurely.~~
  **RESOLVED 2026-08-15 by `llk-002` Phase 1, in `llk-002` §1.1 and §1.2.** Import,
  and specifically **GTFS**; click-to-place stays out, as a GUI feature under a §1.1
  this spec defers whole. OpenStreetMap is not merely later but a harder problem:
  GTFS *stores* the ordered station list — a trip's `stop_times` sorted by
  `stop_sequence` — where an OSM route relation stores members whose order is
  unreliable, so recovering the list is a sub-problem of its own before any of the
  conversion work begins. `llk-002` §2.7 reserves `import_osm_spec.md` as the sibling
  that carries it.

  The deferral did its job. `llk-002` §1.1 records that the importer writes a **file**
  rather than handing a `Network` to the layout, and one of its three reasons is
  §2.6's boundary here: an importer wired into the pipeline would put a zip decode
  behind the seam this spec built so a desktop app could re-lay-out without
  re-parsing. Designing that input path early is exactly what would have cost it.

  Nothing in this spec changes. `llika`'s invocation, its thirteen flags and its
  `--params` file are untouched, and there is no new subcommand — which is why
  `llk-002` carries a `related:` edge rather than a `supersedes:` one.
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
  `llika-core/tests/fixtures/sample_network.json`. 17 stations, 3 lines and **17**
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
- **OQ-6** — ~~`c2` as described penalizes edges that are not *exactly one grid cell*
  long, which makes the target a network of uniform unit edges. Is the target length
  one cell, or the mean edge length, or a per-edge ideal? *(design call.)* **Blocks
  Phase 2.** The uniform-unit reading fights `c5` on any network whose station
  spacing varies, which is every real one.

  §2.2's derived default weakens the objection without settling it: when `g` is the
  median edge length, "exactly one cell" *is* the network's own typical spacing, so
  the target is no longer an arbitrary constant. It re-opens the moment a user passes
  `--grid-spacing`, which is the case the resolution has to cover.~~

  **RESOLVED 2026-08-14 by Phase 2, in §2.3.** `L = max(1, m / g)` in cells, where
  `m` is the median non-zero projected edge length — the same quantity `g` derives
  from. Three consequences, in the order they were argued:

  - **Under the default `g` it is exactly `1.0`**, because the numerator *is* `g`.
    So §2.2's argument holds verbatim rather than approximately: a typical edge is
    one cell, which is the length `c2` reaches for, and the layout starts near the
    criterion's optimum for any network at any scale.
  - **It self-scales under `--grid-spacing`**, which is the case the resolution had
    to cover. A fixed target of one cell would fuse two unrelated jobs into one
    knob — `g` sets the *quantization resolution* as well as the target, so halving
    it to get finer movement would also halve the target edge and contract the whole
    drawing. Here `g` sets the resolution and the network sets the target.
  - **Clamped at one cell**, because §2.2's occupancy invariant puts every post-snap
    edge at least one cell apart, so a target below one is unreachable. Unclamped it
    induces the same ranking — shorter is better — but with a magnitude that grows
    without bound as `g` does, silently re-weighting `c2` against the other four.

  The per-edge ideal `L_e = |e|_projected / g` was the third candidate and is
  rejected outright rather than deferred: its zero-set is a grid map geometrically
  similar to the projected one, which is *no schematization at all*, and it would put
  `c2` in opposition to `c3` and `c5` instead of alongside them.

- **OQ-8** — **Phase 3's octilinearity gate cannot be met on the 17-station fixture,
  as written.** That gate asks for "the fraction of edges within 5 degrees of a
  multiple of 45" to be *strictly greater* than the same measurement on the Phase 1
  output. Measured at Phase 2, the Phase 1 output already scores **`c5 = 0.0`
  exactly** — every one of the 17 corridors is octilinear — so that fraction is
  already `1.0` and nothing can exceed it. `c1` is likewise already `0`.
  *(answerable now.)* **Blocks Phase 3.**

  The cause is a fixture property nobody specified and nobody noticed: OQ-5's
  stations are spaced roughly 2.3 km apart, `g` derives as their median, and so all
  17 snap onto a 7×5 patch of *unit* cells whose every corridor happens to run along
  an axis or a diagonal. The fixture is a good schematic map before any layout
  intelligence runs on it. That is why Phase 1's picture looked better than its own
  scope text predicted.

  Candidate answers, cheapest first: assert a strict decrease in `t` plus
  *non-decreasing* octilinearity, which is the property the gate was reaching for;
  or key the octilinear assertion to a second, deliberately off-angle fixture; or
  re-author the 17-station fixture with
  coordinates that do not land on a unit lattice — **the expensive one**, since every
  Phase 1 gate literal is keyed to it, including the hand-counted edge total, the
  collision pair and `g` itself.

  **RESOLVED 2026-08-15 by Phase 3's review round, which took the first answer** and
  measured that it holds: octilinearity is asserted **non-decreasing**, and the
  strict-improvement burden moves onto `t`, which falls 37.166633 → 22.505867 on the
  fixture under the default weights. Both numbers were measured against the shipped
  `layout::cost::evaluate` by the round-1 reviewer, independently of the author.

  The round also found the consequence that makes the naive repair fail: **Phase 2's
  golden file retires at this phase**, so after it there is no committed artifact of
  "the Phase 1 output" left to measure a baseline against. The gate therefore names
  `iterations = 0` as the reproducible baseline — it must yield the snap-only layout
  bit-for-bit — which is a property worth having in its own right, since it is also
  what lets a caller ask for projection and snapping alone.

- **OQ-7** — ~~§2.4's cluster threshold is `2g`, chosen when `g` was an externally
  supplied constant. Under §2.2's derived default `g` is the median edge length, so
  **at least half of all edges are `≤ g`** and therefore under `2g` by construction:
  the step meant to group "stations joined by very short edges" would group most of
  the network into one rigid unit and the cluster move would degenerate into
  translating the whole map. *(design call.)* **Blocks Phase 4.** Candidate answers:
  scale the threshold to a fraction of the median rather than a multiple; define it
  against the *shortest* edges by percentile; or key it to absolute metres
  independent of `g`. Recorded rather than resolved because Phase 4 has its own
  fixture and gate, and the right answer is measurable there and guesswork here.~~

  **RESOLVED 2026-08-15 by Phase 4's review round, and none of the three candidate
  answers won.** A cluster is a **bridge-side**, per §2.4. The threshold is gone rather
  than re-tuned, and the round is where it was closed rather than the implementation,
  on Phase 3's OQ-1 precedent: the phase said "resolve OQ-7 first" while nothing in the
  phase could discriminate the candidates, which is a design handed to an implementer
  under the name of a decision.

  **The measurements that killed the threshold**, all against the shipped
  `layout::cost::evaluate` and `run_layout`, and reproduced independently by the round-1
  reviewer and the author:

  - **17 of 17** of the fixture's corridors are under `2g` — not "at least half", every
    one — so the whole network is a single cluster.
  - **All five criteria are translation-invariant.** Shifting every station by `(7, −3)`
    leaves `t` at `22.505867`, bit-identical.
  - The search accepts only strict improvements. Those three together mean the step
    could **never fire**, on either committed fixture, at any weights. It is inert
    rather than badly tuned, which is why re-tuning was the wrong shape of answer.

  **And no threshold could have worked**, which is the finding that rules out candidates
  (a) and (b) rather than merely preferring something else. After snapping, edge lengths
  are compressed against `L` by construction: the fixture's seventeen corridors measure
  **1.0 or 1.41 cells and nothing else**, so there is no cut of that distribution that
  separates a cluster from the rest of the map. Candidate (c), absolute metres, survives
  the argument but reintroduces exactly the constant §2.2 removed.

  **The bridge rule is non-degenerate on committed data**, which is what lets Phase 4's
  gate drop its purpose-built fixture: `sample_network.json` yields **7** bridge-sides of
  two or more stations, and a rigid move of the `parkview`/`lakeside` pair — the side of
  the `university`–`parkview` bridge — by `(1, 1)` takes `t` from **22.505867 to
  16.222682**, a 28% improvement no single-station move reaches. That is measured from
  Phase 3's shipped final layout, so it is a real local minimum and not a contrived one.

- **OQ-9** — **The search rescores the whole network per candidate, and runs sweeps
  that provably cannot change anything.** Raised by a code review of Phase 3 after it
  shipped; nothing here is a defect in that phase, and every number was measured
  against the shipped code. *(design call.)* **Blocks nothing; Phase 4 is where it is
  answerable**, since that phase reopens the same loop for cluster moves and brings
  its own fixture and benchmark; Phase 4's scope names it back, so the pointer resolves
  in both directions. Recorded for the same reason OQ-7 was: measurable there,
  guesswork here.

  Two separate things, and the second is the larger and the more surprising:

  - **Cost.** Every candidate move calls `layout::cost::evaluate` over the entire
    network, and `c1` alone is `O(E²)`, so a run is
    `O(iterations · V · r² · E²)`. Measured on a synthetic 200-station network:
    **72.9 s release.** The 17-station fixture is 1.4 s debug and nobody notices, but
    §1 promises "a real metro network" and London is several times larger again. The
    deep fix is a delta score over the edges a move actually touches — only the moved
    station's incident edges, and its own and its neighbours' fans, can change. The
    shallow fixes are named below and change no output bit: hoist `ordered_neighbours`
    out of the candidate loop (it is invariant across a station's candidates), hoist
    `cost::corridors` out of `evaluate` and `overlaps_another_edge` (it is invariant
    across the whole run), and carry the incumbent total forward between stations
    rather than recomputing it — it is already in hand as the previous station's `best`.

  - **199 of the 200 sweeps are provably no-ops, not merely usually ones.** If a sweep
    moves nothing, positions and occupancy are unchanged entering the next; the cooling
    radius is non-increasing, so that sweep's candidate set is a *subset* of the one
    just exhausted over an identical layout. It therefore also moves nothing, and by
    induction so does every sweep after it. Measured: on the fixture, `iterations` of
    1, 2, 3 and 10 give **bit-identical positions** to 200.

    So an early exit here is *output-identical*, not merely output-similar — which is
    what makes this worth asking. **§2.4 says "Nothing detects it and nothing stops
    early", and that sentence converged through three review rounds**, so it is not
    something to quietly reverse. The open question is what it was asserting: that
    early convergence is *correct*, which the induction leaves untouched, or that the
    remaining sweeps must actually execute, which the induction makes pure cost. Phase
    4's round decides, and a decision either way is recorded in §2.4.

  **HALF-RESOLVED 2026-08-15 by Phase 4, and the two halves got different answers.**
  The second is **decided and shipped**: the search exits when an iteration moves
  nothing. It was asserting that early convergence is *correct*, which the induction
  leaves untouched, and not that the sweeps must run. The decision, the induction as the
  cluster pass extends it, and the assertion that carries it are in §2.4. The exit is
  output-identical: `t` is `11.338720` either way and the fixture executes 2 iterations
  rather than 200.

  The first is **left open, deliberately**, and it is now the only performance item in
  this spec. `O(iterations · V · r² · E²)` stands and so does the **72.9 s release**
  measurement. The exit changes the leading factor and not the shape — a run now executes
  as many sweeps as convergence takes rather than `iterations` of them — so the pressure
  is off but the term is not gone. **The three shallow fixes above were deliberately not
  taken**: with the exit in place each buys a constant against a term the deep fix
  removes outright, and one of them has grown a third call site, since `cluster.rs`
  rebuilds `cost::corridors` once per candidate too. The deep fix — a delta score over
  the edges a move actually touches, which for a cluster move is the bridge alone — needs
  a phase or a spec of its own, and should get one before §1's promise of "a real metro
  network" is tested on a city rather than on a fixture.

  **The real-network measurement exists now, recorded 2026-08-16 by `llk-002` Phase 4,
  and it is a cross-spec write that spec's §4 named: 0.13 s release on BART.** 50
  stations, 50 corridors, 3 executed sweeps, and the rest of the pipeline —
  parse, project, snap, render, write — is below the timer at this size, so
  essentially all of it is **~43 ms a sweep**. The 17-station fixture is under
  10 ms. `llk-002`'s OQ-6 recorded that it was breaking the ordering the paragraph
  above recommends, on the judgement that a metro network is the small case and 50
  stations sits inside the envelope 200 was measured at. **The judgement held**, and
  the paragraph above is what made it safe to make.

  **It does not retire the term, and reading it that way would be the mistake.** The
  early exit is doing the work: BART pays 3 sweeps where the 72.9 s figure paid 200,
  so what this measures is convergence on one well-behaved network, not the cost of a
  sweep at scale. `V · r² · E²` per sweep is untouched — a city with a denser graph,
  or one whose search does not settle in three, still lands on it, and 50 stations is a
  quarter of where the 72.9 s was taken. The delta score is still the fix, and this
  number moves it from urgent to schedulable rather than closing it.

  *(Note, 2026-08-17 at Phase 7 — a note and not a correction, because this is a dated
  measurement rather than a live promise, which is why §1's block gets the other
  treatment. The reweight doubles BART's executed sweeps from 3 to 6, so the run is
  ≈0.26 s at the same ≈43 ms a sweep. **The conclusion is untouched** in both directions:
  the per-sweep term is what the delta score attacks and it did not move, and a factor of
  two on a quarter-second run is still schedulable rather than urgent. Worth recording
  only because it shows the leading factor is a property of the weights too, not of the
  network alone.)*

- **OQ-10** — **The search cannot escape a crossing that a dense multi-line core creates,
  at any weighting.** *(design call, and one measurement has to come first.)* **Blocks
  nothing, and is attached to no phase yet** — which §4 says is how a question stays
  unforced, so this entry says plainly what would attach it. Raised 2026-08-18 from a local
  draw of Chicago, outside any phase; the numbers are in **OQ-2** above and the feed's
  provenance in `llk-002`'s OQ-8.

  The shape of it: `layout/candidate.rs` offers one station at a time a free cell within a
  shrinking radius, and `layout/cluster.rs` translates the smaller side of a bridge
  rigidly. Both are **monotone in the total cost** — a move is taken only if it improves —
  so neither can cross a ridge, and untangling two corridors that already cross plausibly
  requires a move that is worse before it is better. Chicago stalls at four crossings from
  25, and stays there under an eighty-fold `w1`, which is what a ridge looks like from
  outside. **`llk-002`'s Phase 6 is where this becomes visible to a reader** rather than to
  whoever ran the sweep: that phase's gate 6 asks whether the map reads as a poster of the
  city, and on Chicago the answer is no *at the Loop*, which is the part of the map a
  person looks at first.

  **The branch that would have closed this outright is measured and gone.** The question
  was whether four is a topological floor or a search floor — if the CTA graph simply
  admitted no crossing-free drawing, there would be nothing to fix and the shipped weights
  would be vindicated. **It is planar.** Tested 2026-08-18 on the imported network with a
  Boyer–Myrvold check: CTA is `V = 141, E = 146`, planar, with a core of 22 nodes and 27
  edges once degree-2 chains are suppressed; BART is planar too, core 12 and 12. So a
  crossing-free drawing of this network exists in the plane and the search reaches none at
  any weighting.

  **One caveat, and it is the whole of what is left of that branch.** Planarity gives a
  crossing-free drawing with arbitrary vertex positions and curved edges. It does **not**
  give one that is octilinear, on an integer grid, at a spacing derived from the network's
  median edge length. So the floor is now known not to be topological, and is either the
  move set or the grid discipline itself — and those two are separated by a different
  experiment than this one, since the second would show up as a floor that moves with
  `--grid-spacing`. It did not: 600 m gives four crossings as 855 m does, which is weak
  evidence for the move set and not proof.

  If the gap is real, the candidate answers are the ordinary ones and each is a phase:
  a move that accepts a worsening step under a schedule, an edge- or corridor-level
  untangling move rather than a station-level one, or a restart from several snapped
  seeds and keep the best. All three change §2.4's search, all three cost run time against
  **OQ-9**'s untouched `V · r² · E²` per sweep, and none of them is a weight.

  **Why this is not OQ-2.** That entry is about *what the criteria are worth relative to
  each other*, and it now records that `w1` cannot be calibrated from Chicago. This one is
  about *what the search can reach* whatever they are worth. They were the same question
  until the eighty-fold sweep separated them, and keeping them merged would put a move-set
  problem behind a slider.

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
  bundling. `build_schematic_svg`. `llika-cli` with `--input` and `--output`.
  **Author the fixture** at `llika-core/tests/fixtures/sample_network.json` to the
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

- **Scope:** `layout/cost.rs` — `c1` through `c5` **to §2.3's operational
  definitions** and the weighted total `t`. `geometry.rs` gains segment intersection
  and angle math, moved here from Phase 1 because `c1` and `c5` are their first
  consumers and this is where they first get tested. Resolve OQ-6 and implement the
  chosen `c2` target `L`.

  `LayoutParams` gains `w1`-`w5` with an **explicit** `Default` impl — not a derived
  one. `#[derive(Default)]` gives every weight `0.0`, which makes `t ≡ 0`, and a
  scorer that scores nothing would pass Phase 3's cost-decrease gate. The first
  values are OQ-2's and are deliberately provisional.

  Tests are **unit tests inside `layout/cost.rs`**: `Network`'s fields are
  `pub(crate)` and `SchematicLayout` has no public constructor, so a hand-built graph
  is constructible from a child module of the crate and not from `llika-core/tests/`.
  Nothing calls any of this yet.
- **Exit gate:** per-criterion unit tests over hand-built graphs with hand-computed
  expected values, each criterion having both a zero case and a known-nonzero case:
  1. `c5` is exactly `0.0` for a single edge at each of the **eight unit cell
     offsets** — `(1,0) (1,1) (0,1) (−1,1) (−1,0) (−1,−1) (0,−1) (1,−1)` — and
     strictly positive at offset **`(2,1)`**, 26.565°. Not 22.5 degrees: that
     direction is not constructible on an integer grid, `tan 22.5° = 0.414…`, and the
     nearest offsets the grid admits are `(2,1)` at 26.565° and `(12,5)` at 22.620°.
  2. `c1` is 0 for a path graph, exactly 1 for a hand-built proper crossing, and
     exactly 1 for a hand-built pair that merely touch — an endpoint of one lying in
     the other's interior — which §2.3's closed-segment test counts.
  3. `c3` is exactly `0.0` for a degree-4 station whose edges leave at 0/90/180/270,
     strictly positive for a degree-4 station with an uneven spread, and exactly
     `π/3` for the best degree-3 station (`135°/135°/90°`) — the floor §2.3 derives.
  4. `c4` is 0 where a line runs straight through a degree-2 station, positive at a
     bend there, and **unchanged by a bend at a degree-3 station**.
  5. `c2` is 0 when every edge is exactly the OQ-6 target and positive otherwise.
  6. `t` is the weighted sum: with one weight at 1 and the other four at 0, `t`
     equals that criterion, for each of the five in turn. And every weight in
     `LayoutParams::default()` is non-zero.
  7. The fixture's SVG is **byte-identical** before and after this phase, checked
     against a golden file — `llika-core/tests/fixtures/golden/sample_network_phase1.svg`,
     generated by the binary at this phase's **base commit** and committed as this
     phase's first commit. The existing cross-process test compares two runs of the
     same build and so cannot serve as the "before". The golden file is retired at
     Phase 3, which is the phase that legitimately changes the picture.
- **Close-out:** seeds `rules/layout-cost.md`, and **updates
  `rules/projection-grid.md`**, which currently states that segment intersection and
  angle math do not exist — this phase falsifies it. Records the OQ-6 resolution
  in §3.

### Phase 3 — single-station hill-climbing
*Produces the observable: **yes** — the first map the layout step has actually
reasoned about. Not "the first that looks schematic": OQ-8 established that the Phase
1 map is already fully octilinear on this fixture, by accident of the fixture's own
spacing. What this phase adds is measurable and visible but narrower than the original
claim — evenly spread junctions and unkinked lines, `c3` 22.515 → 14.137 and `c4`
7.069 → 3.927, with 5 of 17 stations moving.*

- **Scope:** `layout/candidate.rs` (candidate enumeration over §2.4's rings `1..=r`,
  move validity) and `layout/hillclimb.rs` (iteration loop, §2.4's linear cooling).
  All **three** move rejections from §2.4 — occupancy, the pinned order-flip
  predicate, and exact overlap with its own pair set — carrying its **OQ-1
  decision**: exact overlap rejected, ordinary crossings left to `c1`. Cluster step
  remains absent.

  **The integration points, named because the two new files are not the whole
  change.** `run_layout` currently returns after snapping and must call the loop.
  `LayoutParams` gains `iterations` (default 200) and `initial_radius` (default 3),
  both serde-visible, both flagged in Phase 6. `layout/mod.rs` declares the two new
  modules. And **`llika-core/tests/golden.rs` is deleted in this phase**, with
  `tests/fixtures/golden/sample_network_phase1.svg`: it pins the SVG byte-for-byte,
  Phase 2's gate 7 says it retires here, and it fails the moment the picture changes.

  **Grid occupancy needs one addition, and it belongs to this phase.**
  `grid::snap_to_grid` builds a `GridOccupancy` and drops it, and `GridOccupancy` has
  `claim` but no way to release a cell — so a search that moves a station cannot use
  the type Phase 1 built for exactly this job. Add a `relocate(station, from, to)` to
  `grid.rs` and have the search carry the occupancy forward from the snap, rather than
  keeping a private free-cell index in `hillclimb.rs`. Two structures answering "which
  cells are taken" is how they come to disagree.

- **Exit gate:** `cargo test` green, and six assertions.
  1. **`iterations = 0` reproduces the snap-only layout bit-for-bit.** This is the
     reproducible baseline the rest of the gate is measured against, and it is what
     replaces the retiring golden file. Assertion 2 has no meaning without it.
  2. **`t` after the search is strictly lower than `t` at `iterations = 0`** — that
     comparison and explicitly *not* "final iteration versus first iteration". The
     search reaches a fixed point inside the first sweep on this fixture, so the two
     are bit-identical and the first-versus-final reading fails on a correct
     implementation. Measured: 37.166633 → 22.505867. Assertion 1 makes this two
     `run_layout` calls and two `total_cost` calls, all public.
  3. **The fraction of edges within 5 degrees of a multiple of 45 is
     non-decreasing** against assertion 1's baseline — never "strictly greater",
     which OQ-8 showed is unsatisfiable here because the baseline is already 1.0.
  4. **Each of §2.4's three rejections fires, one test each.** A move onto an
     occupied cell is rejected. An order-flipping move is rejected — built on a
     station of **degree ≥ 3**, since the predicate is vacuous below that and the
     test would otherwise pass for the wrong reason; the fixture has four such
     junctions and real flipping candidates at every one, so this needs no hand-built
     graph. And an **exact-overlap** move is rejected, on a hand-built fold-back where
     one edge comes to lie along its own neighbour — which must share an endpoint,
     because that is the case `c1` cannot see and therefore the only one that tests
     the rule rather than the penalty.

     Plus **one negative test, which is the load-bearing one**: a move that leaves a
     station's two edges exactly collinear through it — a legitimate straight-through
     — is **not** rejected. The three positive tests all pass under the
     `segments_intersect` implementation §2.4 warns against, and so does assertion 5
     on most crossing fixtures; this is the only assertion that fails it
     deterministically rather than by luck of fixture shape.
  5. **A second, small fixture carrying a deliberate crossing**, on which `c1` is
     strictly positive at the baseline and strictly lower after the search. The
     17-station fixture scores `c1 = 0` before *and* after, so without this the OQ-1
     decision — the phase's headline call — ships with zero gate coverage. This is
     the same failure OQ-5 was amended to prevent for the snap tie-break, one rule
     over.
  6. **Determinism across processes**, delegated to the existing
     `llika-cli/tests/byte_stability.rs` rather than re-asserted weakly here: two
     in-process runs cannot see a per-process hasher seed, and a `HashMap`-backed
     occupancy carried through the search is the most likely new place for one. The
     existing test runs two processes on defaults and will exercise the search
     automatically; confirm it still passes and that it is reaching the loop.

  Then the human half, named separately because it is **not** reproducible and does
  not carry the gate: open the SVG and confirm the junctions read as evenly spread and
  no line kinks where nothing forces it to. This is the visual judgement OQ-2 promises
  for the provisional weights, and it is the first occasion to make it.
- **Close-out:** seeds `rules/layout-search.md`, updates `rules/layout-cost.md` and
  **`rules/projection-grid.md`** — the latter declares `layout/mod.rs` among its
  sources and states "`run_layout` projects, derives `g`, snaps, and is infallible…
  There is no iteration loop yet", both false after this phase. Records in §3 that
  OQ-1's implementation is closed while its reconciliation with the paper is not.

### Phase 4 — cluster moves
*Produces the observable: **yes** — the same map with a class of local minimum
removed, and the map visibly changes. Measured at this phase's review round: a rigid
move of the `parkview`/`lakeside` pair takes `t` from 22.505867 to 16.222682 on the
committed fixture, which the poster shows. Under the `2g` rule the round replaced, it
would have produced no observable at all — the step could not fire on anything
committed.*

- **Scope:** `llika-core/src/layout/cluster.rs`, implementing §2.4's cluster rule as
  OQ-7 resolved it — bridge-sides of two or more stations, computed once from the
  graph, translated rigidly, with the three rejections in their pinned group forms.

  **The integration points, named because one new file is not the whole change**
  (the omission Phase 3's round 1 caught one phase earlier):

  - `layout/mod.rs` declares `mod cluster;`.
  - `hillclimb::run` gains the cluster pass, **after** the per-station sweep and inside
    the same iteration, over clusters in corridor order.
  - `LayoutParams` gains **`cluster_moves: bool`, default `true`** — the seam the exit
    gate's own headline comparison needs, since `run_layout` is the only public entry
    and `hillclimb::run` is `pub(super)`. It is serde-visible, so Phase 6 derives a
    flag from it like every other field (§2.6).
  - `candidate::is_rotation` becomes `pub(super)`. The group order-flip rule is the
    same predicate over the same sequence, and §2.3's argument against writing one
    comparator twice applies unchanged. `cost::corridors` and
    `cost::incident_directions` are already `pub(super)` and need nothing.
  - Bridge finding is new code with no home yet; put it in `cluster.rs` rather than
    reaching for a `petgraph` algorithm, so its determinism is this crate's to state.
    **Nothing in it may iterate a `HashMap`** — grouping is exactly where one is the
    reflex structure, and §2.2's order rule is what byte-stability rests on.

  **OQ-9 is answerable here and this phase is where it is answered** — it names Phase 4
  and, before this round, Phase 4 did not name it back. Decide it, record the decision
  in §2.4, and note that the bridge rule leaves its induction intact: clusters are a
  function of the graph, not of positions, so a sweep that moves nothing still makes
  every later sweep a no-op.

  **If that decision adds an early exit, output-identity is the assertion it carries**,
  and this fixture cannot be trusted to show it: the natural wrong version — exit when
  the *station* pass moved nothing, ignoring the cluster pass — is invisible here,
  because with both passes on, `iterations` of 1, 2, 3, 10 and 200 are already
  bit-identical. If the decision instead leaves §2.4 alone, say so in the close-out
  rather than leaving OQ-9 silently open a second time.
- **Exit gate:** `cargo test` green — the whole deterministic suite, since this phase
  changes every layout the crate produces — and five assertions:
  1. **`cluster_moves: false` reproduces Phase 3's shipped layout bit-for-bit**, at
     `t = 22.505867` on the fixture. This is the baseline the rest is measured against,
     the same role `iterations = 0` played for Phase 3.
  2. **With cluster moves enabled, `t` on the fixture is strictly lower than that.**
     Keyed to the committed 17-station fixture and not to a purpose-built one, because
     OQ-7's resolution measured a real improving cluster move on it. The comparison is
     *not* vacuous and the direction is not free: hill-climbing is path-dependent, so
     an accepted cluster move can steer the run to a worse fixed point than the
     single-station search reaches.
  3. **The cluster set is the one §2.4 defines**: on the fixture, exactly **7**
     bridge-sides of two or more stations, and none equal to a whole connected
     component. A unit test on the bridge finder over a hand-built graph with a known
     cycle, since a fixture-only check cannot tell a correct bridge finder from one
     that returns every edge — and that graph must also carry a bridge splitting it
     **evenly**, because the smaller-side tie rule is otherwise untested: no tie arises
     on the fixture at all, whose sides are 4, 5, 6, 2, 2, 2 and 3 over 17 stations.
  4. **Each of the three group-form rejections fires, one test each** — in particular
     that a translation onto a cell the cluster itself is vacating is **accepted**,
     which is the load-bearing negative: the reflex occupancy reading rejects it and
     makes the whole feature inert while every aggregate assertion above still passes.
  5. **Determinism across processes**, delegated to `llika-cli/tests/byte_stability.rs`
     as Phase 3 did — two in-process runs cannot see a per-process hasher seed. The
     fixture now exercises the new path at the defaults, so that test covers it;
     confirm it does rather than assuming it.
- **Close-out:** updates `rules/layout-search.md` — which today states "Cluster moves
  do not exist yet", must add `llika-core/src/layout/cluster.rs` to its `sources:`, and
  will need its `max_lines` raised from 55 against a 52-line body. Also updates **§2.4**
  itself, whose threshold and rigid-move rules this phase settles, and records OQ-9's
  decision there.

### Phase 5 — line-bundling renderer
*Produces the observable: **yes** — this is the phase that makes the output read as a
transit poster rather than as a graph drawing, and §1's end-state sentence names its
output by name. **The fixture shows it on one corridor**, not on the whole trunk:
§2.5's last decision measures the split, and `oldtown`–`eastbank` is the single
genuinely parallel pair. That is what OQ-5 engineered the fixture to guarantee, and
overstating it is how a gate comes to assert something the data cannot show.*

- **Scope:** `render/corridor.rs` implementing every rule in §2.5 — runs broken at
  collapse stations, the symmetric `(k − (n−1)/2) · s` offset with `s` from
  `bundle_spacing`, the line-`id` sort key, the corridor-directed normal, the mitre at
  an interior bend, one SVG path per line with `stroke-linejoin="round"`, and markers
  left where they are. Corridor membership is **read from `LineSet` on the graph
  edge**, not re-derived from the station lists.

  **The integration points, named because one new file is not the whole change** — the
  omission this loop has now caught at Phase 3, Phase 4 and here:

  - `render/mod.rs` declares `mod corridor;`. It declares **no submodule at all**
    today.
  - `render/mod.rs:line_path_data` is the only producer of path data and emits bare
    cell centres; it is replaced rather than extended.
  - `RenderParams` gains **`bundle_spacing: Option<f64>`**, defaulting to `None`
    (§2.5). It is the fourth field on a struct Phase 6 derives a flag from per field.
  - **Offsets are applied in SVG user space, after `Viewport::project`**, not in cell
    space. The magnitude is in user units because it derives from `stroke_width`,
    which is; applying it before the transform would couple it to `units_per_cell`.
  - Nothing in `layout/` is touched. The picture changes; the layout does not.

  **Nothing in `corridor.rs` may iterate a `HashMap`.** Grouping edges into runs and
  keying offsets by line id is exactly where one is the reflex structure, and §2.2's
  order rule is what byte-stability rests on — the same ban Phase 4's scope carried,
  for the same reason, one subsystem over.

  **OQ-2 names this phase and this phase now names it back.** The weights were judged
  once, at Phase 3, against an unbundled picture — "judged against half of it", as OQ-2
  puts it — and this is the redraw it nominated. Re-judge them by eye at the visual
  check and record the outcome in OQ-2, including "they stand", which is an outcome and
  not a skipped step.
- **Exit gate:** `cargo test --workspace` green — the whole suite, since this phase
  rewrites the path data of every line in every SVG the crate emits — and seven
  assertions:
  1. **`bundle_spacing: Some(0.0)` reproduces the unbundled SVG byte-for-byte, against
     a golden file generated by the binary at this phase's base commit and committed as
     this phase's first commit.** The reproducible baseline the rest is measured
     against, and the reason the parameter is `Option<f64>` rather than a bare `f64`
     with a magic default.

     **The capture point is the whole assertion.** Unlike Phase 3's `iterations = 0` and
     Phase 4's `cluster_moves: false`, both computable at test time from two public
     calls, the "before" here stops existing the moment `line_path_data` is replaced —
     and nothing in the tree holds it, since Phase 2's golden retired at Phase 3 by
     design. A golden captured *after* the new renderer lands asserts that the code
     agrees with itself, which is the defect Phase 1's assertion 1 exists to prevent.
     This is Phase 2's clause reused verbatim, and it is legitimate here for the reason
     `PHASE3_POSITIONS` was at Phase 4: "no bundling" is a picture that must never
     change again.
  2. **Along `oldtown` → `eastbank`, the Red and Green segments are parallel,
     separated by exactly `bundle_spacing`, and symmetric about the corridor
     centreline.** **This is the only clause that discriminates**, and the gate says so
     because two of the three assertions the phase originally carried are satisfied by
     the *Phase 1* renderer: it already emits one path per line, and unbundled paths
     already coincide at every interchange.
  3. **At `central` and at `market` both paths pass through the identical point, *and*
     at `oldtown` and `eastbank` they do not** — both halves in one test. The first
     alone passes vacuously on a renderer that does no bundling at all, which is
     precisely the state this phase starts from.
  4. **The mitre's four branches, as unit tests over hand-built direction pairs** —
     straight-through gives exactly `1 · s`; a 90° turn gives `√2 · s`; the
     `(5,0)`/`(5,1)` shape §2.5 names gives **`4 · s`, the clamp, and not `10.15 · s`**;
     anti-parallel gives direction `n₁` **at `4 · s`**. That last magnitude is asserted
     as well as the direction: §2.5 reaches it through the clamp rather than through a
     special case, and an implementer who reads the guard as a whole special case would
     return `1 · s`, which a direction-only assertion accepts.

     **Neither committed fixture can carry this, and that is the whole reason it is a
     separate assertion.** On `sample_network.json` the only two bundled run-interior
     stations are `oldtown` and `eastbank`, both straight-through at scale exactly 1;
     the only bend at a run-interior station is `southgate`, which carries one line and
     so multiplies the mitre by zero; `crossing.json` bundles nothing at all, since its
     two lines share no station. **The clamp is structurally unreachable from any
     octilinear fixture** — the sharpest such corner is 2.61 against a limit of 4. So of
     the mitre's four branches only straight-through is reached with a nonzero offset,
     and an implementation writing `1/cos(θ)` for `1/cos(θ/2)`, or omitting the clamp,
     or dividing by zero at anti-parallel, would pass every other assertion here and the
     whole suite besides. This is the failure this document blocked on at Phase 3
     (assertion 5) and Phase 4 (assertion 3), one subsystem over.

     It needs no fixture and no new expected values — §2.5 supplies all four — and it
     is a unit test inside `render/corridor.rs`. The reason is simpler than Phase 2's,
     which was that a hand-built `Network` is only constructible inside the crate: a
     direction pair needs no `Network` at all, but `corridor.rs` is a private module and
     nothing in `llika-core/tests/` can reach into it.
  5. Exactly one `<path>` element per line, kept as a **regression guard and labelled
     as one** — `llika-core/tests/render.rs` already asserts it and it cannot fail
     here.
  6. **Determinism across processes**, delegated to `llika-cli/tests/byte_stability.rs`
     as Phases 3 and 4 did. The fixture exercises bundling at the defaults, so that
     test covers it; confirm it does rather than assuming it.
  7. **The two degenerate inputs still render** — one station with no lines, and two
     stations at identical coordinates. Bundling runs on them too, and a network with
     no corridor at all is the case where run-finding has nothing to iterate. Not a
     division hazard: the normal's divisor is the corridor length, which §2.2's
     occupancy invariant keeps non-zero post-snap, and the mitre's is covered by the
     clamp.

  Then the human half, named separately because it is **not** reproducible and does
  not carry the gate: open the SVG and confirm `oldtown`–`eastbank` reads as two
  parallel strokes converging to single points at `central` and `riverside`. This is
  also OQ-2's occasion.
- **Close-out:** updates **`rules/rendering.md`** — which must add
  `llika-core/src/render/corridor.rs` to its `sources:`, loses its "**There is no
  line-bundling.** Two lines sharing a corridor draw over each other, the later path on
  top" paragraph, and needs `max_lines` raised from 40 against a 37-line body. Also
  **`README.md`**, whose phase table marks this row `drafted` and whose status text says
  trunks "currently overprint rather than running as parallel strokes" — user-facing
  documentation, which §6's close-out hook covers. Records OQ-2's re-judgement.

### Phase 6 — full parameter surface
*Produces the observable: **yes** — the map, made tunable. This is the phase that
delivers §1's promise that a user can improve a good first result, and it is the
last thing the roadmap's UI needs from the core.*

- **Scope:** `clap` flags covering every `LayoutParams` and `RenderParams` field
  (`--grid-spacing`, `--iterations`, `--w-crossings` and the rest), plus `--params
  <file>`. ~~`Serialize`/`Deserialize`/`Default` on both structs.~~ — *that shipped at
  Phases 1–2 and §2.6 records it; it is struck rather than deleted because the scope
  line read as new work.* Structural snapshot tests.

  #### The flag names are mechanical, and §1's invocation is what gives (decision)

  **Every flag is its field name kebab-cased, with no exceptions and no table.**
  `w_crossings` gives `--w-crossings`. §2.6 noticed the tension and left it open;
  §1's end-state block types `--w-crossing`, singular, and **§1 is what changes.**

  The argument is the gate below rather than taste: a test enumerating both structs'
  fields against the registered flags is only worth having if it derives the expected
  flag from the field. Given one hand-written exception it must consult the same
  mapping table the implementation does, which makes it assert that the code agrees
  with itself — the defect Phase 1's assertion 1 exists to prevent. One irregular
  flag costs the phase its only structural guarantee, and `--w-crossing` is not worth
  that.

  **`cluster_moves: bool` defaults `true`, so it needs the off switch and not the on
  one.** clap's `SetTrue` cannot turn a `true` default off. It takes an explicit
  value — `--cluster-moves <bool>` — rather than a `--no-` prefix, because the
  enumeration test derives one flag per field and a `--no-` pair is two names for one
  field. The same rule covers any future `bool`.

  #### `--params` takes both structs, and the container is named here (decision)

  The prior wording said "the whole struct as JSON", singular, while its own two
  examples were a `LayoutParams` key and a `RenderParams` key. There are **two**
  structs and they live in different modules, so a flat object deserializes into
  neither without a hand-written `Deserialize` that §2.8's layout has no home for.

  **The file is one JSON object with two optional keys:**

  ```json
  { "layout": { "iterations": 400 }, "render": { "stroke_width": 8.0 } }
  ```

  Both keys are optional and each omitted one means `Default`, which is the same rule
  `#[serde(default)]` gives the fields inside them. The wrapper is a new
  `Serialize`/`Deserialize` struct in `llika-cli`, not in core: it is a CLI file
  format rather than a library type, and §2.6's argument for the two structs being
  plain is about what a Tauri command passes, which is the structs and not a file.

  **Individual flags override the file, field by field**, and that ordering is stated
  because the gate's first clause depends on it: a flag given alongside `--params`
  wins over the same field in the file, and a field named in neither takes `Default`.

  **Unknown keys are rejected — `deny_unknown_fields` on both structs and on the
  wrapper.** `{"w_crosings": 9.0}` with a typo is otherwise a file that parses to all
  defaults and silently ignores the one value the user cared about, which is the worst
  case for a knob whose whole purpose (§1) is improving a result by eye. This is the
  one place the phase deliberately chooses strictness over tolerance, and it composes
  with `#[serde(default)]` rather than fighting it: missing is fine, misspelled is not.

  **Five things Phase 6 inherits and must not ship silently**, three of them found by
  a code review after Phase 3 and one by this phase's own review round:

  - **`#[serde(default)]` on `LayoutParams` *and* `RenderParams`.** It is absent from
    both — the heading named only the first while the correction below measures both,
    and an implementer following the heading leaves `{"stroke_width": 8.0}` failing.
    It is absent, so a `--params` file
    omitting any field — `iterations`, say — fails to deserialize with `missing
    field` rather than falling back to the `Default` that §2.6 argues is precisely
    the meaning of an unset field. A user writing a file with only the weights they
    care about is the common case, not the exotic one.

    **And until it lands, ~~every field this spec adds~~ every non-`Option` field
    this spec adds is a format break** — Phase 4's `cluster_moves` invalidated every
    `LayoutParams` JSON written before it. That costs nothing today, because no
    `--params` exists and nothing in the tree writes one, which is why Phase 4's code
    review recorded it here rather than fixing it out of phase. It stops being free
    the moment this phase ships the flag, so the attribute goes on **before** the
    first file format is published, not after.

    *(Corrected 2026-08-15, at Phase 5's close-out, and **measured against the
    shipped structs rather than argued**. `serde`'s derive already treats a missing
    `Option<T>` as `None` without the attribute, so `grid_spacing` and Phase 5's
    `bundle_spacing` both deserialize fine when omitted and neither was ever a break;
    `cluster_moves`, a bare `bool`, was one. The struck version is recorded rather
    than replaced silently because it is what a reader reconstructs from the
    `cluster_moves` example alone.*

    *The correction does not weaken this bullet's case, which is the paragraph above
    it and was measured in the same pass: `{"w_crossings": 9.0}` fails with `missing
    field iterations`, and `{"stroke_width": 8.0}` with `missing field
    units_per_cell`. What it narrows is the blast radius the attribute protects —
    eight of `LayoutParams`' nine fields and three of `RenderParams`' four, not all
    thirteen.)*
  - **Validation, with the bounds named and a home.** The prior wording said
    `initial_radius` "needs an upper bound" without giving one, which is a design
    handed to an implementer under the name of a decision — the shape OQ-7 was closed
    for. The bounds:

    | field | accepted | why the bound |
    |---|---|---|
    | `grid_spacing` | finite and `> 0` | `round(x / g)` is NaN at 0 and casts silently to cell 0 |
    | `initial_radius` | `1 ..= 64` | `candidate::spiral_offsets` materialises every cell of rings `1..=r`, rebuilt every sweep; 64 is ~16 600 tuples and 10 000 asks for ~4·10⁸ |
    | `iterations` | any `u32`, including 0 | 0 is the snap-only mode Phase 3's gate is keyed to — a supported value, not a degenerate one |
    | `units_per_cell`, `stroke_width` | finite and `> 0` | a zero or negative scale draws nothing, or inside out |
    | `margin_cells` | finite and `>= 0`, and `> 0` where the network's extent is zero on either axis | Phase 1's scope: a one-station network reduces the envelope to `2 · margin_cells · units_per_cell`, so zero yields a zero-extent document |
    | `bundle_spacing` | finite when `Some` | `Some(0.0)` is the supported disable seam (§2.5) |

    **`initial_radius = 0` is rejected rather than accepted-as-1.**
    `hillclimb::cooling_radius` clamps it to 1, so it is representable and silently
    means something else; `hillclimb.rs`'s own test comment already records that "the
    flag Phase 6 derives rejects it at the boundary rather than silently meaning
    something", and the prior wording named only an upper bound.

    **Validation lives in `llika-cli`, and `run_layout` stays infallible.** Its doc
    says every degenerate input the schema admits "has a defined answer here rather
    than an error", and §2.6 pins the signature a Tauri command calls; making core
    fallible changes both and pushes a `Result` through `build_schematic_svg` and every
    caller. But a clap `value_parser` alone is not enough — **`--params` would bypass
    it entirely**, and it lands in the same phase. So validation is one function run
    **after** the file and the flags are merged, and it is what both paths go through.

    **It takes the network's extent as well as the two structs**, because the
    `margin_cells` row is conditional on the drawn network being degenerate on an axis
    and the parameter pair cannot know that. The CLI has the `Network` in hand by then,
    so this is a signature detail rather than a gap — but it is stated, because "one
    function over the assembled pair" is the natural reading and it cannot satisfy its
    own table. The `j_max == j_min` case — a collinear multi-station network — is
    included by the row's wording rather than only the one-station case Phase 1 argued
    from; it is a pre-existing property of §2.2's envelope and fires only where a user
    asks for a zero margin outright.
  - **`--initial-radius` does nothing at the default weights** — see OQ-2, which
    re-judged the weights at Phase 5 and left them standing. **So the fork this bullet
    named is decided: the flag ships with the inertness stated**, in its `--help` text
    and in `rules/cli.md`, rather than the weights changing first. Reproduced twice
    more since: `r_0` of 1, 2, 3, 5 and 8 give bit-identical positions at
    `t = 11.338720` on the fixture with cluster moves on.

    *(**Reversed 2026-08-17 at Phase 7**, on both halves. The weights did change first
    after all, and the inertness this fork chose to ship was not a property of the flag
    but of the fixture: on BART `r_0 = 1` draws a different map from `r_0 ≥ 2`. The
    `--help` text and `rules/cli.md` both now say saturation rather than inertness, and
    both halves are gated. Recorded rather than struck, because the fork was decided on
    the evidence available and it is the evidence that moved.)*
  - **The CLI summary line, and what is actually missing.** ~~It still prints only the
    grid size~~ — it prints station count, line count and grid; the missing clause is
    `cost … → … over N iterations`. And ~~reporting it needs a public way to get `t`
    out of `run_layout`~~ **is false and was false when it was written**: `lib.rs`
    re-exports `total_cost`, and Phase 3's own gate assertion 2 says so in terms —
    "two `run_layout` calls and two `total_cost` calls, **all public**".

    **The gap that is real is the other half of the sentence.** `hillclimb::run`
    returns the executed sweep count as a `u32` and `run_layout` **discards it** —
    the call is a bare statement — and `SchematicLayout` has no field for it. So
    "over 200 iterations" cannot be printed truthfully today, and printing the
    *requested* count would be a lie the moment Phase 4's early exit fires, which on
    the fixture is immediately: **2 sweeps executed against `iterations = 200`.**
    `SchematicLayout` gains an `executed_iterations` accessor beside
    `grid_spacing()` and `target_edge_cells()`, for the same reason those exist — it
    is a function of the parameters *and* the network, so a caller cannot compute it
    from `LayoutParams`. **This is the phase's one change inside `llika-core/src/layout/`.**
  - **§1's cost literals are fiction, and §1 is what changes.** Measured at §1's own
    flags — `--grid-spacing 900 --iterations 200 --w-crossing 5.0` — the real pair is
    `t` **54.817110 → 12.747375**, against the block's `4820.3 → 611.7`. At the
    defaults it is **37.166633 → 11.338720** in **2** executed sweeps. The prior
    wording said this phase "owns reconciling §1's invocation with the real one" and
    never said which way. **The direction: the CLI prints what it measured, and §1's
    block is corrected in place at this phase's close-out to what the shipped binary
    prints** — a dated `CORRECTED` note beside the text, per the methodology's §6.1
    rule for shipped prose that is now actively misleading. §1 is a goal written
    before anything was built; the binary is not going to be bent to match its
    invented numbers.
  **The integration points, named because "add some flags" is not the whole change** —
  the omission this loop has now caught at Phases 3, 4, 5 and here, and this phase
  needs one more of them than any of those did:

  - **`llika-cli` gains a JSON dependency.** It depends on `llika-core` and `clap`
    alone today, so `--params` has nowhere to parse. Add `serde` and `serde_json` to
    `llika-cli/Cargo.toml`; both are already in the workspace's lock via core.
  - **`llika-cli` has no lib target**, so nothing in `llika-cli/tests/` can name the
    `Args` type. The field-to-flag enumeration test is therefore a `#[cfg(test)]`
    module **inside `main.rs`**, which is the same constraint and the same answer
    Phase 2 gave for `layout/cost.rs` and Phase 5 for `render/corridor.rs`. Shelling
    out to `--help` and grepping is the alternative and is worse: it tests the help
    renderer, not the registration.
  - **The `#[serde(default)]` and `deny_unknown_fields` work lands in
    `llika-core/src/layout/mod.rs` and `llika-core/src/render/mod.rs`** — two *core*
    files, in a scope that otherwise reads as CLI-only.
  - **`llika-core/src/layout/mod.rs` also gains `SchematicLayout::executed_iterations`**,
    per the summary-line bullet. Nothing else in `layout/` is touched, and no
    algorithm changes: `hillclimb::run` already returns the number and `run_layout`
    already throws it away.
  - `llika-cli/src/main.rs` is substantially rewritten. It is the binary
    `llika-cli/tests/byte_stability.rs` invokes, which is why the gate below carries a
    determinism clause.
- **Exit gate:** `cargo test --workspace` green, and six assertions:
  1. **A `--params` file and the equivalent individual flags produce byte-identical
     SVG — with the file holding *non-default* values on at least one field of each
     struct, and a picture that differs from the all-defaults render.** All three
     clauses are the assertion. The two paths resolve a field by different routes —
     the file by serde name, the flag by clap registration — so this is the only
     clause that catches one of them mis-wired; and a file of defaults satisfies the
     bare comparison while proving nothing, since both sides would then be the
     default picture. Use `grid_spacing` and `stroke_width`: both move the output at
     any setting.

     **Not `initial_radius` or `iterations`**, and the reason is recorded so a later
     pass does not "simplify" the choice: `initial_radius` is inert at the shipped
     weights (OQ-2) and `iterations` is inert above the convergence sweep (§2.4's
     early exit, 2 of 200 on the fixture), so a test keyed to either passes whether
     the flag is wired or not.

     **And a third run pins the override direction**, which the two above exercise only
     in isolation: the same file plus a conflicting `--grid-spacing` must render what
     the flag says and not what the file says. One line, and the only clause that
     touches the merge at all.

     *(Buildable, and checked at this phase's review round against the workspace's clap
     4.6.6 rather than assumed: declare the args `Option<T>` with **no**
     `default_value`, so `None` is "absent" and `Some(v)` is "given". The natural wrong
     version — clap defaults on the args — makes `--params` inert, which fails this
     assertion's first two clauses rather than passing them quietly. clap's derive also
     kebab-cases field names itself, so assertion 2 derives what clap registers.)*
  2. **A test enumerating both structs' fields against the registered flags**, failing
     if a field has no flag, so the surface cannot silently fall behind the structs.
     It derives the expected flag by kebab-casing the field name — which is what the
     decision above buys it — rather than consulting a table the implementation also
     consults.
  3. **Each validation bound rejects at its boundary, one case each**, and by both
     routes: a bad value passed as a flag *and* the same value inside `--params` are
     both rejected. The second half is the load-bearing one — a `value_parser` alone
     passes the first and lets every `--params` file through unchecked, which is the
     failure the validation bullet exists to prevent. Include `--initial-radius 0`
     specifically, since it is representable and `cooling_radius` silently clamps it.
  4. **A misspelled key in `--params` is an error, not a silent default.**
     `{"layout": {"w_crosings": 9.0}}` must fail. Without `deny_unknown_fields` it
     parses to all-defaults and renders happily.
  5. **At least two structural snapshot tests** — element and path counts, interchange
     coincidence — explicitly not pixel-exact. Carried as a **regression guard and
     labelled as one**: `llika-core/tests/render.rs` and `llika-core/tests/bundling.rs`
     already assert both, and they are parameter-invariant, so this clause cannot fail
     for any parameter reason. It is here to catch a rewrite of `main.rs` that stops
     calling the pipeline correctly, not to test the flags.
  6. **Determinism across processes**, delegated to `llika-cli/tests/byte_stability.rs`
     as Phases 3, 4 and 5 did. This is the first phase since Phase 1 to rewrite
     `main.rs` — the binary that test invokes — so confirm it still passes rather than
     assuming it, and extend it to one run with `--params` so the new path is covered
     too.

  `--help` listing every flag is **not** a separate assertion: it restates assertion 2,
  which is the stronger form of the same check.
- **Close-out:** seeds `rules/cli.md`. Updates **`README.md`** — user-facing
  documentation, which §6's close-out hook covers: it carries the invocation, the
  summary line without the cost clause, the sentence "only `--input` and `--output`
  reach them", and a `| 6 | Full parameter surface | drafted |` row. Corrects **§1's
  invocation block** in place, per the summary-line bullet. **`CLAUDE.md` needs no
  change** — its observable line carries no invocation, so the prior wording's "updates
  the `CLAUDE.md` observable line if the invocation in §1 has drifted" had no referent;
  revisit only if §1's rewrite changes what the observable *is*, which it should not.

### Phase 7 — the weights, corrected against a real network
*Produces the observable: **yes**, and more directly than any phase since Phase 5 — it
is the same fifty stations drawn better. Nothing is added to the pipeline; one struct
literal changes and BART goes from 48 of 50 corridors octilinear to 50 of 50, with the
Red line's diagonal cut across the trunk and its doubling-back inside the western bundle
both gone. A phase that changes only a default has to argue it produces the observable,
and this one does it by pointing at the picture.*

- **Scope:** the five values in `llika-core/src/layout/mod.rs:LayoutParams`'s `Default`
  impl, from **5.0 / 1.0 / 1.0 / 2.0 / 5.0** to **5.0 / 1.0 / 0.5 / 0.25 / 10.0**; the
  doc-comment above it, which argues for the old numbers; the `--initial-radius` help
  text, which states something now measured false; and the documentation and one test
  literal that quote figures derived from the weights. **No algorithm changes and no new
  parameter.** Every criterion, every rejection and the whole search are untouched.

  #### The numbers come from OQ-2's third judgement, and the direction is not the argmax (decision)

  §3's OQ-2 entry of 2026-08-17 is this phase's whole evidentiary base and is not
  restated here. What it establishes and this phase acts on: **176 of 324 settings in a
  coarse grid dominate the shipped defaults on all five unweighted criteria at once**, so
  the shipped point is not a near-miss; and the lever is the `w5:w4` ratio, where at 5:2
  `c4` outbids `c5` and buys a straight line through a station by paying for an off-angle
  edge.

  **The chosen point is `5 / 1 / 0.5 / 0.25 / 10` and the runner-up is recorded rather
  than discarded.** `5 / 1 / 1 / 0.5 / 10` moves only the ratio the mechanism names and
  is the more conservative change; it reaches `c5 = 0.000` and 50 of 50 too, and differs
  from the chosen point by being worse on `c2` (1.373 against 0.515), `c3` (29.322
  against 24.609) and `c4` (18.850 against 15.708). The chosen point wins on every
  criterion and drew the better picture — the western doubling-back resolves, and one 45°
  connector survives so the map does not read as a circuit board, which the all-axis
  layouts at `w5 ≥ 15` do.

  **The runner-up is a substitution, not a drop-in, and an earlier draft of this phase
  called it one.** Round 1 measured what switching costs: the fixture reaches
  `t = 8.982526` and BART `40.118892`, so gate assertion 3's pinned constant and all four
  rows of the summary-line table move — roughly eight measured figures, one of them a
  gate literal. It remains the substitution to make if the review round prefers the
  smaller blast radius; it is not a one-line change, and this phase says so rather than
  leaving an implementer to discover it against a red test. *(`8.982526` and `40.118892`
  are the two summary-line figures; assertion 3's constant would separately become
  `16.615381`.)*

  **It is deliberately not the grid's argmax.** OQ-2 has warned twice against mistaking
  one party's eye for a calibration, and the grid's best cell is a measurement on one
  city. The chosen point is a *round* setting inside the dominating region, picked to be
  explainable — `w3` and `w4` halved and quartered from where they were, `w5` doubled —
  rather than the cell that scored highest. Someone re-running this on a second city
  should expect to move it again, and §3 says so.

  #### What this phase does not do, stated so a reader does not infer it (decision)

  **It does not resolve OQ-2**, and the close-out must not mark it resolved. What BART
  settles is negative and needs no more evidence: 5/1/1/2/5 is dominated by a reproducible
  mechanism. Which replacement is *right* still rests on one network, and the second city
  that would settle it is not in this tree. OQ-2 stays open with its third entry standing.

  **It does not add a second real network.** That is the measurement OQ-2 actually wants
  and it is a feed acquisition, a licence check and a fixture commit — `llk-002` Phase 4's
  whole scope, done once. Folding it in here would make a one-literal change into that
  phase again, and the honest version of this phase is small.

  **It does not touch `c4` itself**, and the reason is *not* the one an earlier draft of
  this phase gave. That draft argued `w4 = 0` is strictly worse than `0.25` on `c3`
  (37.176 against 34.558) and `c4` (25.133 against 21.206). **Those figures are real but
  come from the `w5 = 5` sweep, and the claim is false at the point this phase ships**:
  at `5 / 1 / 0.5 / w4 / 10`, `w4 = 0` and `w4 = 0.25` produce **byte-identical BART
  layouts** — 0 of 50 stations differ, `c2 0.514719 / c3 24.609142 / c4 15.707963` both.
  Round 1 caught it; it is recorded rather than quietly corrected because the wrong
  version is what a reader reconstructs from the sweep table alone.

  So on BART at the shipped weighting, `w4 = 0.25` and `w4 = 0` are **not
  distinguishable**, and no measurement in this repo argues for keeping `c4`. Keeping it
  is therefore a **design call, stated as one**: `c4` is one of §2.3's five criteria with
  its own zero-set, `w4` is a flag someone can raise, and a network whose lines bend where
  BART's do not is exactly the case it exists for. Deleting a criterion because one city
  cannot see it is the overfitting this phase refuses everywhere else. `0.25` prices it
  low; it does not switch it off.

  **The integration points, measured against the shipped tree rather than predicted** —
  the whole blast radius was established by applying the change, running
  `cargo test --workspace`, and reading the failures:

  - **Exactly one test literal changes in the entire workspace**:
    `llika-core/tests/common/mod.rs:PHASE3_TOTAL_COST`, `22.505867` → **`8.565050`**. `t`
    is defined by the weights, so a pinned `t` cannot survive a reweight; that is
    arithmetic, not a regression.
  - **`PHASE3_POSITIONS` survives untouched**, which is the load-bearing half. The
    `cluster_moves: false` layout is byte-identical before and after, so the assertion
    that "pins a layout that must never change again" is not being quietly relaxed — the
    same test's *positions* clause passes unaltered and only its *cost* clause moves.
  - **`llika-core/tests/golden.rs` passes unchanged**, and it is an independent witness
    worth naming: it renders the fixture through `LayoutParams::default()` and compares
    against a committed SVG. It passing *is* the proof that the reweight does not disturb
    the fixture, obtained at no cost.
  - **`llika-core/tests/cost_on_fixture.rs` passes unchanged, by its own foresight.** Its
    header says it "asserts shape and finiteness, never a magnitude — the weights are
    provisional (OQ-2), so any number pinned here would be pinning an artefact of
    something still expected to change." That judgement is why this phase is one literal
    and not thirty.
  - **`gallery/bart.svg` is regenerated; `gallery/sample-network.svg` and
    `gallery/gtfs-fixture.svg` must NOT be.** Both are byte-identical under the new
    weights — verified by `cmp` against the committed files — so re-rendering them would
    produce a no-op diff that falsely implies they moved.
  - **`llika-cli/src/main.rs`'s `--initial-radius` help text is wrong and this phase owns
    it.** It says "1, 2, 3, 5 and 8 all give bit-identical positions", unqualified. That
    reproduces on the fixture and fails on BART.

    **The corrected text must be written against the *new* defaults, not the old**, and
    round 1 found that the two differ in a way that matters. At `5 / 1 / 0.5 / 0.25 / 10`
    on BART, `r_0` of 2, 3, 5 and 8 are bit-identical and `r_0 = 1` differs in **33 of 50**
    stations. At the *old* weights the picture is messier: `r_0 = 1` differs from `r_0 ≥ 2`
    in all 50, and `r_0` of 3, 5 and 8 each differ from `r_0 = 2` in **8 of 50** while
    reaching an identical `t = 112.087766` — cost-saturation, not position-saturation.
    **Position-saturation above 2 is therefore something this phase creates rather than
    something it documents**, which is the honest form of the claim and the one the help
    text should carry. The "all 50" figure belongs to the pre-phase state only.
  - **Four summary lines change wherever they are quoted**, all measured at the new
    weights:

    | | before | after |
    |---|---|---|
    | fixture, defaults | `37.166633 → 11.338720 over 2` | `13.539238 → 4.662836 over 2` |
    | fixture, §1's flags | `54.817110 → 12.747375 over 3` | `48.328948 → 5.357277 over 3` |
    | GTFS fixture feed | `45.227136 → 5.055535 over 3` | `28.079276 → 2.699340 over 3` |
    | BART | `511.450746 → 112.087766 over 3` | `234.460102 → 16.746281 over 6` |

    **Only BART's *picture* changes**; the other three rows are the same layout priced
    differently. A close-out that regenerates art on the strength of a moved number would
    be reading this table wrong.
  - **`c1` stops being the heaviest weight, and four places in the tree say it is.**
    `w_octilinearity` at 10.0 against `w_crossings` 5.0 falsifies
    `rules/layout-search.md`'s "left to `c1`, weighted heaviest",
    `llika-core/src/layout/candidate.rs`'s module header, the doc on
    `llika-core/src/layout/cluster.rs:bridge_overlaps`, and §2.4's own OQ-1 argument
    ("`c1`, weighted 5.0 and the heaviest term in §2.3"). **Two of those are the sources
    `rules/layout-search.md` is generated from, so `/sync-rules` would regenerate the
    error rather than catch it** — which is why this bullet exists rather than a
    close-out line.

    **The design intent survives and the arithmetic is what to write instead.** The
    ranking was always over the weight numbers, not over influence, and the two criteria
    are not commensurable: `c1` counts crossings, so one crossing costs `5.0`, while `c5`
    sums a deviation of at most `π/8` per edge, so the most expensive single off-angle
    edge costs `10 × π/8 = 3.926991`. **A crossing still costs more than any one
    off-angle edge**, which is precisely what "the most visually damaging thing on the
    map" was asserting. The four sites get that sentence, not a deletion.
  - **Two cross-spec writes into `llk-002`, not one, and a third site left deliberately.**
    `specs/import_gtfs_spec.md` §1's end-state block quotes BART's `112.087766` and is a
    live end-state promise, so it is corrected in place with a dated note naming this
    phase — `llk-001` §3's own precedent in reverse, since `llk-002` Phase 4 wrote BART's
    timing into this spec's OQ-9 because OQ-9 owns the question. `llika-gtfs/src/convert.rs`'s
    `draws_the_same_line` doc comment is the second and carries **both** numbers.

    **The third, `specs/import_gtfs_spec.md` §3's OQ-7 resolution, is left as it stands**,
    and the phase says so rather than missing it. It is a dated record of why a decision
    was made, which §6.1 sanctions leaving; and the decision survives, measured — at the
    new weights the merged network costs `16.746281` against the doubled network's
    `25.277674`, so merging is still cheaper. **The sweep counts invert** (3-against-5
    becomes 6-against-3), so the doc comment's phrasing "over 3 sweeps against 5" must be
    rewritten rather than renumbered; `llika-gtfs/tests/real_feed.rs:re_adding_the_absorbed_directions_moves_the_layout`
    asserts only that the layouts differ and passes unchanged.
  - **Test prose moves that no test literal does.** `PHASE3_TOTAL_COST`'s doc comment in
    `llika-core/tests/common/mod.rs` records a provenance — measured by Phase 4's round-1
    reviewer, written into §3 before the code existed — which will not describe the new
    value; it gets this phase's provenance instead. And
    `llika-core/tests/cluster.rs`'s assertion-2 comment quotes `16.222682`, which is
    weight-derived and moves. Neither is a literal the compiler or the suite can catch.
  - **`README.md` line 24 says "All 6 phases shipped — v1 is complete."** A seventh phase
    falsifies it, and it sits one line above the table row the close-out already names.
  - **`gallery/README.md`'s "What it also shows" paragraph is the sentence this phase most
    directly falsifies**, and it is not the summary line. It reads "Not every stroke lands
    on a multiple of 45 degrees … it stops with a handful of edges off-angle and a few
    corridors kinked", then names OQ-2 and says "a second network with a different shape
    is what would settle them. This is that network." **That paragraph asked this phase's
    question and this phase answers it** — 50 of 50 corridors octilinear, `c5` exactly
    zero — so it is rewritten to say what the second network settled, not merely
    renumbered.
- **Exit gate:** `cargo test --workspace` green, and five assertions:
  1. **BART draws to the measured criteria vector at the shipped defaults** — imported
     from `llika-gtfs/tests/fixtures/bart.zip` through `ImportParams::default()`, laid out
     through `LayoutParams::default()`, then **all five** criteria asserted:
     `c1 == 0.0` and `c5 == 0.0` by exact equality, and `c2 == 0.51471863`,
     `c3 == 24.60914245`, `c4 == 15.70796327` to an **absolute** `1e-6`, written inline as
     `(actual - expected).abs() < 1e-6`.

     **Absolute and inline, for two reasons round 2 established.** `common::close` is
     `llika-core/tests/common/mod.rs`'s and is a *relative* comparison
     (`|a − e| ≤ tol · |e|`); `llika-gtfs/tests/common/mod.rs` has no such function and
     `real_feed.rs` is the one test in that crate declaring no `mod common;`, so the name
     does not resolve where this assertion lands. And a relative `1e-6` against a
     six-decimal literal is only sound for values `≥ 0.5` — `c2` at `0.5147…` sits inside
     it with 27% headroom today, and a future re-measure landing `c2` below 0.5 would fail
     a correct implementation. Eight decimals and an absolute bound remove both hazards.

     **The last three are the assertion, and `c1`/`c5` alone would be a gate that passes
     for the wrong reason** — round 1 found this and it is OQ-8's own lesson landing on
     this phase. `c5 == 0` is met by the entire `w4 ≤ 0.5` family, so shipping
     `5 / 1 / 0.5 / 0.25 / **5**` — the doubling of `w5` omitted, which is the one lever
     this phase's mechanism argument names — satisfies `c1 = 0` and `c5 = 0` while drawing
     a materially worse map: `c2` 1.887302, `c3` 34.557519, `c4` 21.205750, measured. Only
     `c2`, `c3` and `c4` tell the shipped point from that near-miss.

     **Exact for `c1` and `c5`, tolerant for the rest.** §2.3 records that all eight unit
     offsets give exactly `+0.0` from `octilinear_deviation`, so a sum over octilinear
     edges is exactly zero and a tolerance would only hide a near-miss; `c1` is an integer
     count. The other three are sums of transcendentals and take `common::close`.

     **`ImportParams::default()`, not `--route-types 1`**, matching
     `llika-gtfs/tests/real_feed.rs:import_bart`: the default is `[0, 1]`, BART's feed
     carries no `route_type = 0`, and both select the same twelve routes. The assertion
     belongs beside `llika_draws_the_bart_network` in that file.

     **What this gate cannot pin, stated rather than left to be rediscovered: `w1`.**
     Round 2 scanned 720 weight tuples against the whole gate and found **exactly two
     survivors — the shipped point and `10 / 1 / 0.5 / 0.25 / 10`** — so every plausible
     mis-implementation of this phase is caught, including `w5` left at 5, `w3` left at 1,
     `w4` left at 0.5 or 2, the runner-up, and every uniform rescaling (assertion 3 is
     scale-sensitive where assertion 1 is scale-invariant, which is what makes the pair
     complementary). The one exception is `w_crossings`: both fixtures and BART reach
     `c1 = 0` at every weighting that survives the other clauses, so nothing in the gate —
     or in the tree — constrains it. That is not a defect of this phase, which leaves `w1`
     alone and states the tuple verbatim in its scope; closing it would need a
     crossing-bearing real fixture, which §1.1 and this phase's second decision both put
     out of scope.

     **This pins a qualitative property to a third-party snapshot that expires.**
     `llika-gtfs/tests/fixtures/bart.md` records the copy valid to 2026-08-30, and its
     "Refreshing it" section calls that a deliberate act because "every literal a gate
     hand-counts from it moves too". A refreshed BART could fail this assertion with
     nothing wrong in the code, so **this phase adds a line to that refresh note** saying
     the criteria vector moves with the feed. It is an amendment to prose, not an entry
     appended to an inventory — that file holds no list, and the hand-counted totals live
     in `real_feed.rs`. It is the first item there that is a property of the *layout*
     rather than of the feed.
  2. **The fixture is unmoved** — `run_layout` at `LayoutParams::default()` on
     `sample_network.json` gives positions bit-identical to the pre-phase layout. Already
     covered by `golden.rs`, so the clause is *confirm it still passes*, not write it
     again; it is listed because a phase that changes a default must state which pictures
     it is promising not to change, and this is that promise.
  3. **`PHASE3_TOTAL_COST` is re-measured, not adjusted until green.** The new value is
     obtained by evaluating the shipped `cost::evaluate` at the `cluster_moves: false`
     layout and reading `total`, and it must equal `8.565050` to `1e-6`. The distinction
     is the assertion: a constant edited until the test passes asserts nothing, which is
     precisely why the old one was measured by Phase 4's reviewer rather than captured.
  4. **`--initial-radius` behaves as its corrected help text claims**, on BART: `r_0` of
     2, 3, 5 and 8 give bit-identical positions, and `r_0 = 1` differs from them. Both
     halves, because the first alone is what the old text asserted and the second is the
     correction. On the fixture all five remain identical, which is what makes the
     over-general claim explicable rather than merely wrong.

     **Its cost is accepted rather than unnoticed.** Five BART layouts run ≈32 s under a
     debug `cargo test` (`r_0 = 8` alone is 19.1 s; ≈2 s release), against
     `llika-gtfs/tests/real_feed.rs`'s current 6.2 s — so this assertion makes that file
     roughly six times slower. That is the price of the only clause that keeps a shipped
     `--help` string honest, and it is paid. An implementer who finds it intolerable drops
     `r_0 = 5`, not the `r_0 = 1` comparison, which is the half that carries the
     correction.
  5. **Determinism across processes**, delegated to `llika-cli/tests/byte_stability.rs`
     and `llika-gtfs/tests/byte_stability.rs` as every phase since Phase 3 has. No source
     file those tests invoke is rewritten here, so this is a confirmation rather than a
     new risk — recorded because skipping it on that reasoning is how a phase discovers
     otherwise.

  **A crossing count on the fixture is not an assertion**: it is 0 before and after, so
  the clause would pass without the phase.
- **Close-out.** Three `rules/` files, not two. **`rules/layout-cost.md`** — its weights
  paragraph names all five values and carries the dominated finding; **its body is at 60
  lines against `max_lines: 60`, so the rewrite must free a line or raise the cap**, the
  way Phases 4 and 5 both named theirs. **`rules/cli.md`**, whose `--initial-radius`
  sentence records that the `--help` overstates — that sentence retires once this phase
  corrects the help text, and the rule keeps position-saturation stated as this phase's
  own doing — and **it too is at its cap, 70 lines against `max_lines: 70`**, so it carries
  the same free-a-line-or-raise-it instruction as `layout-cost.md`; the replacement
  paragraph is likely shorter than what it retires, so this may cost nothing.
  **`rules/layout-search.md`**, for the `c1`-heaviest correction, together with
  the two doc comments it is generated from — regenerating it without those first would
  restore the error.

  **`README.md`**: two summary lines, the "One knob does nothing at the shipped weights"
  paragraph, the "All 6 phases shipped — v1 is complete" status sentence, and a Phase 7
  row on the `llk-001` table. **`gallery/README.md`**: BART's summary line *and* the "What
  it also shows" paragraph. Regenerates **`gallery/bart.svg`** alone — the other two
  gallery files are byte-identical and re-rendering them would fake a diff. Corrects
  **§1's `CORRECTED 2026-08-15` block** with a second dated correction beneath the first,
  not an edit of it. Makes the two `llk-002` writes named above.

  **`CLAUDE.md`: none needed** — its observable line carries no invocation and no
  parameter values, and this phase changes neither what the observable is nor how it is
  produced. Stated rather than omitted, per §3's reconciliation step.

  **`llika-gtfs/tests/fixtures/bart.md`** gets the refresh-note amendment gate assertion 1
  requires — enumerated here because this list is where an implementer counts the
  artifacts, and the assertion alone is easy to read past.

  **`specs/INDEX.md` must be regenerated** by `spec-lint --write-index`: writing `shipped`
  flips the rollup `partial → done`, and `.spec-lint.yaml`'s `index.mode: check` makes a
  stale index a lint failure.

  **Writes `shipped` into `phases[]`.** **Leaves OQ-2 open**, per this phase's second
  decision — the close-out adds one line to its third entry recording which point shipped.
  **`llk-001` §3's OQ-9 is left with a note rather than a correction**: its BART reading
  (0.13 s, 3 executed sweeps, ≈43 ms a sweep) doubles to 6 sweeps at the new defaults, and
  its conclusion — the delta score is schedulable, not urgent — survives that. The
  asymmetry with §1, which *is* corrected, is deliberate: §1 is a live promise about what
  the binary prints, OQ-9 is a dated measurement. **Phase 6's recorded fork is reversed
  and gets one line saying so**: that phase decided "the flag ships with the inertness
  stated … rather than the weights changing first", and this phase takes the other branch
  on both halves.
