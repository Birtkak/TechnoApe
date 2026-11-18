# Deep shells: can blocks stay near 1 m all the way to the centre?

**Question.** The developer wants to dig to and through the centre. The grid's layers are log-spaced so a cell at
radius r is r/R m wide and thick (`crates/engine/src/world/body.rs`), so blocks shrink with depth. `docs/design/world-data.md`
("Digging to the centre") proposes shells: below a set radius a new shell starts with half the columns a face edge,
so blocks grow back to full size, and 2 x 2 upper columns sit on one lower column at the seam. Can shells reach the
centre with blocks near 1 m, what do the numbers look like, and what does a seam and the centre look like to a player?

**Answer.** Yes, to a ball about 33 m (variant A) or 47 m (variant B) in radius, on every planet size. Blocks stay
between 0.5 and 1.0 m (A) or 0.71 and 1.41 m (B), with the same 2 to 1 jump at every seam. The ball is where shell n_k
reaches 3 chunks a face edge (96 columns); one more halving is 1.5 chunks. The ball can be left solid (a), filled with a
Cartesian cube (b), or the shells can go on halving with a chunk that is as wide as the face edge (c). **Recommended: B,
and (c).** The reasons are at the end.

## Method
Pure arithmetic and drawing (`shells.py`, `pictures.py`; Python, numpy, Pillow; nothing from the engine is run).
- R = n / sqrt(2 pi / 3), as `Body::new`. A cell at radius r in shell k is 2^k r / R wide and thick (the mean size; the
  equi-angular grid is 1.09 m at a face centre and 0.77 m at a face edge, the pictures of the section use the mean).
- Variant A: shell k runs from R/2^k to R/2^(k+1) and has n/2^k columns a face edge. Variant B: shell 0 runs from R to
  R/sqrt2, shell k >= 1 from R/(sqrt2 2^(k-1)) to R/(sqrt2 2^k).
- Layers are log-spaced: L = ceil(ln(top/bottom) / ln(1 + 2^k/R)); cells = 6 n_k^2 L. CS = 32 (`world/mod.rs`), so a
  shell needs n_k divisible by 32. The list stops at the first shell that is not. Luna of the standard planet has
  n = 1664 (`bake/luna.rs`, `LUNA_N`), 52 chunks, which halves only to 13.
- The cells are counts of addresses, not of stored blocks. Nothing is stored for air or untouched rock.

<!-- tables -->
### Shell tables (`python -I shells.py`)

**small, variant A (seams at R/2, R/4, ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 530.7 | 265.3 | 768 | 24 | 1.00 / 0.50 | 369 | 1.31e+09 |
| 1 | 265.3 | 132.7 | 384 | 12 | 1.00 / 0.50 | 185 | 1.64e+08 |
| 2 | 132.7 | 66.3 | 192 | 6 | 1.00 / 0.50 | 93 | 2.06e+07 |
| 3 | 66.3 | 33.2 | 96 | 3 | 1.00 / 0.50 | 47 | 2.6e+06 |
| total | 530.7 | 33.2 | | | | 694 | 1.49e+09 |

n = 768, R = 530.7 m. 4 shells, 694 layers from the surface, 1.49e+09 cells. The shell after the last would have 48 columns a face edge = 1.5 chunks, not a whole number. **Ball left at the centre: radius 33.2 m** (498 m below the surface).

**small, variant B (seams at R/sqrt2, R/(2 sqrt2), ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 530.7 | 375.2 | 768 | 24 | 1.00 / 0.71 | 185 | 6.55e+08 |
| 1 | 375.2 | 187.6 | 384 | 12 | 1.41 / 0.71 | 185 | 1.64e+08 |
| 2 | 187.6 | 93.8 | 192 | 6 | 1.41 / 0.71 | 93 | 2.06e+07 |
| 3 | 93.8 | 46.9 | 96 | 3 | 1.41 / 0.71 | 47 | 2.6e+06 |
| total | 530.7 | 46.9 | | | | 510 | 8.42e+08 |

n = 768, R = 530.7 m. 4 shells, 510 layers from the surface, 8.42e+08 cells. The shell after the last would have 48 columns a face edge = 1.5 chunks, not a whole number. **Ball left at the centre: radius 46.9 m** (484 m below the surface).

**standard, variant A (seams at R/2, R/4, ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 4,245 | 2,123 | 6144 | 192 | 1.00 / 0.50 | 2,944 | 6.67e+11 |
| 1 | 2,123 | 1,061 | 3072 | 96 | 1.00 / 0.50 | 1,472 | 8.33e+10 |
| 2 | 1,061 | 530.7 | 1536 | 48 | 1.00 / 0.50 | 737 | 1.04e+10 |
| 3 | 530.7 | 265.3 | 768 | 24 | 1.00 / 0.50 | 369 | 1.31e+09 |
| 4 | 265.3 | 132.7 | 384 | 12 | 1.00 / 0.50 | 185 | 1.64e+08 |
| 5 | 132.7 | 66.3 | 192 | 6 | 1.00 / 0.50 | 93 | 2.06e+07 |
| 6 | 66.3 | 33.2 | 96 | 3 | 1.00 / 0.50 | 47 | 2.6e+06 |
| total | 4,245 | 33.2 | | | | 5,847 | 7.62e+11 |

n = 6144, R = 4,245.4 m. 7 shells, 5,847 layers from the surface, 7.62e+11 cells. The shell after the last would have 48 columns a face edge = 1.5 chunks, not a whole number. **Ball left at the centre: radius 33.2 m** (4,212 m below the surface).

**standard, variant B (seams at R/sqrt2, R/(2 sqrt2), ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 4,245 | 3,002 | 6144 | 192 | 1.00 / 0.71 | 1,472 | 3.33e+11 |
| 1 | 3,002 | 1,501 | 3072 | 96 | 1.41 / 0.71 | 1,472 | 8.33e+10 |
| 2 | 1,501 | 750.5 | 1536 | 48 | 1.41 / 0.71 | 737 | 1.04e+10 |
| 3 | 750.5 | 375.2 | 768 | 24 | 1.41 / 0.71 | 369 | 1.31e+09 |
| 4 | 375.2 | 187.6 | 384 | 12 | 1.41 / 0.71 | 185 | 1.64e+08 |
| 5 | 187.6 | 93.8 | 192 | 6 | 1.41 / 0.71 | 93 | 2.06e+07 |
| 6 | 93.8 | 46.9 | 96 | 3 | 1.41 / 0.71 | 47 | 2.6e+06 |
| total | 4,245 | 46.9 | | | | 4,375 | 4.29e+11 |

n = 6144, R = 4,245.4 m. 7 shells, 4,375 layers from the surface, 4.29e+11 cells. The shell after the last would have 48 columns a face edge = 1.5 chunks, not a whole number. **Ball left at the centre: radius 46.9 m** (4,199 m below the surface).

**huge, variant A (seams at R/2, R/4, ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 8,491 | 4,245 | 12288 | 384 | 1.00 / 0.50 | 5,886 | 5.33e+12 |
| 1 | 4,245 | 2,123 | 6144 | 192 | 1.00 / 0.50 | 2,944 | 6.67e+11 |
| 2 | 2,123 | 1,061 | 3072 | 96 | 1.00 / 0.50 | 1,472 | 8.33e+10 |
| 3 | 1,061 | 530.7 | 1536 | 48 | 1.00 / 0.50 | 737 | 1.04e+10 |
| 4 | 530.7 | 265.3 | 768 | 24 | 1.00 / 0.50 | 369 | 1.31e+09 |
| 5 | 265.3 | 132.7 | 384 | 12 | 1.00 / 0.50 | 185 | 1.64e+08 |
| 6 | 132.7 | 66.3 | 192 | 6 | 1.00 / 0.50 | 93 | 2.06e+07 |
| 7 | 66.3 | 33.2 | 96 | 3 | 1.00 / 0.50 | 47 | 2.6e+06 |
| total | 8,491 | 33.2 | | | | 11,733 | 6.09e+12 |

n = 12288, R = 8,490.9 m. 8 shells, 11,733 layers from the surface, 6.09e+12 cells. The shell after the last would have 48 columns a face edge = 1.5 chunks, not a whole number. **Ball left at the centre: radius 33.2 m** (8,458 m below the surface).

**huge, variant B (seams at R/sqrt2, R/(2 sqrt2), ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 8,491 | 6,004 | 12288 | 384 | 1.00 / 0.71 | 2,943 | 2.67e+12 |
| 1 | 6,004 | 3,002 | 6144 | 192 | 1.41 / 0.71 | 2,944 | 6.67e+11 |
| 2 | 3,002 | 1,501 | 3072 | 96 | 1.41 / 0.71 | 1,472 | 8.33e+10 |
| 3 | 1,501 | 750.5 | 1536 | 48 | 1.41 / 0.71 | 737 | 1.04e+10 |
| 4 | 750.5 | 375.2 | 768 | 24 | 1.41 / 0.71 | 369 | 1.31e+09 |
| 5 | 375.2 | 187.6 | 384 | 12 | 1.41 / 0.71 | 185 | 1.64e+08 |
| 6 | 187.6 | 93.8 | 192 | 6 | 1.41 / 0.71 | 93 | 2.06e+07 |
| 7 | 93.8 | 46.9 | 96 | 3 | 1.41 / 0.71 | 47 | 2.6e+06 |
| total | 8,491 | 46.9 | | | | 8,790 | 3.43e+12 |

n = 12288, R = 8,490.9 m. 8 shells, 8,790 layers from the surface, 3.43e+12 cells. The shell after the last would have 48 columns a face edge = 1.5 chunks, not a whole number. **Ball left at the centre: radius 46.9 m** (8,444 m below the surface).

**Luna (standard), variant A (seams at R/2, R/4, ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 1,150 | 574.9 | 1664 | 52 | 1.00 / 0.50 | 798 | 1.33e+10 |
| 1 | 574.9 | 287.5 | 832 | 26 | 1.00 / 0.50 | 399 | 1.66e+09 |
| 2 | 287.5 | 143.7 | 416 | 13 | 1.00 / 0.50 | 200 | 2.08e+08 |
| total | 1,150 | 143.7 | | | | 1,397 | 1.51e+10 |

n = 1664, R = 1,149.8 m. 3 shells, 1,397 layers from the surface, 1.51e+10 cells. The shell after the last would have 208 columns a face edge = 6.5 chunks, not a whole number. **Ball left at the centre: radius 143.7 m** (1,006 m below the surface).

**Luna (standard), variant B (seams at R/sqrt2, R/(2 sqrt2), ...)**

| shell | top r (m) | bottom r (m) | columns a face edge | chunks a face edge | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|
| 0 | 1,150 | 813.0 | 1664 | 52 | 1.00 / 0.71 | 399 | 6.63e+09 |
| 1 | 813.0 | 406.5 | 832 | 26 | 1.41 / 0.71 | 399 | 1.66e+09 |
| 2 | 406.5 | 203.3 | 416 | 13 | 1.41 / 0.71 | 200 | 2.08e+08 |
| total | 1,150 | 203.3 | | | | 998 | 8.49e+09 |

n = 1664, R = 1,149.8 m. 3 shells, 998 layers from the surface, 8.49e+09 cells. The shell after the last would have 208 columns a face edge = 6.5 chunks, not a whole number. **Ball left at the centre: radius 203.3 m** (947 m below the surface).

### The centre ball, variant A (`python -I shells.py`)

| planet | ball radius (m) | (a) ball volume (m3), share of planet | (b) cube edge (blocks), blocks, corner at (m) | (c) shells to 3 columns, extra layers, cells, last radius (m) |
|---|---|---|---|---|
| small | 33.2 | 1.53e+05, 2.4e-04 | 66, 287,496, 57.2 | 5, 49, 3.8e+05, 1.04 |
| standard | 33.2 | 1.53e+05, 4.8e-07 | 66, 287,496, 57.2 | 5, 49, 3.8e+05, 1.04 |
| huge | 33.2 | 1.53e+05, 6.0e-08 | 66, 287,496, 57.2 | 5, 49, 3.8e+05, 1.04 |
| Luna (standard) | 143.7 | 1.24e+07, 2.0e-03 | 286, 23,393,656, 247.7 | 5, 197, 2.97e+07, 4.49 |

Option (b), standard planet: the cube's face against the innermost shell (cell = the shell's cell where it is cut by the cube face):

| where on the cube face | distance from the centre (m) | shell cell width (m) | along the tilt (m) | footprint on the face (m2, a block is 1) | shell cells across one block edge | shell left above that point (m) |
|---|---|---|---|---|---|---|
| face centre | 33.0 | 0.50 | 0.50 | 0.25 | 2.01 | 33.3 |
| mid of an edge | 46.7 | 0.70 | 0.99 | 0.70 | 1.42 | 19.7 |
| cube corner | 57.2 | 0.86 | 1.49 | 1.29 | 1.16 | 9.2 |

<!-- /tables -->

## Pictures (`out/`, `python -I pictures.py`)
Light and dark checkered cells, a 1 m scale bar and a 1.8 m player in each.
- [`section-A.png`](out/section-A.png), [`section-B.png`](out/section-B.png): a section across the first seam, 24 x 16 m,
  cells to scale, a shaft 2 lower cells wide dug down 5 layers below the seam (tan upper shell, blue lower shell, red seam).
  In A the upper cells are 0.50 m and the lower 1.00 m; in B 0.71 and 1.41 m.
- [`centre-a.png`](out/centre-a.png), [`centre-b.png`](out/centre-b.png), [`centre-c.png`](out/centre-c.png): the
  innermost 80 m through the centre (the plane through faces +-x and +-z, so the face edges show as the corners where the
  column rows change), variant A, a 2 m shaft from the top, the player at its end.
- [`whole.png`](out/whole.png): the whole standard planet, every seam a ring, both variants, with each shell's radii and
  block sizes. The inner rings are a few pixels; the pictures above show them large.

## The three ways to fill the centre ball (variant A, standard planet)
- **(a) A solid, undiggable ball** of 33.2 m radius (1.5e5 m3, 4.8e-7 of the planet). Nothing to build: the last shell
  ends on it, the world rule is "bedrock core". Cost: you cannot reach the centre, only come within 33 m, and the
  ball is a visible hole in a rule ("dig through the planet") that the developer asked for. On Luna the ball is 144 m
  (A) or 203 m (B) because its 52 chunks halve only twice; that is 2e-3 of Luna's volume and a ball you can stand beside.
- **(b) A Cartesian cube of 1 m blocks** whose faces touch the ball: 66 blocks a side, 287,496 blocks, corners 57 m from the
  centre (24 m into the last shell, which the cube replaces; 9 m of shell is left above a corner). Pros: 1 m blocks
  right to the middle, gravity and lighting in the middle could be flat. Cons: it is a second world grid with its own
  addresses, and the last shell's cells meet it badly (table above): at a face centre 2.01 shell cells cross one
  block edge (an almost clean 2 to 1), at an edge middle 1.42, at a corner 1.16, so the seam is not a whole ratio
  anywhere but a point. Footprint of a shell cell on the cube face runs 0.25 m2 at the centre to 1.29 m2 at a corner
  (5x), and the cell is stretched 1 to 1.73 along the tilt at a corner. For Luna the cube is 286 blocks a side, 23 million.
- **(c) Keep halving** below one chunk a face edge: n = 48, 24, 12, 6, 3 columns, five more shells, 49 more layers,
  3.8e5 more cells, to a ball of **1.04 m radius** (less than one block, one solid cell). Pros: the same rule all the way,
  blocks stay 0.5 to 1 m, you dig to the centre within a block, and it costs almost nothing in cells. Cons: a chunk is
  32 cells a side, so a shell with 3 to 24 columns a face edge has less than one chunk: the chunk edge has to become
  min(CS, n_k) and the address "n/CS an integer" has to become "n_k a multiple of the chunk edge". Luna (n = 1664)
  halves to 13 columns and a ball of 4.5 m, not 1 m, because 13 is odd.

## Measured answer
- **Shells reach the centre**: 7 shells, 5,847 layers (A) or 4,375 (B) from the surface to the ball on the standard planet
  (4,212 and 4,199 m down), 7.6e11 and 4.3e11 addressable cells. The ball is 33.2 m (A) and 46.9 m (B), the same for
  768, 6144 and 12288, because it depends only on the 96 columns of the last whole-chunk shell, not on the planet.
- **A against B.** A keeps blocks no bigger than the surface's 1 m and halves them (0.5 m, one eighth the volume) to the
  seam; B straddles 1 m (0.71 to 1.41). B needs 1.8x fewer cells, and the
  worst block is 1.41 times from 1 m (A: 2 times). The seam is a 2 to 1 jump in both, so the T-junction problem is
  identical. B's blocks below the first seam are bigger than at the surface (1.41 m wide and thick next to a 1.8 m player);
  A's are small enough to build detail. B's ball is 14 m bigger.
- **Recommendation: B, with (c).** B because "near 1 m" is what was asked: a block never looks more than 1.41 times
  bigger or smaller than at the surface, against twice smaller in A, with about half the cells. (c) because it is the
  only option that reaches the centre, costs 3.8e5 cells, and one rule (a chunk is the whole face edge when the shell is
  narrower than 32 columns) covers all of it. With B, (c) ends at a ball of about 1.5 m radius (computed by halving
  46.9 m five more times, not tabled). If the chunk change is not wanted, (a) is the fallback; (b) is not
  recommended: a second grid, and no clean ratio to the shell above it.

## Not tested
This spike is arithmetic and flat pictures. It did not test:
- **Meshing across a seam.** The 2 to 1 step means T-junctions between a lower cell's face and two upper cells' faces,
  cracks and light leaks; the ratio of a cube to the shell in (b) is worse. Nothing was meshed.
- **Physics across a seam**: a player standing across it, a 1.41 m block next to 0.71 m ones, reach and step-up height.
- **Saves, streaming, the wire**: how a shell number enters an address, chunk sizes of 3, 6, 12 or 24 for (c),
  subscriptions through the centre, where a shaft's chunks come from.
- **Worldgen**: ores, caves and heat below the 192 m the grid reaches today; the bake of the planet.
- **Gravity and the camera at the centre**, the sky through a shaft, the equi-angular or conformal grid's own stretch
  (the table uses the mean size).
- The number of cells is addressing, not memory; real counts depend on what is dug.

## Variant L (layered, no visible seam)
The developer found the 2 to 1 seams in solid rock ugly. L follows the planet's own layers (`layered.py`, standard planet):
- **Crust and mantle**: one cube-sphere grid, no seam from the surface to the core-mantle boundary Rc. A block is r/R m
  wide and thick, so it shrinks smoothly from 1.00 m to 0.546 m (Rc = 0.546 R, Earth's proportion) or 0.707 m (Rc = R/sqrt2).
- **Outer core**, Rc down to Ri = 0.19 R (807 m): liquid metal, drawn as a smooth fill. Under it, hidden, variant A's
  halving shells (started at Rc) carry the liquid's columns and what a player builds there once it is drained.
- **Inner core**, radius Ri: a plain Cartesian grid of 1 m cubes, no shells, no centre problem. The ball's surface is a
  staircase against local down.
- Pictures: [`layered-whole.png`](out/layered-whole.png) (both Rc side by side),
  [`layered-mantle-bottom.png`](out/layered-mantle-bottom.png) (24 x 16 m at Rc, the player in a tunnel of 0.55 m blocks,
  liquid under the lowest layer), the inner core's surface on a grid axis and 45 degrees off it, drawn with down down the
  picture, each with liquid above and drained:
  [`layered-core-axis.png`](out/layered-core-axis.png), [`layered-core-axis-drained.png`](out/layered-core-axis-drained.png),
  [`layered-core-45.png`](out/layered-core-45.png), [`layered-core-45-drained.png`](out/layered-core-45-drained.png);
  [`layered-centre.png`](out/layered-centre.png) (innermost 40 m, a tunnel through the centre, the player weightless);
  [`layered-built.png`](out/layered-built.png) (drained outer core at the first hidden seam, a wall and a platform built
  across it: the seam shows only in what was built).

<!-- L tables -->
### Variant L tables (`python -I layered.py`)

Standard planet, n = 6144, R = 4,245.4 m, Ri = 0.19 R = 806.6 m.

**Crust and mantle: one grid, no seam** (block at r is r/R m wide and thick):

| Rc | Rc (m) | block at the bottom (m) | layers surface to Rc | cells | blocks a m3 at the bottom | depth of the mantle (m) |
|---|---|---|---|---|---|---|
| 0.546 R (Earth) | 2,318 | 0.546 | 2,570 | 5.82e+11 | 6.14 | 1,927 |
| R/sqrt2 (large core) | 3,002 | 0.707 | 1,472 | 3.33e+11 | 2.83 | 1,243 |

**Hidden shells under the liquid** (variant A's halving, started at Rc, stopped at Ri; seams at Rc/2, ...):

| Rc | hidden shell | top r (m) | bottom r (m) | columns a face edge | chunks | block width top / bottom (m) | layers | cells |
|---|---|---|---|---|---|---|---|---|
| 0.546 R (Earth) | 0 | 2,318 | 1,159 | 6144 | 192 | 0.546 / 0.273 | 2,944 | 6.67e+11 |
| 0.546 R (Earth) | 1 | 1,159 | 807 | 3072 | 96 | 0.546 / 0.380 | 770 | 4.36e+10 |
| R/sqrt2 (large core) | 0 | 3,002 | 1,501 | 6144 | 192 | 0.707 / 0.354 | 2,944 | 6.67e+11 |
| R/sqrt2 (large core) | 1 | 1,501 | 807 | 3072 | 96 | 0.707 / 0.380 | 1,319 | 7.47e+10 |

**Gravity**, falling linearly from the surface's g at R to 0 at the centre:

| where | r (m) | g as a fraction of the surface's |
|---|---|---|
| surface | 4,245 | 1.0000 |
| Rc 0.546 R (Earth) | 2,318 | 0.5460 |
| Rc R/sqrt2 (large core) | 3,002 | 0.7071 |
| Ri = 0.19 R | 807 | 0.1900 |
| 400 m | 400 | 0.0942 |
| 100 m | 100 | 0.0236 |
| 10 m | 10 | 0.0024 |

**Stair steps of the inner core** (a ball of 1 m cubes whose centres lie within 806.6 m; the plane through the centre and two grid axes, 80 m of walk along the tangent):

| angle from a grid axis | largest step (m, peak to peak of the sawtooth along down, 2 m window) | mean step (m) | floor tilt, mean (deg from level) | share of the walk tilted over 45 deg | floor tilts that cover over 5 % of the walk (deg) | rms about the mean line (m) |
|---|---|---|---|---|---|---|
| 0 | 1.00 | 0.11 | 0.0 | 0 % | 0 | 0.32 |
| 15 | 0.97 | 0.72 | 19.3 | 7 % | 15, 75 | 0.29 |
| 30 | 1.29 | 0.88 | 36.8 | 24 % | 30, 60 | 0.29 |
| 45 | 1.35 | 0.72 | 43.6 | 0 % | 45 | 0.26 |

<!-- /L tables -->

**What L gives.** No seam anywhere a player digs in rock: the mantle is one grid, the liquid has no blocks, the inner core
is perfect cubes to the very centre (the centre needs nothing special: a 2 m shaft through it, picture above). Seams exist
only under the liquid, and show only in what a player builds there after draining it. Gravity at the centre is zero and
rises linearly (0.0024 of the surface's at 10 m, 0.19 at Ri).

**What L costs.**
- **Small blocks at the mantle's bottom**: 0.546 m (6.1 blocks a m3, 2,570 layers, 5.8e11 cells) or 0.707 m (2.8 a m3,
  1,472 layers, 3.3e11 cells). The surface's 1 m is only at the top; there is no regrowth.
- **Stepped floors on the inner core**: a ball of 1 m cubes is flat only where a grid axis points up. Off axis the floor is
  a sawtooth: 15 deg off, treads tilted 15 and 75 deg; 30 deg, 30 and 60 deg; 45 deg, 45 deg only, with a sawtooth of up to
  1.35 m peak to peak (1.0 m at 0 deg, as a step every ~20 m of walk). A player walking on the inner core meets 1 m steps
  and a floor that changes tilt with the compass; the mean tilt of the floor is 0 to 44 deg depending on direction, never
  the ball's 0. Gravity is 0.19 of the surface's there, so it is walkable only if step-up and jump height are generous.
- **The ragged join** where the Cartesian ball meets the liquid's columns: the ball's staircase surface is not on any
  shell's cell boundary, and the hidden shell above it (0.38 m cells at 807 m) ends in a cut surface at Ri. Not drawn.
- **Liquid metal can never freeze into the hidden shells without showing seams**: a frozen layer is made of the hidden
  shells' blocks, which jump 2 to 1 at Rc/2, Rc/4 (the hidden-shell table, 1,159 m for Earth's Rc, 1,501 m for the large core).
  The pictured wall is the best case: it has the step because a player chose to build across it.
- The rock is no longer all one size: blocks run 1.00 to 0.55 (or 0.71) m, but smoothly, as the game does today in the top
  192 m.

**Not tested for L** (on top of the list below): the player walking on the staircase (step-up, slopes, vehicles); meshing
and lighting of the 1 m Cartesian ball against the shell above it; how the liquid is simulated and drained; what a block
of the hidden shells looks like when exposed to the liquid; the bake, saves and streaming of three zones; the stair-step
numbers are from one plane through the centre and two grid axes, not the full sphere (any direction off both axes is
rougher); and gravity as a straight line is an assumption, not a model.

**Recommendation updated.** Take **L with Rc = R/sqrt2**: rock without seams, blocks never under 0.71 m (the same floor as
B, with fewer layers and cells than the 0.546 R choice, 3.3e11 against 5.8e11 over the mantle), a centre that needs no rule
at all, and the shells' trouble confined below the liquid. Rc = 0.546 R is Earth-like but gives 0.55 m blocks to build with.
If a 1.0 to 1.35 m sawtooth floor at 0.19 g is not acceptable to the developer, the fallback is **B with (c)**, as above;
in that case L's smooth mantle can still be used down to the first B seam. This spike did not measure how the staircase
feels, so that is the developer's call.
