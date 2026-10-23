# 0004 – World scale (0.5 m voxels) and first look (16×16 textures)

- **Superseded** by ADR 0033 (12.5 cm voxels, flat colours); removed with ADR 0036.
- **Status:** accepted (2026-10-21)
- **Date:** 2026-10-21

## Context
The developer chose **0.5 m voxels** over the Minecraft 1 m baseline, for a finer,
more Noita-like feel of the material simulation, and **Minecraft-style
16×16 textures** from the first visible build.

## Decision
- One voxel = 0.5 m. A player is 1.8 m = ~3.6 voxels tall (collision box
  4 voxels high, 2 wide). Reach, jump height and block-placement rules are
  defined in metres and converted, so a later scale change does not ripple
  through gameplay code.
- World height: 768 voxels (384 m; sea level at voxel 256, mountains to
  ~250 m above sea level, caves 128 m deep). Chunks stay 16³ voxels (8 m).
- Textures: a 16×16-pixel-per-tile atlas, nearest-neighbour filtering, one
  tile per material face via godot_voxel's `VoxelBlockyModelCube`
  (`atlas_size_in_tiles`, per-side tiles). Placeholder tiles are generated
  procedurally until real art exists.

## Consequences
- **8× the voxels per cubic metre** of a 1 m world. Everything per-voxel is
  8× more expensive for the same metres: generation, meshing, simulation,
  memory, saves, network deltas. P2/P5 margins absorb this on the dev PC
  (2 M-voxel active region ticks in <2 ms). Budgets are set in *metres*
  (bubble radius, render distance); Android was dropped (ADR 0009), so they
  are tuned against the dev PC.
- Render distance in chunks doubles for the same metres: 256 m view =
  32 chunks of full detail; the LOD horizon (clipbox, 7 LODs ≈ 1 km of
  voxels = 2 km of... no: 1024 voxels = 512 m) needs one more LOD level to
  reach the same metres. Test 8 LODs in the skeleton.
- Greedy meshing becomes worth doing early (4× more faces per m²).
- Column storage must be compact: 768-voxel columns, run-length or
  palette-compressed chunks on disk and in the far store.
- Texture atlas limits: 16×16 tiles per atlas in godot_voxel's cube model
  = 256 tiles; more materials than that need a second library/material.
