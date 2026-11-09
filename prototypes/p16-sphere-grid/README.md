# p16: sphere grid for small planets of 0.5 m blocks

**Question.** Which grid should wrap a 0.3–1 km planet of 0.5 m blocks so that building, digging, machines and the sim all have a correct local "up"? (ADR 0170, `docs/plans/planet-rebuild.md`, wave 1.)

**Answer (recommendation).** Use **(b) a cube-sphere with quad-prism blocks**: 6 faces of N×N columns, the equi-angular warp, and geometric radial layers. It is the only candidate that:
- keeps square-ish blocks with a correct up everywhere;
- nests the 12.5 cm fine cells exactly (4N);
- lets every flat-grid engine technique work per face.

Its costs are known and local:
- 8 corners where 3 blocks meet instead of 4;
- corner cells skewed up to 30°;
- block areas varying ±18%;
- a straight row of blocks bending where it crosses one of the 12 face seams. This is 0° at the middle of a seam and up to 60° at a corner.

The runner-up is **(c) the rounded cube**, as a taste call for anyone who wants six flat Minecraft continents rather than a round planet. **(d) Cartesian + radial gravity** is ruled out for building. **(a) geodesic hexes** have the best cell shape, but they don't fit square machines, cubic chiselling or greedy meshing.

## Candidates

| | grid | where "up" is right | special places |
|---|---|---|---|
| (a) | geodesic hex prisms: dual of an icosahedron subdivided F times, 10F²+2 cells (PlanetSmith-style) | everywhere | 12 pentagons |
| (b) | cube-sphere: 6 faces × N² quad prisms; gnomonic, equi-angular or Nowicki warp (Blocky Planet-style) | everywhere | 8 corners (3 cells per vertex), 12 seams |
| (c) | rounded cube: a Cartesian grid on a cube with rounded edges; gravity is the normal of the rounded box (6 gravity faces) | only on the flat faces | 12 edge bands, 8 corner caps |
| (d) | Cartesian grid, sphere carved in, radial gravity (what the p14/p15 benchmark uses) | only at 6 points | the whole planet |

The cell counts are matched: (a) and (b) have the same number of cells as 0.5 m squares covering the sphere. At R = 300 m that is N = 868 / F = 673, about 4.5 M surface cells. At R = 1000 m it is N = 2894 / F = 2242, about 50 M.

## Measured (`results.json`)

### Cell shape at planet scale
Taken from the full face; R = 300 m and R = 1000 m give the same numbers to 3 digits.

| grid | area max/min | edge min–max (m) | worst corner angle off ideal | cells >10° off | cells >10% off mean area |
|---|---|---|---|---|---|
| (b) gnomonic | **5.18** | 0.33–0.69 | 30° | 44% | 84% |
| (b) equi-angular | 1.41 | 0.38–0.54 | 30° | 33% | 31% |
| (b) Nowicki | 1.33 | 0.40–0.69 | 30° | 11% | **0.6%** |
| (a) hex (plain subdivision) | 1.99 | 0.22–0.34 (hex edge; 0.31 ideal) | **10.7°** | **0.3%** | 49% |
| (a) the 12 pentagons | 0.44 × mean | | | | |
| (c), (d) | 1.00 (cubes) | 0.5 | 0° | 0% | 0% |

The gnomonic and equi-angular area ratios match the published cubed-sphere values (5.14 / 1.41, Andrews & Jablonowski 2025), which validates the code.

The hex numbers are for the plain normalised subdivision. **Relaxed hex layouts were measured too** (`relax.cpp`, `results_hex_relaxed.json`, F = 100; the ratios don't depend on F):

| hex layout | area max/min | worst corner angle off 120° | hexes >10% off mean area |
|---|---|---|---|
| plain subdivision | 1.95 | 10.4° | 49% |
| Lloyd relaxation, 50 iterations | 2.85 | 6.3° | 45% |
| spring relaxation, 50 iterations | 3.22 | 6.4° | 47% |
| spring relaxation, 300 iterations | 3.98 | 6.0° | 41% |

Simple relaxation makes the hexes more regular but the sizes less even: the cells around each pentagon shrink. Evening out the areas needs an explicit equal-area construction (ISEA, Heikes–Randall); those were not built here. In every layout the 12 pentagons stay and the cells still don't nest, so this doesn't change the recommendation.

### Block tilt against gravity
Applies to grids whose blocks don't follow the sphere (sampled by area).

| grid | mean tilt | area tilted >5° | >15° | >30° | flat build area | radius variation (look from orbit) |
|---|---|---|---|---|---|---|
| (d) Cartesian + radial | 31.9° | 99% | 90% | 60% | ~0 | sphere |
| (c) rounded cube k=0.25 | 8.6° | 33% | 25% | 13% | 63% | 1.55× (a die) |
| (c) rounded cube k=0.5 | 16.8° | 62% | 49% | 27% | 32% | 1.37× |
| (c) rounded cube k=0.75 | 24.6° | 85% | 71% | 42% | 10% | 1.18× |

(a) and (b) have 0° tilt by construction: each block column runs along the radius.

### Neighbour lookup
Cost of all lateral neighbours of a random cell, ns, one core, -O2 (single run):
- Cartesian 6-neighbour index arithmetic: **2.6**
- cube 4: **6.2**; cube cells on a seam: **47**
- hex 6: **6.9**; hex cells on an edge: **49**

Seam cells are 4/N of the cube's cells (0.5% at R = 300 m). A real engine works on padded per-chunk arrays (as a Cartesian engine does), so the fold is only paid when it fills the borders of chunks that touch a seam. Performance doesn't decide between the grids.

### Straight rows across a seam
This is the turn of a straight row of blocks (a wall or conveyor) where it crosses a seam, summed over the 12 blocks around the crossing, at R = 300 m.

| distance of the crossing from the nearest corner/pentagon | 0 | 8 | 32 | 64 | 128 | seam middle |
|---|---|---|---|---|---|---|
| (b) equi-angular | 60° | 59° | 56° | 52° | 44° | 0.07° |
| (b) Nowicki | 60° | 57° | 50° | 42° | 30° | 0.04° |
| (a) hex (icosahedron edge) | 36° (near the pentagon it crosses 2 edges) | 18° | 17° | 15° | 12° | 0.03° |

Inside a face, rows of the gnomonic and equi-angular grids are exact great circles (0° turn). Nowicki rows curve by 0.025° per block.

So a long conveyor stays topologically straight everywhere, but it visibly bends where it crosses a seam. This is the trade for having square cells on a sphere: Gauss–Bonnet puts the 720° of curvature somewhere, and the cube puts it on 12 seams and 8 corners.

### Footprints
Placement of square (cube) or hex-ball machines, tested on every origin cell of N = 64 / F = 32. The result depends only on the topology, not on N.

| footprint | origins that fail | max distance of a failing origin from a corner/pentagon |
|---|---|---|
| (b) 2×2 / 4×4 / 8×8 / 16×16 | 6 / 54 / 294 / 1350 | 0 / 2 / 6 / 14 blocks (= W−2) |
| (a) hex radius 1 / 2 / 4 / 8 | 12 / 72 / 372 / 1692 | 0 / 1 / 3 / 7 blocks (= k−1) |

A W×W footprint stamps as a regular W×W patch everywhere, across seams included, unless it covers a corner. Either grid only needs "no placement over the 8 corners / 12 pentagons".

### Radial layers (a) and (b)
Block width grows with radius. Over the playable band (64 m crust to +30 m terrain):
- at R = 300 m, blocks are 0.39 m at the crust bottom and 0.55 m at the terrain top (1.40×);
- at R = 1000 m, they are 0.47–0.52 m (1.10×).

With **geometric layer thickness** (each layer 0.5/R thicker than the one below), blocks stay cubic at every depth: 202 layers over the band at R = 300 m, against 188 uniform layers. One shell covers the whole band.

Deeper digging would need Blocky-Planet-style shells: halve N each time the width halves. That nests exactly 4:1 on quads and only approximately on hexes.

## Fine cells, chiselling, fluids, gases
- **(b):** the 12.5 cm cells are the same cube-sphere with 4N per face and 4 sub-layers per block, so they nest exactly. The neighbour and seam code is the same (`CubeSphere::step` with `4N`). A chiselled block is a 4×4×4 array in block-local (i, j, layer). Fluid fill levels sit per column and gases on the same grid.
- **(a):** hexes don't subdivide into hexes. With a 2F fine grid, 3 of every 4 fine cells straddle two blocks, because the fine cells at triangle-edge midpoints lie on the coarse cell borders. Fine cells would have to be triangle prisms (24 per block layer) or a local Cartesian grid that doesn't line up with the neighbouring block. Neither gives cubic chiselling.
- **(c) and (d):** plain cubes, so fine cells are trivial. (d) has its tilt problem everywhere, (c) only in its edge bands.

## What the choice costs every other system (b, recommended)
- **Engine/meshing:** per face, a chunk is a regular (i, j, layer) array. Binary greedy meshing, palettes and empty-chunk shortcuts all work unchanged. The vertex shader maps packed (face, i, j, layer) to the sphere with `to_sphere()`·r, which fits p15's packed quads.
  - Pick N as a multiple of the chunk size: R = 300 m → N = 864 = 27·32 (0.502 m blocks); R = 1000 m → 2880 = 90·32.
- **Far planet / seamless orbit:** a quadtree per face is the standard chunked-LOD planet layout. Blocky LOD is natural there: coarser levels are N/2, N/4… with exact 4:1 parents.
- **Collision/physics:** blocks aren't axis-aligned in world space, so the player controller and collision run in the local tangent frame (as on any sphere grid). (c) and (d) keep AABB collision.
- **Sim (technosim):** the cellular automaton runs on (face, i, j, layer) arrays. Only chunk borders on a seam use the fold, and 3-way corners need one special case.
- **Machines and networks:** they are graphs over cells, so the grid doesn't touch them. Footprints are forbidden over the 8 corners. Rotations are 4 everywhere; around a corner, a row turns 90° in 3 steps instead of 4.
- **Building:** builds follow the curvature. A "level" floor 50 m long sags 1.04 m at R = 300 m (0.31 m at 1 km), and gravity turns 1° every 5.2 m at R = 300 m. This is true of every sphere grid; only (c)'s flat faces avoid it.
- **World gen:** noise is sampled at the 3D position, so it is seamless on every grid.
- **Server chunks:** chunk id = (face, ci, cj, layer).

## Why not the others
- **(a) hex:** has the best shape, but GregTech machines, rooms and conveyors would all be hexagonal, chiselling can't be cubic, and greedy meshing (square row bitmasks) only merges along the 3 hex axes. It is also more seam code: 30 edges and 12 pentagons, against 12 seams and 8 corners. Choose it only if a hex world is wanted as the look.
- **(c) rounded cube:** 32–63% of the surface is perfectly flat, axis-aligned Minecraft. But from orbit it is a rounded die (1.37–1.55× radius variation), and its edge bands carry (d)'s tilt problem. It is a valid taste choice ("6 flat continents") and is cheap, because it is a Cartesian engine with a different gravity function.
- **(d) Cartesian + radial:** 90% of the planet has blocks tilted more than 15° from gravity, so every floor is a staircase (`images/sheet_sections.png`). It stays as the benchmark stand-in for p14/p15, since its meshing and streaming cost is the same order as (b).

## Images (`images/`, only the sheets are committed)
- `sheet_surface.png`: (a) and (b) at R = 300 m, 18 m across: a typical patch, a seam and a pentagon/corner. Cells are coloured by area against the mean (blue small, white mean, red big).
- `sheet_orbit.png`: all four grids from orbit. (c) and (d) are coloured by block tilt.
- `sheet_projections.png`: the gnomonic, equi-angular and Nowicki cube-spheres at N = 12.
- `sheet_sections.png`: (d) at 45° latitude and (c) 10° into a rounded edge, with 0.5 m blocks against a level floor.

## Published approaches
- Blocky Planet (Bowerbyte): a cube-sphere with a pre-distorted grid and 4× shells by depth. Its noted problems are corners ("topology breaks"), structures wrapping near them, and a hollow core. https://www.bowerbyte.com/posts/blocky-planet
- Cubed sphere: Ronchi, Paolucci & Iacono 1996 (J. Comp. Phys.); equi-angular is the common choice (CAM-SE, LFRic). The area ratios (5.14 gnomonic / 2.30 equi-edge / 1.41 equi-angular) are from Andrews & Jablonowski. https://arxiv.org/pdf/2505.05624, https://www.cesm.ucar.edu/sites/default/files/2025-06/2025cesmandrews.pdf
- Geodesic/H3 grids: there are always 12 pentagons, and hex area varies with distance to the icosahedron vertices. https://h3geo.org/docs/core-library/restable/, https://en.wikipedia.org/wiki/Geodesic_grid
- Grid-aligned voxels misalign with gravity on a cube-mapped planet: https://blog-6kz.pages.dev/posts/non_cubic_voxels. StarMade on why spheres don't fit a block world: https://starmadedock.net/threads/starmade-dev-update-why-spherical-planets-dont-fit-in-a-block-world.22996
- PlanetSmith: no public tech write-up was found, so the hex-prism attribution is from its look.
- Space Engineers and Astroneer: smooth (non-block) voxel planets, so they don't apply to block grids.

## Files and how to run
`grid.h` is the header-only library: `CubeSphere` (topology with an integer cube-point fold, 3 warps) and `HexSphere` (canonical icosahedral cells, neighbours incl. pentagons). `test.cpp` holds the headless tests. `measure.cpp` produces the numbers and the cell dumps, and `render.py` makes the images and the (c)/(d) numbers, then writes `results.json`.

```bash
export PATH="$HOME/Tools/mingw-dl/mingw64/bin:$PATH"   # GCC 16.2
g++ -std=c++17 -O2 -o test.exe test.cpp && ./test.exe
g++ -std=c++17 -O2 -march=native -o measure.exe measure.cpp && mkdir -p out && ./measure.exe
python -I render.py   # numpy + Pillow
```

The tests cover, for cubes N ∈ {1, 2, 3, 8, 17, 64}:
- counts and neighbour symmetry;
- 24 corner cells;
- total area = 4π;
- every row loops back after 4N steps.

For hexes F ∈ {1, 2, 3, 5, 8, 21}:
- counts of 10F²+2 and exactly 12 pentagons;
- neighbour symmetry;
- degree sum = 60F²;
- each cell's canonical form is the same point.

The whole run takes ~20 s.

**Limits of this spike.**
- The neighbour timings are one run on the dev box.
- Equal-area hex layouts (ISEA, Heikes–Randall) were not built; only Lloyd and spring relaxation were measured.
- The in-engine look check (`blocks.cpp` + `godot/look.gd`, Godot 4.7.2) is work in progress: it renders, but the sheet and its findings are not written up yet.
- The choice between equi-angular and Nowicki is one function (`to_sphere`) and doesn't change the topology. Decide it by look: equi-angular gives straight rows inside a face, Nowicki more even areas.
