# Deep world

The layered deep grid, the mantle, the liquid metal sea and a body's interior derived from its row: a planet that can be
dug to and through its centre. What holds today. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came
from; INDEX.md lists every number. The design is `docs/design/deep-world.md` and `docs/design/planet-model.md`.

## The developer's calls

- **Dig through the whole planet** (2026-11-18): the grid reaches the centre. [0152]
- **The layered grid over shells with seams**: the developer saw the pictures of seven halving shells and found them
  ugly; a plain cube grid for the whole deep (stepped floors at full gravity) was not wanted either. [0152]
- **The large core** (a large liquid outer core under the mantle). [0152]
- **Gravity grows toward the mass**: it rises through the mantle and falls evenly to zero at the centre (2026-11-18:
  "it grows going down toward the mass"). [0152, 0154]
- **Liquid metal never freezes into blocks**: in the outer core it is liquid or gone, so natural rock never shows a seam.
  [0152]
- **New worlds only**: a save from before keeps its 192 m floor. [0152]
- **Everything gets a deep grid** (2026-11-23): every body with blocks, not Terra alone; a core is liquid when the body
  is big and hot enough (its row's `heat`), and nothing is faked. [0184]
- **Build one feature a step**: the mantle, then the metal sea and its hidden shells, then the inner core's cube grid,
  then field, pressure and caves. [0152]

## The zones of a planet

For Terra (radius R, 4245 m on the standard planet). [0152]

| Zone | Radius | Grid |
|---|---|---|
| Crust and mantle | R down to R/sqrt 2 (the top 1243 m) | today's grid carried on down; blocks shrink smoothly from 1 m to 0.71 m |
| Liquid outer core | R/sqrt 2 down to 0.19 R (807 m) | liquid metal drawn smooth; under it halving shells carry its columns and what a player builds there once drained |
| Solid inner core | the last 0.19 R | a plain grid of 1 m cubes; no shells, nothing special at the centre |

- Seams exist only in what a player builds in the drained core (a wall's width changes by a fifth). [0152]
- The inner core's surface is stepped away from its six axes (a 1.35 m sawtooth at 45 degrees); how that walks at a
  third of gravity is unknown until step 3 is played; shells for the inner core are the way back. Where the cube ball
  meets the liquid's columns the join is ragged and shows only once the core is drained. [0152]
- Luna and the small planets: from generation 9 every body with blocks big enough for the deep crust has the grid down
  to its core's top, over a metal sea where its core holds liquid (Luna, and Mars by 0185) and a closed floor where it is solid
  (Callisto); their own deep zones under that come with step 3. [0152, 0154, 0162, 0184 "Every body gets the
  deep grid"]
- **Built so far:** step 1 (the mantle) and step 2 (the metal sea, without the hidden shells). The inner core's cube grid,
  the hidden shells under layer 0, pressure making metal well up, core mining and its ledger are not built. [0152,
  0154, 0156]
- (checked in code and ADRs: 0152 puts the solid inner core at 0.19 R; 0162 derives it as `(1 - heat)` of the core's
  radius, Terra's `heat` 0.65 giving 0.35 x R/sqrt 2 = 0.25 R. Nothing reads either yet; reconcile when step 3 is
  built.)

## The mantle (step 1)

A new world's grid goes down to R/sqrt 2; the saves made before keep their floor and every layer index they hold. [0154]

- **Layer 0 moves down, every layer above gains the mantle's count in index** (as the 192 m crust did, ADR 0063), per
  world; every reader already asks `0..layers`. `Body::with_mantle` (before the map) moves `r0` down by
  `mantle_layers(n)` layers and adds them to `layers`; `Body::mantle` holds the count. A planet for which `deep_layers`
  is 0 (the 500 m class) gets 0. `mantle=0` on the game, the server and `report=1` makes one without. [0154]
- **Per world, recorded.** The grid choice is the world's generation: `Body::gen` (`world::GENERATION`, 0160 says
  gen >= 1 puts the grid down through the mantle, gen 0 is the worlds before it); the save's last field is `gen`
  (version 20 had `mantle: bool` in that byte; a 20 reads as generation 1 with the mantle and 0 without, a 19 or
  older as 0); `Welcome` carries `gen` and the client and bot build the same grid. A recording from a seed says it by
  `Ev::Mantle` right after its Start; one without (from before) replays on the old floor, one from a save as the save
  says. An old world written again stays on its 192 m floor. [0154, amended 0160, 0163] (also multiplayer-saves-and-data.md)
- **Whole steps of 256 layers** (`MANTLE_STEP`): the mantle is the multiple of 256 layers nearest R/sqrt 2, so the cells
  of every level and the chunks of levels 0 to 3 stand on the same layers above the crust's floor as on an old world
  and the surface is meshed and drawn as before. Chunks of level 4 and up begin elsewhere against the surface (the
  orbit view differs from its baseline by 0.20 % where the old grid gives 0.03 %); aligning them would need a multiple
  of 2048, past R/sqrt 2 and out of one root band. [0154]
- **Sizes:** the 4 km planet 1280 mantle layers under the 595 it had (1875 in all), layer 0 at 2997.8 m, 4.2 m under
  R/sqrt 2, 1248 m under the radius, the lowest block 0.706 m. The 8 km planet 2816 under 601 (3417), 2534 m deep, 47 m
  under R/sqrt 2. The 1536 test planet 256, 378 m deep (67 m under R/sqrt 2, the lowest block 0.64 m). Both large
  planets fit one root band of the streamer (2048 and 4096 layers). Blocks at the mantle's bottom are 0.71 m: 2.8 to
  the cubic metre, so digging is slower there. [0152, 0154]
- **The GPU counts from the old floor:** the client gives the GPU a body's `r0` at layer `mantle` and every chunk's
  base `mantle` layers lower (`frame.rs`), so every shader's look hash and the light's random seeds read the numbers
  an old world gives them; no shader changed. The cell looks of effects (cracks, gliding blocks, piles, break cubes)
  count the same way (`fx::gpu_cell`); the two CPU hashes of a layer that pick a roll take the mantle off too
  (`edit::held`, `crack::drop`), as the ore halo does. [0154]
- **Mantle rock:** `data/rocks.toml` names `mantle = "peridotite"`. `Body::rock` gives it under R - 192 m in every
  province on a body with a mantle and today's layers above. Peridotite (block 58, rubble 191) is as hard as quartzite
  (10, grade 1), the hardest rock of the crust: a steam-age pick digs it. Its texture is a draft. [0154] (also
  rock-minerals-and-ore.md)
- **Nothing new is placed in the mantle by this step:** caves, ores, water, piles and plants keep their depths under
  the ground (their placement works in radii); an ore body and its halo are cut at the old floor (`OreBody::span`), so
  the ore budget holds. Later generations place caves by rock (0159, 0161) and form rocks by pressure (0158). [0154]
- **Heat** carries on by the province's rule (air plus `heat` C a metre): at the floor, about 1245 m down, the shield
  is about 150 C over its air, the basin 186, the folded belt 224, the ocean floor 249, the volcanic arc and the rift
  435; under the spawn the report says 201.0 C. [0154]
- **What scales with the layer count:** `Solid::gaps` looks up every chunk band of a column for edits (59 bands where
  there were 19); `Streamer::new` makes roots per 2048-layer band; the horizon occluder is the sphere of `r0`, now
  R/sqrt 2, so less is culled behind the planet. Its cost: `docs/perf-debt.md`, row "The mantle". [0154]
- **Locks:** seed 1337 report on the 4 km planet: every province, cave and mineral count equals main's (copper 339 035
  blocks, tin 16 391, iron 808 378, coal 888 247, aluminium 190 359, lead 34 957, gold 4 572, uranium 997); `made_hash`
  differs (caves' layer coordinates gain 1274; with `mantle=0` it is 2a79455bb6161781 as before). [0154]
- **Open for the developer** (plain defaults taken): peridotite's hardness and grade (`iron-nickel`'s 20 and grade 3
  would wall the mantle off until a later age); its look; the line at exactly R - 192 m (no fold, no mixing with the
  crust's last layer); the jump at depth is about 12 % lower at the floor (1.135 g). [0154]
- Below the floor nothing is dug: a dig under layer 0 is refused. [0154, 0156]

### Closed floor [0179, 2026-11-23]
Layer 0 of a body with no metal sea under it (`Body::core` none: Luna, Mars, the icy moon, a world without the mantle)
is a closed floor: no dig or edit takes it, so the void under the grid is never open or drawn (F9 2026-11-21 "moon
is not able to be mined": a shaft to layer 0 on Callisto showed the stars through its bottom). One test,
`World::closed_floor`, read by `edit::changes` (the server's edit and the client's prediction: a floor cell is left
out, as a cell a player stands in is), `edit::dig_time_with` (no hold on it) and `World::set_block` (a save's edit
there from before is dropped on load, so its hole closes). Terra's layer 0 over her metal sea digs as before (0156).
Alternatives rejected: a grid under layer 0 for small bodies (their deep zones wait for their step); refusing a whole
shape that touches the floor (a box dug down to it would fail whole).
Open: each small body's own deep zones (deep-world "Luna and the small planets").

### Every body gets the deep grid [0184, 2026-11-23]
The developer's call (2026-11-23): "everything gets a deep grid"; a body's core is liquid when its row's `heat` leaves
it some, and nothing is faked. From generation 9 (`world::DEEP_BODIES`, now `GENERATION`) every body with blocks (not
gas) goes on `Body::with_mantle` as Terra does: its grid carried down through the crust's 192 m and its mantle to the
whole `MANTLE_STEP` nearest its core's top (`Interior::core_share`, the row's `iron`). The size rule is Terra's, unchanged:
a body big enough for the 192 m deep crust (`deep_layers`: radius at least 4 x 192 m) gets the grid, a smaller one keeps
its old floor; on the 4 km and 8 km planets that is every rocky and icy body (Luna 1150 m, Callisto 1061 m, Mars 2300 m
on the 4 km one), on the 1536 and 768 test planets only the bodies over 768 m. Under layer 0: a core with liquid
(`interior.liquid > 0`: Luna's thin shell at heat 0.05) is the metal sea, as Terra's (Mars too since 0185 made her core liquid); an all-solid core (then Mars and
Callisto, heat 0) ends the grid on a closed floor (0179): on a rocky body the core's metal, `iron-nickel`, on an icy one
(Callisto, whose row's core is rock of 3000 kg/m3) stone (`Body::rock`: layer 0 of a deep body without a metal sea; one
compare inside the mantle's branch; `Body::icy` from the row's `kind`). Every layer of the old grid keeps its
radius at its index plus the mantle's; caves, tubes, ores and the bakes place by radius, so no bake's bytes change and
no bake version moved. Worlds of generation 8 and older are built as before, to the bit.
Alternatives rejected: a deep crust that scales with the radius for small bodies (only the test planets have any,
and Terra on the 768 planet has no deep grid either: one rule for all); a grid under layer 0 of a solid core (step 3's
inner core).
Replaces: 0179's closed floor on Luna, Mars and the icy moon in new worlds (it holds there only over a solid core, on a
body too small for the deep grid, and in older worlds); "Luna has no liquid core and no mantle" in "Luna and the small
planets".
The mantle's rock comes from the one pressure rule (`Body::rock`, 0158): Mars (8.4 m/s2) turns to peridotite under its
crust; Luna (3.6 m/s2) and Callisto never reach peridotite's 10 MPa, so their crust's last rock (Luna's), or the
biome's (stone, ice), goes on down to the floor (seen on the noise land, not on their maps).
Locks: seed 1337 `report=1` on the 4 km planet at generation 8 and 9 is the same in every count, depth and distance (Luna's
provinces, baked tubes, caves and minerals; Mars's and Callisto's kinds, heights and marked tubes); only Luna's
`made_hash` differs (c817d3d375b5ad4b -> 1af7d86fe60e5a63: her caves' and ores' layer coordinates gain her mantle).
Open: whether the small bodies' mantles should be their own rock (a composition, not a pressure, rule); a solid core's own grid (step 3); the load
standing on Luna doubles (0.9 -> 1.8 s, the streamer walks her deep column; `docs/perf-debt.md`), RAM and frame flat.

## The metal sea (step 2)

The liquid outer core under the mantle's floor. [0156]

- **One level a body:** `Body.core: Option<f64>`, a layer coordinate like `sea`; `with_mantle` sets it to 0, the
  bottom of layer 0, on a body that got mantle layers (and, by 0162, a liquid core). Luna, a planet too small for a
  mantle, `mantle=0` and older saves have none. No save field, no wire field, no pumping, no flow, no grid under
  layer 0. Generated, stored nowhere, as the open sea's `SEA` account.
- **Calm:** it does not rise into a shaft; a breached floor shows a still surface at its level. Pressure making it well
  up is a later step.
- **Things float on it:** rock (3.3) and apes are far lighter than liquid iron (7); physics stays, the cell under
  layer 0 still holds a player or rubble up, digging and placing under layer 0 stays refused. No swimming in metal.
- **It burns:** a survival player whose feet are in a layer-0 cell that is not solid, on a body with `core`, loses a
  point of health every `vitals::BURN` = 6 steps (0.1 s), to the death "<name> burned in the liquid metal". The count
  lives on the server's client (`burn`), not saved. Creative is unhurt.
- **It is drawn as a glowing surface:** `metal_quads` (`world/water.rs`) gives a level-0 chunk at layer 0 one quad of
  kind 6 per column whose layer-0 cell is not solid (generated or edited), with the trailing water quads. `vs_sea` lays
  it as a flat river quad 1/8 of the cell over its bottom and marks it (`side` bit 2); `fs_water` gives it
  `metal_sea`: opaque, unlit, emissive `METAL_GLOW` (linear 4.0, 2.6, 0.9 before the shoulder) with two octaves of value
  noise drifting slowly as darker convection cells (down to 0.3 of the glow). No tide, waves, transparency or
  refraction; no GPU layout change.
- **It lights the hole:** `World::light_of`: a layer-0 cell open to the metal is a light of `CORE_LIGHT` ([1.0, 0.66,
  0.26], a lamp's strength) in `set_block` and `refresh_defs`, so a dig, an edit sent to a client and a loaded save all
  light it.
- **`floor=k`** on a stand run stands on the bottom of layer k.
- **Shortcuts:** quads only in level-0 chunks, one a column, unmerged; a large opened floor wants them merged as the
  sea's are. **Not done:** pressure, heat from hot air or rock, sound, particles, swimming in metal, the 8 km planet's
  tuning, a drained core with the metal taken out and its ledger, the field (`deep-world.md` 3 of the design). [0156]

## A body's interior from its row

A body's row states what it is made of and its interior is derived from the row, not Terra's numbers spread over the
engine as constants. Nothing observable changed: Terra and Luna come out as before; generation stays. [0162]

- **The row's makeup** (`bodies::Makeup`, checked in `bodies.rs`, every message names the body):

| Input | Unit | Terra | Luna | Check |
|---|---|---|---|---|
| `kind` | rocky, icy, gas | rocky | rocky | icy and gas refused: "not built yet" |
| `iron` | the metal core's radius over the body's | 1/sqrt 2 (stated) | 1/sqrt 2 (Terra's) | over 0, under 1 |
| `heat` | 0 dead (core all solid) .. 1 young (all liquid) | 0.65 | 0.05 | 0..1 |
| `density` | crust, mantle, core, kg/m3 | 2700, 3300, 7920 | the same (Terra's) | 500..30000, none lighter than the one over it |
| `weight_g` | m/s2, optional: the pull rock is weighed with, where not the body's own | none | 19.62 | over 0 |

- `iron` is **stated** as a radius share, not derived: 1/sqrt 2 is the grid's choice (ADR 0152), parsed to
  `FRAC_1_SQRT_2` to the bit. The core's density is stated as 7920 = 2.4 x 3300, whose quotient is the literal 2.4 to
  the bit. The crust's and the mantle's densities moved out of `rocks.toml` into the rows; `rocks.toml` keeps the
  mantle's rock boundaries and the provinces.
- **Derived** (`crates/engine/src/interior.rs`, `Interior`, computed once per body when the world is made and kept on
  `Body::interior`; no per-cell cost): the zones, centre out: solid inner core to `(1 - heat)` of the core's radius,
  liquid outer core to `iron x R`, mantle to the crust's floor `R - DEEP` (never under the core's top), crust to R,
  each with its density.
- **The pull's profile** (`pull`, `pull_integral`): core share c = `iron`, density ratio k = core / mantle, `norm = k
  c^3 + 1 - c^3`, x = r / R; inside the core `k x / norm`, over it `(k c^3 + x^3 - c^3) / (norm x^2)`, outside `1 /
  x^2`. For Terra it is 1.135 g at the core's top and falls evenly to nothing at the centre; 192 m down on the 4 km
  planet it is about 1.002 g, so old worlds are in effect unchanged. `gravity(r) = surface_g x pull(r / R)`. [0154
  stated it with R/sqrt 2 and k = 2.4 for every body; 0162 reads the row.]
- **Mass** `4/3 pi mantle norm R^3`; `enclosed(r)` and `zone_mass(k)` from the zones. **Surface gravity from mass:** the
  world's gravitational constant `k = G R0^2 / M0` is set by the first body pulling `interior::G` (19.62, ADR 0037's
  tuned feel, whatever the planet's size), and every other body pulls `k M / R^2`.
- **Weight and pressure:** `rock_weight(ground, r)` = density x `weight_g` x R x `pull_integral`, the crust's over its
  floor, the mantle's under it; `Body::pressure` adds the water with the same `weight_g` (see
  rock-minerals-and-ore.md, 0158).
- **Liquid core:** volume `4/3 pi (rc^3 - ri^3)` and its share of the body's volume.
- **Magnetic field** (one number over Terra's): `field = (liquid share / first body's) x (spin / first body's)`; 0
  without liquid metal. The share (not the volume) so Terra's `planet=` size does not change it. The spin is the sky's
  (`interior::spin`: Terra once a sidereal day, Luna once a sidereal month). Terra 1.0, Luna 0.0119. What the world does
  with it: bodies-and-orbits.md. [0162, 0164]
- **Readers:** `World::gravity` and `surface_g` (`frames.rs`), `Body::rock`, `rock_weight`, `pressure`, `with_mantle`
  (the mantle's layer count to the row's core top; the metal sea only with a liquid core) in `body.rs`; `phys::G`,
  `fall.rs` and `vitals.rs` read `interior::G`, the one felt-pull constant. Kept as the grid's, not a body's: `CRUST`,
  `DEEP`, `MANTLE_LIFT`, `MANTLE_STEP`.
- **Terra's numbers on Luna, to be made Luna's own at the next generation** (each reproduces today's behaviour): `iron`
  1/sqrt 2 (the Moon's core is about 0.2), `density` 2700, 3300, 7920, `weight_g` 19.62 (her rock is weighed with
  Terra's pull, so her pressure is Terra's at the same depth), falling blocks, the jump and fall damage use
  `interior::G` on every body. **Terra's simplifications on every body:** the pull reckons the crust at the mantle's
  density (it weighs at its own 2700); inner and outer core share one density. The next generation may change them; it
  must bump `GENERATION` and keep this profile for older worlds.
- A new rocky body is a row; its pull, weight and field follow. Gas and icy bodies are refused until built.
- **Locks:** `made_hash` for seed 1 planet 6144 (`mantle=0`, `gen=1`, `gen=2`, default) unchanged
  (0926f028ddcd85a7, 37b2d1b1006b66f7, a2ef250b3ed4203a, bfb878ce14d32b75); the weight of rock at 7 radii and the pull at
  12 radii asserted to the bit for Terra (Luna within 1e-12 relative: her surface pull is `k M / R^2`).
- **Open (the design's order):** the geotherm and the regime (heat with depth stays the provinces' `heat`), the field's
  effects, one bake pipeline, the surface, sky and orbit inputs as rows. [0162]

## No longer holds

- 0152's gravity "about 1.13 of the surface's at the liquid's top": now 1.135 g from the formula (0154, 0162).
- 0152's "Each build step below gets its own ADR" and its status of plan: steps 1 and 2 built (0154, 0156); step 3
  (inner core grid) is not.
- 0154's `ToClient::Welcome` carries `mantle` and the save's version 20 `mantle: bool`: replaced by the generation number
  (`gen`, 0160) in the same byte and in Welcome; and the welcome by the list of bodies (0163).
- 0154's gravity formula with a = R/sqrt 2 and k = 2.4 for every body: generalised by 0162 (the row's `iron` and
  `density`); identical for Terra to the bit.
- 0154's "nothing new is placed in the mantle": caves by rock (0159) and formation by pressure (0158) came later,
  behind generation numbers.
- 0156's status `proposed`: built; its "mantle=0 and saves from before have none" still holds.
- "Luna has no liquid core and no mantle" (0152, 0154, 0162) and 0179's closed floor on Luna, Mars and the icy moon:
  in a world of generation 9 they have the deep grid (0184); older worlds keep both.
- 0162's `world::G`, `CORE`, `CORE_DENSE`, `pull_inside`, `pull_integral` in `frames.rs`, `Rocks::density` and
  `Rocks::mantle_density`: removed.
