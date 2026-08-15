---
title: layout-search
sources:
  - llika-core/src/layout/hillclimb.rs
  - llika-core/src/layout/candidate.rs
covers: >
  the hill-climbing sweep and its ordering, the cooling schedule, the candidate
  set and its tie-break, and the three hard move rejections with their predicates
max_lines: 55
generated: 2026-08-15
---

# Layout search

Single-station hill-climbing. `llika-core/src/layout/hillclimb.rs:run` sweeps every
station and moves each to whichever nearby free cell lowers `t` (`rules/layout-cost.md`)
most. It mutates the positions in place and keeps the snap's `GridOccupancy` in step
through `llika-core/src/grid.rs:GridOccupancy::relocate`. Cluster moves do not exist yet.

**No randomness, and three rules make that a property rather than an intention.**
Stations sweep in input-file order; candidates come in the grid's spiral order; and the
comparison is a strict `<` against an incumbent seeded with the station's own cell, so a
station stays put unless something improves and the **earlier** cell wins a tie. Equal-cost
candidates are common rather than exotic — a symmetric neighbourhood produces them
constantly.

**The schedule.** `llika-core/src/layout/hillclimb.rs:cooling_radius` is
`r_k = max(1, round(r_0 * (1 - k / iterations)))`, from `LayoutParams::initial_radius`
(3) over `LayoutParams::iterations` (200), both `u32` so `f64::from` is total. The clamp
is load-bearing at the far end, where `round(3 * 0.005)` is already 0 by `k = 199`. At
`iterations = 0` the loop body never runs, so the `0/0` is unreachable and the layout is
the snap bit-for-bit — the baseline every gate measurement is taken against.

**Candidates** are `llika-core/src/layout/candidate.rs:spiral_offsets`: rings `1..=r` of
`llika-core/src/grid.rs:ring`, concatenated. The station's own cell is not among them.

**Nothing detects convergence and nothing exits early.** The count is a bound, not a
target. On `sample_network.json` the search reaches its fixed point inside sweep 1 and the
other 199 are no-ops, which is correct behaviour. An isolated station never moves: with no
incident edges every candidate scores identically and none is strictly lower.

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
