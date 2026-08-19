# Llika

[![CI](https://github.com/Ivapo/Llika/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Ivapo/Llika/actions/workflows/ci.yml)

**Llika** is Quechua for *net* — a mesh woven from threads. A transit schematic is
exactly that: a mesh of stations and lines, drawn as threads that run parallel along a
shared trunk and converge to a single point at every real interchange.

This is a schematic map generator. It takes a real transit network — stations with real
latitudes and longitudes, lines as ordered station lists — and draws it in the
straight-line, mostly-45-degree style of a transit poster, trading geographic accuracy
for legibility.

```console
$ llika --input network.json --output bayside.svg
wrote bayside.svg — 17 stations, 3 lines, grid 2270m, cost 13.539238 → 4.662836 over 2 iterations
```

Nothing in the pipeline is specific to a metro. A network is stations and ordered line
lists, which a tram or bus system satisfies as readily as an underground.

## Status

**v1 is complete — all seven of its phases shipped.** The pipeline runs end to end and reads as a
transit diagram: stations are snapped to a grid and then hill-climbed against five
weighted criteria, one station at a time and then in rigid groups, and lines sharing a
corridor are drawn as parallel strokes that converge to a single point at a real
interchange. Expect junctions that fan out evenly, lines that do not kink, bundled
trunks, and — on BART — every one of its 50 corridors on a multiple of 45 degrees.
Every layout and render parameter has a flag, so a good first result can be improved
by eye.

**Two real networks are committed here now**, and they are what the weights are judged
against: BART's 50 stations and the MBTA's 119, both in `gallery/`. The second was
added because one city cannot calibrate five criterion weights — and it earned its
keep immediately, by being the first network in this repository that the crossings
weight can be seen to decide anything about.

| Phase | | |
|---|---|---|
| 1 | Thin end-to-end slice: JSON in, SVG out | ✅ shipped |
| 2 | The five cost criteria | ✅ shipped |
| 3 | Single-station hill-climbing | ✅ shipped |
| 4 | Cluster moves | ✅ shipped |
| 5 | Line-bundling renderer | ✅ shipped |
| 6 | Full parameter surface | ✅ shipped |
| 7 | The weights, corrected against a real network | ✅ shipped |
| 8 | `w_crossings`, re-judged | 📋 drafted, post-v1 |

Phase 8 is **not part of v1** and may yet be cut. It exists because the second real
network below found the first concrete evidence against a shipped weight — one crossing
that a higher `--w-crossings` removes, at a measured cost in line straightness — and a
finding with no phase attached is one nothing will ever force.

**GTFS import is done too.** A separate spec adds `llika-gtfs`, a second binary that
turns a published GTFS feed into a network file the one above draws. Platforms collapse
into the stations a rider changes at, so an interchange draws once and the lines through
it share a corridor, and each route draws the line it is normally operated as. And it
has now been run on two real cities: BART's feed, committed to this repository and
drawn in `gallery/bart.svg`, and the MBTA's, which is **not** committed — at 18.6 MB it
would be re-committed on every refresh, so what lands instead is the network it imports
to, in `llika-gtfs/tests/fixtures/golden/mbta.json` with its provenance beside it. That
one is drawn in `gallery/mbta.svg`.

| Phase | | |
|---|---|---|
| 1 | A feed becomes a drawable network | ✅ shipped |
| 2 | Platforms collapse to stations | ✅ shipped |
| 3 | The representative trip | ✅ shipped |
| 4 | A real city | ✅ shipped |
| 5 | The tables are streamed, not collected | ✅ shipped |
| 6 | A second city, of a different shape | ✅ shipped |

Out of scope, deliberately: station-name labels, a GUI, and importing from
OpenStreetMap.

## Try it

```console
$ git clone https://github.com/Ivapo/Llika && cd Llika
$ cargo run -p llika-cli -- \
    --input llika-core/tests/fixtures/sample_network.json \
    --output map.svg
```

Open `map.svg` in any browser.

## Importing a real network

`llika-gtfs` reads a published GTFS feed — a `.zip` or an unpacked directory, from
disk; it never goes to the network — and writes a network file:

```console
$ llika-gtfs --input llika-gtfs/tests/fixtures/bart.zip --output bart.json --route-types 1
wrote bart.json — 50 stations, 6 lines, 12 of 14 routes matched, 0 dropped, 6 merged
$ llika --input bart.json --output bart.svg
wrote bart.svg — 50 stations, 6 lines, grid 3322m, cost 234.460102 → 16.746281 over 6 iterations
```

That is a real feed and those are real numbers — BART's published archive is
committed at that path, with its provenance and licence in `bart.md` beside it. The
map it draws is `gallery/bart.svg`.

**Two commands, because the file between them is the point.** A metro network as
published carries a depot spur nobody rides, a station name in shouting caps and one
branch that makes the map worse. A schematic map is a designed object, not a projection
of a database, and `bart.json` is an ordinary input file — readable and hand-editable —
where that editing happens before anything is drawn.

`--route-types` takes a comma-separated list of GTFS `route_type` values and defaults to
`0,1`: tram, streetcar and light rail, plus subway and metro. Filtering is not optional.
A city feed is mostly buses, a schematic poster of a bus network is a different design
problem, and the layout is superlinear in both stations and edges.

Import is lossy on purpose, so it says what it did:

```console
$ llika-gtfs --input feed --output city.json
wrote city.json — 11 stations, 5 lines, 6 of 7 routes matched, 1 dropped, 0 merged
```

**A station is the thing a rider changes at, not a platform.** A GTFS stop with two
directions is two ids and an interchange can be six, so the platforms of one station
are merged into it — otherwise every interchange draws twice and no two lines ever
share a corridor. A stop that names no parent is its own station, which is the right
reading of a feed that does not model them.

**A route draws the line it is normally operated as.** A published route has many trips
— two directions, short-turns, weekend patterns, one-off specials — and only one of them
can be the line on the map. The one drawn is the pattern the most of its trips run, which
is not the longest: a route's longest trip is often a rare special that runs twice a year
and serves a depot, so drawing it puts a line on the poster no rider has taken.

**Two routes that draw one line become one line.** A feed may publish each direction
as its own route — BART's `Yellow-N` beside `Yellow-S`, same colour — and drawing both
puts every line on the poster twice. A route whose stations are another's, equal or
exactly reversed, is absorbed into the earlier one, and `merged` above is how many. The
surviving line keeps the feed's own name, so `Yellow-S` is what you will find in the
file; renaming it to `Yellow` is the kind of edit the file exists for.

A route left with fewer than two stations once its platforms merge is **dropped**, and
`dropped` above is how many. That is an ordinary outcome rather than a failure: the
file is still written and still drawn, because one degenerate route in someone else's
published data should not make a whole city unimportable.

Only the topology is read — stops, routes, trips and stop times. Calendars,
frequencies, transfers, fares and `shapes.txt` are ignored; the real track geometry is
precisely what schematization throws away.

## Tuning

The first automatic result is meant to look good with no tuning. The flags are there to
improve a good result, not to rescue a bad default.

Every layout and render parameter has a flag, and the flag is always the field name
kebab-cased — no exceptions to remember:

| | |
|---|---|
| `--grid-spacing <m>` | cell size in metres. Default: the network's median edge length |
| `--iterations <n>` | ceiling on search sweeps. The search stops as soon as one moves nothing |
| `--initial-radius <rings>` | how far a station may move on the first sweep, 1–64 |
| `--cluster-moves <bool>` | translate whole bridge-side groups as well |
| `--w-crossings`, `--w-edge-length`, `--w-angular-resolution`, `--w-straightness`, `--w-octilinearity` | the five criteria weights |
| `--units-per-cell`, `--margin-cells`, `--stroke-width`, `--bundle-spacing` | the drawing |

`--bundle-spacing 0` turns bundling off, and `--iterations 0` snaps without searching.

The same surface as a file, for the settings worth keeping:

```console
$ cat tuned.json
{ "layout": { "w_crossings": 9.0 }, "render": { "stroke_width": 8.0 } }
$ llika --input network.json --output map.svg --params tuned.json --stroke-width 10
```

Name only the fields you care about; the rest take their defaults. Individual flags
override the file field by field, so the run above draws at stroke width 10. A
misspelled key is an error rather than a silent default — the whole point of a knob you
are tuning by eye is that it took effect.

One knob saturates at the shipped weights, and says so in `--help`: `--initial-radius`.
Beyond two rings it buys nothing — on BART 2, 3, 5 and 8 draw the same map, and on the
17-station fixture all of 1 through 8 do — while each extra ring costs `O(r²)`
candidates a sweep. It is not inert: on BART, `r_0 = 1` is a different map.

## Input format

One JSON file. A line's edges are the consecutive pairs of its **ordered** station list,
which is what makes a shared corridor a topological fact rather than a geometric guess.

```json
{
  "stations": [
    { "id": "central", "name": "Central", "lat": 37.780000, "lon": -122.416590 },
    { "id": "market",  "name": "Market",  "lat": 37.779102, "lon": -122.392722 }
  ],
  "lines": [
    { "id": "red", "name": "Red Line", "color": "#E4002B",
      "stations": ["central", "market"] }
  ]
}
```

Five conditions are hard errors, never warnings and never silently repaired: an unknown
station reference, a duplicate station id, a duplicate line id, a line with fewer than
two stations, and the same id twice consecutively. A station referenced by no line is
legal — an isolated node is a degenerate but drawable map.

## How it works

Three coordinate systems, then a search.

1. **Projected plane, in metres.** A hand-rolled equirectangular projection centred on
   the network's centroid. At single-city scale a projection crate would buy accuracy the
   layout step immediately discards.
2. **An integer grid.** Cell size defaults to the *median edge length* of the network
   itself, so a typical edge is exactly one cell for any city at any scale. One station
   per cell; collisions spiral outward deterministically.
3. **Hill-climbing** against five weighted criteria — crossings, edge length, angular
   resolution at a station, line straightness, and four-gonality, the penalty that pulls
   every edge toward a multiple of 45°. Five separable terms rather than one fused score,
   so each becomes an independent slider. Every station is tried against each free cell
   within a radius that shrinks over the run; three hard rules reject the moves that
   would tear the network, and everything else is left to the cost.
   Then whole **clusters** move rigidly, which gets the search out of a dead end no
   single station can: a group hanging off the map by one long edge cannot shorten that
   edge by moving any one of its members. A cluster is the smaller side of a bridge —
   structural and parameter-free, rather than a length threshold that would have to be
   right for every city at once.
4. **SVG**, with the y-axis flipped once — latitude increases north, SVG `y` increases
   down.

Given the same input and parameters, output is byte-identical across processes. Input
order is the iteration order everywhere it is observable, which is what makes that true
rather than merely intended.

## Layout

A Cargo workspace, three crates:

- **`llika-core`** — the library: data model, projection, grid, layout, renderer. Parse,
  lay out and render are each public in their own right, so a caller can parse a file
  once and re-render on every parameter change.
- **`llika-cli`** — a thin binary named `llika`.
- **`llika-gtfs`** — the GTFS importer, library and binary. It depends on `llika-core`
  for the schema types and on nothing of its layout or renderer, and adds no dependency
  to it in return.

## Development

This repo is developed spec-driven, with two kinds of document doing one job each:

- **`specs/`** — why a decision was made, and the plan. Append-only once accepted; it
  does not track the code.
- **`rules/`** — what is true right now. It *does* track the code, and is corrected
  against its own declared sources.

Review rounds are recorded in `specs/reviews/`, including the findings that were wrong.
Start at `specs/INDEX.md`.

```console
$ cargo test --workspace
$ cargo clippy --workspace --all-targets
```

## Reference and provenance

The layout method follows Stott, Rodgers, Martinez-Ovando and Walker, *Automatic Metro
Map Layout Using Multicriteria Optimization*, IEEE TVCG 17(1):101–114, 2011
([doi:10.1109/TVCG.2010.24](https://doi.org/10.1109/TVCG.2010.24)). The cost criteria
here are this project's own operational definitions of the criteria that paper names,
not quotations from it. Label placement is out of scope; the line-bundling renderer is
original, as the paper covers layout and not rendering.

[LOOM](https://github.com/ad-freiburg/loom) (Bast, Brosi, Storandt, Univ. Freiburg) is
prior art whose papers are worth reading. It is GPL-3.0 and **no LOOM source is vendored
into this tree**.

## Licence

MIT — see [LICENSE](LICENSE).
