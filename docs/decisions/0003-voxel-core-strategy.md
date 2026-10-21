# 0003 – Voxel core strategy: godot_voxel for rendering/LOD, native simulation

- **Status:** **accepted** 2026-10-21 after P5
  (`prototypes/p5-sim-spike/README.md`): native kernel ticks a 2 M-voxel
  region with 400 k moving cells in 1.7 ms on 12 threads; pushing changed
  chunks into godot_voxel costs ~0.11 ms per changed chunk of frame time;
  GDScript is ~90× slower per thread. Option 3 (hybrid) confirmed.
- **Date:** 2026-10-21

## Context
The game needs (a) blocky voxels, (b) Noita-style material simulation near
players, (c) horizons of 1–2 km with player builds visible, (d) a headless
server and an Android client. P2 measured godot_voxel and language options;
see `prototypes/p2-voxel-benchmark/README.md` and
`docs/research/2026-10-21-far-view-vs-near-simulation.md`.

## Options considered
1. **godot_voxel for everything** (data, edits via VoxelTool, LOD). Mature
   rendering/LOD/streaming. But its data model is tuned for static worlds
   with occasional edits; a cellular automaton touching thousands of voxels
   per tick through its script API would be slow, and its internals are not
   callable from a separate GDExtension.
2. **Own voxel core in GDExtension (C++)** for data + simulation + meshing +
   LOD. Full control, but LOD streaming alone is years of work that
   godot_voxel already did.
3. **Hybrid: own native simulation store, godot_voxel as the renderer.**
   The simulation owns authoritative chunk data and pushes changed blocks
   into VoxelTerrain / VoxelLodTerrain through block-level writes. godot_voxel
   handles meshing, LOD rings (clipbox), collisions, instancing.
4. **Fork godot_voxel and add the simulation inside it.** Deepest
   integration, but we inherit a large C++ codebase to maintain and
   upstream drift.

## Decision (proposed)
Option 3, hybrid. P5 must prove two numbers: simulation throughput (active
voxels per ms, C++ threads vs compute shader) and the cost of pushing changed
blocks into godot_voxel each tick (re-mesh latency, main-thread time). If the
push path is too slow, fall back to option 4.

Fixed regardless of P5:
- `VoxelLodTerrain` + `VoxelMesherBlocky` + `streaming_system = CLIPBOX`.
- Hot loops in C++ (GDExtension). GDScript orchestrates. C# not used in the
  shipped game (Android export experimental).
- Chunk flags `active / dirty_mesh / dirty_lod / edited`; simulation bubbles
  around players and anchors; settle-on-unload.

## Consequences
- Simulation ticks at a fixed rate (Minecraft-style; 60 Hz tested, 20–30 Hz
  likely for shipping) decoupled from rendering; changed chunks are pushed
  once per tick. Frame cost ≈ 0.11 ms × changed chunks, so bubble size and
  tick rate are the two budget knobs.
- Push changed chunks through **`VoxelTool.paste`**, never
  `try_set_block_data`: the latter swaps buffers and godot_voxel applies
  mesh results out of order under load, leaving stale geometry (P5 §C).
  If godot_voxel is forked later, add a per-block mesh version check.
- Per-chunk activity flags are accumulated locally and flushed once per
  chunk per tick (false sharing cost 4.5× in P5 v1).
- Toolchain on the dev box: portable MinGW-w64 GCC 16.2 (no admin needed)
  + SCons + godot-cpp branch 4.5. MSVC optional later (needs an admin prompt
  at the PC).
- CI must build the extension for Windows x64, Linux x64 (server) and
  Android arm64.
- godot_voxel pinned to a tagged release (or our own CI build) before Phase 2.
