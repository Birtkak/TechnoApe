# NNNN – The Mars-like planet's baked map (planet-model step 5a)

- **Status:** proposed
- **Date:** 2026-11-19

## Context
`docs/design/planet-model.md`, build order step 5: the first body made only from a row. The developer's call: "a
Mars-like planet: rocky, cold, thin air, a dead core and so no field, old craters and dry river beds, giant dead
volcanoes with long lava tubes". This step (5a) makes its **baked map** only, through the stage pipeline of
0165-bake-stages, and pictures of it. It is not a body in any world yet: no row in `data/bodies.toml`, no change to
saves, the wire, the sky, the renderer or world generation. Before it, `bake::check` refused mixing families: the
`crust` family was Luna's code, `erosion` meant Terra's, and a list that was neither Terra's nor Luna's was baked on
every call under a file name nobody wrote (0165's shortcut). Constraints: Terra's and Luna's maps to the bit (the
stage lock, `made_hash` bfb878ce14d32b75), deterministic from seed and row, under a minute at full size.

## Options considered
1. **The meaning of a stage on a crust by the list** (a crust with `erosion` is Mars's kind) – needs nothing in the
   row, but `["crust", "impacts", "volcanism"]` would then mean maria on a planet and shields on nothing. Rejected.
2. **By the row: a moon's crust is Luna's, a planet's crust (a body that circles the sun) is this bake (chosen).**
   Real-world reason: Luna's maria flood the basins on the side facing her planet (the thin near-side crust); a
   planet's own volcanism is a hot spot under one plate. `check` takes whether the body is a moon; a moon's crust
   still refuses `erosion`.
3. A general planet generator with options – not asked for; this body's stages only.
4. For the valleys' drainage: on the cratered ground (tried first: networks died in the first crater, 2,227 valley
   cells, most mouths in 1-cell crater lakes) against **on the regional ground, 8 cells wide, carved into the real
   ground with beds that only fall (chosen)**: the valleys run through and breach crater rims as the real ones do,
   and only large hollows held lakes (18,199 valley cells).

## Decision
**The row** (`data/drafts/bodies-mars.toml`, read after bodies.toml's rows by its tests and by `bakemap=`; not loaded
by the game):

```toml
name = "Mars"            # a working name, the developer's to change
parent = "sun"
stages = ["crust", "impacts", "volcanism", "erosion", "ice"]
size = 1.85              # 3328 columns, 104 chunks = 13 x 8 (as Luna's 52 = 13 x 4); 2300 m radius
sun = 65212.5            # 1.5 AU in its own radii (Terra's 23500 x 1.5 x 1.85)
age = 1.0                # NEW key: the share of the impacts' power law its surface has collected
generator = "hills"; ground = ["regolith", "regolith", "stone"]; air = false   # stand-ins, read by no stage
kind = "rocky"; iron = 0.5; heat = 0.15; density = [2900.0, 3500.0, 6700.0]
```
Thin air (about 0.01 atm), no liquid water and the sky colour are not keys yet: no stage reads them. `age` is the only
new key; `parse` refuses it on any row that does not bake a planet's crust, and outside 0 < age <= 1.
The grid: 6144 / 1.9 = 3233.7 would be 101 chunks (prime, one level of the chunk tree); 1.85 gives 104 = 13 x 8.

**`bodies::Bake`** gains a third field, `Option<Planet>` (`name`, `age`), set when the list starts with `crust` and the
body circles the sun; `get`, `for_hash`, `file`, `version` go to `bake::mars` for it. `bake::check(stages, moon)`:
on a planet's crust `erosion` needs `impacts`; on a moon's crust it is refused ("not built yet on a crust without
plates of a moon"); `climate` and `life` on any crust, `impacts` and `volcanism` on plates, stay refused.

**The stages** (`crates/engine/src/bake/mars.rs`; 4 m cells, m = 832, 4,153,344 cells):
- `crust`: the dichotomy. The plains are a cap of 38 % of the surface round a centre within ~25 deg of the north
  pole (the seed tilts it), lobed by noise (+-0.12 rad), smooth at -25 m; the highlands at about +35 m, rough, rising
  24 m away from the plains (the real ones slope north, so their water ran to the plains); a scarp of ~65 m over
  ~160 m between. The plains are "made" at `T_PLAINS` = 0.8. New code; Luna's noise helper.
- `impacts`: two giant basins on the highlands, far from each other (1100-1650 m wide, 0.045 d deep but at most
  `BASIN_DEP` = 66 m, so a larger planet's basins stay in the world's columns, flat floored:
  the planet's lowest ground, own profile); then **Luna's craters**: her power law `K` times `age`, D from 12 m (the
  dust filled smaller ones) to 0.1 r, her crater shapes and her `Land::stamp` (without her kinds). A crater is kept
  only where the ground it fell on is older than it, so the plains' older craters lie buried; with `volcanism` listed
  those younger than `T_VOLC` = 0.9 wait for it.
- `volcanism` (one plate over a hot spot, dead now): a bulge (0.6 rad, +70 m) on the dichotomy's edge, highland side,
  the azimuth nearest the equator and far from the basins; lava floods its low ground (dome + 44 m, to -20 m at its
  lobed edge, so it spills onto the plains): older craters drown, high rims stand out. Three shields in a row across
  it (NE-SW, 0.13-0.16 rad, 150-190 m over their foot) and the giant past its NW edge (0.3 rad, summit 348-360 m,
  a 35 m basal cliff, two nested calderas), each a near-linear flank `(1-x)^1.15` with a flat caldera (7 % of its
  height). Lava tubes down the flanks (2 a shield, 5 on the giant; balls of r 4-6.5 m every 1.2 r under a 5-9 m roof,
  two pits each), as Luna's map marks hers (same tuple, same `Map::tubes`). The canyon: from the bulge's flank along
  the dichotomy's edge (0.24 rad inside it), away from the giant, 0.9-1.05 rad long, its floor falling from +45 to
  -8 m eastward, half width 35-100 m, a second trough beside its middle. Then the young craters.
- `erosion` (long ago, dry): three chaos patches at the canyon's end (floor -10 m with knobs) and an outflow channel
  from each toward the plains (half width 20-30 m, floor -10 to -29 m) until 60 m out on them. Then **Terra's
  drainage** (`land::drain`: Priority-Flood and D8) with the old shore at -12 m as its sea, on the ground 8 cells wide
  (`land::upsample` of the cell means); valleys where 6e4 m^2 drain, not on lava or shields; their beds carved
  (1.2 (A/A0)^0.3 m, to 7 m, banks widened past 4 A0) and made to fall from the springs down (a rim in the way is cut
  through); lake beds (water 2 m or more over the regional ground) where valleys end, flattened 3 m under the spill;
  the basins' floors under the shore are old sediment; deltas (fans of 25-90 m) where valleys over 4 A0 met standing
  water. The beds are the map's `rivers` (flow 0: the water is gone), so the world's river cutter (`Map::river`)
  will cut them; the caps' valleys are dropped (buried).
- `ice`: layered caps, north 0.2 rad (45 m thick), south 0.15 rad (32 m), lobed, the thickness terraced in 3 m steps,
  three spiral troughs; frost on the floors of craters of 30 m and more poleward of 60 deg.
- Nothing reads where a player lands: the map's `spawn` (Luna's rule, gentle ground nearest +y) is picked after the
  last stage.

**The map's kinds** (biome ids after Luna's; rows of `data/biomes.toml` come with the run that puts the body in the
world): 22 cratered highland, 23 dust plain, 24 lava plain, 25 basalt shield, 26 canyon floor, 27 chaos, 28 outflow
channel, 29 sediment basin, 30 delta, 31 polar ice, 32 frost. `Map::rock` answers None on it (no provinces yet).

**The map file:** version 201 (Terra's count from 1, Luna's from 101, a planet's crust's from 201), Luna's layout
(the map, then the tubes). **Cache name:** `saves/bakes/seed-<seed>-planet-<n>-<row name, lower case, other
characters '-'>-<low 32 bits of FNV of "<stages:?> <age:?>">-v201.map`, e.g. `seed-1-planet-3328-mars-89b8b05e-v201.map`:
another list, age or bake version is another file, a renamed body bakes once more. Baked once, kept in memory by path
(`mars::get`, `keep`) and read back from the file; a file that is no map of that version, seed and n is baked again.

**Seeing it:** `technoape-server bakemap=data/drafts/bodies-mars.toml body=Mars seed=1 [n=] [out=prefix]` bakes it,
keeps it as the game would find it, writes `<prefix>-height.png` (altimetry colours with relief shading and a scale),
`-kinds.png` (the kinds in flat colours with a legend) and `-features.png` (grey relief; craters seen over 40 m,
basins, shields and calderas, tubes and pits, canyon axis, channels, valleys, lake beds, deltas, chaos and caps), an
equirectangular projection 2400 x 1200 centred on the canyon (`bake/pictures.rs`, the image and 3 x 5 font of
`prototypes/world-bake`), and prints one JSON line of the numbers.

## Numbers (full size, n = 3328, play build)
| | seed 1 | seed 2 |
|---|---|---|
| bake | 2.7-4.3 s (crust 0.5, impacts 0.8-1.1, volcanism 0.4, erosion 0.8-1.0, ice 0.02) | 3.3-3.5 s |
| map bytes | 21.0 MB | 21.0 MB |
| lowland share (the crust's) | 0.372 | 0.378 |
| highland / plains mean height | 35.2 / -24.6 m | 35.8 / -24.6 m |
| shares: highland, plain, lava, shield, sediment, caps, frost | 0.50, 0.30, 0.082, 0.034, 0.043, 0.015, 0.015 | 0.49, 0.31, 0.079, 0.035, 0.054, 0.016, 0.014 |
| craters made / seen | 64,653 / 46,538 | 64,378 / 46,700 |
| seen over 24 m per km^2: highland, plain, lava, shield | 272, 62, 36, 34 | 270, 57, 32, 30 |
| basins | 1545, 1095 m | 1596, 937 m |
| shields (radius, foot, summit m) | 311/87/250, 314/107/296, 344/89/262; giant 690/29/352 (caldera 23.6 m) | 342/87/265, 324/111/263, 355/89/260; giant 690/29/352 |
| canyon | 2234 m | 2116 m |
| outflow channels | 698, 660, 776 m | 532, 474, 268 m |
| valley cells; mouths into plains, basins, lake beds, other | 18,199; 20, 4, 237, 21 | 18,528; 21, 0, 241, 27 |
| deltas; tubes, balls, pits | 136; 11, 533, 21 | 139; 11, 576, 21 |
| height range | -41.1 .. 352.0 m | -42.8 .. 351.9 m (the world's columns: -56 .. 378) |

The height range by size (the basins' depth held to 66 m): n 832: -30.3 .. 355.9, -33.0 .. 348.4; n 3328: -41.1 ..
352.0, -42.8 .. 351.9; n 6144: -45.4 .. 360.8, -51.4 .. 350.1 (seeds 1, 2). Before the bound n 6144 reached -96.6.

What the row keeps across seeds: the plains' share and heights, one bulge on the edge with three shields and the
giant past its NW side, the canyon along the edge away from the giant ending in chaos and three channels into the
plains, two basins in the highlands, caps at both poles, the crater densities. What the seed moves: the plains'
centre and lobes (so the bulge's and canyon's longitude), the basins' places and sizes, every crater, the valley
networks and lakes, the shields' heights within their ranges, the tubes' paths.

## Checks
Red first, each seen failing for the right reason before the code that makes it pass:
- `bodies::the_mars_draft_row_parses_and_its_list_is_accepted` (red: "stage 'erosion' is not built yet on a crust
  without plates", the row refused);
- `bodies::bad_rows_are_refused`, extended (red on the planet cases: erosion refused on a planet's crust): a planet's
  `["crust","erosion"]` needs impacts, `climate`/`life` on a crust, `impacts` on plates, age 0 or 1.5, age on a
  plates row or without stages are refused with a message; a moon's crust gets no `Planet`;
- `bake::mars::a_planets_bake_is_the_same_twice_and_round_trips_through_its_file` (red with the stages stubbed:
  no tubes, no beds): same hash twice, bytes round trip, the file name, another list or age another file, read back
  from the file (a valid map put there is what comes back), baked again over a broken file;
- `bake::mars::the_map_has_what_the_row_calls_for`, seeds 1 and 2 at n = 1664 (red with the stages stubbed: "no lava
  plain"): every kind present; heights in the world's columns; lowland 0.3-0.46; highland - plains > 50 m; crater
  density on highlands > 3x plains and lava; 2 basins; 4 shields with one giant, each with a caldera (> 5 m, its
  floor under its rim) and SHIELD at its centre, the giant > 250 m over the highland mean, > 1.5x the others' height
  and radius; canyon 0.85-1.1 rad, its axis on canyon floor; 3 channels ending under the shore; no valley reach
  running uphill; > 500 valley cells, >= 80 % of mouths into plains, basins or lake beds; a delta; caps over both
  poles (inner 0.1 rad); >= 6 tubes and pits, all on shields; the bulge moves with the seed and the giant's angle to
  the landing point differs between seeds.
- `bake::mars::heights_stay_in_the_worlds_columns_at_every_size`, seeds 1 and 2 at n = 832 and 6144 (red before the
  basins' depth was bounded: "seed 1 n 6144: heights -96.6 to 360.8").
- The round-trip test also refuses a version-201 map with a kind outside 22..=32 (red before `Map::from_bytes`
  checked it). `bakemap=` refuses an n that is not whole chunks (n=0 exits 2 with a message, no NaN bake).
- Every map cache writes through `<file>.<pid>.tmp`, so two processes baking one map never rename each other's
  half-written file (the kept file names and bytes of Terra, Luna and the planet are unchanged).
- Locks untouched and green: `the_stages_bake_terra_and_luna_to_the_bit_as_before`; `report=1 seed=1 planet=6144`
  made_hash bfb878ce14d32b75 (Terra) and cdd810732c670a46 (Luna, generation 4), as 0166 has them.
  `cargo test -p engine`: 460 passed, 0 failed, 23 ignored.

## Consequences / what the next runs need
- **The body in the world:** the row moves into `data/bodies.toml` (a third row: `MAX` bodies is 4); saves, Welcome
  and recordings already carry N bodies (0163) but a world made before must keep two bodies: **generation 5** adds
  the third; the sky's pose (a ring round the sun until real orbits, 0163); the wire sends its map (version 201 must
  be accepted by `net` and `bake::find` by the planet's file name, not (seed, n, version)); `World::with_grid` reads
  its map by `Bake::get`; `world::AMP` holds its 352 m summit.
- **Places that would meet a Mars map wrongly** once it is a body (the reviewer's list, to fix in that run):
  `crates/client/src/globe.rs` (about lines 64, 73: `version > 100` takes it for Luna's and `.min(6)` gives it
  Luna's colours); `crates/engine/src/biomes.rs` (about line 622: 22 rows registered, kinds 22..=32 read past the
  table); `crates/engine/src/world/body.rs` (about lines 103, 146, 153, 301 test `== luna::VERSION`, so a Mars map
  takes Terra's branches); `Bake::plates()` (false for it, which today means Luna's kind of map).
- **Windows and Linux must bake the same bytes:** maps are never sent, each side bakes its own and `state.rs`
  compares hashes. This bake adds float thresholds a last-bit difference could flip: `sin`/`cos`/`acos`/`powf`
  from the platform's libm (the canyon's width, the valley depth law `powf(0.3)`, the bulge's azimuth), the
  frost line `|z| > 0.87`, lake water `>= 2 m`, `SHORE` -12 m, crater age against the ground's age (`t >= made`),
  and the cover test of seen craters; a Linux bake of seeds 1 and 2 must be compared with Windows' hash first.
- **Rock provinces and ground:** `Map::rock` from its kinds as Luna's (old cratered crust: basaltic breccia; basalt
  shields and lava plains; sediment in lake beds, basins and deltas: clays and sulphates; canyon floors: layered
  deposits; caps: water ice and dry ice), `rocks.toml` rows with `body`, `biomes.toml` rows 22-32 and their blocks
  (red dust over everything; the ground triple of the row is Luna's regolith until then).
- **Air and sky colour:** a key for thin air (about 0.01 atm) and the butterscotch sky; dust storms later.
- **Ore and tubes:** `caves.toml`'s round tubes in its shield basalt from `Map::tubes` (as Luna's); minerals by its
  provinces (iron oxides in the dust, sulphates and clays in the lake beds and deltas, ice under the caps and frost).
- Known looks to judge on the pictures: most valleys end in highland lake beds (237 of 282 mouths), few reach the
  plains; the caps are thin bands in the projection (a polar view would read better); a lake bed fills a basin flat
  (seed 2's second basin) and buries its craters; the shields are smooth cones with no flank detail; no chaos-like
  fretted terrain along the scarp; frost on many craters at high southern latitudes.
