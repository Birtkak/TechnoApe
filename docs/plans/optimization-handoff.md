# Handoff: the optimisation pass before world generation

For the Opus coding session. Written 2026-11-10 by the design session after reading the renderer, the
shaders, the streamer, the mesher and terrain generation, and measuring four quick runs. Paste the block
under "Prompt" into a new session; the rest of this file is what it points at.

## Prompt

> Read `CLAUDE.md`, then `docs/plans/optimization-handoff.md` in full. Do the work packages there in order,
> A first. Each package is: measure before, change, measure after, keep it only if the number moved, record
> it in one ADR (`0025-optimisation-pass.md`, one table row per package, like ADR 0012) and commit it on its
> own. Stop and ask the developer only where the file says "taste call". Do not run `tools/check.sh` per
> package: use `cargo test`, the quick runs, and `bash tools/views.sh` for anything that touches pixels; run
> the benchmark series once before A (the table in `docs/benchmarks.md` predates the lighting) and once at
> the end. Do not start world generation work; package E only prepares for it.

## Where the time is now (measured 2026-11-10, quick runs, 4 km planet, 1080p)

| run | fps | frame p50 / p99 / max ms | gpu p50 ms | mesh job µs | VRAM MB | note |
|---|---|---|---|---|---|---|
| stand | 255 | 3.9 / 5.5 / 9.2 | 2.9 | 931 | 1064 | 4 frames over 8.3 ms |
| fly | 213 | 4.6 / 6.0 / 8.2 | 3.6 | 755 | 1064 | |
| edit | 182 | 5.4 / 7.9 / 12.7 | 3.9 | 997 | 1832 | 14 frames over 8.3 ms, cpu 12 ms |
| orbit | 293 | 3.4 / 5.0 / 9.4 | 2.6 | 648 | 1064 | 1076 missing chunks, 16 holes |

Before the lighting (ADR 0022) the same runs were 2,300 to 3,600 fps with 42 µs mesh jobs and 495 MB VRAM.
The budget (ADR 0004, p99 <= 8.3 ms) had a factor 12 of headroom; `edit` now has 5 %.

What the reading found, by cost:

1. **About 1 ms of every frame is not attributed to anything.** `gpu_ms` times only the G-buffer pass to
   the end of the screen pass. The `expand` compute pass, the BLAS builds and the per-frame TLAS build are
   outside it, and `cpu_ms` includes waiting in `get_current_texture`. Frame 3.9 ms against 2.9 ms timed.
   (Inference from the numbers, not measured: package A measures it.)
2. **The bounce light's smoothing runs at full resolution on a half-resolution signal.** `gi_trace` sends
   one ray per 2x2 pixels, then `temporal` and four `atrous` passes run on every pixel, each tap loading
   two textures (one Rgba32Float) and doing a mat4 multiply and a normalize in `point_at`. ADR 0022's own
   numbers: 1.1 ms smoothing + 0.2 ms history of the 2.8 ms.
3. **Terrain generation evaluates the full noise stack for every column of every job, and again on every
   remesh.** `generate` calls `Body::height` for 34x34 columns: about 16 octaves of OpenSimplex each
   (three warp samples, continent, hills, ridge or isle), plus two `tan` and a normalize in `Body::dir`.
   `sea_quads` then calls it again for 32x32 columns. An edit's remesh pays all of it though no height
   changed. 78 % of `fly`'s jobs end empty or solid and still paid it. Physics pays it per column per step
   (`World::column_height`). This is where 42 µs became 931 µs.
4. **Rays are traced in the forward fragment shader, in octree order.** `fs` calls `light()` (a sun ray,
   up to 3 lamp rays, a reflection ray on metal) for every fragment that passes the depth test at the time
   it is drawn. Draws are in the selection's stack order, not front to back, so hidden fragments can be
   shaded and then overwritten. Size unknown: package D measures it with a sort.
5. **The selection walks the whole octree from the roots every frame** with a hash lookup per node and
   more per child (`resolved`, `splits`): 0.36 ms standing still, 1.5 ms in `edit`. `dispatch` snapshots
   edits (`World::edits_for`) on the frame thread.
6. **Stale BLAS live for 120 frames.** `Rt::forget` runs only when new quads land on the same offset; a
   replaced mesh usually gets a new offset, so its BLAS waits for the `KEEP_FRAMES` sweep. `edit` holds
   770 MB more VRAM than `stand`. No BLAS is compacted.
7. **Every pixel loops over all the lamps** (up to 64, with an insertion sort) and the client gathers at
   most the 64 nearest by scanning every edited chunk. A base with hundreds of lamps cannot be lit right,
   and ADR 0022 already measured 6.4 ms p99 with 31.
8. **Five times the quads the spike had**, because greedy merging is off on the curved grid. It costs the
   raster pass little; it costs BLAS memory (about 570 MB), BLAS build time while streaming, and upload.

Not audited in depth: the server tick and the water step (the soak's 1.25 ms per tick for 50 bots is
fine today). Seen in passing, for later: `ToClient::Edits` and `ToClient::Players` go to every client
whatever the distance (`crates/server/src/lib.rs` near lines 476 and 491).

## Work packages, in order

### A. Account for every millisecond (first; nothing else is trusted without it)
- GPU: a timestamp pair per stage: `expand` + BLAS + TLAS builds, the G-buffer pass, `gi_trace`,
  `temporal`, the four `atrous` together, the screen pass. Compute passes take `timestamp_writes`; the
  acceleration-structure build needs `TIMESTAMP_QUERY_INSIDE_ENCODERS` (check the adapter has it; if not,
  time it by A/B with the build skipped on a still camera).
- CPU: split `pre_acquire_ms` into the server's messages, the edit loop, `streamer.apply`, select,
  dispatch, the table build, `rt.record`, and the wait inside `get_current_texture` on its own.
- All of it in the JSON line as p50/p99, and in each `spikes` entry.
- Then name where `edit`'s 12 ms frames go (12 in a row at t = 9.5 s: cpu 12 ms, select 1.5, submit 0.9,
  the other 9 ms unnamed). Fix it if it is small; otherwise report it before going on.
- Done when: the stages sum to within 10 % of the frame, in all four runs.

### B. Stop rebuilding the TLAS for a camera that moved
Only if A shows the TLAS build (GPU plus `rt.record` on the CPU) over about 0.15 ms.
- Today every instance carries the body-to-camera transform, so the TLAS is rebuilt every frame. Rays are
  traversed in the BLAS's own space (the body's frame) anyway, so tracing in the body's frame loses no
  precision against today.
- Give every instance the identity transform and keep the mask `1 << body`. In `ray()` and `seen()` turn
  the origin and direction into the body's frame: `cam_hi + cam_lo + transpose(rot) * p`, and
  `transpose(rot) * d`. Bodies overlap at the TLAS origin; the masks keep them apart, and the moon lies
  inside the planet's unmeshed core.
- Rebuild the TLAS only on frames where the instance list changed.
- That needs the custom index to stay valid between frames: make the chunk table persistent (an index
  per ready mesh, written when the mesh is applied, freed when it is released) instead of rebuilt in draw
  order each frame. The per-frame work is then the indirect arguments only.
- Check: `bash tools/views.sh` unchanged (shadows, reflections, the moon's terrain shadows from the
  planet), and a filmstrip of `orbit` across the change of frame.

### C. Smooth the bounce at the resolution it is traced at
- `hist`, `smooth-a`, `smooth-b` at half resolution; `temporal` and `atrous` run there, guided by the
  G-buffer of the pixel each 2x2 traced this frame. Three passes (1, 2, 4 half-pixels cover what 1 to 8
  full pixels did).
- The composite upsamples with the four nearest half-res samples weighted by normal and plane distance
  against the full-res G-buffer (the weights `atrous` already uses), so edges stay sharp.
- `point_at`: replace the per-tap mat4 multiply with a direction interpolated from the four corner rays,
  passed in the uniform.
- Target: the denoise stages from about 1.3 ms to under 0.4 ms.
- **Taste call:** show the developer before/after stills of the lamp room and a sunlit slope
  (`place=` shots, `capture_w=1280`) before `views.sh accept`.

### D. Shade each pixel once
- Step 1 (ten lines): sort the visible draws front to back by the distance already computed in `select`.
  Measure the G-buffer stage. If it drops less than 10 %, overdraw is not a problem: stop here.
- Step 2, only if step 1 shows a real cost and it still matters: move `light()` out of `fs` into a
  compute pass over the G-buffer (the fragment shader writes albedo, normal, distance, material flags;
  the compute pass writes `direct`). One `light()` per visible pixel whatever the depth complexity. Water
  keeps its forward path.

### E. A height cache under generation, physics and water (the base for the new world generation)
- A per-column height is a pure function of (body, face, level, i, j). Cache it in tiles: one tile per
  chunk footprint with its padding, `[f32; 34*34]` plus min and max, keyed (body, face, level, pos.x,
  pos.y), shared (`Arc`) between the chunks stacked over it, remeshes, `sea_quads`, and at level 0
  `World::column_height` and `Solid::gen_floor`. A bounded cache, evicted by age like the slots; built on
  the workers, never on the frame thread.
- `generate` and `sea_quads` take the tile instead of calling `Body::height`. The empty / solid
  early-outs use the tile's min and max.
- Inside a tile's build, compute the `tan` per row and per column once (68 instead of 2,312).
- The heights do not change by one bit: the soak's final hash and `views.sh` stay identical. That is the
  test.
- Do not add a lattice or SIMD noise now: the developer redoes world generation next, and the cache is
  the seam that work plugs into (per-column layers computed once, then sampled by every consumer).
- Target: a remesh after an edit under 100 µs (`edit`'s `job_us_avg`), first-time jobs 30 % cheaper where
  chunks stack, `orbit` without holes at the same speed.

### F. Select when something changed, cull every frame
- Split `Streamer::select` in two. The LOD walk (which leaves, which cover) depends on the camera's
  position and the slots' states only: redo it when the camera moved more than a quarter of the finest
  chunk's split margin, or a job was applied, a block or tile touched, or a slot evicted. The frustum and
  horizon tests depend on the look: run them every frame over the cached leaf list.
- Move `edits_for` and the wet-tile lookup off the frame thread where possible (snapshot the edit maps of
  the touched level 0 chunks as `Arc`s and filter in the job).
- Target: select under 0.1 ms on a still camera, under 0.5 ms in `edit`.

### G. BLAS lifetime and size
- Drop a BLAS the frame its mesh is released: `Streamer::apply` and `evict` report released offsets,
  the client calls `Rt::forget` for each. `KEEP_FRAMES` goes.
- Then try compaction (`ALLOW_COMPACTION`, if wgpu 30 exposes it for BLAS) on chunks that have been
  still for a second; keep it only if VRAM drops by more than 25 % with no frame-time cost.
- Target: `edit` VRAM within 150 MB of `stand`.

### H. Lamps by place, not by count
- A grid of 8 m cells around the camera in each body's frame (lamps reach 8 m), each cell a range into a
  list of the lamps that can reach it; built on a worker when the edits change. The shader looks up one
  cell and runs today's strongest-3 logic over that list only.
- Keep an index of light-giving edits in `World` (updated in `set_block` / `put_edit`) so nothing scans
  the edit map; lift the cap from 64 to about 1024 within a larger reach.
- Target: 31 lamps cost what 3 do today; 500 lamps in a base hold the budget.

### I. Fewer quads: a prototype first (`prototypes/greedy-sphere/`), not engine code
- Why merging cracks today: a merged quad's edge is a straight chord, the small quads beside it follow
  the sphere through every grid point.
- The idea to test: place vertices by trilinear interpolation between exact cube-sphere positions on a
  lattice every 8 cells, and never merge across a lattice line. Along any grid-aligned edge the
  interpolation is linear, so a merged quad's edge passes exactly through its neighbours' corners. The
  shift from the true sphere is 8 * cell / R of a cell: 0.1 % at level 0, sub-pixel at every level from
  the distance that level is shown at.
- Question for the README: quads, BLAS memory and build time with merging up to 8x8, and are there
  cracks in the filmstrips at seams, cube corners and level borders? Physics keeps the exact grid.
- Adopt only with the developer's yes (**taste call**: it moves every vertex by up to a few millimetres).

## The target: `stand` at 800 fps or more (the developer, 2026-11-10)

That is a frame of 1.25 ms at 1080p on the 4 km planet. Measured after packages A to I2 (2026-11-11,
`stand quick=1`, p50, three runs): the frame is 2.03-2.17 ms (about 470 fps), against 3.6 ms before the
pass.

| stage | before the pass | now | budget for 1.25 | how |
|---|---|---|---|---|
| TLAS and BLAS (`rt`) | 0.19 | 0.00 | 0.00 | done (B) |
| G-buffer: raster, sun, lamp and reflection rays | 1.36 | 0.78 | 0.45 | J: thinner G-buffer, the eclipse terms off the pixel; D step 2 if overdraw shows |
| bounce rays (`trace`) | 0.14 | 0.18 | 0.15 | as is |
| history and smoothing (`temporal` + `atrous`) | 1.24 | 0.52 | 0.30 | J: fold `temporal` into the first pass, two passes |
| composite (`screen`) | 0.15 | 0.19 | 0.15 | reads a thinner G-buffer |
| GPU idle between frames (`gap`) | 0.51 | 0.46 | 0.05 | a second frame in flight (see below) |
| margin | | | 0.15 | |
| **frame** | **3.6** | **about 2.1** | **1.25** | |

Is it in reach without changing the look? On paper, yes, but only just, and only with two items that
have not been tried:
- The idle gap. It survived B (the CPU now hands the GPU its commands 0.8 ms sooner), so it is not late
  recording. The likeliest fix is letting the next frame's work queue while the present's copy runs.
  That does not change the picture; it adds a frame of latency, which is the developer's call.
- The G-buffer at 0.45. J's items, measured one at a time.

Nothing in the table needs a look change. If both items fall short, the half-resolution sun shadow (J's
taste call) is the reserve.

### J. The last 0.9 ms (after B, C and D, in this order, each measured)
- **A thinner G-buffer.** 28 bytes a pixel now. The distance comes from the depth buffer (reverse-Z
  Depth32Float is exact enough; check the bounce rays' origins far away), the normal as two 16-bit
  octahedral numbers, `direct` as Rg11b10Float: 12 bytes. Every later pass reads less too.
- **D's step 2 regardless of overdraw** if the direct light is still over 0.5 ms: in a compute pass the
  sun's ray, the eclipse terms and the lamp loop run once per pixel, and pixels facing away from the sun
  skip early as now.
- **The eclipse terms off the pixel.** `left_by` runs asin, atan2 and acos for 4 bodies, twice, per
  pixel; no other body is near the sun's disc most of the time. One test on the CPU per frame sets a flag
  that skips the loop.
- **Fewer smoothing passes.** With C done, try two passes and a longer history while moving; fold
  `temporal` and the first pass into one.
- **Taste call, only if still short:** anything that changes the look, such as the sun's shadow ray at
  half resolution with the same history, or the bounce at a third. Show stills first; the developer may
  prefer 600 fps with today's picture.

## Rules for the whole pass
- One package, one commit, one ADR row with before and after from the same quick runs.
- Anything that changes pixels goes through `tools/views.sh` and a filmstrip you read yourself.
- Determinism: the soak's final hash for seed 1 must not change in any package.
- If a package does not move its number, revert it and write that down. That is a result.
