---
title: projection-grid
sources:
  - metro-core/src/geometry.rs
  - metro-core/src/projection.rs
  - metro-core/src/grid.rs
  - metro-core/src/layout/mod.rs
covers: >
  the equirectangular projection, the derived grid spacing and its fallback, cell
  rounding, the claim-order and spiral tie-break, and what run_layout produces
max_lines: 55
generated: 2026-08-14
---

# Projection and grid

Three coordinate systems. This file owns the first two; `rules/rendering.md` owns
SVG user space.

**1. Projected plane, in metres.** `metro-core/src/projection.rs:Projector` centres
on the arithmetic mean of the station latitudes and longitudes, summed in input
order. `metro-core/src/projection.rs:project` is
`x = (lon - lon_c) * 111_320 * cos(lat_c)`, `y = (lat - lat_c) * 111_320`, with `y`
increasing **north**. `metro-core/src/geometry.rs:Point2` is the point type and
carries `distance`; segment intersection and angle math do not exist yet — they
arrive with the cost criteria.

An empty station list takes centroid `(0, 0)` rather than the `NaN` the mean would
give.

**2. The integer grid.** `metro-core/src/grid.rs:GridPoint` has `i`, `j`, with `j`
increasing north like the plane. `metro-core/src/grid.rs:raw_cell` rounds
`(x/g, y/g)`; it is public because the difference between the raw cell and the
final position is the only observable proof that a collision occurred.

`g` comes from `metro_core::layout::LayoutParams::grid_spacing`, an
`Option<f64>` in metres. `Some(m)` is used as given. `None`, the default, calls
`metro-core/src/grid.rs:derive_grid_spacing`: the lower-middle median
(`metro-core/src/grid.rs:median_lower`) of the **non-zero** projected edge lengths.
Where that set is empty — no edges at all, or every edge degenerate — `g` is
`metro-core/src/grid.rs:FALLBACK_GRID_SPACING_M`, 500 m. A median of the empty set
is undefined and a zero median makes every cell a `NaN` that casts to 0.

**One station per cell.** `metro-core/src/grid.rs:GridOccupancy` claims cells in
input-file order; first claim wins. A station whose cell is taken spirals out
through `metro-core/src/grid.rs:ring`: increasing Chebyshev ring, and within a ring
increasing angle from due east. No two cells of a ring share a ray, so that is a
total order. `metro-core/src/grid.rs:snap_to_grid` is the whole pass.

Because of this, every **post-snap** edge is at least one cell long and degeneracy
is confined to the pre-snap plane.

**`run_layout`.** `metro-core/src/layout/mod.rs:run_layout` projects, derives `g`,
snaps, and is infallible. `metro-core/src/layout/mod.rs:SchematicLayout` holds
`positions`, `projected` and `grid_spacing`, all indexed by station index. Read `g`
back from **there**, never from `LayoutParams`: the default is a function of the
parameters *and* the network. There is no iteration loop yet.
