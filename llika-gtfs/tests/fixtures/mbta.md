# `golden/mbta.json` — a real network, derived and committed

The rapid-transit network of the Massachusetts Bay Transportation Authority, as
**this importer read MassDOT's published GTFS feed on the date below**. It is
`llk-002` Phase 6's product: the second real city in this repository, and the
network `llk-001`'s OQ-2 was measured against after three requests for exactly
that measurement.

**The feed itself is not here, and that is the decision rather than an
omission.** OQ-8 settles it: the feed which earns a place in the tree is the one
that serves as the *example*, and `bart.zip` already does — it is `README.md`'s
worked example, `gallery/bart.svg`'s provenance and the fixture eight tests read.
A second archive would do only the third of those, at 18.6 MB re-committed on
every refresh. So what lands is `golden/mbta.json`, tens of kilobytes of this
project's own output, and this file records where it came from.

| | |
|---|---|
| source | <https://cdn.mbta.com/MBTA_GTFS.zip> |
| retrieved | 2026-08-18 |
| published | `last-modified: 2026-08-17`, `feed_version` "Summer 2026, 2026-08-17T19:35:03+00:00, version D" |
| valid | **20260810 → 20260905**, from `feed_info.txt` |
| publisher | MBTA (`feed_info.txt`, `feed_id` `mbta-ma-us`) |
| feed size | 18,590,242 bytes |
| feed sha256 | `300b21b4f49ce9dfae488dcb06a66d379b4a37902c3fb293950c9d4eb72c51bb` |
| licence | MassDOT / MBTA Developers License Agreement — [PDF](https://cdn.mbta.com/sites/default/files/2023-08/mbta-massdot-develop-license-agreement.pdf), linked from <https://www.mbta.com/developers/gtfs> |
| imports to | 119 stations, 8 lines, 8 of 400 routes matched, 0 dropped, 0 merged |

Read at `ImportParams::default()` and no flag: MBTA's eight rapid-transit routes
are `route_type` 0 and 1, so the default selects them without being told, and the
369 bus, 14 commuter-rail and 9 ferry routes are filtered out.

## The licence, and what it asks of this repository

§3.1 grants "non-exclusive, limited, and revocable rights to **use, reproduce,
and redistribute the Data**" — the same sentence shape BART's agreement carries,
and the reason this feed was chosen over Chicago's. It has no purpose
limitation, no "may not transfer outside your application", and no
delete-on-termination clause, all three of which CTA's has and none of which a
git history can honour. Its obligations bear on this tree in two places:

- **§4.1 asks for acknowledgement and forbids MassDOT's and the MBTA's logos and
  trademarks.** So this file credits **MassDOT and the MBTA** as the source of
  the data, and no MBTA mark appears anywhere in this repository. The map in
  `gallery/mbta.svg` is one this project *generates* from the Data; it is not the
  MBTA's own system map and is not claimed to be.
- **§5.4 keeps title with MassDOT.** The repository's MIT licence covers this
  project's own code and never extended to the Data or to what is derived from
  it.

**A committed network is a derived database, and that is the reading this file
has to make rather than dodge.** `golden/mbta.json` is this project's output —
stations, ordered line lists and colours, produced by `llika-gtfs` — *and* it is
a database derived from someone else's. §3.1's grant reaches it: it permits
redistribution of the Data and carries no share-alike term that would reach a
derivative, which is the clause OQ-8 warned could come out stricter than
committing the archive whole. It did not.

**So `bart.md`'s "It is the only third-party data in this repository" still
stands, and is deliberately left alone.** No second archive lands, so that
sentence is true of *feeds*, which is what it is about — the bytes a publisher
published, held unmodified as evidence of what they publish. This file is the
other kind of thing: not a snapshot but an output, and evidence only of what this
importer produces. The distinction is written here so a reader does not have to
reconcile the two on their own.

## Refreshing it

**The feed moves fast** — its declared window is 27 days, against BART's ~230 —
and `https://cdn.mbta.com/MBTA_GTFS.zip` always serves the current build. So a
second person fetching later gets a *different feed*, and the gates keyed to this
one fail. That is deliberate: **absent skips, stale fails**. A missing archive is
a fresh clone and the tests say `ignored` with the URL; a changed archive is a
fact about the world that a gate must not paper over.

Refreshing is one deliberate act that re-measures everything keyed to the feed:

```console
$ curl -L -o llika-gtfs/tests/fixtures/mbta.zip https://cdn.mbta.com/MBTA_GTFS.zip
$ shasum -a 256 llika-gtfs/tests/fixtures/mbta.zip
$ cargo run -p llika-gtfs -- --input llika-gtfs/tests/fixtures/mbta.zip \
    --output llika-gtfs/tests/fixtures/golden/mbta.json
$ cargo run -p llika-cli -- --input llika-gtfs/tests/fixtures/golden/mbta.json \
    --output gallery/mbta.svg
```

Update every row of the table above in the same commit, and say in the message
which gate literals moved with it. They are:

- `tests/mbta_feed.rs` — the station, line, route and stop counts.
- `tests/golden.rs` — `golden/mbta.json` itself, byte for byte.
- `tests/mbta_feed.rs:mbta_draws_to_the_measured_criteria_vectors` — **the one
  that is not a property of the feed at all.** It pins the five cost criteria
  this network *lays out* to, at four weightings, which is `llk-001` OQ-2's
  fourth measurement. A refreshed MBTA can fail it with nothing wrong in the
  code, and re-measuring it is part of the refresh rather than a bug to chase.

## What this network is, and the two edits it invites

119 stations, 120 corridors, one connected component — **cyclomatic number 2**,
against BART's 1. It is also the first network in this repository that a
weighting can be seen to *decide* something about: it draws one crossing at the
shipped weights and none at `--w-crossings 100`.

Two things in it are `llk-002` §1.1's case for the intermediate file rather than
defects in the import:

- **All four Green Line branches are separate `route_id`s sharing `route_color`
  `#00843D`**, so they draw as four indistinguishable green lines.
- **`Red` comes out at 17 stations**, one of its two southern branches, because a
  line is a path and OQ-1's representative trip picks the modal pattern.

Both are exactly the edits `golden/mbta.json` being an ordinary, hand-editable
input file exists for, and neither is a rule this project would change to avoid.
