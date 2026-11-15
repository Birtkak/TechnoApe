# Square blocks: is there a cube-sphere grid without the stretch?

**Question.** On the game's grid (equi-angular cube-sphere, ADR 0003) blocks are square only near a face
centre: near a face edge they are 1.4 to 1 rectangles, near a corner 60° diamonds (the developer, 2026-11-14:
"it feels like you are playing on a warped world or a wrong resolution"; in-game shots of a checkerboard floor
in `out/warp/`). Would the conformal cube-sphere, or a blend towards it, feel better, and what does it cost?

**Answer.** Yes for the shape. On the conformal grid every block top is square (stretch over 1.1 to 1 on 0.0 %
of the surface, against 72.7 % now) and no corner is off 90° by more than 5°. The price is size near the 8 cube
corners: blocks are under 0.75 m within about 450 m of a corner (2.3 % of the surface) and under 0.5 m within
about 90 m (0.1 %). Blends do not help: half way still has the diamonds and already has the small blocks.

## Measured (`python -I measure.py`, `results.json`)
The standard planet: N = 6144 columns a face edge, R = 4245 m, mean block top 1 m². Shares are of the ground.
"Stretch" is the long side over the short side of the block's top, counting skew (1 a square, 1.41 the
rectangle at an edge now, 1.73 the diamond at a corner now).

| grid | block at face centre | at a face edge | top stretch over 1.1 | over 1.25 | corner off 90° by over 5° | over 15° | blocks under 0.9 m | under 0.75 m | under 0.5 m |
|---|---|---|---|---|---|---|---|---|---|
| equi-angular (now) | 1.09 x 1.09 m | 1.09 x 0.77 m | 72.7 % | 40.0 % | 54.1 % | 16.3 % | 0 % | 0 % | 0 % |
| blend 0.25 | 1.10 | 1.08 x 0.84 | 63.4 % | 21.7 % | 44.0 % | 7.5 % | 3.4 % | 0 % | 0 % |
| blend 0.5 | 1.10 | 1.07 x 0.91 | 45.8 % | 3.7 % | 28.9 % | 1.3 % | 7.4 % | 0.4 % | 0 % |
| blend 0.75 | 1.11 | 1.06 x 0.98 | 7.5 % | 0.1 % | 7.7 % | 0.0 % | 9.4 % | 1.3 % | 0 % |
| conformal | 1.12 x 1.12 m | 1.05 x 1.05 m | 0.0 % | 0.0 % | 0.0 % | 0.0 % | 10.4 % | 2.3 % | 0.1 % |

Conformal block size against the distance to the nearest cube corner (along the face's diagonal):

| from the corner | 715 m | 284 m | 113 m | 45 m | 25 m | 10 m |
|---|---|---|---|---|---|---|
| block | 0.84 m | 0.67 m | 0.54 m | 0.43 m | 0.35 m | 0.26 to 0.32 m (not resolved) |

- **The whole block.** Layers stay 1 m tall, so a small block is a pillar: 0.5 m wide and 1 m tall at 90 m from
  a corner. Counting the height, 3.9 % of the conformal planet's blocks are over 1.25 to 1 (40.0 % now), 0.9 %
  over 1.5 to 1 (4.8 % now). At a face centre the conformal block is 1.12 m wide and 1 m tall.
- **Rows across a seam** no longer kink (the old spike: 0.7° a block at 128 blocks from a corner against 21.8°).
- **A bigger planet does not move the shares**, only the metres.

## Pictures (`images/`)
- `sheet_blocks.png`: a 13 x 13 checkerboard from straight above, one scale (the white square is 1 m), at a face
  centre, 3/4 of the way to an edge, at an edge, and 300 m and 60 m from a corner; now, blend 0.5, conformal.
- `sheet_maps.png`: one face of each grid: the top's stretch, the whole block's stretch, the block's size.

## What it would take (not measured here)
- The grid's map in the engine and in the vertex shader: now `tan`, then a table or a series. Its cost on the
  frame and on meshing is unknown.
- A new bake: the land is made by place on the sphere, and every column moves. Saves of today would not fit.
- The corners: about 90 m around each where blocks are under half size. ADR 0003 already makes the 8 corners
  landmarks; a peak, crater or lake that is not built on would cover it.
- Movement, reach and recipes count blocks, so near a corner a walk covers more blocks a second.

## The table for the in-game trial (`python -I warp_table.py`, `warp_table.bin`)
The conformal grid written as today's grid with its face coordinates warped first: 193 x 193 moves (da, db),
bilinear between them, one table for every face and planet. Exactly symmetric, no move across an edge, so the
faces share their edge points; the script checks that and that the block 100 columns from a corner is 0.43 m.
Inside the last table cell before a corner the blocks level off (about 0.26 to 0.32 m) rather than shrink to
nothing.

## Limits
- The conformal table is the old spike's (`../sphere-grid/smooth.cpp`, 192 points a face edge, kept here as
  `conformal_face.npy`): one table cell is 32 blocks, so nothing within about 30 m of a corner is resolved, and
  the wobble in the corner checkerboards is the table, not the grid. The engine would need the exact map.
- Flat tops only: no terrain, no walls, nothing seen at eye level or moving.
- The `measure.py table` step needs `../sphere-grid/out/harmonic.tab`, which `smooth.exe` writes.
