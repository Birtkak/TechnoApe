# ADR 0066: Smooth teleports: instant hops, a short cover on visible ground, viewer-first streaming

Status: accepted (2026-10-26)

## Context
Bug report 2026-10-25_21-10-57: "make teleporting around more smooth". Every move past 24 m (/tp, /goto,
/spawn, a respawn, a bed, a standing stone) put up the loading cover until godot_voxel had no task left,
i.e. until the whole 2 km horizon around the destination was generated (100k-280k tasks). Measured with
the new `tp_test=1` burst (seed 1579829108, survival, on foot; `game/out/tp_before/`): covers of 3.2 s for
a 60 m hop, 4.8 s for 95 m, 8-22 s for 160 m to 3 km jumps (the log in the report: 9.9-13.4 s). Nobody fell
through the ground (the body already waits for its voxels, `Player._ground_ready`); the falls and deaths
in that report came from the blast gadgets, not from the teleports.

Lifting the cover as soon as the near blocks were meshed (`is_area_meshed`) was tried first: covers of
1.6-2.3 s, but the filmstrip showed the player standing in a hole over the grey sea plane with 1-2 m LOD
cubes around: the same picture as the report's screenshot. With clipbox streaming a LOD n block is only
shown under an active LOD n+1 parent whose 8 children are all loaded, from the coarsest LOD down, and
godot_voxel's task priority runs every finer LOD before any coarser one (band1 = MAX_LOD - lod), so the
coarse blocks over the player come last and the meshed detail stays hidden for most of the horizon load.

## Decision
1. **Hops are instant.** A move under 100 m onto terrain that is already drawn (`World.is_view_ready`: the
   destination's ground shows at LOD 0-3) gets no cover; the coarse meshes there sharpen in place and the
   body hangs 0.1-0.2 s until its voxels are in. Moves under 24 m never had a cover.
2. **The teleport cover waits for visible ground, not the horizon.** `World.near_ready` samples the
   generator's surface on rings around the player (feet to 16 m at LOD 2 or finer, 32 and 48 m at LOD 3 or
   finer, points more than 12 m above or below the player skipped) and asks godot_voxel whether the mesh
   block there is applied and visible (`debug_get_mesh_block_info`); the cover fades once 95 % show, the
   bar shows that share. Under it the player and animals stay frozen, workers run at cores - 2 and the
   upload budget is off, as before. A quick cover is a blink: it cuts to dark and fades back in over
   0.35 s, the title and bar only appear after 0.6 s. Cap 20 s (an edited surface that never shows).
3. **Viewer-first streaming under the cover** (`prototypes/p11-lod-horizon/voxel_patch_priority.py`, our
   godot_voxel build): `VoxelLodTerrain.viewer_chain_first` gives the blocks within `128 << lod` voxels of
   the viewer at every LOD the top priority band, so the chain of parents over the player loads first. It
   is a global switch, on only while a teleport cover is up: always on, flight streaming (speed=20) lost
   ~1.3 ms p99 and had a 25 ms worst frame; off, the build measures as before.
4. `tp_test=1` (`scripts/debug/tp_test.gd`) stays as the check: 15/60/95 m hops, 500 m, 3 km, /spawn,
   /goto lake, /goto cassiterite_vein, /kill; per jump the cover time, frames under it and in the 4 s
   after it, where the slow ones went (a probe after each child of Main and World), the landing, and a
   filmstrip row.

## Measured (`game/out/tp_before/`, `game/out/tp_after/`, same machine, 2 other Godot runs alive)
| jump | cover before | cover after | slow frames after the cover / cut, before -> after |
|---|---|---|---|
| hop 15 m | none | none | 0 -> 0 |
| hop 60 m | 3.2 s | none | 0 -> 1 |
| hop 95 m | 4.8 s | none | 1 -> 0 |
| 500 m | 12.8 s | 8.0 s | 0 -> 0 |
| 3 km | 11.0 s | 7.5 s | 0 -> 0 |
| hop 40 m right after it | 2.2 s | none | 14 -> 0 |
| /spawn (3 km) | 22.0 s | 8.9 s | 26 -> 0 |
| /goto lake (680 m) | 12.4 s | 7.0 s | 3 -> 0 |
| /goto vein (160 m) | 7.9 s | 4.1 s | 1 -> 1 |
| /kill respawn (760 m) | 15.4 s | 12.7 s | 8 -> 2 |
No jump fell or landed hard in either run. Hops have 1-3 frames of 19-27 ms at the cut itself (the first
0.25 s): the threaded LOD update holds godot_voxel's data lock while it re-centres the clipbox, and the
first main-thread voxel read (particles, the player's physics) waits for it.

## Consequences
- The far horizon streams in behind a lifted cover (LOD cross-fades, ADR 0020); frames after the lift held
  the budget in quiet runs, but with other Godot processes on the box they reach 20-60 ms while 100k+
  generation tasks run (main thread ~1 ms: CPU contention, not our code). Holding fewer workers after the
  lift was tried in an interleaved A/B and showed no difference under that noise; not shipped.
- Under the cover single frames of 0.5-3 s happen when a second far jump follows within ~10 s: godot_voxel's
  engine updater applies the cancelled tasks of the previous horizon in one frame. Hidden by the cover,
  but the bar stalls; a time-budgeted dequeue in godot_voxel would fix it.
- The build needs `voxel_patch_priority.py` applied; without it `viewer_chain_first` is absent and the
  covers are ~4 s longer (Main checks for the property).

## Amendment 2026-10-26: shorter covers

The developer chose a shorter cover that shows coarse ground sharpening after it. The cover now lifts when
the ground out to 48 m shows at LOD 3 (1 m cubes) or finer, and waits only for the sim chunks within 1 chunk
(2 m) of the player (`LoadingScreen.NEAR_BUBBLE_R`) instead of the 9x9 near window, which the trace showed
was the last thing holding it (ground visible at ~3 s, bubble at ~7 s). `tp_test=1`, same seed, a perf agent's
Godot run alongside: covers 1.6-4.5 s (were 5-18 s), no falls or hangs, 7 frames over 16.7 ms after the
covers (worst 23.8 ms).
