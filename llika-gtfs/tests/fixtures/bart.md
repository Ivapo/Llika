# `bart.zip` — a real feed, redistributed

The published GTFS feed of the San Francisco Bay Area Rapid Transit District,
committed as a **dated snapshot** so the phase that meets real data has a gate
that runs on `cargo test` from a fresh clone. It is the only third-party data in
this repository. `feed/` beside it is the hand-authored fixture and is a
different kind of thing entirely — engineered to exercise rules, and the one
every other gate reads.

| | |
|---|---|
| source | <https://www.bart.gov/dev/schedules/google_transit.zip> |
| retrieved | 2026-08-16 |
| published | `last-modified: 2026-08-04`, `feed_version` 72, valid 2026-01-12 → 2026-08-30 |
| publisher | Bay Area Rapid Transit (`feed_info.txt`) |
| size | 892,312 bytes |
| sha256 | `affdc4d70cac01f71e54f049c754ba36824a885edcdef2ef8b024820c9e93080` |
| licence | BART Developer License Agreement — <https://www.bart.gov/schedules/developers/developer-license-agreement> |

**The bytes are BART's, unmodified.** Not repacked, not filtered to the four
tables this crate reads, not stripped of the 1.5 MB `shapes.txt` §1.2 never
opens. A snapshot that has been edited is no longer evidence of what a publisher
publishes, which is the whole reason to hold one.

## The licence, and what it asks of this repository

The agreement grants "non-exclusive, limited and revocable rights to use,
reproduce, and redistribute BART Data", which is what makes committing it legal
rather than merely convenient. Three of its terms bear on this tree:

- **No BART trademarks or copyrighted materials in association with the Data.**
  So no BART logo anywhere near it, and BART's official system map is theirs —
  a map this project *generates* from the Data is not that map, and is not
  claimed to be.
- **The grant is revocable and the Data is "as is".** Hence the snapshot rather
  than a claim about what BART currently publishes: this file is what was served
  on the date above, and nothing here asserts it is still current or correct.
- **BART maintains title.** The repository's MIT licence covers this project's
  own code and never extended to this file.

## Refreshing it

The feed moves — BART rebuilds it as schedules change, and this copy expires
2026-08-30. Refreshing is a deliberate act, not a chore to automate: every
literal a gate hand-counts from it moves too, which is the same trap
`import_gtfs_spec.md`'s OQ-5 fixes the synthetic fixture's properties to avoid.

```console
$ curl -L -o llika-gtfs/tests/fixtures/bart.zip https://www.bart.gov/dev/schedules/google_transit.zip
$ shasum -a 256 llika-gtfs/tests/fixtures/bart.zip
```

Update every row of the table above in the same commit, and say in the message
which gate literals moved with it.

One of those literals is not a property of the feed at all.
`tests/real_feed.rs:bart_draws_to_the_shipped_criteria_vector` pins the five cost
criteria this network **lays out** to, which `llk-001` Phase 7 chose its default
weights against — so a refreshed BART can fail it with nothing wrong in the code,
and re-measuring it is part of the refresh rather than a bug to chase.
