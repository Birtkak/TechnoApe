# 0070 – Luna's bake

- **Status:** accepted (W6 of `docs/plans/worldgen-plan.md`). The developer took the three recommendations
  (2026-11-12): ice under the dust in polar shadow, Luna from orbit as it is, and a brighter Luna in Terra's
  night sky as a follow-up, not built here (end of this ADR).
- **Date:** 2026-11-12

Numbered 0070, not 0063: W4's session (rock, caves, ore) runs at the same time and takes the numbers from 0063 on.

## Context
Luna stood on noise hills. Worldgen.md's Luna table asks for a short pipeline of its own: old pale crust, giant
basins flooded with basalt on the side facing Terra (the maria), craters by a power law and by age, dust by
age, lava tubes, ores Terra lacks, and the polar shadow computed rather than assumed.

**Size and orbit (the plan's first step) were already done** by ADR 0041: Luna is 3.7 times smaller than Terra
(1664 columns under the standard 6144, 1150 m in radius) and 60 Terra radii out, so every total eclipse just
covers the sun. The plan's older proposal (N = 3072, an orbit of 30 km) predates that; **the ADR and
`data/bodies.toml` were followed**. The `orbit` benchmark's route is laid out in Terra radii and already flies to
this Luna.

## Options considered
1. **A map of its own type, with its own save, download and protocol.** Clean, but it doubles code that already works.
2. **The same `bake::Map`, under a version number of its own** (Terra's maps count 1 to 5; Luna's from 101, now 102). The
   file name, the in-memory cache and the hash all key on the version, so the two never meet. The mips, the
   spline, the roughness, the save-by-hash and the download (`fetch_map`) all work unchanged. Luna's kinds of
   ground become rows of `data/biomes.toml` after Terra's 15, so `world.rs` draws them with no new code.
3. Luna generated on the fly from the seed, with no map. Cheap, but the server (Linux) and the clients
   (Windows) could disagree on a column through transcendental functions; the map is what keeps them equal.

## Decision
Option 2. `crates/engine/src/bake/luna.rs`:

- **Cells of 2 m** (Terra's are 8), so a crater 6 m across is three cells. 832 cells per face edge; the map is
  20.8 MB (Terra's 18).
- **Crust:** noise of ±8 m around 40 m over the world's datum.
- **Basins:** four 0.5 to 0.9 Luna radii wide, within about 45° of body -x (the face toward Terra, sky.rs
  `moon`), and two anywhere. They are shallow for their width: 2.5 % of the diameter, worn as the oldest.
- **Craters:** N(>D) = K A / D² with K = 0.2, from 6 m to 0.13 radii (150 m), each with an age `t` (0 the oldest
  crust, 1 now). They are stamped oldest first:
  - a parabolic bowl, depth 0.15 D, capped at 3 (D/20)^0.45 m;
  - from 30 to 80 m, a flat floor;
  - from 120 m, terraces; from 100 m, a central peak;
  - debris falling as x⁻³ to 3 radii;
  - older ones shallower and lower-rimmed (sharpness 0.35 + 0.65 t).
- **The maria flood at t = 0.35:** each basin facing Terra fills with basalt to half its rim's height over its
  floor, with ±0.4 m of wrinkles. Craters older than that are drowned or left as ghost rims.
- **Dust is in the kind of ground:**
  - highland: 4 m of regolith on anorthosite;
  - mare: 1 m of dark regolith on mare basalt;
  - craters younger than t = 0.96 have bright fresh dust to 1.3 radii, 1 m on the rock under it;
  - craters over 30 m and younger than t = 0.995 throw 12 rays up to 8 radii.
- **Ores** are what each rock is, with the counted ore bodies left to W4:
  - mare basalt: ilmenite (titanium, oxygen);
  - anorthosite: aluminium;
  - 1 % of craters over 15 m are iron impactors: a 3 m lens of iron-nickel under their floor;
  - a KREEP (thorium) province: a sector of highland round the largest mare;
  - helium-3 in the old highland dust.
- **Lava tubes:** in each mare, one or two wandering chains of balls:
  - radius 3.5 to 5.5 m, roofs 5 to 9 m thick;
  - two pits each where the roof fell in (the ball breaks the surface);
  - kept off face seams;
  - stored in the map as (cell, depth, radius) and made `Body::caves` (ADR 0062) when the world is built.
- **Polar shadow:**
  - computed on 8 m cells within 20° of either pole;
  - the sun's highest is the orbit's 2° tilt plus the disc's 0.25°;
  - a cell is shadowed when 32 rays toward that sun, marched 700 m over the ground, are all blocked;
  - shadowed cells become the kind `polar shadow`; 1 m of regolith on ice, the ice to 4 m down (its band), then anorthosite.
- **Blocks** 200 to 205 (anorthosite, mare regolith, mare basalt, fresh regolith, iron-nickel, kreep), from 200
  so W4's rocks can take the ids after 29.
- **Plumbing:**
  - `World::with_maps` and `World::baked` stand Luna on its map with no sea; `World::with_map` keeps the hills.
  - `ToClient::Welcome` carries `luna`, and `ToServer::Map(hash)` asks for either map.
  - Saves are version 11, with `luna`; a 10 (W4's) or older keeps Luna on its hills, so its edits there stay on
    their ground.
  - The new-world page bakes both maps.
  - `run=stand` takes `body=1` and `look=terra|luna`, for the views.

## Checks
| Check | Result |
|---|---|
| Crater power law (`luna_report`, the standard Luna, seed 5) | made N(>D) for D = 6, 12, 24, 48, 96 m: 92164, 22864, 5644, 1314, 205, slope -2.04 against the stated -2 (the cut at 150 m bends the top bin); still seen: 68000, 17902, 4700, 1155, 196, slope -1.96 (the smallest hide under younger craters and maria) |
| Kinds, km² of 16.6 | highland 12.9, mare 2.0, fresh highland 1.2, fresh mare 0.21, kreep 0.25, metal 0.017, polar shadow 0.009 (123 cells of 8 m, out to 20° from the poles); 7 tubes, 289 balls, 14 pits |
| Bake time | 2.0 to 2.5 s for the standard Luna (8 threads for the crust and the shadow; the craters are one thread); 0.2 s for a 416 Luna |
| Tests (red first where new) | `craters_follow_the_power_law_and_the_small_ones_saturate` (red: it expected the uncut law on a small Luna), `maria_face_terra_and_every_kind_is_there` (red: 10 % maria on Terra's side), `a_polar_bowl_is_in_shadow_and_flat_ground_is_not`, `luna_stands_on_its_bake` (red: a roof check off by one), `a_bake_is_the_same_twice_and_reads_back_without_touching_terras_versions`, `a_version_9_or_10_keeps_luna_on_its_hills`, the polar-shadow row's ice in `biomes.rs` (red before the data line); `cargo test --workspace` green |
| `run=orbit quick=1` | holes 0 in every run; frame p99 4.4 to 5.2 ms; budget_ok in some runs, the others failed on GPU waits while another session used the GPU (below) |
| Gate | `tools/check.sh`, 2026-11-12, run while four other sessions' cargo builds ran: tests, soak (hash 0ce6987bd250b9c8) and replay pass, and every view has 0 cracks. It fails on: (1) the four season views, 4.2 % of pixels off their baselines, the same with main's binary (6e78189), so the baselines predate the season looks and this branch does not change them; (2) the speed guard on every run, mesh jobs 2 to 3 times the baseline on Terra-only runs too, against about 260 us for both binaries when timed in turn; (3) feel, 2 corrections at t = 0. Run again on a quieter PC, both binaries have none, and jerk p99 is 25.6k / 25.5k new and 29.3k / 25.6k old. The gate needs a run on a quiet PC before the merge. |
| Views | `out/luna/`: low Luna orbit (the maria toward Terra, a rayed crater), Terra at full moon (fov 70 and 30), the north pole, the map sheet `map.png`. Not added to `tests/views/` until the developer has seen them |

**CPU cost:**
- 2 to 2.5 s more when a new world is made, once per seed.
- Loading a Luna map (mips and roughness) on a worker: not timed alone.
- On Luna, `Body::col_caves` scans up to about 300 balls a call, on a worker for meshing and on the frame thread
  for physics (bucketing caves is W4's planned change, ADR 0062).
- Terra is untouched: its map, hash and frame are the same.
- Measured: `run=orbit quick=1`, main's binary (6e78189) and this one in turn, three runs each, means:

| | old | new |
|---|---|---|
| frame thread CPU p50 / p99, ms | 1.88 / 4.67 | 1.75 / 4.60 |
| frame_cores | 0.905 | 0.875 |
| mesh job, us (all / after load) | 242 / 199 | 266 / 218 |
| worker_cores | 0.179 | 0.185 |
| RAM, MB | 474 | 531 |

  Meshing Luna's cratered ground costs about 10 % more per job than its hills, and the map adds 57 MB of RAM. The
  frame thread is no slower. Some runs of both binaries failed the budget on frames over 16.7 ms (old: one of
  18.8 ms; new: 24, 83 and 144 ms). Read again, every such frame waited in `acquire`, on the GPU (15 to 220
  ms), with 1 to 7 ms of its own CPU work, 3 to 7 s into the run, over Terra before the flight reaches Luna.
  Another session had the GPU 13 to 42 % busy. These are not Luna's frames; the gate's run on a quiet PC is
  the check.

## Review
A fresh reviewer read the diff. No serious bugs: Terra's bytes and hash, the save layouts, Welcome, the
download, the bots, the soak and the replay agree. Its findings, and what was done:

1. A new world whose Terra map was already kept skipped the bar and baked Luna silently. **Fixed:** the page bakes
   when either map is missing. (Luna has no stage of its own on the bar; 2 s.)
2. `col_caves` scans every ball, and tubes crossed face seams, where caves are cut. **Fixed:** tubes stop short of
   seams. **Left:** bucketing caves is W4's planned change (ADR 0062), and the cost is measured below.
3. A Luna under about 64 columns had no craters, so nothing flooded. **Fixed:** the maria flood after the craters
   if no crater came after their time.
4. Roofs were measured from the cell's height, while the drawn ground has up to 3 m of detail. **Fixed:** roofs
   are 5 to 9 m.
5. A recording that starts from a seed names Terra's map only. **Left, a `shortcut:`:** it replays on this build's
   Luna, so one made before W6 replays on other ground there.
6. Weak tests. **Fixed:** a polar-shadow test, and the dust check reads a cell whose 9 x 9 neighbours are all one
   kind. **Left:** a golden hash of a small Terra map; Terra's hash is still guarded by the soak's map and the old
   map files reading back.

## Merged with W4 (rock, caves, ore; ADR 0063)
- W4's map (version 5) gained a `rock` field. Luna's maps carry it empty and became version 102, so a 101 file
  is baked again; the check that a map's rock provinces are there skips Luna's version.
- Luna's tubes go into `Body::caves` through W4's `set_caves`, so they are bucketed per tile like Terra's caves
  (the reviewer's finding 2 is settled). Luna gets none of W4's caves or ore bodies: its `rock` is empty.
- W4's blocks take ids 30 to 56; Luna's stay 200 to 205, under the 255 limit, with no name shared (W4's
  `basalt` is Terra's, Luna's is `mare basalt`).
- W4's save is version 10; Luna's became 11 and reads 10 and 9 with the same layout. This branch's builds before
  the merge also wrote a 10, with `luna` after it and on the old crust; read as W4's 10 it would put Luna's edits
  on the hills and skip the crust's shift. A 10 or 9 must now leave no bytes over, so such a save is refused
  (the merge's reviewer found it; no such save was handed over).
- The merge's review also found, in main's code, that `bake::for_hash` rebuilds the map layouts 1 to 3 but not 4,
  so a save naming a deleted version 4 map stopped the server. **Fixed after the merge:** it rebuilds 4 too (this
  bake without its rock). `every_version_a_save_names_is_baked_back_without_its_file` (red on 4 first) bakes
  back 5, 4, 3, 2, 1 and Luna's 102 by their hashes with no file on disk. Luna's 101 is not rebuilt: no save
  that names Luna's map reads with a 101 (the pre-merge 10s are refused).

## A brighter Luna in Terra's night sky (the developer's follow-up, taste call 2)
Luna is real ground lit by the sun, 0.65 white times its albedo, with no exposure that adapts to the night: a
full moon came out at a luma of about 125 of 255, grey. `shader.wgsl` `moon_gain` multiplies the sunlight on
Luna (the copper of a lunar eclipse with it, so an eclipse keeps its ratio to a full moon) by
**`MOON_NIGHT_GAIN` = 2.6**, the developer's to tune, only when the camera is in another body's air and that
body's night (faded in by the sun's height from 0.05 to -0.1). By day, from space, in orbit round Luna and
standing on it the gain is 1. Nothing else changes: Luna's size, its phases, the moonlight on Terra's ground
(the CPU's `moon_col`), Terra's own light (its bounce rays never meet Luna).

| Check | Result |
|---|---|
| Full moon from Terra, fov 30 (`out/moon/fullmoon-fov30-*.png`) | brightest 40 pixels: luma 125 old, 242 new; the highlands read white, the maria still grey |
| A lunar month at one hour (`out/moon/lunar-month.png`, old over new, a day a frame) | the phases keep their shape; luma 115 to 139 old, 231 to 252 new on the 7 nights it is up; the other 9 days, ground, the same |
| Luna from its low orbit, and Terra from Luna's ground | the same as main's code: 0.31 % and 0.0 % of pixels differ by more than 8 of 255, as two runs of one binary do (0.42 %); mean luma 32.02 both |
| GPU, `quick=1`, three runs each, old and new in turn: `stand` / `stand` at a full moon / `orbit` | p50 1.305 / 1.190 / 1.461 ms old, 1.318 / 1.184 / 1.397 new; p99 the same to 0.01 ms. One uniform branch and a smoothstep per sunlit pixel; no CPU change |
| Review | a fresh reviewer: no defect. Notes left: the ambient term and the metal glint are not raised, so Luna's contrast grows a little; a bright rock could clip to white (the post pass's shoulder rolls it off) |

## Consequences
- W4's ore bodies and material ledger should count Luna's iron lenses and its KREEP province the same way.
- `Ev::Start` should name Luna's map when recordings next change.
- The prototype rule (a throwaway first) was met by the bake's own `luna_report` and its sheet, at about 300
  lines; no separate prototype folder.

## Taste calls (decided 2026-11-12: all three as recommended; 1 is in the data, 2's brightening is a follow-up)
1. **Luna's ice.** Shadow exists: 0.009 km² on the default seed, in crater floors near both poles. **Recommended:** ice
   blocks under 1 m of dust in the `polar shadow` kind (its band); one data line. The alternative is no ice.
2. **Luna from Terra's surface.** At its decided size Luna is 0.53° wide: about 12 pixels across at fov 30 (1280 wide), half that at the
   default 70. The maria show, faintly, as a grey mottle. It is grey rather than bright, because the night's
   exposure treats it as ground. **Recommended:** keep the size (ADR 0041), and brighten Luna in Terra's night
   sky so a full moon reads as one; the shader is the client's, so that is a follow-up.
3. **Luna from orbit.** The dark maria, the pale highlands and the bright rays read as a moon. The maria's edges
   are near circles with blocky shores. **Recommended:** accept as is; ragged shores would come from more
   crust noise at the basins' rims.
