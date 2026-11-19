# Caves

Caves by rock and formation: the old worms, lava tubes, fissures, karst, the rules every cave obeys, and Luna's
caves. What holds today. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists
every number. Generation numbers: `rock-minerals-and-ore.md`.

## The developer's calls
- "Realistic lava tubes and caves, all depending on rock and formation", in a game that is "a nice combination of a
  crafting game and realism": real-world logic decides where a cave is and why, and a cave is something a player
  finds, enters and uses. [0159]
- "Basalt and tubes": the arcs get basalt flows so lava tubes are not only a coastal rift's. [0160]
- Nothing placed for where the spawn is except copper: caves too. A tube within 1.2 km of the spawn was dropped as a
  target. [0160]
- Karst is the next kind after tubes and fissures. [0161]
- The kinds' numbers are drafts for the developer. Open data calls: more tubes (a basalt flow layer higher on the arcs
  or flows favouring high ground, higher arcs in the bake), the median tube length (51 m against 150 asked), deeper
  karst (a table toward the nearest valley floor rather than six tenths of the height, or thicker limestone). They
  change what is generated, so they wait. [0159, 0160, 0161]

## Rules every cave obeys
- A cave is balls of air (`world::Cave`: face, (i, j, layer), radius, plus `h` half height in layers, `floor` the
  flat cut that far under its middle, `kind`; a plain ball's `h` and `floor` are `r`), made once when a planet loads
  (`caves::make`, from the map and the seed) and asked by every reader through `Body::col_caves` (the mesher's
  `generate`, `generated_under`, physics, `Solid::gaps` for water, the ore ledger, `ore_at`) through `Cave::span`: one
  formula; a plain ball's span is the old one to the bit. Balls are bucketed by chunk footprint (`cave_tiles`, 32
  columns a tile). Coarse chunks (level > 0) show no caves. Made from the seed in a fixed order, so chunk order
  cannot matter. No plant stands on a cave's open mouth. [0063, 0159, 0160, 0161]
- **Off limits:** seams (12 columns; a cave stays inside its face), the spawn (64 columns for the kinds, 100 and 40
  for the old worms), the sea (no ball where the ground is under 3 m + its radius over the sea; a round tube uses its
  radius, the older tube its height), and water: no cell under or beside (the eight columns round) a column with
  generated water over its ground (sea, lake, river: `water_at`); no opening where water stands. [0063, 0159, 0160]
- **The host rule** (the kinds of `data/caves.toml`): every ball is checked before it is kept: every cell it hollows
  under the ground (`caves::cells`) must be, as generated without the cave, one of the kind's hosts: rock by
  `Body::rock` under the 4 m soil band (the band is the biome's ground, not rock), in a column of one of its
  provinces. Only an opening (a skylight shaft, a trench, an open slot, a sinkhole) may cut the band, and only in the
  columns where it breaks the ground; so tube roofs and blind fissure tops lie at least 4.5 m down. A tube ends at the
  first ball that fails; a fissure leaves that ball out and goes on. [0159]
- Ore is placed after caves, so a cave cell is never ore. [0159]
- No plant stands where its crown would hang over an open mouth (generation 2 on; `Body::candidate_of` asks every
  column of the crown's square, first the cave tiles under it, one lookup a tile, `col_caves` only where a tile
  holds a cave). [0160, 0161]
- At most 16 kinds (a kind's seed keeps 4 bits); a kind shorter than 40 balls' length is refused at load. [0159]
- Cross-platform risk: the walks use the platform's `atan2`, `sin`, `cos` over hundreds of compounding steps, so a
  Linux server and a Windows client could carve different caves; `made_hash` compares them; fixed-point headings or
  caves sent by the server would close it. [0159, 0063]
- Not done: tubes, fissures and karst across face seams; caves at coarse levels; walls between karst and other kinds
  are not held to 3 m (they never share a cell); the crater cut into a skylight heap's top by neighbouring balls;
  skylight rubble as a placed `basalt rubble` block. [0159, 0160, 0161]

## `data/caves.toml`
One row a kind (loader, checks and a test in `caves.rs`): `name`, `shape` (`tube` | `fissure` | `karst`), `hosts`
(rock names some province of `data/rocks.toml` is made of), `provinces`, `generations = [least, most]`, `depth`
[least, most] metres under the ground (roof, floor), `spacing` (a candidate start every so many columns) and `chance`,
`width`, `height`, `length` [least, most]; a tube's `skylight` (metres between), `trench` (chance an opening is a
fallen-in stretch), `branch` (per 100 m), `slope` (least fall a metre); a fissure's `open` (share open at the
surface); a karst's `skylight`, `branch`, `hall`, `hall_height`, `sinkhole`, `shaft` (and no `trench`, `slope` or
`open`). An unknown rock or province, a window out of order or past the crust's 192 m, a height that does not fit its
window, a field of another shape are refused with the row's name. A row's place in the file seeds it: rows are
appended, an old row is given an end in `generations`, never changed. [0159, 0160, 0161]

## Which worlds
- The kinds run on a planet with the mantle (`Body.mantle > 0`, generation 1 and on) and, from generation 4, on another
  body whose provinces a row names and which has no mantle (Luna). Old saves, `mantle=0`, small planets generate as
  before. ADR 0063's tube worm is not made on a mantle world; its limestone worm is not made where karst replaces it
  (generation 3 on a mantle planet; a small planet without the mantle keeps it). [0159, 0161, 0166]
- ADR 0063's worms (the provinces' `caves` key) are the home's only. Gates in `caves::make`: worms
  `b.gen < 3 || b.mantle == 0`, the kinds `b.mantle > 0` on the home and without a mantle on another body. [0165,
  0166]

## The old worms (generation 0, and the limestone worm up to generation 2)
- Limestone systems that hold to the water table (six tenths of the height over the sea smoothed over 40 m, at least
  3 m under the ground) and end where the limestone does; lava tubes 3 to 6 m under basalt ground, downhill, with
  skylights. Kept off seams (12 columns), the spawn (40) and the sea (3 m of land). Which rock a worm is in is asked at
  its ball's middle only. Kept in code unchanged. [0063, 0159]

## Lava tubes (basalt; rift and volcanic arc)
- The generation-1 row (`generations = [1, 1]`): a chain of balls a metre apart, each stretched to the tube's height
  and cut flat a third of the way down (an arched roof over a flat floor). One start per 48 x 48 columns of rift or
  arc, 80 %; 3-8 m wide, 2.5-5 m tall, roof 4.5-6.5 m down, 150-700 m; a skylight about every 200 m, 20 % trenches.
  The walk: a metre a step, heading turned at most 0.1 rad a metre toward the ground's fall (read 4 m either side)
  with a slow winding; the floor never rises downhill; it ends where the fall drops under `slope`, the host or province
  ends, the ground rises past the depth window, near the sea, water, a seam or the spawn; shorter than 40 m is not
  made. Openings: the first within 0.3 to 0.8 of `skylight`, then every 0.7 to 1.3 of it (one at least); a shaft of
  balls 3 to 5 m long, or with chance `trench` a fallen-in stretch of 20 to 50 m; under each the floor is left in a
  heap up to 0.45 of the height, falling off over its radius + 3 m. Branches with chance `branch` per 100 m, two deep.
  [0159] Kept for generation 1 only. [0160]
- **Round lava tubes** (generation 2 on; row `generations = [2, 255]`, walked by `caves::round_tube`): a chain of
  round balls a metre apart (`h = r`), each cut flat a third of its radius under its middle: a flat floor, an arch
  over it, two thirds of the width tall. Width drawn per tube (4 to 7 m), waving by 12 % over some 250 m. A start in
  the host first climbs the ground's slope (read 16 m either side) while it and a width out each way are basalt at
  the tube's depth, then downhill as above (least slope 0.008), roof 4.5 to 6 m down, turning off water seen 10 m
  ahead. A round tube ends where it would come within 3 m (wall to wall) of another; a branch (0.3 per 100 m after
  its first 25 m) leaves at 0.6 to 1 rad, bends on to its side over its first 80 m and is let near its parent for 25 m
  only. Skylights as above, the shaft at least 2.3 m in radius (at least 3 x 3 columns open at the ground). [0160]
- Seed 1, 4 km planet: generation 1 had 7 tubes (762 m); generation 2 has 98 tubes, 5,543 m, median 51 m, nearest
  2,012 m from the spawn (arcs lie low; most end at the coast). [0160]

## Fissures (granite, gneiss, schist, quartzite, andesite, basalt, gabbro; every land province)
Faults and joints open along contacts between rock bodies; in hard rock a crack stays open, in soft sediment it
closes. One start per 256 x 256 columns, 70 %, on a border within 24 m. The walk follows the nearest border between
map provinces half a metre a step, at most 0.5 m a step toward the border found across its heading (scanned 10 m
either side); a slot 1 to 2 m wide (r at least 0.6), 10 to 60 m tall (waving slowly, never under the least), its top 2
to 5 m under the ground, 80-300 m long; with chance `open` (35 %) a stretch of 10 to 40 m rises through the ground
(ramping over 5 m), where no water stands. It ends where the border is lost, near the sea, a seam or the spawn. Row
generations 1 and on. Seed 1, 4 km planet: 122 fissures, 11,282 m, 22 open stretches, 272,502 open cells (the same at
generations 1 and 2). [0159, 0160]

## Karst (limestone; the basin; generation 3 on)
Real karst: water made acid by the soil dissolves limestone along joints and bedding down to the water table. Dry
only: underground lakes, rivers and springs are the next run (the water model is not touched). Only `limestone` is a
host (marble karst left for later). [0161]
```toml
name = "karst"; shape = "karst"; hosts = ["limestone"]; provinces = ["basin"]
depth = [7.0, 60.0]; spacing = 160; chance = 0.8
width = [3.5, 6.0]; height = [2.6, 4.0]; length = [250.0, 1200.0]
skylight = 60.0; branch = 0.8
hall = [10.0, 30.0]; hall_height = [6.0, 15.0]
sinkhole = [9.0, 18.0]; shaft = [3.0, 5.0]
```
- **Start:** a candidate in the basin seeks the band of the slope where a passage can run, at most 160 m.
- **Passage** (`caves::passage`): a ball a metre, an arch over a flat floor (the lower quarter of the height cut
  flat), `width` and `height` drawn per system. The floor is 0.3 m over the highest water table across it
  (`caves::water_table`) and follows it at most 0.08 m a metre; it runs along the table's contour (not down its
  slope), leaning 0.35 rad toward the fall, winding, steering by what lies 8 m ahead. It ends where a ball would open
  a non-limestone cell, a column outside the basin, wet ground, under the table of its own column, or into the soil
  band; where its floor leaves the depth window; near the sea, a seam or the spawn keep-out (`Probe::off`); or within
  3 m of another passage's wall (`crowds`; a branch is let near its parent for its first 25 m).
- **Branches:** 0.8 per 100 m after the first 25 m, two levels deep, up the slope on their side, half the length, a
  little narrower.
- **Halls:** where a branch leaves (if it fits) and along a passage every 80 m at least where the rock allows: a
  flat-floored room `hall` wide and `hall_height` tall at the passage's floor, made 2 m narrower and 1 m lower a try
  down to the least, else none; it keeps 3 m from other passages, and its floor's layer lies at most `caves::STEP` = 1
  layer over or under every floor of the passage through it (so a player who climbs one block walks in and out).
- **Sinkholes:** along each passage, the first within a third of `skylight`, then at least `skylight` apart, wherever
  the limestone reaches up to the soil band and both parts fit: a bowl (a squashed ball `sinkhole` wide at the ground,
  as deep as a quarter of its width or more where the ground slopes) and a shaft `shaft` wide down to the passage. They
  keep 3 m from other passages and are in the crowding set at their real radius. Only these cut the soil. A system
  with no sinkhole is not made.
- Seed 1, 4 km planet: 94 systems, passages 10,290 m (median system 80 m), 232,366 open cells, 69 halls, 140
  sinkholes, nearest sinkhole 800 m; the largest system 872 m, floor 8.0 to 12.9 m under the ground. Passages lie only
  where the ground is some 20 to 35 m over the sea, 8 to 13 m of cover; halls are mostly the least size. Limestone lies
  under the soil band on 40 % of the land. [0161]
- For the water run: every passage's floor lies 0.3 m over the water table of its columns (80 % or more of stations
  within 1.5 m of it); the main passage is the first in `System::axis` up to its first jump of 1.5 m or more; a spring
  is where the main passage's lowest end is nearest a valley wall; what lies below the table is solid limestone now.
  [0161]
- Not done: water in it, dripstone and rubble, marble karst, sinkholes without a passage, larger halls where rock is
  thin. [0161]
- Next kinds in order proposed: sea caves (cliffs on the sea, cut at sea level), magma (chambers under arcs and
  rifts: needs lava as a liquid), deep caverns (the mantle's: geodes, gas, aquifers). [0159]

## Luna's ore and caves
- Luna's ores: `rock-minerals-and-ore.md` (Luna's rock and ore). Her caves: from generation 4 `with_luna` keeps
  `make`'s caves and drops the baked chains of ADR 0070; up to generation 3 the baked tubes are her caves as before.
  [0166]
- One row at the end of `data/caves.toml`, `lunar lava tube`: the round tube rule in mare basalt in the maria,
  `generations` 4 on, 6 to 10 m wide, 4.5 to 20 m deep, 300 to 1500 m, a start every 192 columns at 0.7, a skylight
  every 150 m (with the fallen roof's heap under it), `slope` 0.001 at least (the maria are flat flows). On the
  standard Luna (seed 1): 15 tubes with branches, 210 m median, 1,321 m longest, 6.7 km in all, 44 openings. They stop
  at the anorthosite and in a deep crater. Shape: one round passage, its lower third filled flat, its width waving
  slowly, a heap under each skylight, made by her rock field (the baked chains of ADR 0070 were beaded balls on a random
  walk: 7 tubes, 216 m median, 289 m longest, 12 pits). [0166]
- The kinds of `caves.toml` run on another body without a mantle; a karst needs a sea and is not made on Luna;
  Luna's `caves::water_table` is minus infinity (dry to the bottom). [0166]
- Cost: Luna's world build 0.04 s at generation 3 against 0.72-0.85 s at 4 (the tube walk). [0166]

## Cost
- A planet load's cave walk on the 4 km planet: 0.14 s (generation 1 before review) -> 0.30 -> 0.73 (generation 2)
  -> 2.26 s (generation 3), on the server and every client; chunk jobs 403 -> 478 us from the flow noise
  (`docs/perf-debt.md` rows). [0159, 0160, 0161]

## No longer holds
- 0159's generation-1 tube as the lava tube of new worlds: kept for generation 1 only, replaced by the round tube at
  generation 2 (0160).
- 0159's "the kinds ride on the mantle flag" shortcut: replaced by the generation number (0160).
- 0159's volcanic arc without basalt (no tube starts there): replaced by 0160's flows.
- 0159's target of 8 tubes on seed 1: replaced by 0160's 98.
- 0063's limestone worm on generation 3 mantle worlds: replaced by karst (0161).
- 0159's/0160's `Body::candidate_of` plant crown check as written: made cheaper in 0161 (same answer).
- 0166's table of baked chains versus round tubes for Luna: decision made (B); the baked tubes of ADR 0070 stay for
  generations up to 3 only.
