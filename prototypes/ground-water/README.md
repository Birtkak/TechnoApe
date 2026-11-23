# Ground-water spike: a water table held by a formula, counted when touched

**Question** (`docs/design/liquids.md`, "The rule": a fluid at rest is a formula, touched it is counted units,
and the formula is the state the simulation settles to). Ground water does not exist today: caves are dry, there
are no springs, a dug well finds nothing, and `caves::water_table` is a shape (the ground smoothed over 40 m, times
0.6, at least 3 m down) used only to keep karst dry. How should ground water be held so that

- (a) a water table exists under the land as a formula,
- (b) a cave or a hole below the table holds water at the table's level and one above it is dry,
- (c) a spring appears where the table meets the surface and feeds the rivers honestly,
- (d) digging a well or draining a cave changes it as counted units that conserve exactly?

**Answer, short.**
1. The table is the steady state of Dupuit-Darcy flow with rain recharge, solved by the bake on its 8 m cells with
   the sea, rivers and lakes as fixed heads and the ground as a ceiling. Where the solve presses against the
   ground, the cell seeps: that is the spring, and its flow is the recharge uphill of it, to the micro-unit.
2. Touched ground water is counted per **8 m cell** (one head for 8 x 8 columns), not per column. Then the bake's
   field is exactly its rest (0.0012 units off after 20,000 steps). Per 1 m column it is not: the columns' own rest
   stands a mean 8 to 22 cm and up to 1.4 m from the bake's field.
3. A cave, well or dug hole is open water in the engine's gaps. It trades with its cell by Peaceman's well index,
   which puts a 1 m well's level within 0.5 % of the 1 m model's. It fills at a rate set by the rock (exactly
   10 times faster for 10 times the K) and draws a Thiem cone (within 4 %).

## The formula

Steady, unconfined, Dupuit (flow horizontal, one head a column of rock):

    div( K b grad h ) + R = 0         where h < z_ground
    h = z_ground, spring flow q_s = R + div(K b grad h) >= 0     where it would rise over the ground
    h = h_fixed                       on the sea, a river reach's surface, a lake's level
    b = h - z_base

| term | physical quantity | where the game has it |
|---|---|---|
| `h` | hydraulic head: the height the table stands at, m | new: the bake's output |
| `z_base` | the aquifer's floor: the top of the first rock that holds no water (shale, unbroken granite), m | new: from `data/rocks.toml` layers plus a new `k` a rock |
| `b` | saturated thickness, m | derived |
| `K` | hydraulic conductivity, m/s (how easily the rock passes water; Darcy's law `q = -K grad h`) | new: a field on rock rows |
| `K b` | transmissivity, m2/s | derived |
| `R` | recharge, m/s: the share of rain that soaks down to the table | new share times the bake's rain less evaporation (what `Reach::flow` already gathers) |
| `S_y` | specific yield: the share of the rock's volume that drains | new: a field on rock rows. It drops out of the steady state. It sets how many units a metre of table holds and how fast the table moves |
| `h_fixed` | the heads that never move at rest: sea level, river surfaces, lake levels | the bake's rivers and lakes |

Closed forms exist for uniform cases. They are the tests' references:
- between two equal heads `L` apart on a flat floor: `b^2 = b0^2 + (R/K) x (L - x)`;
- a round mound: `b^2 = c - R/(2K) r^2`.

The real map has neither, so the formula is a field the bake solves. It uses the same discrete equations the
counted simulation steps (Gauss-Seidel over the cells, the seepage ceiling as a clamp), so the field is the
simulation's rest by construction. The test is that stepping it does not move it.

## The counted model (`src/gw.rs`)

- A column (or cell) holds `w` **micro-units**: `liquid::U` x 65,536 a cubic metre, an integer.
  - Ground water moves less than a unit a step. 600 mm of rain a year is 0.1 units a column a game day, so whole
    units would round every flow to zero.
  - It trades whole units with open water and keeps the remainder, so the ledger stays an integer.
- The head comes from `w` through the storage curve: `S_y` in rock, 1 in a void (a cave or a dug well).
  - **Digging changes the curve, not `w`.** The pore water of the dug-out rock runs into the hole, the head drops,
    and the neighbours refill it. A dig needs no special case.
- **A step:**
  1. Darcy on every link: `q = K_harm (b_i + b_j)/2 (h_i - h_j) dt`, truncated to whole micro-units, the same
     number off one and onto the other.
  2. Then rain, then a pump (it takes only what stands over the void's floor).
  3. Seepage over the ground goes to that column's spring account.
  4. A fixed head is reset, and the difference is booked to its account.
- **The ledger:** held + to the fixed heads + springs + pumped - rain never changes. It is asserted after every
  step of every test, along with no negative column.
- With an arithmetic-mean thickness on a flat floor, the link flux is `K/2 (b_i^2 - b_j^2)`. The 5-point
  difference of a quadratic in `b^2` is exact, so **the Dupuit parabola and mound are the discrete model's exact
  rest**. The tests see them to 0.01 units.

## Results

`cargo test --release -- --nocapture` (9 tests) and `cargo run --release` (every number below).

Seeing each test fail first:
- The first eight failed with the Darcy links switched off: 4,152 units off the formula, the spring 1.7 % of its
  recharge, and so on.
- The well-index test failed on the old rule that a pump needs a void.
- The baked test then failed once more, 19,600 units off, on a real bug: a fixed head over a sea bed read its
  column's water, not the sea. It was fixed in `step`.

### The formula is the rest (step 2)

| case | from the formula, after | from elsewhere, settles to within |
|---|---|---|
| slice, 101 columns between two rivers, K 1e-5, 600 mm/yr | 0.0071 units (20,000 steps of 400 s) | 0.0071 units from 10 m (all low), 0.0121 from 25 m (flooded, springing) |
| 3D patch 41 x 41 m, its edge reading the mound's formula (as a stored patch borders unstored ground), K 1e-4, 900 mm/yr | 0.0110 units | 0.0113 units from 19 m |
| bake-like stretch, 40 x 40 cells of 8 m: sea, a river down a valley, two rocks in 40 m stripes (K 1e-5 / 1e-6), rain 316 to 631 mm/yr, a hill the table cuts (12 seeping cells) | 0.0012 units (20,000 steps of 600 s) | 0.0014 units from sea level everywhere |
| the same at rest, 1,000 steps | rain in 235,175,382,000 micro-units, out 235,175,382,002; springs carry 53 % | |

### The cases (step 3)

All on a flat table 20 m over the floor, held at the edges of a 61 m square, `S_y` 0.2, unless named.

| case | measured | check |
|---|---|---|
| a cave 17 to 23 m (through the table), drained dry, K 1e-5 | back to 19.99976 m (within 1 unit) in 812,990 s | fills to the table, never over it |
| a cave 22 to 26 m (over the table) | holds nothing after 10,000 steps; the table unmoved | dry |
| a well dug from the ground to 5 m under the table, drained, 90 % refilled | K 1e-5: 7,000 s; K 1e-4: 700 s; **ratio 10.00** | the rate is the rock's |
| pumping 2 L/s from it, K 1e-4 | drawdown 0.569 m at 1 m, 0.452 at 2 m, 0.224 at 8 m, falling with distance out to 25 m; s(2)-s(8) 0.2286 m against Thiem's 0.2206 | a cone (within 4 %) |
| pumped 20 times harder | runs dry, and the pump gets only the inflow | |
| the pump stopped | back within a unit of the table in 155,820 s | recovers |
| a spring: a 60 m slope from a divide to a cut where the table meets the ground, 1 m of rain a year | spring 2,551,800,000 micro-units against 2,551,800,000 rained in over the window; no seep uphill | **flow = the recharge uphill, exactly** |
| a 2 L/s well held per 8 m cell with Peaceman's term `Q/(2 pi T) ln(0.198 dx / r_w)`, against the 1 m model's well | 0.8345 m against 0.8386 m (the cell's own head 0.4951) | **within 0.5 %** |

### The scale of the formula (the surprise)

The same 320 m stretch was solved on 8 m cells (the bake) and on 1 m columns. The 8 m field was read bilinearly at
each column and clamped to the column's ground. Both were solved to a residual under 1e-6 of the rain.

| | mean | median | p99 | max | columns within 1 unit | within 1/64 block | within 0.1 m |
|---|---|---|---|---|---|---|---|
| two rocks in stripes | 223 mm | 105 mm | 1,274 mm | 1,388 mm | 0.2 % | 9.2 % | 49 % |
| one rock | 82 mm | 66 mm | 343 mm | 455 mm | 5.7 % | 24.6 % | 66 % |

A per-column store woken from the bake's field would drift by decimetres to its own rest. A cave pool would be seen
rising or sinking 10 cm for no reason, and a patch would never come back within a dead band of the formula to be
forgotten. Two things cause this:
- the 8 m grid samples the ground and the rock boundaries coarsely (the seepage ceiling and the K jumps are first
  order);
- the column scale cannot be solved globally: 226 M columns.

**So the counted model lives on the bake's cells.** A well or cave is a sub-cell feature with a well index, which
is how reservoir simulators handle wells smaller than their grid.

### Cost (step 4)

Single thread, no fat LTO, measured alone on the PC (`tools/slot.sh measure`):

| | ms a step | per column |
|---|---|---|
| 10,000 disturbed columns | 0.155 | 15.5 ns |
| 100,000 disturbed columns | 1.65 | 16.5 ns |
| the bake: one face, 768 x 768 cells, Gauss-Seidel at omega 1.95 to 1e-6 of the rain | 4.9 s, 475 sweeps (10 ms a sweep) | six faces about 29 s |

Held per 8 m cell, 10,000 disturbed columns are about 160 cells, so the step cost is negligible. The explicit
step's stable bound is `S_y dx^2 / (4 K b)`: at 8 m, 2,000 s for sand and 200 s for gravel, so a 1 Hz step never
nears it. The bake's solve would nearly double today's 36 s bake. Multigrid would cut it; nothing was tuned.

### Rates against the calendar

The relaxation time is `tau = S_y L^2 / (K b)`, with b = 20 m. A game day is 1800 s, a game year 601,200 s. The
calendar's factor is 52.49 (ADR 0187's).

| rock | K m/s | S_y | L | real | game time at real rates | at x52 |
|---|---|---|---|---|---|---|
| clean sand | 1e-4 | 0.25 | 100 m | 14.5 days | 2.1 game years | 13 game days |
| silty sand, sandstone | 1e-5 | 0.15 | 10 m | 0.87 days | 42 game days | 0.8 game days |
| silty sand, sandstone | 1e-5 | 0.15 | 100 m | 87 days | 12.5 game years | 79 game days |
| fractured limestone | 1e-5 | 0.05 | 100 m | 29 days | 4.2 game years | 26 game days |
| basalt, jointed | 1e-6 | 0.05 | 100 m | 289 days | 42 game years | 265 game days |
| fractured granite | 1e-8 | 0.01 | 10 m | 58 days | 8.3 game years | 53 game days |
| shale (an aquitard) | 1e-11 | 0.02 | 10 m | 317 years | | (why it is a floor) |

What a player meets, from the tests:

| | real rates | at x52 |
|---|---|---|
| a well in silty sand, 90 % refilled | 7,000 s (3.9 game days) | 133 s |
| the same well in sand | 700 s (0.4 game days) | 13 s |
| a 6 m cave drained in silty sand, back to the table | 813,000 s (1.35 game years) | 8.6 game days |
| a dry sand well after hard pumping, back within a unit | 156,000 s (87 game days) | 1.7 game days |

**Is recharge weather-driven?** Yes: it is rain, and ADR 0187's reading takes the climate's exchange at the
calendar's pace. But the table's height depends on `R/K` alone. Recharge sped up 52 times with Darcy at real rates
would multiply every mound's `b^2 - b0^2` by 52 (its height over the rivers about 7 times) and drown the hills in springs. So the factor cannot be
split: **either the whole ground-water clock (recharge and Darcy together, a step of 52 dt) runs at the
calendar's pace, or none of it does.** Either way the formula is unchanged, because only `R/K` enters it. The
pump and the dig are a player's hands. A pump's rate is its own, but the refill is the rock's, so it follows
whichever clock is chosen.

## What surprised

- **The bake's field is not a 1 m column's rest.** The gap is decimetres, too large for any dead band, so the
  store's grain is the cell (above).
- **The analytic Dupuit shapes are the discrete model's exact rest** (with arithmetic-mean thickness), so the
  formula and the simulation can agree to hundredths of a unit, not to a tolerance.
- **Springs close exactly:** the spring at a cut gave every micro-unit the slope rained in. In the bake-like
  stretch, springs carried 53 % of the rain, and the rest went to the sea and the river through the rock.
- **Recharge is tiny in units:** 0.1 units a column a game day. Ground water needs a finer integer than open
  water, or nothing moves.
- **Digging needs no rule:** the dug rock's pore water is already counted, and only the storage curve changes.

## What the engine would need (step 5)

Read, not edited:
- `crates/engine/src/liquid.rs` (`Ground`, `materialize`, the reach ledger)
- `crates/engine/src/caves.rs` (`water_table`, karst kept over it)
- `crates/engine/src/bake/mod.rs` (`Map`: `h`, `rock`, `rivers`, `lakes`, `climate.rain` per air column)
- `bake/rivers.rs` (`Reach::flow` = rain less evaporation, gathered)
- `data/rocks.toml`

**Data** (routine defaults from textbook ranges, with tests):
- each rock row gets `k` (m/s) and `yield`;
- a recharge share (of rain less evaporation) per biome or soil;
- the aquifer's floor is the top of the first layer with `k` under about 1e-9, at most the crust's floor.

**Bake** (a map version and a new world generation):
- after rivers, lakes, climate and rock, it solves the field per cell;
- it stores `table` (f32 m over the sea, 14 MB for the 4 km planet's 3.54 M cells, or i16 cm at 7 MB) and each
  seeping cell's spring flow;
- the solve must cross face seams (here a face's edge was closed) and run single-threaded in a fixed order. A
  red-black or multigrid solve is deterministic too, but gives other bits;
- clients get the map bytes and never re-solve;
- karst and tubes are cut against the new table, not `caves::water_table`.

**`Ground`:**
- `table(c) -> i64`: the cell's head in units;
- `aquifer(cell) -> (base, k, yield, recharge)`;
- `generated` per gap, not per column: a closed gap whose floor is under the table holds water up to
  `min(table, ceil)`. Today `generated(c)` is one range, the open water's. Caves under the table become wet
  generated water, drawn by clients like the sea.

**Stored** (server only):
- a sparse `FxMap<cell, i64>` of micro-units beside `Liquids`, stepped at 1 Hz in index order;
- it is woken by an edit that opens a void under a cell's table, or by a gap there losing water;
- it is forgotten when every cell is within a dead band of the field (as rivers are);
- a gap's open water trades with its cell by Peaceman's index, in whole units, and the remainder stays in the
  cell;
- springs book to the account the cell's runoff goes to (`Ground::reach`), so a reach's flow is unchanged at
  rest. The recharge is a share of the very flow it already carries, not an addition;
- the save gets one new section, which is empty for old saves. Nothing new goes on the wire: rock water is unseen,
  and gap water is sent as today.

**Stays formula:**
- the table;
- every cave or gap under it (full to the table);
- the springs' flow at rest.

**What would break:**
- Old worlds would suddenly have wet caves, and their karst sits by the old heuristic. Old worlds must stay dry,
  gated by generation as caves were.
- Determinism holds if fluxes stay `f64` truncated in index order without fused multiply-add (Rust's default),
  and nothing iterates a hash map's order.

**Steps, each a test seen failing first:**
1. The rock rows' `k` and `yield` and the bake's field (a map version). Test: the stored field is the counted
   model's rest.
2. `generated` per gap. Test: a cave under the table loads full and one over it dry.
3. The cell store and the well index. Test: a dug well fills, a pump draws a cone, and the ledger closes with
   `Liquids`.
4. Springs into the reach accounts. Test: a reach's flow at rest is unchanged.

## The developer's calls

1. **The ground-water clock:** real rates (a well in silty sand refills in 3.9 game days, a drained cave in 1.35
   game years), or the calendar's 52 times for the whole of ground water (133 s, 8.6 game days). Recharge and
   Darcy cannot be split (above).
2. **Flooded caves:** a real table floods some generated caves (sumps, lakes in caves) and puts every cave's water
   at one level. Wanted? If so, karst and tubes move with the new generation.
3. **Visible springs:** should the bake start a stream (a new reach) where a seeping cell's flow is large, or show
   wet ground and a trickle?
4. **Bake time:** about +29 s, nearly doubling the bake, unless a multigrid pass is built.

The routine calls taken here:
- the cell as the store's grain;
- micro-units;
- Dupuit;
- Peaceman's index;
- K and S_y from textbook ranges;
- the floor rule.

## Not covered

- The engine was not driven. This is a standalone copy with flat ground and no gaps; a void stands in for a gap's
  open water.
- Dupuit only:
  - no vertical flow;
  - no perched tables over a lens of clay;
  - no confined aquifers;
  - no lag through the unsaturated zone;
  - no karst conduit flow (it would be the caves' open water).
- Constant recharge: no seasons, no droughts.
- No tides on the sea's head, and no salt.
- The face solve was a tiled synthetic stretch with closed face edges, not a real map.
- The values of K and S_y were not checked against a real aquifer. Cross-machine determinism was not run.

## How to run

    cd prototypes/ground-water
    cargo test --release -- --nocapture
    cargo run --release
