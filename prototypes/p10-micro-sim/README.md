# P10 – Material simulation at micro resolution (ADR 0012 gate 1)

**Question:** can the native falling-sand kernel tick 6.25 cm cells under
heavy activity within the ADR 0009 budget, and what must change?

**Code:** kernel in `prototypes/p5-sim-spike/src/sim_world.*` (v5: dirty boxes,
fall velocity); bench in `game/tools/micro_sim_bench.gd` (uses the game's
material table):

```
Godot_..._console.exe --headless --path game --script res://tools/micro_sim_bench.gd -- cell=0.0625 seconds=20 boxes=1 gravity=1
# only=fall+avalanche+flood+tnt_static+tnt_loose+black_hole+fire   cell=0.125   boxes=0 gravity=0 (P5 behaviour)
```

**Answer (details and tables in `docs/research/2026-10-22-core-architecture-sim.md`):**
yes, with conditions. Generated-ground events (TNT, black hole, fire) cost
0.5-2 ms per tick at 6.25 cm; bulk loose material costs up to 14-19 ms per
tick after v5 (25-36 ms before) and must run off the main thread under a
per-tick budget. Micro cells move in slow motion at one cell per tick; v5's
fall velocity brings a 6 m drop from 3.5 s to 1.5 s (real 1.1 s); liquids
still spread slowly and need fullness levels.
