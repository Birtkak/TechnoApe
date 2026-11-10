# Lighting spike: hardware ray queries on the planet's terrain

**Question.** The developer wants ray-traced light, for three reasons:
- the sun as the only source;
- moonlight as reflected sunlight, with eclipses as real blockage;
- reflections on metal, and later telescopes.

Can wgpu 30 trace that on the dev PC (RTX 3070) within ADR 0004's 8.3 ms frame?

**Setup.** `src/main.rs` builds level 0 terrain chunks around the 4 km planet's spawn with the engine's
`generate` and `mesh`. Each greedy quad becomes two world-space triangles, one BLAS per chunk, all under one
TLAS. `src/trace.wgsl` is a compute pass at 1920×1080. For every pixel it casts:
- a camera ray (closest hit);
- a soft-shadow ray toward a random point of the sun's 0.53° disc;
- in mode 1, a reflection ray on *every* pixel (the worst case: everything metal) with its own shadow ray;
- in mode 2, 4 shadow samples instead of 1.

Run: `cargo run --release -- r=8 mode=1 out=shot.png` (r = chunk radius around the spawn).

**Backends.** Vulkan offers `EXPERIMENTAL_RAY_QUERY`, vertex return and RT pipelines. wgpu's DX12 backend offers
none of them, so the client stays on Vulkan (its default).

**Measured** (wall time over 50 frames in one submit, release build):

| Terrain around the camera | Triangles | Camera + shadow | + reflection everywhere | 4 shadow samples | Rebuild 8 chunks + TLAS | Build everything |
|---|---|---|---|---|---|---|
| 368 chunks (136 m across) | 274k | 0.95 ms | 1.60 ms | 2.74 ms | 0.30 ms | 6 ms |
| 1395 chunks (270 m) | 1.0 M | – | 1.82 ms | – | 0.36 ms | 22 ms |
| 3197 chunks (400 m) | 2.3 M | – | 1.76 ms | – | 0.44 ms | 47 ms |

**Answer.** Yes, by a wide margin.
- The trace cost hardly grows with the terrain: about 1.8 ms for a camera ray, a shadow ray and a reflection
  ray with its own shadow on every pixel.
- An edit or a newly streamed chunk costs a BLAS rebuild of about 0.05 ms.
- Building 3000 chunks from scratch (47 ms) must be spread over frames, like meshing is.

**Found, for the real version:**
- **Cracks.** Greedy quads meet at T-junctions, which is not watertight: rays slip through hairline cracks
  (dotted lines in the shots). The ray geometry needs edges split at T-junctions or a small overlap.
- **Wrong normals.** A few half-quads shade with the wrong normal (dark triangles). Not found yet.
- **The CPU side.** The spike meshes 3000 chunks on one thread (4.5 s). The game already meshes on workers;
  the triangles for ray tracing come from the same quads (or a GPU pass that expands them).
- **The far planet, the moon and the sun.** These are better as analytic spheres in the shader than as
  triangles. An exact ray–sphere test gives eclipses and the planet's own shadow at any distance.
