# ADR 0109: Bigger, longer caves; cave water as sheets on the floor

Status: accepted (2026-11-02). Follows ADR 0077 (cave storeys, mouths), 0080 (galleries), 0104 (world water is
static; only spills simulate).

## Context
Bug reports 2026-11-02 18-56-22 ("cave generation overall needs an overhaul: clunky and too small") and 18-55-49
("water in caves is a bunch of small ponds instead of a big pond or a stream"), seed 1015715378. The rooms
(`n_cheese_`, 90 m period) and tunnels (`n_spag1/2_`, 55 m period, 2-4 m wide) of ADR 0035/0077 read as blobs:
short, every width the same, a bend every few metres. Cave pools were a 25 cm puddle sunk into every flat 0.5 m
tread where `n_pool_` was positive and all four neighbours were rock, so a tilted floor carried a puddle per step.

## Decision
1. **Rooms and tunnels grow and straighten** (`WorldGen::cave_value`, the big-cave variant only): cheese period
   90 -> 133 m and threshold 0.58 -> 0.55 (rooms ~1.5x across, smoother walls); spaghetti period 55 -> 83 m (a bend
   per ~40 m: a tunnel can be followed) and width 0.03 -> 0.045 (3-5 m), scaled by the gallery offset noise
   (`galoff`, 200 m period) to 0.5-1.5x, so a tunnel narrows and swells along its length. Galleries (ADR 0080) are
   ~4 m wide and 3 m tall instead of 3 x 2.5. Halls, ravines and mouths are unchanged.
2. **Cave water is a sheet on the floor** (`FineGen::generate_block_impl`): where `n_pool_` (now ~40 m blobs) is
   positive, cave air up to `POOL_DEPTH` (12 voxels, 1.5 m) above the nearest rock below becomes the cave biome's
   pool (water, sludge, lava), the depth growing from nothing at the blob's edge to 1.5 m at its middle. A flat hall
   floor gets one pond per blob; a tilted gallery floor (0.5 m treads) a stepped stream. The sheet crosses block
   bounds: the floor is searched up to `POOL_DEPTH` under the block through the cave lattice, which already reaches
   that far (`cy0`). LOD 0 only, like crystals. The puddle-into-the-floor code and its `enclosed` test are gone.

## Consequences
- Water follows the floor, so on a tread edge the upper tread's water stands as a 0.5 m face over the lower one
  (a cascade look on streams). The blob edge thins to a single voxel of water, no wall. Static until touched
  (ADR 0104): a dug tread spills like any still water.
- Generator time: unchanged within noise (`fine_gen_bench.gd`, before/after in the commit).
- No new material ids. `GEN_VERSION` is bumped by the merge, not here.
