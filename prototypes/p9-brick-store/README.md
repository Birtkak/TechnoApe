# P9 – Brick world store (ADR 0012 option B, step 2)

**Question:** does a world stored as bricks of 8³ cells of 6.25 cm, where a
brick is one of today's 0.5 m voxels and is uniform (one byte) until detail
or simulation expands it to 512 cells, fit the memory and edit budgets, and
can it feed the ray-cast near field and the simulation?

**Code:** `prototypes/p5-sim-spike/src/brick_store.{h,cpp}` (`BrickStore`,
registered in the technosim extension; API in `docs/design/brick-world.md`).
Bench: `prototypes/p7-raycast-near/brick_bench.gd`:

```
Godot_..._console.exe --headless --path prototypes/p7-raycast-near --script res://brick_bench.gd -- bricks=128
```

**Measured 2026-10-22 (5600X, generated world seed 1337, surface band around sea level + 40):**

| | 32 m cube (64³ bricks, 134 M cells) | 64 m cube (128³ bricks, 1.07 G cells) |
|---|---|---|
| fill from the generator (uniform bricks) | 83 ms, 64 blocks | 215 ms, 512 blocks |
| memory as generated | 1.3 MB | 10 MB |
| expand every surface brick (what chisel/sim would touch) | 0.9 ms, 3 435 bricks (1.3 %) | 14 ms, 34 042 bricks (1.6 %) |
| memory with all surfaces expanded | 2.9 MB | 27 MB |
| 200 000 random cell writes (each expanding a brick) | 49 ms, 4 059 writes/ms | 58 ms, 3 422 writes/ms |
| 1000 TNT: 10 m sphere carved to air | 0.66 ms, 4 395 bricks | 0.44 ms, 2 402 bricks |
| black hole: 5 m sphere per tick, 30 ticks, moving | 4.4 ms, 10 836 bricks | 2.7 ms, 5 914 bricks |
| collapse sweep over every brick | 12 ms | 50 ms |
| feed of the whole 32 m near field as 16³ chunks | 178 ms for 128 MB | 154 ms |
| process memory | 24 MB | 24 MB |

**Answer:** yes.
- A 64 m cube of real generated world is 10 MB as bricks, 27 MB with every
  surface brick expanded; the 32 m near field is 1.3 / 2.9 MB. The 1 G-cell
  figure of the dense layout never materialises because 98 % of bricks are
  uniform.
- Mass edits are store-trivial: a 10 m crater costs under a millisecond and
  touches a few thousand bricks, so the per-frame budget of ADR 0012 is
  spent on the GPU upload and the far-field remesh (a few MB), not on the
  store.
- The pool never shrinks on its own (free slots are reused); a periodic
  compaction or a cap is needed for long sessions.
- Second pass (same day): `collapse_dirty()` collapses only the bricks
  touched since the last clear: 0.1 ms for the black-hole bricks (138
  collapsed) against 50-60 ms for the full sweep. The chunk feed got a
  uniform-brick fast path (8-cell memset runs): the whole 32 m near field
  now extracts in 69 ms instead of 154 (load-time; per frame only dirty
  chunks move).
- Native accessors (`raw_cells_mut`, `raw_cells`, `raw_ref`, `mark_dirty`)
  let the simulation kernel work on brick memory directly.

**P7 caster fed from the store (`feed=brick`, `brick_feed.gd`):** the real
generated world (seed 1337, 32 m around the spawn) loaded as uniform bricks
with surfaces expanded, a chisel walking the surface at 30 Hz and a 10 m
sphere carved at 4 s (`shots/n512_brick_world.png`):

| volume | frame p50 / p95 / p99 | store fill | dirty chunks p50 / max | upload p99 / max |
|---|---|---|---|---|
| 256³ | 1.8 / 2.0 / 2.1 ms | 71 ms | 8 / 47 | 0.13 / 0.39 ms |
| 512³ | 2.8 / 4.6 / 4.6 ms | 83 ms | 8 / 47 | 0.17 / 0.46 ms |

(The carve in this run sat mostly in air, so it changed few chunks; the
crater numbers are the bench's.)

**Sliding window (`cam=follow`):** the store is a 64 m cube and the 32 m
ray-cast window slides inside it with toroidal texture addressing (texel =
world cell mod n), so a move re-uploads only the slab of chunks that entered
the window. Flying 2 m/s for 8 s at 512³: 20 slides of one or two chunk
slabs (1024-2048 chunks, 4-8 MB), 5.7 ms p50 / 11.5 ms max per slide,
frames 5.8 / 6.7 / 7.0 ms p50 / p95 / p99 overall
(`shots/n512_sliding_window.png`; the striped ribbons are the test chisel
writing cells in mid-air). The slab upload should be spread over frames or
moved to a worker in the game; the GPU readback check (`verify_upload`)
matched the store cell for cell. Lesson: `.glsl` compute shaders are
imported to SPIR-V, so after editing one run `--headless --import` or the
old shader keeps running.

**Next:** the game's
`World` owning a `BrickStore` with godot_voxel reading its majority view.
