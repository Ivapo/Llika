---
id: llk-002
title: gtfs-network-import
note: >
  Turning a published GTFS feed into a Llika input file — stop and route tables
  read, platforms collapsed to stations, one representative trip per route — so
  the schematic map can be drawn from a real city rather than a hand-authored
  fixture.
status: draft
last_updated: 2026-08-15

phases:
  - name: "Phase 1 — thin end-to-end slice: a feed becomes a drawable network"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 2 — station identity: platforms collapse to stations"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 3 — the representative trip"
    reviewed: null
    shipped: null
    cut: null
    by: null
  - name: "Phase 4 — a real city"
    reviewed: null
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
$ llika-gtfs --input bart.zip --output bart.json --route-type 1
wrote bart.json — 50 stations, 6 lines, from 6 of 213 routes, 1 route dropped

$ llika --input bart.json --output bart.svg
wrote bart.svg — 50 stations, 6 lines, grid 3410m, cost 812.4 → 190.7 over 14 iterations
```

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

| file | columns used |
|---|---|
| `stops.txt` | `stop_id`, `stop_name`, `stop_lat`, `stop_lon`, `location_type`, `parent_station` |
| `routes.txt` | `route_id`, `route_short_name`, `route_long_name`, `route_type`, `route_color` |
| `trips.txt` | `route_id`, `trip_id` |
| `stop_times.txt` | `trip_id`, `stop_id`, `stop_sequence` |

The mapping onto `llika-core/src/io.rs:InputSchema` is nearly direct, which is
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
and never the row order or a difference between values.

### 2.2 Station identity is the load-bearing problem (decision, recorded)

A GTFS `stop_id` is usually a **platform**, not a station. A metro stop with two
directions is two `stop_id`s; an interchange served by three lines can be six.
`stops.txt` records the relationship in two columns: a row with
`location_type = 1` is a parent station, and a platform row carries that row's id
in `parent_station`.

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

`--route-type` takes one or more numeric GTFS values. Its default is OQ-2.

### 2.4 The small conversions, each with a trap

- **Colour.** `route_color` is six hex digits **without** a leading `#`, and
  `llika-core/src/model.rs:Line`'s `color` goes straight into an SVG `stroke`
  attribute. So it is prefixed. It is also optional, and frequently absent or
  white; a missing or unusable colour takes a value from a fixed fallback palette,
  assigned in route order so the same feed always gets the same colours.
- **Name.** `route_short_name` and `route_long_name` are each optional and either
  may be empty; a feed where both are empty is legal. Prefer the short name, fall
  back to the long, fall back to `route_id`. `llk-001` never draws the name, so
  this is about the file being readable by a person editing it.
- **Coordinates.** `stop_lat`/`stop_lon` are required and are degrees. `llk-001`
  §2.2 projects them; nothing here transforms them.
- **Ids.** GTFS ids are unique per file by the standard, which discharges
  `llika-core/src/io.rs:InputError::DuplicateStation` and `DuplicateLine` — but
  *collapsing* maps many platform ids onto one station id, so the importer must
  emit each parent station **once**, and that is where a duplicate would be
  introduced if it were emitted per platform.

### 2.5 Crate layout

A third workspace crate, mirroring `llk-001` §2.8's split for its reason — the
algorithm is written once in Rust and serves both the CLI and the later Tauri
app:

- **`llika-gtfs`** — library. Feed reading, collapse, trip selection, and the
  conversion to `InputSchema`. It depends on `llika-core` for the schema types
  and on nothing of `llika-core`'s layout or renderer.
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
case. `serde` and `serde_json` are already in the workspace.

**Input order is the iteration order here too.** `llk-001` §2.2 makes it the rule
everywhere it is observable, and byte-stability across processes rests on it —
`llika-cli/tests/byte_stability.rs` is the standing check. An importer that walks
a `HashMap` of stops to emit the `stations` array produces a different file per
process, and every downstream guarantee `llk-001` proved would evaporate at the
one step upstream of all of them. Stations are emitted in `stops.txt` row order
and routes in `routes.txt` row order.

### 2.6 What the importer reports

Import is lossy by design — routes are filtered out, platforms are merged, and
some routes are dropped (OQ-3). Silence about that is how someone concludes the
tool lost a line. `ImportReport` carries counts: routes seen, routes kept, routes
dropped and why, stops seen, stations emitted. The binary prints the summary line
§1 shows; the library returns the struct, so the later Tauri app can show the same
thing in a panel.

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

- **OQ-2** — **What does `--route-type` default to?** *(design call.)* **Blocks
  Phase 1**, which needs a default to run at all. Metro alone (`1`) is the
  narrowest reading and draws nothing at all for a city whose system is trams;
  metro and tram and light rail (`0,1`) covers most of what reads as a metro map;
  adding rail (`2`) pulls in regional networks that sprawl past the poster form.
  A feed where the filter matches nothing needs a stated answer too — an error, or
  an empty network, which `llk-001` §2.1 does accept as drawable.

- **OQ-3** — **What happens when a real feed trips one of `llk-001`'s five error
  conditions?** *(design call.)* **Blocks Phase 2.** §2.2 settles exactly one of
  them: consecutive duplicates are folded, because that one is an artefact of
  collapsing rather than a defect in the feed. The others are open, and the
  question is whether the importer repairs, drops the route, or fails the whole
  import.

  The argument against failing wholesale is that one malformed route in a
  two-hundred-route feed would then make a city unimportable, and the user cannot
  fix someone else's published data. The argument against silent repair is
  `llk-001` §2.1's, which says in terms that none of the five is a warning and
  none is silently repaired. §2.6's report is the shape of the compromise —
  **drop the route and say so** — but "drop" and "repair" have different answers
  for different conditions and this needs deciding one at a time, not as a policy.

- **OQ-4** — **Which real feed does Phase 4 use, and is it committed?**
  *(needs-input — a licence decision and a download.)* **Blocks Phase 4.** Feeds
  are typically ODbL or CC-BY and often tens of megabytes, so committing one is a
  licensing question and a repository-weight question at once. The alternative is
  a documented download URL and a test that skips when the file is absent, which
  keeps `cargo test` green on a fresh clone at the cost of a gate that does not
  run by default. Recorded rather than guessed because the answer changes what
  Phase 4's gate can assert.

- **OQ-5** — **The fixture feed does not exist and must be authored.**
  *(answerable now.)* **Blocks Phase 1.** No GTFS data is in the tree. Authoring a
  minimal feed by hand is cheap — four small CSVs — and licence-free, and it is
  the direct precedent of `llk-001`'s own OQ-5, whose lesson was that a fixture
  under-constrained at Phase 1 has to be re-authored at Phase 5 with every gate
  literal keyed to it.

  So the properties are fixed **now**, before it is written, and each names the
  phase that cannot be gated without it:

  - **A station whose platforms are split**, with a `location_type = 1` parent and
    at least two platform rows pointing at it, served by **two different routes** —
    without it Phase 2's collapse has nothing to merge and its headline assertion
    is vacuous;
  - **a station with no `parent_station` at all**, so the "a platform without a
    parent is its own station" rule is exercised rather than assumed;
  - **a trip that serves two platforms of one station consecutively**, which is
    the self-loop §2.2 folds — and it must be there, because
    `llika-core/src/io.rs:InputError::RepeatedStation` firing on a real feed is
    the failure the fold exists to prevent;
  - **a route whose longest trip is not its modal one**, which is the only thing
    that can discriminate OQ-1's candidates at Phase 3;
  - **at least one route of a filtered-out `route_type`**, so Phase 1's filter is
    shown to remove something rather than passing vacuously;
  - **non-consecutive `stop_sequence` values** on at least one trip — 10, 20, 30 —
    since a reader that uses row order passes every other assertion here;
  - **a route with no `route_color`**, for §2.4's fallback.

- **OQ-6** — **Scale.** A real metro feed is a few hundred stations, and
  `llk-001`'s OQ-9 measured 72.9 s release at 200 with a cost of
  `O(iterations · V · r² · E²)`. *(deferred by evidence — it is `llk-001`'s open
  item, not this spec's.)* Blocks nothing here, and this spec is what finally makes
  it measurable on something other than a synthetic: the 200-station network that
  measurement came from was a throwaway and is not in the tree. Phase 4 is where a
  real number appears, and it belongs in `llk-001`'s OQ-9 when it does.

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
  `--route-type` filter, resolving **OQ-2**. §2.4's colour, name and id
  conversions. The binary, and §2.6's report.

  **No collapse and no trip selection.** `parent_station` is ignored, so every
  platform is a station; the representative trip is the naive longest, which
  **OQ-1** says is wrong and Phase 3 fixes. Both are deliberate: this phase is
  the plumbing, and shipping the two hard rules alongside it would mean debugging
  three things with one picture to tell them apart.

  **Author the fixture feed** to every property OQ-5 fixes, including the ones
  Phases 2 and 3 need — it does not exist and cannot be copied.
- **Exit gate:** `cargo test --workspace` green, and:
  1. the fixture feed imports, and the file it writes is **accepted by
     `llika-core/src/io.rs:Network::from_input`** — the whole point of the format,
     and the assertion every later phase's map depends on;
  2. hand-counted literals from the fixture, written into the test by a person and
     not recomputed by the code under test: the station count, the line count, and
     one line's full ordered station list;
  3. the `route_type` filter **removes** the fixture's out-of-scope route, and the
     count it reports matches;
  4. a trip with non-consecutive `stop_sequence` reads in the right order — the
     assertion a row-order reader fails and everything else passes;
  5. a route with no `route_color` gets a fallback, and the same feed imported
     twice in **two separate processes** gives byte-identical JSON. Two in-process
     runs would not do, for `llika-cli/tests/byte_stability.rs`'s reason: Rust
     seeds its default hasher per process;
  6. both input forms — the `.zip` and the unpacked directory — produce
     byte-identical output;
  7. `llika` draws the imported file without error.

  Then the human half, which does not carry the gate: open the SVG. It will show
  every interchange twice; that is Phase 2's subject and not a defect here.
- **Close-out:** seeds `rules/gtfs-import.md`. Updates `README.md`, which today
  says input is hand-authored JSON. Commit the crate, the fixture feed, and the
  workspace member.

### Phase 2 — station identity: platforms collapse to stations
*Produces the observable: **yes**, and this is the phase where the imported map
starts to read as a transit diagram rather than a duplicated one. Interchanges
merge, `LineSet` starts recording more than one line per edge, and the
line-bundling renderer `llk-001` Phase 5 shipped finally has something to bundle
on real data.*

- **Scope:** `stations.rs` implementing §2.2 — platforms collapse into their
  `location_type = 1` parent, a platform without a parent is its own station, and
  consecutive duplicates in a line's station list fold to one. Resolve **OQ-3**
  for the remaining four error conditions and implement the answer, with §2.6's
  report gaining the dropped-route counts.
- **Exit gate:** `cargo test --workspace` green, and:
  1. on the fixture, the split-platform station emits **once**, and the two routes
     serving it share **one corridor** — read from
     `llika-core/src/model.rs:lines_between`, not from the JSON, since the claim
     is about the graph `llk-001` builds;
  2. **both halves in one test:** that station's `degree` is what the corridors
     make it, *and* the pre-collapse import gave a different, larger station count.
     The first alone passes on a feed that never had split platforms;
  3. the parentless platform survives as its own station;
  4. the consecutive-duplicate fold fires on the fixture's engineered trip, and
     `Network::from_input` accepts the result — the assertion that would fail with
     `RepeatedStation` without the fold;
  5. each of OQ-3's decided cases, one test each, and the report's counts match;
  6. cross-process byte-stability, still.
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
  2. a route with exactly one trip still imports, which is the degenerate case
     every selection rule has to survive;
  3. the choice is deterministic under a tie — two patterns with equal counts —
     and the tie-break is stated, on `llk-001` §2.4's grounds: equal-cost
     candidates are common rather than exotic, and leaving the tie to enumeration
     order makes the output depend on it;
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
