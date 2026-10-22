# 0011 – Bodies collide against the voxel grid, not terrain mesh colliders

- **Status:** accepted
- **Date:** 2026-10-22
- **Follows:** ADR 0010 (where the frame spikes came from), ADR 0009 (budgets)

## Context
ADR 0010 traced the remaining frame spikes to godot_voxel building a physics
collider on the main thread for every re-meshed terrain block. Any churn
(fire, flowing water, digging) and any streaming re-meshes blocks, so the
cost scales with activity, not with what the player touches. The player
(CharacterBody3D + capsule) and dropped items (RigidBody3D) were the only
users of those colliders; mining, placing and the third-person camera
already ray-cast the voxel data.

## Decision
- **VoxelBody** (`scripts/physics/voxel_body.gd`) moves an axis-aligned box
  through the voxel grid with godot_voxel's `VoxelBoxMover` (Minecraft-style
  AABB moves against the blocky models' collision boxes), reporting floor,
  ceiling and per-axis blocking. Step climbing (one voxel) is built in.
- The **player** is a Node3D with its own velocity: walk, sprint, jump,
  gravity, step-up and fly through VoxelBody; it stands still while the chunk
  it is in, or the one below, has not streamed in (unloaded voxels read as
  air). `is_on_floor()` keeps its meaning for the body animation.
- **Dropped items** are Node3Ds moved by VoxelBody with gravity, vanilla drag
  (air 0.98, ground 0.6 × 0.98 per 20 Hz tick), a small bounce, and sleep at
  rest (re-checked 4 times a second, so mining the block below drops them).
- **Collision masks** in the blocky library: solids and powders block
  (mask 1); liquids, gases and fire do not (mask 0, no mesh collision). The
  player swims: sinks at 1 m/s, rises at 2 m/s holding jump, half speed;
  water breaks a fall. Micro-voxel plants keep no collision boxes.
- **Terrain mesh colliders are off** (`Settings.terrain_mesh_collisions`,
  default false); the VoxelViewer no longer requests collisions.

## Measurements (reference PC, vsync off)

| Run | Collision | avg | p95 | p99 | worst | RAM |
|---|---|---|---|---|---|---|
| Forest fire, 574 voxels lit | mesh colliders | 2.47 ms | 6.06 ms | 12.12 ms | 18 ms | 624 MB |
| Forest fire, 574 voxels lit | voxel grid | 1.99 ms | 2.78 ms | 3.03 ms | 16.7 ms | 285 MB |
| Flight 20 m/s, 40 s | mesh colliders | 4.94 ms | 16.18 ms | 21.91 ms | 45 ms | 408 MB |
| Flight 20 m/s, 40 s | voxel grid | 1.17 ms | 1.85 ms | 2.08 ms | 6.9 ms | 225 MB |

Walk (65 m over terrain), 2x2 shaft mining with fall damage and item pickup,
swimming out of a lake and items resting on grass all behave as before
(autopilot `walk=1`, `mine=1`, `target=lake`, `scatter_items=1`).

## Consequences
- Edits block movement the same frame the voxel is written; no collider lag,
  so the "unstick" code is only a safety net for voxels simulated into a body.
- Anything new that needs physics against terrain (mobs, projectiles,
  vehicles) uses VoxelBody or voxel ray-casts, not PhysicsServer queries.
- One isolated 16.7 ms frame remains in the fire run (godot_voxel mesh
  uploads); it is at the budget line and is the next thing to watch.
- Swimming is minimal (no breath, no water physics on items yet).
