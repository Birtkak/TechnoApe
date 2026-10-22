# 0015 – Surface stains (Noita): coatings on empty cells, fed by the simulation

- **Status:** accepted
- **Date:** 2026-10-22 (asked for: "make it so voxels can be stained like
  pixels in Noita can be when you bleed on it or spill oil")

## Context
In Noita, blood, oil and soot leave coatings on the ground they touched, and
the coating stays after the liquid is gone. Our voxels are 0.5 m: tinting a
whole block per stain looks like paint buckets, not splatter. The work had to
stay out of the simulation kernel (another session iterates on it) and
within the frame budget (ADR 0009, CLAUDE.md rule 8: GDScript never iterates
voxels).

## Decision
- **A stain belongs to an empty cell** (air or a passable liquid/gas): where
  the liquid or flame was. It colours the faces that border that cell:
  floors fully, walls up to a tide line that rises with the amount, ceilings
  at a third. The solid voxel itself is unchanged, so chiselling and mining
  need no stain logic; placing a block into a stained cell clears it.
- **Pixel splatter in the shader** (`terrain_common.gdshaderinc`,
  `surface_stain`): per stain pixel (16 per voxel edge, the atlas resolution)
  the amount is interpolated across neighbouring cells in the face plane and
  thresholded against two noises, so edges are ragged and thin stains are
  speckles. Item cubes and the viewmodel use `world.prop_material`, a copy
  with stains off.
- **Storage**: a sparse map world chunk -> 16 x 256 RGBA8 image (rgb =
  colour x amount, a = amount), at most 1024 chunks (16 MB), farthest
  evicted. The GPU sees a 64 x 32 x 64 m window around the player in a
  Texture2DArray (layer = world y mod 64, texel = world x, z mod 128); a
  window move rewrites only slots that held or receive stains, and the
  shader trusts only the overlap of old and new window until those layers
  are uploaded (4 layers of 64 KB per frame).
- **Sources**: `SimBubble.chunks_changed` lists the chunks each tick
  changed; StainMap queues them and reads them on frames when no tick runs
  (`is_idle()`; a read during a tick would wait for it). One native chunk
  copy, then `PackedByteArray.count/find` locate blood, oil, fire (soot) and
  lava (scorch); GDScript touches only the hits. Water in a stained cell
  washes it. Budget: 0.4 ms per frame (measured max 0.55 ms: the check is per
  chunk).
- **Blood** is material 47: a liquid with a lifetime (200 ticks), so a
  puddle runs, soaks in and leaves its stain. Losing health bleeds: a
  splatter at the feet, liquid blood for hits of 3+ half-hearts.
- Stains are presentation: not saved with the world yet, not sent over the
  wire.

## Consequences
- Cost with nothing stained: +0.08 ms/frame (forest, 1280x720: p99 2.54 ->
  2.62 ms). A spill test (`stain_test=1`) shows 40-240 frames over 8.3 ms per
  16 s with stains on or off: those frames carry ~3 MB of mesh uploads from
  the moving liquids (remeshing pushed chunks), not stain work. Stain-on runs
  pushed ~20% more chunks than stain-off runs; a headless check shows the
  kernel's results are identical with and without the reads. Push counts
  depend on frame timing by design: pastes run under a wall-clock budget
  between ticks, and a chunk changed again while queued is pasted once.
- Forest fire (`fire_test=14`, the worst dirty-chunk load): stains on vs
  `stains=0` p99 9.5-10.1 vs 10.5 ms, max ~20 ms both, scan max 0.57 ms.
  Both fail ADR 0009, as does the commit before stains: VoxelLodTerrain builds
  mesh resources on the main thread, and the sun shadows make those frames
  worse (`shadows=off`: p99 7.2, max 12.9 ms). With a separate render thread
  (`rendering/driver/threads/thread_model=2`, experimental in Godot 4.7) and
  everything on, stains included: fire p99 5.9-6.1 / max 13.4-14.4 ms, spill
  test p99 5.75 / max 7.4 ms, both within budget.
- Soot from fire and scorch from lava come free with the same path; more
  sources are a line in `StainMap.sources`.
- The atlas is full (tile 47 = blood): the next tile needs a bigger atlas.
- Open: saving stains with chunks; stains on micro models and chiselled
  cells; drying (blood darkening over time); per-stain gloss for oil.
