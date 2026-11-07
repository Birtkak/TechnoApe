# ADR 0147: Prospecting density mode, the exact ore share of the rock below

Status: accepted (2026-11-07)

## Context
The prospecting pick (ADR 0061, 0091) only says which veins lie within range. Bug report
2026-11-07_10-36-27 asked for Vintage Story's density mode: strike anywhere and read how much of the
rock below is ore. In Vintage Story that is a chance-weighted reading of the ore map; ours must be certain.

## Decision
1. **A fourth mode, "density" (Ore in the rock below)**, beside nearest / all / map in the pick's panel
   (Shift + right click, `/prospect menu density`). Exact picks only; the flint pick still senses.
2. **The reading is a census, not a roll:** the generator's own blocks for a `Settings.prospect_width_m`
   (3 m) square column from the struck voxel down `Settings.prospect_depth_m` (12 m) are generated natively
   (`FineGen.generate_block`) and counted natively (`ShapeTool.tally`, new, 2 bytes per voxel). GDScript sees
   only a {material id: count} dictionary (CLAUDE.md rule 8).
3. **"Rock" is every voxel the pickaxe digs** (mining.json tool `pickaxe`: stone kinds and ores; not soil,
   gravel, water or air). The share of an ore is its voxels over all rock voxels. Ores are those the veins hold
   (`Prospecting.ore_names`); the panel's ore choice narrows the line to one ore. No ore: "No ore in the rock below".
4. Density mode puts nothing on the map: it reads a column, not a vein.

## Consequences
- It reads the generated world, so rock you mined out yourself still counts. Exact for untouched ground.
- One tap generates ~55 k voxels on the main thread (measured in the run notes of the commit). Raise the
  depth or width only with that cost in mind.
