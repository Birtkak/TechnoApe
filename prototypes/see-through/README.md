# Spike: see-through leaves and still box models (2026-11-12)

Step 1 of `docs/design/see-through-and-boxes.md`. Decided in ADR 0068. Throwaway code on the branch, behind
run arguments; with every switch off the game is as it was (the shader's spike `discard`s are stripped from
the source at load, the fixed viewpoints compare within 0.1 %, `cargo test` passes).

| argument | what it does |
|---|---|
| `leaves=opaque` (default) | today: leaves are opaque cubes |
| `leaves=fancy` | the four leaves, shrub and reed are **cutout**: on level 0 the mesher culls a face only against an opaque neighbour (faces between two leaf blocks stay), the fragment shader discards a hashed 2 x 2-texel hole pattern (two in five); coarse levels draw the block opaque in its mean colour |
| `leaves=shell` | cutout holes, but faces between leaf blocks dropped (today's mesher): the cheapest see-through crown, hollow inside |
| `light=1` (default) / `2` / `3` | the design's three light options, for shadow and bounce rays alike (shader.wgsl `trace`) |
| `clutter=16` / `4` / `1` | still box models on grass, one in 16 / 4 / every column, on level-0 chunks |
| `mblas=1` | the models' quads go into the chunk's BLAS (cast and take ray shadows) |

Fast leaves in the design (faces between leaves dropped, opaque, mean colour) cost what `base` costs: it is
today's mesh and today's shader.

## How it was measured
Dev PC, release build (fat LTO, `target-cpu=native`), default settings, off-screen, `quick=1`, the shared
`out/.lock` held around each series. Each row is the median of 2-3 runs (`bash prototypes/see-through/run.sh
<series> <reps>`, then `python -I prototypes/see-through/summarize.py out/see-through/<series>.jsonl`).

- **Frame thread work ms**: the frame thread's stages at p50 less the driver's waits (acquire, present,
  between), as `tools/perf.py` counts it.
- **GPU ms** by stage at p50: `gbuffer` holds the terrain draw *and* its sun, lamp and moon rays (they are traced
  in the g-buffer fragment shader); `trace` is the bounce pass (one ray per 2 x 2 pixels); `rt` the BLAS builds.
- **p99 frame** is the best of the reps. Another session compiled on the PC through these series and the GPU was 36 % busy with none of our runs going:
  frame-time tails of 10-200 ms landed on random rows, the no-change baseline included, and the GPU p50 of a row
  repeated within 3 % between reps. So the medians are sound, the tails are an upper bound.
- **Mesh time per chunk** comes from `run=meshbench at=...` (below): the 273 surface chunks of level 0 around
  the forest, each meshed five times on one thread and the fastest kept, so another process cannot inflate it.
  The `mesh us/chunk` column in the frame tables is the same on the game's busy workers and much noisier.
- **VRAM**: `VRAM MB` is the driver's figure and moves in 128-256 MB blocks; `wgpu alloc MB` is what wgpu holds,
  the finer number.

## Question 1: cutout terrain and its light

The forest: the old benchmark spawn, a broadleaf forest with a stream, noon in summer.

```
run=stand at=1125.34,3634.37,1915.81 time=23625 eye=9 pitch=-28 yaw=120   (+ leaves=, light=)
run=fly   at=1125.34,3634.37,1915.81 time=23625 alt=12 speed=15            (+ leaves=, light=)
```
(`run=fly` with `at=` is new: the great circle starts there, 12 m over the ground at 15 m/s, through the forest.)

Mesh time per chunk (meshbench, forest chunks, level 0):

| mesher | mesh us/chunk | quads/chunk | bytes/chunk |
|---|---|---|---|
| today (`leaves=opaque`, and `shell`) | 102 | 966 | 7.7 KB |
| `leaves=fancy` | 137 (+34 %) | 1357 (+40 %) | 10.9 KB |

```
run=meshbench at=1125.34,3634.37,1915.81 leaves=opaque|fancy
```

The frame (3 reps each):

| run | quads/chunk | frame thread work ms | gbuffer ms | trace ms | GPU ms p50 | p99 frame ms | VRAM MB | wgpu alloc MB |
|---|---|---|---|---|---|---|---|---|
| stand, today | 1018 | 0.864 | 0.954 | 0.567 | 2.231 | 3.62 | 813 | 609 |
| stand, fancy, light 1 | 1438 | 0.891 | 1.633 | 0.351 | 2.921 | 4.82 | 813 | 637 |
| stand, fancy, light 2 | 1438 | 1.072 | 2.151 | 0.748 | 3.615 | 5.03 | 813 | 637 |
| stand, fancy, light 3 | 1438 | 0.810 | 1.931 | 0.665 | 3.325 | 5.03 | 813 | 637 |
| stand, shell, light 2 | 1018 | 1.018 | 1.711 | 0.470 | 3.105 | 4.61 | 813 | 609 |
| fly, today | 1087 | 0.769 | 0.738 | 0.246 | 1.780 | 5.72 | 815 | 629 |
| fly, fancy, light 1 | 1529 | 0.816 | 1.340 | 0.263 | 2.210 | 11.68 | 1069 | 663 |
| fly, fancy, light 2 | 1529 | 0.878 | 1.759 | 0.397 | 2.970 | 8.94 | 1069 | 663 |
| fly, fancy, light 3 | 1529 | 0.869 | 1.601 | 0.359 | 2.792 | 6.49 | 1069 | 663 |
| fly, shell, light 2 | 1087 | 0.901 | 1.405 | 0.353 | 2.575 | 5.47 | 813 | 630 |

What the numbers say:
- **The frame thread does not care** (0.77-1.07 ms in every row, the spread is noise): cutout and every light
  option are GPU and worker costs.
- **Cutout itself** (fancy, light 1) costs +0.4-0.7 ms GPU, nearly all in the g-buffer: more faces, overdraw
  through the holes, and a shader that can discard loses early depth on the terrain draw.
- **Light 2** adds another +0.5-0.8 ms on top. Each shadow ray that meets a hole goes on nearest-first, up to
  four more queries; a crown with faces between its leaves has many faces in a row.
- **Light 3** is cheaper than light 2 by 0.2-0.3 ms and is exact (no step limit), even though the spike flags
  *every* chunk's geometry non-opaque, so the ground and trunks call the candidate loop too. Splitting each
  BLAS into an opaque geometry and a cutout one would only cheapen it.
- **VRAM**: +28-34 MB wgpu (more quads and larger BLASes); the driver's figure jumps a 256 MB block on the fly.
- **The medians hold ADR 0004**: GPU 3.6 ms at worst of 8.3. **The fly-through's tails are not settled.**
  Per run, fly p99 was 15.2 / 7.7 / 5.7 ms today, 11.9 / 11.7 / 175 with fancy and light 1, 11.4 / 12.5 / 8.9
  with light 2, 8.6 / 10.4 / 6.5 with light 3, 8.3 / 5.5 / 44 with shell. The stand view stays at 3.6-5 ms in its
  best runs. Fancy's fly tails sit over 8.3 ms more often than today's, but the PC was busy throughout, so whether
  that is the 40 % more quads to upload and build BLASes for while streaming through a forest, or the PC, needs
  a quiet rerun (`q1.txt`, fly rows) before step 2 ships.

What it looks like (`crowns-by-light-option.png`: today, then fancy with light 1, 2, 3; `shadow-under-a-crown.png`:
the ground beside the trunk with light 1, 2, 3):
- Light 1: the crowns have holes, but under them lies a **solid block-shaped shadow**, the same as today's.
- Light 2 and 3: the shadow under a crown is **dappled**: specks of sun scattered through the dark, thickest
  near the crown's edges, where a ray crosses fewer leaf faces. Under the middle of a thick crown it is still
  nearly solid, as it would be under a real tree. Light 2 and 3 look alike in single frames (each frame takes
  new points on the sun's disc; the bounce smoothing spreads the specks).
- With fancy leaves the inside of a crown shows its inner faces, darker, through the holes: a crown reads as
  leafy volume. With `shell` it reads as a hollow box of leaves (not filmed for this README; the numbers are above).

## Question 2: still box models in the chunk mesh

Three hard-coded models (client `spike.rs`), painted at 32 pixels per metre: a **pebble** of 3 tilted boxes
(18 quads), a **plant** cross of 2 planes with empty pixels, both sides (4 quads), a **mushroom** of 6 boxes
(36 quads). On the mesher's worker every air cell over a grass top is hashed: one in 16, 4 or every column gets
a model (4 : 4 : 2 pebble, plant, mushroom), one of 16 turns and a place in its cell
(`engine::mesh::clutter`). Level-0 chunks only: at default Detail (k = 4) about 470 of them around the camera,
to roughly 128-256 m.

**The quad format: 8 bytes, as a terrain quad.** `lo` = the cell (x6 y6 z6), the turn (4 bits), the place in the
cell (3 + 3 bits), bit 31 set; `hi` = a stand-in block for a ray's colour, and the face (16 bits) in a **face
table**. The face table holds each model face once: origin, two edges, its painting's place: 48 bytes a face,
2.8 KB for the three models. The vertex shader turns and places the face; the fragment shader reads the
painting and discards an empty pixel. Nothing per model instance is stored but the quads.

**Where the paintings live: in the block textures' buffer** (ADR 0045), at layer 1792 on (the blocks take far
fewer). The terrain pass already binds it, so drawing models costs no binding and no VRAM; the BLAS expand
pass and the bounce pass now bind it too (they read a model's face or an empty pixel at a hit). A second
buffer would cost the same memory and one more binding everywhere for no gain; it is worth it only when the
models' paintings outgrow the 2 MB (512 K texels; a 128 x 128 painting is 16 K texels, so about 30 such models
beside the blocks) – and then raising TEX_LAYERS is the cheaper answer.

The meadow by the spawn's forest:
```
run=stand at=0.0347,0.8675,0.4975 time=23625 eye=1.7 pitch=-20 yaw=0   (+ clutter=, mblas=, light=)
run=fly   at=0.0347,0.8675,0.4975 time=23625 alt=6 speed=15            (+ clutter=, mblas=, light=)
run=meshbench at=1125.34,3634.37,1915.81 clutter=16|4|1
```

Mesh time per chunk (meshbench, level 0 around the forest; grass under and between the trees):

| density | mesh us/chunk | quads/chunk | model quads | bytes/chunk |
|---|---|---|---|---|
| none | 102 | 966 | 0 | 7.7 KB |
| 1 per 16 columns | 159 | 1554 | 588 | 12.4 KB |
| 1 per 4 | 159 | 3281 | 2315 | 26.2 KB |
| every column | 176 | 10270 | 9304 | 82.2 KB |

The +57 us is the spike's scan of all 32 x 32 x 32 cells for grass under air, the same at every density; the
quads themselves cost little (+17 us for 9300 of them). A scan of the occupancy bits the mesher already has
would take most of the 57 us away.

The frame (2 reps each; `fly, c4, BLAS` 1 rep, its other run measured no frames):

| run | quads/chunk | model quads | frame thread work ms | gbuffer ms | trace ms | GPU ms p50 | p99 frame ms | VRAM MB | wgpu alloc MB | quad buffer MB |
|---|---|---|---|---|---|---|---|---|---|---|
| stand, none | 951 | 0 | 0.896 | 0.782 | 0.197 | 1.827 | (10.6, contended) | 819 | 570 | 16 |
| stand, 1/16 | 1757 | 805 | 0.913 | 0.839 | 0.310 | 1.908 | 3.26 | 813 | 570 | 19 |
| stand, 1/4 | 4182 | 3231 | 0.684 | 0.944 | 0.431 | 2.018 | 3.38 | 819 | 570 | 26 |
| stand, 1/1 | 13893 | 12942 | 0.537 | 1.784 | 0.227 | 2.857 | 4.39 | 825 | 570 | 53 |
| stand, 1/4, BLAS | 4182 | 3231 | 0.611 | 0.984 | 0.454 | 2.084 | 3.30 | 1069 | 729 | 26 |
| stand, 1/1, BLAS | 13893 | 12942 | 0.633 | 1.868 | 0.265 | 2.981 | 4.36 | 1587 | 1209 | 53 |
| stand, 1/1, BLAS, light 2 | 13893 | 12942 | 0.554 | 1.923 | 0.424 | 3.101 | 4.09 | 1593 | 1209 | 53 |
| fly, none | 1180 | 0 | 1.131 | 0.822 | 0.271 | 1.862 | (24.8, contended) | 815 | 610 | 20 |
| fly, 1/16 | 1959 | 779 | 0.919 | 0.931 | 0.406 | 2.020 | (19.7, contended) | 814 | 619 | 24 |
| fly, 1/4 | 4318 | 3137 | 0.675 | 1.298 | 0.270 | 2.175 | 5.50 | 825 | 609 | 32 |
| fly, 1/1 | 13675 | 12495 | 0.724 | 1.930 | 0.254 | 3.041 | 5.69 | 813 | 609 | 66 |
| fly, 1/4, BLAS | 4315 | 3135 | 0.734 | 1.323 | 0.305 | 2.226 | 5.01 | 1071 | 785 | 32 |
| fly, 1/1, BLAS | 13673 | 12500 | 0.657 | 1.991 | 0.294 | 3.133 | 5.65 | 1587 | 1340 | 66 |
| fly, 1/1, BLAS, light 2 | 13675 | 12495 | 0.617 | 2.114 | 0.589 | 3.365 | 5.87 | 1593 | 1306 | 66 |

What the numbers say:
- **The frame thread does not grow with the count**: 0.54-0.91 ms with 13 000 model quads a chunk against
  0.90-1.13 with none. The models arrive inside the chunk's mesh; the frame thread draws one range per chunk as
  before. There is no ceiling like ADR 0047's 1000 pieces.
- **GPU**: +0.08 ms at 1/16, +0.19 at 1/4, +1.0-1.2 at every column, nearly all in the g-buffer (vertices and
  the plants' discarded pixels). All within ADR 0004.
- **The BLAS** costs little time (+0.05-0.12 ms GPU) but **a lot of VRAM**: +160 MB wgpu at 1/4, +640-730 MB at
  every column (the driver's figure 819 to 1587 MB). The quad buffer grows by 37-46 MB at every column, 10 at 1/4.
- `light 2` with models in the BLAS lets light through the plants' empty pixels for +0.1-0.3 ms.

What it looks like (`models-c4-c1-close.png`: 1 per 4, every column, and close up): grey pebbles lying tilted, plant
crosses whose blades show the ground between them, red mushrooms with white spots, each turned and placed off
the cell's middle. Without the BLAS they cast no shadow and take the sun as if nothing stood around them; a
mushroom in the shade of a tree is still shaded (it is lit by the same rays as the ground).

## Recommendations

**Question 1, light: option 3**, a candidate loop over non-opaque geometry. It is the exact one (no step limit)
and measured cheaper than option 2: +1.0-1.1 ms GPU over today's forest with fancy leaves, GPU 3.3 ms of 8.3,
frame thread unchanged. Build it with two geometries per chunk BLAS, opaque quads flagged OPAQUE and cutout quads
not, so the ground and trunks never enter the loop (the mesher then groups cutout quads at the end of the land;
the spike flagged everything non-opaque, an upper bound). Glass then joins the same loop with a tint.

**Leaves: fancy as the default.** +34 % mesh time on a worker (35 us a chunk), +40 % quads, +0.4-0.7 ms GPU, and
a crown that reads as leafy volume and lets dappled light through its edges. Fast (today's mesh, opaque, mean
colour) stays the setting for weaker PCs. `shell` is not worth a setting: it saves little over fancy (0.2-0.4 ms)
and shows hollow crowns.

**Models: the store** is the block textures' buffer (layers past the blocks'), the **quad** 8 bytes with a face
table, as measured. **Clutter default: 1 per 4 columns of grass on level 0** (+0.19 ms GPU, +2300 quads and 18 KB
a chunk, frame thread flat); every column as the "many" setting (+1 ms GPU, 82 KB a chunk). The range is level 0
as the streamer picks it, so it follows Detail; a shorter range would need chunks remeshed as the camera moves,
which costs more than it saves. **Not in the BLAS for clutter** (hundreds of MB of VRAM at density); in the BLAS
for a block row's machine or a lone model, where the count is small.

What would change the answers:
- A real painted leaf texture with more or fewer holes than the hashed two in five: more holes make option 3's loop
  run longer (more candidates per ray); measure again with the first painted leaves.
- A forest denser than this one (jungle) or a higher Detail: more level-0 cutout faces; fancy's g-buffer cost grows
  with the leaf faces in view.
- Option 3 split into two geometries is predicted, not measured, to be cheaper than the spike's all-non-opaque one.
- Clutter with 64-box models (the design's guess at a limit) would be 5-6x the quads of these; 1 per 4 of such
  models is about the measured every-column row.

## Not measured, not verified
- **Glass**: not built. Option 3's candidate loop is where its tinted shadows would go (multiply the tint on each
  glass candidate instead of confirming it), so it is cheap to add then, but nothing of it was shown here.
- Option 3 with an opaque/cutout geometry split; a mesher scan of the occupancy bits for clutter.
- Frame-time tails on a quiet PC: another session was compiling during the series, with outside GPU load. The forest
  fly-through with fancy leaves is the one to rerun (see Question 1).
- Light 2 and 3 compared in single frames only; not filmed over time (the bounce smoothing may make the specks shimmer).
- The far LOD's mean-colour leaves were drawn but not compared side by side with today's far forest.
- Bounce rays use the same `trace` as shadow rays, so they also pass through holes; reflections in metal too. Not
  looked at separately.

## The review, and what it changes
A fresh reviewer read the diff (nothing breaks with the switches off; the bit layouts agree everywhere). Its findings:
- **Light 2 and 3 are not quite like for like.** With fancy leaves, two leaf blocks side by side put two faces in
  one plane, each with its own holes. Light 2 takes one of them at random and, at a hole, steps past both;
  light 3 needs a hole in both. So light 2 lets more light through a crown (about 0.4 of a shared plane against
  0.16) and its dappling is the brighter. The costs stand. For step 2: hash a cutout face's holes by its plane,
  not its cell, so the two faces agree (or keep one of them).
- **The clutter scan skipped grass on the padding's layer 0** (a chunk's own layer 0 then got no models): about
  1/32 of the ground, so the measured model counts and their costs are a few percent low. Fixed after the
  measurements; not rerun.
- **`discard` in the one terrain shader** can cost early depth on every terrain pixel, so fancy's g-buffer cost
  includes that, not only the leaves. Step 2 draws cutout quads in a pipeline of their own (they are grouped at
  the end of the land quads for the BLAS split anyway), so opaque terrain keeps early depth.
- `seen()` reads a model quad's normal as a terrain quad's (+x) with `mblas=1`: bounces and reflections off
  models are shaded wrongly. Spike only.
- The meshbench's best-of-five now runs only with `at=` (the reviewer caught it changing the rates of the plain
  meshbench too); with `at=`, its chunks-per-second rates count five meshes a chunk and are not comparable.
- With every switch off the mesher now allocates no second column set; it still does four atomic adds per near
  mesh (the spike's counters).
