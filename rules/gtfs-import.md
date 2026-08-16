---
title: gtfs-import
sources:
  - llika-gtfs/src/lib.rs
  - llika-gtfs/src/feed.rs
  - llika-gtfs/src/stations.rs
  - llika-gtfs/src/trips.rs
  - llika-gtfs/src/convert.rs
  - llika-gtfs/src/main.rs
covers: >
  the four GTFS tables read and how their optional columns are typed, the
  route-type filter and its default, the platform-to-station collapse and the
  fold it needs, the rule deciding which stops become stations and at which row,
  which routes are dropped and which are merged into another line, the colour,
  name and representative-trip conversions, and what the importer reports
max_lines: 115
generated: 2026-08-16
---
# GTFS import

`llika-gtfs` turns a published GTFS feed into a `llika-core/src/io.rs:InputSchema` and
writes it as a file rather than handing a `Network` to the layout: a real feed produces
something you will want to edit, and that JSON is where it happens. `llika-core` gains no
dependency in return. `llika-gtfs/src/lib.rs:import` takes a `.zip` or an unpacked
directory, chosen on `is_dir`, and returns the schema with an `ImportReport`;
`llika-gtfs/src/lib.rs:to_json` pretty-prints it with a trailing newline, to be edited.

## What is read

Four tables and no others — `stops.txt`, `routes.txt`, `trips.txt`, `stop_times.txt` — into
`llika-gtfs/src/feed.rs:Feed`, each a `Vec` in file row order, through
`llika-gtfs/src/feed.rs:Source`, the one abstraction over the two input forms. Calendars,
transfers, `shapes.txt` and arrival times are never opened: this is topology, not the
timetable GTFS is. An entry is read whole, not streamed, so a huge one is unhandled.

**Every column that is not unconditionally Required is an `Option` *and* carries
`#[serde(default)]`** — the `Option` covers an empty *cell*, the default an absent
*column*, and a parser missing either dies on the first real feed. So
`llika-gtfs/src/feed.rs:Stop` types `stop_lat`, `stop_lon`, `location_type` and
`parent_station` optional, `llika-gtfs/src/feed.rs:Route` both names and `route_color`, and
`llika-gtfs/src/feed.rs:StopTime` its `stop_id`.

**Only `location_type` 0 (or empty) and 1 are read at all** —
`llika-gtfs/src/feed.rs:is_stop_or_station`. Entrances, generic nodes and boarding areas
are exactly the rows that may carry no coordinates, and reading them would put an `Option`
position on every downstream station for rows that can never be drawn.

## Platforms collapse into stations

A GTFS `stop_id` is usually a **platform**, and `llika-gtfs/src/stations.rs:collapse_map`
gives every read row the identity of the station a rider changes at: a `location_type = 0`
row takes its `location_type = 1` parent's id, a platform with no parent is its own
station, a station row is itself — one hop, never a chain, since GTFS forbids a station
carrying a `parent_station`. A `parent_station` naming a non-station is a hard error:
falling back to the platform's own id would silently reproduce the duplicated interchange
this exists to prevent.

**It is not an option and not a flag.** Without it two lines through one interchange never
share a corridor, leaving `llika-core/src/model.rs:LineSet` a singleton, `degree` counting
platforms, and every split station drawn twice.

Collapsing makes a trip through two platforms of one station a self-loop, which
`llika-core/src/io.rs:InputError::RepeatedStation` rejects, so
`llika-gtfs/src/stations.rs:resolve` folds **consecutive** duplicates to one. Not data loss
— the rider is at one station — and a line returning later keeps both visits.

## Which routes and stops survive, and in what order

`llika-gtfs/src/convert.rs:to_schema` keeps the routes whose `route_type` is in
`llika-gtfs/src/lib.rs:ImportParams`, which **defaults to `0,1`** — tram, streetcar and
light rail, plus subway and metro. Filtering is mandatory: a city feed is mostly buses, and
the layout costs `O(iterations · V · r² · E²)`. The field is `Vec<u16>` not `u8`, because
the Hierarchical Vehicle Type extension puts metro at 401 and runs to 1700.

A kept route falling below two stations after the fold is then **dropped** —
`llika-gtfs/src/lib.rs:DropReason`, the one live member of `llk-001`'s five conditions —
and the drop is an ordinary outcome that exits 0: one degenerate route in someone else's
data must not make a city unimportable, and no repair exists.

A route left drawing a line an earlier route already drew is then **merged** into it —
`llika-gtfs/src/convert.rs:draws_the_same_line`, true when the two station lists are equal
or exactly reversed, reported as `llika-gtfs/src/lib.rs:MergedRoute`. A feed publishing
each direction as its own route otherwise draws every line twice; the predicate reads no
name, colour or `direction_id`. **Not cosmetic**: the graph is identical, `LineSet` keying
a corridor on an unordered pair, but `llika-core/src/layout/cost.rs:c4_straightness` sums
over *lines*, so an unmerged pair pays every bend twice and lands the layout elsewhere.
Both checks run first: neither a dropped nor a merged route contributes a station or takes
a palette index.

**A station is emitted iff some surviving line references it**, the reference being the
collapsed identity, so the parent row emits and its platforms do not. A stop only a
filtered-out, dropped or merged route serves emits nothing — otherwise it draws as a stray
isolated marker `llika-core/src/io.rs:from_input` accepts as legal. A referenced id
`stops.txt` does not define as a stop or a station is a hard error, and so is a kept row
with an empty coordinate cell: a skip cascades into `UnknownStation` a step later, and a
silent `(0, 0)` drags the centroid into the Gulf of Guinea.

**Input order is the iteration order** — stations in `stops.txt` row order, lines in
`routes.txt` row order, the `HashMap`s lookup-only and never walked to produce output. It
is the step upstream of every determinism guarantee `llika-core` holds, and
`llika-gtfs/tests/byte_stability.rs` gates it end to end. **A collapsed station sits at its
parent's row**, with that row's name and coordinates, and a merged pair keeps the
**earlier** route's id, name, colour and direction — picture decisions, since `llk-001`
§2.2 resolves two stations rounding to one grid cell by array order, first claim wins.

## The conversions

`llika-gtfs/src/trips.rs:representative_stop_ids` picks a route's trip: the one running
**the pattern the most of that route's trips run**, ties going to the pattern whose first
trip is the earlier `trips.txt` row. Not the longest, which is often a rare special serving
a depot — a line no rider has taken. Two trips are one pattern by their **station**
sequence, collapsed and folded, so a per-trip bay assignment cannot split one drawn line
and halve its vote; a modal pattern under two stations is dropped rather than promoting a
rare runner-up. Rows sort by `stop_sequence` **value**, never row order: the values need
not be consecutive, nor stored in order.

`llika-gtfs/src/convert.rs:stated_color` prefixes `#`, GTFS writing six bare hex digits
where `Line`'s `color` goes straight into an SVG `stroke`. The fallback fires only on
absent, empty or not-six-hex-digits — **an explicit `FFFFFF` is kept as white**, since GTFS
defaults an omitted colour to white, so the two are one value downstream and two cells in
the file. `llika-gtfs/src/convert.rs:FALLBACK_PALETTE` is eight colours indexed by position
among the *surviving* routes needing one, modulo eight; `llika-gtfs/src/feed.rs:Route`'s
name falls short → long → `route_id`, stricter than GTFS requires.

`llika-gtfs/src/main.rs:Args` is `--input`, `--output` and `--route-types`, a
comma-separated list — the flag being its field kebab-cased, the rule `rules/cli.md` states
for the sibling binary. `llika-gtfs/src/main.rs:summary` prints the station and line
counts, `kept of seen routes matched`, `N dropped` and `N merged`, the last appended rather
than placed by meaning so an existing gate's substring stays true. In
`llika-gtfs/src/lib.rs:ImportReport`, `routes_kept` counts the routes that **matched the
filter**, not the line count: seen is kept plus filtered, lines written are kept minus
dropped minus merged. *Which* route dropped or merged stays in the struct, off stdout.
Nothing is written until it all succeeds.
