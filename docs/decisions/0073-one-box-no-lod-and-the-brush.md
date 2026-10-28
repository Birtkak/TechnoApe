# ADR 0073: One box of voxels and no LOD; the brush flush with the face; placed powder falls

Status: accepted (2026-10-26)

## Context
Four bug reports on 2026-10-26 (20-24-11 "many visual bugs", 20-25-14 "placed sand not simulated",
20-26-24 "building ghosts show up wrong or not and places wrong", 20-26-39 "destroying is wrong too")
and the developer's call: "Building and mining/destroying is completely broken now and LOD too. Remove
the entire LOD system and make world manipulation great. Placing was off centre, boxes did not show up
or showed the wrong size and it was not clear from the menu what would do what. Think Lay of the Land
world manipulation. Far away voxels got this weird stitching pattern, likely due to the fog."

What the screenshots show: the ground next to the player drawn at 25 cm or 50 cm cubes while the
block they just placed is 12.5 cm voxels; the ghost shell half sunk in that coarse ground; a dug hole
whose inside is the coarse mesh's dark faces. ADR 0072's four LODs with 32 m rings kept the detail
under the player hidden: clipbox shows a LOD only under an active parent, from the coarsest down, so
LOD 0 stayed invisible for seconds to minutes while every edit went into LOD 0 data and was drawn by
a mesh it never touched. The ordered 4x4 dither of the horizon dissolve was the "stitching".

Lay of the Land itself draws far terrain with voxel LODs and seamless transitions (its devlog "Adding
Seamless LOD Transitions to my Voxel Game"); it is a smoother-looking mesher than our blocky one, and
its transitions are what ADR 0020 and 0072 tried and failed to get right at 12.5 cm. Its tools are
what the brush already copies: normal / box / cube dig modes, shapes with gizmos, box mode by two
clicks, prefabs.

## Decision
1. **No LOD. The drawn world is one clipbox of full-detail voxels** (`VoxelLodTerrain`, `lod_count`
   1, `lod_distance` = `view_distance` = `Settings.view_distance_m`), and the horizon is its edge.
   Measured at seed 272905368, 1280x720, vsync off, ADR 0014 method, no other Godot process:

   | box radius | first load | standing 20 s: p99 / max | 20 m/s flight 16 s: p99 / max | RSS / VRAM |
   |---|---|---|---|---|
   | 32 m | 2.1 s | 4.6 / 5.5 ms | 4.0 / 4.8 ms | 2.1 GB / 0.6 GB |
   | **40 m (default)** | 4.2 s | 6.2 / 8.8 ms | 5.2 / 5.7 ms | 2.3 GB / 0.9 GB |
   | 48 m | 6.9 s | 31.6 / 47 ms, 114 frames > 16.7 | 7.1 / 10.8 ms | 2.6 GB / 1.3 GB |

   40 m holds ADR 0009's budget; 48 m does not. The menu offers 24, 32 and 40 m (presets low /
   medium / high); `MAX_VIEW_M` is 40. First load fell from 17.6 s (ADR 0072) to 4 s. Gone with the
   rings: `render_distance_m`, `lod_count`, `lod_ring_m`, `lod_vertical_ratio`, `lod_fade_s`, the
   `lods_for_view` derivation, the LOD cross-fade (`u_lod_fade`, `lod_fade_discard`), the "Full
   detail to" menu row, `near_ready`'s per-LOD search. godot_voxel's LOD code and our P11 patches
   stay in the DLL (the fluid, remesh, upload-budget, stream and box-mover patches are still used).
2. **The world ends in the sky, not in a dither.** `terrain_common.gdshaderinc` no longer discards
   pixels on a Bayer pattern. From `horizon_start` (75 % of the view distance) each terrain pixel
   blends its lit colour out and the sky's own colour in (`horizon_sky`: the gradient
   `sky.gdshader` draws, now in `sky_common.gdshaderinc`, evaluated for the pixel's view direction),
   and is gone at `horizon_end`. The see-through liquids and gases fade their alpha over the same
   band. `Atmosphere` sets the sky uniforms (zenith, horizon, ground = haze colour, sunset, sun
   direction, eclipse) on every terrain material (`World.horizon_materials`), so the blend tracks
   time of day and grade. `far_fade` (textures toward their tile average) now runs 28-40 m so the
   colour is flat before it blends.
3. **The brush is flush with the face and centred on the aim** (`Brush`, "placing was off centre").
   The box shapes (cube, flat, column) sit against the aimed face, into the material when digging
   and out of it when placing, centred on the aim across the face and snapped there to the 0.5 m
   block grid from size 4 up (so placed blocks tile without gaps; sizes 1-2 snap to their own size).
   Before, every box snapped all three axes to its own size grid: a 1 m cube could land a metre
   from the aim, half of it in the air. Round shapes are unchanged (around the aimed voxel; resting
   on the face when placed). `Brush.place_bounds` gives the placement outline; `Brush.explain` says
   in words what a dig or a placement does ("a 1 m cube into the aimed face").
4. **The ghost is exact at every size.** The box shapes' shell is one unit shell scaled to the box
   (no half-resolution shell for big boxes); the outline comes from the same bounds as the cells.
   The round big brushes keep the half-resolution shell (ADR 0065). With one LOD the shell is
   depth-tested against the mesh the edit will change, so it no longer sinks into coarse ground. A
   dig's shell and outline draw through the terrain (no depth test): a cube flush with the aimed
   face lies inside the ground and was otherwise invisible. The newest dig and placement (aim, face,
   box, cells) are recorded in the brush stats of the run JSON and of bug reports (`last_dig`,
   `last_place`).
5. **The world tools panel says what the clicks do** (`ToolsMenu`, "not clear from the menu what
   would do what"). Two "Now" lines at the top: "left click: dig a 1 m cube into the aimed face",
   "right click: place a 1 m cube of Sand standing on the aimed face" (or "nothing to place: hold a
   block"). The selected shape's and size's description stands under their rows; every build mode
   has its description beside its button, not only in a tooltip; the key list names both clicks.
   Prefabs get their own row only when there are any.
6. **Placed powder keeps its shape on ground and falls in the air** ("placed sand not simulated").
   ADR 0071 placed powders as their still kind so a built cube stops sliding apart; a still cube set
   down in the air then floated. `SimWorld::try_loosen` now also loosens a still powder whose own
   cell was just placed or changed (the edit stamp) and has an opening straight below: Minecraft's
   placed sand falls, Noita's static ground stays until shaken. Test:
   `test_sim_v6.test_placed_still_sand_keeps_its_shape_on_ground_and_falls_in_the_air`.

Defaults taken without asking: the 40 m default (the budget's edge), a cube that goes *into* the
aimed face when digging (Lay of the Land's cube mode) rather than around the aimed voxel, the 0.5 m
grid for centring, the panel's wording.

## Consequences
- The horizon is 40 m. Distant mountains, far builds and the view from a high flight are gone until
  a far layer that looks right exists (a painted heightmap horizon from the generator's column query,
  or impostors); ADR 0072's dissolve band is where it would join. Flying high, the ground below
  dissolves at 30-40 m.
- One box: everything drawn is editable and simulated at full detail, first load is 4 s, and an edit
  is drawn by the mesh of the data it changed.
- Old `settings.cfg` files with 200 m or more are clamped to 40 m.
- Tests: `test_brush` (the new box rules, `explain`), `test_tools_menu`, `test_menus` (presets by
  view distance), `test_sim_v6` (placed sand), `test_lighting_grades`. Checks: `out/nolod/`
  (the measurement runs and the look at seed 272905368; the dig / place run at the report's seed
  1078288609 and spot).
- The sim extension is rebuilt (`prototypes/p5-sim-spike`, `SimWorld::try_loosen`).

## Amendment 2026-10-27: view distances past the budget
The developer asked for view distances up to 100 m. The menu now offers 24, 32, 40, 56, 72 and 100 m
(`Settings.VIEW_CHOICES_M`, `MAX_VIEW_M` 100); the default and the high preset stay 40 m. Measured on the
dev box (autopilot, seed 1337, vsync off): 40 m p99 2.7 ms, max 9 ms, budget OK; 100 m p99 7.9 ms but
80 frames over 16.7 ms (worst 238 ms) and 4.7 GB RSS, so past 40 m the budget does not hold.
