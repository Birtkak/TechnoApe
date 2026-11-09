# Voxel rendering research: how the fastest voxel engines are built, and what the engine takes from them

Researched 2026-11-09 by web search for the engine spike; every claim has its source. Numbers marked
*self-reported* come from a project's own README or store page, not from an independent test. "The spike" and
"p15" below mean the Rust + wgpu engine spike the game's engine started from (ADR 0002).

**Changed since, on the cube-sphere grid (ADR 0003):** greedy merging is off (merged quads crack on the curved
grid), the per-direction draw ranges are gone (they saved vertices, not frame time), the height is one noise
sample per column instead of a 3D lattice, and culling tests bounding spheres instead of boxes.

## Summary

| Area | Technique | Verdict |
|---|---|---|
| Meshing | Binary greedy meshing on worker threads | **adopted** |
| Meshing | Meshing in compute shaders | rejected |
| Vertex format | 8-byte packed quad, vertex pulling | **adopted** |
| Draw submission | One buffer + sub-allocation + multi-draw-indirect + per-direction ranges | **adopted** |
| Draw submission | CPU frustum + analytic horizon culling | **adopted** (instead of Hi-Z) |
| Draw submission | GPU Hi-Z occlusion culling in compute | rejected for the benchmark, wanted later for caves |
| Draw submission | Mesh shaders | rejected (available in wgpu, not needed) |
| Alternative | Ray marching a voxel volume / SVO / SVDAG | rejected |
| Data | No stored terrain: seed + sparse edit map, empty/solid shortcuts | **adopted** |
| Data | Palette-compressed chunks | not needed yet (nothing is stored); the game will need it |
| Data | Height noise on a coarse lattice, interpolated | **adopted** |
| Data | SIMD noise (FastNoise2) | rejected for the spike (C++ dependency), wanted later |
| LOD | Blocky chunk octree, 8 levels, camera-relative origins | **adopted** |
| Streaming | Priority by distance and visibility, prefetch along velocity, upload budget | **adopted** |
| Frame pacing | No blocking maps, immediate present, queue staging writes | **adopted** |
| Frame pacing | Separate render thread | not done |

## Meshing

**Binary greedy meshing.** Occupancy is stored as one bit per cell in 64-bit columns; hidden faces are removed
a whole column at a time with `col & ~(col >> 1)`, and quads are merged with bit operations per slice.
The reference C++ implementation reports 50-200 µs per 62³ chunk, 74 µs average single-threaded on a Ryzen
3800X (*self-reported*) [1]. The Rust port reports 65 µs (opaque only) to 300 µs on a Ryzen 5 5500, 7-30x
faster than the classic `block-mesh-rs` (*self-reported*) [2].
**Adopted** (`src/mesh.rs`, 32³ chunks with a 1-cell border, same-material merge). Measured: `run=meshbench`.

**Faces across chunk borders.** Both references mesh a padded chunk (62 + 2), so border faces are culled with
no neighbour lookups at mesh time [1]. **Adopted**: the generator fills the 34³ padded grid itself from the seed
and the edit map, so a job never waits for a neighbour chunk.

**Worker threads.** [1] reports 108 µs per chunk in a thread pool (slower per chunk than single-threaded, from
memory bandwidth), so throughput scales less than linearly. **Adopted**: rayon pool of (cores - 2).

**Compute-shader meshing.** Public reports are either against single-threaded C# (90-160x, not comparable) or
note that reading the mesh back for collision is the slow part [3]. A CPU mesher at ~100 µs per chunk already
outruns what the benchmark needs, the game needs the cells on the CPU anyway (collision, simulation, saves), and
a GPU mesher competes with rendering for the frame budget. **Rejected.**

## Vertex format

**Packed quads + vertex pulling.** [1] packs a face into 8 bytes (6-bit x, y, z, width, height + material) and
expands it in the vertex shader from `gl_VertexID`; no vertex or index buffer. A conventional face is 4 vertices
of 12-32 bytes plus 6 indices, so 60-150 bytes. **Adopted**: 8 bytes per quad (x6 y6 z6 w5 h5 dir3 + material),
6 vertices generated per quad from `vertex_index`. The ablation `fat=1` uploads 96 bytes per face instead.

## Draw submission

**One buffer, multi-draw-indirect, per-direction culling.** [1] draws every chunk with one
`glMultiDrawElementsIndirect` and skips face groups that point away from the camera. **Adopted**: one storage
buffer with a first-fit free list, quads grouped by direction per chunk, one `multi_draw_indirect` per frame; a
chunk contributes at most the 3 direction ranges that can face the camera (up to 5 when the camera is inside its
box). The indirect buffer is rebuilt on the CPU each frame.

**wgpu indirect validation.** wgpu by default injects a compute pass that validates indirect draw arguments;
a WebGPU best-practices benchmark measured 3 ms of a 6 ms pass spent there when indirect buffers were split, and
~10 µs when batched [4]. **Adopted**: one indirect buffer, and `InstanceFlags::empty()` switches the validation off.

**GPU Hi-Z occlusion culling.** The standard GPU-driven design tests each instance against a depth pyramid in
compute and writes the indirect buffer and count on the GPU [5]. One measured case in a different workload
(a BIM viewer) found a big gain on interior views, none on exterior overviews, and a regression from many empty
sub-draws [6]. On a small planet the dominant occluder is the planet itself, and that test is analytic: a chunk
whose box lies inside the shadow cone of the never-meshed core sphere is hidden. **Adopted the analytic horizon
test on the CPU; Hi-Z rejected for the benchmark.** It becomes worth it when the world has large caves and
buildings, where terrain hides terrain; wgpu supports what it needs (compute, `MULTI_DRAW_INDIRECT_COUNT`).

**Mesh shaders.** wgpu has had mesh shaders since v28 (December 2025): full on Vulkan, passthrough shaders on
DX12 and Metal, behind `EXPERIMENTAL_MESH_SHADER` [7][8]. In Minecraft, the mesh-shader renderer Meshelium
reports 1.5x at 12 chunks and 5.3x at 64 chunks over the game's Vulkan renderer on an RX 9070 XT
(*self-reported*) [9]; the gain is per-meshlet culling at long view distances. p15 draws a few hundred chunk
ranges, all pre-culled, with 8-byte quads. **Rejected**: no measured bottleneck for them to remove, and they
would tie the engine to an experimental wgpu path.

## Alternatives to meshing: ray marching

- **Teardown**: each object is a small 3D texture; the renderer rasterizes its bounding box and marches a DDA
  through the volume in the fragment shader, with mips to skip empty space; the world volume is about
  1252 x 128 x 1252 voxels and levels hold roughly half a billion voxels (secondary sources) [10][11].
- **Sparse 64-trees**: 358 M voxels in 224 MB (0.62 bytes per voxel, against 1.02 for an ESVO); edits of
  compressed nodes are "quite a bit more complicated" and not timed [12].
- **SVDAG (Kämpe 2013)**: a binary grid orders of magnitude smaller than an SVO, ray tracing on par with
  triangle ray tracing [13].
- **Aokana (2025)**: 256³ chunks as shallow SVDAGs, octree LOD, streaming, Hi-Z tile culling, compute ray
  marching. ~6 ms per frame at 64K³ (about ten billion voxels) on an RTX 3060 Ti, 424 MB VRAM, 2-4x faster than
  HashDAG. **No runtime voxel edits**, no transparency [14].

Paper comparison for this game: the benchmark planet draws under 100,000 merged quads per frame from the ground
(measured in the spike); rasterizing that takes about 0.1 ms on the reference GPU, while ray marching costs a per-pixel traversal of 2 M pixels
however little geometry there is, and the fastest structures (SVDAG) cannot be edited at runtime, which rules
them out for a fully diggable world. Ray marching wins when voxels are far smaller than pixels can resolve by
meshing (Teardown's 10 cm, Aokana's billions); with 0.5 m blocks the mesh is tiny. **Rejected.** It stays the
right tool for the 12.5 cm fine detail (chiseled blocks, rigid pieces): small volumes in a box, Teardown-style.

## Data

**Nothing stored.** The world is a function of the seed plus a sparse map of edited blocks; a chunk's cells
exist only inside its meshing job. Sky and core are skipped before any noise is evaluated (box against the
sphere shell), and a chunk that turns out all air or all solid stores nothing. **Adopted.** RAM per km of planet
is therefore the edit map only.

**Palette compression.** Minecraft-style palettes store 1-4 bits per block instead of 8-16, per chunk [15].
**Not needed in the spike** because no chunk is stored; the game will need it for chunks that carry edits,
machines and fluids. Estimate: a 32³ chunk at 4 bits is 16 KB against 32 KB as bytes.

**Noise.** FastNoise2 (SIMD) is 5-7x faster than FastNoise Lite in its own benchmark table (*self-reported*,
units not stated) [16]. p15 uses FastNoise Lite (pure Rust port) and gets its speed-up another way: the height
field is evaluated on a lattice (every 1-4 cells, at most 8 m) and interpolated. **Lattice adopted; FastNoise2
rejected for the spike** (C++ build dependency), worth revisiting.

## Far planet / LOD

**Blocky LOD.** Distant Horizons and Voxy keep Minecraft's look at distance by re-meshing aggregated voxels;
published numbers are qualitative (hundreds of chunks of view) [17]. Aokana merges eight LOD n chunks into one
LOD n+1 chunk on a CPU octree [14]. **Adopted**: a chunk octree with 8 levels (cells 0.5 m to 64 m, always 32³
cells per chunk), a chunk splits when the camera is nearer than `k` chunk edges, coarse cells sample the block
at their centre, a parent stays on screen until all its children are meshed. Positions are f64 on the CPU and
camera-relative f32 on the GPU, so there is no precision loss at the moon (a floating origin without moving
the world).

Not done: skirts or transition cells between levels. Blocky levels differ by whole cells, so a crack can show
at a level border; the filmstrips are the check.

## Streaming

Priority by distance in chunk edges, hidden chunks last, visible uncovered holes first; a second octree walk
from the position `prefetch` seconds ahead along the velocity; a byte budget per frame for uploads. No public
engine publishes a "top speed without holes" number to compare with; the spike measured ours.

## Frame pacing

All uploads go through `queue.write_buffer` (wgpu copies into its own staging memory, no map stall); GPU
timestamps are read through a ring of 8 map-read buffers that are only read once their map has completed;
present mode is Immediate. wgpu's own layers cost CPU compared with `wgpu-hal` or raw Vulkan in draw-call-heavy
scenes (Blade's bunnymark: wgpu slows at 15-20K objects, wgpu-hal at 60K, *self-reported*) [18]; with one
multi-draw per frame that overhead does not apply.

## What wgpu cannot do yet (against raw Vulkan / DX12)

| Feature | wgpu 30 | Matters here? |
|---|---|---|
| Mesh shaders | experimental; full on Vulkan, passthrough on DX12/Metal [7][8] | no |
| Multi-draw-indirect, with count | yes, native only (`MULTI_DRAW_INDIRECT_COUNT`) [8] | used / available |
| Bindless (binding arrays) | texture and buffer arrays behind native features; partly experimental [8] | later, for many block textures; an array texture is enough |
| Hardware ray tracing | BLAS/TLAS and ray queries, experimental [7] | no |
| Sparse/virtual buffers, explicit memory placement | no | no |
| Explicit multi-queue, async compute | no | no |
| Per-frame validation and resource tracking cost | cannot be fully removed | small with one draw call |

Nothing on this list blocks the planet game. The risk is API churn instead: wgpu ships a breaking major
version about every three months (v28 December 2025, v29, v30 by August 2026) [7].

## Gaps against this research

- No Hi-Z occlusion culling (reasoned above; a cave-heavy scene would need it).
- No LOD seam handling.
- No SIMD noise.
- Single-threaded frame: selection, uploads and encoding share one thread.

## Sources

1. cgerikj, binary-greedy-meshing (C++): https://github.com/cgerikj/binary-greedy-meshing
2. binary-greedy-meshing Rust crate: https://docs.rs/crate/binary-greedy-meshing/0.4.2
3. GPU voxel mesh generation reports: https://github.com/artnas/unityvoxelmeshgpu and https://symetry.itch.io/planet-generation/devlog/541145/compute-shaders
4. WebGPU indirect draw best practices (toji.dev), found through a mirror: https://toji.dev/webgpu-best-practices/indirect-draws
5. GPU-driven Hi-Z culling tutorial: https://daslang.io/doc/reference/tutorials/vulkan/12_gpu_driven.html
6. IfcOpenShell viewer Hi-Z commits: https://dev.glassvision.dedyn.io/Glassvision/IfcOpenShell/commit/8596d53a4aa75eb88a862f538813dae798ad54a0
7. wgpu releases: https://github.com/gfx-rs/wgpu/releases
8. wgpu mesh shading and feature docs: https://wgpu.rs/doc/wgpu/documentation/extensions/mesh_shading/index.html , https://docs.rs/wgpu-types/latest/wgpu_types/struct.FeaturesWGPU.html
9. Meshelium: https://www.curseforge.com/minecraft/mc-mods/meshelium
10. Teardown frame breakdown: https://juandiegomontoya.github.io/teardown_breakdown.html
11. PC Games Hardware, Teardown tech test: https://www.pcgameshardware.de/Teardown-Spiel-73677/Specials/PC-Game-Test-Benchmark-Voxel-Dynamic-Destruction-1394545/2/
12. Fast voxel ray tracing using sparse 64-trees: https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing
13. Kämpe, Sintorn, Assarsson, High Resolution Sparse Voxel DAGs: https://research.chalmers.se/en/publication/182658
14. Aokana: https://arxiv.org/html/2505.02017v1
15. Efficient block storage with palettes: https://ogyrecdev.github.io/posts/efficient-block-storage-palettes
16. FastNoise2: https://github.com/Auburn/FastNoise2
17. Distant Horizons vs Voxy: https://gurugamer.com/pc-console/distant-horizons-vs-voxy-the-ultimate-lod-mod-showdown-for-minecraft-26068
18. Blade graphics performance notes: https://rust-digger.code-maven.com/src/blade-graphics-0.5.0/etc/performance.md
