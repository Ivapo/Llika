---
title: rendering
sources:
  - llika-core/src/render/mod.rs
  - llika-core/src/render/corridor.rs
  - llika-core/src/lib.rs
covers: >
  RenderParams defaults, the viewport transform and its y-flip, the document
  envelope, what elements the SVG holds, line bundling and its mitre, and the
  convenience entry point
max_lines: 70
generated: 2026-08-15
---

# Rendering

`llika-core/src/render/mod.rs:RenderParams` defaults to `units_per_cell` 40,
`margin_cells` 2, `stroke_width` 6, `bundle_spacing` `None`. The margin defaults
**above zero** because a one-station network has `i_max == i_min`, so the envelope
reduces to `2 * margin_cells * units_per_cell` and a zero margin gives a zero-extent
document. It deserializes with `#[serde(default, deny_unknown_fields)]` — an omitted
field takes its `Default`, an unrecognised one is an error — and `rules/cli.md`
carries the argument and every bound.

`llika-core/src/render/mod.rs:Viewport` owns the third coordinate system:

```
svg_x = (i - i_min + margin_cells) * units_per_cell
svg_y = (j_max - j + margin_cells) * units_per_cell      // flipped
```

The flip is load-bearing — latitude increases north, SVG `y` increases down — and
omitting it draws the map upside down while passing every count-based check. Tests
read positions through `Viewport::project` rather than re-deriving the transform.
An empty layout takes bounds of all zeros, leaving a document that is margin alone.

`llika-core/src/render/mod.rs:render_to_string` emits, in order: a white background
`<rect>` over the viewBox, one `<path>` per line in input order, and one `<circle>`
per station in input order. Each path spans a line's whole station list — one `M`
then `L`s — so corner rounding comes free from `stroke-linejoin="round"`.
Coordinates are formatted to three decimals.

`llika-core/src/render/corridor.rs:Bundling` draws lines sharing a corridor as
parallel offset strokes, reading the `LineSet` already on the graph edge rather than
re-deriving shared pairs from the station lists. A **collapse station** — the line
set changes, or degree `!= 2` — takes offset zero, so bundles merge to a point at
real interchanges. `!= 2` and not `> 2`: a terminus shared by two lines is degree 1.
A **run** is a maximal path of corridors carrying one line set, broken at every
collapse station; within it the line at index `k` of `n` takes signed offset
`(k - (n-1)/2) * s`, straddling the centreline, with `n = 1` falling out as zero. The
sort giving `k` is by line **`id`**, the string — not input order, which is the rule
everywhere else — which is why a duplicate line id is rejected.

The offset side belongs to the **run**, directed from its lower-indexed endpoint, so
two lines walking a shared corridor in opposite list order cannot land on the same
side; a run with no endpoints, or two that are the same station, runs from its
lowest-indexed station towards its lower-indexed run-neighbour. Offsets are keyed by
(line, position-in-list) and applied **after** `Viewport::project`, in user units.
Markers do not move.

At a bend, `llika-core/src/render/corridor.rs:mitre` points the offset along
`normalize(n1 + n2)` over the two corridors' left normals, scaled by `1/cos(θ/2)`
computed as `2/|n1 + n2|`. `llika-core/src/render/corridor.rs:MITRE_SCALE_CLAMP`
bounds that at 4 and subsumes the anti-parallel case, where the direction is
undefined and the factor diverges — octilinearity is not an invariant, and the
viewport is sized from station extents, so an unclamped mitre escapes the viewBox
unseen. `bundle_spacing` of `Some(0.0)` disables bundling and reproduces the
pre-bundling picture byte-for-byte. A corridor with a collapse station at **both**
ends has zero offset at each, so its lines overprint — on the sample fixture,
`central`–`market`. An accepted v1 limit.

Output is byte-stable across processes: the walk order is input order and the `svg`
crate sorts element attributes by name before writing.

`llika-core/src/lib.rs:build_schematic_svg` is the convenience entry point —
validate, lay out, render. The three steps stay public in their own right so a
caller can parse once and re-render on parameter changes; `llika-cli/src/main.rs`
takes that path rather than the convenience one.
