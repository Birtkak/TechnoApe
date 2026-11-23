# Water-phases spike: ice, water and vapour as one ledger, moved by heat

**Question** (`docs/design/liquids.md`, "Phases: one substance, three states" and step 5; ADR 0183: phase by
temperature and pressure, water first, one exact ledger, rates not switches, nothing fake):

1. What is the simplest rule that moves counted units (U = 4096 a block, `liquid::U`) between an ice block, water
   in a gap and vapour in an air cell, from temperature, heat flow and latent heat, with the boiling point from the
   air's pressure?
2. Where is heat held: (a) only near a heat source or a phase boundary, the rest reading the air, or (b) a
   temperature in every stored column? Memory and time for 100,000 stored columns with a few thousand active cells.
3. Does it behave (ice block, pond, pot, thin air and no air, the sea's formula against stored sea)?
4. What does real physics take in game time, and where would a time scale be needed?
5. What would the engine need?

**Answer, short.** The rule is the enthalpy method in whole units and millijoules, with a vapour flux from
Clausius-Clapeyron, and both ledgers (units and energy) close exactly in every step of every test. Real rates play at
the size of a player's hands (a pot boils in 21 min, goes dry in 2.8 h) but not at the size of the calendar: the
game's year is 52 times shorter than a real one, so a pond gets 6 cm of ice in a game winter, a 1 m3 ice block in
+10 C air lasts 3.2 game years, and stored sea takes 63 game years to grow its sheet. That is a call for the
developer (options below). Heat should be held sparsely (a): **0.08 ms a step and 0.24 MB against 2.4 ms and 3.4
MB** for a temperature per stored column. The sea's formula is exactly where stored sea settles, once the sea gives
**15.4 W/m2 from below**. That value is what makes the bake's sheet (one block where the year counts -8 C) the rest
state. The formula then says the sheet should be thicker where it is colder (2.2 blocks at -16.6 C, 4.2 at -30 C)
and that thinner ice should lie between -1.1 and -8 C, where the bake shows open sea.

## The rule

A **node** is the water at one place: `i` units of ice, `w` of water and an enthalpy `h` in mJ, counted from
"every unit at 0 C in the phase it is in". There is one node per ice block, per gap's water (its ice as a layer on
top), and per pot. An **air cell** holds vapour `v` units, a temperature, a total pressure `p`, and the heat it took
in.

1. **Heat flows** go into `h` in whole millijoules and are booked to where they came from (the air cell, a fire, the
   sea below), so energy is an integer ledger:
   - to air: `q = (h_c + h_r) A (T - T_air)`;
   - through an ice layer `d` thick over water at 0 C: `q = (0 - T_air) / (d / k_ice + 1 / h)`. This is
     quasi-steady: the ice holds no heat, which is fair because ice's Stefan number at 10 K is 0.06;
   - from a fire or the sea's floor: a fixed number of watts.
2. **Melt and freeze** (`Node::settle`): when `h >= L_f` per unit and there is ice, whole units become water and
   `h` drops by `L_f` each. When `h <= -L_f` per unit and there is water, the reverse. While both phases are there,
   `|h| < L_f` per unit and the node is at 0 C. Otherwise `T = h / (units x c)`.
3. **Vapour** (`exchange`): the flux is `J = beta (p_sat(T_s) - p_v)` in kg/m2/s, with the saturation pressure
   `p_sat = 611.657 Pa x exp(-(L M / R)(1/T - 1/273.16))` over water (`L_v`) or over ice (`L_s = L_f + L_v`).
   - Under the boiling point (`p_sat < p`), the vapour diffuses through the air:
     `beta = 0.622 h_c / (c_p p)` (the Lewis analogy).
   - At or over it (`p_sat >= p`, always the case in no air), the vapour pushes the air aside and leaves at the
     kinetic limit (Hertz-Knudsen, `beta = sqrt(M / (2 pi R T))`). It is limited by the heat the node holds over
     the transition point, so a pot stays at its boiling point and the fire sets the rate.
   - `J A dt L` mJ moves from `h` into a credit `vc`. Each whole unit's worth of credit moves one unit to the air
     cell's `v`, and the unit carries its sensible heat. Deposition (frost) runs the same way backwards.
4. **The transition point** comes from the same curves. Over the triple point (611.657 Pa) the boiling point is
   `1/T_b = 1/273.16 - ln(p/611.657) / (L_v M/R)`. Under it there is no liquid, and ice leaves at the
   sublimation point (`L_s` in place of `L_v`). In no air, that point is absolute zero.
5. **Ledgers**, asserted after every step of every test:
   - units: ice + water + vapour (+ melt run off) never change;
   - energy: `sum(h + vc + L_f w) + L_s v + air heat + sources` never changes, to the millijoule. `L_s` is exactly
     `L_f + L_v` per unit, so a unit taken round all three phases closes to the millijoule.

`L_v` is fitted (2.404 MJ/kg) so that the liquid curve passes the triple point and 100 C at 1 atm. One value serves
every temperature, so the curve is within 4 % of the real one (2245 Pa at 20 C, real 2339).

**The formula for untouched sea ice** (the state the column settles to, `rest_ice`): at rest the heat up through
the ice equals the sea's heat from below `F`:

    d_rest = k_ice ((0 - T_year) / F - 1 / h)        (0 if negative; the whole depth if F = 0)

`F = 8 / (1 / k_ice + 1 / h) = 15.4 W/m2` makes `d_rest` exactly one block at the bake's `t_sea_ice` = -8 C
(`bake/life.rs`, on its `cold` year mean).

Shortcuts, all named in the code:
- A block of ice is the same U units and mass as a block of water (real ice is 917 kg/m3, so it would be 1.09 blocks).
- Fresh water freezes at 0 C, the sea too (real sea water freezes at -1.8 C).
- The water under ice is one temperature, so it never keeps 4 C under the ice as a real lake does.
- No lateral heat flow between columns.
- An ice block stays a cube as it shrinks, its melt leaves at 0 C into the gap under it, and the ground under it is
  not followed.
- `c_p` and the molar mass ratio are Earth's air on every body (Mars's CO2 would give 0.41, not 0.622).
- A deposited unit arrives at 0 C.
- The last half unit of a block melts at the area of a 5 cm cube (this is why the block melts 3.3 % early, below).

## Results

`cargo test --release` (6 tests) and `cargo run --release` (every number below).

### Transition points

| air | Pa | water leaves at |
|---|---|---|
| 1 atm | 101,325 | 100.00 C |
| a 1 km peak on Terra (scale height 4.2 km at 19.62 m/s2) | 79,895 | 93.75 C |
| the 3 km air top on Terra | 49,673 | 81.87 C |
| Everest, 0.33 atm | 33,437 | 72.55 C (real 71) |
| Mars-like, 700 Pa | 700 | 1.96 C |
| Mars, 0.006 atm | 608 | -0.07 C (under the triple point: ice goes to vapour) |
| 1 Pa | 1 | -62.3 C |
| no air | 0 | -273.15 C |

### The tests

| test | measured | real-world figure / check | pass |
|---|---|---|---|
| a 1 m3 ice block at 0 C in +10 C air, h = 5 + 5 W/m2/K, air at ice's vapour pressure at 0 C (51 % humidity) | **exactly 4096 units of water, 0 vapour**; 1,935,086 s | the shrinking cube's law `3 rho L / (5 h dT)` = 2,001,300 s; -3.3 % from the half-unit floor, as computed | yes |
| the same from -10 C | 4098 water, -2 vapour (frost settled on the cold block); 1,961,656 s | | ledger exact |
| the same in a breeze (h_c 20) | 774,017 s | h for air is 2 to 25 W/m2/K free, 10 to 100 forced, radiation about 5 | |
| 1 m pond at 0 C under -10 C air, h = 14.5, no ground heat | the ice at each 10 cm within 0.06 % of Stefan's law with the air's resistance | `t = rho L / dT (d^2 / 2k + d / h)` | yes |
| the same: the root of time | thickness grows as t^0.61 between 0.3 m and 0.9 m (pure Stefan 0.5; the air's resistance k/h = 0.15 m adds the rest) | | yes |
| the same: stops conducting | heat up 87.8 W/m2 at 0.1 m, 21.1 at 0.9 m, 19.3 at 1 m | | yes |
| the same with the vapour exchange (air saturated over ice at -10 C) | solid 22.5 % sooner: the sublimation cools the warmer ice surface (about 0.35 mm/day off the top) | real lake ice sublimates tenths of a mm a day | not asserted |
| pot, 41 units (10 kg) at 15 C, 3 kW fire, 0.3 m2 losing heat, 0.05 m2 open | boils after 1,280 s, dry after 10,006 s, plateau and highest 100.000 C, **air cell +41 units** | a 2 to 3 kW kitchen ring brings 10 L to the boil in 15 to 25 min | yes |
| pot at 700 Pa, -60 C air | plateau 1.956 C (the curve's 1.956), +41 units | | yes |
| ice block at 608 Pa, -20 C, 2 kW fire beside it, 20 game days | **no water ever**; 90 units to vapour, block steady at -18 C (diffusive sublimation takes the fire's heat) | Mars's ice sublimates | yes |
| ice block in no air, 500 W of sun, 20 game days | **no water ever**; 248 units to vapour, block -84 C | | yes |
| the same in shadow | 233 units, block -88 C (it cools itself by sublimating) | why shadowed polar ice survives on airless bodies | |
| sea: a stored stretch of 32 columns, 20 m deep, air -0.5 to -30 C, F = 15.4, 25 real years at 1 h steps | laid at the formula: every column within 1 unit; from open water at 0 C: within 2 units (cap: 2 % or 4) | | yes |

The sea's formula against the stretch (12 of its columns):

| air C | formula, units | blocks | laid, after | open, after | the bake |
|---|---|---|---|---|---|
| -0.50 | 0 | 0 | 0 | 0 | open |
| -3.18 | 1251 | 0.31 | 1251 | 1252 | open |
| -5.86 | 2835 | 0.69 | 2834 | 2835 | open |
| -8.55 | 4418 | 1.08 | 4418 | 4418 | 1 block |
| -16.59 | 9168 | 2.24 | 9168 | 9168 | 1 block |
| -30.00 | 17085 | 4.17 | 17085 | 17083 | 1 block |

### Where heat is held (question 2)

100,000 stored sea columns (4 blocks deep, at their formula's rest) and 3,000 active cells: 2,000 pond columns away
from their rest, 500 ice blocks and 500 pots. Vapour is on, which is the dearer path (an `exp` a surface). A step is
one tick (`net::TICK_HZ` = 20; water itself steps at `phys::DT` = 1/60 s). Single thread, median of 200 steps, no
fat LTO, on a PC shared with other sessions:

| | memory | ms a step | ms a second at 20 Hz | at 1 Hz |
|---|---|---|---|---|
| (b) a temperature per stored column | 3.44 MB | 2.43 | 48.7 | 2.43 |
| (a) only near sources and phase boundaries | 0.24 MB | 0.079 | 1.58 | 0.079 |
| (a)'s check, once a second, of the 100,000 at rest against the formula | | 0.056 | | 0.056 |

Either way each stored node keeps its ice units: a u16 is 0.2 MB for 100,000.

**Recommendation: (a), stepped once a second.** The formula is the rest state, so a column at rest needs no
temperature: its air cell's temperature and the formula say what it holds. A column gets a temperature when it is
off its rest by more than a unit, or when an edit puts a heat source or ice near it. It loses the temperature again
once it is back at rest (as `liquid.rs` forgets water that matches generation). (b) costs 30 times the time and 14
times the memory, puts 100,000 temperatures in the save, and buys nothing at rest. Every rate in the tests is slow
against a second, so a 1 Hz heat step is enough. A node whose step would move it more than about 1 K (a nearly
empty pot, ice in vacuum at first) would need sub-steps. The pot's boil cap already keeps it on its plateau.

## Game time (question 4)

A game day is 1800 s, 48 times shorter than a real day. A game year (334 days) is 601,200 s: 7 real days, 52 times
shorter than a real year. The physics runs per second, so:

| process (real rates) | real time | game time |
|---|---|---|
| 10 L pot to the boil on 3 kW | 21 min | 0.7 game days |
| the pot dry | 2.8 h | 5.6 game days |
| ice a game night (900 s) at -10 C | 0.4 mm | (a real 12 h night: 18 mm) |
| ice a game winter (84 game days) at -10 C | 6 cm | (a real 90-day winter: 0.88 m) |
| 1 m pond frozen solid at -10 C | 114 days | 5,450 game days, 16.3 game years |
| 1 m3 ice block melted in +10 C still air | 22 days | 1,075 game days, 3.2 game years |
| stored sea from open water to 90 % of its sheet at -8 C | 1.2 years | 63 game years |

What a player does with their hands (a fire, a pot, a furnace, a block beside lava) plays at real rates. What
follows the calendar (a winter's lake ice, a cap melting in spring, a dug-open polar sea freezing back) gets a
fiftieth of the time it has in the real world. The untouched sea is not affected, because the formula carries it.
Only water a player touched lags. **A time scale is needed exactly where a climate drives a stored place.**

The honest options (the developer's call; none is built):
1. **Real rates everywhere, no scale.** Nothing is faked. The cost: touched polar water stays open for game decades,
   an ice block placed in a warm biome outlives three game years, a winter pond gets a 6 cm skin (enough to see,
   too thin to stand on). Fires, pots and furnaces feel right.
2. **The rows change, not the physics** (the tide spike's rule). The day and year are rows of the sky. A longer
   year makes seasons do more, but at real rates even a real year's winter gives 0.88 m, so every other system that
   reads the calendar would follow. An ice block's size is no row.
3. **One factor on the air's heat transfer, about 52** (the year's compression). A winter grows a real winter's ice
   and a block melts in about 20 game days. But the air then cools 52 times faster than any fire heats, and a pot
   left in the air would freeze in seconds. This is a fake by ADR 0183's own words; it is listed only because it is
   the usual game answer.
4. **Stored water at rest relaxes to the formula at the calendar's rate** while no player is near: the air
   grid's own 10 s steps, run 52 times faster for those columns only, and real rates near players. This is honest
   about the rest state and fast where nobody watches, but it is a time scale by another name, and a player who
   watches a pond sees it freeze slower than the one behind him.

## What surprised

- **The sea needs heat from below, or there is no formula at all.** With F = 0 the sea freezes to the bottom over
  years, never to a rest. F = 15.4 W/m2 makes the bake's sheet the rest state. The real Arctic gets 2 to 10 W/m2
  from the ocean, and summer melt does the rest of the real balance, which a yearly mean cannot see.
- **The bake's sheet holds at one temperature only.** The rest state is thicker where it is colder (2.2 blocks at
  -16.6 C, 4.2 at -30 C), and partial sheets lie between -1.1 C and -8 C where the bake shows open sea.
- **Sublimation matters on Terra too:** with the vapour on, a freezing pond sheds a fifth of its time, because the
  ice surface is warmer than the air and dries into it.
- **Mars's 0.006 atm sits just under the triple point:** at 608 Pa no liquid forms at all, while at 700 Pa a pot
  boils at 2 C. A body's pressure decides whether it can hold puddles.
- **Airless ice keeps itself cold.** In shadow a block sublimates down to -88 C and then nearly stops (233 units in
  20 days, most of it in the first hour). In 500 W of sun it settles at -84 C. No switch does this; the vapour
  pressure curve does.

## Not covered

- The engine was not driven. This is a standalone copy of the physics, with flat 1D columns and no flow while
  freezing: the water under ice does not move, and moving water does not carry heat.
- No lateral heat flow: a fire warms only the pot it is under, and "within a few blocks of a source" is not
  modelled.
- Snow, rain into the cell, clouds, and the air grid's own evaporation pass: the sea test runs with the vapour off and
  says so.
- Seasons: every test holds the air at one temperature. The formula's thickness is the rest under the year's mean,
  not a winter's peak.
- No real-world measurement was checked for the ice block. The time follows from textbook h values (5 to 25
  W/m2/K).
- Salt, the 4 C density maximum, ice floating by buoyancy (ice is the same units as water), pressure melting.
- Costs are of this copy (single thread, no LTO, a shared PC), not of a server tick.

## What the engine would need (question 5)

Read, not edited: `crates/engine/src/liquid.rs`, `data/blocks.toml` (`ice`, id 10: hardness, look, nothing
thermal), `crates/engine/src/weather.rs`, `crates/engine/src/bake/life.rs` (`t_sea_ice` = -8 on `cold`) and
`crates/engine/src/world/heights.rs` (`ice_sheet`: one block in the surface's layer where the biome is ICE, maps 7
and later).

- **A substance row** (`data/substances.toml` or a column of `fluids.toml` as liquids.md plans): water's triple
  point, `L_f`, `L_v`, `c` of each phase, `k_ice`, its solid block (`ice`) and its gas (the air grid's vapour), with
  tests.
- **`liquid::Node`:** an ice layer, in units (a u16 is enough for 16 blocks), floating on the gap's water. The gap's
  head under ice is the water's own. `counted()` and `generated_total()` take ice in. `ColWater` becomes (floor,
  ceil, water, ice), so `ToClient::Water` and the save change: a format change on the wire and on disk, its own
  decision entry.
- **A sparse heat store beside `Liquids`** (`FxMap<(Col, gap), Thermal { h: i64, vc: i64 }>`, about 48 B a cell),
  stepped once a second on the server. It is woken by edits (a flame, a furnace, lava, an ice block, poured water),
  by the 1 Hz rest check of stored columns against the formula, and by an air cell crossing 0 C. It forgets a cell at
  rest. It belongs in the server's tick beside `furnaces()` (`crates/server/src/tick.rs`). It is not on the wire;
  the save keeps it.
- **Ice blocks:** a placed or dug-out ice block (an edit) becomes a node. Its melt is `Liquids::pour` into the gap
  under it, and its last unit is an edit (ice to air). A gap's ice layer that reaches U units could become an ice
  block edit, or stay a layer (a rendering and taste question). Edits of ice must enter the water ledger, which
  today does not count blocks.
- **The air grid** (`bake::air::Air` through `weather.rs`): a way to add and take vapour units in a cell (the
  reverse of the runoff path, so its ledger stays exact), each cell's temperature (`Here.temp`, already there) and
  its pressure for the boiling point. A body without air (`air = false`) is 0 Pa, so its ice sublimates.
- **Heat sources** as watts: furnaces (`crates/server/src/furnaces.rs`), flames (ADR 0117), lava and the deep rock.
- **The bake's sheet** from the formula: its thickness by the year's `cold` temperature at the cell. The baked map
  keeps the biome, not the climate (`sky.rs`'s own shortcut on `BIOME_CLIMATE`), so this is a bake format change.
  Until then, F = 15.4 W/m2 makes today's one-block sheet right at -8 C only.
- **Client:** draw an ice layer thinner than a block on a gap, and show temperature in F3.

**Steps, each a test seen failing first:**
1. The substance row and the sparse heat store, with ice blocks only. Test: a placed ice block in +10 C air becomes
   exactly U units in the gap under it, and the ledger closes.
2. The ice layer in the node, with the wire and save format (its decision). Test: a stored pond freezes from the top
   by Stefan's law and thaws back.
3. Vapour into the air grid's cells, and the pressure. Test: a pot over a furnace boils off into its cell; on Mars
   and Luna, ice sublimates.
4. The formula: the bake's sheet thickness from the climate (a bake format change), and the rest check that wakes
   and forgets columns. Test: a stored polar stretch does not move.
5. The time scale, once the developer has chosen.

## How to run

    cd prototypes/water-phases
    cargo test --release
    cargo run --release
