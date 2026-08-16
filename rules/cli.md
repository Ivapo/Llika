---
title: cli
sources:
  - llika-cli/src/main.rs
covers: >
  the flag surface and the rule that names it, the --params file and how flags
  override it, the validation bounds and why they live here, and the summary line
max_lines: 70
generated: 2026-08-15
---

# CLI

This is `llika`, which draws a network file. The workspace's other binary,
`llika-gtfs`, writes one; its surface is `rules/gtfs-import.md`.

`llika-cli/src/main.rs` is the whole binary — no lib target, so nothing under
`llika-cli/tests/` can name its types and the field-to-flag test lives inside the
file as a `#[cfg(test)]` module. It calls `parse_input`, `Network::from_input`,
`run_layout` and `render_to_string` separately rather than through
`build_schematic_svg`: the same pipeline, but it is what the desktop app will do, so
the CLI is the standing proof the split works.

**One flag per field, and the flag *is* the field name kebab-cased.** Thirteen of
them — nine `LayoutParams`, four `RenderParams` — with no exception and no mapping
table, so `w_crossings` gives `--w-crossings`. The reason is the test: it derives
the expected flag by kebab-casing the serde field name, and one hand-written
exception would force it to consult the same table `main.rs` does, leaving it
asserting only that the code agrees with itself. Every parameter arg is `Option<T>` with **no `default_value`**, so `None` means
absent and `Some(v)` means given; defaults on the args would make `--params` inert.
`--cluster-moves` takes an explicit `true`/`false` rather than being a `--no-` pair:
the field defaults `true`, which clap's `SetTrue` cannot turn off, and a pair would
be two names for one field. `allow_negative_numbers` is on, since the five weights
and `bundle_spacing` accept negatives and clap otherwise reads `-1` as a flag.

**`--params <file>` is one JSON object with two optional keys**, `layout` and
`render`, deserialized into `llika-cli/src/main.rs:ParamsFile` — here rather than in
core because it is a file format, not a library type. All three structs carry
`deny_unknown_fields`, so a misspelled key, or a misspelled *section*, is an error
rather than a file that parses to all-defaults and silently discards the one value
someone was tuning. **The file first, then the flags over it, field by field**; a
field named in neither takes its `Default`. Missing is fine, misspelled is not.

**`llika-cli/src/main.rs:validate` holds every bound**, run once over the merged
pair. Not a set of clap `value_parser`s, which would leave `--params` unchecked; and
not in core, where `run_layout` is infallible by design.

| field | accepted |
|---|---|
| `grid_spacing` | finite, `> 0` — `round(x / g)` is a NaN at zero and casts to cell 0 |
| `initial_radius` | `1 ..= MAX_INITIAL_RADIUS` (64) — **0 rejected**, since `cooling_radius` clamps it to 1 and it would silently mean something else |
| `iterations` | unbounded; 0 is the supported snap-only mode |
| `units_per_cell`, `stroke_width` | finite, `> 0` — a zero scale draws nothing |
| `margin_cells` | finite, `>= 0`; and `> 0` where the network is flat on an axis |
| `bundle_spacing` | finite; 0 is the supported disable seam |

The five weights carry no bound: any real number weights a criterion, zero switches
it off. `--initial-radius` is inert at the shipped weights and its `--help` says so
— `c2` prices every ring-2-or-further move out of contention, so 1, 2, 3, 5 and 8
give bit-identical positions while costing `O(r²)` candidates each.

`llika-cli/src/main.rs:Extent` supplies the conditional half of the `margin_cells`
row, measured on the **projected plane** so a network straddling a pole is judged on
what gets drawn. It is the *network's* extent and not the laid-out grid's, leaving
one case uncaught: a network with real extent can still snap into a single cell at a
coarse `--grid-spacing`, where a zero margin gives a zero-extent document. Closing
that needs the extent read back from a layout, which cannot run until `grid_spacing`
and `initial_radius` are checked — so it would split validation in two and lose the
one property the single function exists for.

**The summary line** reports station and line counts, `grid_spacing()`, the cost
pair and `executed_iterations()` — the last three read off `SchematicLayout`
(`rules/projection-grid.md`) and never off `LayoutParams`, each being a function of
the parameters *and* the network. The "before" cost is the same run at
`iterations: 0`, and the sweep count is the **executed** one: on
`sample_network.json`, 2 against the 200 asked for.

Nothing is written until every check has passed, so a rejected run leaves no
half-written output, and output is byte-stable across processes by either route.
