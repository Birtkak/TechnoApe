# Air grid spike: Terra's air (ADR 0010, `docs/design/air.md`, worldgen plan W3)

**Question.** Can one coarse air grid (columns of about 128 m, four layers, the eight passes of air.md)
run for the whole planet inside the server tick, under 0.5 ms of it? And does running it to a steady state
on the baked heights give a believable climate: temperature, wind and rain, with rain shadows behind
ranges, wet windward coasts and dry belts? The bake's climate (worldgen stage 5) will be this model.

**Answer: yes on Terra, with a caveat for larger planets.**
- **Cost on Terra (4 km, N 6144): 0.23 ms per tick** on one thread (a step of 4.6 ms once a second,
  spread over 20 ticks), 0.07 ms on all 12 threads. Under the 0.5 ms target.
  - The 8 km planet costs 0.95 ms on one thread (0.27 on all), the 12 km one 1.89 ms (0.48 on all).
    Over the target single-threaded; it scales with the column count, so a larger planet runs the grid on
    a worker or splits faces (air.md's checkerboard).
- **Steady state:** about 20 days of model time from the first guess. Day 30 against day 60: 0.04 K mean
  difference per column, rain correlation 1.000. The day repeats; there is no drift.
- **The bake's step:** 10 s steps give the same climate as the live 1 s step (0.08 K, rain correlation
  0.993). 30 s steps do not (0.70 K, rain correlation 0.87). So the bake runs 30 days of 120 steps:
  **5.4 s for Terra** on all threads.
- **Water's ledger:** exactly constant, every day of every run (a test, and an assert in every climate run).
- **Determinism:** the same state on 1 and 4 threads (a test).

Throwaway code: its own cargo project. `prototypes/world-bake` uses it as a library for stage 5.

## Run it
```
cd prototypes/air
cargo test --release                       # ledger, determinism, the bands, the seams
cargo build --release
target/release/air.exe mode=cost           # a step on the 4, 8 and 12 km planets, 1 and all threads
# heights from the bake (cd ../world-bake; target/release/world-bake.exe seed=5 planet=6144 draw=0):
target/release/air.exe mode=climate bake=../world-bake/out/seed-5-planet-6144/bake.bin.gz days=30 out=out/s5
target/release/air.exe mode=compare a=out/x/means.bin b=out/y/means.bin   # two runs' last days
```
`climate` takes `days=`, `dt=` (seconds a step) and `planet=`; it prints one JSON line and writes
`air-sheet.png` and `means.bin`. `AIRDBG=1` prints zonal means per layer and the wettest columns.

## The model (`src/lib.rs`)
- **Grid:** the bake's cube-sphere (its `grid.rs`, the engine's axes and warp). Columns of N/128 per face
  edge, so 48 x 48 a face on Terra, 13,824 columns, 55,296 cells. Each column has four edges; an edge's
  normal and width are its true shared boundary on the sphere (two cell corners), so a cell's faces close
  exactly over seams and corners.
- **Layers:** the surface layer to 30 m over the ground, low to 150 m, middle to 500 m, high to 1,500 m
  (air.md).
- **State:** per cell potential temperature (f32), vapour, cloud and one pollutant (u32 units of 1/4096
  block, ADR 0020); wind per edge and layer (f32); per column the ground's temperature, soil water and
  snow. 1.6 MB for Terra.
- **Determinism:** water and dirt are integers that are only ever moved, so their ledgers are exact.
  Heat and wind are f32 in a fixed order: the same on one machine, not across machines (only the server
  runs it). Rayon only maps cells to values; each reads the old state.

| # | Pass | What it does here |
|---|---|---|
| 1, 2 | sun, ground | sunlight by the sun's height (no tilt: the subsolar point circles the equator once a 1,200 s day), less cloud and albedo (sea, land, snow); outgoing heat A + B T, less under cloud; the ground warms the surface air; the upper layers cool toward a fixed radiative reference per column |
| 3 | wind | relaxes toward bands (trades, westerlies, polar easterlies; toward the equator in the tropics, poleward in the middle latitudes, a balancing return aloft) plus a breeze toward warmer columns; the surface wind is slowed across height steps, the low layer by how much of it a mountain fills |
| 4 | carry | first-order upwind, in whole units: a cell gives a share of what it holds through each outflow edge (at most 90 % in a step), computed the same from both sides |
| 5 | lift | air that piled up in a layer rises to the next, air that left is replaced from above (sinks, warms); a top that piles up sinks; convection mixes a layer with the one above where it is warmer |
| 6, 7 | water, land | evaporation from water and wet soil toward saturation at the ground's temperature, faster in wind; vapour over saturation to cloud, with latent heat; cloud over a limit rains, or snows below 0 C, washing dirt down; rain fills the soil, which seeps to the rivers; snow melts |
| 8 | events | not built (storms, lightning, fog are reading the state; they cost nothing measurable) |

**Compressed by design:** the air cools 0.1 K per metre near the ground (a 120 m peak is 12 K colder than
the coast) and 0.015 K/m above 150 m. Heat capacities are small enough that a 20 minute day gives a swing.
Every constant is in `Params` (later `data/air.toml`).

## Measured (release, fat LTO, `target-cpu=native`, Ryzen 5 5600X; `out/.lock` held for the timings)

### Cost (question 1)
| planet | columns | cells | state | step, 1 thread | per tick, 1 thread | step, 12 threads | per tick |
|---|---|---|---|---|---|---|---|
| Terra, 4 km (N 6144) | 13,824 | 55,296 | 1.6 MB | 4.59 ms | **0.229 ms** | 1.43 ms | 0.072 ms |
| 8 km (N 12288) | 55,296 | 221,184 | 6.3 MB | 19.0 ms | 0.951 ms | 5.41 ms | 0.270 ms |
| 12 km (N 17376, the bake's default) | 110,976 | 443,904 | 12.7 MB | 37.7 ms | 1.887 ms | 9.53 ms | 0.476 ms |

A live step is once a second of world time, spread over the 20 ticks (air.md).

### Does it look like weather (question 2)? Seed 5, Terra, day 30
Zonal means (latitude band, surface air C, rain mm/day over all and over land, the land's day swing K):

| lat | T | rain | rain, land | swing, land |
|---|---|---|---|---|
| 0-10 | 21.7 | 1.03 | 1.58 | 6.3 |
| 10-20 | 18.9 | 0.30 | 0.62 | 6.2 |
| 20-30 | 16.5 | 0.13 | 0.30 | 6.2 |
| 30-40 | 12.5 | 0.08 | 0.13 | 5.1 |
| 40-50 | 9.7 | 0.23 | 0.23 | 4.0 |
| 50-60 | 0.0 | 0.31 | 0.27 | 3.2 |
| 60-70 | -12.0 | 0.26 | 0.35 | 1.9 |
| 70-80 | -14.4 | 0.15 | 0.25 | 1.0 |
| 80-90 | -16.6 | 0.14 | 0.18 | 0.4 |

- **A wet equator, dry belts at 20 to 40, wetter westerlies at 40 to 70, cold dry poles.** The
  circulation does it, not a rain-by-latitude rule: the trades converge at the equator and lift, the air
  sinks and dries at about 25 degrees.
- **Rain shadows:** land where the surface wind blows up the slope gets **1.18 mm/day**, where it blows down
  **0.25**: 4.7 times. Seeds 1 and 9: 0.95 against 0.20, 0.89 against 0.19.
- **Wet coasts:** land rain by distance to the sea, in columns: 0.48 (next to it), 0.69 (2-3), 0.49 (4-7),
  0.35 (8 and more).
- **Ice at the poles:** the surface air is under -10 C poleward of 60 degrees; peaks are colder by height.
- **Day and night:** the land's air swings 6 K at the equator, 1 K at 70 degrees; the sea's under 1 K.
- **Dusk rain:** rain over land by local hour peaks at 17-18 h (and again at 5-6 h, the coldest air), about
  25 % over its low.
- **Sea breeze: weak.** At the coast the wind toward land swings by about 0.15 m/s over the day, strongest
  offshore at dawn (a land breeze) and onshore in the evening. Neighbouring columns mix their heat within a
  minute, so a 128 m grid has little contrast at the coast itself. It is a knob (`thermal`) for the live
  weather, not something the biomes need.
- **The sheets:** `images/air-sheet-seed-5.png` and `-seed-1.png`. Top row: land and height; mean surface
  temperature with the surface wind; rain on a log scale (the sea darker). Then the land's day swing, and the
  cloud at five times of the last day (the night side dark).

### Smear (question 3)
A puff of dirt over the sea, carried 60 live steps: its spread is 1.3 columns (standard deviation) and 19
columns hold over 1 % of its peak. First-order upwind smears a plume over about 3 columns in a minute. For
the climate this does not matter. For smoke plumes in play it may; a limited second-order scheme (van Leer)
would be about 2x the carry's cost. Not needed for the bake.

### Ledger (question 4)
The sum of vapour, cloud, soil, snow and the sea is the same unit for unit after every day of every run
(60 days asserted), and in the test over 2,400 steps. Dirt the same (in the air plus washed down).

### A repeating year (question 5)
There is no year: no tilt (ADR 0022; seasons were not chosen for now). The day repeats instead. The change
of the daily mean temperature per column falls from 3 K (day 1) to 0.01 K (day 20) and 0.003 K (day 30);
the rain map changes by under 1 % a day after day 25. Day 30 and day 60 agree to 0.04 K and 1 % of rain.

### Day swing (question 6)
6 K at the equator, 3 K at 55 degrees, under 1 K over the sea. That is mild: a night is cool, not
dangerous. It is set by the ground's and the air's heat capacity (`cap_land`, `cap_air`); making nights
dangerous is the developer's taste call when survival's cold is built, and does not change the biomes.

### Mistakes found on the way (each measured, then fixed)
- **The planet warmed to 50 C:** the upper layers relaxed toward "the layer below plus 4 K", and the sinking
  air brought that heat down: a ratchet. They now cool toward a fixed radiative reference per column.
- **Rain in the dry belts:** the return winds aloft carried four times the air the surface brought, so
  vapour piled up aloft at 20 degrees and rained there. The bands now balance over the column.
- **Rain in square blobs on the seams:** the flux used the centre-to-centre direction and a guessed width,
  which do not close around cells where the grid bends. A steady east wind gained 0.7 of a column's width
  a second at a corner. With the true boundaries: under 0.02 (test
  `the_grid_folds_over_seams_with_four_edges_a_column`, seen failing at 0.70).

## Tests (each seen failing first)
- `water_and_dirt_are_never_made_or_lost`: 2,400 steps, the ledger asserted every step. Failed with the
  sea's evaporation not taken from the sea.
- `the_same_world_runs_the_same_weather`: 1 thread against 4. Failed with a thread-dependent term in the
  carry.
- `trades_blow_from_the_east_and_westerlies_from_the_west`. Failed with the bands' sign flipped.
- `the_grid_folds_over_seams_with_four_edges_a_column`. Failed at 0.70 with the old edge geometry.

## Not done
- Coriolis turning as a force (the bands stand in for it), storms, lightning, fog, rooms, fires and chimneys,
  clients' deltas, radiation.
- Seasons: no tilt until the developer picks one; the model takes the sun's direction from one function.
- Lakes and rivers: rain on land seeps and runs off to one sea ledger; the bake routes it (stage 6).
- A second-order carry (question 3).
- Data file: `Params` becomes `data/air.toml` when the grid moves into the engine.

## The live grid's tick (W3, 2026-11-12)

**Question** (ADR 0010, plan W3): what does the live grid cost the server's tick on Terra and the huge planet,
target under 0.5 ms, once it has the sky's seasons (ADR 0055: 20 degree tilt, 30 minute day, 54-day year)? And
can the bake keep a cheap tilt-free climate, or must it run a year?

**Answer:** the step runs on a worker; the server's own thread pays **under 10 microseconds** per hand-over,
so the tick's cost is the hand-over, not the step. The CPU the step takes elsewhere, averaged per tick, is far
under the target on both planets at the live step of 5 s. **The bake must run a year:** a tilt-free bake is off
by 15 to 20 % of rain at 50 to 70 degrees, a year repeats the next within 1 %.

`target/release/air.exe mode=tick [dt=5]` (the test world, tilt 20 degrees, a 1,800 s day):

| planet | columns | step, 1 / 4 / 12 threads | CPU per tick at dt 5 s, 1 thread | at dt 1 s | hand-over on the server's thread (send, take, worst) | state |
|---|---|---|---|---|---|---|
| Terra, 4 km | 13,824 | 4.43 / 1.47 / 1.27 ms | **0.044 ms** | 0.214 ms | 3.5, 0.5, 13 us | 1.6 MB |
| huge, 8 km | 55,296 | 17.5 / 5.6 / 4.4 ms | **0.175 ms** | 0.862 ms | 3.7, 0.5, 13 us | 6.3 MB |

- The worker steps the state, then builds a view (the surface air, rain and runoff per column): 1.6 ms on Terra
  with all threads, under the 1 s the server waits before taking it back.
- At dt 1 s the huge planet's step is 0.86 ms of CPU a tick on one thread, over the target as a share of a
  core; at 5 s it is 0.18. The spike's earlier finding holds: 10 s steps give the 1 s climate (0.08 K, rain
  correlation 0.993), so the live step is **5 s** (once per 100 ticks) and the bake's 10 s.

`target/release/air.exe mode=year [spin=20] [held=n] [years=2]` (Terra's test world, dt 10 s):

| bake's year | cost (12 threads) | mean abs dT per column | rain over the planet | rain correlation | worst band |
|---|---|---|---|---|---|
| one continuous year, after 20 days at the equinox | 12.7 s + 4.9 s | (the reference) | 0.593 mm/day | | |
| the next year after it (`years=2`) | 13.6 s | 0.10 K | 0.5930 / 0.5936 | 1.000 | 0.6 % |
| only 5 days' spin-up, then year 1 against year 2 | 1.5 s + 13.4 s | 0.12 K | 0.5835 / 0.5936 | 1.000 | 3 % |
| four held days of the year (1/16, 3/16, 9/16, 11/16), 12 days each | 11.5 s | 0.10 K | 0.5965 | 0.999 | 3 % |
| the same, 4 days each | 3.8 s | 0.32 K | 0.5871 | 0.998 | 1.5 K, 6 % at 50-60 degrees |
| the same, 2 days each | 1.9 s | 0.55 K | 0.5858 | 0.995 | 1.7 K, 14 % |
| one held day at the equinox: the bake without seasons | 0.5 s | 0.67 K | 0.5971 | 0.992 | **18 % less rain at 60-70 degrees, 1.4 K** |

- The seasons move rain poleward in summer; the high latitudes keep snow and soil water across seasons, so
  a held day needs about 12 days to settle there, and four of them cost what the real year costs.
- So the bake runs **10 days at the equinox, then one continuous year** at 10 s (about 16 s on 12 threads, from
  5 s for the old tilt-free day). The year ends at the spring equinox, world time 0, so the live grid starts
  from the bake's last state with nothing to settle.
