# 0067 – Tides: a sea level that follows Luna

- **Status:** accepted (W5's second half, `docs/plans/worldgen-plan.md`). The developer's calls (2026-11-12): 1 m from
  low to high at spring tides, following Luna. The rest are routine defaults, recorded here.
- **Date:** 2026-11-12

## Context
The sea was one level (ADR 0020): every column under it holds generated water, stored nowhere, and touched water
trades with the open sea at that level, booked to `sea_extra`. The design (`docs/design/rivers-and-tides.md`,
"Tides") makes the level a pure function of place and time, as the sky's poses are, so the tide costs no storage
and needs no message. The spike is `prototypes/tides/README.md`; it was small enough to be this change.

## Options considered
1. **A pure function, asked where the level is needed** (the design). Nothing stored, nothing sent: the server, the
   physics, the water and the shader each evaluate it from the world time.
2. **The sea as live water everywhere, driven by the tide's force.** Real currents, but every ocean column stored
   and stepped: millions of nodes. Against ADR 0020's rule.
3. **One global level that rises and falls** (no bulge). Cheaper still, but no lunar day, no springs and neaps by
   place, and the volume would not be kept.

## Decision
Option 1.

**The function** (`sky.rs`, `Sky::tide` and `Tide::at`):
`tide(up, t) = 1.5 (A_L (up . luna)^2 + A_S (up . sun)^2) + base`, Luna's and the sun's directions in Terra's body
frame from the sky's poses. A_L = 0.5 m, A_S = A_L / 3 (the design's proposal), so low to high at springs is
1.5 (A_L + A_S) = 1 m, at neaps half that. `base` = -1.5 (A_L l^T M l + A_S s^T M s), where M is the mean of
d d^T over the ocean's directions (`Body::ocean`, 4096 even directions whose ground is under the sea, at load):
the tide's mean over the actual ocean is then exactly 0 (tested on a lopsided ocean to 1e-9 m).
- **The water is not created or destroyed.** The bulge under Luna is the water of the low ring 90 degrees off it;
  `base` keeps the generated sea's volume. Stored water still trades with the open sea only at the stored edge,
  booked to `sea_extra` as before, and a column forgotten books what it holds against what generation gave it when
  it was stored (`gen`), so `counted()` stays exact whatever the tide did meanwhile.
- **Two highs a lunar day** (1920 s): 16 minutes apart (the design's 10.6 predates ADR 0041's month and day).
  Luna's orbit leans 2 degrees (ADR 0041), so the two daily highs differ mostly by latitude and the sun's season.
- At the filmed beach (31 degrees N) springs run -0.36 to +0.64 m, neaps about 0.6 m low to high.

**Where it is asked:**
- **Generation** (`Body::water_tide`, through `world::Solid`): the sea's level in a column is the mean plus the tide
  there. The tidal band (columns whose ground's top is the layer over the mean sea's) goes wet and dry with no
  storage. A river's last columns are the sea's while the sea stands over them (the old rule of `water_at`): river
  mouths become estuaries at high water and rivers again at low, with no new code.
- **Taken once a second.** `Solid::at` takes the tide at the whole second of world time: generation's water steps
  once a second, at most 3 mm. The water's caches (`gen_at`, `river_at`) hold for that second; `Liquids::step`
  sees the second change (`Ground::tide_epoch`) and runs `retide`: the caches are dropped, and stored water beside
  the open sea whose level moved a dead band (1/64 block) or more is woken, so its open end lets the sea in or out.
  A loaded save starts at its own second (`restore`), so a loaded world steps as the running one did.
- **Physics:** `World::water_surface` asks the same function, so swimming and the camera's underwater test follow.
- **The sea's quads** are meshed once, for the highest tide (`sky::TIDE_HIGH`, 1 m over the mean, which
  `Tide::at` never passes as `base` is never positive): at level 0 the cell over the mean's, so the tidal band has
  water quads; coarser chunks keep the mean's cell and its solid test for the shallows. The shader (`vs_sea`) lays
  each vertex at the mean plus the tide, from Luna's and the sun's directions in `Globals::tide`; where the ground
  stands higher it hides the water. **The tide never remeshes the sea.**
- **The waterline rule:** the mean sea lies 1/8 under a layer's top so a block never flickers through it; a moving
  sea crosses the tops. The shader keeps the level 1/64 layer off any top and steps over it (3 cm, at once).
- **The client's stored-water mesh** re-meshes its wet tiles' unstored sea columns every 4 s of world time, one tile
  a frame.
- No save or wire change: the tide follows from the clock, which both already carry.

## Checks
- Tests, each seen failing first: `sky::the_tide_is_a_metre_at_springs_half_at_neaps_and_twice_a_lunar_day` and
  `the_tide_moves_water_and_makes_none` (a flat stub failed both); `liquid::a_dug_harbour_follows_the_tide_in_and_out`
  (a harbour behind a 2-wide channel follows a 1 m tide in and out over two tides, `counted()` exact every step;
  without `retide` it never moved); `world::the_tidal_band_is_wet_at_high_water_and_meshed_for_it` (with the mean's
  quads it found none over the band). `the_sea_is_generated_where_the_ground_is_low` and
  `rivers_and_lakes_hold_generated_water` now expect the sea at its tide.
- The soak across many tides: `technoape-server soak=1 planet=1536 bots=4 ticks=3000 seed=7 warp=60` (the clock 60
  times faster, a new `warp=` of the soak): 150 minutes of world time, about 9 tides, 18 dams built and 17 broken,
  the ledger exact every tick, the same hash twice, 30 s a run. Unwarped, 5 minutes with 8 bots: 142 s against
  main's 157 s, the same water awake.
- The gate (`tools/check.sh`, once): tests, soak and replay pass. The views `high` (5.1 %) and `orbit` (1.4 %)
  differ by the tide: at the views' time the sea stands high there and the beaches on the band's layer are under
  water; not accepted, the developer's to see. The perf rows failed while another session's mesh benchmark ran on
  the PC (sim's tick 268 ms in the gate, 2.0 to 2.3 ms in quick runs alone right after; 12 alternated runs on a
  quiet PC matched main): to repeat on a quiet PC before merging.
- Filmstrips: `out/tides/day.png` (a spring day, a frame a minute), `high-low.png`, `month.png` (each day's daylight
  high and low over a month: springs on days 4-5 and 11-13). The script: `prototypes/tides/film.py`.
- Review: a fresh reviewer read the diff. It found the ledger, the GPU layout and Luna sound, and seven points:
  (1) coarse chunks lost their shallows' solid test: fixed, coarse levels keep the mean's cell; (2) a loaded save
  retided mid-second: fixed in `restore`, and the server sets the save's clock before restoring; (3) a river mouth
  over the sea was woken every second: fixed, `retide` compares with the level `open_ends` holds; (5) the client's
  4 s re-mesh of every wet tile on one frame: fixed, one tile a frame; (4) under `/timewarp` the epoch changes
  every tick and `retide` walks the stored columns each tick: left, a shortcut below; (6) a film under the dead band
  can stay on a beach node above a falling sea: left (cosmetic, counted); (7) plants on the tidal band stand in water
  at high tide: left for the developer (below).

## Cost (CPU first)
| What | Before | After |
|---|---|---|
| `Tide::at` (per vertex, per query) | - | 4.4 ns |
| `World::ground()` (the poses, per query) | ~0 | 148 ns |
| a sea column's `generated()` | 249 ns | 340 ns |
| frame CPU p50, `stand` / `fly` / `sim` (ms, two runs each, alternated with main's binary) | 1.73, 1.74 / 2.06, 2.33 / 2.10, 2.09 | 1.72, 1.76 / 2.17, 2.22 / 2.16, 2.14 |
| server tick p50 in `sim` (ms) | 1.80, 2.12 | 1.81, 1.97 |
| GPU: `vs_sea` | - | two dot products and a few multiplications a vertex |

Within the runs' spread everywhere; `budget_ok` held except one 21 ms frame in one `fly` run on each binary.

## Consequences
- A tide mill needs no rule: a pond with a gate fills at high water and drains through a wheel at low.
- **Shortcuts:** under `/timewarp` (seconds of world time per tick) `retide` runs every tick, O(stored columns); add a
  minimum interval in saved steps if a warped server with large floods shows it. A dry stored column at the coast is
  checked once a second against its neighbours' generation.
- **Open for the developer:** a wet-sand look for the tidal band (it reads without one in the filmstrips); plants
  are placed by the mean sea, so grass on the band stands in water at high tide.
