# ADR 0067: Animal crowds hold the frame budget

Status: accepted (2026-10-26)

## Context
Bug reports of 2026-10-25: 21-11-51 "100 bears kill performance" (4 fps, three `/summon bear 32`) and
21-13-39 "very bad performance" (5 fps, the same 90 bears still standing there). The reports' JSON put
241 ms of a frame in mob physics (`mobs.frame_ms`); at that cost the physics loop runs its 8 catch-up
steps every frame, so the whole game falls to a few frames a second.

Measured with the new `mob_bench=100` run (bears scattered over 6 m, 8 m ahead of the player), per physics
step: think 2.3 ms, animate 0.7 ms, move 39 ms, of which 38.6 ms were `VoxelBody.move`, i.e. godot_voxel's
`VoxelBoxMover.get_motion`, for ~75 moves (0.52 ms each). The mover gathers the collision boxes of every voxel
in the box swept by the body; a bear is 1.4 m = 11 voxels a side, ~2 000 voxels a move, and godot_voxel read
each one with `VoxelData::get_voxel`, which takes the spatial lock and looks the block up in the map every time
(its own source marks it "TODO Optimization: read the whole box of voxels at once").

## Decision
1. **VoxelBoxMover reads per block** (`prototypes/p11-lod-horizon/voxel_patch_boxmover.py`, applied after the
   other godot_voxel patches). `collect_boxes_blocky` fetches the LOD0 blocks of the query box once
   (`get_blocks_with_voxel_data`), reads their buffers directly and skips uniform blocks without a collision
   model in one test. A block with no LOD0 data still goes through `get_voxel` voxel by voxel, so the boxes the
   mover sees are the same as before. The player, item entities and every animal use this path.
2. **Far and standing animals step less often** (`Mob`): every physics frame within 16 m of the player, every
   2nd within 40 m, every 4th beyond, staggered by uid, with the skipped time added to the step. Running
   animals (a chase, a bolt: faster than 2 m/s) keep full rate; standing ones (no walk target, on the ground)
   step at most every 2nd frame. Thinking stays at 10 Hz.
3. **Neighbour grid** (`MobManager.neighbours`): a 2 m grid rebuilt once per physics frame; the entity push
   (`separate`) only tests animals in the 3 x 3 cells around it instead of every active animal.
4. **Standing pose holds**: `MobModel.animate` is skipped while an animal stands still with its head straight
   and the last call already set that pose (not for the wild models, whose tails may move).
5. **Benchmark and stats**: `mob_bench=N [animal=] [mob_bench_r=M]` (DebugCapture) and `mobs.step_ms` in every
   snapshot (think / move / collide / animate ms per physics step, collides per step).

## Measurements
`mob_bench=N speed=0 load_cap_s=60 seed=1579829108 vsync=0`, 1280x720, three runs each, back to back on a
machine shared with one or two other Godot runs (`other_godot_processes` 1-2); medians of the runs, ms:

| case | before p50 / p99 | after p50 / p99 | mob physics per step, after |
|---|---|---|---|
| 100 bears within 6 m (the report) | 255 / 286 | 4.6 / 11.1 | 3.1 (think 1.0, move 1.5 of which collide 1.0, animate 0.5) |
| 100 bears within 30 m | 172 / 197 | 5.0 / 9.3 | 1.8 |
| 1 bear (control) | 4.1 / 7.0 | 5.1 / 7.1 | 0.05 |

A bear's collision move: 0.52 ms before, ~14 us after. Frames over 16.7 ms with 100 bears: none or one in
~3 700. The clump still misses the p99 8.3 ms line by ~3 ms: a frame with a physics step pays ~3 ms of
animal GDScript (75 of 100 bears walk or are pushed every step); the next lever would be moving the walk
step itself into native code or interpolating the models so near animals can step at 30 Hz too.

## Consequences
- The godot_voxel DLL must be rebuilt with the new patch; until then the game runs, but crowds are as slow
  as before.
- Animals beyond 16 m move at 30 Hz or 15 Hz; at that distance the steps are a few pixels.
- Liquids: the 10 fps report (21-17-38, water spreading over the map) is the liquid simulation itself
  (`sim.tick_ms` 84 ms, 21 228 bodies, 220 active chunks); left to the liquids work.
