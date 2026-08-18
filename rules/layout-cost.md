---
title: layout-cost
sources:
  - llika-core/src/geometry.rs
  - llika-core/src/layout/cost.rs
  - llika-core/src/layout/mod.rs
covers: >
  the plane and integer-grid geometry helpers, the five cost criteria and their
  zero-sets, the c2 target length, and the weights that combine them into t
max_lines: 60
generated: 2026-08-14
---

# Layout cost

Five criteria, lower is better, combined by `llika-core/src/layout/cost.rs:Cost::total`
as `t = w1*c1 + w2*c2 + w3*c3 + w4*c4 + w5*c5`. Each is separately public because the
weights become UI sliders, so each must be independently computable.
`llika-core/src/layout/cost.rs:evaluate` scores all five;
`llika-core/src/layout/cost.rs:total_cost` returns `t` alone — what the search minimises
(`rules/layout-search.md`), once per candidate move.

**The domain is the integer grid.** Every criterion takes `&[GridPoint]` indexed by
station index, never a `SchematicLayout` and never the metre plane: the search moves
stations between cells and scores candidate position sets no layout owns.

| | symbol | is | zero when |
|---|---|---|---|
| `c1` | `c1_crossings` | count of edge pairs sharing no endpoint whose **closed** segments meet | none do |
| `c2` | `c2_edge_length` | `Σ (\|e\|/L − 1)²` | every edge is `L` cells |
| `c3` | `c3_angular_resolution` | `Σ_v Σ_k \|θ_k − 2π/deg(v)\|` over degree ≥ 2 | edges are evenly spread |
| `c4` | `c4_straightness` | `Σ (π − φ)` over each line's interior degree-2 stations | no line bends |
| `c5` | `c5_octilinearity` | `Σ` deviation from the nearest 45° | every edge is octilinear |

`c1` is closed-segment, so a touching endpoint and a collinear overlap both count, and it
**excludes endpoint-sharing pairs** — which the search's overlap rejection deliberately
does not (`rules/layout-search.md`). `c3` orders by direction, ties broken by neighbour
index, inside `llika-core/src/layout/cost.rs:incident_directions`, which sorts so that
ordering exists once: the search is pinned to it. `c4` sums per line, so two lines over
one corridor each pay their own bend, and `φ` is the interior angle — never the turn.

**`c3` has a positive floor at odd degrees.** An even spread needs `8/d` integral, so
it is reachable at degree 1, 2, 4 and 8 only. Degree 3's best is 135°/135°/90°,
costing `FRAC_PI_3` — **not** `PI / 3.0`, one ulp away and failing an equality test.

**The target `L`** is `llika-core/src/layout/mod.rs:target_edge_cells`: `max(1, m/g)`, `m`
the median non-zero projected edge length `g` itself derives from, so exactly `1.0` under
the default `g`. Read it off `SchematicLayout`, for `rules/projection-grid.md`'s reason.

## Geometry

`llika-core/src/geometry.rs:Point2` is the metre plane and is not scored. Everything else
works on `GridPoint`, which is what makes the criteria exact:
`llika-core/src/geometry.rs:segments_intersect` is sign tests on `i128` cross products
(`llika-core/src/geometry.rs:orientation`), so no epsilon exists to tune.

`llika-core/src/geometry.rs:direction` normalises into `[0, 2π)`. **Load-bearing**:
Rust's `%` keeps the dividend's sign, so a raw `atan2` would make
`llika-core/src/geometry.rs:octilinear_deviation` return a *negative* cost for any edge
pointing south. Normalised, all eight unit offsets give exactly `+0.0` — assert by
equality, never `to_bits`. `llika-core/src/geometry.rs:interior_angle` is exactly `π`
for collinear opposite offsets at any angle, octilinear or not.

## The weights

`llika-core/src/layout/mod.rs:LayoutParams` carries `w_crossings` 5.0,
`w_edge_length` 1.0, `w_angular_resolution` 1.0, `w_straightness` 2.0,
`w_octilinearity` 5.0 — named for what they weigh, since the names are serde-visible
and `rules/cli.md` derives a flag from each by kebab-casing it. `Default` is
**written out, never derived**: a derived one zeroes every weight, making `t ≡ 0` and
every cost-decrease gate vacuous. The values are provisional (OQ-2) and **dominated on
BART** — 176 of 324 grid settings beat all five at once; at `w5:w4` = 5:2, `c4` outbids `c5`.
