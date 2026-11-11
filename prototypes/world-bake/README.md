# World bake spike: the land of Terra (worldgen W1)

**Question.** Can stages 1 to 4 of `docs/design/worldgen.md` run once when a world is made, fast enough
and seam-free on the cube-sphere? The stages are plates, their boundaries, uplift against stream-power
erosion, sea level, then rivers and lakes. The budget proposal: 30 s for Terra, 2 min for the huge planet,
under 10 MB on disk, under 100 MB of server RAM. The answers are measured below.

**Answer.** Yes for time and seams; memory is over the proposal.
- **Time:** Terra bakes in **13.8 s** and the huge planet in **70.7 s** (release build, this PC).
- **Determinism:** the same seed gives the same hash twice, and another seed a different one.
- **Rivers:** every river runs downhill to the sea or a lake. This is a test, seen failing first.
- **Seams:** the seams and corners do not show, after one fix (below).
- **Disk:** the result is **2.3 MB** for Terra and **8.2 MB** for the huge planet, compressed. That is
  heights, plates and rivers only; W2's full record will be bigger.
- **Memory (over the proposal):** the bake process peaks at **174 MB** for Terra and **673 MB** for the
  huge planet.

Throwaway code: its own cargo project, nothing depends on it, no engine code changed.

## Run it
```
cd prototypes/world-bake
cargo build --release
target/release/world-bake.exe seed=1 planet=12288            # one bake: JSON line, out/seed-1-planet-12288/{sheet,orbit}.png, bake.bin.gz
target/release/world-bake.exe mode=seeds planet=12288        # 12 seeds: out/seeds-planet-12288.png
target/release/world-bake.exe mode=relief seed=1             # relief variants: out/relief-seed-1.png
cargo test --release                                         # grid folds, determinism, rivers, sea share
```
- **Options:** `planet=` (12288 Terra, 24576 huge), `mountain=` and `sea=` (the relief in metres),
  `coarse_iters=` and `fine_iters=`, `draw=0` (no pictures), `variants=50:25,120:60` (for relief).
- **Seam measure:** `SEAMS=1` prints the mean slope inside faces and across seams at each step.
- **Parameters:** every knob, and the three taste calls marked `TASTE CALL`, are in one place: `Params`
  in `src/bake.rs`.

Copies of the sheets are in `images/` (`out/` is not kept in git).

## What it does, step by step, and where each method comes from
All steps run on the engine's own cube-sphere: six faces of m x m cells, with the engine's face axes and
the equi-angular warp (ADR 0003). A map cell therefore lines up with chunks.
- **Cell size:** 8 m cells (worldgen.md), so m = N / 16.
  - Terra: 768 cells per face edge, 3.5 M cells.
  - Huge: 1536 per face edge, 14.2 M cells.
- **Coarse level:** 4 x 4 fine cells, so 32 m.
- **Why 8 m:** a river at 8 m is one cell wide, the size worldgen.md stores per cell. A grid of 4 m cells has as many cells as the
  huge planet at 8 m, so it would take about 70 s for Terra.

| # | Step | Method | Source |
|---|---|---|---|
| grid | neighbours, distances, areas | Neighbours past a face edge come from the extended face's 3D point, found again on the sphere. Every distance is the chord and every cell area the true one, so no step knows where a face ends. Corner cells have 7 neighbours. | ADR 0003 |
| 1 | plates | Voronoi growth on the sphere (multi-source Dijkstra) from 8 to 14 random seeds. Each plate grows at its own pace through a noisy cost, so borders wander. Each plate turns about its own Euler pole. Continental crust grows as a noisy blob inside plates until it covers 48 % of the surface. | worldgen.md stage 1; Cortial et al. 2019 |
| 2 | boundaries | At each boundary cell, the relative velocity against the boundary's direction gives convergent, divergent or transform. Then an uplift profile by kind (table below). Distance from the boundary comes by Dijkstra. Ocean depth grows with the square root of the distance from a ridge (cooling with age). Continental margins slope down into shelves. Old worn ranges come from a ridged noise inside continents. 3 to 6 hot spots. | Cortial et al. 2019 (no time steps); plate cooling, Parsons and Sclater 1977 |
| 3 | uplift against erosion | dh/dt = U - K A^0.5 S, solved implicitly along the receivers from the coast up. Linear hillslope diffusion. 160 steps at 32 m, then refined to 8 m with fine detail and 24 more steps. | Braun and Willett 2013 (the O(n) implicit solver); Cordonnier et al. 2016 (uplift against stream power for terrain) |
| 3 | drainage, lakes | Priority-Flood with epsilon from the coast fills every depression to its spill point. The receiver is the steepest lower neighbour (D8). Drained area accumulates down the flood order. A cell whose filled surface stands over its ground is a lake. | Barnes, Lehman and Mulla 2014; O'Callaghan and Mark 1984 |
| 4 | sea and coast | The sea level is found by bisection so that the connected ocean covers 55 % of the surface by area (ADR 0021). A hollow under that level inland that is smaller than 0.2 % of the surface is not sea: it fills to a lake. Heights are then scaled linearly so the highest land is `mountain_m` and the deepest sea is `sea_depth_m`. | ADR 0021 (quantile cut) |
| 4 | rivers | Cells draining over 0.15 km² are river cells, each a segment to its receiver with its area. Lakes are the connected lake cells, with their level. | worldgen.md: "rivers as lines beside the grid" |

Boundary kinds and their uplift (amplitude times the convergence speed; widths are shares of the radius,
so they grow with the planet as continents do in ADR 0021):

| meeting | uplift |
|---|---|
| continent meets continent | a collision belt, 0.05 R wide (212 m on Terra) |
| ocean dives under a continent | a volcanic arc inland on the continent (0.035 R), a trench on the ocean |
| ocean meets ocean | an island arc on the plate that rides over, a trench on the other |
| plates part | a ridge in the ocean (through the depth), a rift valley sinking in a continent (0.012 R) |
| transform | a little |

## Measured (release build: fat LTO, `target-cpu=native`; Ryzen 5 5600X, 12 threads; `out/.lock` held)

Seconds per stage, seed 1:

| stage | Terra (N 12288, 3.5 M cells) | huge (N 24576, 14.2 M cells) |
|---|---|---|
| grid | 0.11 | 0.42 |
| 1+2 plates and boundaries | 0.51 | 2.02 |
| 3 erosion, coarse (160 steps at 32 m) | 2.64 | 12.00 |
| refine to 8 m | 0.35 | 1.41 |
| 3 erosion, fine (24 steps at 8 m) | 9.35 | 51.52 |
| 4 sea and coast | 0.77 | 3.25 |
| 4 rivers and lakes | 0.02 | 0.09 |
| **total** | **13.75** (budget 30) | **70.70** (budget 120) |
| peak working set, MB | 174 | 673 |
| on disk, gz: heights as i16 dm, plates, rivers | 2.3 MB | 8.2 MB |
| uncompressed | 10.7 MB | 43.1 MB |

- **Seed to seed:** 12 Terra seeds took 13 to 24 s each, including drawing between bakes.
- **Where the time goes:** the fine erosion steps are 70 %. Each step is one Priority-Flood (a heap over
  the 1.6 M land cells), a receiver pass, an O(n) accumulation and the implicit solve, about 0.4 s for
  Terra.
  - The huge planet costs 5.1x Terra for 4x the cells: the heap's log factor and memory traffic.
  - Most of it is single-threaded by design (the flood and the solve run in order). Only the receivers,
    the diffusion and the refinement run on rayon, cell by cell.
- **Steady state:** it is not reached. After the last step the largest change on land is still 1.4 m per
  step (of 50 m relief).
  - Twice the steps (400 coarse, 60 fine) took 41 s: the same land, a little more cut up, 28 lakes
    instead of 34 (`out/long/`).
  - So the default is a choice of the wait (taste call 4), not of correctness.
- **Memory:** 174 MB is over the 100 MB proposal. That proposal is for what the server keeps; the bake's
  arrays at their peak are a different number. What the server would keep is height and surface,
  2 x 4 bytes per cell: 28 MB for Terra and 113 MB for the huge planet as f32.
  - The bake's peak can come down by storing directions as i16 or computing them, by not cloning h each
    step, and by a flat neighbour table instead of the HashMap. That is W2's work.
- **Disk:** 2.3 MB holds the height (i16, decimetres), the plate of each cell and the river segments.
  W2 adds rock, soil, climate and biome per cell (worldgen.md: about 16 bytes). These are smooth fields
  and compress well, but the huge planet is already at 8.2 MB, so **the huge planet will exceed 10 MB**
  unless the stored map is coarser than 8 m there.

### Determinism
- `cargo test`: the same seed twice gives the same hash, and another seed a different one (small planet).
- Terra, run twice:
  - seed 1: `9c9d3a95444b780f` and `9c9d3a95444b780f`;
  - seed 2: `0039d6825df9dd33`.
- **How:** every queue breaks ties by cell index, every sum runs in a fixed order, and rayon only maps
  cells to values one by one. The sea level is `select_nth_unstable` (deterministic) and then a fixed
  bisection.

### Seams and corners
- **First result: a seam showed.**
  - Two of the four close-ups showed the sea cut by a straight line exactly on a seam.
  - Measured as the mean slope between neighbours: 0.037 m/m inside faces, 0.123 across seams after
    refining. A 3.3x step.
  - **The cause:** the refinement was bilinear inside each face, clamped at the face's border. Each side
    of a seam held its last coarse value for half a coarse cell, then jumped.
- **The fix:** refine with a tent filter over the coarse cell and its neighbours by true distance, which
  folds over seams like everything else.
- **After the fix, inside faces against across seams:**
  - coarse result: 0.037 and 0.034;
  - refined: 0.034 and 0.025;
  - final: 0.045 and 0.036.
  - The seam is no steeper anywhere; it is a little smoother, because the tent's reach is 1.3 cells.
- **The pictures agree.** `sheet.png` has close-ups 600 m across of the two corners and two seam points
  with the most coast in view (seam or corner at the centre; height, then the river map). Neither the
  coast nor the rivers nor the shading shows a line or a kink there.
  - Corner cells have 7 neighbours and their true, smaller area, so drained area does not jump there.
  - The grid test checks that the cell areas sum to the sphere's within 0.1 %.

### Rivers
- **The test:** `every_river_reaches_the_sea_or_a_lake_downhill`, three seeds on the 1 km planet.
  - From every river cell it follows the receivers to the sea or a lake.
  - The water surface must fall at every step, and the ground downstream must never stand above the
    water.
  - Seen failing first: with every 500th land cell's receiver sent uphill, it stops on "cell 177102 to
    177293: the water climbs from 0.149 to 0.344 m". It passes again with that change reverted.
- **On every bake:** the JSON's `drains` runs the same check. It was true for all 12 seeds and the huge
  planet.

## Reading the sheets
- **`images/sheet-seed-1.png`** (one Terra):
  - top row: the cube unfolded three times. Height with rivers; plates (red convergent, blue divergent,
    yellow transform, bright continental crust); rivers and lakes.
  - bottom row: the globe from two sides, then the four close-ups (height, then rivers).
- **`images/orbit-seed-1.png`:** the globe from orbit, 1000 px.
- **`images/sheet-seed-1-huge.png`:** the same sheet for the huge planet.
- **`images/seeds-planet-12288.png`:** seeds 1 to 12, left to right and top to bottom, a globe and the
  unfolded cube each.
- **`images/relief-seed-1.png`:** one seed at 50/25 m, 120/60 m and 200/100 m. For each: the globe, a
  close-up 1.4 km across the highest peak, and a section 2.2 km long through it (2 m a pixel, heights 2x).

### What I see
- **Seeds and their land:**
  - Every seed has 2 to 4 continents and island chains, and is clearly its own world.
  - Plate boundaries read from orbit: a long straight arc range (seed 9's coast range), island arcs and
    trenches offshore, mid-ocean ridges as pale lines.
  - Seeds 3 and 11 have many big lakes. Seed 2 has a large open ocean with one long arc.
  - Seeds 1, 5 and 7 have the most varied land: ranges, plains and gulfs.
- **Coasts:** the coastlines are ragged in close-ups (flooded valleys, rias). The sea floor is smooth.
- **The ranges are narrow and the land is low (the main point for the relief call):** with the relief
  scaled by the highest peak, on Terra seed 1 at 50 m:
  - half the land is within **1.5 m** of the sea, 90 % under 7 m, 99 % under 20 m;
  - the sea's median is 8 m deep and its deepest 25 m;
  - so the continents are lowland plains, and the mountains are belts a few hundred metres wide at
    collision zones and arcs.
  - ADR 0021's noise today has plains rising 14 m inland and hills to ±10 m.
  - Raising `mountain_m` scales everything, so at 120 m the plains would be 3.6 m median. Alternatives
    the code can do in a line: scale by the 99th percentile of land instead of the peak, or raise the
    interior uplift.
- **The shape stays across the variants:** relief is the last, linear step, so the three variants are one
  shape at three heights. At 200 m the snow line shows; at 50 m the coast range is a hill.

## Not done (W2 or later)
- Single-direction flow (D8) makes straight reaches on gentle slopes. Multiple flow or a meander pass
  would be W2's when river beds are cut into blocks.
- No sediment deposition: no deltas or floodplains yet (stage 7 reads "what the rivers carried").
- No rock hardness in the erosion. All rock erodes alike until stage 9 gives provinces.
- No coast types (cliffs, beaches) and no spawn score: stages 4b and 11.
- The 8 corners hold nothing yet (ADR 0027); the bake does not keep them clear.
- Luna's pipeline (W6) is not here.
