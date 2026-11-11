# Greedy quads on the sphere (ADR 0025, package I)

**Adopted (2026-11-11) as ADR 0025's I2:** the developer chose the lattice. In the engine it is capped by level
(`Body::lattice`: 8 cells, halved where the sag would pass half a pixel), and merging is on by default.

**Question.** The mesher can already merge faces (`greedy=1`), but it is off by default. A merged quad's edge
is a straight chord. The small quads beside it follow the sphere through every grid point, so the fear was
cracks (ADR 0018 counted 406 crack pixels with `greedy=1`).

The idea to test is a lattice:
- vertices are placed by trilinear interpolation between exact grid points on a lattice every 8 cells;
- merges never cross a lattice line;
- so a merged edge lies inside one lattice cell, where the interpolation is linear, and passes exactly
  through its neighbours' corners.

The README had to answer four things for merging up to 8×8:
- how many quads;
- BLAS memory;
- BLAS build time;
- whether there are cracks at face seams, cube corners and level borders.

Physics keeps the exact grid either way.

## What was built

- **`lattice.patch`** (against commit 9023b49), the lattice variant. It is not in the engine. It changes two
  things:
  - In `mesh.rs`, a merge stops at every 8th cell.
  - In `shader.wgsl`, a new `place()` interpolates the four lattice columns and two lattice layers around a
    vertex. Both the drawn vertices (`vs`) and the traced ones (`expand`) use it, so the BLAS matches the
    picture.

  Apply it with `git apply prototypes/greedy-sphere/lattice.patch` and run with `greedy=1`.
- **`measure.sh <tag>`** runs one build with greedy off and on:
  - the meshbench's quads per chunk (level 0, around the spawn);
  - the quick `stand`, `fly` and `orbit` runs' stages and wgpu's allocated GPU memory (`gpu_alloc_mb`);
  - the six fixed viewpoints' crack counts.
- **`cracks.sh <exe> <tag> greedy=1`** hunts cracks close up. It takes shots at a face seam, a cube corner
  and the level border of the `lod` view:
  - eyes at 1.7, 12 and 60 m, looking 12° down;
  - then eyes at 1.7, 60, 400 and 1500 m, looking 2° down (grazing);
  - four directions each.

  Each shot's cracks are counted at full resolution: the clear colour wherever a ray must meet a body
  (ADR 0018).

"Chord" below is today's `greedy=1`, unlimited merges with exact corners. "Lattice" is the patch.

## Measured (dev PC, 1080p, 4 km planet, quick runs, two reps where noted)

| | no merging (today) | chord (`greedy=1`) | lattice (patch, `greedy=1`) |
|---|---|---|---|
| quads per level-0 chunk (meshbench) | 1261 | 841 (−33 %) | 866 (−31 %) |
| `fly` G-buffer p50, ms (2 reps) | 1.28 / 1.33 | 0.75 / 0.74 | 0.73 / 0.74 |
| `fly` frame p50 / p99, ms (2 reps) | 2.61 / 5.19, 2.66 / 4.90 | 2.29 / 4.21, 2.23 / 3.81 | 2.18 / 3.78, 2.24 / 3.84 |
| BLAS builds, `fly` `rt` stage p99, ms | 0.50 | 0.43 | 0.43 |
| GPU memory allocated, MB (`stand` / `fly` / `orbit`) | 644 / 697 / 736 | 511 / 443 / 554 | 517 / 450 / 575 |
| crack pixels, six fixed views | 0 | 0 | 0 |
| shots with crack pixels, `cracks.sh` (36 at 12°, 48 grazing) | not run | 0 of 84 | 0 of 84 |
| pixels off the baseline, `orbit` view | 0.003 % | 2.15 % | 1.73 % (1.68 % unmerged) |

`stand` is not in the table: its bounce trace flips between 0.18 and 0.39 ms from run to run in every build,
which hides a difference of this size. In `stand` the G-buffer moved 0.98 → 0.91 ms with chord and was 0.79
with the lattice, but in runs where the trace sat in its fast mode.

## Answer

- **Merging pays.** About a third fewer quads. In `fly`, 0.55 ms less G-buffer, a p99 about 1 ms lower,
  15 % cheaper BLAS builds, and 130 to 250 MB less GPU memory.
- **The lattice is not needed against cracks at 1080p.** Chord merging counts 0 crack pixels in all 90
  shots, the same as the lattice. ADR 0018's 406 pixels came from the skirt bug fixed on 2026-11-10, not
  from chords. A chord's T-junction gap is a height step along a shared edge: at most 8 mm at level 0. It
  shows only edge-on and stays under a pixel.
- **What the lattice buys is smaller shifts.** Both variants move the drawn surface off the exact grid by
  a sagitta, L² / 8R with R ≈ 3.9 km:

  | level | chord, longest merge | chord shift | lattice, 8 cells | lattice shift |
  |---|---|---|---|---|
  | 0 | 32 cells = 16 m | 8 mm | 4 m | 0.5 mm |
  | 4 | 256 m | 2.1 m | 64 m | 13 cm |
  | 6 | 1 km | 34 m | 256 m | 2.1 m |

  The far levels are where it shows: 2.15 % (chord) and 1.73 % (lattice) of the `orbit` view's pixels
  move. Most of the lattice's 1.73 % comes from the placement itself (1.68 % even unmerged). The lattice
  should shrink with the level, for example to a fixed number of metres.
- **Cost.** The lattice's `place()` calls `frame_at` four times per vertex instead of once. With a third
  fewer vertices the frame times come out level with chord.

**Not checked:**
- sparkle at T-junctions in motion (the counter counts still frames);
- the moon;
- the huge planet;
- shadows on merged faces up close: the traced geometry is the merged quads, grown by 0.01 cell as today.

## For the developer (taste call)

Turning merging on moves every drawn vertex, by millimetres near you and by metres far away. Three ways:
- **(a) Chord** (`greedy=1` as it is): no new code, the largest far shifts.
- **(b) The lattice**: this patch, with its lattice capped in metres at the far levels.
- **(c) Leave merging off.**
