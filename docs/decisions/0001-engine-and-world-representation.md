# 0001 – Engine and world representation

- **Status:** **accepted** 2026-10-21 after P1 (headless loop works: gdUnit4,
  screenshot, C# build all from CLI) and P2 (godot_voxel streams a 2 km LOD
  world at <1 ms/frame; see `prototypes/p2-voxel-benchmark/README.md`).
  Language consequence refined in ADR 0002/0003: GDScript + C++, not C#.
- **Date:** 2026-10-21

## Context
Solo developer, AI-assisted workflow. Needs: text-based project files the AI
can edit, headless verification, strong procgen/terrain tooling, enough
performance for streamed open worlds, long-term stability. See
`docs/research/2026-10-21-foundation-research.md`.

## Options considered
1. **Godot 4.7** – best AI fit (text scenes, CLI, MCP, tests); perf needs
   native code for hot loops; Terrain3D / godot_voxel for terrain.
2. **Unity 6** – strongest C# corpus and ecosystem; scenes are AI-opaque;
   heavier headless loop; licensing history.
3. **Bevy** – clean ECS, but API churn and thin corpus hurt AI accuracy.
4. **Web (TS + Three.js)** – fastest iteration, weakest for 3D open world.
5. **Unreal / custom engine** – rejected: AI cannot edit Blueprints; custom
   engine is years of plumbing.

## Decision (2026-10-21)
- **Engine:** Godot 4.7, prototyped alone (no Unity side-by-side). Becomes
  *accepted* once P1 (headless loop) and P2 (voxel chunk streaming benchmark)
  pass. Unity (P4) only if P2 fails.
- **World representation:** blocky voxels. Candidate tech: godot_voxel
  (VoxelTerrain + VoxelMesherBlocky) vs a custom chunk mesher; P2 compares.
- **Perspective:** 3D first-person.
- **Multiplayer:** single-player now, co-op later. Architecture rule from day
  one: simulation (world state, inventory, crafting) is separate from
  presentation, deterministic from seed, and could run on a host.

## Consequences
- GDScript or C# for glue; C# or GDExtension for generation/meshing hot loops.
  P2 decides which.
- Crafting/inventory core written engine-agnostic and unit-tested.
- No direct node-to-node game-state mutation from UI/input; everything goes
  through the simulation layer so a network boundary can be inserted later.
- Save format must be chunk-based and versioned.
