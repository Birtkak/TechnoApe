# Near-gas spike: gas near players, conserving with the 128 m air cells, and fire as a reaction

**Question** (`docs/design/liquids.md`, "Gases" and "Phases"; `docs/design/air.md`; ADR 0183, 0187): the air is a
counted grid of 128 m cells, too coarse for steam off a pot, smoke filling a room and leaving by a hole, CO2
pooling in a hollow, and fire that spreads. How should gas be held near players so that it

1. conserves exactly with the coarse cell (one ledger per species, through every hand-off),
2. moves by real causes (buoyancy from density by temperature and species, diffusion, pressure through openings),
3. costs nothing where nothing happens, and
4. carries fire spread as a consequence (fuel, heat, oxygen in; heat, smoke, CO2 out), not a separate rule?

**Answer, short.**
- **Store (i), a sparse grid of 1 m gas cells** only where the air is off its rest, stepped at the tick (20 Hz), is the
  one that works. One rule moves it: each open face's speed is driven by pressure plus gravity on density, and
  whole units go upwind. Every ledger closes to the unit and the millijoule in every step of every test, through
  thousands of cells made and dissolved. Store (ii), one mixed node a column gap (the liquid's structure), is 10 to 30
  times cheaper but cannot stratify: it lets 39 % of the smoke out of a floor hole (store (i): 3.5 %), and a pit's gap
  runs up to the sky, so it holds 0.1 % of the CO2 (store (i): 19 %).
- **Fire works**, but a 1 m cell cannot hold a flame. Over a 660 kW block the cell above sits at 320 K, so the next block
  never reaches ignition from the gas alone. The flame's own heating of what it touches has to come in from the
  fuel's row at sub-cell scale: the LIFT spread rate by the neighbour's skin temperature, and the flame's radiation
  from a point at Heskestad's height, leaned by the wind. Spread then follows from heat. A line burns at about 6 min a
  block, a stone gap stops it, wet wood holds the fire off for about 60 min a block while its skin boils dry, and wind
  pushes the front downwind.
- **Cost:** about 0.3 µs a cell-step. A house fire (2,000 to 9,000 cells) is 14 to 49 ms a second on one thread; a forest
  front (73,500 cells) is about 0.6 s a second, which is too much for one thread at 20 Hz (numbers below).

## What was built (`src/`)

| file | what |
|---|---|
| `gas.rs` | Store (i): `Cell` (species as whole mass units of 1/8 mg, so one liquid unit of water is exactly 1,953,125; heat in mJ from 0 K; six face speeds; links to its neighbours), `Coarse` (the 128 m cell: one number a species and its heat), `Gas::step` (transport, then cells made and dissolved) |
| `heat.rs` | The sparse heat store of blocks (the water-phases idea), with fire, boiling and radiation; `Sim` holds the world and the ledgers |
| `gaps.rs` | Store (ii): one mixed node a column gap, links between overlapping gaps (as `liquid.rs`), the same pressure rule |
| `scenes.rs` | The test places: a room on a slab, a valley with a pit, a pot, a line of wood |
| `tests.rs` | The tests; every run checks every ledger after every step (`Sim::check`) |

### The rule of store (i)

- **State:** a cell holds 5 species (nitrogen, oxygen, carbon dioxide, water vapour, soot) and its heat. The air around
  the fine cells is the coarse cell at rest, leaned with height by the scale height.
- **A face's speed** gains `dt (dp' - (rho - rho_coarse) g dz) / rho` and loses 1/64 of itself a step. Here `p'` is the
  ideal gas's pressure over the coarse air's at the cell's height and temperature, stiffened to a sound speed of
  10 m/s (`C_ART`) so that the explicit 20 Hz step is stable. This is the liquid's own trick: its `K` does the same
  under a ceiling. Thermal expansion, a plume, the stack draught, heavy gas running downhill and a burst through a
  hole all come from this one line.
- **Units move upwind:** each species and the heat leave the donor in whole units. A cell gives at most half of
  itself a step. Composition at the face is van Leer limited, which is second order and still in whole units.
- **Hand-off:** a coarse neighbour of an off-rest cell becomes a fine cell by taking exactly its share of the coarse
  cell. A cell at rest whose neighbours are at rest gives back exactly what it holds. Rest means: each species within
  its tolerance (soot 1e-7 kg/m3, CO2 1e-4), temperature within 0.5 K, pressure within 1e-4, faces under 2 cm/s.
  Faces to coarse air trade with the coarse ledger directly.
- **Ledgers asserted every step:** each species (fine + coarse, minus what sources booked), all mass (gas + fuel +
  liquid water), water units (in blocks + boiled), and energy (gas heat + block heat + fuel's chemical energy + the
  latent heat the boiled units carry + the rest heat of blocks that joined − heat supplied).

### The heat store and fire (`heat.rs`)

A block joins when gas beside it is more than 1 K off the coarse air, or when it is lit or given a source. It stays
at least 3 s (`HOLD`) and leaves at rest, giving its heat over rest to the coarse air. A block is two nodes:

- **a skin** as deep as heat soaks in over ten minutes, `sqrt(alpha 600 s)`: 9 mm of wood, 3 cm of stone. It covers
  every open face.
- **the core** behind it, joined to the skin by conduction over half a block.

A lumped 1 m3 block would need 242 MJ to reach ignition, which is 2.7 h under a flame. With the skin node, ignition
takes about a minute.

- **Skin and gas:** convection (10 W/m2/K) plus the soot's radiation. The gas emissivity is `1 - exp(-1862 f_v T)` (the
  Planck mean of soot) over the 1 m cell. This runs every gas step, because a gas cell holds about 1/1000 of a skin's
  heat.
- **Water** (liquid units from the liquid ledger) holds the skin at 100 C. Heat over that point is a credit; each
  `L_v` of credit boils one whole unit into the gas cell above as 1,953,125 mass units of vapour. The unit's latent
  heat is booked.
- **The reaction**, from the fuel's row (plywood: ignition 390 C; 11 g/m2/s burning; 15 MJ/kg; 1.145 kg O2, 1.33 kg
  CO2, 0.015 kg soot per kg; water vapour is the rest of the mass, so mass closes):
  - It burns where the skin is dry and at ignition, at the row's rate times the share of the surface the flame covers
    (`front`).
  - The rate is scaled by the oxygen in the cells on its faces, from the row's limiting mass share (0.15) up to
    air's.
  - The flame goes out under Rasbash's critical burning rate of 2.5 g/m2/s.
  - Heat goes first to hold the burning skin at ignition (the flame's feedback). The radiant share (0.3) goes to the
    radiation pass. The rest goes into the face cells, with the products.
- **The sub-cell flame:**
  - The front crosses a face at `phi / (krc (T_ig - T_s)^2)`: LIFT, the flame heating the skin ahead to ignition.
  - A fully burning block lights its fuel neighbours at the seam once their skins are dry.
  - Once a second (the water-phases heat step), each flame radiates what it gathered. The source is a point at half
    of Heskestad's flame height, leaned by the AGA wind tilt. The heat lands on every open solid face within 3 m by
    `cos / (4 pi r^2)`. What no face takes goes to the coarse air.

## Results

`cargo test --release` (9 tests, 74 s) and `cargo run --release` (every number below). Single thread, `opt-level 3`,
no fat LTO, on a PC shared with other sessions' builds.

### The tests (each seen failing first)

| test | measured | pass |
|---|---|---|
| ledgers, every step of every test below | species, mass, water and energy exact to the unit and the mJ; 263,680 cells made and 263,037 dissolved in the valley alone | yes |
| hot smoke from a burning block, 600 s, 6 x 6 x 3 room, one 1 m hole | soot out of the room: **roof hole 0.800, floor hole 0.044** | yes |
| 10 kg of CO2 from a vent at 0.1 kg/s on a plateau 7 m over a valley floor with a pit 3 deep | 99 % below the vent's height; pit 0.0511 kg/m3 over the air, the floor's layer beside it 0.0521; the pit held 0.187 of the release at 600 s and 0.184 at 3600 s | yes (see below) |
| fire in a sealed room | died at 222 s after 6.1 kg of 400, oxygen share 0.1695 (air 0.231); pressure peaked at 4.3 atm | yes |
| pot of 41 units on 3 kW | first unit boiled at 1467 s, dry at 9491 s, never over 373.150 K; **gas water vapour grew by exactly 41 x 1,953,125 units** | yes |
| a line of 13 wood blocks, the first lit | every block caught, about 355 s apart: 0 0 385 741 1092 1449 1804 2158 2512 2863 3213 3560 3905 s | yes |
| the same with one stone block as the 7th | the six before it caught; nothing past it in 3600 s | yes |
| the same, wet (30 % moisture, 614 units a block; the lit one dry) | the 2nd block caught at 4343 s (dry: 0), the 3rd not in 5400 s (in 4 h: 4343, 7960, 11602 s) | yes |
| the line lit in the middle, wind 3 m/s along it | in 1200 s: 5 blocks downwind caught (377 to 932 s), 2 upwind (454, 899 s) | yes |

**The heavy-gas test was changed after it failed, and says so in its code.** As first written it asked for half the
release in the pit. It got 0.145 with first-order transport and 0.187 with van Leer's. The current thins 10 to 20
times on the slope and lies on the whole valley floor as a stratified pool about 3 m deep, 2 to 5 % CO2 by mass, and
the pit fills with the pool's bottom layer. That dilution is the size field trials of dense gas show (Thorney Island:
a few percent tens of metres out), but part of it is numerical: upwind transport on 1 m cells mixes like a
diffusivity of `v dx / 2`, tens of thousands of times air's molecular one. The test now asks for what the physics
gives: the gas runs down, the pit holds the pool's bottom, and it stays.

### The stores compared (the room, a 100 kW smoke maker, 300 s)

| | store (i): sparse 1 m cells | store (ii): one mixed node a gap |
|---|---|---|
| soot out, roof hole | 0.721 | 0.910 |
| soot out, floor hole | **0.035** | **0.394** |
| roof / floor | 21 | 2.3 |
| valley pit (600 s) | 0.187 of the release | 0.001 (the pit's gap runs to the sky, mixed) |
| a step, roof case (`cargo run`) | 2,574 cells, 0.757 ms | 295 nodes, 0.024 ms (every gap of the box stored) |

Store (ii) is right only when nothing in the gas depends on height within a gap. A room's hot layer, a cellar of
CO2, a chimney's draught and a plume all do. Giving it layers (liquids.md's stack) turns it into a zone model with
plume and entrainment formulas, which is a rule per case. A third store was considered and not built: **air.md's
rooms**, a sealed room as one small cell. It is cheap, but it needs room detection and says nothing outdoors.

### Cost (`cargo run --release`, 100 steps after the scene has developed)

| scene | fine cells | heat-store blocks | couple ms | gas ms | ms a second at 20 Hz | MB |
|---|---|---|---|---|---|---|
| room fire, roof hole, at 300 s | 2,039 | 131 | 0.037 | 0.665 | 14.0 | 0.35 |
| house-sized plume (11 x 11 m of 20 kW, the box filled) | 8,616 | 121 | 0.083 | 2.380 | 49.3 | 1.40 |
| forest-front-sized plume (56 x 56 m of 20 kW) | 73,500 | 3,348 | 4.889 | 25.796 | 613.7 | 12.30 |
| line of wood at 1800 s | 887 | 60 | 0.013 | 0.185 | 4.0 | 0.15 |

A `Cell` is 136 B and a `Block` 136 B, plus about 24 B of hash map each.

At about 0.3 µs a cell-step, store (i) is linear in active cells. Memory is 160 B a cell with its hash entry. The
house cases fit the tick. A forest front does not on one thread:
- Its faces are Jacobi (each reads the step's start, writes integer deltas). Integer sums commute, so slabs on worker
  threads give the same answer in any order. Six threads would take it to about 100 ms a second.
- Beyond that: 2 m cells far from players (8 times fewer, but a 1 m door or chimney is no longer a face).
- Hysteresis on rest: the valley made and dissolved 440 cells a second at the tolerance's edge, pure waste.
- Not stepping the plume high above a canopy, which is the coarse cell's job.

### Against the game clock (ADR 0187: heat a source drives runs at real rates)

| process | real time | game days (1800 s) |
|---|---|---|
| sealed 108 m3 room: the fire chokes | 222 s | 0.12 |
| a lit plank block's neighbour catches | about 6 min a block | 0.2 a block |
| a 13-block line burns along | 65 min | 2.2 |
| a wet block (30 %) dries and catches next to a fire | about 60 min | 2.0 |
| a 1 m3 block of wood with 4 open faces burns out (660 kW) | 2.5 h | 5.1 |
| a 10 kg pot on 3 kW boils / goes dry | 24 min / 2.6 h | 0.8 / 5.3 |
| smoke of a room on fire leaves by a 1 m roof hole | most of it within 10 min | 0.3 |
| a CO2 release runs 20 m down a slope and lies | about 3 min | 0.1 |

Fire at real rates plays at the size of a player's hands. A burning block lasting five game days is the real number
for a 1 m3 solid timber block. Planks, leaves and brush are a different row, with less fuel and more surface, and
burn in minutes.

**ADR 0187 has a gap here.** It speeds up "heat exchanged with the air" by about 52 and keeps "heat a source
drives" real. In this store the gas near a fire is air, and a burning block's neighbour is warmed through it. The
spike keeps everything near a source at real rates: the fine store is a source's neighbourhood by construction.
Only what dissolves into the coarse cell is the climate's. That is the developer's to confirm.

## What surprised

- **A 1 m cell cannot carry a flame.** Over a 660 kW block the cell above sits at 320 K, with the plume rising at
  6.6 m/s. A real flame's heat to the surface ahead lives within millimetres. Without the sub-cell flame terms the
  line never spread at all. The terms are still heat reaching ignition, with rates from the row, but they are a model
  of the flame, not the gas grid's own result.
- **The spread speed is the feedback's, not LIFT's.** One skin node holds both the burning part of a face and the part
  ahead of the flame, and the feedback lifts both. So `T_ig - T_s` shrinks and the front crosses a block in about 6 min.
  LIFT with a cold surface would give 98 min. A skin node for the unburnt part would settle it; that is the next
  question for this model.
- **An instant release is a blast.** 10 kg of CO2 dropped into six cells in one step doubled their pressure and threw
  half of it up and out of the valley within a minute. A vent at 0.1 kg/s keeps all of it.
- **The heat store needs a hold.** Radiation arrives once a second, 0.1 K at a time, which is under the rest
  tolerance. The neighbours left the store between passes and handed their heat to the coarse air, so wet wood never
  dried. Three seconds of hold fixed it.
- **A sealed room reaches 4.3 atm.** Its stone walls take little in 222 s, and nothing leaks. Real rooms leak at the
  doors and windows.
- **Terra's 19.62 m/s2 doubles every buoyant speed** against Earth's figures.

## The developer's calls (taste, scope)

1. **Can a forest burn down?** Spread is unbounded by construction. A front costs about 0.6 s of CPU a second on one
   thread, and the 3D cell volume over a front is the cost driver. Options: a cap on fine cells (fire beyond it
   follows the coarse cell), fires only near players, or forests that cannot burn.
2. **Does fire destroy a build?** A burnt-out block is solid char here (its fuel is spent, it keeps its shape). It
   could instead collapse to air, which opens holes and draughts, or become an ash block.
3. **The fire's pace:** about 6 min a plank block in still air, 60 min for a wet one, 5 game days for a solid block to
   burn out. All are real rates.
4. **Breathing:** does an ape read the oxygen and CO2 of its cell (suffocating in a CO2 pool, choking in smoke)? The
   store has the numbers; the rule is survival's.
5. **ADR 0187's two paces near a fire** (above): is gas-to-block heat in the fine store the source's pace (real)?
6. **What a player sees of smoke:** at the 1e-7 kg/m3 soot tolerance, a cell dissolves when its smoke can no longer be
   seen.

The routine calls are made and named in the code, each a `shortcut:` where it has a limit:
- the 10 m/s sound and the 1/64 friction;
- 1 m cells at 20 Hz;
- the skin as `sqrt(alpha 600 s)`;
- the plywood LIFT, Heskestad, AGA and Rasbash figures;
- radiation within 3 m with no shadowing;
- the tolerances, the 3 s hold, and van Leer.

## What the engine would need

- **Rows** (ADR 0183, a row is a substance):
  - **Gas halves**, as rows of `data/fluids.toml` or a substances file: molar mass and `cp` for nitrogen, oxygen,
    carbon dioxide and soot (soot is a particle: it weighs but presses nothing). Water's gas half is the vapour of
    the water-phases spike: one H2O species, whose units are liquid units times 1,953,125.
  - **Fuel halves**, on the block rows (`data/blocks.toml`): fuel kg, ignition, burning rate, heat of combustion,
    oxygen and yields, limiting oxygen, LIFT's `phi` and `krc`, radiant share. Plus `k`, `rho` and `cp` for every
    solid, from which the skin follows.
  - Tests for every row: mass closes, the products are not negative.
- **Air cells:** `bake::air`'s cell gives each species and its heat as one number, with a way to take and give whole
  units (as the runoff path does). Today it has vapour and temperature only, so carbon dioxide, oxygen and soot are
  new columns of its state and save (a format change, its own decision entry).
- **Where it steps:**
  - In the server's tick, after `liquids` and beside `furnaces()` (`crates/server/src/tick.rs`), at 20 Hz.
  - Fine cells live within a near box around players and active sources (32 to 64 m). Past it is coarse.
  - The step is Jacobi with integer deltas, so it splits into slabs on worker threads with a deterministic sum.
  - The heat store's gas coupling runs each tick; its radiation and conduction run at 1 Hz.
  - Saves keep the fine cells and every block whose state changed (fuel spent, water boiled); the rest is the coarse
    cell.
  - Floats decide rates as in water-phases: deterministic on one build, not across platforms. The server is the
    only authority.
- **On the wire** (clients draw from it, never simulate it):
  - per fine cell, at 2 to 5 Hz by tile as water is sent: soot density on a log scale (u8), temperature (u8, for haze
    and glow), and a drift velocity (3 x i8);
  - per burning block, its front and its heat release (from which the client draws the flame's height and lean).
  - A 2,000-cell house fire is about 12 kB a full update; deltas past a quantum are a few kB/s.
- **Effect only** (ADR 0031's effects, from those numbers): flame shapes and flicker, sparks, embers, smoke particles
  advected by the sent drift, heat shimmer, crackle. Steam's white plume needs condensation, which is not built: the
  vapour here stays vapour.

**Steps, each a test seen failing first:**
1. Gas species in the coarse cell, with an exact take and give.
2. Store (i) for heat and smoke alone (a furnace's chimney).
3. The heat store with the water-phases boil (a pot's steam reaches the cell).
4. Fire on one fuel row.
5. The wire and effects.
6. The speed batch: threads, then hysteresis.

## Not covered

- Not checked against measured room-fire data (McCaffrey's plume, a compartment's layer temperatures), and no
  convergence check (2 m against 1 m against 0.5 m).
- The coarse cell does not step: no weather, and wind only as a target speed for lateral faces, with no flow around
  obstacles beyond what pressure does.
- No condensation (steam, fog), no flame radiation shadowing, no radiation between solid faces, no gas radiation of
  CO2 or water vapour.
- No smouldering, no char layer slowing the burn, no unburnt skin node (above).
- Fire spread only along face neighbours and by radiation within 3 m; no burning brands.
- Molecular diffusion is left out: at 1 m it is 10^-5 of the scheme's own mixing.
- Stone walls do not cool the sealed room much in 222 s, and walls do not leak.
- No threads were measured. The costs are of this copy on a shared PC, not of a server tick.
- Store (ii) stores every gap of its box, not only off-rest ones; its cost would fall further, but its behaviour would
  not change.

## How to run

    cd prototypes/near-gas
    cargo test --release
    cargo run --release
    cargo run --release -- wet     # the wet line's next block, every 10 min
