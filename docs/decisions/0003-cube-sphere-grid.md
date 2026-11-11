# 0003 – The planet grid is an equi-angular cube-sphere; its seams are part of the world

- **Status:** accepted (the block is 1 m and the fine cells 8 to its edge since ADR 0033; the grid stands)
- **Date:** 2026-11-09 (grid from the sphere grid spike, `prototypes/sphere-grid/README.md`; the seams are
  the developer's call)

## Context
ADR 0001 left the sphere grid to research. The spike measured four grids for 0.5 m blocks on 0.3-1 km planets:
- **Hex prisms:** the best cell shape, but hexagonal machines and rooms, no cubic chiselling.
- **Cube-sphere:** square-ish blocks, correct up everywhere, nests the 12.5 cm cells exactly.
- **Rounded cube:** a die from orbit, with tilted blocks along its edges.
- **Cartesian grid with radial gravity:** 90% of the surface tilted more than 15°.

The cube-sphere's cost is its 12 seams and 8 corners:
- **Under bare terrain:** a look check of rendered blocks showed the seams cannot be seen.
- **Straight builds:** a row of blocks turns where it crosses a seam, in one step. The turn is 0° at the seam
  middle, about 26° at 256 blocks from a corner and up to 60° at a corner. It does not shrink with planet size
  or depth.
- **Relaxing it away:** smoothing the grid so rows cross without a kink (the conformal cubed sphere) shrinks
  the blocks near every corner instead: under 0.45 m within 98 m of each corner (21% of the surface),
  centimetres at the corner. Blends only move the problem.
- **Other games:** shipped games dodge it. Space Engineers-style games use smooth terrain plus a free grid per
  build on huge planets. Eco uses a flat world that wraps. Blocky Planet and Seed of Andromeda live with the
  seams.

## Decision
- **Grid:** the equi-angular cube-sphere with quad-prism blocks: 6 faces of N x N columns.
  - **N = 32 * m * 2^L with a small m** (amended 2026-11-10 while building the engine): the chunks are 32
    columns wide and the LOD tree halves the resolution per level, so only such an N lets the tree tile a
    face. N sets the radius, R = N * 0.5 m / sqrt(2 pi / 3), so the mean block is 0.5 m wide:
    N = 3072 (m = 3) is the 1 km planet (R = 1061 m), N = 1536 the benchmark planet (531 m), N = 512 the
    moon (177 m). The spike's N = 2880 for exactly 1000 m would allow only one LOD level.
  - Radial layers are geometric (each 0.5/R thicker than the one below), so blocks stay cubic at every depth.
    Layer 0 starts at the core (64 m under the nominal surface); nothing below it is stored or dug.
  - Blocks are always level: gravity is radial and every column runs along the radius.
- **Fine 12.5 cm cells:** the same grid at 4N with 4 sub-layers per block, so they nest exactly.
- **World data:** chunk id = (face, ci, cj, layer); each chunk is a regular (i, j, layer) array.
  - Neighbours fold over a seam only at chunk borders on a seam; the 3-block corners are one special case.
  - Meshing, palettes and the cellular automaton work per face unchanged, with one exception: greedy
    merging is off, because a merged quad is a straight chord across the curved grid and leaves cracks next to
    smaller quads (3 cm for a 32 block quad at 1 km). Every face is one quad per block; merging can return
    inside a lattice with interpolated corners if quad counts ever matter.
  - The vertex shader maps each vertex's global (face, i, j, layer) to the sphere, relative to the camera, so
    quads and chunks share their corners exactly.
- **Seams are a feature of the world, "the planet's grain":**
  - **Shown:** in build mode (a faint line, like chunk borders) and on the map, so a turning conveyor reads as
    a rule of the planet, not a bug.
  - **Networks are topological:** conveyors, pipes and cables keep working across a seam and turn there by the
    seam's angle.
  - **Footprints:** a machine footprint may not cover a corner cell. A W x W footprint stamps as a regular
    patch everywhere else, across seams included.
  - **The 8 corners are landmarks:** world gen puts a peak, crater or deep lake on each, with something worth
    finding there.
  - **Spawn** sits at a face centre, far from every seam.

## Not taken
- **Conformal or relaxed cube-sphere:** removes the kink but shrinks blocks over 21% of the surface.
- **Hex prisms:** hexagonal machines, no cubic chiselling.
- **Rounded cube:** a die from orbit, with tilted blocks along its edges.
- **Cartesian with radial gravity:** tilted floors almost everywhere.
- **Flat wrapping world drawn round:** no real sphere for the seamless orbit of ADR 0001.
- **Nowicki warp:** areas are more even, but rows curve inside a face. Equi-angular keeps rows straight inside
  a face, and the seams carry all the turning, where they are shown.

## Consequences
- **Player movement and collision** run in the local tangent frame (as on any sphere grid). Blocks are not
  axis-aligned in world space.
- **Block size varies:** blocks range 0.39-0.55 m across a face (area ±18%) and grow with height (1.40x over
  the band at R = 300 m). Recipes and units count blocks, never volume in m³.
- **Seam fold** at chunk borders: the sim, meshing and server chunks pay it only on seam chunks (0.5% of
  cells at R = 300 m).
- **Long straight builds** turn at seams; build tools show the seam before the player commits.
- **Open:** what the corner landmarks are (content, later). The block look is settled (ADR 0002).
