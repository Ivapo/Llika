# Llika

**Llika** is Quechua for *net* — a mesh woven from threads. A transit schematic is
exactly that: a mesh of stations and lines, drawn as threads that run parallel along a
shared trunk and converge to a single point at every real interchange.

This is a schematic map generator. It takes a real transit network — stations with real
latitudes and longitudes, lines as ordered station lists — and draws it in the
straight-line, mostly-45-degree style of a transit poster, trading geographic accuracy
for legibility.

```console
$ llika --input network.json --output bayside.svg
wrote bayside.svg — 17 stations, 3 lines, grid 2270m
```

Nothing in the pipeline is specific to a metro. A network is stations and ordered line
lists, which a tram or bus system satisfies as readily as an underground.

## Status

**Early. 2 of 6 phases shipped.** The pipeline runs end to end and writes a real SVG,
but the layout intelligence is not built yet — stations are snapped to a grid and drawn,
with no optimisation pass. Expect a recognisable network, not yet a transit poster.

| Phase | | |
|---|---|---|
| 1 | Thin end-to-end slice: JSON in, SVG out | ✅ shipped |
| 2 | The five cost criteria | ✅ shipped |
| 3 | Single-station hill-climbing | drafted, blocked |
| 4 | Cluster moves | drafted |
| 5 | Line-bundling renderer | drafted |
| 6 | Full parameter surface | drafted |

Phase 3 is where the output starts to look schematic. It is blocked on a question about
the source paper's move-rejection rule, and on a gate that turned out to be unsatisfiable
against the test fixture — both recorded as open questions in the spec.

## Try it

```console
$ git clone https://github.com/Ivapo/Llika && cd Llika
$ cargo run -p llika-cli -- \
    --input llika-core/tests/fixtures/sample_network.json \
    --output map.svg
```

Open `map.svg` in any browser.

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
3. **Hill-climbing** *(phase 3)* against five weighted criteria — crossings, edge length,
   angular resolution at a station, line straightness, and four-gonality, the penalty
   that pulls every edge toward a multiple of 45°. Five separable terms rather than one
   fused score, so each becomes an independent slider.
4. **SVG**, with the y-axis flipped once — latitude increases north, SVG `y` increases
   down.

Given the same input and parameters, output is byte-identical across processes. Input
order is the iteration order everywhere it is observable, which is what makes that true
rather than merely intended.

## Layout

A Cargo workspace, two crates:

- **`llika-core`** — the library: data model, projection, grid, layout, renderer. Parse,
  lay out and render are each public in their own right, so a caller can parse a file
  once and re-render on every parameter change.
- **`llika-cli`** — a thin binary named `llika`.

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
