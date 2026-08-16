# Gallery

Maps kept for reference — to look at, to compare against after a change, and to
show what the generator currently produces. **Nothing checks them.** They are not
a gate and no test reads them, so a file here can go stale; the commands below
are what refresh it, and each prints the summary line the map was drawn from.

| file | what it is | drawn from |
|---|---|---|
| `sample-network.svg` | 17 stations, 3 lines — the hand-authored fixture `llk-001` was built against | `llika-core/tests/fixtures/sample_network.json` |
| `gtfs-fixture.svg` | 11 stations, 5 lines — the GTFS fixture feed, imported and drawn | `llika-gtfs/tests/fixtures/feed/` |
| `bart.svg` | 50 stations, 6 lines — **a real city**: BART's published feed, imported and drawn | `llika-gtfs/tests/fixtures/bart.zip`, `--route-types 1` |

All three at default layout and render parameters; the one flag above is on the
import, not the drawing. A map drawn with layout flags is worth keeping too —
name the file for what the flags were exploring, and say so in the table.

## Refreshing them

```console
$ cargo run -p llika-cli -- \
    --input llika-core/tests/fixtures/sample_network.json \
    --output gallery/sample-network.svg

$ cargo run -p llika-gtfs -- --input llika-gtfs/tests/fixtures/feed --output /tmp/gtfs-fixture.json
$ cargo run -p llika-cli -- --input /tmp/gtfs-fixture.json --output gallery/gtfs-fixture.svg

$ cargo run -p llika-gtfs -- \
    --input llika-gtfs/tests/fixtures/bart.zip --output /tmp/bart.json --route-types 1
wrote /tmp/bart.json — 50 stations, 6 lines, 12 of 14 routes matched, 0 dropped, 6 merged
$ cargo run -p llika-cli -- --input /tmp/bart.json --output gallery/bart.svg
wrote gallery/bart.svg — 50 stations, 6 lines, grid 3322m, cost 511.450746 → 112.087766 over 3 iterations
```

The GTFS one is two commands because the import writes a file rather than handing
a network to the layout — that intermediate file is where a person edits what the
feed said, and the README at the repo root argues why.

## What to expect from them

The fixture feeds are engineered to exercise rules, not to look like anywhere, so
those two read as *very* simple posters — the shapes are right and there is not
much of them. **`bart.svg` is the one to judge the output on**, and what it shows
is a real system recognisable as itself: Antioch on a long yellow spur, the
Richmond–Millbrae corridor bundled down the left, the airport and Berryessa
branches, the two-station grey stub to Oakland airport.

What it also shows is where the layout still falls short of a poster. Not every
stroke lands on a multiple of 45 degrees — the search is a hill-climb that stops
when a sweep buys nothing, and on 50 stations it stops with a handful of edges
off-angle and a few corridors kinked where a draughtsperson would have run them
straight. `llk-001`'s OQ-2 says the five criterion weights are provisional and
that a second network with a different shape is what would settle them. This is
that network.
