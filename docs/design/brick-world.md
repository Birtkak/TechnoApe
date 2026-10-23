# Brick world (ADR 0012, option B)

- **Status:** design, 2026-10-22; P9 (`prototypes/p5-sim-spike/src/brick_store.*`,
  bench in `prototypes/p7-raycast-near/brick_bench.gd`) measures it. Steps 1-3
  below are done (ADR 0021, 2026-10-23: the game's near field renders from a
  wrapped store the simulation bubble writes; `near_field=1`).

## The one data model

A **cell** is 6.25 cm (1/8 of the 0.5 m voxel of ADR 0004). A **brick** is
8×8×8 cells = exactly one of today's voxels. A brick is either

- **uniform**: one material id (the whole brick is stone, air, water…). This
  is what the generator produces and what the far world is; it costs one
  reference word, or
- **expanded**: 512 cells with their own material ids, allocated from a pool.
  Chiseled blocks, the surface of a simulated pool, a burning bush, the rim
  of a crater.

The world therefore stores what it stores today (one id per 0.5 m) and
spends memory only where fine detail exists. Collapsing an expanded brick
whose cells are all one material (and, in the sim, settled) returns it to a
uniform brick.

| scale | bricks | cells | refs (4 B) | if 5 % expanded (512 B each) |
|---|---|---|---|---|
| 32 m cube (near field) | 262 144 | 134 M | 1 MB | 6.7 MB |
| 64 m cube | 2.1 M | 1.07 G | 8 MB | 54 MB |
| today's 16³ voxel block | 4096 | 2.1 M | 16 KB | 105 KB |

## Who reads what

- **Far field (godot_voxel, LOD, 2 km):** reads bricks as voxels: a uniform
  brick is its material; an expanded brick reports its majority material.
  Nothing changes in the mesher or the generator path; the generator's
  `generate_block` output *is* a block of uniform bricks.
- **Near field (ray-cast, 32 m):** reads cells. Uniform bricks are sent as
  one byte and expanded ones as 512; the GPU scatter writes both into the
  volume texture; brick occupancy is a by-product (uniform air = empty).
- **Simulation:** ticks expanded bricks in its active set only. Uniform
  bricks are walls or air to it; a reaction or a moving cell at the border
  of a uniform brick expands that brick first. A settled homogeneous brick
  collapses. (The sim chapter of the research doc decides the kernel's own
  layout; the store exposes bricks and 16³ chunk views so either works.)
- **Building:** placing a block writes a uniform brick; chiseling expands a
  brick and edits cells; a slab or stair is a chiseled brick pattern. The
  chisel block entity of ADR 0006 is no longer needed for shape, only for
  function (containers etc. stay entities).
- **Mass edits** (spells, TNT): a C++ carve over bricks, uniform bricks
  inside the shape are rewritten as one word, only rim bricks expand. The
  store reports dirty bricks; the feed and the far mesher consume them under
  a per-frame byte budget and queue the rest (ADR 0012 rule).

## API of the store (P9 spike, GDScript-facing)

```
configure(size_bricks: Vector3i)
load_voxels(origin_brick: Vector3i, size: Vector3i, ids: PackedByteArray)  # 16-bit ids, godot_voxel order -> uniform bricks
brick_material(b) -> int (-1 if expanded) ; is_expanded(b)
expand(b) ; collapse(b) -> bool ; collapse_all() -> int
get_cell(c) / set_cell(c, mat)          # cell coords; set expands as needed
fill_cells(min, max, mat) ; carve_sphere(centre_cell, radius_cells, mat)
get_chunk_bytes(chunk) -> PackedByteArray(4096)   # 16^3 cells, SimWorld layout, for the GPU feed and the sim
get_dirty_bricks() / clear_dirty()
stats() -> {bricks, expanded, pool_bytes, ref_bytes, uniform_ratio}
```

## Streaming, saving, network

- The store is a moving window of bricks around each player (client) or
  each loaded region (server), filled from the generator and from saved
  regions. Saved regions keep uniform bricks as today's voxel ids and
  expanded bricks as 512-byte blobs behind a brick index; unchanged
  generated bricks are not saved at all (seed + GEN_VERSION regenerate).
- Deltas over the wire are dirty bricks: 4 bytes for a uniform brick, 512
  for an expanded one, both compressible.
- Determinism: the server's simulation owns expanded bricks; clients apply
  deltas and run only presentation-side prediction.

## Migration from today

1. P9: the store + bench (memory, expand/collapse, cell writes, carve, chunk
   feed). Gate: within the ADR 0009 budgets.
2. The P7 caster fed from the store instead of SimWorld chunks; the player
   stands in a brick world in the prototype.
3. In the game (ADR 0021): the sim bubble owns the writes into a wrapped
   `BrickStore` (every loaded chunk, every tick's changes, micro patterns
   expanded); godot_voxel keeps its own voxel data for the far field, fed by
   the bubble's pushes as before; the near field renders from the store
   through `NearFieldFeed` + `near_scatter.glsl` + `near_field.gdshader`.
   3b (open): the far field reads the store's majority view instead of its
   own data, so LOD 0 can stop at the near field's edge.
4. Chisel tool: expand + edit cells; vegetation micro models become
   expanded bricks produced by the generator (no library models needed).
