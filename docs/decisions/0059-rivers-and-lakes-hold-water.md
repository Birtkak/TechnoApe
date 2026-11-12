# 0059 – Rivers and lakes hold water

- **Status:** accepted (routine technical calls; how a river and a waterfall look is open for the developer)
- **Date:** 2026-11-12

## Context
Worldgen plan W5. The bake leaves river lines and dry beds (ADR 0043, 0056), and lakes are flats with no
water. `docs/design/rivers-and-tides.md` asks for a reach at rest described by the bake, live water only
where disturbed, and a ledger across the hand-over. ADR 0020's sea is the model: generated still water
costs nothing, only disturbed water is simulated, and the soak checks the count every tick. The developer
decided on 2026-11-12: rivers and lakes first, tides later. Until W3's air grid runs live, a reach's runoff
is the bake's mean rain over its catchment.

## Options considered
1. **Simulate every river all the time.** Exact flow everywhere, but every wet column is a live node: tens
   of thousands on the 4 km planet, every step, for water nobody touches.
2. **Generated water with a moving surface (the sea's way, extended).** Generation gives each river column
   a smooth level and a flow. Touched columns go live, with the river's own friction, so live water carries
   the reach's flow at the generated level. Once a body is back within a dead band of generation it is
   forgotten, moving or not. Open ends trade with the reach's account.
3. **Reaches as tanks only, no columns.** Cheap, but a dam would have nothing to hold back, and a bucket
   could not go into a river.

## Decision
Option 2. The spike (`prototypes/river-reach/README.md`) answered the design's five questions with numbers:
- A bucket in a river is forgotten after 3.5–4.5 s, at slopes of 0.005 to 0.1.
- A touched waterfall is forgotten after 7 s.
- A dam's pond stands 1.7 blocks high. Broken, its wave is live for 214 s, at about 0.1 ms a tick.
- A pillar in the river keeps one column and no work.
- A wheel's flow read costs 1.8 ns.
- The graph of 4,000 reaches ticks in 15 µs.

**The map, version 3** (`bake::Map`):
- It keeps the lakes (cell, level) and each river cell's flow. The ground under a lake is its bed.
- Maps of versions 2 and 1 still read, with dry beds; `for_hash` rebuilds them from a fresh bake.
- `Lines` adds per segment its level at each end, its fill, its flow and its reach. The level never falls
  under the lake or sea it runs into, and never rises downstream.
- Fill: the bed's depth × min(0.7 · (runoff ratio)^0.3, 0.85), dry under 0.15 m.
- Flow: Manning's v = fill^(2/3) · √slope / 0.035, clamped to 0.3–2 m/s.
- Reaches are cut at springs, confluences and lake outlets, plus one reach per lake.
- A reach holds what it takes in for its length over its speed; a lake holds it 600 s.
- `Map::water` gives a point's river or lake water.

**Generation** (`Body::water_at`, the liquid's `Ground`):
- Each column gets its water top, its account (a reach, or `SEA`), and for a river its flow per column of
  width, its fall and its lateral runoff.
- The sea wins wherever it stands higher.
- No plant grows in a river or a lake.

**Live water** (`liquid.rs`, from the spike):
- A river column's friction (`drag_for`) is chosen so that steady live flow equals generation.
- A woken river column starts at the steady speed.
- Open ends are booked to the reach's account (`reach_extra`):
  - upstream, the river pours in its flow;
  - downstream, it holds the generated level;
  - a lake holds its level.
- Every 15 steps a whole body within U/64 of generation is forgotten. So is any body that fell asleep since
  the last pass (`slept`): before, a body that slept unmatched stayed stored for good.

**The reaches' water** (`run_reaches`, once a second):
- Each account passes on extra/hold, at least a unit a second, to the reach below and finally to the sea.
- `Liquids::rain(reach, units)` is the entry point for W3's live runoff, booked as `rained`.
- The count is `counted() - rained`. Saves (version 9) keep `rained` and every reach's account.
- `reach_flow` is a wheel's read: one lookup.

**Drawing** (`world::river_quads`, `vs_sea`, `fs_water`):
- A chunk that holds a wet column's level gets a quad of kind 5 at that level: a plane through the column's
  middle with its fall along i and j (18 + 7 + 7 bits in q.y).
- A lake's columns are level, so they merge as the sea's do.
- Water in wet tiles is the client's water mesh's, which reads the same `generated`.
- The fragment shader runs ripples downstream along the fall, at 0.5–3 m/s by steepness, and whitens rapids.
- None past chunk level 6.
- Cost:
  - A chunk away from the ground (by its height tile) costs 0.1 µs.
  - Lookups are limited to cells near water. `Lines::wet` is a mask of cells within a wet channel's half
    width (sampled every quarter cell) and lake cells with their ring. `Map::water` returns early outside
    it, which every caller gains from, not just the drawing. `Map::wet_near` checks a chunk's footprint.
  - A chunk that holds a river's level costs 113 µs on average: 173 µs at level 0, 78 µs at level 3.
    Before the mask it was 216 µs.

**Physics:** in water, jump held with a one-block ledge ahead climbs out. A river's bed steps a block under
0.7 m of water, where swimming up never lifted the feet over the step.

**The soak:** bots dam rivers (a creative plug of stone across the channel, dug out 2–6 s later), on the
smallest planet with wet rivers (1536).

## Consequences
- **Measured** — see the table below.
- **Taste calls open:** how a river, a lake and the rapids look (the filmstrips); whether real waterfalls
  (a drop of a block or more) should exist. The smooth generation has none: steep reaches are rapids.
- **Shortcuts:**
  - Live rain is not wired in yet. Once W3 merges, the units W3 hands to `rain()` leave the air ledger's
    `sea` term, so the two ledgers together stay exact.
  - The gate's soak is still on planet 256, which has no rivers. The dam soak is a cargo test on 1536.
  - A sloped river quad steeper than 0.25 layer a column is drawn in steps.
  - Lake outlets can stay live (the spike's open question).
- **Save versions:** W3 (ADR 0060) merges first and owns version 8: version 7's layout plus a trailing
  `air`. This branch writes 9 and reads W3's 8 through the layout of 7: postcard reads no further than the
  type, so the air is left unread and starts again from the map's climate, as W3 does for a 7 (test
  `a_version_8_from_the_air_branch_loads`). The merge must make 9 W3's 8 plus `reaches` after `air`, read 8
  with W3's own type and keep the air, and keep the test.
- **Tests seen red first:** the liquid tests (the spike stands as their red), the reach drain, the reach
  holds, the save's reaches, the soak's dams, the quads, the plants and the walker. Two were written
  alongside their code and not seen failing: the map v3 test and the save test (a compile error only).
- **Review:** see the table below.

| | |
|---|---|
| Benchmark (quick, dev PC) | Mesh job average: `stand` 309–312 µs (baseline 294), `fly` 296–302 µs (baseline 273). The rest is on worker threads; `budget_ok` true, no spikes; GPU p50 unchanged (1.40 / 1.61 ms). |
| Reach tick | A second of steps with all 298 reaches of the 4 km planet holding water: 34–47 µs (`run_reaches` once, plus 60 empty steps). The spike's graph of 4,000 reaches ticked in 15 µs. |
| Soak | Planet 1536, 4 bots, 900 ticks: 7 dams built, 6 broken, the ledger exact every tick, the same hash twice. |
| Review | A fresh reviewer read the engine, server and shader diff. It found no unit appearing or vanishing and no determinism problem. Fixed, each with a test seen red first:<br>(1) `touch_tile` re-meshed only the sea's chunks, so a river patch going live or forgotten drew two surfaces or a hole. Now `world::water_keys` covers every level in the tile.<br>(2) A pit dug in a river's bed kept 11 columns awake forever. A dug column now matches at the river's level and stays stored, asleep.<br>(3) Restored river patches loaded frozen; they are woken.<br>(4) Per-step lookups: `reach` and `river` are cached like `generated`; the runoff books only when it adds; `water_at` checks the wet mask before any geometry.<br>Answered: `forget_matching` also walks sea bodies; limiting it to river nodes broke three sea tests, so it stays (at most `settle`'s cost again, 4 times a second). The bucket in a river refills from the reach's account, as generated water is free. Dam bots stay creative for the rest of a soak; the 256 soak keeps its survival bots. |
