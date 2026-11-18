# NNNN – Dry karst in the basins' limestone (generation 3)

- **Status:** proposed
- **Date:** 2026-11-18

## Context
After lava tubes and fissures (ADR 0159, 0160) the developer chose karst as the next cave kind, with the standing
taste calls: realistic caves that depend on rock and formation, where real-world logic says where a cave is and
why, and each is something a player finds, enters and uses; nothing placed because of where the spawn is.

Real karst: rain water, made acid by the soil, dissolves limestone along its joints and bedding planes down to the
water table. That gives sinkholes (dolines) at the surface, some with an open throat; shafts from a sinkhole down to
the cave level; passages that wind near-level along the water table and the bedding and join into a network that
drains toward the nearest valley or coast; and halls where passages meet or the roof broke down.

In `data/rocks.toml` limestone is one province's: the basin (sandstone 25 m, shale 15 m, limestone 45 m, granite)
from the datum 60 m over the sea, folded 6 m. So its top lies some 20 m over the sea, under 40 m of sandstone and
shale on high ground and right under the soil where the land is low. ADR 0063's limestone worm already walks at the
water table (`caves::water_table`: the ground's height over the sea, smoothed over 40 m, six tenths of it, and at
least 3 m under the ground). Seed 1, 4 km planet: limestone lies under the soil band on 40 % of the land and at the
water table under 43 % (`report=1` `limestone`).

This run is dry: underground lakes, rivers and springs are the next. The water model is not touched.

## Options considered
1. **Tune ADR 0063's worm** (balls at the table, random turns): no sinkhole, no hall, no rule of water; its balls
   cut whatever lies at the table's depth into a string of round rooms. Rejected; kept unchanged below generation 3.
2. **Passages down the table's slope** (as a tube runs down the ground): tried first. The table is six tenths of the
   height, so going down it the passage rises toward the ground at four tenths of the ground's fall, and every
   passage left its depth window within tens of metres (0 of 921 starts on the 1536 planet grew 40 m). Rejected.
3. **Passages along the table's contour, leaning down it** (chosen): a gallery near-level at the table keeps its
   depth, as a real one follows the bedding; it leans 0.35 rad toward the fall (where the system drains) and steers
   toward the nearest heading 8 m ahead where it can go on.
4. Halls only where branches meet: rejected after the test: at the table there is too little limestone and ground
   over the floor for a 6 m hall at most junctions (8 to 10.6 m from floor to ground at the four junctions of the
   1536 planet, 10.5 needed). Halls are also made along a passage where the roof could break down: where the
   rock allows.
5. Marble (the folded belt's 15 m band) as a host: real marble karst exists, but the band is thin and folded;
   left for later. Only `limestone` is a host.

## Decision
**Generation 3** (`world::GENERATION` = 3). From it ADR 0063's limestone worm is not made (`caves::make`; below 3 it
is exactly as before), and the row `karst` of `data/caves.toml` (`generations = [3, 255]`, appended, the old rows
untouched) is walked by `caves::karst`:

```toml
name = "karst"; shape = "karst"; hosts = ["limestone"]; provinces = ["basin"]
depth = [7.0, 60.0]; spacing = 160; chance = 0.8
width = [3.5, 6.0]; height = [2.6, 4.0]; length = [250.0, 1200.0]
skylight = 60.0      # metres between sinkholes along a passage, at least
branch = 0.8         # per 100 m
hall = [10.0, 30.0]; hall_height = [6.0, 15.0]
sinkhole = [9.0, 18.0]; shaft = [3.0, 5.0]
```
A karst row must have `skylight`, `branch`, `hall`, `hall_height`, `sinkhole` and `shaft` and no `trench`,
`slope` or `open`; the four karst fields are refused on a tube or a fissure; ranges are checked with a message.

**A system** (one `caves::System`; `System::halls` new):
- *Start:* a candidate in the basin seeks the band of the slope where a passage can run (up the slope where the
  table lies too near the ground, down where the table lies over the limestone), at most 160 m.
- *Passage* (`caves::passage`): a ball a metre, an arch over a flat floor (the lower quarter of the height cut flat),
  `width` and `height` drawn per system. Its floor is 0.3 m over the highest water table across it and follows it
  at most 0.08 m a metre (near-level); it runs along the table's contour, leaning down it, winding, and steers by
  what lies 8 m ahead. It ends where a ball would open a cell that is not limestone, in a column outside the basin,
  under or beside wet ground (ADR 0159's water rule), under the water table of its own column, or into the soil
  band; where its floor would fall under the table or leave the depth window; near the sea, a seam or the spawn's
  keep-out (`Probe::off`, unchanged); or where it would come within 3 m of another passage's wall (the round tubes'
  `crowds`, by passage number; a branch is let near its parent for its first 25 m).
- *Branches:* 0.8 per 100 m after the first 25 m, two levels deep, up the slope on their side (tributaries), half
  the length, a little narrower.
- *Halls:* where a branch leaves (if it fits) and along a passage every 80 m at least where the rock allows: a
  flat-floored room `hall` wide and `hall_height` tall at the passage's floor (or over the highest table under it),
  made 2 m narrower and 1 m lower a try down to the least, else none. A hall keeps 3 m from other passages.
- *Sinkholes:* along each passage, the first within a third of `skylight`, then at least `skylight` apart, wherever
  the limestone reaches up to the soil band and both parts fit: a bowl (a squashed ball `sinkhole` wide at the
  ground, as deep as a quarter of its width or more where the ground slopes, so all of it opens) and a shaft
  `shaft` wide from it to the passage. Only these cut the soil. ADR 0160's plant rule already keeps every plant out
  of and from over them.
- A system with no sinkhole is not made (nothing leads into it).

**One rule for every reader:** the balls go into `Body::caves` as every kind's do; `col_caves` serves the server,
the client, physics and the mesher alike; made from the seed in a fixed order, so chunk order cannot matter.

**The plant check, cheaper:** `Body::candidate_of`'s crown rule (ADR 0160) now first asks the cave tiles under the
crown's square, one lookup a 32-column tile, and asks `col_caves` per column only where a tile holds a cave (same
answer: a tile lists every cave within its reach). Every plant candidate of the 1536 planet: 515-545 -> 437-451 ns
(three runs each, test build), the same 63,094 plants.

**For the water run:** every passage's floor lies 0.3 m over the water table of its columns, or higher where the
table fell faster than 0.08 m a metre (the test: 80 % or more of the stations within 1.5 m of it). So a karst
system is the space just over the table: water filling a passage to the table is what a flooded passage would be,
and a spring is where the main passage's lowest end is nearest a valley wall. The water run will need: the
station's table (`water_table` at the axis point), which passage is the main one (the first in `System::axis` up to
its first jump of 1.5 m or more), and a rule for what lies below the table (now solid limestone).

## Numbers (drafts for the developer)
Seed 1, planet 6144 (`technoape-server report=1 seed=1 planet=6144`):

| | |
|---|---|
| made_hash gen 0 / 1 / 2 | 0926f028ddcd85a7 / 37b2d1b1006b66f7 / a2ef250b3ed4203a: as main |
| made_hash gen 3 | d8e179a52e3e5209 |
| limestone on land | 40.2 % under the soil band, 43.3 % at the water table |
| karst systems | 94, passages 10,845 m (median system 80 m), 240,336 open cells |
| largest system | 872 m of passages, floor 8.0 to 12.9 m under the ground, 11 halls, 12 sinkholes |
| halls, sinkholes | 80, 147 |
| nearest sinkhole | 800 m from the spawn (not tuned) |
| old worm's balls | 12,575 in limestone at gen 2; none at gen 3 |
| cave walk at load | 0.70 s (gen 2) -> 2.33 s (gen 3); the report's world 1.2 -> 2.8 s |

Go there (`technoape run=stand seed=1 planet=6144 ...`):
- a sinkhole: `at=0.118121,0.908955,0.399811 depth=0.0 yaw=6 pitch=-40 eye=14` (its middle
  `at=0.118314,0.907488,0.403072`; the shaft's foot `at=0.118415,0.907520,0.402970 depth=9.0 yaw=77 pitch=75`)
- a passage 60 m from an opening: `at=0.129737,0.907354,0.399846 depth=8.6 yaw=107 pitch=0`
- a hall: `at=0.203486,0.912160,0.355749 depth=10.5 yaw=0 pitch=5` (10 m across, 6 m tall, 1,169 m from the spawn)

Shallow: the table at six tenths of the height means a passage lies only where the ground is some 20 to 35 m over
the sea, with 8 to 13 m of cover; the halls are mostly the least size (10 m, 6 m) for want of rock over the table.
Deeper systems need a lower table (a calls for the water run: e.g. the table toward the nearest valley floor rather
than six tenths of the height) or thicker limestone near the ground; both change what is generated, so they wait
for the developer.

## Checks
| Check | Result |
|---|---|
| `generations_0_1_and_2_generate_as_before` (1536 planet hash of caves, ores, rock) | lock recorded on main's code; red with the karst row set to `[2, 255]` (gen 2 hash 1968489159290958858 against 12775723512097726191), green |
| `karst_is_made_only_in_limestone_provinces_and_only_from_generation_3` (+ no old worm at 3) | red ("0 karst systems on the 1536 planet"), green |
| `karst_is_cut_above_the_water_table_and_its_passages_lie_at_it` | red ("0 of 0 karst cells"), green |
| `karst_passages_are_near_level_and_each_system_is_walkable_from_a_sinkhole` (0.1 m a metre; flood fill over generated air from the throat's top reaches every cell and the lowest end) | red ("0 systems walked"), green |
| `karst_halls_reach_their_stated_size` | red ("0 halls"), green after halls along passages |
| `a_sinkhole_is_an_open_bowl_8_m_across_with_no_plant_in_it` (+ throat open to the passage) | red ("0 sinkholes"), green |
| `karst_systems_keep_a_wall_between_them` | red with the crowding off (-10.2 m), green |
| `a_cave_kind_opens_only_its_host_rock_in_its_provinces`, `a_cave_kind_keeps_clear_of_water` (ADR 0159's, now with karst) | red ("karst: 0 made"), green |
| `a_cave_is_the_same_whichever_chunk_asks_first...` (tube and karst) | red ("a Karst crossing a chunk border"), green |
| caves file test (karst row, 6 bad rows refused) | not seen red: the parser came before the test |
| server: save round-trip of generation 2 and 3 | red at `GENERATION` 2 ("this world is generation 3, this build knows up to 2"), green |
| server: welcome told 1/2/3, the client's caves the server's; replay of 0-3 | green; not red at `GENERATION` 2 (the server builds a world of generation 3 by argument), nor with karst moved to generation 2 (generation 3 still drops the old worm, so its caves differ anyway) |
| `cargo test --workspace` | green after the last code edit |
| `bash tools/views.sh` (play build) | ok: no cracks, diffs 0 to 0.26 %, no view shows a sinkhole (not accepted) |
| Pictures (seed 1, `time=270`, the lines above) | a sinkhole from 14 m up: a clearing in forest with a square-ish pit some 8-10 m across, stepped dirt sides and pale limestone at its foot; its throat from above: a stepped shaft through soil into limestone, sunlit floor; a passage with lamps every 3 m: flat floor, straight walls, a low flat-looking roof (the arch is only 3 layers), an orange-brown patch at the far end not identified; a hall (10 m, 6 m) with 8 lamps: a room with terraced, bedded limestone walls and a stepped ceiling, a passage leading off. The sky is magenta in the worktree: cause not found (the worktree holds every file main tracks; files main's checkout keeps untracked were not looked at; the stderr names no missing file) |

## Consequences
- New worlds are generation 3; saves and servers of 2 and older keep their ground.
- A basin's lowland has sinkholes in clearings; under them a near-level gallery in pale limestone, branches, rooms.
- Cost: the cave walk at load 0.7 -> 2.3 s on the 4 km planet, on the server and every client (`docs/perf-debt.md`).
- Not done: water in it (the next run), dripstone and rubble, marble karst, sinkholes without a passage under them
  (a doline over a filled throat), halls over the least size where rock is thin, karst across face seams, walls
  between karst and other kinds' caves (they are never in limestone, so they never share a cell, but a wall
  between them is not held to 3 m).
