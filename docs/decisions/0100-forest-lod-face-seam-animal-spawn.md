# 0100 – Forest in the far chunks, trees over the face seams, animals clear of trees

- **Status:** accepted (built; waits for the developer before merge: engine code)
- **Date:** 2026-11-13

## Context
Three engine bugs from one round of play and the soak:

1. **The soak** (`technoape-server soak=1 seed=79085 planet=256 warp=32 only=14 ticks=13`) found an animal inside
   the ground at tick 12. `herds::stand` picks the air over a column's ground, but an animal's box (the
   player's, 0.6 m wide) spans neighbouring columns, where a trunk or a low crown of ADR 0092's trees can be.
2. **F9 2026-11-13_14-33-44**, "there is a forest here but lod shows a barren land": from 400 m over a cold
   forest the far chunks were bare snow. Plants are generated for level-0 tiles only (`build_tile`), so every
   coarser level had the ground and nothing on it.
3. **F9 2026-11-13_14-48-48**, "the seam here is very obvious": 13 columns from a cube face's edge in a
   temperate forest. `Body::candidate` refused any plant within `REACH` (6) columns of a face edge, so every
   seam of the planet was a straight bare strip about twelve columns wide through every forest (seen from
   60 m up: `out/seam/s2_report_top_before.png`). Clutter kept the same margin. Terrain height, biome and
   lighting were continuous across the seam; only plants were cut.

## Options considered
- **Seam:** (a) keep each tree on its face and clip crowns at the edge: still a visible line of half trees.
  (b) **chosen:** feet anywhere inside the face; a column within `REACH` of an edge also reads the next
  faces' spots, located in their own grid by `unfold` (the inverse of `fold`, the existing integer map over a
  seam). A crown over the seam is then the same blocks on both sides, by construction.
- **Far forest:** (a) mesh real plant blocks into coarse chunks: a coarse cell is 2-32 m, a block-exact crown
  cannot be drawn there. (b) a separate canopy mesh or impostors: new renderer work. (c) **chosen:** in a
  coarse tile the canopy *is* the ground: where the real trees stand over the coarse column, its height is
  the top of the highest of them and its top, band and rock are that tree's top block (its leaves). The
  existing mesher, skirts and level selection need nothing new; the work stays in `build_tile` on the
  workers.

## Decision
- **Animals** (`herds.rs`): `stand_clear` = `stand` whose whole box is clear (`phys::blocked`). Herd
  generation and drift use it and try their next spot otherwise, as they did over water. `stand` itself is
  unchanged: `move_on`'s climb-out for an animal a block was placed on still uses it. `/spawn <x> live` still
  uses `stand` (the player picks that spot).
- **Seams** (`world.rs`): `candidate` takes any foot inside its face; its river check reads across a seam
  through `fold`. `plants_with(face, i, j, cand)` reads this face's spots and, for a column near an edge, each
  adjacent face's in that face's grid; faces are read in index order, so where two plants meet both sides of
  a seam pick the same one. Round a cube corner (both coordinates past an edge) `unfold` does not reach, so a
  crown there is cut at the corner's diagonal; there are eight corners. Clutter (`clutter::ground_under`) lies
  up to the edge.
- **Far forest** (`world.rs`, `build_tile` for level > 0): the trees (plants with a trunk) at the coarse
  column's middle (level 1) or its four quarter points (level 2 and up) are read with the same `plants_with`
  and spot cache as level 0; the highest top, if over the coarse ground, becomes the column's height and its
  block the column's three materials (rock none). It is the real tree at those points, so it agrees with the
  level-0 forest; a forest's top stays broken where those points fall in gaps between crowns. Taking the
  highest of four keeps a 16 m cell over a forest green (one middle sample missed the crowns in 57 % of
  forest columns at level 4).

## Evidence
- Tests, each seen failing first: `herds::tests::herds_stand_clear_of_trees` (animals 14, 18, 31 made inside
  a block), `world::baked_tests::trees_grow_over_face_seams` (0 trees within 6 columns of an edge against 375
  in the next 6; then crown blocks found over the seam at a lower share than on their own face, fixed by
  reading faces in a fixed order), `world::baked_tests::far_tiles_carry_the_canopy` (0 of 564 forest columns
  at a canopy at level 1). The soak repro passes; a 2 minute soak of 50 bots (seed 1) passes.
- Stills (`out/seam/`, before = main's play build): the report's view and from 60 m over it, two other seams
  (faces 0/2 and 0/5) from 150 m and along and across at 40 m: `seam_compare.png`. The forest report's view
  and 100, 400 and 1500 m over its forest and grass border: `lod_compare.png`. A fly run: `fly_compare.png`.

- **Review** (a fresh subagent given the diff and the task): `unfold` checked by hand against `fold`, no
  out-of-range reads, both sides of a seam resolve two plants the same way. Findings: (1) a lakeside crown at
  one quarter point turned a mostly-water coarse column into a leaf pillar that hid the lake: fixed, a column
  whose middle is under water keeps its ground; (2) a column near a seam asks one to three more faces' spots,
  about three times the `candidate` work at a corner on a cache miss, also for physics on the frame thread:
  bounded and cached, written in the perf-debt row; (3) clutter's trees-near rule sees only its own face's
  trunks, so tree-side clutter is a little thinner within two columns of a seam: cosmetic, left.

## Consequences
- The bake and `saves/bakes/` are unchanged. Every world gains trees and clutter along its seams; a save's
  edits there still win over them.
- Far tiles over forest cost more to build (perf-debt row "Forest in the far chunks"); the frame thread is
  not touched.
- The canopy is a ground, not leaves: from far off a forest edge is a 10-20 m step, its sides leaves to the
  ground. Its colour is the leaf block's own texture at the coarse cell's scale.
- Fixed views that look over a forest or along a seam render differently (accept after a look).
