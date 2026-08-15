---
title: rendering
sources:
  - metro-core/src/render/mod.rs
  - metro-core/src/lib.rs
covers: >
  RenderParams defaults, the viewport transform and its y-flip, the document
  envelope, what elements the SVG holds, and the convenience entry point
max_lines: 40
generated: 2026-08-14
---

# Rendering

`metro-core/src/render/mod.rs:RenderParams` defaults to `units_per_cell` 40,
`margin_cells` 2, `stroke_width` 6. The margin defaults **above zero** because a
one-station network has `i_max == i_min`, so the envelope reduces to
`2 * margin_cells * units_per_cell` and a zero margin gives a zero-extent document.

`metro-core/src/render/mod.rs:Viewport` owns the third coordinate system:

```
svg_x = (i - i_min + margin_cells) * units_per_cell
svg_y = (j_max - j + margin_cells) * units_per_cell      // flipped
```

The flip is load-bearing — latitude increases north, SVG `y` increases down — and
omitting it draws the map upside down while passing every count-based check. Tests
read positions through `Viewport::project` rather than re-deriving the transform.
An empty layout takes bounds of all zeros, leaving a document that is margin alone.

`metro-core/src/render/mod.rs:render_to_string` emits, in order: a white background
`<rect>` over the viewBox, one `<path>` per line in input order, and one `<circle>`
per station in input order. Each path spans a line's whole station list — one `M`
then `L`s — so corner rounding comes free from `stroke-linejoin="round"`.
Coordinates are formatted to three decimals.

**There is no line-bundling.** Two lines sharing a corridor draw over each other,
the later path on top. Parallel offsets are a later phase.

Output is byte-stable across processes: the walk order is input order and the `svg`
crate sorts element attributes by name before writing.

`metro-core/src/lib.rs:build_schematic_svg` is the convenience entry point —
validate, lay out, render. The three steps stay public in their own right so a
caller can parse once and re-render on parameter changes; `metro-cli/src/main.rs`
takes that path rather than the convenience one.
