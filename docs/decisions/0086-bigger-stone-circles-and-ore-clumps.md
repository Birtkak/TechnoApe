# ADR 0086: Bigger stone circles, ore stringers in clumps

Date: 2026-10-27. Status: accepted (GEN_VERSION 25).

## Context

Two bug reports from the exported build: the stone circle ruin "should be bigger" (2026-10-27 18-10-49: a
ring ~8 m across of 2-4 m stones) and ore should be "clumped together more instead of these sporadic voxels"
(2026-10-27 17-49-40: a shaft wall sprinkled with lone ore voxels). The speckles were the ore stringers of
ADR 0077: a one-voxel sheet in which each voxel was ore with chance 20-45 %, picked one by one. The vein
lenses (ADR 0064) already dig out as lumps.

## Decision

- Ruin footprints may reach 7 m from the centre (`WorldGen::RUIN_MAX_HALF` 14 macro cells, `RuinTable.MAX_HALF_M`).
  Stone circles take 5-7 m (was 3.5-5), 10-14 stones (was 8-12) 3.5-5.9 m tall (was 2-3.9), 0.9 m thick and
  1.7 m wide (was 0.6 x 1.1), a bigger altar. Ruins keep their old placement margin (14 cells from the cell's edge), so none moves;
  a big circle's ramp may reach 4 cells into the next cell, which `ruins_near` searches with the new maximum.
- Stringer sheets are 2.5 times thicker (`STRINGER_HALF` 0.03) and hold ore only where a value noise ~0.45 m
  across is over a threshold that keeps 8 % (20 m down) to 18 % (200 m down) of the sheet, so the clumps hold
  the same amount of ore as before; one ore per 0.5 m cell, so a clump is mostly one ore.

## Consequences

Measured with `game/tools/ruin_ore_sheet.gd` (seed 1337, ~21 M rock voxels round four deep caves): ore
2.58 per mille before, 2.64 after; tallest stone of the nearest circle 3.25 m before, 5.75 m after.
