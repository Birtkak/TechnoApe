# ADR 0142: Mines need shoring

Status: accepted (2026-11-04). The developer's pick for a new original feature. Builds on rigid pieces (P12,
ADR 0037) and static ground that loosens (ADR 0013); caves and halls as generated (ADR 0134) are untouched.

## Context
Stone never moves: a player can hollow out a 30 m hall under a mountain and nothing happens. Dirt and sand
ceilings already crumble (ADR 0013), but rock is where mines are. The ask: a big room or tunnel dug under rock
caves in unless supported; it warns first (creaks, dust and pebbles, a prop bending) so a player who listens can
prop it; then the unsupported part breaks off as rigid pieces and crushes what is below. Fair and readable, not
a stress solver; event-driven and cheap; generated caves stand.

## Options considered
1. **A stress or max-flow solver over the rock** (the rigid pieces' wood strength). Real, but rock is everywhere:
   a flow over a mountain per dig is unbounded, and the result is hard for a player to read.
2. **Loosen rock like dirt** (ADR 0013 chances). Cheap, but no warning, no props, and halls would rain apart.
3. **A span rule on the roof surface** (chosen): every dug roof cell needs a wall within its rock's span or a
   prop within the prop's radius; props carry limited area.

## Decision
- **Roof, wall, span.** A roof cell is a solid cell with open space under it. The roof surface is walked over the
  8 columns around each cell, stepping up or down at most 3 voxels; a step into rock that goes on down is a wall,
  a rise of more than 3 (a shaft) holds nothing. Distance to the nearest wall is a Dijkstra over that surface
  (octile steps, half voxels). Span = `1 m + 1 m x mining hardness`, at most 4 m (`Settings.support_span_*`):
  sandstone 1.8 m, stone 2.5 m, ore 4 m; wood and what cannot be mined hold and never fail. A 3 m tunnel always
  stands; a 5 m stone room stands; a 7 m one fails in its middle.
- **Props** are furniture kinds with a `"prop"` entry: timber prop (stick over a log, by hand: the Flint Age)
  radius 1.5 m / 4 m2, stone pillar (three cobblestone at a table) 2.5 m / 12 m2, iron mine beam (five iron plates,
  the Iron/Steam age) 3.5 m / 30 m2. They stand floor to roof (a per-entity `height`, saved; the model stretches)
  and need a roof within 3 m. A prop carries the dug roof cells only it holds; past its capacity it fails and holds
  nothing. Any solid column from floor to roof (a pillar of built blocks) is a wall.
- **Only digs change the load.** Digs mark their 0.5 m blocks (Main._on_dug: pick and brush; the laser, crater
  rod and TNT by sphere; the box dig by cells). Only roof cells over a dug block can fail, so caves and halls the
  generator made stand as generated. The marks are not saved: a world reloads standing (shortcut, see below).
- **Event-driven, off the main thread.** `MineSupport` collects dirty boxes and queues one scan every 0.25 s
  (`support_scan_s`) plus one per prop change; the native scan (`RigidPieces.support_*`, `src/rigid_support.cpp`)
  covers the dug box plus two reaches around it, runs on the sim's driver thread before a tick (as the rigid
  piece checks do) and hands its result over in `collect()`. Nothing scans the world.
- **Warning, then the fall.** An unheld roof creaks (`tree/creak`, pitched down), drops dust and up to six real
  gravel cells, and bends its overloaded props for `warn_s(ratio)`: 60 s just past its span, 5 s at twice it
  (`support_warn_min_s/max_s`). Any later scan that finds it held cancels it. At the end the unheld cells and 0.5 m
  of rock above them (1 m past 1.5 x the span) drop as one rigid rock piece (`support_drop`; rock pieces are body
  like wood at 2.5 x the mass), overloaded props snap, and what stands within 4 m under it is hurt.
- **Modes.** Survival digs only (`GameMode` rule `cave_ins`); `/support modes all` makes creative and spectator
  digs count; `cave_ins=0` turns it off.
- **Seeing it.** Holding a prop and aiming at a roof, the look-at panel says how far the nearest wall is, what the
  rock spans and whether it holds, or when a creaking roof falls. `/support [stats|scan|probe|modes|on|off]`, F3
  "Support:", JSON `support` (scan ms on the driver thread, the main thread's part per dig, open warnings).
- **Ids.** None: the reserved 222..223 stay reserved. Props are furniture (FURNITURE voxels, open to the rule).

## Consequences
- Native change is one new file plus small hooks in rigid_pieces.cpp (bindings, the job hook, `collect`, and
  `finish_body` treating non-leaf cells as body so a piece can be rock). The sim kernel is untouched.
- Shortcut: dug marks live in memory only; a save and reload forgets which roofs were dug, so an unpropped room
  survives a reload. Saving them per chunk is the upgrade if players use that.
- Shortcut: a scan box is capped (6 m of dug box, 80 k roof cells); a very long laser cut is scanned around its
  middle and later digs fill in the rest.
- Measured 2026-11-04 (dev box, default render distance, `mine_test=pick|laser`, cave_ins on/off back to back,
  2 rounds, a 3 x 3 m tunnel 14 m under rock): pick (132 blocks) p99 6.27 / 3.77 ms on vs 6.26 / 3.79 ms off;
  laser (19 shots) p99 4.35 / 4.23 ms on vs 4.30 / 4.18 ms off. Native scan per dig, on the driver thread:
  median 0.03-0.04 ms (pick), 2.1-2.2 ms (laser, max 4.3 ms); the main thread's part per dig 0.015 ms median.
  Evidence: `game/out/mine_collapse/` (basalt roof, warning at 0.2 s, falls at 28 s) and `game/out/mine_propped/`
  (pillar at 3.3 s, held), each `support_strip.png` + `support_burst.png`.
- The laser test's 40-degree sweeps cut wider than 3 m in places, so it raised warnings there (17-18 per run).
