# ADR 0077: Cave storeys, entrances and ore stringers

Date: 2026-10-27. Status: accepted (GEN_VERSION 23).

## Context

The developer asked for a cave generation overhaul with the choices left to Claude: entrances, deep caves,
"multiple segments", how the ores sit. ADR 0052 gave caves tunnels, halls, ravines, straight ramp mouths and
geodes; ADR 0063 flat treads and pools. What was missing: a reason to go *down* (every depth looked like the
last), vertical travel between the tunnel layer and the halls (you found a hall by luck), halls that read as
one noise blob, entrances that were all the same straight tube, and ore that lived only in the 96 m lenses,
so a cave wall never glinted.

## Decision

All in `WorldGen::cave_value` / `mouths_near` and the FineGen column pass; nothing new in GDScript.

1. **Three storeys.** The cheese noise's y-squash eases from 1.6 (flat rooms) to 0.5 past ~100-170 m down, so
   the deep is tall rifts and chasms, not more rooms. Tunnels near the top, halls (ADR 0052) from 15-35 m down,
   rifts below, the lava sea at the bottom.
2. **Entrances and connectors** share the 64 m mouth cell, one roll per cell: 10 % a **ramp** that now winds
   (its centre line sways ±1.2 r, a bend per ~15 m), 6 % a **sinkhole** (a funnel twice the shaft's radius at the
   rim, a 2.5-4.5 m shaft 30-68 m deep, ending in the hall storey), 16 % a **chimney** (a 1.5-3 m shaft starting
   15-47 m down in the tunnels, 80-180 m long, leaning with depth, through the halls into the rifts; never open to
   the sky). `cave_roof` opens the ground over sinkholes and the wider ramp; `ravine_near` keeps trees off
   sinkhole rims. `find_nearest("sinkhole" | "chimney")`.
3. **Pillars.** A 2D noise (~11 m period) per column; where it peaks the hall field becomes rock from floor to
   ceiling, 2-3 m thick. Halls read as colonnades, and the pillars are what a glance remembers.
4. **Ore stringers.** Below 20 m, where a 3D value noise sits within 0.022 of its middle (wavy sheets about
   one voxel thick, ~4 % of the rock) and within 1.5 m of cave air, 30-60 % of the sheet's voxels are ore
   (deeper = richer), in the ore of the column's 96 m cell: the lens's type where the cell has one, otherwise
   a weighted pick (`stringer_type`). A cave wall shows what the cell's lens holds; the lens stays the prize
   (stringers are a few nuggets per metre of wall, and only near caves).

## Consequences

- GEN_VERSION 23: old saves are not loaded over the new terrain.
- Every `cave_value` sample now does 9 cell hashes down to 260 m instead of 40 m (the chimney range); the
  per-mouth maths is only for cells that rolled a mouth. Measured with `tools/cave_sheet.gd` (`ms` per row).
- Rejected: Noita-style stamped dungeons (parked, world-generation.md "Next"), a cave-river layer (water needs
  the hydrology pass), per-storey biomes (the depth bands of `biomes.json` already do that).
