# 0021 – The ray-cast near field in the game (ADR 0012 step 3)

- **Status:** accepted for the flag; the default stays off until the user plays it
- **Date:** 2026-10-23 (the user: "Go" on "spend the Fable credits on ADR 0012
  step 3, wiring the ray-cast near field into the real game")

## Context
ADR 0012 chose option B: the 6.25 cm cell is the world grid, stored as bricks
(P9), simulated at micro resolution (P10), ray-cast within 32 m (P7) and
meshed with LOD beyond. Every spike passed in isolation; nothing had been
integrated. The shader for it had been copied into `game/` the day before and
wired to nothing. The one untested piece ADR 0012 names is the seam at 32 m
between the ray-cast cells and the meshed voxels, and the budget of the whole
(ADR 0009: p99 ≤ 8.3 ms, no frame > 16.7 ms, VRAM ≤ 6 GB) with both renderers
running. The render thread rule (ADR 0016) forbids driving the shared
`RenderingDevice` from the main thread, which P7's scripts did.

## Decision
`near_field=1` (`Settings.near_field`, off by default) turns on
`scripts/visuals/near_field.gd`:

- **The bubble is the store's writer.** `SimBubble` already wrote every tick's
  changes into a `BrickStore` (step 3's bridge); it now also writes every
  chunk it loads. The store is a toroidal window of 128³ bricks addressed by
  world brick coordinates (`BrickStore.set_wrap_origin`; slot = brick & 127),
  so sliding it with the bubble costs nothing. Micro vegetation and detail
  (`materials.gd` `micro:` kinds) are written as expanded bricks holding the
  same cell patterns the MicroMesher models use (`set_micro_patterns`), so a
  tuft of grass is the same tuft on both sides of the seam. godot_voxel keeps
  its own voxel data for the far field, as today; the near field shows what
  the simulation holds, which is ahead of the meshes by one push.
- **`NearFieldFeed` (native) decides what to upload.** A toroidal GPU window
  of n³ cells (512 = 32 m, 256 = 16 m) follows the camera in whole 16-cell
  chunks (1 m). Sliding queues the plane of chunks that entered; store dirty
  bricks queue their chunks. `pop_upload(max_bytes)` packs pending planes
  first (innermost first, so the valid box grows back), then dirty chunks,
  within `near_upload_mb` (8 MB) per frame; the rest waits. Its **valid box**
  is the part of the window whose texels hold the current world: on a slide
  it shrinks by the pending slabs and grows as they land. A full reset (a
  teleport) queues one axis of planes, not three.
- **GPU path on the render thread.** `near_scatter.glsl` (one workgroup per
  chunk) transposes the chunk bytes into the R8 2D-array volume and writes the
  chunk's eight brick-occupancy texels in the same pass. All RD calls, and the
  `Texture2DArrayRD` wrappers, are created and used inside
  `RenderingServer.call_on_render_thread` callables; three staging buffer sets
  rotate per upload. `--render-thread safe` runs them inline.
- **Godot lights it.** `near_field.gdshader` marches the valid box only
  (two-level DDA as in P7) and writes `ALBEDO`, `NORMAL`, `EMISSION`, `DEPTH`
  and `LIGHT_VERTEX` (the hit position), so the sun, its shadow maps, the sky
  ambient, SSAO and fog are the same as on the meshes. It casts no shadow
  itself: the terrain meshes inside its box still render into the shadow
  passes (the discard below skips orthographic projections), so the near
  field receives the shadows of the geometry it replaces. Water continues the
  ray to the solid behind it and tints by depth, with the liquid shader's
  ripple normal and specular on top faces.
- **The meshes inside the box are hidden per pixel.** `terrain_common`'s
  `near_field_discard(wpos, PROJECTION_MATRIX)` drops fragments inside the
  valid box (inset by 2 mm so faces on its planes stay with the mesh); the
  node sets `near_min`/`near_max` on the atlas and liquid materials every
  frame. The prop material (items, viewmodel) never discards.
- **Checks for Claude.** `near_debug=1` tints ray-cast pixels magenta;
  `near_verify=1` reads volume and occupancy layers back once the window is
  uploaded and the bubble loaded, compares 18,000 sampled cells with the
  store and prints one JSON line (`bad` must be 0); the autopilot JSON carries
  `near_field` (uploads, MB, pending slabs/chunks, valid box, store stats).
  gdUnit: `test_near_field_feed.gd` (wrapped store, micro patterns, slabs and
  valid box under a budget, dirty chunks inside the window only).

## What was found on the way
- **Stale sampler wrappers.** `Texture2DArrayRD` wrappers made on the main
  thread after the render thread had created the RD textures bound a stale
  texture about one run in three: the volume read back correct, the shader
  marched garbage (9 ms of GPU), the near field drew nothing while the meshes
  inside its box were hidden (one sees the caves 16 m down), and one
  `TextureStorage::Texture` leaked at exit. Made on the render thread right
  after the RD textures: 6 of 6 runs clean, no leak.
- **Freeing RD resources at exit** needs `RenderingServer.force_sync()` after
  the `call_on_render_thread` that frees them, or they leak.
- **Packing** 8 MB of chunks from the store takes 6-7 ms on the main thread
  (the first fill and a teleport, 16 frames); in play it is under 1.2 ms at
  p99 even at 40 m/s (table). Packing on a worker thread is the next step if
  the first fill ever shows: today the loading screen covers it.
- The 16 m cube (256³) is almost free; the seam at 8 m from the camera is
  invisible without the debug tint: grass tufts, trunks and canopy continue
  across it (`game/out/nf_dbg16/final.png` in the session, the magenta half
  is the near field).

## Measured (dev box, 1280x720, vsync off, 20 s after load, `near_field=1`, 32 m cube)
Budget matrix (ADR 0014 scenarios; one Godot on the machine, `other_godot_processes` 0 in every row):

| scenario | p50 | p99 | max | > 8.3 ms | > 16.7 ms | VRAM MB | uploads / MB in 20 s | pack ms p99 |
|---|---|---|---|---|---|---|---|---|
| fly 6 m/s, 30 m up | 3.16 | 3.96 | 4.79 | 0 | 0 | 679 | 153 / 676 | – |
| fast flight 40 m/s | 3.70 | 5.09 | 10.53 | 4 | 0 | 1182 | 947 / 3860 | 1.2 |
| walk + mine + place | 3.03 | 3.43 | 3.84 | 0 | 0 | 936 | 31 / 174 | 0.01 |
| forest fire (`fire_test=14`) | 4.18 | 4.97 | 6.42 | 0 | 0 | 1274 | 406 / 234 | 0.12 |
| water spill (1.5 m ball / 2 s) | 3.37 | 4.19 | 4.83 | 0 | 0 | 938 | 568 / 558 | 0.29 |
| 8 m blast / 2 s | 3.33 | 3.85 | 5.22 | 0 | 0 | 929 | 358 / 356 | – |
| 50 × 1 m craters / 2 s | 3.30 | 3.79 | 4.59 | 0 | 0 | 934 | 40 / 176 | 0.02 |

Every scenario is within the ADR 0009 budget. The 32 m volume adds 128 MB of
VRAM (plus 48 MB of staging buffers); the fast flight uploads 190 MB/s (a
1 m slab of 4 MB per metre of travel per axis) with the frame's packing under
1.2 ms at p99, and its four frames over 8.3 ms are render-thread time (the
uploads) with no main-thread or GPU share.

The cost of the near field itself, standard walk (12 s): off p50 2.36 / p99
3.00 / max 3.69 ms, 621 MB VRAM; on 3.15 / 4.16 / 5.24 ms, 811 MB. Standing
still: 32 m cube p99 3.8 ms, 16 m cube 3.5 ms. `--render-thread safe`: p99
3.9 ms and the readback check clean, so the callables run inline there.

The readback check (`near_verify=1`) was clean (0 of 18,000 cells, 0 of 1,200
bricks wrong) in every run after the wrapper fix, 32 m and 16 m, both upload
budgets (4 and 8 MB), standing and walking. The first fill of the 32 m window
is 128 MB in 16 frames of 8 MB; the whole run uploads about 174 MB standing
(fill plus the bubble's loads).

The peer session's slow walk run of the same morning (p99 10.5 ms) was the
overlap with this matrix; on a quiet machine the flag off costs nothing
(3.00 ms above, same as before this change).

## Consequences
- Collision, mining, placing and the simulation are unchanged: they never used
  the meshes (ADR 0011). The near field is presentation only.
- Micro cells are painted, not material: a cell of an expanded micro brick
  holds `128 + atlas tile` (the MicroMesher paint), so reeds, petals and
  mushroom stems keep their tiles; the shader reads the tile straight from
  the id. Such a brick's material is not recoverable from its cells (its
  majority is air, which is how the far field treats micro bricks anyway);
  a reader of the brick world treats ids >= 128 as decoration.
- The near-field water is tinted (P7's depth mix) where the mesh water is
  alpha-blended with a ripple glint; at a lake shore the two blues differ a
  little across the seam. A taste call for when the flag goes default.
- Stains (ADR 0015) and the animated water/lava/fire tiles are not yet in the
  near-field shader; both are shader work on the existing `surface_stain` and
  `terrain_sample` helpers.
- Fluid levels (the peer's change of the same morning: a partly full liquid
  voxel is stored in the terrain as `FLUID_LEVEL_BASE + slot * 8 + eighths - 1`,
  256 and up) never reach the store as raw values: loaded chunks go through
  `SimWorld.write_chunk_to_store`, which has already decoded them, and both
  write paths store a partly full liquid as an expanded brick with its bottom
  `eighths` cell layers filled (`BrickStore.set_brick_filled`), so the near
  field shows the same water level the fluid meshes do, in eight steps.
  `test_brick_bridge.gd` covers it.
- Terrain meshes inside the box are still built and drawn (discarded per
  pixel): the LOD 0 radius cannot shrink below the box until the far field
  reads the store (brick-world.md step 3b).
- `game/shaders/occ.glsl` and `scatter.glsl` (untracked copies of P7 files
  from the day before) are unused; `near_scatter.glsl` replaces both.
- Open: chiseling as brick expansion (ADR 0006's overlay meshes go) is the
  next step of ADR 0012; it needs an edit path from the player into the
  store's cells and the far field's majority view.
