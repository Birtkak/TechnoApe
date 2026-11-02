# ADR 0122: Steam and water as network channels

Status: accepted (2026-11-03). Follows ADR 0113 (networks), 0117 (LV machines, the battery's flip, ordered sinks),
0097/0099 (the steam age's pipes, boiler and tank). Network logistics only: ADR 0119 owns how water moves in the
world (`sim_world.cpp`); a pipe here never spills into the world, and the only world edits stay the pump's
bucket-style scoop and the bucket itself.

## Context
`BoilerEntity.feed` walked pipes once a second in GDScript with a hard reach (`steam_pipe_max` = 8, copper counting
double) to set each steam machine's `boiler` and to top boilers up from tanks. CLAUDE.md listed fluid channels on the
solver as open; the pump (ADR 0117) needed a way to hand its water on.

## Decision
- **Two more channels on the same solver**, solved once a second after energy: `steam` (bar a second) and `water`
  (buckets a second). Pipes conduct both: a bronze pipe `pipe_steam_cap` (1.0) and `pipe_water_cap` (2.0), soft
  copper half of each, and a run is capped by its weakest pipe. **No reach limit**: thirty pipes carry like one.
- **Steam**: a boiler is a source offering its pressure; every machine with a `boiler` slot (hammer, sieve, crusher,
  blast furnace, turbine, compressor) is a sink of `steam_per_use` a second without any code change: `Networks`
  gathers them by the slot and, after the solve, points each sink that got flow at its component's first boiler, so
  the machines keep drawing from `boiler.draw()` as before. Sinks fill in placement order (ADR 0117): when a run's
  pipes carry less than the machines want, the last machines placed wait.
- **Water**: the pump is a source (what it holds, up to a pipe's worth a second), the boiler a sink of one bucket
  while its tank is low, the **tank is storage like the battery box**: a low sink while it fills (it takes only what
  boilers leave) and a source while it gives, flipping by itself with a second's lag. Fractions of a bucket that a
  proportional share hands out accumulate in each entity and move as whole buckets.
- `feed` and `steam_pipe_max` are gone; the tank's `add_bucket` stays for hand-offs.

## Measured
`tools/network_bench.gd` `steam_10k`: a boiler, 10 000 bronze pipes and 500 hammers solve in the same ~1 ms as the
energy line (one solve a second, three channels: under 4 ms a second on the main thread with a factory of that size).

## Open (defaults, developer to confirm)
- Capacities (1 bar/s bronze, 2 buckets/s) and whether copper should also leak.
- Steam as pressure shared from one boiler versus a per-pipe pressure drop: the first is what feed did; a drop is
  a per-component rule later.
- Whether tanks should ever give to pumps' channel the other way (they never fill a pump).

## Consequences
- `test_fluid_channels.gd` (a turbine over thirty pipes and the pipe pulled out, the weakest pipe capping a run with
  machines taking turns, pump to tank to boiler over pipes, saves); `test_steam_age.gd`'s reach test became a
  no-reach-limit test; `test_steam_age_2.gd`'s tank top-up waits a second for the flip.
- The look-at panel's `Networks.networks` now holds the steam and water components too.
