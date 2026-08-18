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
wrote gallery/bart.svg — 50 stations, 6 lines, grid 3322m, cost 234.460102 → 16.746281 over 6 iterations
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

What it also shows is what a second network was for. This paragraph used to say
the map fell short of a poster — a handful of edges off-angle, a few corridors
kinked where a draughtsperson would have run them straight — and pointed at
`llk-001`'s OQ-2, which said the five criterion weights were provisional and that
a network of a different shape was what would settle them. This is that network,
and it settled the negative half: the weights it was drawn under were dominated,
by a mechanism rather than by taste, and `llk-001` Phase 7 replaced them. **Every
one of BART's 50 corridors now lands on a multiple of 45 degrees**, at no
crossings, and the Red line's diagonal cut across the trunk and its doubling-back
inside the western bundle are both gone.

Three of the 50 run on the diagonal and the other 47 on an axis, which is what
keeps it from reading as a circuit board — the all-axis layouts a heavier `w5`
produces do. That balance is not tuned for: the weights are a round point chosen
inside the region that beat the old ones, not the best cell of the grid that found
it. OQ-2 stays open on its positive half — *which* replacement is right still
rests on this one city, and a third network is what would settle that.
