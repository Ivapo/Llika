---
title: layout-search
sources:
  - llika-core/src/layout/hillclimb.rs
  - llika-core/src/layout/candidate.rs
  - llika-core/src/layout/cluster.rs
covers: >
  the hill-climbing sweep and its ordering, the cooling schedule and the convergence
  exit, the candidate set and its tie-break, the three hard move rejections with their
  predicates, and bridge-side clusters and their rigid translation
max_lines: 85
generated: 2026-08-15
---

# Layout search

Two passes per iteration. `llika-core/src/layout/hillclimb.rs:run` sweeps every station
and moves each to whichever nearby free cell lowers `t` (`rules/layout-cost.md`) most,
then `llika-core/src/layout/cluster.rs:pass` offers the same candidate offsets to whole
groups. It mutates the positions in place and keeps the snap's `GridOccupancy` in step
through `llika-core/src/grid.rs:GridOccupancy::relocate`.

**No randomness, and three rules make that a property rather than an intention.**
Stations sweep in input-file order, clusters in corridor order; candidates come in the
grid's spiral order; and the comparison is a strict `<` against an incumbent seeded with
the current position, so nothing moves unless something improves and the **earlier** cell
wins a tie. Equal-cost candidates are common rather than exotic — a symmetric
neighbourhood produces them constantly.

**The schedule.** `llika-core/src/layout/hillclimb.rs:cooling_radius` is
`r_k = max(1, round(r_0 * (1 - k / iterations)))`, from `LayoutParams::initial_radius`
(3) over `LayoutParams::iterations` (200), both `u32` so `f64::from` is total. The clamp
is load-bearing at the far end, where `round(3 * 0.005)` is already 0 by `k = 199`. At
`iterations = 0` the loop body never runs, so the `0/0` is unreachable and the layout is
the snap bit-for-bit — the baseline every gate measurement is taken against.

**Candidates** are `llika-core/src/layout/candidate.rs:spiral_offsets`: rings `1..=r` of
`llika-core/src/grid.rs:ring`, concatenated. The station's own cell is not among them.

**The search stops when an iteration moves nothing in *either* pass**, and `run` returns
how many it executed — the number `SchematicLayout::executed_iterations` carries out to
callers. The exit is output-identical, not merely output-similar: positions
and occupancy are unchanged entering the next iteration, the radius is non-increasing so
its candidate set is a subset of the one just exhausted, and clusters are a function of
the graph rather than of the positions — so by induction every later iteration is a
no-op too. On `sample_network.json` the search converges inside sweep 1 and executes 2
iterations of the 200 asked for. Testing the station sweep alone would stop one pass
short, because a cluster can improve a layout at which no single station can. An
isolated station never moves: with no incident edges every candidate scores identically
and none is strictly lower.

## The three rejections

`llika-core/src/layout/candidate.rs:is_valid_move`, all hard. It borrows the positions
mutably and restores them, because two of the three are properties of the map *after* the
move.

1. **Occupancy** — the target cell is taken.
2. **Order flip** — the station's neighbours, in `c3`'s direction order, must be a *cyclic
   rotation* of what they were. Cyclic, not positional: a fan that rotates whole has torn
   nothing, and the positional reading rejects 4.6× as many moves. It is therefore vacuous
   below degree 3, and constrains junctions only.
3. **Exact overlap** — no edge at the station may come to lie *along* another: collinear,
   sharing more than one point. Tested against every other edge **including
   endpoint-sharing ones**, deliberately unlike `c1`, because the fold-back it exists for
   is invisible to `c1` by construction. Identity is by position in the corridor list, never
   by comparing endpoint pairs, which are unnormalised.

**Ordinary crossings are not rejected** — they are left to `c1`, weighted heaviest. The
predicate is `llika-core/src/geometry.rs:segments_overlap` and **not**
`llika-core/src/geometry.rs:segments_intersect`, which is closed: using the latter forbids
every legitimate collinear straight-through and silently restores a hard-crossing
rejection, freezing the search.

## Clusters

A cluster is the **smaller side of a bridge**, ties to the lower minimum station index,
sides of one station dropped. `llika-core/src/layout/cluster.rs:find` builds them once
before the first iteration and never again — bridges are a property of the graph, not of
the layout — by walking it without each corridor from both endpoints: the corridor is a
bridge exactly when the two walks do not meet, and those reach-sets are the two sides.
Nothing in the file iterates a `HashMap`. On `sample_network.json` that is 7 sides, of
sizes 4, 5, 6, 2, 2, 2 and 3, and **they nest rather than partition**, so one station is
translated by several. `LayoutParams::cluster_moves` (default `true`) switches the pass
off, which reproduces the single-station layout exactly.

The three rejections take group forms in `llika-core/src/layout/cluster.rs:is_valid_move`.
Occupancy is read with the cluster's own cells vacated — a cell held by a member counts as
free, or a one-cell translation is rejected by the cluster's own body and nothing ever
fires. Order flip is evaluated at the bridge's two endpoints only. Overlap is evaluated on
the bridge edge alone, against every other edge: an intra-cluster edge and an external one
can never share an endpoint, so the dropped pairs are exactly `c1`'s.
`llika-core/src/layout/cluster.rs:apply` relocates members in decreasing projection along
the offset, so each lands on a cell its predecessor has just vacated and `relocate`'s
free-target precondition holds.
