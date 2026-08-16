---
title: gtfs-import
sources:
  - llika-gtfs/src/lib.rs
  - llika-gtfs/src/feed.rs
  - llika-gtfs/src/trips.rs
  - llika-gtfs/src/convert.rs
  - llika-gtfs/src/main.rs
covers: >
  the four GTFS tables read and how their optional columns are typed, the
  route-type filter and its default, the rule deciding which stops become
  stations, the colour, name and representative-trip conversions, and what the
  importer reports
max_lines: 85
generated: 2026-08-15
---

# GTFS import

`llika-gtfs` turns a published GTFS feed into a `llika-core/src/io.rs:InputSchema`
and writes it as a file. **It writes a file rather than handing a `Network` to the
layout**: a real feed produces something you will want to edit, and the JSON between
import and layout is where that happens. `llika-core` gains no dependency in return.

`llika-gtfs/src/lib.rs:import` takes a path — a `.zip` or an unpacked directory,
chosen on `is_dir` — and returns the schema with a
`llika-gtfs/src/lib.rs:ImportReport`. `llika-gtfs/src/lib.rs:to_json` is the written
form, pretty-printed with a trailing newline: the file is meant to be hand-edited.

## What is read

Four tables and no others — `stops.txt`, `routes.txt`, `trips.txt`,
`stop_times.txt` — into `llika-gtfs/src/feed.rs:Feed`, each a `Vec` in file row
order, through `llika-gtfs/src/feed.rs:Source`, the one abstraction over the two
input forms. Calendars, transfers, `shapes.txt` and the arrival times are never
opened: this reads GTFS as topology, not as the timetable it is. An archive entry is
read whole rather than streamed, so a city's huge `stop_times.txt` is unhandled.

**Every column that is not unconditionally Required is an `Option` *and* carries
`#[serde(default)]`.** The two do different jobs and a parser missing either dies on
the first real feed: the `Option` covers an empty *cell*, which `csv` hands to serde
as `None`, and the default covers an absent *column*, which serde otherwise reports
as a missing field. So `llika-gtfs/src/feed.rs:Stop` types `stop_lat`, `stop_lon`,
`location_type` and `parent_station` optional, `llika-gtfs/src/feed.rs:Route` both
name columns and `route_color`, and `llika-gtfs/src/feed.rs:StopTime` its `stop_id`.

**Only `location_type` 0 (or empty) and 1 are read at all** —
`llika-gtfs/src/feed.rs:is_stop_or_station`. Entrances, generic nodes and boarding
areas are exactly the rows that may carry no coordinates, and reading them would put
an `Option` position on every downstream station for rows that can never be drawn.

## Which routes, and which stops become stations

`llika-gtfs/src/convert.rs:to_schema` keeps the routes whose `route_type` is in
`llika-gtfs/src/lib.rs:ImportParams`, which **defaults to `0,1`** — tram, streetcar
and light rail, plus subway and metro. Filtering is mandatory rather than a
convenience: a city feed is mostly buses, and the layout costs
`O(iterations · V · r² · E²)`. The field is `Vec<u16>` and not `u8`, because the
Hierarchical Vehicle Type extension puts metro at 401 and runs to 1700.

**A station is emitted iff some kept line's station list references it.** So a
`location_type = 1` parent row is read but emits nothing on its own account, and a
stop only a filtered-out route serves emits nothing either — without the rule both
draw as the stray isolated markers `llika-core/src/io.rs:from_input` accepts as
legal. A referenced id `stops.txt` does not define as a stop or a station is a hard
error, and so is a kept row with an empty coordinate cell: a skip cascades into
`UnknownStation` a step later, naming the wrong problem, and a silent `(0, 0)` puts
a station in the Gulf of Guinea and drags the whole centroid with it.

**Input order is the iteration order.** Stations come out in `stops.txt` row order
and lines in `routes.txt` row order; the `HashMap`s are lookup-only and never
walked to produce output. It is the step upstream of every determinism guarantee
`llika-core` holds, and `llika-gtfs/tests/byte_stability.rs` gates it end to end.

## The conversions

`llika-gtfs/src/trips.rs:representative_stop_ids` picks a route's trip: **the one
with the most stops**, ties going to the earlier `trips.txt` row. The rule is known
to be wrong — a route's longest trip is often a rare special serving a depot — and
stands so the question is settled against a drawn map. Its rows are then sorted by
`stop_sequence` **value**, never row order: the values need not be consecutive, and
the file need not store them in order.

`llika-gtfs/src/convert.rs:stated_color` prefixes `#`, GTFS writing six bare hex
digits where `Line`'s `color` goes straight into an SVG `stroke`. The fallback fires
only on absent, empty or not-six-hex-digits — **an explicit `FFFFFF` is kept as
white**, since GTFS defaults an omitted colour to white, so the two are one value
downstream and two different cells in the file.
`llika-gtfs/src/convert.rs:FALLBACK_PALETTE` is eight colours indexed by position
among the *kept* routes needing one, modulo eight. `llika-gtfs/src/feed.rs:Route`'s
name falls short → long → `route_id`, the last rung stricter than GTFS requires.

`llika-gtfs/src/main.rs:Args` is `--input`, `--output` and `--route-types`, a
comma-separated list — the flag being its field kebab-cased, the rule `rules/cli.md`
states for the sibling binary. `llika-gtfs/src/main.rs:summary` prints the station
and line counts and `kept of seen routes matched`, with no dropped-route count
because nothing here can drop a route yet. Nothing is written until it all succeeds.

**Two rules are not implemented, and both show in the drawn map**: `parent_station`
is read but unused, so a platform is its own station and every interchange draws
twice; and the representative trip is the naive one above.
