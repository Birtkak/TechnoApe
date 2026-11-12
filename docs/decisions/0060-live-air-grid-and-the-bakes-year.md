# 0060 – The live air grid, and the bake's climate over a year with seasons

- **Status:** accepted (routine technical calls); the tilt, the seasons' strength and the biomes' look stay the
  developer's open taste calls (ADR 0055, 0043)
- **Date:** 2026-11-12

## Context
W3 (`docs/plans/worldgen-plan.md`) had the climate and fifteen biomes baked (ADR 0042, 0043) and the sky's
tilt and seasons (ADR 0055). It still lacked five things:

- the map keeping each cell's baked temperature;
- the air grid's tick spike (ADR 0010);
- the live grid on the server (`docs/design/air.md`);
- the rain check (the bake against a year of live weather);
- views at four seasons.

The bake's air had its own tilt-free sun and a 20 minute day (ADR 0055 left it so), while the sky has a 30
minute day and a 54-day year. Two models of one planet cannot agree on rain.

## Options considered
1. **The bake runs a real year with the sky's sun, at the same step as the live grid** (chosen). It costs
   seconds of bake, measured in `prototypes/air` (README, "The live grid's tick").
2. **The bake keeps a tilt-free day.** Rejected: against a seasonal year it is off by 0.67 K a column and
   by 18 % of rain at 60 to 70 degrees.
3. **The bake runs four held days of the year (fixed declinations) and averages them.** Rejected: the high
   latitudes need about 12 days each to settle (snow and soil water carry across seasons), so it costs what
   the year costs (11.5 s against 12.7) and is still 3 % off.
4. **A live step of 5 s or 1 s.** Rejected: at 5 s it rained 10 to 20 % less in every biome than the bake's
   10 s year (the rain check, seen failing). The same step on both sides makes the live year the bake's
   (0.1 %) and halves the live CPU.

## Decision
- **The spike** (`prototypes/air`, `mode=tick` and `mode=year`): a live step on Terra costs 4.43 ms on one
  thread and 1.27 ms on 12. On the 8 km planet it is 17.5 and 4.4 ms. Handing the state to a worker and
  taking it back costs the server's thread under 10 µs.
- **The bake's air** (`bake::air`): the sun is `Sky::sun_dir` with the sky's tilt. `Air::year` runs 10 days
  held at the spring equinox, then one year of 10 s steps from world time 0, and returns the year's means
  (potential temperature, the day's swing, rain). It ends at the year's turn, so its last state is the
  weather at world time 0.
- **The map, version 3** (`bake::Climate`): per air column (128 m), the ground and water share, the year's
  three means, and the grid's state at the year's end.
  - A cell's baked temperature is `Map::temp`. It applies the bake's own smoothing (`land::upsample_at`,
    which `land::upsample` now calls), less the lapse by the cell's height. The test checks that it equals
    `life::Life::temp` at every cell that is not a lake. It failed with the lapse left out.
  - Version 2 maps are read as they were, with no climate, and give back their own bytes, so their hash
    holds. A climate whose state does not fit its grid, or holds NaN or impossible amounts, is refused.
  - F3 shows the live air where the server sends it. Otherwise it shows the map's mean with ADR 0055's
    season and hour terms, or the biome's band on an older map.
- **The live grid** (`engine::weather::Weather`, on `Server::weather` when the map has a climate):
  - One step is `DT` = 10 s of world time, so a step comes every 200 ticks.
  - When the grid is a step behind the clock, a worker thread takes it for up to 8 steps. The server takes
    it back `LAG` = 20 ticks later, and waits if the worker is not done.
  - A job's work is fixed when it is handed over, and what the game reads changes only on the tick it comes
    back. Thread timing never shows: a save between hand-over and return changes nothing (a test, seen
    failing with an early show).
  - Under a time warp it keeps up to 80x, then jumps. A clock set back moves the grid's clock with it (a
    test, seen failing).
  - Its view per column: the surface air, rain, cloud cover, wind, and the runoff to rivers over the job.
    The view's hash is in `Server::hash`, so the soak and the replay check the weather.
  - The save, version 8, keeps the grid's state. Versions 4 to 7 load as before, and their air starts from
    the map's.
  - Each player gets `ToClient::Air(Here)` every 40 ticks: the air at its feet, about 24 bytes. Bots report
    it too.
- **Runoff** for W5: `View::runoff` counts water units per air column over the last job, and
  `Weather::column(dir)` finds a cell's column. The water is booked in the air's own ledger (its `sea`
  term). Joining that ledger with the liquid's is W5's call (shared note `w3-runoff-for-w5.md`).
- **The rain check** (`weather::tests`): the bake's mean rain per biome against the next year of live
  weather, each smoothed to the map's cells the same way. The tolerance is `RAIN_TOLERANCE`, 5 %.
  - The tests' 256-column planet is within 0.1 %.
  - Terra (seed 1337, `cargo test --release -p engine -- --ignored terra_rain`) is within 1.0 % in every
    biome. Desert is +1.0 %, dry scrub +0.9 %, the rest under 0.5 %.
  - The test failed at the 5 s step and tolerance 0.
- **Tests run the engine at opt-level 2** (`[profile.test.package.engine]`). A small planet's bake now runs
  a year: the bake tests took 2.5 minutes unoptimised and take 31 s now. The floats give the same bits.
- **Views:** `season-spring`, `-summer`, `-autumn`, `-winter` look south from the spawn's forest (about 30
  degrees north) at noon on the four seasons' days. The shadows shorten toward summer and lengthen toward
  winter. They are not accepted (no baselines).

## Measured (`docs/benchmarks.md`, 2026-11-12)
- **Bake** of Terra on 8 threads: 35.9 s, up from 25.0. The map is 19.2 MB, up from 17.8.
- **Live air on the CPU:**
  - 1.4 ms on its worker (all threads) every 10 s of world time;
  - 0.022 ms a tick on one thread, against the 0.5 ms target;
  - under 10 µs on the server's own thread.
- **The 8 km planet:** 0.09 ms a tick on one thread at this step.
- **Soak:** seed 1 gives `eca98f7116931379` twice. The gate's 3-minute soak and its replay agree:
  `b774f156722fc5ac`, at all 180 checks.
- **The spawn's live air at noon** (Terra, 1337): spring 13.4 C, summer 15.9 C, autumn 14.0 C, winter
  10.6 C. That is about ±3 K, milder than ADR 0055's F3 formula (±7 K at 30 degrees).

## Review (CLAUDE.md principle 12)
A fresh subagent read the engine and server diff. It found no nondeterminism, deadlock or misread format.
It checked the job scheme, the save versions 4 to 8, the map versions 1 to 3, the bit-identity of
`upsample_at`, the time warp and the cost on the wire. Its findings, and what became of each:

- **A clock set back froze the weather** until the clock passed the grid's again. Fixed, with a test.
- **Trust boundary:**
  - `Climate::check` computed sizes before checking `m`'s range. Fixed: the range comes first.
  - `Air::restore` took any amount of water. Fixed: over 2^28 units a cell is refused, and the view sums
    cloud in u64.
  - `Map::temp` underflowed on an empty map. Fixed.
- **Old saves on a v1 or v2 map need their bake file.** The bake's climate changed, so a fresh bake no
  longer gives the old map's hash. While the file is in `saves/bakes/`, which a bake never deletes, such a
  save loads. Without it, the game stops, as for any other map that comes out different. Recorded here and
  in `save.rs`; accepted.
- **After a save loads, the first view shows no rain or runoff** for up to one step (10 s). Cosmetic;
  nothing compares that hash with the server's that kept running. Left.
- **Recordings made before this change** (`out/rec`) can misread their `Loaded` save. Recordings are
  per-session. Left.

## Consequences
- **The biomes moved a little** with the seasonal climate. The old fixed views differ from their baselines
  by 5 to 59 % of pixels (seam, corner, lod, high, orbit); coast is unchanged. They wait for the developer,
  with the four season views.
- **Merging with W5:** W5's branch also writes save version 8, with another layout. Whichever merges
  second takes version 9 and reads the other's 8. W5 then wires runoff into its reaches (`Liquids::rain`),
  and the units it hands over leave the air's `sea` term, so the two ledgers stay exact together.
- **Survival's cold, the snow line by season, crops and solar power** can read `Weather::here`.
- **Not in W3** (the roadmap's next weather step):
  - clouds, rain particles, storms, lightning and fog;
  - deltas of the grid to clients;
  - rooms, fires and chimneys;
  - rain on awake chunks;
  - air.md's yearly swing, rain per season and snow days kept by the bake.
- **Open taste calls for the developer:**
  - the tilt (20 degrees);
  - how strong the seasons are (the live air gives about ±3 K at the spawn, `SEASON_K` ±7 K in F3's
    fallback);
  - each biome's look after the shift.
