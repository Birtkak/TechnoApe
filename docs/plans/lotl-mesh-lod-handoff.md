# Hand-off: a mesher and LOD system like Lay of the Land's

For a fresh session in this repo (`C:\Users\dev\Desktop\AI shizzle\TechnoApe`, read `CLAUDE.md` first). It is a
research-and-build job for the strongest model; expect native C++ work and days of iteration. Work in a branch;
the 40 m no-LOD world (ADR 0073) stays the shipped default until the new horizon beats it in play.

## The goal

TechnoApe is a 12.5 cm voxel world (ADR 0033) that takes its look from Lay of the Land (LotL, Steam app
2776090): flat per-voxel colours, no textures. Today the game draws **one 40 m box of full-detail voxels and
ends in the sky** (ADR 0073). Lay of the Land draws terrain to a very long distance, "in minute detail even at
altitude" (reviews), with voxel levels of detail and **seamless transitions** (the developer's devlog video
"Adding Seamless LOD Transitions to my Voxel Game", https://www.youtube.com/watch?v=WfxuCEyK16I). Build the
equivalent for us: distant terrain that looks like the near terrain, only smaller on screen, with no visible
pops, rings, terrace rims or black trenches, and that keeps up with 20 m/s flight. The developer's words
(2026-10-26): "unless the LOD can be made 5x better in a normal amount of work, hide distance another way", and
later "think Lay of the Land world manipulation / how does Lay of the Land handle far away distances?".

## What exists and what failed (read these before designing)

- `docs/decisions/0020-lod-policy-and-horizon.md`, `0072-short-horizon-in-haze.md`, `0073-one-box-no-lod-and-the-brush.md`
  and `prototypes/p11-lod-horizon/README.md`: three attempts with godot_voxel's `VoxelLodTerrain` +
  `VoxelMesherBlocky` (our own build of commit 045ff932 with the P11 patches). Measured results and screenshots
  (`game/out/horizon/`, `game/out/nolod/`, bug report 2026-10-26_19-36-29) show why they were dropped:
  1. **The blocky mesher at LOD n draws 2^n-voxel cubes.** A one-voxel water step becomes a full-height dark
     face (black trenches), beaches get terrace rims, hills become stacks of 1 m slabs, forests become cubes.
     The generator (`src/fine_gen.cpp`, `FineGen::generate_block(origin, size, lod, bpv)`) already produces
     coarse data per LOD (leaf domes at LOD 4, caves carved to LOD 2); the data is not the problem, the
     cubes-of-cubes look is.
  2. **Clipbox streaming hides the fine level under the player** ("a LOD only shows under an active parent, from
     the coarsest down"): edits were drawn by coarse meshes for seconds to minutes. Any LOD design must keep
     LOD 0 visible wherever it is meshed, immediately.
  3. **The 4x4 Bayer cross-fade and horizon dither read as a "stitching" pattern.** ADR 0073 replaced the
     horizon dither with a per-pixel blend into the sky's own colour (`sky_common.gdshaderinc`,
     `horizon_mix` / `horizon_sky` in `terrain_common.gdshaderinc`); keep that idea for transitions.
  4. **Streaming could not keep up in flight** at 8 LODs / 2 km; 4 LODs / 200 m loaded in 17.6 s.
- Budgets (ADR 0009, measured as ADR 0014 says, `autopilot=1 ... vsync=0`, JSON `budget_ok`): p99 frame
  <= 8.3 ms, no frame > 16.7 ms after load, RAM <= 4 GB, VRAM <= 6 GB on the dev box (RTX 3070). The 40 m box
  costs p99 6.2 ms standing and 0.9 GB VRAM; whatever you add sits on top of a 32-40 m full-detail box.
- Per-voxel work is C++ (principle 8): godot-cpp 4.5 at `~/Tools/godot-cpp`, MinGW GCC 16.2, SCons; the game's
  own extension is `prototypes/p5-sim-spike` (technosim), godot_voxel's source at `~/Tools/godot_voxel`
  (rebuild notes in `game/README.md`, "The voxel extension is our build").
- Shaders: `game/assets/shaders/terrain_common.gdshaderinc` (flat per-voxel colours from a palette tile by a
  hash of the voxel position, `far_fade` toward tile averages, the horizon blend), `sky_common.gdshaderinc`.
- Scripted eyes: `autopilot=1 speed=20 alt=30 duration=16` flights, `speed=0 alt=40 pitch=-0.05` far poses,
  `/shot <name>`, filmstrips (`capture_every=1`), `tp_test=1` for teleport readiness; never judge motion from
  one frame. `game/out/nolod/stand_40.json` and `fly_40.json` are the reference numbers to beat or match.

## Step 1: find out exactly what Lay of the Land does

Nothing in `docs/research/2026-10-24-lay-of-the-land-building-tools-fluids.md` covers distance. Before writing
code, watch (read the captions of) the LOD devlog above and any other LotL devlogs on the same channel
("Simulating Thousands of Voxels", "Meet the Biomes"), the Steam patch notes about render distance and the
handheld graphics settings, and screenshots at altitude. Answer, with sources, in a new
`docs/research/<date>-lay-of-the-land-distance.md`:
- How far is the full-detail radius, and how many levels follow? What is a far voxel's size?
- How does a far level look: cubes of cubes, a smoothed surface (dual contouring / marching cubes over the
  voxel grid with the voxel colours), a height-field skin, or impostors? Do far faces keep per-voxel colour
  noise or average to a flat colour?
- How are transitions hidden: geomorphing, dithered cross-fade, distance blend, skirts?
- How are water, steep cliffs, trees and buildings handled at distance (the four things our blocky LODs got
  wrong)?
- What does the far terrain do with player edits?
If the devlog does not say, infer from footage and say so.

## Step 2: design, as an ADR (next free number) with the measurements plan up front

Likely shape, to confirm against step 1:
- **Near: the 32-40 m full-detail box as today** (one `VoxelLodTerrain` with `lod_count` 1, or LOD 0 of a
  multi-LOD terrain if the hiding problem of point 2 is solved in `voxel_lod_terrain_update_clipbox_streaming.cpp`:
  show a meshed LOD 0 block as soon as it is meshed, hide its parent's part under it; a P11-style patch).
- **Far: a different mesher, not blocky cubes of cubes.** Candidates to spike in `prototypes/p13-far-mesh/`
  (one folder, README with question and measured answer):
  a. a *surface* mesher over the coarse voxel data (one quad per exposed face is what we have; instead a
     smoothed surface: dual contouring / surface nets over the LOD data, vertex colours from the voxel palette
     averages, no textures), so hills read as hills and water as a flat sheet;
  b. a *column height-field skin* from `FineGen`'s column query (`height_at`, `column_info`: height, top
     material, water level, tree cover) drawn as a clipmap of quads with vertex colours and a tree-dome
     instancing layer: cheap, looks right from 100 m, cannot show caves or overhangs, cannot show edits;
  c. godot_voxel's `VoxelMesherTransvoxel` on SDF data generated from FineGen's coarse levels (smooth LOD with
     built-in transition cells); needs an SDF channel from the generator.
  Measure each at the far pose and in flight; pick by look first (the developer judges screenshots), cost second.
- **Transitions**: per-pixel distance blend between the near box and the far layer in the terrain shaders
  (the `horizon_mix` band is where they meet), not a 4x4 ordered dither.
- **Edits**: the far layer reflects player edits within a frame or two (re-sample the column / coarse data of
  the edited block), or state plainly that it does not and why.
- **Streaming**: the far layer generates on worker threads, never on the main thread, and fills in coarse
  first; a 20 m/s flight never shows holes.

## Done looks like

- Screenshots at the ADR 0072 report pose (seed 272905368, `speed=0 alt=40 pitch=-0.05`) and at altitude
  100 m show terrain to 400 m+ that the developer prefers over the 40 m box, with no rings, rims, trenches or
  dither; mountains and forests read as such.
- `autopilot=1 speed=20 alt=30 duration=16 vsync=0` and `walk=1 speed=0 duration=20` pass `budget_ok`
  with no other Godot process; first load under 10 s; numbers in the ADR table next to the 40 m box's.
- A placed 4 m cube and a dug crater are visible from 150 m.
- gdUnit tests for the far mesher's data path (headless), `test_menus` for any new settings, the full suite green.
- `docs/design/vision.md` section 6, `docs/design/systems.md`, `game/README.md`, `CLAUDE.md` (the one-line
  world description) updated; `docs/plans/to-test.md` gets a playtest entry.
- Commits: imperative subject, body says why; never `git add -A`.
