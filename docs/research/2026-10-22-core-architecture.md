# Core architecture research: resolution, rendering, simulation, engine

- **Date:** 2026-10-22 (joint study by the generator session and the player-loop
  session; the simulation chapter lives in `2026-10-22-core-architecture-sim.md`)
- **Trigger:** Burkelbear Games' "How graphics work in my 3D Noita-like game,
  Grimorium" and the request to "really research what is going to be the best
  for what we want, no problem if we need to change basics".
- **Decision:** ADR 0012 (proposed) records the options and the recommendation.

## 1. What we want (the bar every option is measured against)

From `docs/design/vision.md` and the requests of 2026-10-22:

1. Minecraft/GregTech building and automation on a block grid, plus **every
   voxel chiselable** into fine shapes (ADR 0006: 6.25 cm cells).
2. Noita-style simulated materials: liquids, powders, gases, fire, reactions,
   spells that reshape the world.
3. **Mass edits never choke**: a black-hole spell or a thousand TNT in one
   frame must not stall the game.
4. **Epic horizons**: 2 km of visible world with level of detail, distant
   player builds in view.
5. Windows client + Linux dedicated server, deterministic simulation,
   deltas over the wire; the reference PC is an RTX 3070 / 5600X with p99
   frame <= 8.3 ms, RAM <= 4 GB, VRAM <= 6 GB (ADR 0009).
6. One solo developer with Claude doing the implementation: engineering
   cost is a first-class criterion.

## 2. What the serious voxel games do

| game / work | voxel size | world storage | renderer | simulation | horizon | notes |
|---|---|---|---|---|---|---|
| Minecraft, Vintage Story | 1 m blocks (VS chisel 6.25 cm) | chunk arrays, palettes | meshed, rasterised, LOD mods | cellular rules per block tick | km with LOD mods | our starting point; VS chisel = block entity + per-block mesh |
| Noita (Falling Everything) | 1 px (2D) | chunks with dirty rects | 2D sprites of the pixel buffer | CPU falling-sand CA, multithreaded checkerboard | screen + nearby chunks | the simulation model we already copied (P5) |
| Teardown | ~10 cm, per-object grids that move/rotate | one grid per object, hundreds of millions of voxels, world capped at 4000 voxels across (texture limit) | GPU ray marching, several acceleration layers, one-bit-per-voxel occlusion bitmap for ray-traced shadows/AO (CPU updates it) | CPU voxel-vs-voxel physics, debris; no structural integrity | small maps, no LOD | custom engine (OpenGL, later D3D12); author says Unity/Unreal would have needed rewrites |
| Grimorium (Burkelbear, WIP) | "micro-voxels" (size unpublished, looks like 5-10 cm); 16-bit voxel = 12-bit id + 4-bit variant (colour variation, liquid fullness, gas density, electricity) | 16³ chunks with flags, recursive grouping, one fixed toroidal memory pool (chunks overwritten in place); >100 M voxels in ~200 MB | per-pixel compute DDA at 1/3 resolution with jitter + temporal accumulation; transparent materials accumulate, gases as density; two reduced-LOD rings; emissive voxels condensed into per-chunk lights; grass/leaves procedural in the shader from a hash (zero storage) | **GPU** compute CA on active chunks (SSBO groups, atomics for contention), edits batched per chunk, one CPU<->GPU round trip per frame; explosions as r² DDA rays from the centre with hardness vs power; bounded flood fill (~32 k voxels) for islands that become rigid bodies | dungeon rooms + 2 LOD rings | started as a Godot ray-marching prototype with bricks, now custom C++; Steam minimum spec RTX 30-series (details: sim chapter) |
| Aokana (paper, 2025) | voxelised scenes to 64K³ | 256³ chunks as sparse voxel DAGs, LOD by merging 8 chunks into one | GPU ray marching that coexists with mesh passes, Hi-Z culling | none (static) | open world, streamed | ~6 ms/frame on an RTX 3060 Ti at 64K; no runtime edits, no transparency |
| Sparse 64-trees (2024 guide) | any | 4×4×4 nodes with 64-bit masks, 0.6 B/voxel | GPU ray tracing with ancestor memoisation | none | large worlds as many trees on a top grid | edits need a pool allocator and relinking; LOD not solved |
| Brickmaps (2013 onwards) | any | coarse grid of pointers to dense 8³ bricks | two-level DDA; what P7 does | friendly to edits (brick granularity) | pageable from CPU | the simplest editable GPU structure; what Grimorium's prototype used |
| voxel-nexus (2026 WIP) | 64³ volumes | brickmap per volume + 8/16/32³ summaries in rings | both a raster path and a compute ray-cast path share LOD selection | – | whole world through fog | targets p99 <= 10 ms on an RTX 3060 laptop; first qualification run failed on both paths |

Grimorium's choices that matter for us (from its three devlogs, written up
in the sim chapter): the simulation runs on the GPU, which rules it out for
our headless, deterministic, authoritative server, so our kernel stays on
the CPU; the toroidal fixed pool and the R16 id+variant cell are cheap wins
for the brick store and the caster; rendering at reduced resolution with
temporal reprojection would need a compute caster writing its own motion
vectors (a spatial shader cannot), which is the next renderer step if the
budget ever tightens; explosions as shell rays and bounded island checks
are the model for our spells.

Patterns worth copying: every ray-cast engine uses a two-level structure
(bricks or wide trees) and skips empty space; every edit-heavy engine keeps
edits at brick granularity; lighting from voxels is done with a cheap
occupancy copy of the world (Teardown's bitmap, our 8³ occupancy); large
worlds are rings of coarser copies around the player, not one resolution.

## 3. Our measurements

**P7 (`prototypes/p7-raycast-near`)**: ray-cast R8 volume with brick
occupancy, sun shadow ray, depth-composited with meshes, 1920x1080, RTX 3070:

| volume | extent at 6.25 cm | per-frame upload | p50 | p99 | VRAM |
|---|---|---|---|---|---|
| 256³ | 16 m | 4 MB | 3.3 ms | 3.7 ms | 132 MB |
| 512³ | 32 m | none | 3.3 ms | 6.1 ms | 236 MB |
| 512³ | 32 m | 16 MB | 5.8 ms | 7.4 ms | 268 MB |

Uploads cost ~3 GB/s (whole 128 MB cube in one frame = 45 ms stall).

**P8b (P7 fed by the native SimWorld, dirty chunks only, `results_sim.jsonl`)**:

| volume | sim Hz | frame p50 / p95 / p99 | sim tick p50 / p99 | dirty chunks p50 / max | upload p99 | 8 m TNT box carve | ticks after TNT max |
|---|---|---|---|---|---|---|---|
| 256³ | 30 | 1.7 / 2.5 / 2.6 ms | 1.3 / 2.1 ms | 62 / 751 | 0.7 ms | 8.7 ms | 2.1 ms |
| 512³ | 30 | 2.8 / 5.0 / 5.6 ms | 2.8 / 6.6 ms | 99 / 805 | 1.8 ms | 10.2 ms | 4.1 ms |

The scene was mostly static ground with a pool, a spring and a glowing-ore
field, so the active fraction was low (max 287 of 32 768 chunks); the sim
chapter measures heavy activity. The TNT frame stayed within budget because
only the touched chunks (805 × 4 KB = 3.3 MB) were uploaded.

**P9 (`BrickStore`, `prototypes/p9-brick-store/README.md`)**: a 64 m cube of
generated world is 10 MB as uniform bricks and 27 MB with every surface
brick expanded (1.6 % of bricks); a 10 m crater carved to air costs 0.4-0.7
ms and touches 2-4 k bricks; a moving 5 m black hole for 30 ticks 3-4 ms;
200 k random cell writes 50-60 ms; the whole 32 m near field extracted as
16³ chunks 150-180 ms (load-time, per frame only dirty chunks).

**P7 fed from the BrickStore (`feed=brick`)**: the real generated world
around the spawn, ray-cast at 6.25 cm from bricks, with a chisel walking the
surface at 30 Hz: 512³ at 2.8 / 4.6 ms p50 / p99, uploads under 0.5 ms.

**Sliding window**: a 32 m window moving through a 64 m brick store with
toroidal addressing re-uploads only the entering chunk slab (4-8 MB, 5.7 ms
p50 per slide); frames stay at 5.8 / 7.0 ms p50 / p99 while flying.

**Meshed path today (ADR 0010/0011 runs):** forest-fire churn p99 12.1 ms
with mesh colliders, 3.0 ms with voxel-grid collision; a 20 m/s flight p99
21.9 -> 2.1 ms. Large edits still remesh every touched 32³ block.

## 4. Budget arithmetic

| near field | cells | 1 B/cell on GPU | sim state at 3 B/cell | note |
|---|---|---|---|---|
| 32 m at 6.25 cm | 134 M | 134 MB | 402 MB | P7/P8b measured |
| 64 m at 6.25 cm | 1.07 G | 1.07 GB | 3.2 GB | over the RAM budget alone |
| 64 m at 12.5 cm | 134 M | 134 MB | 402 MB | same cost as 32 m at 6.25 cm |
| 2 km at 0.5 m (today, LOD) | – | meshes | 0.5 m sim bubble 24 m | godot_voxel |

Uniform-brick compression changes the picture: a brick whose 512 cells are
one material costs 1 byte plus an index. In a generated world almost every
brick is uniform (solid rock, air, water); only surfaces, chiseled blocks and
simulated cells expand. Expected RAM for a 64 m micro field is then tens of
MB, not gigabytes, and the sim only ticks expanded bricks.

## 5. Candidate architectures

**A. Status quo plus.** 0.5 m meshed world everywhere, chisel detail as
meshed overlays (ADR 0006), 0.5 m sim bubble, mitigations from ADR 0010/0011
(voxel-grid collision, edit budgets, worker-thread remesh).
Pros: exists, cheapest, physics/LOD/lighting from Godot. Cons: mass edits are
bounded by remeshing (tens of ms for dozens of blocks), the simulation stays
coarse (0.5 m cells never look like Noita), chisel rendering costs triangles.

**B. Brickmap world, ray-cast near field, meshed far field (recommended).**
The world's native grid becomes the 6.25 cm cell, stored as a brickmap whose
bricks are exactly today's 0.5 m voxels (8³ cells): uniform bricks are one
byte (what the generator and the far world already are), expanded bricks hold
512 cells. Around the player (32 m) the bricks are ray-cast directly from a
GPU copy (P7), fed by the simulation at brick granularity (P8b); beyond that
godot_voxel keeps meshing the uniform-brick view with LOD to 2 km. Chiseling
is just expanding a brick; the sim runs on expanded bricks inside the bubble
and collapses them back when homogeneous; mass edits touch bricks, so their
cost is proportional to the bricks changed and can be budgeted per frame.
Pros: measured renderer, keeps the horizon and the generator, one data model
for building/chisel/sim, Minecraft-scale building still snaps to the brick.
Cons: two renderers to keep consistent (lighting, shadows into Godot's shadow
maps, transparency), a seam at 32 m, the simulation must be rewritten for
micro cells on bricks (sim chapter), voxel-grid collision only (done).
Cost: a quarter's work on the engine side, in spikes first.

**C. Full micro-voxel ray-cast world (Grimorium / Teardown).** Drop meshing
and godot_voxel; the whole visible world is ray-cast from LOD volumes
(Aokana-style chunk merging for the far rings), everything simulated.
Pros: one renderer, instant edits everywhere, best Noita look.
Cons: 2 km horizons need streamed LOD volumes with no runtime edit support in
the literature we found; transparency, fog, shadows and physics all become
ours; Grimorium needed a custom engine and an RTX 30 minimum; a solo
developer loses every Godot system we lean on. Cost: a year, and it conflicts
with the building-on-a-grid feel.

**D. Change the base voxel to 0.25 m with meshing.** Halfway house: 8× the
voxels of today for the same metres, no new renderer. Rejected: it pays most
of the cost without reaching either goal.

## 6. Engine: Godot or custom?

P7 shows Godot's RenderingDevice is enough: 2D-array textures, compute
passes, storage buffers, a spatial shader writing DEPTH that composites with
the mesh world, all from GDScript with the C++ extension for the data.
Grimorium left Godot for C++; Teardown's author prefers custom engines and
says off-the-shelf engines would need rewrites. For us the deciding facts
are different: we already have the native extension for all per-voxel work,
Godot gives the client UI, audio, input, networking, the editor and the Linux
server export, and the only engine seams option B needs are the ones P7
already crossed (depth) and both of the others are closed: shadows from the volume into Godot's
shadow maps work (P7 `cam=shadow`) and see-through water costs 0.5 ms.
Recommendation: stay in Godot; revisit only if a measured seam cannot be
crossed.

## 7. Open risks and the spikes that close them

1. **Simulation at micro resolution** (peer chapter): tick cost under heavy
   activity, sparse active bricks, memory, the TNT and black-hole events.
2. ~~Shadows from the ray-cast volume onto meshes~~: **closed 2026-10-22**.
   P7 with `cam=shadow` shows the volume's shadow on the Godot ground plane
   with the ray-cast terrain's own silhouette (`out/shadow/n256_1.png`): the
   DEPTH-writing shader runs in the directional shadow pass. Shadows from
   meshes onto the volume still need a lookup of Godot's shadow map inside
   the caster (or a voxelised copy of nearby meshes); not yet tested.
3. ~~Transparency~~: **closed 2026-10-22** for water: on a water hit the ray
   continues through water cells to the next solid and the colour is tinted
   by the metres of water crossed, with a Fresnel rim; depth is the water
   surface so meshes composite against it. Cost +0.5 ms p50 at 512³ fed by
   the sim (3.3 / 6.1 ms p50 / p99 instead of 2.8 / 5.6). Glass and ice would
   use the same path with another tint.
4. **The 32 m seam**: the far mesh must hide under the near volume
   (overdraw) or be cut; the near volume must be fed from the same bricks the
   far mesher reads (one source of truth).
5. **Determinism and the server**: the server simulates micro bricks
   headless; the client only renders. Deltas are dirty bricks.

## 8. Recommendation (final, both chapters)

The simulation chapter (`2026-10-22-core-architecture-sim.md`, P10) answers
gate 1 with "yes, with conditions": generated-ground events cost 0.5-2 ms
per tick at 6.25 cm, bulk loose material 14-19 ms worst case after kernel
v5, so the tick must leave the main thread, carry a work cap with distance
tick-rate LOD, keep ground static until loosened, and upload by dirty box;
the dense sim window is 24-32 m by 16 m inside the 32 m render window, with
a brick-sparse kernel as the later step. 12.5 cm is the fallback.


Option B. The measurements say the renderer and the feed are already inside
budget on the reference PC; what remains is the simulation at micro
resolution and three engine seams, each a short spike. It keeps the parts of
the game that already work (generator, horizons, building, block entities)
and turns the chisel grid into the world's real grid instead of a special
case.
