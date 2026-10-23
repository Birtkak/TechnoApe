# 0033 – A Lay of the Land world: one 12.5 cm voxel grid, flat colours, brushes

- **Status:** proposed 2026-10-23. The direction (scale, look, interaction) is
  the user's call and is settled; the architecture below is gated by the P12
  spike (`docs/plans/lotl-overhaul.md`) before any of it replaces the 0.5 m game.
- **Date:** 2026-10-23

## Context
The user: "I want the world to be like Lay of the Land. Fully voxel gameplay.
So this is a whole generator/graphics overhaul", then "this will work great
with our Noita simulated world", and that the target is its vibe rather than
its exact technology.

Lay of the Land (Southern Cross Interactive, solo developer, Unreal Engine 5,
1.0 on 2026-04-08): every voxel is 10 cm (the developer, on the Steam forum),
the world is fully destructible, fire spreads, sand collapses, water flows,
gas suffocates, trees are cut voxel by voxel and fall, cave roofs can be
dropped on enemies. Terrain comes from layered simulations (water carves
valleys, roads route between places). Building uses voxel shapes (cylinder,
cone, slopes), terrain sculpting and path drawing. Crafting happens in the
world (tools assembled on the ground, cooking on a campfire, smelting, clay
molds, casting), which our Flint Age already does (ADR 0023, 0030). The look
is flat-coloured voxels with soft lighting; the recommended spec is an RTX
3060 Ti with hardware ray tracing at 1080p 60 fps.

Today the gameplay grid is 0.5 m with 16x16 Minecraft textures (ADR 0004).
Fine detail exists only as a second grid: 6.25 cm cells in bricks (ADR 0012),
ray-cast within 32 m (ADR 0021), simulated in a 32 x 16 x 32 m window
(ADR 0029), with the 0.5 m world reading each brick's majority. Both flags are
off by default. Mining, placing, collision, the far field and saves are 0.5 m.

## Options considered (the user chose; recorded for the why)
- **Voxel size:** 12.5 cm (chosen: the power of two nearest 10 cm, so LOD,
  chunking and generator math stay clean, and the micro window's cell budget
  covers 64 m instead of 32 m); 10 cm exactly (0.5 m becomes 5 voxels, about
  twice the cells); 6.25 cm (finer than Lay of the Land, 8x the cells, half
  the reach); 25 cm (cheapest, chunkier).
- **Look:** flat per-voxel colours (chosen); a pixel texture spread across many
  voxels; flat colours plus generated per-material patterns.
- **Interaction:** brushes and material amounts (chosen); 0.5 m blocks mined
  and placed on a fine grid; both, per tool.

Architecture (technical, decided here, confirmed or reversed by P12):
1. **One grid.** The 12.5 cm voxel is the world: gameplay, simulation,
   collision, saves and rendering all use it. godot_voxel meshes it with LOD
   (LOD k = 12.5 cm x 2^k; a 2 km horizon is LOD 0..10).
2. Keep ADR 0012's two grids with a 12.5 cm cell: a 0.5 m world of bricks
   with 4³ cells. Keeps two views of every voxel (the majority problem, ADR
   0029's walls between windows) for no gain once the voxel itself is small.
3. A custom renderer for everything (Teardown style). ADR 0012 already
   rejected it as a year of work.

## Decision
- **Scale (user):** one voxel = 12.5 cm everywhere. Tunables stay in metres
  (`Settings.VOXEL_M` becomes 0.125). World 384 m tall = 3072 voxels; sea
  level stays at 128 m. Player 1.8 m = 14.4 voxels, auto step-up 0.5 m =
  4 voxels. Chunk and mesh-block sizes in voxels are retuned by P12.
- **Look (user):** each material is a small colour palette in data; a voxel's
  colour is picked in the shader from a hash of its world position (no data
  cost, stable across LODs and edits). Soft light does the rest: baked voxel
  AO from the mesher, sky light, fog, a warm grade. ADR 0004's texture atlas
  goes for terrain; item sprites (16x16 icons) stay.
- **Interaction (user):** tools dig a blob whose size and shape follow the
  tool and its tier; the inventory keeps Minecraft slots and stacks, but a
  stack of terrain is an amount of a material (dirt 340). Placing uses shapes
  (cube, cylinder, cone, slope, path), Lay of the Land style. Flint Age
  in-world crafting (ADR 0023, 0030) is kept and re-expressed at 12.5 cm.
- **Architecture:** option 1, one grid. P12 measures two ways to draw the
  near ground: (a) meshed all the way in, (b) the ADR 0021 ray caster over
  a 512³ window at 12.5 cm (64 m, the same cost P7 measured for 32 m at
  6.25 cm) with meshes from LOD 1 out. The ADR 0009 budget picks.
- **The simulation ticks the voxels.** The bubble's cell is the voxel; ADR
  0029's window, majority publishing and held chunks go away. P10's v6
  kernel conditions (background tick, distance LOD, cell budget, static
  ground that loosens, liquid fullness) carry over unchanged.
- **Structural collapse** is new: a detached voxel island (a cut trunk, an
  undermined cave roof) becomes a falling rigid body of voxels and lands as
  voxels again. Its own ADR when it is built.
- **Migration:** the new world runs behind `world_v2=1` until it beats the
  old one in play and in the budget; then the 0.5 m path, the brick store's
  majority view and the textures are deleted. Old saves do not load
  (`GEN_VERSION` bump, new save name).

Superseded when the flag goes default: ADR 0004 (scale and look), ADR 0012
and 0029 (two grids; their kernel conditions stay), ADR 0006's 6.25 cm chisel
grid (chiseling is editing 12.5 cm voxels; there is nothing finer), the mining
and placing parts of ADR 0005. ADR 0021's caster survives only if P12 picks
variant (b).

## Consequences
- Easier: one data model, no majority view, no walls between simulations,
  destruction and simulation everywhere the bubble reaches, every material
  change visible as itself. Texture art for terrain disappears; materials
  become palettes in data.
- Harder: 64x the voxels per cubic metre of today, 16x the faces per square
  metre of surface. Memory and meshing scale with the LOD 0 radius in metres,
  so that radius is what P12 tunes. The generator must produce shapes at
  12.5 cm (smooth slopes, trees, rocks) instead of 0.5 m blocks. Every 0.5 m
  model (stations, furniture, animals, micro detail at 6.25 cm) is redrawn at
  12.5 cm, losing half the resolution of today's micro models.
- Revisit: if P12 cannot hold the budget with a useful LOD 0 radius, 25 cm
  near-LOD with 12.5 cm only inside the bubble is the fallback, decided with
  the user (it changes the look).
