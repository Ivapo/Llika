---
id: llk-002
title: gtfs-network-import
note: >
  Turning a published GTFS feed into a Llika input file — stop and route tables
  read, platforms collapsed to stations, one representative trip per route — so
  the schematic map can be drawn from a real city rather than a hand-authored
  fixture.
status: accepted
last_updated: 2026-08-15

phases:
  - name: "Phase 1 — thin end-to-end slice: a feed becomes a drawable network"
    reviewed: 2026-08-15
    shipped: 2026-08-15
    cut: null
    by: null
  - name: "Phase 2 — station identity: platforms collapse to stations"
    reviewed: 2026-08-15
    shipped: null
    cut: null
    by: null
  - name: "Phase 3 — the representative trip"
    reviewed: 2026-08-15
    shipped: null
    cut: null
    by: null
  - name: "Phase 4 — a real city"
    reviewed: 2026-08-15
    shipped: null
    cut: null
    by: null

extends: null
supersedes: null
superseded_by: null
related: [llk-001]
reference: >
  The GTFS Static reference (gtfs.org/schedule/reference). Out of scope from it:
  everything temporal — `calendar`, `calendar_dates`, `frequencies`, and the
  arrival/departure times in `stop_times` — plus `shapes`, fares, transfers,
  pathways and accessibility. This spec reads GTFS as a **topology** format and
  discards the timetable it actually is. Nobody in this repo has yet validated
  the field list in §2.1 against a real feed; §4's Phase 4 is where that happens,
  and a difference found there is a recorded correction, not a defect.
---

# GTFS network import

## 1. Goal

`llk-001` ships a pipeline that draws a schematic map from a JSON file of
stations and lines. Nothing produces that JSON. The only network in the tree is
`llika-core/tests/fixtures/sample_network.json`, seventeen stations hand-authored
to exercise the layout's gates — so the project draws its own fixture and has
never drawn a city.

This spec closes that. It reads a published GTFS feed and writes a
`llk-001` input file.

**The observable is `llk-001`'s observable** — the schematic map, an octilinear
straight-line transit diagram delivered as one static SVG. This spec produces no
observable of its own; it produces the *input* to one, and every phase below is
judged on whether a real network comes out the far end drawn. A phase that
produces only a better JSON file is argued for explicitly.

The end state, concretely — two commands, because the intermediate file is the
point:

```console
$ llika-gtfs --input bart.zip --output bart.json --route-types 1
wrote bart.json — 50 stations, 6 lines, 7 of 213 routes matched, 1 dropped

$ llika --input bart.json --output bart.svg
wrote bart.svg — 50 stations, 6 lines, grid 3410m, cost 812.431907 → 190.774521 over 14 iterations
```

**Every number in that second block is illustrative and none has been measured**,
which is stated here rather than discovered later: `llk-001` §1 wrote an invented
cost pair into its own end-state block and carried it through six phases before
Phase 6 corrected it in place, wrong by two orders of magnitude. The `llika` line's
*format* is real — six decimals and the **executed** sweep count, per
`llika-cli/src/main.rs:run` — and Phase 4 is where the figures are replaced with
what the binary printed.

`bart.json` is an ordinary `llk-001` input file: readable, hand-editable, and
accepted by `llika-core/src/io.rs:Network::from_input` unchanged.

### 1.1 The two steps stay two steps (decision, recorded)

The importer writes a file; it does not hand a `Network` to the layout. Three
reasons, and the third is the one that would be expensive to discover later.

**A real feed produces something you will want to edit.** A metro network as
published carries a depot spur nobody rides, a station name in shouting caps, a
`route_color` of `FFFFFF`, and one branch that makes the map worse. The
schematic-map tradition is editorial — a poster is a designed object, not a
projection of a database. An inspectable JSON file between import and layout is
where that editing happens, and it costs nothing because
`llika-core/src/io.rs:InputSchema` already derives `Serialize`.

**It keeps `llk-001` §2.6's boundary intact.** That section built the whole API
around parsing once and re-laying-out on every parameter change, so a Tauri
command can hold a `Network` in memory across slider drags. An importer wired
directly into the pipeline would put a zip decode behind that boundary.

**And it means this spec contradicts nothing `llk-001` shipped.** `llika`'s
invocation, its thirteen flags and its `--params` file are untouched; there is no
new subcommand and no restructured CLI. Under the methodology's §6.1 that is the
difference between a `related:` edge and a `supersedes:` one, and the design that
earns the cheaper edge is the better design here on its merits anyway.

### 1.2 Non-goals

- **No network access.** Input is a local `.zip` or a local directory. No
  Overpass, no HTTP, no feed registry. A downloader is a different program and it
  makes every test depend on someone else's uptime.
- **No OpenStreetMap.** It is the reserved sibling of this spec (§2.7), not a
  mode of it.
- **No timetable.** Times, calendars, frequencies and transfers are read past.
  This is a topology importer; see the `reference` note.
- **No `shapes.txt`.** The real track geometry is precisely what schematization
  throws away — `llk-001` draws straight strokes at multiples of 45 degrees, and
  a polyline of the true alignment has nowhere to go.
- **No click-to-place.** `llk-001`'s OQ-4 named two candidate input methods and
  this spec takes one of them. The other is a GUI feature and `llk-001` §1.1
  defers GUIs whole.
- **No label placement**, still. Importing real station *names* does not make
  them drawable; that remains `llk-001` §1.1's deferred sub-problem, and the
  names ride in the JSON unused.

## 2. Design

### 2.1 What is read, and what it becomes

Four files of a GTFS feed, and no others:

| file | columns used | of these, optional in the feed |
|---|---|---|
| `stops.txt` | `stop_id`, `stop_name`, `stop_lat`, `stop_lon`, `location_type`, `parent_station` | all but `stop_id` — see below |
| `routes.txt` | `route_id`, `route_short_name`, `route_long_name`, `route_type`, `route_color` | both names, `route_color` |
| `trips.txt` | `route_id`, `trip_id` | neither |
| `stop_times.txt` | `trip_id`, `stop_id`, `stop_sequence` | `stop_id`, under GTFS-Flex |

**Every column above that is not `Required` unconditionally is read as an
`Option`**, and the third column says which. This is stated as a rule rather than
left to the table because the reflex parser types them all bare and dies on the
first real feed: `stop_lat`/`stop_lon` are **Conditionally Required**, present for
platforms and stations but *optional* for generic nodes and boarding areas, and
`stop_times.stop_id` is forbidden outright on a GTFS-Flex row.

**`location_type` has five values, not two.** `0` (or empty) is a stop or
platform, `1` a station, `2` an entrance or exit, `3` a generic node, `4` a
boarding area — and 2, 3 and 4 are exactly the rows that may carry no coordinates
while *requiring* a `parent_station`. **Only `0` and `1` rows are read at all**,
and the reason is the parse rather than the picture: those three are the rows whose
coordinates may be absent, and reading them means every downstream station carries
an `Option` position for the sake of rows that can never be drawn. A `stop_times`
row may only reference `location_type` 0 or empty, so an entrance could not reach a
line's station list even if it were read — the exclusion keeps it out of the *parse*
and out of §2.2's collapse targets, not out of the map.

**Which rows become stations, stated once because §2.2 changes the answer and two
sections would otherwise disagree.** A `Station` is emitted for a stop **iff some
kept line's station list references it**, and the reference is whatever identity
is current — the platform's own `stop_id` before §2.2 lands, the parent's after.
So `location_type = 1` rows are read (they are the collapse targets) but emit
nothing on their own account, and a platform serving only a filtered-out route
emits nothing either. Without this rule the parent rows draw as stray isolated
markers, which `llika-core/src/io.rs:Network::from_input` accepts as legal and
which nobody wants on a poster.

**A route dropped under OQ-3 is not a kept line**, so it references nothing and
contributes no stations. Stated as a rule rather than left to that intent: the
station a dropped route uniquely served would otherwise emit as exactly the stray
isolated marker the paragraph above exists to prevent, and the two readings differ
by one in a literal Phase 2's gate commits to.

The mapping onto `llika-core/src/io.rs:InputSchema` is otherwise direct, which is
the reason GTFS is the first importer and not OSM (§2.7):

- a **stop** becomes a `llika-core/src/model.rs:Station` — `stop_id` → `id`,
  `stop_name` → `name`, `stop_lat`/`stop_lon` → `lat`/`lon`;
- a **route** becomes a `llika-core/src/model.rs:Line` — `route_id` → `id`, a
  name from the two name columns (§2.4), `route_color` → `color`;
- a route's **representative trip**, its `stop_times` sorted by `stop_sequence`,
  becomes that line's ordered station list. This is the whole reason the ordered
  list in `llk-001` §2.1 is a topological fact rather than a geometric guess: GTFS
  states it outright.

`stop_sequence` is required to increase along a trip but **not** to be
consecutive — feeds using 10, 20, 30 are common — so the sort key is the value
and never the row order or a difference between values. **Nor is the file required
to store the rows in that order**, which is the property a gate has to be keyed to:
ascending values in ascending row order are read identically by a correct reader
and by one that ignores `stop_sequence` entirely.

*(One column is deliberately absent and would have to be added: `trips.direction_id`,
which OQ-1 names as a candidate for choosing a route's representative trip. If OQ-1
resolves that way, this table gains it and Phase 3 says so.)*

### 2.2 Station identity is the load-bearing problem (decision, recorded)

A GTFS `stop_id` is usually a **platform**, not a station. A metro stop with two
directions is two `stop_id`s; an interchange served by three lines can be six.
`stops.txt` records the relationship in two columns: a row with
`location_type = 1` is a parent station, and a `location_type = 0` row carries
that row's id in `parent_station`. §2.1 is what keeps the other three location
types out of this rule.

**Imported naively, this destroys the one invariant `llk-001` is built on.**
`llika-core/src/io.rs:Network::from_input` makes an edge *the corridor* — a second
line over the same consecutive pair joins the existing edge rather than adding a
second one, which is what `llika-core/src/model.rs:LineSet` records and what the
whole of `llk-001` §2.5 reads to draw bundles. Two lines through one station on
two different platform ids never produce the same pair, so they never share an
edge. The consequences are not subtle:

- `LineSet` stays a singleton everywhere, so **the line-bundling renderer has
  nothing to bundle** and every trunk draws as separate parallel strokes that
  never converge;
- `llika-core/src/model.rs:degree` counts platforms, so a four-line interchange
  reads as four degree-2 stations, and `c3` — the criterion `llk-001` §2.3 says
  "keeps a multi-line interchange legible" — has no interchange to work on;
- the map shows every station twice.

So **platforms collapse into their parent station**, and the collapse is not an
option or a flag: a station is the thing a rider changes at, and that is the
`location_type = 1` row. Where a platform has no `parent_station` it is its own
station, which is the correct reading of a feed that does not model parents at
all.

**Collapsing creates a self-loop, and `llk-001` rejects those.** A trip serving
two platforms of one station in sequence collapses to the same id twice
consecutively, which is `llika-core/src/io.rs:InputError::RepeatedStation`. The
repair is stated here rather than left to an implementer: **after collapsing,
consecutive duplicates in a line's station list are folded to one.** That is not
data loss — the rider is at one station — and it is the only one of `llk-001`'s
five conditions that is an artefact of this step rather than a defect in the
feed. OQ-3 covers the other four.

### 2.3 Which routes, and the answer to "all of them" (decision, recorded)

A city feed is mostly buses. `route_type` distinguishes them — 0 tram or light
rail, 1 subway or metro, 2 rail, 3 bus, 4 ferry, and a longer tail — and a feed
with two hundred bus routes imported whole produces a network that is neither
drawable nor legible.

**Filtering is mandatory, not a convenience.** Two reasons, and the second is
measured rather than aesthetic:

- a schematic poster of a bus network is a different design problem, and nothing
  in `llk-001` was built for it;
- `llk-001`'s OQ-9 measured the layout at **72.9 s release on 200 stations**, with
  a cost of `O(iterations · V · r² · E²)`. A bus network is an order of magnitude
  past that. The importer that hands the layout ten thousand stations is the
  importer that makes `llk-001` look broken.

**`--route-types` takes a comma-separated list of numeric values** —
`--route-types 0,1` — plural because it is plural, and named by `llk-001` Phase
6's rule that a flag is its field kebab-cased with no exceptions. Its default is
OQ-2.

**The numbers above are the base set, and many European feeds do not use them.**
The Hierarchical Vehicle Type extension puts metro at `401` and runs to `1700`,
so a feed can be a perfectly good metro network that `--route-types 1` matches
nothing in. Recorded here rather than solved: whether the filter learns the
extended ranges is a Phase 4 question, discoverable only against a feed that uses
them, and the flag already lets a user name the number themselves.

### 2.4 The small conversions, each with a trap

- **Colour.** `route_color` is six hex digits **without** a leading `#`, and
  `llika-core/src/model.rs:Line`'s `color` goes straight into an SVG `stroke`
  attribute. So it is prefixed.

  **The fallback fires on absent or malformed, and never on a colour the feed
  actually stated** — including `FFFFFF`. The predicate is exactly: the cell is
  missing or empty, or it is not six hexadecimal digits. An explicit white line is
  *kept as white*, and that is not an oversight: §1.1 names a white `route_color`
  as one of the things the human fixes in the intermediate file, and an importer
  that silently overrode it would be making the editorial judgement §1.1 assigns
  to the person. This needs saying because GTFS makes the two look alike — an
  omitted `route_color` **defaults to `FFFFFF`** by the standard — so "missing"
  and "white" are one value downstream and two different cells in the file, and
  the importer distinguishes them at the cell.

  **The fallback palette is fixed, and so is the index.** Eight colours, in order:
  `#E4002B`, `#00843D`, `#0057B8`, `#FF8200`, `#753BBD`, `#00A3E0`, `#FFB81C`,
  `#7C878E` — the first three are the sample fixture's, so an imported map reads
  in the same register as the hand-authored one. The index is the route's position
  among **the kept routes that need a fallback**, counted in `routes.txt` row
  order, modulo eight. Counting only the routes that need one keeps every colour
  in the palette used before any repeats; the modulo is what makes a ninth
  fallback defined rather than a panic.
- **Name.** Prefer `route_short_name`, fall back to `route_long_name`, fall back
  to `route_id`. Both name columns are **Conditionally Required** — the standard
  says at least one must be defined — so the `route_id` rung is stricter than GTFS
  requires and exists because a feed that breaks that rule should still import.
  `llk-001` never draws the name, so this is about the file being readable by a
  person editing it.
- **Coordinates.** Degrees, and **required on every row this spec reads** — GTFS
  makes them Conditionally Required, and the condition is satisfied for exactly the
  `location_type` 0 and 1 rows §2.1 keeps. That is why §2.1's third column types
  them `Option` and this bullet does not contradict it: the `Option` is what
  survives the *parse*, and a kept row that arrives with an empty coordinate cell
  is a **hard error** naming the `stop_id`. Not a skip, which cascades into
  `llika-core/src/io.rs:InputError::UnknownStation` one step later, and not a
  silent `(0, 0)`, which puts a station in the Gulf of Guinea and drags the whole
  network's centroid with it. `llk-001`
  §2.2 projects them; nothing here transforms them.
- **Ids.** GTFS ids are unique per file by the standard, which discharges
  `llika-core/src/io.rs:InputError::DuplicateStation` and `DuplicateLine` — but
  *collapsing* maps many platform ids onto one station id, so the importer must
  emit each parent station **once**, and that is where a duplicate would be
  introduced if it were emitted per platform.

### 2.5 Crate layout

A third workspace crate, mirroring `llk-001` §2.6's split for its reason — the
algorithm is written once in Rust and serves both the CLI and the later Tauri
app:

- **`llika-gtfs`** — library. Feed reading, collapse, trip selection, and the
  conversion to `InputSchema`. **The library** depends on `llika-core` for the
  schema types and on nothing of its layout or renderer — its own tests do call
  `build_schematic_svg`, which is Phase 1's gate 9 and adds no dependency the
  schema types did not already bring.
- its binary, **`llika-gtfs`** — reads a feed, writes a JSON file.

```
llika-gtfs/src/
  lib.rs        import(), ImportParams, ImportReport
  feed.rs       the four tables, read from a .zip or a directory
  stations.rs   parent-station collapse
  trips.rs      representative-trip selection
  convert.rs    Feed -> InputSchema
  main.rs       the binary
```

**`llika-core` gains no dependency and no code.** A zip decoder and a CSV reader
have no business behind the boundary `llk-001` §2.6 built, and the schema types
this crate needs are already public.

New dependencies, both in `llika-gtfs` alone: `csv`, and `zip` for the archive
case. `serde` and `serde_json` are already in the lock via `llika-core`, but the
workspace has no `[workspace.dependencies]` table, so `llika-gtfs` declares its
own — no registry fetch, one manifest edit.

**Input order is the iteration order here too.** `llk-001` §2.2 makes it the rule
everywhere it is observable, and byte-stability across processes rests on it —
`llika-cli/tests/byte_stability.rs` is the standing check. An importer that walks
a `HashMap` of stops to emit the `stations` array produces a different file per
process, and every downstream guarantee `llk-001` proved would evaporate at the
one step upstream of all of them. Stations are emitted in `stops.txt` row order
and routes in `routes.txt` row order.

**A collapsed station takes its parent row's position, not its first platform's**
— and that is a picture decision, not a tidiness one. Determinism holds either
way, so this does not rest on the paragraph above it: what rests on it is
`llk-001` §2.2's grid tie-break, which resolves two stations rounding to one cell
by *`stations` array order, first claim wins*. The two readings hand the same
network to the layout in different orders and get different maps. The parent row
is the choice because it is the row a person editing the JSON would look for, and
GTFS does not require a parent to precede its children.

### 2.6 What the importer reports

Import is lossy by design — routes are filtered out, platforms are merged, and
some routes are dropped (OQ-3). Silence about that is how someone concludes the
tool lost a line. `ImportReport` carries counts: routes seen, routes kept, routes
dropped and why, stops seen, stations emitted. The binary prints the summary line
§1 shows; the library returns the struct, so the later Tauri app can show the same
thing in a panel.

*(Recorded 2026-08-15, at Phase 1's close-out. The shipped summary line stops one
clause short of §1's — `wrote city.json — 14 stations, 6 lines, 6 of 7 routes
matched`, with no `, N dropped`. Nothing in Phase 1 can drop a route: OQ-3 deflates
to `LineTooShort` alone, which is unreachable until platforms collapse. Phase 2 adds
both the drop and the clause. A permanent `0 dropped` would have been format
stability bought with a number that cannot move, and §1's block is the end state
rather than a per-phase contract.)*

### 2.7 Reserved: importers

This spec is one **kind** of importer. The reserved sibling is
**`import_osm_spec.md`** — route relations from OpenStreetMap — and when it is
written it carries `extends: llk-002`, because it is a new subject under the
framework this spec sets up rather than more of this one.

The framework is small and is stated here so the sibling has something to extend:
an importer is a program that turns an external transit data source into a
`llika-core/src/io.rs:InputSchema`, emits it as a file rather than a `Network`
(§1.1), reports what it dropped (§2.6), and adds no dependency to `llika-core`.

**GTFS is first, and OSM is not merely later.** In GTFS the ordered station list
*is* a stored field — a trip's `stop_times` sorted by `stop_sequence` — so this
spec's hard problems are identity (§2.2) and selection (OQ-1). An OSM route
relation stores members whose order is unreliable, mixes stops with platforms and
with the ways between them, and is frequently split into disjoint segments, so
recovering the ordered list is a sub-problem of its own before any of this spec's
work begins. Building the harder one first would have meant debugging the
ordering heuristic and the conversion together, with only a picture to tell them
apart — which is the argument `llk-001` §4 made for splitting its own Phase 2 out
of Phase 3.

Nothing else here is a reserved namespace.

## 3. Open questions

- **OQ-1** — **Which trip represents a route?** A route has many trips: two
  directions, short-turns, express variants, weekend patterns, and one-off
  specials. The station list a line draws with is whichever trip is picked.
  *(design call.)* **Blocks Phase 3**, and Phase 1 ships a deliberately naive
  answer so the question is decided against a drawn map rather than in the
  abstract.

  The naive answer — **the trip with the most stops** — is wrong in a way worth
  recording, because it is what an implementer reaches for: a route's longest trip
  is often a rare special that runs twice a year and serves a depot, so the line
  drawn is one no rider has taken. Candidates: the **modal stop pattern**, the
  sequence shared by the most trips of that route, which is by construction the
  line as normally operated; the longest trip among those with `direction_id = 0`;
  or the union of all patterns, which is a different object — a route's *network*
  rather than a line — and would need `llk-001` to accept a line that is not a
  path.

- **OQ-2** — ~~**What does `--route-types` default to?** *(design call.)* **Blocks
  Phase 1**, which needs a default to run at all.~~ Metro alone (`1`) is the
  narrowest reading and draws nothing at all for a city whose system is trams;
  metro and tram and light rail (`0,1`) covers most of what reads as a metro map;
  adding rail (`2`) pulls in regional networks that sprawl past the poster form.

  **RESOLVED 2026-08-15 by Phase 1, in `llika-gtfs/src/lib.rs:ImportParams`: `0,1`.**
  The middle reading, and the argument is that the two ends each fail a real city
  outright. `1` alone is the narrowest *statement* but not the safest default:
  it hands a tram city an empty file, and §2.6's report saying "0 of 412 routes
  matched" is a correct answer nobody asked for. `2` fails the other way — a
  regional network is an order of magnitude past the 200 stations `llk-001`'s OQ-9
  measured at 72.9 s, so the widest default is the one that makes `llika` look
  broken on the first city someone tries.

  What made this a default rather than a required flag is the half that closed in
  review: an empty match is not an error, so a user whose system this misses gets a
  valid file and a report that says so, and widens the flag themselves. The
  asymmetry is the whole answer — being one mode too narrow is a flag away from
  fixed, and being one mode too wide is a 72-second wait for a map that does not
  read as a poster.

  **Half of this closed during review, against the code, per the methodology's §4
  rule that code-answerable questions are answered in the round rather than at
  implementation time.** The question was what a feed matching nothing should do,
  and whether an empty network is even drawable. It is: `{"stations":[],"lines":[]}`
  run through the shipped `llika` exits 0 and writes a valid 160×160 SVG — measured
  by the round-1 reviewer, not inferred from `llk-001` §2.1, which only blesses an
  isolated *node*. So an empty match is **not** an error; it imports an empty
  network and §2.6's report says nothing matched. What stays open is only the
  default set.

- **OQ-3** — **What happens when a real feed trips one of `llk-001`'s five error
  conditions?** *(design call.)* **Blocks Phase 2.**

  **Only one condition is actually live, and saying so is most of the answer.**
  §2.2 settles `RepeatedStation`: consecutive duplicates are folded, because that
  one is an artefact of collapsing rather than a defect in the feed. §2.4 discharges
  `DuplicateStation` and `DuplicateLine` — GTFS ids are unique per file by the
  standard, and §2.1's emit-once rule is what stops the collapse reintroducing a
  duplicate. `UnknownStation` cannot arise **on a conforming feed**, since §2.1
  emits a station for every id a kept line references — but the ids come from
  `stop_times.txt` and the stations from `stops.txt`, so a `stop_times` row naming
  an id that `stops.txt` omits, or one that resolves to a `location_type` 3 or 4
  row §2.1 does not read, has nothing to resolve to. That is a malformed feed and
  it belongs to Phase 4's conformance work, not to this question.

  That leaves **`LineTooShort`** — a route whose representative trip has fewer than
  two distinct stations after collapsing, which is reachable on a *conforming* feed
  and is what this question is really about.

  The argument against failing the whole import is that one bad route in a
  two-hundred-route feed would make a city unimportable, and the user cannot fix
  someone else's published data. The argument against silent repair is `llk-001`
  §2.1's, which says in terms that none of the five is a warning and none is
  silently repaired — and there is no repair available here anyway: a route with
  one station is not a line, and inventing a second station is not a thing an
  importer may do.

  So the two arguments do not conflict and the shape is settled: **drop the route
  and say so in §2.6's report.** What Phase 2 still has to decide, and the reason
  this stays open rather than becoming a decision in §2, is whether a route dropped
  this way is an ordinary outcome or a *reported failure* — whether the binary
  exits non-zero when it drops one. Ordinary is the likely answer, since Phase 4
  expects real feeds to carry them; a phase that measures how often it actually
  fires is a better place to settle it than this one.

- **OQ-4** — **Which real feed does Phase 4 use, and is it committed?**
  *(needs-input — a licence decision and a download.)* **Blocks Phase 4.** Feeds
  are typically ODbL or CC-BY and often tens of megabytes, so committing one is a
  licensing question and a repository-weight question at once. The alternative is
  a documented download URL and a test that skips when the file is absent, which
  keeps `cargo test` green on a fresh clone at the cost of a gate that does not
  run by default. Recorded rather than guessed because the answer changes what
  Phase 4's gate can assert.

- **OQ-5** — ~~**The fixture feed does not exist and must be authored.**
  *(answerable now — it must be written.)* **Blocks Phase 1.**~~
  **RESOLVED 2026-08-15 by Phase 1**, which wrote it to
  `llika-gtfs/tests/fixtures/feed/` — six CSVs, nineteen stops and seven routes,
  carrying every property below including Phases 2 and 3's. The property-to-row map
  lives in that crate's `tests/common/mod.rs`, beside the literals keyed to it.

  One property needed a shape the list below does not fix, and it is recorded here
  because the list is what a later phase will read: **the fixture's out-of-scope
  route is a bus (`route_type` 3), not a tram.** A tram route would have made Phase
  1's filter assertion depend on OQ-2's answer, and the two questions are supposed to
  be separable. The same route is also the only one serving one stop (`DEP`), so the
  filter is shown to remove a *station* and not merely a line — §2.1's emit rule has
  no other witness in the gate.

  No GTFS data is in
  the tree. Authoring a minimal feed by hand is cheap — four small CSVs — and
  licence-free, and it is the direct precedent of `llk-001`'s own OQ-5, whose
  lesson was that a fixture under-constrained at Phase 1 has to be re-authored at
  Phase 5 with every gate literal keyed to it.

  **What is open here is the file, not the design.** The property list below is
  settled and is not a question; it is stated inside the OQ, rather than in §2,
  because `llk-001`'s OQ-5 has exactly this shape — it fixed its fixture's
  properties in the question and was marked `RESOLVED` by the phase that authored
  the file. The round-1 reviewer read the settledness as a misfiled decision, and
  the precedent is why it stays here.

  So the properties are fixed **now**, before it is written, and each names the
  phase that cannot be gated without it:

  - **A station whose platforms are split**, with a `location_type = 1` parent and
    at least two platform rows pointing at it, served by **two different routes** —
    without it Phase 2's collapse has nothing to merge and its headline assertion
    is vacuous;
  - **`stops.txt` rows that interleave**, so that another *emitted* station's row
    falls strictly between that parent row and its first platform row — say the
    order `A_parent, B, A_p1, A_p2, C`. Phase 2's gate on §2.5's emission position
    is otherwise vacuous, and vacuous in the way that is hardest to see: under the
    natural grouped layout `A_parent, A_p1, A_p2, B, C` the parent-row rule and the
    first-platform-row rule emit the identical array, so the assertion passes for an
    implementation doing the opposite. Interleaved, they give `[A, B, C]` against
    `[B, A, C]`;
  - **a station with no `parent_station` at all, referenced by a kept route**, so
    the "a platform without a parent is its own station" rule is exercised rather
    than assumed. The reference is part of the property, not incidental: §2.1 emits
    only referenced rows, so an unreferenced parentless platform emits nothing and
    the assertion fails for a reason that has nothing to do with the rule;
  - **a kept route whose representative trip collapses to fewer than two distinct
    stations** — the simplest shape is a two-stop route serving two platforms of
    one station. It is what makes `LineTooShort` reachable, which after OQ-3's
    deflation is the only condition that question is still about, and Phase 2's
    gate on it cannot fire without it. It must be a **different** route from the
    consecutive-platforms trip below, which Phase 2 requires to *survive*
    `from_input`. Note the consequence, since Phase 2's gate consumes it: this
    route is a valid line at Phase 1 and is dropped at Phase 2, so the two phases'
    committed **line** counts differ by one as well as their station counts;
  - **a trip that serves two platforms of one station consecutively**, which is
    the self-loop §2.2 folds — and it must be there, because
    `llika-core/src/io.rs:InputError::RepeatedStation` firing on a real feed is
    the failure the fold exists to prevent;
  - **a route whose longest trip is not its modal one**, which is the only thing
    that can discriminate OQ-1's candidates at Phase 3;
  - **a route with exactly one trip**, and **two patterns of a third route tied on
    trip count** — Phase 3's gate needs both, and they are fixed here rather than
    at Phase 3 for this question's whole reason: extending the fixture later moves
    every Phase 1 and Phase 2 literal keyed to it;
  - **at least one route of a filtered-out `route_type`**, so Phase 1's filter is
    shown to remove something rather than passing vacuously;
  - **one trip whose `stop_times` rows are written *out of* `stop_sequence`
    order**, and separately non-consecutive values — 10, 20, 30. **The second alone
    proves nothing**, which is worth stating because it is the natural thing to
    write: ascending values in ascending row order are read identically by a
    correct reader and by one that never looks at `stop_sequence`. Only rows out
    of order separate sorting from not-sorting;
  - **a `location_type = 2` entrance row with no coordinates**, so §2.1's
    read-only-0-and-1 rule and its `Option` coordinates are exercised rather than
    assumed — the case that hard-fails a parser typed off the reflex reading;
  - **exactly one kept route with no `route_color`**, and one whose `route_color`
    is explicitly `FFFFFF`, since §2.4 treats those as *different* and only a
    fixture carrying both can show it. **Exactly one**, because §2.4 indexes the
    palette by position among the kept routes *needing* a fallback: a second
    colourless route moves the first one's index and with it Phase 1's committed
    literal. Every other kept route states a colour.

- **OQ-6** — **Scale.** A real metro feed is a few hundred stations, and
  `llk-001`'s OQ-9 measured 72.9 s release at 200 with a cost of
  `O(iterations · V · r² · E²)`. *(deferred by evidence — it is `llk-001`'s open
  item, not this spec's.)* Blocks nothing here, and this spec is what finally makes
  it measurable on something other than a synthetic: the 200-station network that
  measurement came from was a throwaway and is not in the tree. Phase 4 is where a
  real number appears, and it belongs in `llk-001`'s OQ-9 when it does.

  **One tension, recorded rather than resolved.** `llk-001`'s OQ-9 says the delta
  score "should get [a phase or a spec] **before** §1's promise of 'a real metro
  network' is tested on a city rather than on a fixture" — and Phase 4 runs that
  test without it. The judgement is that a metro network is the small case: §1's
  example is 50 stations against a measurement taken at 200, so Phase 4 sits inside
  the measured envelope and the ordering `llk-001` recommended costs nothing to
  break here. A feed that turns out to be much larger is the case that changes the
  answer, and Phase 4's timing is what would say so.

## 4. Implementation phases

Strictly sequential — each depends on the one before. Each is one plan-mode pass
and each carries the two standing plan steps (a commit plan, a reconciliation
step).

None of these produces an observable of its own; §1 says so. What each produces is
a **better drawn map from real data**, and that is what the phase headers below
claim and what the gates measure.

### Phase 1 — thin end-to-end slice: a feed becomes a drawable network
*Produces the observable: **yes, through `llika`** — a GTFS feed goes in and an
SVG comes out the far end. This phase is large for `llk-001` Phase 1's reason:
every smaller cut produces nothing anyone can see. A CSV reader draws no map and
a collapse rule with nothing to collapse into draws no map. Taking the slice whole
means everything after it improves a picture that already exists.*

- **Scope:** the `llika-gtfs` crate per §2.5, with `csv` and `zip`. `feed.rs`
  reading the four tables of §2.1 from **either** a `.zip` or a directory —
  both, because every published feed is a zip and requiring a manual unzip in the
  phase that promises a real network undercuts the point. `convert.rs` producing
  `InputSchema`, in `stops.txt` and `routes.txt` row order per §2.5. §2.3's
  `--route-types` filter, resolving **OQ-2**. §2.4's colour, name and id
  conversions. The binary, and §2.6's report.

  **No collapse and no trip selection.** `parent_station` is ignored, so a
  platform is its own station; the representative trip is the naive longest, which
  **OQ-1** says is wrong and Phase 3 fixes. Both are deliberate: this phase is
  the plumbing, and shipping the two hard rules alongside it would mean debugging
  three things with one picture to tell them apart.

  **The naive rule still needs a tie-break, and it is the one Phase 3 will need
  anyway**: where two trips of a route tie on stop count — two equal-length
  direction variants, which is the ordinary case rather than the exotic one — the
  earlier in `trips.txt` row order wins. Without it the choice is stable-but-
  arbitrary rather than nondeterministic, so gate 7 would still pass; naming it
  removes a guess for free and keeps `llk-001` §2.2's input-order rule unbroken one
  file over.

  **§2.1's emit-rule is what makes that consistent, and it holds unchanged in both
  phases** — a station is emitted iff a kept line references it. Before the
  collapse the references are platform ids, so the fixture's `location_type = 1`
  parent rows are read and emit nothing; after it they are parent ids, and the
  platforms emit nothing. Neither phase emits an unreferenced row, so neither
  draws a stray marker, and the station count moves for exactly one reason.

  **Author the fixture feed** to every property OQ-5 fixes, including the ones
  Phases 2 and 3 need — it does not exist and cannot be copied.
- **Exit gate:** `cargo test --workspace` green, and:
  1. the fixture feed imports, and the file it writes is **accepted by
     `llika-core/src/io.rs:Network::from_input`** — the whole point of the format,
     and the assertion every later phase's map depends on;
  2. hand-counted literals from the fixture, written into the test by a person and
     not recomputed by the code under test: the station count, the line count, and
     one line's full ordered station list. **Both counts are consumed by Phase 2's
     gate 2**, which is why they are written down here rather than derived there.

     **The ordered list reads a route with a single trip pattern**, named in the
     test. Keyed to OQ-5's longest-≠-modal route it would break at Phase 3 by
     design, and churning a literal across phases is what this document works
     elsewhere to avoid;
  3. the `route_types` filter **removes** the fixture's out-of-scope route, and
     the count §2.6 reports matches;
  4. **the trip whose rows are written out of `stop_sequence` order reads in
     `stop_sequence` order** — asserted as a full ordered station list against a
     hand-written literal, and on a route whose representative trip is stable
     across OQ-1's resolution, for assertion 2's reason. This is the only assertion
     that separates a reader that sorts from one that does not, and the naive
     version of it — non-consecutive values in ascending row order — passes on
     both;
  5. the `location_type = 2` entrance row **does not fail the parse**, and is not
     a station. **The first half is the one that discriminates**: the second holds
     under §2.1's emit rule whether the `location_type` filter exists or not, since
     no line references the entrance — while a parser typed off the reflex reading
     of §2.1 dies on that row's empty coordinates before either half is reached;
  6. a route with no `route_color` gets the first palette colour; a route whose
     `route_color` is `FFFFFF` is drawn **white**, unchanged. Both halves, since
     §2.4 treats them as different cases and the fallback is where an implementer
     would collapse them;
  7. the same feed imported twice in **two separate processes** gives
     byte-identical JSON. Two in-process runs would not do, for
     `llika-cli/tests/byte_stability.rs`'s reason: Rust seeds its default hasher
     per process;
  8. both input forms — the `.zip` and the unpacked directory — produce
     byte-identical output;
  9. `llika` draws the imported file without error. **Not through the binary**:
     `CARGO_BIN_EXE_llika` is only set for integration tests of the package
     declaring that binary, and §2.5 points the dependency from `llika-gtfs` at
     `llika-core` and not at `llika-cli`. So this calls
     `llika_core::build_schematic_svg` on the imported schema, which is the same
     pipeline and keeps the dependency direction the crate layout pins.

  Then the human half, which does not carry the gate: open the SVG. It will show
  every interchange twice; that is Phase 2's subject and not a defect here.
- **Close-out:** seeds `rules/gtfs-import.md`. Updates `README.md`, which today
  lists GTFS import under "out of scope for v1". Records the resolution of
  **`llk-001`'s OQ-4** in that spec — it asked which input method follows v1 and
  this is the answer, and §4's "resolve inline, don't delete" means the question
  gets its `RESOLVED` note rather than being left open in a spec that is `done`.
  Commit the crate, the fixture feed, and the workspace member.

### Phase 2 — station identity: platforms collapse to stations
*Produces the observable: **yes**, and this is the phase where the imported map
starts to read as a transit diagram rather than a duplicated one. Interchanges
merge, `LineSet` starts recording more than one line per edge, and the
line-bundling renderer `llk-001` Phase 5 shipped finally has something to bundle
on real data.*

- **Scope:** `stations.rs` implementing §2.2 — platforms collapse into their
  `location_type = 1` parent, a platform without a parent is its own station, and
  consecutive duplicates in a line's station list fold to one. §2.5's parent-row
  emission position. Resolve **OQ-3** — which, as that question now records, is
  `LineTooShort` alone — and implement the answer, with §2.6's report gaining the
  dropped-route counts.
- **Exit gate:** `cargo test --workspace` green, and:
  1. on the fixture, the split-platform station emits **once**, and the two routes
     serving it share **one corridor** — read from
     `llika-core/src/model.rs:lines_between`, not from the JSON, since the claim
     is about the graph `llk-001` builds. Note it takes station *indices*, so the
     test resolves ids through the network first;
  2. **both halves in one test**, each against a hand-written literal rather than
     prose: that station's `degree` equals a number a person counted from the
     fixture's corridors, *and* the station count equals a second literal that is
     strictly below Phase 1's. The first alone passes on a feed that never had
     split platforms.

     **The "before" is Phase 1's committed literal, not a re-run.** §2.2 says the
     collapse is not an option or a flag, so after this phase nothing can produce a
     pre-collapse import — and adding a flag to make the comparison runnable is
     exactly what §2.2 forbids. The number carries forward from Phase 1's gate 2,
     which is why that assertion writes it down — and so does the **line** count,
     which drops by one too: OQ-5's short route is a valid line before the collapse
     and is dropped after it;
  3. the parentless platform survives as its own station. OQ-5 puts it on a kept
     route deliberately, so a failure here means the rule is missing rather than
     that §2.1's emit rule dropped an unreferenced row;
  4. the consecutive-duplicate fold fires on the fixture's engineered trip, and
     `Network::from_input` accepts the result — the assertion that would fail with
     `RepeatedStation` without the fold;
  5. **`LineTooShort` fires on OQ-5's short route**: it is dropped, the drop is in
     §2.6's report, the rest of the import succeeds, and **the binary's exit status
     is whatever OQ-3 decided** — asserted, because that exit status is the only
     thing OQ-3 still has open and a gate that checks the drop but not the status
     leaves the decision untested. The fixture carries that route for this
     assertion alone, and it is a *different* route from gate 4's — that one has to
     survive;
  6. a collapsed station sits at its **parent's** row position in the `stations`
     array, not its first platform's — §2.5, and it changes the map through
     `llk-001` §2.2's first-claim-wins tie-break;
  7. cross-process byte-stability, still.
- **Close-out:** updates `rules/gtfs-import.md`. Records OQ-3's resolution in §3.

### Phase 3 — the representative trip
*Produces the observable: **yes** — the lines drawn become the lines as operated.
Narrower than Phase 2 and stated as such: on a well-behaved feed the naive choice
is often already right, and what this phase removes is a failure that is severe
when it happens rather than frequent.*

- **Scope:** `trips.rs`, resolving **OQ-1** and implementing it. The decision is
  made against a drawn map — Phase 1 shipped the naive answer precisely so this
  one could be judged rather than argued.
- **Exit gate:** `cargo test --workspace` green, and:
  1. on the fixture's engineered route — whose longest trip is **not** its modal
     one — the line drawn is the modal one, asserted as a full ordered station
     list against a hand-written literal. This is the only assertion that
     discriminates, and the fixture carries the property because OQ-5 fixed it at
     Phase 1;
  2. the fixture's single-trip route still imports, which is the degenerate case
     every selection rule has to survive;
  3. the choice is deterministic on the fixture's **tied pair** — two patterns with
     equal trip counts — and the tie-break is stated, on `llk-001` §2.4's grounds:
     equal-cost candidates are common rather than exotic, and leaving the tie to
     enumeration order makes the output depend on it.

     All three of these read the **shared** fixture, whose properties OQ-5 fixed at
     Phase 1 for exactly this reason. Authoring a second feed here would be the
     cheaper-looking answer and the wrong one only if it displaced the shared
     one — but extending the shared one *now* is what moves every Phase 1 and
     Phase 2 literal, and OQ-5 is what stops that;
  4. cross-process byte-stability, still.
- **Close-out:** updates `rules/gtfs-import.md`. Records OQ-1's resolution in §3.

### Phase 4 — a real city
*Produces the observable: **yes, and for the first time on data nobody in this
repo authored.** Every gate to here runs on a fixture engineered to exercise the
rules; this phase is where the rules meet a feed written by someone who never read
this spec. It is the phase that makes §1's promise true.*

- **Scope:** resolve **OQ-4** — which feed, and committed or downloaded — then
  run the importer on it and fix what breaks. What breaks is not knowable from
  here, which is why this phase exists rather than being folded into Phase 1: the
  `reference` note records that the field list in §2.1 has not been validated
  against a real feed, and this is where it is. Expect malformed rows, absent
  optional columns, ids with characters nobody anticipated, and at least one
  assumption in §2.1 that is wrong.

  **Four hazards are nameable in advance, and naming them is not the same as
  designing for them** — they are here so the phase is sized honestly, and each is
  a correction recorded against §2.1 if it fires:

  - a **UTF-8 BOM** on the first header cell, which the `csv` crate does not strip,
    so `stop_id` fails to deserialize on a feed that is otherwise perfect;
  - zip entries nested under a **top-level directory**, so the four files are not
    at the archive root where §2.1's reading assumes them;
  - `stop_times.txt` at **hundreds of megabytes** for a city feed, which has to be
    streamed and filtered to the kept trips rather than collected — a shape
    decision the fixture is far too small to force;
  - `stop_times.stop_id` being **Conditionally Required**, forbidden on a
    GTFS-Flex row carrying `location_group_id` or `location_id`. §2.1 already
    types it `Option`; what is open is whether such a row is skipped or is an
    error.

  Also the scale reading **OQ-6** asks for: how long `llika` takes on the real
  network, recorded in `llk-001`'s OQ-9, which is the spec that owns the question.
- **Exit gate:** the named feed imports, `Network::from_input` accepts the result,
  and `llika` draws it — reproducible by a second person from the feed and the
  documented invocation alone. Every correction to §2.1 recorded as one. Plus the
  human half, which for this phase is the real test: open the SVG and judge whether
  it reads as a transit poster of that city.
- **Close-out:** updates `rules/gtfs-import.md` and `README.md`. Records the
  layout timing in **`llk-001` §3, OQ-9** — a cross-spec write, named here because
  it is the one fact this spec produces that belongs to another.
