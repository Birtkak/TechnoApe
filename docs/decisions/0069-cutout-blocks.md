# 0069 – Cutout blocks: see-through leaves, shrub and reed, a Leaves setting, light through the holes

- **Status:** accepted (built; step 2 of `docs/design/see-through-and-boxes.md`, on ADR 0068's measured answers)
- **Date:** 2026-11-12

## Context
The developer wants leaves and plants you see through. ADR 0068's spike measured how: light option 3 (a candidate
loop over non-opaque geometry), fancy leaves (faces between leaf blocks kept) as the default, a Leaves setting
with a fast choice, coarse levels opaque in a block's one colour. It left open whether fancy holds ADR 0004's
p99 on the forest fly-through (measured while another session compiled). This builds it for real, replacing
the spike's run switches.

## Decision
- **Data:** a block row may say `see = "cutout"` (`blocks::See`); the four leaves, shrub and reed do. A cutout
  block's painting may have pixels of alpha 0 or 255, nothing between (`textures::decode(bytes, see)` names a
  refused pixel); an opaque block's file with a not-opaque pixel is refused as before, so no texture changes
  meaning. A face's mean colour leaves its empty pixels out.
- **Pattern:** a new pattern `leaves` (11): leafy texels, and holes of `size` texels, `amount` of them, hashed by
  the texel's place on the face's *plane* (shader `leaf_hole`, Studio `pattern::hole`), so the two coplanar
  faces between neighbouring leaf blocks have their holes in the same places (the spike's review). On an opaque
  block, with fast leaves, or with flat colours a hole is the block's colour, darker (x 0.6). Unpainted leaves
  are therefore see-through; the spike's hashed stand-in is gone.
- **Mesher:** `mesh_cut(cells, lattice, cutout)`: one bit per block id (`mesh::CUT`); with `cutout` (fancy, level 0
  only) a face is culled only against an opaque neighbour, and the cutout blocks' quads go last among the land
  quads (`Mesh::cut`). Without it, and in a chunk with no cutout block, it is the old mesher bit for bit (a test
  holds both to a face-by-face reference).
- **Drawing:** the cutout quads draw in a pipeline of their own (`fs_cut`, which discards an empty texel) after
  the opaque terrain (`fs`, no discard, early depth kept). Coarse levels draw a cutout block in its one colour.
- **Light (option 3):** each chunk's BLAS has two geometries, its opaque quads flagged OPAQUE and its cutout quads
  not; `trace` in shader.wgsl runs the candidate loop and confirms a candidate unless its texel is empty
  (`hit_empty`, summer's look). Sun, moon, lamp, bounce and reflection rays all go through it. `Chunk.opq` maps a
  hit in the second geometry to its quad.
- **Setting:** Graphics, **Leaves**: fast (**the default**: the old mesher, opaque, one BLAS geometry) or fancy.
  Fancy was meant to be the default (ADR 0068); measured, its fly-through misses ADR 0004's p99 in most runs (below),
  so it ships as the choice, not the default. `leaves=fancy|fast` on a run overrides it; a change remeshes level 0 while the old meshes stay drawn.
- **Studio (Textures page):** an Eraser (X) for a cutout block, empty pixels as a checkerboard on the canvas,
  previews, thumbnails and the pattern panel; the block's Kind (Opaque, Cutout) with a line saying what each is
  for, written to its row in data/blocks.toml as one undo step; empty pixels are a reserved colour
  (`art::EMPTY`) inside the Studio, alpha 0 in the file.
- The spike's clutter models (`spike.rs`, `mesh::clutter`, `vs_mbox`) are **removed**, not kept dormant: step 6
  builds still models from ADR 0068's decision, not from the spike's hard-coded faces. Its measuring aids stay:
  `run=fly at=` and `run=meshbench at=` (now with `leaves=fancy`).

## Measured
Dev PC, release builds (fat LTO, native), off-screen, `quick=1`, the shared `out/.lock` held around the series,
3 reps each, 2026-11-12 21:07-21:25. "Today" is the build before this step (984bebf, the spike's switches off).
The PC: another session compiled at 21:07-21:08 (before these reps ran) and its runs waited on the lock; no
other game or compiler ran during these rows as far as `busy.log` saw. Forest lines as ADR 0068:
`run=stand|fly at=1125.34,3634.37,1915.81 time=23625 ...` (`prototypes/see-through/step2.txt`, `today.txt`,
`step2.sh`).

CPU first. Mesh time per chunk (`run=meshbench at=<forest> [leaves=fancy]`, the forest's 273 surface chunks,
best of five each, one thread):

| mesher | mesh us/chunk | quads/chunk | bytes/chunk |
|---|---|---|---|
| today | 56.9 | 966 | 7.7 KB |
| fast (`mesh_cut(.., false)`) | 60.3 | 966 | 7.7 KB |
| fancy | 79.2 (+39 %) | 1357 (+40 %) | 10.9 KB |

(The spike measured 102 / 137 us on a busier PC; the ratio holds.) Fast's 3.4 us is within the run-to-run spread
of this benchmark; it runs the old loop.

The frame (median of 3; p99 the best rep, every rep in the text below):

| run | frame thread work ms | gbuffer ms | trace ms | GPU ms p50 | p99 frame ms | VRAM MB | wgpu alloc MB |
|---|---|---|---|---|---|---|---|
| stand, today | 0.575 | 0.978 | 0.578 | 2.284 | 3.48 | 815 | 609 |
| stand, fast | 0.629 | 1.350 | 0.296 | 2.606 | 3.65 | 816 | 610 |
| stand, fancy | 0.626 | 2.211 | 0.682 | 3.849 | 4.81 | 816 | 638 |
| fly, today | 0.589 | 0.741 | 0.234 | 1.818 | 3.75 | 825 | 629 |
| fly, fast | 0.577 | 0.806 | 0.457 | 1.896 | 3.87 | 818 | 631 |
| fly, fancy | 0.574 | 1.833 | 0.360 | 3.055 | 6.27 | 1072 | 664 |

- **The frame thread does not change** (0.57-0.63 ms in every row).
- **Fancy on the fly-through does not hold ADR 0004.** p99 per rep: today 4.4 / 4.2 / 3.7 ms (one frame of 18.9 ms in
  rep 2), fast 3.9 / 3.9 / 4.3, fancy **6.3 / 11.9 / 10.8**, 30-50 frames over 8.3 ms a run. A rerun on the fixed
  build: 12.0 / 11.9 / 12.5. The standing forest holds it (fancy p99 4.8-5.9 ms).
- **The cause:** the slow frames are not their own work. Their GPU passes take ~3 ms and the frame thread ~2 ms,
  but `acquire` waits 8-9 ms: the GPU is busy with work outside the timed passes. They apply no chunks, and
  fancy and fast build the same number of TLASes and mesh jobs, so the cause is not uploads or streaming volume.
  It is not overdraw either: the timed g-buffer stays at 1.8 ms. A run with the cutout geometry flagged OPAQUE
  (light option 1, solid shadows) cut the tails to 5.1 / 5.4 ms in two of three runs (10.5 in the third). Most of
  it is therefore the non-opaque BLAS geometry, at a cost the timestamps do not see: the driver's build of a
  non-opaque BLAS, or the traversal around a TLAS rebuild. The rest is fancy's 40 % more geometry.
- **What was done:** fast is the default; fancy stays the choice, honest about its cost. Not solved: finding which
  untimed GPU work it is (timestamps around `build_acceleration_structures`; building cutout chunks' BLASes at a
  lower per-frame budget; or option 1 for far chunks only).
- **Fast is not quite free on the GPU:** +0.32 ms p50 standing (g-buffer +0.37: the leaves pattern's hole hash and
  the vec4 texel path), +0.08 ms flying, frame thread and p99 unchanged. Within the budget, but not zero as the
  task asked; a cheaper fast would skip `leaf_hole` when no cutout pipeline draws.
- VRAM: fancy +29-35 MB wgpu (more quads, larger BLASes); the driver's figure jumped one 256 MB block on the fly.

Pictures: `out/film-crown.png` (under a crown at noon): a block-shaped shadow beside the trunk, dappled with
specks of sun, thicker towards its edges; `out/film-edge.png` (the spawn's forest edge at noon): shrubs and crowns
show holes and inner faces; the fixed viewpoints changed only on crowns, shrubs and the far forest (one colour
on coarse levels), looked at before `views.sh accept` (seam 6.2 %, lod 3.6 %, corner and the seasons 0.9-1.9 %).

## Consequences
- See-through crowns with dappled shade under them; the cost is on the GPU and the meshing workers, none on the
  frame thread.
- A new kind or a new painting with holes reaches the game through a rebuild (blocks.toml is built in); the
  Studio's Kind tip says so.
- Rays read a leaf texel's summer look: a season whose paint moves the holes shades through summer's holes.
- Painted cutout faces are not mirrored across a shared plane, so between two painted leaf blocks the two faces'
  holes differ and light needs a hole in both.
- Glass (step 3) joins the candidate loop with a tint.

## Review
A fresh reviewer read the diff before the final commit (nothing run). It confirmed the layouts (`ChunkGpu` and
`Chunk` 32 bytes, `opq` at 20), the indirect offsets, the two geometries' vertex ranges, `hit_quad` with one
geometry either way, the hole formula in shader and Studio, the masked bit-25 readers, and that fast runs the old
mesher with one opaque geometry. Its findings:
1. `hit_empty` read the texel from the BLAS's grown corners, so a ray in the 1 cm lip read the neighbour cell's
   texel: a speckled fringe on leaf shadows. **Fixed**: the same barycentrics on the ungrown corners.
2. A new Kind or painting with holes reaches the game and the Studio's 3D view only after a rebuild (blocks.toml is
   built in). **Answered**: the Kind chips' tips say so.
3. Flat colours: a face whose middle texel was a hole drew all hole colour, and rays still went through holes of
   leaves drawn solid. **Fixed**: no holes with flat colours, in drawing and in rays.
4. Undoing "Kind: cutout" wrote opaque without the check that other files have no empty pixels. **Fixed**: undo and
   redo refuse it the same way (no test written for it).
5. The Studio's reserved empty colour `[254, 1, 253]` could be made on the Palette page; `check=1` names it.
   **Left** (low).
The review fixes were measured in the rerun above (fancy fly still 12 ms: the lip fix did not change the cause).
