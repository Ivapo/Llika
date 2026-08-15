---
title: data-model
sources:
  - metro-core/src/model.rs
  - metro-core/src/io.rs
covers: >
  station/line/network types, the corridor edge and its line set, the
  five-condition input error contract, and input order as iteration order
max_lines: 50
generated: 2026-08-14
---

# Data model

`metro-core/src/io.rs:InputSchema` is the whole input file: `stations` and `lines`,
both `Vec`, both deserialized by serde. `metro-core/src/model.rs:Station` carries
`id`, `name`, `lat`, `lon`; `metro-core/src/model.rs:Line` carries `id`, `name`,
`color` and an **ordered** station id list.

`metro-core/src/model.rs:Network` wraps a `petgraph` `UnGraph<usize, LineSet>`.

**The edge is the corridor.** `metro-core/src/io.rs:from_input` walks each line's
consecutive pairs and, where an edge already joins that pair, inserts the line index
into the existing edge rather than adding a second edge. So
`metro-core/src/model.rs:degree` counts corridors: a station where two lines run
side by side is degree 2, not degree 4. `metro-core/src/model.rs:lines_at_station`
unions the line sets of the incident edges;
`metro-core/src/model.rs:lines_between` reads one corridor's set.

**Input order is the iteration order everywhere it is observable.** `stations` and
`lines` are vectors in input-file order, graph node indices coincide with station
indices (nodes are added in a single ordered pass), and
`metro-core/src/model.rs:LineSet` is a `BTreeSet<usize>` of line indices. The
`index_by_id` `HashMap` is lookup-only and is never walked. Rust seeds its default
hasher per process, so a map iterated to produce output would make the same input
render differently in a second process.

## The error contract

`metro-core/src/io.rs:InputError` has exactly five variants, all hard errors, none
repaired: `UnknownStation`, `DuplicateStation`, `DuplicateLine`, `LineTooShort`
(fewer than two stations), `RepeatedStation` (the same id twice consecutively, a
self-loop). Checks run in a fixed order — duplicate stations, duplicate lines, then
per line: length, consecutive repeat, unknown reference — so an input violating two
conditions always reports the same one.

**A station referenced by no line is legal**, and so are two stations at identical
coordinates. Both are drawable, and both are covered by tests; the numeric hazard
they create lives in `rules/projection-grid.md`, not here.

JSON syntax errors are `serde_json`'s and come back from
`metro-core/src/io.rs:parse_input`, deliberately a different type from the five
semantic conditions.
