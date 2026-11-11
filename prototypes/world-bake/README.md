# World bake spike: the land of Terra (worldgen W1)

**Question.** Can stages 1 to 4 of `docs/design/worldgen.md` run once when a world is made, fast enough
and seam-free on the cube-sphere? The stages are plates, their boundaries, uplift against stream-power
erosion, sea level, then rivers and lakes. The budget proposal: 30 s for Terra, 2 min for the huge planet,
under 10 MB on disk, under 100 MB of server RAM. The answers are measured below.

**12 km default (2026-11-11).** The developer proposed planets of 8 km (small), 12 km (default) and 20 km
(big), because on Terra's 4 km the features change too quickly. The bake now defaults to 12 km (N 17376 of
1 m blocks, ADR 0033), with 48 fine steps and fewer lakes. Seed 5:

| planet | N (1 m blocks) | time | peak RAM | disk | lakes | lake share of land | land p50 / p90 / p99 m |
|---|---|---|---|---|---|---|---|
| 12 km, before the lake work (24 fine steps) | 17376 | 148 s | 1.4 GB | 26 MB | 191 | 9.4 % | 22.4 / 47.2 / 66.7 |
| **12 km, now (48 fine steps)** | **17376** | **218 s** (292 before the 2nd speedup) | **1.6 GB** | **26 MB** | **119** | **4.6 %** | **17.1 / 44.7 / 67.1** |
| 4.2 km (old Terra), now | 6144 | 20 s (28 before) | 0.21 GB | 3.6 MB | 42 | 1.7 % | 19.3 / 51.0 / 78.5 |
| 20 km, before the lake work | 28960 | 399 s | 3.9 GB | 68 MB | 286 | 15 % | 20.6 / 45.6 / 62.9 |

- **The bake is over its 2 min proposal at 12 km** (3.6 min); the fine steps are 75 % of it. A 12 km bake
  has 8x Terra's cells at 8 m.
- **The second speedup (2026-11-11), exact:** 292 to 218 s at 12 km, 28 to 20 s at 4 km. The same land:
  rivers, lakes, lake cells, percentiles and the last step's change all equal. A hash of the heights alone
  was equal on two seeds; the bake's hash changes because the river and lake lists follow the flood order.
  - Measured at 12 km before it, a fine step took 5.2 s: the flood 3.5 (one thread), grouping the basins
    0.64, finding the oceans 0.29.
  - **The drainage runs per land mass:** no water crosses from one mass to another, so each floods, and
    groups its basins, on its own thread. The receivers stay one parallel pass over all cells. Seed 5's
    largest mass holds 57 % of the land, so this alone gave 1.5x on the flood.
  - **Big masses flood through buckets** (`Buckets`): a key's top 20 bits pick a bucket, and only the
    current bucket is a binary heap. The same pops as the radix heap, about 20 % faster on the big mass.
  - Now a fine step at 12 km takes about 3.1 s: the largest mass's flood 1.7, the oceans and masses
    (two serial labellings) 0.55, the basins 0.5.
  - Tests: `the_monotone_queues_pop_in_heap_order` (failed with the bucket push broken on purpose), and
    the slow-path test now checks the water level, receivers and area to the bit with the order by mass.
- **Where the lakes came from**, measured at 12 km:
  - More coarse steps do not change them: 1.27 M lake cells at 320, 640 and 1280 coarse steps.
  - Refining to 8 m starts the fine level at 15 % lake share. The fine steps drain that: 24 steps leave
    9 %, 48 leave about 4.5 %, 64 leave 3.3 %.
  - Most of what is left is shallow: lagoons the rising sea level leaves behind coastal ridges, and pits
    the 8 m hillslope diffusion makes by filling one-cell river beds.
- **What changed:**
  - **A bug, fixed:** the flood's epsilon (1e-4 per cell) added up over plains wider than about 500 cells
    (4 km) to the lake threshold. Such plains counted as lake floors, so erosion never cut them. The lake
    test now uses the spill level without the epsilon (`Drainage::lvl`). Test
    `a_wide_plain_is_no_lake_and_a_pit_is`, seen failing first (506,596 plain cells taken for lake).
  - **48 fine steps** instead of 24 (the developer: "give erosion more steps").
  - **A lake must be 1 m deep somewhere** (`min_lake_m`). A shallower one is a puddle or lagoon, filled to
    flat ground at its level. The rivers test checks it, and failed with the filling switched off.
- **Tried and dropped** (each lowered the plains or did not help):
  - Filling the refine's hollows before the fine steps: the median land fell from 29 to 17 m on Terra.
  - Silting lakes up by a share of their depth per step: wide shallow flats, more lake area at the
    coarse level, and capping lake floors at the lake level halved the median land.
  - Weaker diffusion at 8 m: no fewer lakes on Terra.
- **The median land at 12 km is 17 m against 22 m before.** That is the plains the developer chose higher,
  now 5 m lower: a taste check for the next look.
- **1 m blocks (ADR 0033):** `planet=` counts 1 m columns now, so every N is half what it was at the same
  size in metres: 6144 is the 4.2 km planet, 12288 the 8.5 km one. The tests bake 768 (small) and 1536
  (rivers). The bake's 8 m cells are unchanged, so is everything it measures in metres.

**Answer (4 km Terra, before the 12 km default).** Yes for time and seams; memory is over the proposal.
- **Time:** Terra bakes in **13.1 s** and the huge planet in **61.6 s** (release build, this PC, seed 5),
  with the higher plains and the 120/60 m relief the developer chose (below). Asked "make it faster", the
  bake went from 18.4 and 99.3 s to these with the same result to the bit (same hash; below).
- **Determinism:** the same seed gives the same hash twice, and another seed a different one.
- **Rivers:** every river runs downhill to the sea or a lake. This is a test, seen failing first.
- **Seams:** the seams and corners do not show, after one fix (below).
- **Disk:** the result is **3.6 MB** for Terra and **13.3 MB** for the huge planet, compressed, at 120 m
  of relief. The developer: file size matters little, performance does. That is
  heights, plates and rivers only; W2's full record will be bigger.
- **Memory (over the proposal):** the bake process peaks at **174 MB** for Terra and **673 MB** for the
  huge planet.

Throwaway code: its own cargo project, nothing depends on it, no engine code changed.

## Run it
```
cd prototypes/world-bake
cargo build --release
target/release/world-bake.exe seed=1                         # one 12 km bake: JSON line, out/seed-1-planet-17376/{sheet,orbit}.png, bake.bin.gz
target/release/world-bake.exe mode=seeds planet=6144         # 12 seeds of the 4 km planet: out/seeds-planet-6144.png
target/release/world-bake.exe mode=relief seed=1             # relief variants: out/relief-seed-1.png
cargo test --release                                         # grid folds, determinism, rivers, sea share
```
- **Options:** `planet=` in 1 m columns (17376 the 12 km default, 6144 the 4 km planet), `min_lake=`,
  `coarse_dt=`, `fine_dt=`, `mountain=` and `sea=` (the relief in metres),
  `coarse_iters=` and `fine_iters=`, `draw=0` (no pictures), `variants=50:25,120:60` (for relief).
- **Seam measure:** `SEAMS=1` prints the mean slope inside faces and across seams at each step.
- **Parameters:** every knob, and the three taste calls marked `TASTE CALL`, are in one place: `Params`
  in `src/bake.rs`.

The 12 km sheets are `images/sheet-seed-5-12km.png` and `images/orbit-seed-5-12km.png`. Copies of the sheets are in `images/` (`out/` is not kept in git).

## What it does, step by step, and where each method comes from
All steps run on the engine's own cube-sphere: six faces of m x m cells, with the engine's face axes and
the equi-angular warp (ADR 0003). A map cell therefore lines up with chunks.
- **Cell size:** 8 m cells (worldgen.md), so m = N / 8 with N in 1 m columns.
  - 12 km (default): 2172 cells per face edge, 28.3 M cells.
  - 4 km: 768 cells per face edge, 3.5 M cells (the tables below were measured there).
- **Coarse level:** 4 x 4 fine cells, so 32 m.
- **Why 8 m:** a river at 8 m is one cell wide, the size worldgen.md stores per cell. A grid of 4 m cells has as many cells as the
  huge planet at 8 m, so it would take about 70 s for Terra.

| # | Step | Method | Source |
|---|---|---|---|
| grid | neighbours, distances, areas | Neighbours past a face edge come from the extended face's 3D point, found again on the sphere. Every distance is the chord and every cell area the true one, so no step knows where a face ends. Corner cells have 7 neighbours. | ADR 0003 |
| 1 | plates | Voronoi growth on the sphere (multi-source Dijkstra) from 8 to 14 random seeds. Each plate grows at its own pace through a noisy cost, so borders wander. Each plate turns about its own Euler pole. Continental crust grows as a noisy blob inside plates until it covers 48 % of the surface. | worldgen.md stage 1; Cortial et al. 2019 |
| 2 | boundaries | At each boundary cell, the relative velocity against the boundary's direction gives convergent, divergent or transform. Then an uplift profile by kind (table below). Distance from the boundary comes by Dijkstra. Ocean depth grows with the square root of the distance from a ridge (cooling with age). Continental margins slope down into shelves. Old worn ranges come from a ridged noise inside continents. 3 to 6 hot spots. | Cortial et al. 2019 (no time steps); plate cooling, Parsons and Sclater 1977 |
| 3 | uplift against erosion | dh/dt = U - K A^0.5 S, solved implicitly along the receivers from the coast up. Linear hillslope diffusion. 320 steps at 32 m, then refined to 8 m with fine detail and 24 more steps. | Braun and Willett 2013 (the O(n) implicit solver); Cordonnier et al. 2016 (uplift against stream power for terrain) |
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
| grid | 0.11 | 0.43 |
| 1+2 plates and boundaries | 0.56 | 1.94 |
| 3 erosion, coarse (320 steps at 32 m) | 4.98 | 20.78 |
| refine to 8 m | 0.35 | 1.42 |
| 3 erosion, fine (24 steps at 8 m) | 6.34 | 34.04 |
| 4 sea and coast | 0.74 | 2.92 |
| 4 rivers and lakes | 0.02 | 0.09 |
| **total** | **13.10** (budget 30; 18.37 before) | **61.60** (budget 120; 99.25 before) |
| peak working set, MB | 189 | 720 |
| on disk, gz: heights as i16 dm, plates, rivers | 2.7 MB (3.6 at 120 m) | 9.8 MB (13.3 at 120 m) |
| uncompressed | 10.7 MB | 43.1 MB |

- **Seed to seed:** before the speedup, 12 Terra seeds took 17 to 20 s each. After it, three runs of seed 5
  took 13.1 to 13.9 s.
- **Where the time goes:** the fine erosion steps are 58 % and the coarse ones 32 %. Each step is one Priority-Flood (a heap over
  the 1.6 M land cells), a receiver pass, an O(n) accumulation and the implicit solve, about 0.4 s for
  Terra.
  - The huge planet costs 4.7x Terra for 4x the cells, mostly memory traffic.
- **The speedup (2026-11-11), exact:** measured per step on Terra's fine level, the flood was 4.9 s of the
  24 steps, the implicit solve 2.1, the coast seeding 0.4.
  - The flood's binary heap is now a radix heap (a monotone queue: Priority-Flood never pushes below the
    last key). The same keys come out in the same order: 4.9 to 3.5 s.
  - The area sum and the implicit solve run per river basin in parallel: basins share no cell, and inside
    a basin the order is the flood's. Solve 2.1 to 0.3 s; the area costs 0.4 to 0.9 s more for grouping.
  - The coast seeding runs on rayon.
  - Kept as the reference: the test `the_fast_drainage_matches_the_slow_path` runs the old binary-heap
    flood and serial area sum beside the new ones and wants the same order and the same bits. It failed
    when the basin sweep was broken on purpose.
  - The flood is now the largest single cost and still serial. Splitting it by land mass would keep the
    heights but change the order of the river and lake lists, so the hash.
- **Longer erosion steps, tried and not taken:** the solver is implicit, so `coarse_dt`/`fine_dt` (default
  1) allow fewer, longer steps. They change the land, so they are a taste call, not a free speedup. Seed 5:

| steps | Terra s | land p50 / p90 / p99 m | lakes |
|---|---|---|---|
| 320 coarse, 24 fine (default) | 13.9 | 29.6 / 55.6 / 77.7 | 80 |
| 160 coarse at dt 2 | 11.6 | 24.3 / 52.6 / 77.2 | 99 |
| 80 coarse at dt 4 | 8.9 | 19.7 / 51.3 / 81.0 | 88 |
| 160 coarse at dt 2, 12 fine at dt 2 | 7.4 | 32.6 / 61.5 / 85.4 | 183 |

- **Steady state:** it is not reached. After the last step the largest change on land is still 1.8 m per
  step (of 50 m relief); the plains are still rising.
  - With the old, low plains, twice the steps (400 coarse, 60 fine) took 41 s: the same land, a little
    more cut up, 28 lakes instead of 34 (`out/long/`).
  - So the default is a choice of the wait (taste call 4), not of correctness.
- **Memory:** 174 MB is over the 100 MB proposal. That proposal is for what the server keeps; the bake's
  arrays at their peak are a different number. What the server would keep is height and surface,
  2 x 4 bytes per cell: 28 MB for Terra and 113 MB for the huge planet as f32.
  - The bake's peak can come down by storing directions as i16 or computing them, by not cloning h each
    step, and by a flat neighbour table instead of the HashMap. That is W2's work.
- **Disk:** 2.7 MB at 50 m relief, 3.6 MB at 120 m (taller heights compress worse), holds the height (i16, decimetres), the plate of each cell and the river segments.
  W2 adds rock, soil, climate and biome per cell (worldgen.md: about 16 bytes). These are smooth fields
  and compress well. At 120 m the huge planet is already 13.3 MB, over the 10 MB proposal. The
  developer said file size matters little next to performance (2026-11-11), so this is accepted, not a
  problem to fix.

### Determinism
- `cargo test`: the same seed twice gives the same hash, and another seed a different one (small planet).
- Terra, run twice:
  - seed 1: `dc857243a29b98c1` and `dc857243a29b98c1`;
  - seed 2: `d7537fe44cdb9183` (both at 120/60 m).
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
- **`images/sheet-seed-5.png`, `images/orbit-seed-5.png`:** the same for seed 5, Terra's default.
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
  - Seed 11 is a lake district: 432 lakes on a high plateau. Seed 2 has a large open ocean with one long
    arc.
  - Seeds 1, 5 and 7 have the most varied land: ranges, plains and gulfs.
- **Coasts:** the coastlines are ragged in close-ups (flooded valleys, rias). The sea floor is smooth.
- **Higher plains (the developer's call, 2026-11-11).**
  - **The first bake** scaled the relief by the highest peak and had little uplift inside continents. Half
    the land lay within 1.5 m of the sea, 90 % under 7 m and 99 % under 20 m (seed 1, 50 m). The
    continents were lowland plains, with mountains only in narrow belts at the plate edges. ADR 0021's
    noise has plains rising 14 m inland and hills to ±10 m.
  - **Asked "higher plains"**, the uplift inside continents went up and the coarse erosion got twice the
    steps. The old shields went from 0.05 to 0.35 of the peak uplift and the old worn ranges from 0.35 to
    0.8 (`Params::shield_uplift`, `old_range_uplift`). Measured on seed 1, land heights at the 50th, 90th
    and 99th percentile, in metres:

| shields / old ranges | coarse steps | p50 | p90 | p99 | lakes | Terra s |
|---|---|---|---|---|---|---|
| 0.05 / 0.35 (first bake) | 160 | 1.6 | 7.2 | 19.8 | 34 | 15.9 |
| 0.2 / 0.6 | 160 | 4.1 | 12.0 | 21.6 | 48 | 16.4 |
| 0.35 / 0.8 | 160 | 6.3 | 15.2 | 24.1 | 91 | 16.9 |
| 0.5 / 1.0 | 160 | 7.9 | 17.4 | 26.6 | 197 | 15.4 |
| **0.35 / 0.8 (default now)** | **320** | **8.4** | **17.1** | **25.7** | **117** | **18.4** |
| 0.5 / 1.0 | 320 | 10.7 | 20.0 | 28.8 | 136 | 18.3 |

  - The default gives every continent highlands, ranges and cut valleys, not one coast range on a plain.
    Over the 12 seeds the median land height is 8 to 15 m.
  - **The price is lakes:** 49 to 432 per seed (1.5 % of the land on seed 1), mostly small basins where
    uplift outruns the rivers. Seed 11 is a lake district.
  - Stronger plains (0.5 / 1.0) gave more lakes for 2 m more of median height, so I stopped at 0.35 / 0.8.
- **Relief 120 m / 60 m (the developer's call, 2026-11-11):** the defaults in `Params`.
  - Land on seed 1: half above 20 m, 90 % under 41 m, 99 % under 62 m.
  - Over the 12 seeds the median land is 19 to 35 m. Seed 11 is the highest: median 35 m, p99 85 m.
  - The colours are by absolute metres, so the sheets now show brown uplands and grey and white peaks.
- **Default seed 5 (the developer's call, 2026-11-11),** from the sheet of 12 (row 2, first globe): the
  most varied land, with plains, ranges and gulfs. Its land at 120 m: median 30 m, p99 78 m. It is
  `DEFAULT_SEED` in `src/main.rs`; the engine's default seed (1337) moves to it in W2.
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
