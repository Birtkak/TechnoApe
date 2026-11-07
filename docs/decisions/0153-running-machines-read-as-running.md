# ADR 0153: Running machines read as running

Status: accepted (2026-11-07)

## Context
Machines (ADR 0097, 0099, 0113, 0117, 0131) changed palette when lit, but nothing moved or puffed while they
worked: a factory looked the same running or stopped. What existed was generic: ParticleFx smoked every lit
block entity the same way, TorchLights gave every lit one a fire light (a loaded belt and a running lathe glowed
like fires at night) and GameAudio's machine hums keyed on `is_lit()`, which the steam hammer, sieve and crusher
never are. The fun pass asked for running machines to read at a glance, data-driven per kind, cheap enough for a
factory of ~30 in view, and easy to extend (a water wheel comes next).

## Decision
1. **One data file per kind, `data/machine_fx.json`, read by `MachineFx`** (`scripts/visuals/machine_fx.gd`, a
   child of Main). Per block entity kind: `when` (a GDScript `Expression` on the entity, default `is_lit()`),
   `starved` (an Expression: wants to work, has no power or steam), `emit` (particle effects at `per_s` with an
   optional `if`), `pulse` (a burst when a value rises from 0: the hammer's blow), `parts` (boxes over the model
   that `spin`, `bob`, or a `needle` that follows a value such as `pressure` or `charge / wh`), `shake_m` (the
   model trembles), `lamp` (steady green while working, blinking red while starved) and `light` (fire, lamp or
   none for TorchLights). Positions are fractions of the footprint box, turned with the entity's `facing` the way
   `WorkPalette.rotate_y` turns the model. A new machine is a JSON entry, no code.
2. **Expressions, not per-kind code.** Each machine already exposes its state (`powered()`, `loads`, `pressure`,
   `current()`); the data names it. A failed Expression counts as false and is reported once in `MachineFx.errors`;
   `test_machine_fx.gd` evaluates every entry on a real entity, so a typo fails the test, not the game.
3. **Nothing new to compile mid-game.** Particles go through ParticleFx's two existing pools (four new effects:
   `machine_steam`, `steam_jet`, `chimney_smoke`, `machine_sparks`). Parts and lamps share one ShaderMaterial with
   per-instance colour and glow (instance uniforms), drawn under the loading screen by `MaterialWarmup`. Lamp
   parameters are set only when the state changes (they cross to the render thread).
4. **Cost is bounded by distance.** Machines within `Settings.machine_fx_m` (32 m) are found twice a second;
   only those are evaluated (10 Hz, pulses every frame) and drawn. Farther ones are still. Bursts still pass
   ParticleFx's distance cull and per-frame budget.
5. **One source of truth for "working".** GameAudio's machine loops play while `MachineFx.working(e)` holds, so
   the steam hammer, sieve, crusher and compressor hiss while they run (new loops `machine_boiler` and
   `machine_steam` on the existing steam asset, 3D voices that fade with distance). ParticleFx leaves kinds with an
   entry to this data; TorchLights asks `MachineFx.light_of`.
6. **Belts already draw their items riding** (ADR 0100); unchanged, they only lose the firelight.

## Consequences
- Debug: `/machinefx [force|off|film [frames]]` (forced on is the effects' cost bench), `/stations fx [n]` lays 30
  machines of every animated kind on a pad with a camera over them, `/stations factory cam` films the real line.
  Main's JSON carries `machine_fx` stats (near, working, starved, parts, bursts, ms_avg, ms_max, scan_ms_max).
- Measured (dev box shared with 7 other Godot runs, so frame times are noise): 30 machines forced on cost
  MachineFx 0.21-0.25 ms a frame on average, the scan that builds the parts 1.4-1.6 ms once.
- Parts are plain boxes over the voxel model, not cut out of it: a roller or needle sits on the casing. A model
  with its own moving voxels (the turbine's blades, the hammer's head) keeps them.
- Open: the hammer head still moves by a view rebuild (ADR 0097); per-kind sounds beyond the steam loop wait
  for new assets.
