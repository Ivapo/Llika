# Gallery

Maps kept for reference — to look at, to compare against after a change, and to
show what the generator currently produces. **Nothing checks them.** They are not
a gate and no test reads them, so a file here can go stale; the commands below
are what refresh it, and each prints the summary line the map was drawn from.

| file | what it is | drawn from |
|---|---|---|
| `sample-network.svg` | 17 stations, 3 lines — the hand-authored fixture `llk-001` was built against | `llika-core/tests/fixtures/sample_network.json` |
| `gtfs-fixture.svg` | 11 stations, 5 lines — the GTFS fixture feed, imported and drawn | `llika-gtfs/tests/fixtures/feed/` |

Both at default parameters. A map drawn with flags is worth keeping too — name
the file for what the flags were exploring, and say so in the table.

## Refreshing them

```console
$ cargo run -p llika-cli -- \
    --input llika-core/tests/fixtures/sample_network.json \
    --output gallery/sample-network.svg

$ cargo run -p llika-gtfs -- --input llika-gtfs/tests/fixtures/feed --output /tmp/gtfs-fixture.json
$ cargo run -p llika-cli -- --input /tmp/gtfs-fixture.json --output gallery/gtfs-fixture.svg
```

The GTFS one is two commands because the import writes a file rather than handing
a network to the layout — that intermediate file is where a person edits what the
feed said, and the README at the repo root argues why.

## What to expect from them

The fixture feeds are engineered to exercise rules, not to look like anywhere, so
these read as *very* simple posters — the shapes are right and there is not much
of them. `import_gtfs_spec.md`'s Phase 4 is the first map drawn from a city
somebody actually rides, and it is the one to judge the output on.
