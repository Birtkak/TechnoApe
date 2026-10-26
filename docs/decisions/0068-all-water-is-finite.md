# ADR 0068: All water is finite

Status: accepted (2026-10-26)

## Context
Bug reports of 2026-10-25 (exported build, seed 1579829108):
- 21-17-05 "lava is pushing the water out and while it makes sense there would be overflow there is increasingly
  spawning infinite water. i think we need to scrap reservoir and go back to all limited liquids again".
- 21-17-38 "yes this will infinitely scale at this height and fill the map" (10 fps, 220 sim chunks awake, the river
  spreading over the plains).
- 21-01-01 "broken waterfall": the generated sheet drawn as a knitted chevron pattern.

How the water multiplied under ADR 0049: lava sinking through a river traded places with its still water, the
displaced water rose as moving water, came to rest on the river (a *big* body) and joined it as still water
(`FLAG_JOINED`); every joined cell was a source again, allowed to pour `POUR_BUDGET` = 255 more cells into any
opening, and the lava kept opening cells. Each cell of river made up to 255 cells of water, without end as long as
the lava moved.

The developer's call (2026-10-26): "change back to all water being finite and simulated".

## Decision
1. **No reservoirs, no bodies.** `measure_bodies`, `body_class`, `FLAG_JOINED`, `BIG_BODY_CELLS`, `POUR_BUDGET`
   and the pouring in `reservoir` are gone. Nothing in the world makes water.
2. **Still until disturbed, then finite.** Generated liquid (`water_static` and the other `liquid_static` liquids:
   rivers, lakes, the sea, cave pools, sludge pools, waterfall sheets) sleeps until a *fresh* opening touches it: air
   beside or below it that changed this or the last two ticks (stamp 0 = never, so a reload or the air the generator
   left never wakes it). Then that cell becomes its moving kind (`water`), fullness kept, and is plain
   mass-conserving Noita water from then on. The cell it leaves is the next fresh opening, so a hole beside a river
   drains the river into it cell by cell, the river's level beside the hole drops, and the world holds exactly as
   much water as before.
3. **Kept from ADR 0049:** air over a river's own lower still cells (each 12.5 cm step of its surface) is no
   opening; moving water in a hole never climbs sideways out onto still water; gravity never waits for the viscosity
   roll; a cascade keeps a pouring stream whole; sand and lava sink through still water by trading places (the
   water rises as moving water: finite). Water on a still body of its kind that reacts with it stays awake.
4. **New:** moving water that already rests on a still body spreads flat over it (a water bomb on a lake is a film,
   not a heap). Still water stays still under it (a reload shows what was poured, at rest).
5. **No waterfalls.** The developer, shown the sheet with softer streaks: "remove the waterfall, it sucks". The
   generator no longer hangs a still-water sheet from the lip (`GEN_VERSION` 21), and `waterfalls_near` lists
   nothing, so there is no spray, no roar and no waterfall landmark; the river still drops over its cliff. Real
   falling water needs a flowing river, which finite water in a 64 m window cannot give: see Alternatives. The
   shader's foam on falling faces stays for pours (two-voxel lanes, long soft streaks; a falling face colours as a
   thin sheet, never deep).

`get_stats` reports `still_woken` (still cells turned moving) instead of `bodies_measured` / `bodies_drained` /
`body_cells_max`; the bubble's JSON `still_woken`.

## Measured
- Kernel tests (`test/test_sim_v6.gd`): a lake drains through a breach and the world holds the same water after;
  a pond gives a pocket its water and the rest sleeps on; the walled-off pocket spills what it held; sand sinks
  through a lake; lava falls every tick; river steps stay quiet; water poured on a lake spreads flat and loses
  nothing; loaded water beside generator air stays.
- `water_test=1 target=river` (headless) and a lava bomb into a river, a waterfall capture: see the commit.

## Consequences
- A river or lake beside a hole drops where it drains: a crater in a river is a lasting dent with a lower pool in
  it. The developer chose this over a world that makes water.
- A disturbed stretch of river is moving water from then on and is baked into the terrain as `water` when the
  window walks on; it rests where it stands (plain moving water at rest is quiet).
- Nothing grows without bound: the sim's cost after a lava spill is bounded by the water the lava displaced.

## Alternatives
- Keep big bodies but cap the pouring per body (not per cell): still a source of water; the developer asked for
  none.
- Flowing rivers (a source at the spring, a sink at the sea, moving water in between) for real waterfalls: needs
  flow beyond the 64 m window and a generator that agrees with it; out of scope here, a Phase 3 topic.
