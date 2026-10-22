# 0011 – Near-field rendering: keep meshing, or ray-cast the bubble

- **Status:** proposed (spike P7 measured 2026-10-22; direction call open)
- **Date:** 2026-10-22

## Context
The world is meshed: godot_voxel builds meshes and colliders per 32³ block for
0.5 m voxels at all LODs, and micro-voxel detail (vegetation, the ADR 0006
chisel grid at 6.25 cm) is meshed by `MicroMesher`. Burkelbear Games'
Grimorium shows the other model: one dense micro-voxel grid, every cell
simulated, rendered by GPU ray casting, no meshes, any edit visible the same
frame. Two requirements make that attractive here: "every voxel chiselable"
(ADR 0006) and "a black-hole spell or a thousand TNT must not choke" (asked
2026-10-22). Both are expensive on the meshed path: remeshing and collider
rebuilds scale with the number of touched blocks and run partly on the main
thread (ADR 0010 measured p99 11.7 ms in a forest fire from colliders alone).

Fixed constraints: 0.5 m blocks for Minecraft-style building (ADR 0004),
2 km LOD horizons, Godot 4.7 + godot_voxel, the native `technosim` sim with a
24 m bubble, reference PC RTX 3070 with p99 <= 8.3 ms (ADR 0009).

## Options considered
1. **Keep meshing everything.** Known, works, LOD and physics for free.
   Mass edits stay bursty: remesh N blocks + colliders; micro detail costs
   triangles. Mitigations exist (worker-thread remesh, voxel-grid collision
   instead of mesh colliders, edit budgets per frame) but the floor is the
   mesher.
2. **Ray-cast the near field at 6.25 cm** (P7): a 32 m cube (512³, 134 M
   cells, 1 byte each) around the player ray-cast by a two-level DDA on the
   volume's bounding box, depth-composited over the meshed world beyond it.
   Measured: 3.3 ms p50 / 6.1 ms p99 static, 5.8 / 7.4 ms with 16 MB of sim
   data uploaded per frame, 236-300 MB VRAM. Edits cost bytes changed at
   ~3 GB/s (32 MB ≈ 10 ms, the whole cube 128 MB ≈ 45 ms stall), so dirty
   tracking per slice/brick is mandatory. Needs: the simulation at micro
   resolution (512× today's cells per metre) or a mixed grid (0.5 m sim cells
   with 6.25 cm detail only where chiseled/decorated), lighting consistency
   between the two renderers, collision from the voxel grid, water/glass
   transparency in the caster, a C++ edit path (GDScript carving of a sphere
   took 55 ms).
3. **Ray-cast the near field at 0.5 m** (same shader, 64³ for 32 m): trivial
   cost but no visual gain over meshing except instant edits; micro detail
   would still be meshed. A stepping stone, not a destination.
4. **Whole world ray-cast** (Grimorium proper): impossible at 2 km horizons
   (10^13 cells at 6 cm; even 0.5 m everywhere is 10^10) and incompatible
   with godot_voxel LOD. Rejected.

## Decision (proposed)
Rendering is no longer the blocker for a Grimorium-style near field; the
simulation is. Before choosing 2, run the matching simulation spike: can
`SimWorld` (or a sparse-active-set rewrite of it) tick a 32 m bubble at 6.25
cm, or at 12.5 cm, at 20-30 Hz on the reference PC, and what does a 1000-TNT
frame cost in the kernel? If yes at either resolution, adopt option 2 for the
bubble with a 0.5 m meshed world beyond it, chisel detail rendered by the
caster, and dirty-brick uploads capped per frame. If no, stay with option 1
plus the ADR 0010 mitigations and per-frame edit budgets, and keep option 3
in reserve for instant-edit feel.

## Consequences
- Either way, mass edits get an explicit budget: bytes or blocks per frame,
  queued, never a single-frame stall. This is a requirement from now on for
  any edit path (spells, TNT, the sim write-back).
- Option 2 adds a second renderer to keep consistent with Godot's lighting
  (sun, shadows, fog, SSAO) and a voxel-grid collision path; P7 code is the
  starting point.
- The sim spike decides the voxel size question that ADR 0004 left at 0.5 m:
  micro-resolution simulation would be the real "3D Noita" step.
