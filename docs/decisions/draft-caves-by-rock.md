# NNNN – Cave kinds by rock and formation: lava tubes and fissures

- **Status:** proposed
- **Date:** 2026-11-18

## Context
The developer asked for "realistic lava tubes and caves, all depending on rock and formation", in a game that is
"a nice combination of a crafting game and realism": real-world logic decides where a cave is and why, and a cave
is something a player finds, enters and uses. `docs/design/deep-world.md` section 4 lists the kinds chosen (karst,
lava tubes, magma, faults and fissures, sea caves, deep caverns).

Before this, a cave was ADR 0063's: balls of air (`world::Cave`: face, (i, j, layer), radius), made once when a
planet loads (`caves::make`, from the map and the seed) and asked by every reader through `Body::col_caves` (the
mesher's `generate`, `generated_under` and the physics, `Solid::gaps` for water, the ore ledger, `ore_at`).
Two worms: water-table systems in limestone, and short "tubes" of balls 3 to 6 m under rift and ocean-floor basalt.
Which rock a worm was in was asked at its ball's middle only (`Rocks::at`), so a ball's edge could cut other rock.
Coarse chunks (level > 0) show no caves. No cave is made where the ground is under 3 m + its radius over the sea;
no rule for rivers or lakes.

## Options considered
1. **A new primitive per kind** (a swept tube, a plane slab) asked by `col_caves`: exact shapes, but a second
   kind of geometry in the one hot reader, and more to keep in step across the readers.
2. **The ball, stretched and cut** (chosen): `Cave` gains `h` (its half height, layers; `r` on a ball) and
   `floor` (it is cut flat that far under its middle; `r` on a ball) and `kind`. A tube is a chain of balls a metre
   apart, each stretched to the tube's height and cut flat a third of the way down: an arched roof over a flat
   floor. A fissure is a chain of tall thin ones half a metre apart. `col_caves` asks `Cave::span`, one formula
   for every reader; a plain ball's span is the old one to the bit (`h / r` is exactly 1). No new per-cell cost:
   a column still asks the balls of its tile.
3. **The old caves as rows too**: the limestone worm's walk is tied to the water table and its RNG stream; as a
   row it would move every limestone cave of every world. Left in code (`caves::make`), unchanged; its tube worm is
   not made on a planet with the mantle (the row replaces it there).

## Decision
**`data/caves.toml`**, one row a kind (loader, checks and a test in `caves.rs`): `name`, `shape` (`tube` |
`fissure`), `hosts` (rock names that some province of `data/rocks.toml` is made of), `provinces`, `depth` [least,
most] metres under the ground (roof, floor), `spacing` (a candidate start every so many columns) and `chance`,
`width`, `height`, `length` [least, most]; a tube's `skylight` (metres between), `trench` (chance an opening is a
fallen-in stretch), `branch` (per 100 m), `slope` (least fall a metre); a fissure's `open` (share open at the
surface). An unknown rock or province, a window out of order or past the crust's 192 m, a height that does not fit
its window, a tube's field on a fissure are refused with the row's name.

**The host rule.** Every ball of a kind is checked before it is kept: every cell it hollows under the ground
(`caves::cells`, `col_caves`'s rule) must be, as generated without the cave, one of the kind's hosts: rock by
`Body::rock` under the 4 m soil band (the band is the biome's ground, not rock), in a column of one of its provinces.
Only an opening (a skylight shaft, a trench, an open slot) may cut the band, and only in the columns where it breaks
the ground; so tube roofs and blind fissure tops lie at least 4.5 m down. No cell may lie under or beside (the eight
columns round) a column with generated water over its ground. A tube ends at the first ball that fails (it ends where
the basalt ends); a fissure leaves that ball out (the crack does not open in soft rock) and goes on. Ore is placed
after the caves, so a cave cell is never ore. (Review, round 2: the first version asked `Body::rock` in the band
too, which names the province's rock there, and checked water on the axis column only.)

**Lava tubes** (basalt; rift and volcanic arc). Real ones are the drained channels of pahoehoe flows: they run
downhill a few metres under the ground, 3 to 8 m wide, flat-floored, roofed by the flow's crust, hundreds of
metres long, and are found through skylights where the roof fell in. The walk: a metre a step, heading turned at
most 0.1 rad a metre toward the ground's fall (read 4 m either side) with a slow winding; the floor's radius is the
lower of the last floor and the ground less roof less height, so **the floor never rises downhill**; it ends where
the fall drops under `slope`, the basalt or the province ends, the ground has risen past the depth window, the
ground is under 3 m + its radius over the sea or water stands over it (ADR 0063's sea rule, and rivers and lakes
too), near a seam (12 columns + radius + 4) or within 64 m of the spawn; shorter than 40 m is not made. Then the
openings: the first within 0.3 to 0.8 of `skylight`, then every 0.7 to 1.3 of it, one at least (the middle, if
none fits); each a shaft of balls from the ground into the tube's middle, 3 to 5 m long, or with chance `trench` a
fallen-in stretch of 20 to 50 m; under each the tube's floor is left in a heap (its rubble), up to 0.45 of the
height, falling off over its radius + 3 m. An opening over water or out of the host is not made. Branches leave
with chance `branch` per 100 m, two deep, each with its own openings. Play: a skylight is how a player finds one
from the surface; a tube is a ready road and shelter in rock that does not need digging.

**Fissures** (granite, gneiss, schist, quartzite, andesite, basalt, gabbro; every land province). Faults
and joints open along the contacts between rock bodies; in hard rock a crack stays open, in soft sediment it closes.
The walk starts on the nearest border between provinces of the map within 24 m and follows it half a metre a step,
stepping at most 0.5 m a step toward the border found across its heading (scanned 10 m either side), turning
with it; a slot 1 to 2 m wide (r at least 0.6), 10 to 60 m tall (its height waving slowly along it, never under
the least), its top 2 to 5 m under the ground; with chance `open` a stretch of 10 to 40 m rises through the ground
(ramping over 5 m), where no water stands. It ends where the border is lost, near the sea, a seam or the spawn.
Play: a natural shaft into the deep crust, and a clean section of its rock and ore.

**Which worlds.** Only a planet with the mantle (`Body.mantle > 0`: a new world's planet big enough for one, ADR 0152) gets
the kinds (`// shortcut: new generation rides on the mantle flag, ...` in `caves::make` and in `candidate_of`); old
saves, `mantle=0`, small planets and Luna generate as before. Mantle worlds made in the last days change (new
fissures and tubes, and ADR 0063's tube worm is gone from them); no save format changes. On a mantle planet no plant
stands on a cave's open mouth (`candidate_of`: its foot cell open), which the skylight test found.

Water: the kinds follow the existing rule (no ball where the ground is under 3 m + radius over the sea) and also
make no opening where water stands over the ground (sea, lake, river: `water_at`); no new water behaviour.
Ore: a cave cell is no ore by `ore_at`'s existing rule, so the ledger counts what is left.
Seams: a cave stays inside its face (a ball across a seam would be cut there, ADR 0063's shortcut); a tube that
reaches a seam ends.

## Numbers (drafts for the developer)
| | Lava tube | Fissure |
|---|---|---|
| Starts | one per 48 x 48 columns of rift or arc, 80 % | one per 256 x 256 columns, 70 %, on a border within 24 m |
| Size | 3-8 m wide, 2.5-5 m tall, roof 4.5-6.5 m down | 1-2 m wide, 10-58 m tall, top 4.5-7.5 m down |
| Length | up to 150-700 m (ends earlier by the ground) | 80-300 m (fewer where soft rock leaves gaps) |
| Openings | a skylight every ~200 m, 20 % of them trenches | 35 % open over 10-40 m |

Seed 1, planet 6144 (`technoape-server report=1 seed=1 planet=6144`, the `caves.kinds` object):

| | mantle=0 | with the mantle (before) | with the mantle (after) |
|---|---|---|---|
| made_hash | 0926f028ddcd85a7 | f283474fb9f15012 | 37b2d1b1006b66f7 (2e43836cf7857e0a before review round 2) |
| made_hash, mantle=0, after | 0926f028ddcd85a7 (the same) | | |
| lava tubes | | | 7, 762 m, 15 679 open cells, 7 skylights; nearest 2048 m from the spawn (was 8, 841 m) |
| fissures | | | 122, 11 282 m, 272 502 open cells, 22 open stretches; nearest 833 m (was 27 open) |

Go there (seed 1, planet 6144; `report=1` prints these as `nearest_opening.outside` / `.inside`):
- the nearest tube's skylight from the ground: `technoape.exe run=stand seed=1 planet=6144 at=-0.441413,0.770047,0.460632 yaw=-82 pitch=-45`
- inside it, 15 m on, looking back up the tube: `run=stand seed=1 planet=6144 at=-0.438024,0.772069,0.460483 depth=12.2 yaw=165 pitch=5 place=<lamps every 4 cells, 2 up>`
- the nearest open fissure, from the ground: `run=stand seed=1 planet=6144 at=-0.117184,0.748415,0.652796 yaw=-171 pitch=-30 (on the open slot itself)`
- at its bottom looking up: `run=stand seed=1 planet=6144 at=-0.117184,0.748415,0.652796 depth=47.1 yaw=-171 pitch=75 place=0:0:-1:lamp`

**Lava tubes are rare on seed 1, and why.** The rift is 0.6 % of the surface and lies as a low coastal strip, its
ground 9 m under to 8 m over the sea at the starts; most tubes end within 20-40 m by the sea rule or a river, or at
the rift's edge. The volcanic arc (1.3 %) has no basalt in `data/rocks.toml` (tuff over andesite), so no tube
starts there. With the row as asked, the 4 km planet has 8 tubes. To have more, a data call: a basalt flow layer on
top of the arc's layers (real arcs have basaltic flows: Lava Beds in the Cascades), or the ocean floor's islands in
`provinces` (gives none on seed 1: no ocean-floor land). Changing `rocks.toml` changes the rock of every world,
`mantle=0` too, so it needs the generation number of the shortcut first.

## Checks
| Check | Result |
|---|---|
| `caves::tests::the_caves_file_loads_and_a_broken_one_is_refused_and_says_why` | red first (no hosts: no checks), green |
| `a_cave_kind_opens_only_its_host_rock_in_its_provinces` (dense 1536 planet: every cell of every ball, as generated in the same world without them) | red (no caves made), green; round 2 rewritten on the generated block: red, 50 029 of 2.47 M cells in the soil band ("stone" 1.5-3.3 m down under tube roofs), green |
| `a_cave_kind_keeps_clear_of_water` (no cell under or beside a wet column) | round 2: red, 40 360 cells; green |
| the file test: 17 kinds, a tube of 20 m | round 2: red on the 20 m tube loading (the 17-kind case after it was not reached, so not seen red on its own), green |
| `lava_tubes_run_downhill_and_open_to_the_sky_at_skylights` (axis floor exact; lowest cell within 1 layer; openings <= 1.3 x spacing; skylight air from floor to ground) | red (no tubes), then red on a plant standing in a skylight, green |
| `fissures_follow_province_borders_and_are_tall_and_narrow` (within 16 m of a border, <= 2 m wide, >= 10 m tall) | red (none), then red at 9 m tall, green |
| `a_cave_is_the_same_whichever_chunk_asks_first_and_runs_on_across_a_chunk_border` | red (no tube), then red comparing the skirted padding, green on `generate_raw` |
| `without_the_mantle_the_caves_are_the_ones_from_before` (count and hash of the 1536 planet's caves, recorded before the change) | green before and after |
| `report=1 seed=1 planet=6144 mantle=0` made_hash | 0926f028ddcd85a7 before and after |
| Cost | 0.30 s a planet load after review round 2 (0.14 before; the water check over every ball's footprint), `caves.kinds.made_s`, once on the server and every client; world load 0.5 -> 0.8 s; `run=fly quick=1 seed=1 planet=6144`, play build, twice each: job time after load 380 / 378 us before, 380 / 374 after (no change seen; the flight crosses no new cave), frame p99 3.96-4.04 against 4.01-4.14 ms |
| Stills, round 2 (`out/caves/tube-in3.png`, `fis-sky6.png`, `tube-sky3.png`, 1280 wide) | tube interior with lamps every 4 cells: a wide lit hall, flat floor, stepped walls and roof, a wall block close on the right; the new nearest skylight from the ground: a bush and forest floor, no hole seen; the fissure from its own open column: dark forest floor, no slot seen |
| Stills (the lines above) | tube skylight from the ground: a dark pit some 4-5 m across in the forest floor, dim under the canopy; inside with two lamps: a tunnel about 5 m wide with a flat floor and a stepped low arch, running on into the dark (the skylight's light is not seen from 15 m back); fissure from the ground: **not seen** in four tries (rain forest: grass, trunks and crowns in the way); fissure from its bottom: a narrow slot between two walls rising to a bright opening at the top |
| `bash tools/views.sh` (play build) | ok: no cracks, diffs 0 to 0.24 % (season-winter), no cave mouth visible in any; baselines not touched |

## Consequences
- New kinds are rows; a kind's walk is code (`Shape`). The next kinds, in the order proposed: **karst** with its
  water (the limestone worm as a row, sinkholes, and the karst water ADR 0059 would need: springs), **sea caves**
  (at cliffs on the sea, cut at sea level; small and local), **magma** (chambers under arcs and rifts, a heat source:
  needs lava as a liquid), **deep caverns** (the mantle's, rare: geodes, gas, aquifers).
- Not done (review): the shaft and the tube's neighbouring balls (whose floor is the heap's lower slope) cut a
  crater into the top of the skylight heap: the union takes the lowest floor of the balls over a column.
- Not done (review): the tube and fissure walks use the platform's atan2, sin and cos over hundreds of compounding
  steps, so a Linux server and a Windows client could carve different caves (the old worm had the same exposure,
  over fewer steps). `made_hash` compares them; fixed-point headings or the caves sent by the server would close it.
- At most 16 kinds (a kind's seed keeps 4 bits); a kind shorter than 40 balls' length is refused at load.
- Not done: skylight rubble as the `basalt rubble` block (the heap is uncut basalt: generation has no path for a
  placed block other than the rock rule, and adding one is a second path for every reader); tubes across face
  seams; caves at coarse levels (as before); karst rivers, magma, sea caves, deep caverns, decorations.
- Shortcut: the kinds ride on the mantle flag; give worlds a generation number when the next generation rule needs it.
