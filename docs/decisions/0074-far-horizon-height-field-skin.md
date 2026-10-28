# ADR 0074: A far horizon like Lay of the Land's: a height-field skin behind the box, composited per pixel

Status: superseded by ADR 0105 (2026-10-28: the far layer is removed; render distance only). Was: superseded by ADR 0075 (2026-10-27) as the far layer's mesher: the smooth skin neither read
as the voxel world nor could show a player's build, so the far layer is now voxel cubes from the coarse
data. Decisions 5, 6 and 8 (the compositing, the edit scan, the loading) carry over; the skin stays as
`far_mesher=field` for comparison until ADR 0075 is accepted. The 40 m box with no LOD (ADR 0073)
stays the default; the far horizon is a Settings > Video option (off by default) and `far=1`.

## Context
ADR 0020, 0072 and 0073 tried voxel LODs with godot_voxel's blocky mesher three times at 12.5 cm and
dropped them: 2^n cubes of cubes, black water trenches, terrace rims, a 4x4 dither that read as
stitching, clipbox streaming that hid LOD 0 under coarse parents and lost a 20 m/s flight. The hand-off
(`docs/plans/lotl-mesh-lod-handoff.md`) asked for what Lay of the Land has: terrain to the horizon with
no visible transition. The research note (`docs/research/2026-10-26-lay-of-the-land-distance.md`)
found no published technique, only its 10 cm voxel, Unreal, TAA and footage: the silhouette of the
terrain, flat water, trees as blobs to the horizon, colour continuous with the near voxels, haze doing
the rest, nothing coarser than a pixel ever shown. Our frame budget is roughly a third of theirs
(RTX 3070 at 8.3 ms p99 against a GTX 1660 Ti at 30 fps), so the far layer has to be far cheaper than
drawing far voxel volumes.

Measurements plan (set before the work): the ADR 0073 runs at seed 272905368, 1280x720, vsync off, ADR
0014 method, no other Godot process: standing (`walk=1 speed=0 duration=20`), flight (`speed=20 alt=30
duration=16`), the report pose (`speed=0 alt=40 pitch=-0.05`), 100 m up; `budget_ok`, VRAM, first
load, next to `game/out/nolod/stand_40.json` and `fly_40.json`; the look judged by the developer from
`game/out/far/compare_*.png`.

## Decision (P13, `prototypes/p13-far-mesh/README.md`)
1. **The far layer is a column height-field skin, not voxels** (candidate b). Native `FarMesh`
   (`prototypes/p5-sim-spike/src/far_mesh.cpp`, a friend of FineGen and WorldGen) builds tiles of
   64 x 64 cells from the generator's own macro lattice (`macro_point`: the same hc, gw, hydrology and
   biome the voxels come from) with ground_f's swells, ledges and hummocks faded by level; cell size
   doubles per ring: 25 cm to 48 m, 50 cm to 96 m, 1 m to 192 m, 2 m to 384 m, 4 m to 768 m, 8 m to
   1.5 km (a cell stays about 5 px at 1280x720; rings twice as wide quadrupled the triangles for no
   visible gain). Top materials follow generate_block's chain (bed, river
   sand, rock, snow, steep stone, scree, alpine, marsh, biome); vertex colours are the tiles' averages
   (`FarColors`), what far_fade turns the near voxels into by 40 m, plus a per-25 cm speckle in the
   shader (`far_speckle`) because Lay of the Land keeps the speckle at distance.
2. **Rings morph, never pop (CDLOD).** Every vertex carries the height, colour and normal it has at
   the parent level (UV.x, CUSTOM0, UV2); the parent values are computed with the parent's own rules,
   and `test_far_mesh` checks that they equal the parent tile's vertices at every shared position. A
   quadtree splits a tile while its nearest point is inside the next finer ring, so a tile that
   touches a coarser one lies wholly past its ring, fully morphed: no cracks. Stale tiles stay until
   their replacement is drawn (no holes), then their nodes go to a pool (a RenderingServer free blocks
   on the render thread, ADR 0020).
3. **Water is a flat sheet per wet cell** at the highest wet corner's level, coloured by depth over
   the bed's colour: lakes and the sea are flat, a river steps down cell by cell, never a trench.
4. **Trees are instances to 384 m, a baked canopy beyond.** WorldGen's tree cells give crown blobs
   (a 6 x 3 sphere, scaled per FineGen's age formula, spruces taller) and trunks (to 96 m) as
   MultiMesh buffers built natively; past level 3 the skin itself rises by a leaf-coloured canopy
   where the forest is dense, and the last instance level shrinks its trees away over its morph band.
5. **Composited behind the near box, blended per pixel.** `FarLayer` (`scripts/visuals/far_layer.gd`)
   renders its meshes with their own camera into a transparent SubViewport (`FarView`, HDR, no
   tonemap or glow, fog and ambient synced from the world's environment every frame, lit by a
   shadowless copy of the sun) that owns its own World3D: the far camera culls the far meshes only,
   never the thousands of voxel block instances (a second cull of those cost the render thread
   12-16 ms a frame in flight). Tiles build on two low-priority threads of the layer's own (the
   engine's WorkerThreadPool carries the renderer's per-frame group tasks; the sim's and
   godot_voxel's threads already fill the twelve cores). The
   sky shader lays that texture over the sky, and the terrain shaders' horizon blend (ADR 0073's
   fade into the sky's colour) now fades into whatever the far view has behind the pixel
   (`horizon_sky(wpos, cam, SCREEN_UV)`). Nothing of the far layer is ever drawn in front of a near
   voxel; pixels nearer than the blend's start are discarded in the far shaders.
6. **Edits show.** `World.set_voxel` / `write_chunk` mark their chunk column; FarLayer re-reads marked
   columns on worker threads (`FarMesh::column_tops`: the real top solid voxel against the generated
   one, so trees and pickups cancel out and only true changes register) and rebuilds the tiles over
   them with those heights at every level (`out/far/edit2.json`: blast craters at the spawn, 119
   edited columns, 50 tile rebuilds within a second). Not persisted yet: after a reload the far layer
   forgets edits until a column is touched again (open). The hand-off's screenshot of a placed 4 m
   cube and a crater from 150 m is not taken: the scripted shape needs 31 k cobblestone and a blast
   from 60 m up lands outside the sim bubble; it is a playtest item (`docs/plans/to-test.md`).
7. **Not chosen.** (a) Surface nets over FineGen's coarse voxel data (`build_nets_tile`,
   `far_mesher=nets`): 3-4x the triangles (18-26 M), 45 ms a tile against 6, p99 over the budget at
   the pose, and the look is lumpy blobs whose colours average wood, leaves and ground; it would show
   caves and overhangs, which a skin cannot, but nothing at the horizon needs them. (c) Transvoxel on
   an SDF channel: it needs a second VoxelLodTerrain with the clipbox streaming and the hiding that
   ADR 0073 removed, plus an SDF generator; not spiked.
8. **Loading.** The loading screen and the settle check count the far layer's pending tiles; the
   first 260 tiles take about two seconds on the two worker threads after the box.

## Measured (seed 272905368, 1280x720, vsync off, far_m 1536)
| run | 40 m box (ADR 0073) | box + far horizon |
|---|---|---|
| standing 20 s: p99 / max / > 16.7 | 6.2 / 8.8 ms / 0 (today 5.0 / 6.8) | 7.2-7.6 / 8.3-12.4 ms / 0, `budget_ok` |
| report pose (alt 40): p99 / max | 3.5 / 14.7 ms | 4.4 / 8.6 ms |
| 100 m up: p99 / max | 2.9 / 7.4 ms | 3.8 / 8.3 ms |
| 20 m/s flight 16 s: p99 / max / > 16.7 | 5.2 / 5.7 ms / 0 (today 4.9 / 16-31 / 0-2) | 8.0-11 / 50-90 ms / 14-25: **fails** |
| first load (standing) | 4.1 s | 4.9-7.2 s |
| VRAM standing / flight | 0.9 / 0.9 GB | 1.04 / 1.06 GB |
| far tiles / triangles / trees | - | 259 / 2.6-3.0 M / 4.4 k |
| tile build (worker) avg / max | - | 14 / 250 ms (the 8 m level: 4 base-column reads per node) |

Screens: `game/out/far/compare_pose.png`, `compare_high.png`, `compare_seam.png` (left the box
alone, right with the far horizon; `pose3`, `high3`, `seam3` are the final rings): the haze beyond
40 m becomes forest, river and coast to the horizon with a mountain's silhouette at the back; at
ground level the beach runs on past the box's edge with no line.

**The flight does not hold the budget yet.** With the far horizon on, a 20 m/s flight has 14-25
frames over 16.7 ms in a cluster where the near box streams a forest edge (mesh deltas of 36-48 MB
in a frame, past the 4 MB upload budget of `voxel_patch_budget.py`); the same build without the far
layer has 0-2 such frames and the ADR 0073 run had none. Isolated one at a time (P13 README): the
tile builds (frozen after load: still spikes), the drawing (tiles hidden: still spikes), the far view's
render (second cull of the voxel world: fixed by the own world, 12-16 -> 1.4 ms), thread priority,
half resolution, half rate, a 1.5 MB upload budget: none removes the cluster; only not rendering the
far view at all did once. The reading: the far layer leaves the render thread no slack, so the box's
own upload bursts, which it absorbs alone, stall the main thread (the stalls land in godot_voxel's
process, unattributed by the per-node timers). Next: make the apply budget bite on streaming bursts
(it does not: 48 MB passes), then re-measure; until then the far horizon stays off by default, which
ADR 0009 requires anyway.

## Consequences
- Off by default: nothing changes for a player until the option is on. On: +1 to +1.5 ms p99
  standing, +0.15 GB VRAM, the horizon at 1.5 km, and the flight stalls above.
- Known limits: felled far trees still stand as blobs past 30 m; edits are forgotten on reload;
  caves and overhangs are closed in the far skin; far water is opaque and flat; a far tree is a blob,
  not FineGen's lumpy crown; the 4 m and 8 m levels build in 250 ms (a level-independent slope from
  the lattice would cut it). Tunables: `far_speckle`, `far_scale`, `far_threads`, `R0_M`,
  `MORPH_FROM`, `CANOPY_M`, the water colours in `far_water.gdshader`; checks `far_debug=
  noview|nodraw|freeze|halfrate|notrees|flat|nocustom`.
- Tests: `test_far_mesh` (grid, parent values, water, trees, canopy, nets), `test_menus` (the Far
  horizon row). Probe: `tools/far_probe.gd`.
- The sim extension is rebuilt (FarMesh); shaders `far_ground / far_water / far_tree`, `sky.gdshader`
  and `terrain_common.gdshaderinc` carry the far texture.
