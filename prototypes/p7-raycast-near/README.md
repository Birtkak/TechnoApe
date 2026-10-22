# P7 – GPU ray-cast micro-voxel near field (Grimorium-style) inside Godot

**Question:** can a dense micro-voxel volume around the player (6.25 cm cells,
the ADR 0006 chisel grid) be ray-cast on the GPU inside Godot 4.7, composited
with the meshed terrain, within the frame budget (p99 <= 8.3 ms at 1080p on
the reference RTX 3070), and can it absorb mass edits (a black-hole spell, a
thousand TNT) without choking? Prompted by Burkelbear Games' "How graphics
work in my 3D Noita-like game, Grimorium" (custom engine, every voxel
simulated, rendered by ray casting, no meshes).

**Setup (`main.gd`, `raycast.gdshader`, `gen_volume.glsl`, `occ.glsl`):**
- The volume is an R8 2D-array texture on the RenderingDevice (one layer per z
  slice), filled by a compute shader with ground, caves, water and glowing ore.
- A second compute pass writes an 8³-brick occupancy texture.
- The ray caster is a spatial shader on the volume's bounding box (front faces
  culled, works from inside): two-level DDA (brick stride through empty
  bricks, cell steps inside occupied ones), one lambert term, one shadow ray
  towards the sun, and it writes `DEPTH`, so ordinary meshes (red/yellow boxes,
  a ground plane, a DirectionalLight3D with shadows) composite correctly.
- Per frame the CPU uploads `dirty` z slices (`texture_update`, what the
  material simulation would push) and the occupancy pass reruns over them.
- `burst=N` re-uploads the whole volume every N frames, with (`carve=1`) or
  without a CPU-side sphere carved into it: the "everything changed" frame.

```
Godot_..._console.exe --path prototypes/p7-raycast-near -- size=512 dirty=64 burst=0 duration=8 out=res://out
```
One JSON line on stdout; screenshots in `out/` (two kept in `shots/`).

**Measured 2026-10-22, 1920x1080, RTX 3070, Forward+, vsync off (`out/results.jsonl`):**

| volume | extent | per-frame upload | frame p50 | p95 | p99 | upload CPU ms | VRAM |
|---|---|---|---|---|---|---|---|
| 256³ (16.8 M cells) | 16 m | none | 2.8 | 3.0 | 3.0 | – | 124 MB |
| 256³ | 16 m | 64 slices = 4 MB | 3.3 | 3.5 | 3.7 | 0.35 | 132 MB |
| 512³ (134 M cells) | 32 m | none | 3.3 | 5.8 | 6.1 | – | 236 MB |
| 512³ | 32 m | 64 slices = 16 MB | 5.8 | 7.4 | 7.4 | 1.6 / 2.7 | 268 MB |
| 512³ | 32 m | 128 slices = 32 MB | 7.6 | 9.1 | 10.0 | 5.3 / 6.6 | 301 MB |

Without the brick occupancy (naive cell DDA, first run) the 512³ case was
12-14 ms p50 and 24-30 ms p99: the brick skip is what makes it fit.

Mass edits, whole volume re-uploaded in one frame (`burst=60`):

| volume | bytes | CPU ms (upload only) | frame that shows it | with a GDScript sphere carve |
|---|---|---|---|---|
| 256³ | 16 MB | 2.0 / max 2.8 | 3.2 ms | 9-14 ms (carve dominates) |
| 512³ | 128 MB | 41 / max 43 | 49 ms | 95-98 ms, frame 105 ms |

Uploads run at roughly 3 GB/s through `texture_update`, one call per z slice.

**Answer:**
- **Yes for rendering.** A 32 m micro-voxel cube at 6.25 cm ray-casts in 3.3 ms
  median, 6 ms p99, with correct depth compositing and sun shadows, leaving
  room for the meshed world. The 16 m cube is almost free (3 ms). VRAM is
  modest (130-300 MB) because the volume is one byte per cell.
- **Yes for mass edits, with dirty tracking.** The cost is linear in bytes
  changed: a 1000-TNT crater that rewrites a quarter of the 32 m volume
  (32 MB) is about 10 ms of upload, which can be split over two frames; a
  black hole eating a 5 m sphere is under 1 MB and invisible in the frame
  time. Re-uploading the entire 32 m volume (128 MB) in one frame stalls for
  ~45 ms, so edits must be tracked per slice or brick and never pushed whole.
  Today's meshed path cannot make the same promise: large edits remesh dozens
  of mesh blocks and rebuild colliders on the main thread (the player-loop
  session measured p99 11.7 ms from colliders alone in a forest fire).
- **The real question is the simulation, not the renderer.** A ray-cast near
  field only shows what the simulation produces. At 6.25 cm the bubble holds
  512 times the cells of today's 0.5 m bubble; whether the native sim can tick
  134 M cells (or a sparse active set of them) at 20-30 Hz is a separate spike
  on the SimWorld kernel. Rendering is no longer the blocker.
- Limits of this spike: no LOD inside the volume, no transparency (water is
  opaque), one bounce of lighting, no collision (physics still needs meshes or
  a voxel mover), CPU-side carving in GDScript is slow and would be C++.

**Decision pending:** ADR 0011 (`docs/decisions/0011-near-field-raycast.md`).
