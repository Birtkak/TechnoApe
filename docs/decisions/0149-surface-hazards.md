# ADR 0149: Surface hazards: lightning in storms, volcano eruptions

Status: accepted (2026-11-07). Runtime only: `scripts/world/weather.gd` (storms, strikes), `scripts/world/volcano.gd`
(eruptions), the lightning rod (material 139, `LightningRodEntity`, `PlantStamps.for_placed`). No generator change.

## Context
The fun pass (2026-11-07): the simulation should make problems the machines and the player's building solve,
with real damage, and every hazard needs a warning sign and a counter. Storms existed in name only
(`/weather storm` was rain); volcanoes (ADR 0146) are cones with a still lava lake that never does anything.

## Decision
1. **Storms.** A rolled shower in a column with humidity over 0.4 is a storm with chance `storm_chance` (0.3);
   `/weather storm` forces one. Every `lightning_every_s` (12 s, ±60 %) a strike: half of them *distant* (the sky
   flickers at 0.35, `weather/thunder_far` 2-6 s later: the warning), the rest *here*.
2. **A strike lands on the highest surface voxel within `lightning_target_m`** (24 m) of the player, read from the
   sim's own `SimWorld.surface_map` every 0.75 m (64 x 64 columns, ~4 ms native, once a strike; 0.5 m cost 12.8 ms).
   So a tall tree, a tower, a roof or a monolith takes it, and anything the player built counts. Effects: the sky and
   sun blaze (`Atmosphere.flash`, fades over ~0.3 s), `weather/strike` at the spot, `weather/thunder` after
   0.3 s + distance / 340 m/s, flames (`FIRE` cells) fill the air within `lightning_fire_m` (0.6 m) of the hit and the
   hit voxel itself burns if it is burnable (so a plank tower burns down, the fire rain of ADR 0056 answers it), and
   a player within `lightning_hurt_m` (2 m) loses `lightning_damage` (8) health ("lightning").
3. **The lightning rod** (item, crafted from 2 iron rods over a copper ingot at the crafting table) is material 139
   with a `lightning_rod` block entity at its foot: placing it stamps a 2 m pole (`PlantStamps.ROD_H` = 16 voxels,
   drawn as the thin torch micro shape in the chimney-iron tile). `Weather.pick_target` redirects a strike aimed
   within `lightning_rod_m` (24 m, flat) of the chosen spot to the nearest rod's tip and grounds it: flash and
   thunder, no fire, no damage (`stats.grounded`). The entity list gives every rod's position without a world scan.
4. **Eruptions.** Every `volcano_roll_s` (90 s) a crater within `volcano_near_m` (40 m) of the player erupts with
   `volcano_chance` (0.35); `/erupt` forces the nearest one (it must be within 40 m: the sim window). Phases:
   *warn* for `volcano_warn_s` (20 s): a lava loop rumbles at the crater (`volcano/rumble`, pitched down) and a 1 m
   ball of smoke rises over the lake every half second, `volcano/burst` now and then; then *flow* for
   `volcano_flow_s` (40 s): a 0.5 m ball of moving `LAVA` every half second at the *spout* (3 m outside the rim on
   the player's side, on the ground there; the rim is the highest column walking out from the centre toward the
   player), so it runs down the flank, lights wood (ADR 0144) and makes obsidian in water; every 3-7 s a lava bomb
   lands on the flank beyond the rim. It stops at `volcano_lava_cells` (16000) poured. Nothing lands in the crater:
   waking the still lake as a whole (LAVA_STATIC -> moving) cost 65 ms a sim tick (first run, 2026-11-07).
5. **Playtest:** `/weather storm`, `/lightning [here]`, `/erupt`; the JSON summary carries `weather.strikes /
   far_strikes / grounded / last_strike` and `volcano.phase / poured / bombs / crater / rim / spout`.

## Consequences
- Lightning on the monolith: it is the highest thing near spawn, so most strikes at spawn go there (harmless: it is
  not burnable). Metal does not attract strikes beyond the rod. shortcut: no `metal` flag on materials; add one when
  iron roofs exist.
- A storm needs the player outdoors for the hazard to matter; underground nothing happens.
- Lava poured on a flank solidifies where it thins (`film_lifetime_ticks`), so an eruption leaves a tongue of
  obsidian/basalt crust down the cone. Repeated eruptions pile it up; the cone does not grow in the generator.
- Budget (spectator, default settings, 150 voxels over the crater, 2026-11-07): the spot itself misses ADR 0009 with
  no eruption at all (weather=0: p99 33 ms, 409 frames > 16.7 ms, streaming spikes); with an eruption p99 was 28-32 ms,
  so the eruption adds no p99. Sim tick 23 ms during a full eruption. Open: single blasts up to 37 ms (a ball landing
  on fresh lava) and the volcano spot's own frame cost; both need a native look.
- First run lessons: lava or smoke touching the still crater lake wakes all of it (65 ms, then 56 ms sim ticks), so the
  pour goes outside the rim and the ash starts 6 m over the lake.

## Follow-up (2026-11-07, second round)
- The rod is not a station (nothing is crafted in it; the recipe browser's station test). Breaking any voxel of the
  pole takes the whole rod (`footprint_cells`, Main._break_torch) and drops one rod item (mining.json).
- Metal draws strikes: a metal surface voxel (ores but coal, anything iron or copper, rails, the rod) counts
  `lightning_metal_m` (3 m) higher in `pick_target` (`Weather.metal_mask`, by name: materials have no metal flag;
  machines and cables are station voxels and do not count yet).
- `/goto volcano` lands on the rim on the side you come from (`Volcano.rim_toward`), not in the crater lava.
- The 37-135 ms drops were `SimBubble.blast` waiting for the worker pool, which the burning flank keeps busy. The pour
  (a 0.75 m cube) and the ash (a 1 m cube at 60 %) now use `SimWorld.scatter`, single-threaded and in place:
  0.1-0.2 ms each. Bombs stay sphere edits (2.7 ms worst). shortcut: scatter sends no net op (ADR 0145).
