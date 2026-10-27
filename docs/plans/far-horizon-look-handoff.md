# Hand-off: make the far horizon look like the world (ADR 0074 follow-up)

For a fresh session in this repo (`C:\Users\dev\Desktop\AI shizzle\TechnoApe`, read `CLAUDE.md` first, then
`docs/decisions/0074-far-horizon-height-field-skin.md`, `prototypes/p13-far-mesh/README.md` and
`docs/research/2026-10-26-lay-of-the-land-distance.md`). Work on branch `claude/far-mesh-lod` (4 commits ahead of
`main`, not pushed). The 40 m box stays the default; the far horizon stays off by default
(`Settings.far_layer`, `far=1`) until the developer prefers it in play and the budget holds.

## Where it stands

The far layer works: a column height-field skin (native `FarMesh`, `prototypes/p5-sim-spike/src/far_mesh.cpp`),
CDLOD rings, far water, tree instances to 384 m, edits, composited behind the box through its own SubViewport
(`game/scripts/visuals/far_layer.gd`, shaders `far_ground / far_water / far_tree.gdshader`). Seams are clean and
standing holds the budget. The developer has seen the screenshots (`game/out/far/look_pose_40m_up.png`,
`look_100m_up.png`, `look_ground_level.png`, box alone left, far horizon right) and says **the weak spots are a
problem**. Fix them. Reference for the target look: Lay of the Land's own footage (research note; the "Meet the
Biomes" video at 0:03 and the LOD devlog at 0:42): far forest is a speckled canopy of lumpy individual crowns,
mesas keep strata and cliff faces, ground keeps colour speckle, haze is thin enough to see 1-2 km.

## The weak spots, in the developer's order of pain (confirm each in a screenshot first)

1. **Far trees are smooth blobs.** `FarMesh::build_tile` emits one 6 x 3 unit sphere per tree (`crowns`) plus a box
   trunk to level 1, coloured flat per instance. FineGen's real trees are clumpy crowns on limbs
   (`FineGen::build_tree`, `FTree` blobs / segs; far `lod >= 4` already has `far_canopy` leaf domes in
   `generate_block`). Want: lumpy crowns that read as FineGen's trees from 50-400 m. Options to spike and measure:
   a handful of pre-baked crown meshes per tree kind and age (8-12 clumps each, vertex-coloured with light and
   dark leaves, shared MultiMesh per kind), per-instance rotation and scale; or noise-displaced icospheres in the
   vertex shader; or build each tree's blob list natively from the same `FTree` data and merge into the tile mesh.
   Keep the MultiMesh buffer path (GDScript per-instance building cost 30 ms frames earlier). Spruces are cones,
   acacias flat plates, birches slender: the crown shape per kind matters more than detail.
2. **The haze is heavy and the far skin is flat.** The far colours come from the atlas tile averages
   (`FarColors`), then fog (`env_bg` synced from `atmosphere.env`: `fog_per_km` 6.0 day, `aerial` 0.72, `fog_sky_affect`,
   a height mist) and the box's own `horizon_mix`. At 400 m+ everything is a beige wash; the 100 m-up shot
   loses the forest by 600 m. Want: tune the far view's own fog (it is a copy; do not change the near world's look
   without the developer) toward visibility of 1.5 km, keep a little aerial perspective, check noon, golden hour
   and overcast (`/time set`, `Atmosphere` grades). Also the skin shows only a 22 % per-25 cm speckle and no
   shading detail: add what the near voxels give (AO-like darkening in hollows from the normal / slope, strata and
   rock faces on steep slopes: `slope` is already computed per node, `top_material` picks stone / terracotta but
   cliff faces are the same smooth skin; the mesa walls in LotL keep vertical faces and bands), and consider a
   second, finer noise octave and a slight hue variation per tile patch so a plain does not read as a painted
   surface. Check against `game/out/nolod/stand_40/final.png`-style colours so the join at the box edge stays
   invisible (`far_fade` in the box flattens colours to the tile average by 40 m; the skin must match it there).
3. **Weaker, but on the list:** far water is one opaque flat colour (give it the near fluid's tint and a sun glint
   at distance: `terrain_fluid.gdshaderinc`); caves and overhangs are closed (accept); felled far trees stay as
   blobs and edits are forgotten on reload (persist the override map with the save, `Main.save_terrain`).

## The flight (separate blocker for turning it on by default)

A 20 m/s flight (`autopilot=1 speed=20 alt=30 duration=16 vsync=0`) with the far horizon on has 14-25 frames over
16.7 ms in one cluster ~12 s in, where the box streams a forest edge (36-48 MB mesh deltas in a frame); without it
0-2. Eleven isolation runs are in P13's README; the far layer's own work is small. Untried: make
`mesh_upload_budget_mb` (our godot_voxel patch `voxel_patch_budget.py`) actually cap streaming bursts, or throttle
tile building and uploads while the box is bursting (`VoxelEngine.get_stats()` tasks), or give the far view a
fixed low update rate only during bursts. Do not spend the whole session here: the look is the priority, but
re-run the flight after every shader or mesh change that adds cost and report it.

## How to work

- One look change at a time, each with a before / after crop at the same pose: seed 272905368,
  `autopilot=1 far=1 speed=0 alt=40 pitch=-0.05`, `alt=100 pitch=-0.35`, `walk=1 yaw=1.6 pitch=-0.05`
  (existing baselines `game/out/far/nolod_*`, `pose3`, `high3`, `seam3`). Also look at other biomes with
  `target=<biome>` (jungle, taiga, savanna / badlands for mesas, alpine for snow) and read the images: a spruce
  forest and a savanna are where blobs look worst. `tools/far_probe.gd` prints tile data headless.
- Judge by eye first (the developer judges), cost second: standing `walk=1 speed=0 duration=20` must hold p99 8.3 ms
  (now 7.2-7.6) and VRAM 6 GB; report numbers next to `game/out/nolod/stand_40.json`.
- `test_far_mesh` (grid, parent values at shared vertices, water, trees, canopy, nets) and `test_menus` must stay
  green; add a test for any new native data (crown meshes, persisted overrides). Full suite green before
  committing; rebuild the extension (`prototypes/p5-sim-spike`, MinGW, `game/README.md`) and re-import after a
  `class_name` change.
- Tunables already in place: `far_speckle`, `far_scale`, `far_threads`, `far_m`, `far_debug=flat|nocustom|noview|
  nodraw|freeze|halfrate|notrees`, `R0_M`, `MORPH_FROM`, `CANOPY_M` (`far_layer.gd`, `far_mesh.cpp`).
- Update ADR 0074 (its Measured table and Consequences), P13's README, `docs/plans/to-test.md` (the far horizon
  entry) and `game/README.md` when behaviour changes. Commits: imperative subject, the body says why; stage files
  explicitly, never `git add -A`; patch scripts go through the Write tool, not bash heredocs.

## Done looks like

- The developer, shown the three comparison shots again plus one jungle or taiga and one savanna or badlands shot,
  prefers the far horizon on over off; forests read as lumpy crowns, mid-distance ground has texture and shape,
  the horizon is visible to 1-1.5 km at noon, nothing pops at ring changes or at the box edge.
- Standing holds the budget; the flight either holds it or the remaining gap is measured and stated in the ADR.
- ADR 0074 moves from proposed to accepted only on the developer's word.
