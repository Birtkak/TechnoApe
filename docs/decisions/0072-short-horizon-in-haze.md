# ADR 0072: A short horizon that ends in haze

Status: accepted (2026-10-26)

## Context
ADR 0020 drew the vision's 2 km horizon with eight voxel LODs (`VoxelLodTerrain`, clipbox rings of
24 m: 25 cm cubes from 24 m, 50 cm from 48 m, 1 m from 96 m ... 16 m cubes at the far edge). At
12.5 cm voxels that never looked right: bug report 2026-10-26_19-36-29 ("LOD looks very fucked, let's
remove the LOD system") shows coarse rings from 50 m out, black trenches where the coarse mesher turns
a one-voxel water step into a full-height dark face, terrace rims on every beach, and a flat grey
slab with cardboard mountains at the back. Flying, the far rings also streamed too slowly to keep up.
The developer's call: unless the LOD can be made 5x better in a normal amount of work, hide or
simulate distance another way. A painted heightmap horizon (the generator's column query drawn as a
2D clipmap beyond ~100 m) was proposed and turned down; the developer chose **fog first, short
horizon**: voxels to ~200 m, then real atmosphere.

## Decision
- **Horizon = 200 m** (`Settings.view_distance_m`; the settings menu offers 200 and 400 m, old
  `settings.cfg` files with 1-2 km are clamped). `lod_count` is derived from it
  (`Settings.lods_for_view`): 4 LODs with 24 m LOD 0 and 32 m rings (LOD n ends at
  `render_distance_m + lod_ring_m * 2^n`: 25 cm cubes to 56 m, 50 cm to 88 m, 1 m to 152 m, 2 m to
  the edge). A command-line `lod_count=` still wins.
- **The terrain dissolves into the haze**: `terrain_common.gdshaderinc` gets `horizon_start` /
  `horizon_end` (world voxels from the camera; `world.gd` sets them on every terrain material to
  75 % and 100 % of the view distance) and `fade_discard`, the LOD cross-fade's ordered dither reused
  by distance, so the voxel world thins out and is gone at the edge instead of ending at a wall of
  cubes. Pixels that drop show the sky.
- **The sky below the horizon is the haze colour**: `sky.gdshader`'s ground band is set from
  `fog_color_day` (`atmosphere.gd`) and widened (0.12 -> 0.35 of `dir.y`), so from the air the
  dissolve meets haze, not a grey floor. Day fog is thicker: `fog_per_km` 2.8 -> 6.0 (golden grade),
  1.1 -> 3.0 (clear); 30 % of the light is left at 200 m, with aerial perspective 0.72 the far
  terrain already sits in the sky's colour. `far_fade_end_m` 200 -> 160 so textures reach their
  average colour before they dissolve.
- **The freed budget goes into the rings, not LOD 0** (the developer: "make LOD quality better since
  there are fewer now"). Measured at seed 272905368 (the report's world), 1280x720, vsync off, ADR
  0014 method, no other Godot process unless noted:

  | LOD 0 / ring | first load, standing | standing 30 s after load | 20 m/s flight 16 s | RAM / VRAM |
  |---|---|---|---|---|
  | 24 / 24 (ADR 0020 rings) | 11.1 s | p99 6.5 ms, max 7.1, 0 frames > 16.7 | p99 6.5, max 46 | 0.6 / 1.1 GB |
  | **24 / 32 (chosen)** | 17.6 s | p99 7.7 ms, max 9.8, 0 frames > 16.7 | p99 6.9, max 50 (one other Godot running) | 0.9 / 1.4 GB |
  | 32 / 32 | 22.4 s | p99 34 ms, 762 of 1199 frames > 16.7 | p99 2.2, max 21 | 1.1 / 1.8 GB |
  | 24 / 48 | 31.3 s | p99 36 ms (one other Godot running) | p99 2.8, max 28 | 1.4 / 2.0 GB |
  | 48 / 48 | - | - | p99 7.3, max 90 | 1.7 / 1.5 GB |

  Flight numbers flatter the wide settings because the generator cannot keep up and frames idle;
  standing after load is the honest one, and a 32 m LOD 0 fails it outright (the near rings keep
  streaming for the whole 30 s at 35 ms a frame). 32 m rings cost 6.5 s of first load and 0.3 GB and
  hold the budget; 48 m rings do not. `MAX_RING_M` = 48 keeps them one setting away. Presets: low
  16/16, medium 24/24, high 24/32; the menu's "Full detail to" keeps the ring at 32 m unless the
  detail radius is wider.
- The old 8-LOD baseline at the same flight: p99 8.3 ms, max 62, 1.7 GB VRAM, and a first load that
  never settled within the 10 s cap.

## Consequences
- The 2 km horizon and the far-mountain silhouettes are gone; vision.md §6 is rewritten (a horizon
  that never looks worse than the world). Distant mountains and far builds come back only with a
  technique that draws them well: a painted far layer from the generator's column query, or impostors.
  Either sits on top of this change (the dissolve band is where it would join).
- The day look is hazier (visibility ~ 500 m): a tunable (`fog_per_km`, the grades) the developer
  judges in play; `horizon_fade_frac` moves the dissolve band.
- Player edits are drawn to 200 m as before (the rings still come from the voxel data and
  `cache_generated_blocks`).
- First load 11 -> 18 s on this machine from the 32 m rings. If that reads as slow in play, 24/24 is
  the medium preset.
- Checks: `out/horizon/` (before/after at the report's pose, the runs in the table), `test_lighting_grades`
  (the new fog values), `test_menus`, `test_world_gen_lod`.
