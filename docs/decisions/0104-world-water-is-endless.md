# ADR 0104: World water is endless and never simulated; only what it pours is

Status: accepted (2026-10-28), replaces the finite water of ADR 0068

## Context
Bug reports of 2026-10-28 (exported build, seed 1867691517), 14-38-16 "liquids need to be completely different
again", 14-39-27 "slow laggy ugly and unclear, find us the liquid fixes", and the developer: "this has been an
ongoing problem since day one, we really need to hone down on liquids; let's finally find a system that actually
works nicely". Both screenshots show a river that a bucket or a crater has woken:
- 14-38-16: 371 sim chunks awake, sim tick 30.9 ms (the 30 Hz tick has 33 ms), worst frame 21.7 ms; the whole
  surface is a field of small cubes, every cell holding a different number of eighths and drawn on its own.
- 14-39-27: 580 chunks awake, sim tick 35.9 ms (the sim no longer keeps up), push queue 660, worst frame 54.8 ms.

Since ADR 0045 fluids were reworked seven times (0045 ramps and reservoirs, 0049 bodies that pour, 0068 all water
finite, 0069 the height field, 0081, 0092, 0103). Every pass patched the mesher or the wake rule and chased the
symptoms of one decision: ADR 0068 made any opening turn the whole connected stretch of a river into a cellular
automaton forever. A river in the 64 m window is 50-100k cells; once awake its cells exchange eighths without end
and each one remeshes as its own slab. No shipped voxel game runs a per-cell automaton over its rivers: Minecraft
and Vintage Story keep bodies static and endless, Noita and Terraria are 2D and sleep most of a lake. The runaway
that made the developer ask for finite water in 0068 came from 0049's pour budget (every joined cell a 255-cell
source into the automaton), not from endless bodies as such.

## Decision
1. **Still water never wakes.** `water_static` and the other `liquid_static` liquids (rivers, lakes, the sea, cave
   and sludge pools, waterfall sheets) are world water: they never become moving liquid and never enter the kernel's
   liquid update. The automaton is for spilled liquid: buckets, bombs, lava pours, oil, rain, and what a body pours.
2. **A body pours into a fresh opening** (`SimWorld::reservoir`): when air that changed in the last two ticks (or
   its own moving water two eighths or more low) sits under or beside a still cell, the *opening* takes a full cell
   of the moving kind and the still cell stays. A hole under a river takes a stream until it is full; a pocket in
   the bank fills and rests; a breach to lower ground fills to one cell under the surface and goes on flooding what
   lies lower (the river is endless), and only the moving front costs: rested water sleeps.
3. **Head.** A side opening is fed only by a cell with still liquid of its kind directly above it. The surface has
   nothing pressing it sideways, so a stray still cell on top of a body never becomes a spring; the same stops a
   pile of lifted cells from pouring (the shape of the 0068 runaway).
4. **Sinking keeps the level.** Lava or sand sinking into still liquid trades places with it as before (ADR 0049),
   but the lifted cell becomes moving liquid: finite (one cell per cell of the path) and never a still cell on top
   of a body. The body refills what lava boils at its bed (ADR 0081 reactions run against an endless partner).
5. Lone still specks (ADR 0081) still drain away as moving liquid themselves; nothing else about the kernel, the
   mesher (ADR 0069, 0103) or the looks changes here.

## Consequences
- A river beside a hole fills the hole and stays where it was. Craters and trenches in a river are no longer lasting
  dents: the trade the developer accepted on 2026-10-28 for rivers that never simulate.
- World water is endless: buckets draw from a body without changing it, a breached bank floods lower ground until
  the breach is plugged, a cave pool dug from below pours into the cave until the cave is full to the pool. This is
  the Minecraft model; it is bounded by geometry (water never rises above its body's surface), not by a budget.
- The sim cost of a touched river is its moving front, a handful of chunks, instead of the whole stretch.
- Tests: `test/test_sim_0104.gd` (a hole under a body fills and the body keeps every cell; a breach floods at a
  trickle, under 64 eighths a tick with at most 16 chunks awake; lava lifts no still water), the water tests of
  `test_sim_v6.gd` rewritten to the new rule (lake breach, pocket, deep pit, walled-off pocket, sand through a
  lake), `test_sim_0081.gd`, the liquid fullness, film, speed and level suites: all green. `water_test=1` in the
  running game still holds (the river never gains, both holes fill, nothing above its level).

## Alternatives
- Keep finite water and only throttle the wake or simplify the moving-water mesh: a woken river stays an automaton
  of 50k+ cells; slower and never flat.
- Minecraft's distance bound (poured water flows a fixed distance and stops, no hole ever fills): needs a hop
  counter per cell the 4-byte cell has no room for, and a filled crater looks better than a sheet at its bottom.
- Instant level fill (the opening becomes still water at once): simpler and cheaper, but the flow into a hole is the
  part worth watching; chosen against on 2026-10-28.
- Per-column water height field for bodies instead of still cells: the generator, saves and mesher all speak cells;
  a rewrite for no gain while still cells never move.
