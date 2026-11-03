# ADR 0124: The streaming shell reaches under an opening only as deep as the opening

Status: accepted (2026-11-03). Follows ADR 0105 (render distance only: the ground shell), 0108 (near-first streaming).

## Context
Budget hunt on merged main (2026-11-03). A 20 s spectator fly at the default 72 m generates ~145 k blocks, 36 s of
generation thread time on six threads; **100 k of them lie wholly under their columns' ground** and are invisible from
the air (`gen_stats`: `blocks_below`, `pro_us_below`). Generation keeping up is what limits the view distance: at 96 m
the load never settles in 10 s (backlog 57 % busy), at 120 m the backlog stays 95 % busy.

Two rules of `FineGen::ground_range` (the shell's data column, ADR 0105) decide how deep a stack goes:
1. a flat **80 m** under the lowest ground wherever any lattice point of the 16 x 16 footprint has `cave_roof` 0 (a
   ravine line, a sinkhole's rim, a ramp's mouth), so that an opening seen from the air has its walls and floor;
2. `shell_below_m` (**8 m**) under the lowest ground of the 3 x 3 footprints around a stack.

## Decision
1. **An opening keeps a column only as deep as itself.** `WorldGen::open_depth(x, z)` (macro voxels) is the ravine's
   own floor (`ravine_value`'s `deep`, 10-40 m), the sinkhole's bottom (`top + len + r`, 30-70 m) or the first 8 m
   of a ramp's tunnel, plus 2 m; `MacroPt::open_depth` carries it (built with `roof`), and `ground_range` takes
   `min(lowest ground, ground - open_depth - 1 m)` per open lattice point instead of `lowest ground - 80 m`.
   Everything a player can look into from above still has its floor (a ravine seen from inside: walls and sandy floor
   in `game/out/shell_look`); caves, mines and anything dug still get real voxels through the 32 m near sphere and
   the simulation bubble's own viewer (ADR 0105), which this does not touch.
2. **`shell_below_m` stays 8 m here; 4 m is the measured, safe next step** for the overseer and developer to take:
   the ground-detail probe (`detail.missing_share`) reads 0 at 4 m and 0.68 at 2 m (holes), so 4 m is the floor.

## Measured (spectator fly, vsync off, 20 s, quiet machine; blocks below ground / generation thread s / load s / backlog busy)

| | 72 m | 96 m | 120 m |
|---|---|---|---|
| before | 100 k / 38.1 / 8.6 / 21 % | 150 k / 61.9 / >10 / 57 % | 193 k / 76.2 / >10 / 95 % |
| open depth (1) | 96 k / 35.9 / 8.7 / 20 % | 138 k / 53.1 / >10 / 45 % | 169 k / 66.1 / >10 / 76 % |
| (1) + `shell_below_m` 4 | 69 k / 27.3 / **7.2** / 16 % | | |
| (1) + `shell_below_m` 2 | 63 k / 25.2 / 6.5 / 15 % — detail missing 0.68: too thin | | |

Frames hold the budget in every row (p99 2.9-4.4 ms, max under 12 ms). The RAM budget is 8 GB (CLAUDE.md), so 120 m's
4.2 GB is within it; generation is the limit.

## Consequences
- Generator output is unchanged (the shell decides which blocks are asked for, not what they hold): the 1 600-block
  hash probe and `test_fine_gen_top` are the guards; 28 generator tests green.
- Below the shell a stack is generated on demand when the player comes within the near sphere, as before.
- What is left: the 69-96 k blocks that remain are the soil band under the surface of every stack (needed for cliffs,
  banks and what the player digs first). The next cut is per block, not per stack: a block wholly below the soil
  whose only content is rock and ore could take a slimmer pass (`pro_us_below`), which ADR 0105's uniform shortcut
  already gives to the fully uniform ones.
