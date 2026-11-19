# NNNN – A world's generation number; basalt flows on the arcs; round lava tubes; only copper near the spawn

- **Status:** proposed
- **Date:** 2026-11-18

## Context
ADR 0159 gave new worlds lava tubes, but seed 1 (4 km planet) had 7 of them, 762 m in all, the nearest 2 km from
the spawn: rift basalt is 0.6 % of the surface and lies in coastal lowland, and the volcanic arcs had no basalt in
`data/rocks.toml` (tuff over andesite), so no tube started there. A still from inside one showed a wide blocky hall
with a stepped roof. The developer chose "Basalt and tubes", with the standing taste that real-world logic decides
where things are and each thing is something a player finds and uses. Changing `rocks.toml` changes the rock of
every world, so ADR 0159's shortcut ("the kinds ride on the mantle flag") had to become a number first. Later the
same day the developer added: "Remove the need for having ore spawn close to spawn outside for copper ... the rest
is just procedural", widened to any landmark or generation: from this generation nothing is placed because of where
the spawn is, except copper.

## Options considered
1. **More flags beside `mantle`** – one bool per rule: a save field and a wire field per rule, and combinations
   nobody plays. Rejected.
2. **One generation number** (chosen) – a `u8` per world; a rule asks `gen >= k`. 0 the worlds before the mantle,
   1 the mantle's (ADR 0154, 0159), 2 this ADR's. The mantle itself is `gen >= 1`.
3. **Re-gate the mantle's rules on `gen`** – tidier, but it moves code that is locked by hashes for no gain. Left
   on the mantle flag (`b.mantle > 0`), which is the same set of worlds; the comments now point at `gen`.
4. For the flows: **a basalt layer on top of the arc's layers** (rejected: a layer is everywhere, and its top is
   the datum, not the ground, so on a 20 m high arc it is the whole hill or nothing); **flows as fields from the
   ground down** (chosen).
5. For the tube: **the old chain of stretched balls, tuned** (rejected: wide `r` with a squashed `h` is the flat
   stepped roof seen; overlapping walks are the hall); **a swept round passage of round balls** (chosen).

## Decision
**The generation number.** `Body::gen` (`world::GENERATION` = 2 for a new world), set on the planet in
`World::with_grid(.., gen)`, which takes it in place of the old `mantle: bool` (gen >= 1 puts the grid down
through the mantle). Carried:
- in the save: `Save::gen: u8`, `VERSION` 21. A 20's `mantle: bool` is the same one byte, so a 20 reads as
  generation 1 with the mantle and 0 without; a 19 or older reads as 0, as before.
- on the wire: `ToClient::Welcome { gen }` in place of `mantle`; the client and the bot build their world of it.
- refused when newer than the build: `world::known_generation` on a save read, `Server::from_save`, and the
  client's and the bot's welcome ("this world is generation N, this build knows up to M: update the game").
  `gen=` with `mantle=0` and a generation over 0: `gen=` wins, and a line says so.
- in recordings: `Ev::Gen(g)` after `Start` for a world from the seed of generation 2 or later (generation 1 still
  writes `Ev::Mantle`, so old recordings read as before); a replay takes it. One from a save: the save says it.
- by argument: `gen=k` on the game, the server and `report=1` (`server::gen_arg`; `mantle=0` is `gen=0`);
  `report=1` prints `gen`.
A future rule asks `b.gen >= 3` and bumps `GENERATION`; a cave row says the generations it is made in
(`generations = [least, most]` in `data/caves.toml`; a row's place in the file seeds it, so rows are appended and an
old row is given an end, never changed).

**Basalt flows (generation 2).** A province row may state `flow = { rock, share, thick, width }`
(`data/rocks.toml`, `rock::Flow`). The volcanic arc does: `basalt`, share 0.5, 30 m thick, fields some 700 m
across. Real arc volcanoes are andesite and tuff with basalt flows down their flanks (Lava Beds in the Cascades,
Etna); a flow drapes the ground, so it lies from the ground down, not from a datum. `Body::rock_at` puts the flow's
foot radius in `Rock::flow` (the map's ground there less `thick`) where a slow noise of the seed is over a cut that
leaves `share` of 4096 directions over the sphere (worked out once a map load); `Rocks::at` returns the flow's rock
over that radius. Every reader already asks `Rocks::at` through `Body::rock`, `col_rock` or the height tile
(ADR 0062): the mesher at every level, physics, the server, ore hosts and the ledger, the river beds' bare rock,
clutter's `over`. The fields are isotropic: stretching them downslope needs a per-column frame that breaks where
the slope turns, so it was left out.

What else keys on a province's rocks, checked: no mineral in `data/minerals.toml` is hosted in tuff, andesite or
basalt, so the arcs' ore bodies are where they were (their counts move a little: bodies that cross a flow are
basalt round the ore, and the later bodies of the kinds whose ring went, below); clutter's `over` names limestone and
marble only; gravel and rubble go by the block dug, so a basalt flow gives basalt's; the far view's colour is the
generated block, so flows show at a distance; plants go by biome, not rock. Fissures host basalt and andesite both,
so their cells in the arcs are the same in number (272 502 at generation 1 and 2).

**Round lava tubes (generation 2).** A new row of `data/caves.toml` (`generations = [2, 255]`; the old row is kept
as `[1, 1]`), walked by `caves::round_tube`:
- Cross-section: a chain of round balls a metre apart (`h = r`), each cut flat a third of its radius under its
  middle, so the lower third of the circle is filled: a flat floor, an arch over it, two thirds of the width tall.
  The width is drawn per tube (4 to 7 m) and waves by 12 % over some 250 m.
- Path: a start in the host first climbs the ground's slope (read 16 m either side: the flank, not a gully) while
  it and a width out each way are basalt at the tube's depth: a tube starts high on its flow. Then downhill as
  ADR 0159's, gently winding, floor never rising, roof 4.5 to 6 m down, and it turns off water seen 10 m ahead.
- No hall: a round tube ends where it would come within 3 m (wall to wall) of another (looked for as far as the
  widest tube kept so far needs, not only the next 8 m grid cells); a branch (0.3 per 100 m,
  after its first 25 m) leaves at 0.6 to 1 rad, bends on to its side over its first 80 m and is let near its
  parent for 25 m only.
- Ends: the water rule (no cell under or beside a wet column) and the host rule exactly as ADR 0159; the sea rule
  as ADR 0063's (ground under 3 m + radius over the sea; the old tube used its height).
- Skylights: as ADR 0159 (one every 0.7 to 1.3 of 200 m, a trench with chance 0.2), the shaft at least 2.3 m in
  radius, so at least 3 x 3 columns open at the ground; and from generation 2 no plant stands where its crown would
  hang over an open mouth (`Body::candidate_of`: every column of its crown's square asked, only where a cave is).

**Only copper near the spawn (generation 2; the developer's call, 2026-11-18).** `data/minerals.toml` keeps `near`
on copper gravel and deep copper; the rings of tin vein, bog iron, banded iron, coal seam and lead pocket are now
`near_before_2` (validated as `near`; a row has one or the other), which `ore::Kind::ring(gen)` applies below
generation 2 only. Searched for every other generation rule that reads the spawn (engine and server, not tests):
- forces or attracts toward it: only the ore rings, `ore.rs` `place` (the `ring` line, about 380). Gated.
- keeps things away from it (kept): `ore.rs` `place`'s `fits` (spawn keep-out, `SPAWN + a`, about 357);
  `caves.rs` `Probe::off` (64 columns, about 264); `caves.rs` `make` and `walk` (ADR 0063's worms, 100 and 40
  columns, about 697 and 752).
- the spawn point itself (not part of this): the bake's `spawn` (`bake/mod.rs` 139, `bake/luna.rs` 393) and
  `Body::with_map`'s clear spot (`world/body.rs` about 88).
- elsewhere, read only: `clutter.rs` 875, 951, 997 and `herds.rs` 483-796 read the spawn in tests only (flint
  within a walk, herds near it hold by density, not by a rule); `server/commands.rs` 59 and 452 (`/goto spawn`,
  the respawn point) and `phys.rs` 469 are play, not generation. Nothing else was found to pull toward the spawn.

## Numbers (drafts for the developer)
Seed 1, planet 6144 (`technoape-server report=1 seed=1 planet=6144`, `mantle=0` / `gen=1` / default):

| | gen 0 | gen 1 | gen 2 |
|---|---|---|---|
| made_hash | 0926f028ddcd85a7 (as main) | 37b2d1b1006b66f7 (as main) | a2ef250b3ed4203a |
| arc land under a flow | 0 | 0 | 0.491 |
| lava tubes | | 7, 762 m, median 104 m, 7 skylights, nearest 2,048 m | 98, 5,543 m, median 51 m, 98 openings, nearest 2,012 m |
| fissures | | 122, 11,282 m, 22 open | 122, 11,282 m, 22 open (open cells 272 502) |
| world load (report) | 0.5 s | 0.8 s (caves 0.31 s) | 1.2 s (caves 0.73 s) |

Targets asked: 30 tubes (98: met), 5 km (5.5: met), median 150 m (51: **not met**), a tube within 1.2 km of the
spawn (dropped by the developer's later call; it is 2 km, and there is no basalt land 6 m over the sea within 2 km
of the spawn on its face). Why the median is short: the arcs of seed 1 lie low, half their land under 8 m over the
sea, 90 % under 22 m, and a tube ends 3 m + its radius over the sea, so most run from the flow's high edge to the
coast in tens of metres. What did not help much: a gentler least slope (0.015 -> 0.008, kept), wider fields
(1,000 m), the flank's slope and turning off water (kept, +10 tubes). What would: higher arcs in the bake, or
letting the flows' fields favour the arcs' high ground (vents); both are calls for the developer.

Ores at generation 2 against generation 1, nearest body from the spawn (seed 1, 4 km): copper gravel 1,153 m
(both; the 150 m ring finds no stream there on this seed, as before), deep copper 1,287 (both), tin vein 849 ->
959, bog iron 309 -> 309, banded iron 2,316 -> 4,451, coal seam 680 -> 1,672, lead pocket 206 -> 206. Ledger
blocks against the budget (gen 1 -> gen 2): tin 14,673 -> 15,182 of 18,000, iron 788,924 -> 790,631 of 800,000,
coal 742,317 -> 800,097 of 1,100,000, aluminium 189,347 -> 189,297, lead 34,976 -> 34,993; copper, gold, uranium,
diamond unchanged. The ledger counts what is generated, so it adds up; coal and tin were short of the budget before.

## Checks
| Check | Result |
|---|---|
| `save::a_20_loads_as_generation_1_or_0_and_this_version_keeps_the_generation` | red (VERSION 21 refused a 20: "not a save of version 4 to 21"), green |
| `a_client_told_generation_1_or_2_builds_that_worlds_rock` (server: the welcome's gen, a world built of it) | red with the flow gate off (0 arc columns differ), green |
| `generations_0_and_1_generate_as_before` (1536 planet: caves, ores, rock of every 37th column, hash) | regression lock: numbers recorded on main's code before the change; cannot be red; green after |
| `flow_basalt_lies_only_in_provinces_that_state_it_and_only_from_generation_2` | red (0 of 8,103 arc columns flowed), green (share on the small planet 0.2-0.6) |
| `a_cave_kind_opens_only_its_host_rock_in_its_provinces` (ADR 0159's, now at generation 2) | green; tubes in arc flows asserted in the cross-section test |
| `a_round_tubes_cross_section_is_one_flat_floored_opening_at_most_7_m_wide` | red with the old walk (stepped roof: "arch false"), green |
| `round_tubes_do_not_run_side_by_side` (no run over 30 m within both radii + 1 m) | red with the old walk (67 m), green |
| `every_skylight_is_open_to_the_sky_over_3_by_3_columns_with_no_plant` | red with the old walk (crowns over 5 of 12), green |
| `a_recording_replays_with_its_generation` (0, 1, 2) | red ("a Start, Loaded, Herds or Mantle past the start"), green |
| `from_generation_2_only_copper_is_placed_near_the_spawn` (two spawns half a planet apart) | red (tin, iron, coal and lead moved with the spawn at generation 2), green |
| `two_widest_round_tubes_converging_keep_the_full_wall` (review) | red ("not stopped at 9.85 m from tube 0's axis (wall 2.85 m)": the 3 x 3 cells of 8 m missed it), green |
| `a_world_of_a_newer_generation_is_refused_with_update_the_game` (review) | red (a generation-3 save loaded), green |
| `cargo test --workspace` | green after the last code edit (see the review commit) |
| `bash tools/views.sh` (play build) | ok: no cracks, diffs 0 to 0.26 % (not accepted) |
| Pictures (seed 1, `time=270`) | from the surface the tube's mouth by the stand point is an open trench slot through grass, no plant over it; inside (lamps in the floor every 4 m) a flat dark basalt floor between straight walls under an open trench, the round arched part not in frame; looking up, the opening's rim; the arc flank from 120 m is forest and grass: the flows lie under the soil and do not show |
| `run=fly quick=1` gen 1 -> 2 | frame p99 6.66 -> 6.43 ms; chunk jobs after load 403 -> 478 us; `loaded in` 2.60 -> 3.33 s |

## Consequences
- New worlds are generation 2; saves of 20 and older keep their ground.
- The arcs show basalt flow fields among their tuff (under the soil, in cliffs, river banks and bare steep ground).
- A tube is a round road through the basalt; most on seed 1 are short.
- Cost: a map load 0.4 s more on the 4 km planet (the cave walk 0.31 -> 0.73 s: more starts, the climb, the
  crowding check; the game's load 2.60 -> 3.33 s), on the server and every client; chunk jobs after load
  403 -> 478 us (the flow noise per arc column); `docs/perf-debt.md`.
- Not done: flows stretched downslope; flows that favour high ground; tubes across face seams; the platform's
  float functions in the walks (ADR 0159); the median tube length target.
