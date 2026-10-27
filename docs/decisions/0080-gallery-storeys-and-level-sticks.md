# ADR 0080: Gallery storeys, open cave tops, sticks on level ground

Date: 2026-10-27. Status: accepted (GEN_VERSION 24).

## Context

Three bug reports on seed 714593343 after ADR 0077:

- 15-46-51, "caves basically still impossible to navigate and follow". A vertical slice at the report shows why:
  the cave air is the cheese blobs and the noodles, which climb and drop every few metres and end in pockets.
  Nothing runs level for long, so there is no passage to follow.
- 15-20-15, "weird cave generation and floating grass" over a sinkhole / ravine. Where the cave field ends a
  band (4 voxels) or two under the ground, the skin above it stays: a slab of grass with tufts over the hole.
- 15-16-47, "twigs on cliffsides with like 3 voxels; stick and stone spawn rate is a bit overtuned". A fallen
  stick is laid per column on that column's ground, so one that crosses a ledge or a cliff leaves a few voxels
  on each step.

## Decision

1. **Gallery storeys** (`WorldGen::cave_value`, big-cave variant). Every 14 m down, from 6 m to 110 m under the
   ground, a storey of passages: the zero lines of a 2D noise per storey (`n_gallery_`, ~60 m period, offset
   per storey), ~3 m wide where the noise is steep and wider at the junctions, 2.5 m tall with a flat floor and
   ceiling. The storey's height tilts with a slow 2D noise (`n_galoff_`, ±5 m over ~100 m, under 10 degrees),
   so a passage can be walked for hundreds of metres. Noodles, chimneys, sinkholes and halls cross the
   storeys and connect them. A network, not a single worm: it loops, so a player can always come back.
2. **Open cave tops** (FineGen column pass). In a column that may open to the sky (`cave_roof == 0`: sinkholes,
   ramps, ravines), when the cave field is within three bands of the ground, the top two bands are opened too,
   with no plants on them. The cave lattice reaches 2 macro units further down so every block decides this the
   same way.
3. **Sticks only on level ground.** A stick stamp is dropped when the ground at either end (half length + 1
   voxel along its direction) differs by more than 2 voxels from the ground at its middle.
4. **Fewer sticks and stones.** Stick cells: ~70 % of the old chance (0.03 + 0.37 x tree density instead of
   0.05 + 0.5). Loose stones on bare rock and gravel: 2.2 % of columns instead of 5 %, flint 1 % instead of
   1.6 %; the biome pebble shares are halved.
5. `find_nearest("cavern")` skips hall pillars (ADR 0077 left it out, so `/tp` and the hall test could land
   in a pillar).

## Measured

- Evidence: `game/out/adr0080/caves_before_after.png` (vertical slice before/after, three storeys after).
- Same seed, 96 m square at the twig report (old growth forest on a slope): twig voxels 21568 -> 4017, twig
  voxels on a step (ground off by more than 3 voxels within 3 voxels) 4697 -> 326, loose stone voxels
  12399 -> 8978.
- `cave_sheet.gd` at the cave report: cave share 0.127 -> 0.139; ms per row unchanged within noise (2 extra 2D
  noise reads per lattice sample, a third only inside a storey's band).

## Consequences

- GEN_VERSION 24: old saves are not loaded over the new terrain.
- `test_pickups_are_big_enough_to_see` samples a 64 m square instead of 32 m (sparser pickups).
- Not verified in a window: how a gallery reads with the cave lighting, and the sinkhole rim from the
  15-20-15 report (headless counts and slices only).
- Rejected: worm (Perlin-worm) tunnels. They need per-worm state across chunks; the zero-line network gives the
  same followable passages from stateless noise.
