# ADR 0154: Water wheels run on flow; weak dams leak, then break

Status: accepted (2026-11-07). Fun pass row G (`docs/plans/fun-pass.md`). Builds on the 48 V sources (ADR 0123),
endless world water (ADR 0104), the one fluid system (ADR 0119) and running machines (ADR 0153).

## Context
The ADR 0123 water wheel turned whenever the slab under it was wet, so a bucket of still water in a trough made the
same power as a river. Dams did not exist: any wall of dirt held any head of water for ever. The fun pass asks for
hazards that do real damage with a warning and a counter: a wheel that needs running water, and a dam that a
careless builder makes too thin, which seeps, creaks and then floods what is downstream.

## Decision
- **Flow, measured.** `SimWorld.count_moving_in_box` counts the cells of a liquid in a box that moved this or the
  last tick (the per-cell move stamps the kernel already keeps; a substep's mark carries its tick). Once a second
  `WorkSystem.wheel_drive` reads the 0.5 m slab under a wheel's footprint: drive = moving cells / (cells x
  `wheel_flow_full` 0.1), plus the river's own still water (`water_static`, never simulated, ADR 0104) weighted
  by the generator's `river` value at the wheel (a channel's current; a lake, the sea, a pond and a trough give
  nothing). No flow, no power. The EMF is still `volts x drive` behind the dynamo's ohms (ADR 0123).
- **A bigger wheel.** `large_water_wheel`: the same paddle model at 2 m (`BlockEntities.SCALE` 4), its 2 x 2 m
  slab, 50 V behind 0.3 ohm (up to ~2 kW at full flow); crafted from two water wheels, planks, an iron rod and a
  copper coil. Both have a `machine_fx.json` entry (splash while turning); the model keeps its stepped paddles.
- **Dams are what the player builds.** `Dams` (`scripts/physics/dams.gd`) remembers every 0.5 m block a placement
  touches (Main `_on_placed`, `_on_placed_many`; at most 512, oldest dropped; not saved) and once a second hands the
  boxes to the native `SimWorld.dam_scan`. Generated banks are never scanned, so rivers stay as generated.
- **The load.** For a wall cell with liquid against one horizontal face: the head is the liquid column on that face
  from the cell's row up; the run is the wall behind it along that axis up to the first non-wall cell (at most 16);
  the dry head is the liquid column past the run. load = (head - dry head) / sum of the run's `hold`
  (`data/dams.json`: dirt and grass 3, gravel 2, sand 1.5, clay 5, planks 6, snow 1 head cells per cell of
  thickness). A material without a hold (stone, cobble, bricks, logs, metal) never fails and backs any run it is
  in. A 0.5 m dirt wall holds 1.5 m of water; 1.5 m of dirt holds 4.5 m.
- **Warning, then the break.** Load 0.5..1: a slow weep (now and then a cell of water on the dry face, a drip) and
  it holds. Load >= 1: it creaks (`tree/creak`, louder and lower as time runs out) and seeps harder for 40 s at load
  1 down to 8 s at load 3, then the overloaded rows wash out through the whole thickness, the chat says so, and the
  native water pours through (static ground above loosens on its own, ADR 0013). The thinner dam left behind is
  judged again on the next scan, so a breach widens. A scan that finds the block no longer overloaded cancels it.
- **Counters.** Build it of stone or brick, make it thicker, or keep the level down with a spillway (the head is
  measured, so a lower pool loads it less).
- **Test layout.** `/stations dam [cam]` and `/scenario dam`: a still pool, a race with a water wheel (cable and lamp
  on top), a 0.5 m dirt dam holding 2 m and a dry pit; the race fills, the dam weeps and creaks, breaks, the flood
  turns the wheel and lights the lamp until the pit is full. `dams=0` turns dams off.

## Consequences
- The /stations power trough of still water no longer turns its wheel (shows "no running water").
- The scan is a native pass over at most 512 x 64 cells a second (sub-millisecond); it waits for a running tick
  like `count_in_box`.
- Not done: saving the remembered blocks (a reloaded world forgets which blocks are dams until they are built
  on again), wheel output by the water's speed rather than its moving-cell count, dams of rigid pieces.
