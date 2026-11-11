# Effects: one system for particles and motion

- **Status:** design proposal, 2026-11-11 (the developer: "implement an animation and particles system
  directly in the engine itself so all future particles and animations will use that"). Nothing here is
  built. The coding session spikes it, measures it and writes ADR 0031.
- Today's one-offs, which move onto it: the water's splash droplets (`crates/client/src/water.rs`), the
  sparks and the dip of the block in hand (`hud::sparks`, `App::acted`), the menu's slide (`hud::menu`).

## Why one system
Digging, placing, landing, machines, weather, creatures and tools will each want things that fly, fade and
move. Built one at a time, each invents its own spawning, movement, light, limits and cost. Built once, a
new effect is a row of data and a line that says "play this here".

## The rules
1. **Effects are presentation.** They run on the client only, never in the server's tick, never over the
   wire as particles, and nothing in the simulation may depend on one (principle 7). Two clients may show
   different sparks; they never show a different world.
2. **The world says what happened; the client decides how it looks.** An effect starts from an *event*:
   a cell changed (the client already gets every edit as a delta, so another player's digging sparks for
   free), the player's own predicted action, a landing, later a machine's or the weather's event from the
   server. Events are small and few; an event names its kind and place, never its particles.
3. **An effect is data** (`data/effects.toml`, with a schema and a test, principle 6): what it emits and
   how it moves. A new machine's smoke is a new row.
4. **A hard budget, measured** (principle 8): a cap on live particles, the oldest and farthest dropped
   first; the frame thread only swaps buffers; a benchmark row; and the settings' "Particles: few, normal,
   many" (`settings.md`) scales the counts.
5. **Off where pictures must repeat:** benchmark and view runs play no effects unless asked (`fx=1`), so
   the reference views and the speed baseline do not move. `run=fx` is the effects' own benchmark.

## Particles
- **What one is:** a place (in its body's frame, so it turns with the planet), a velocity, an age and a
  life, a size, a colour or a block and a texel of it, a spin, and its effect's row.
- **What a row says:** how many and over how long; where they start (a point, a box, a block's faces);
  speed and spread; gravity (the body's own, radial: `phys` has it), drag, and whether they collide with
  the ground (stop, bounce or die); life, and how size and opacity run over it; what they look like (a
  small cube of the block's own texels, a flat square facing the camera, a streak along its velocity); a
  light they give, if any (sparks from a forge).
- **Movement:** plain steps on the client's frame clock, in the body's frame. Collision is a column-height
  lookup, not the physics' full test, and only for rows that ask for it.
- **Drawing:** all particles in one instanced draw, into the same G-buffer as the terrain, so the sun,
  the shadows and the bounce light fall on them as on everything else and they are hidden by what is in
  front. *To spike:* whether each particle traces its own sun ray in the fragment shader as terrain does,
  or takes one visibility per particle from the CPU side; and whether they join the traced scene (they
  should not cast shadows: thousands of tiny boxes in the TLAS is the wrong cost).
- **Budget to hold** (*proposal*): 4,096 live at "normal"; under 0.15 ms of the frame's GPU time and
  under 0.05 ms on the frame thread with all of them alive on the reference machine; no allocation per
  frame.

## Motion (animations)
- **A tween** is a start time, a length and a curve (ease in, out, both, a spring, a shake) giving 0 to 1.
  **A track** maps it onto a value: a move, a turn, a scale, a fade, a colour.
- Anything drawn can carry tracks: the block in hand (the dip when it acts, a bob when walking), a block
  just placed (it grows into place in a tenth of a second), a block being broken (see below), a machine's
  part, a door, a HUD pane, a menu row.
- Tracks are data too where content needs them (a machine's parts), and plain calls where code does
  (`play(hand, DIP)`).
- Animations of the world's blocks are drawn by the client over the real block; the block itself changes
  only when the server says so.

## The first effects (what the developer asked for)
1. **Break:** the block bursts into a dozen small cubes of its own texels that fly out, fall under the
   body's gravity, land, lie for a moment and fade. From the cell's delta, so every player sees it.
2. **Place:** the new block grows from nine tenths to full size in a tenth of a second, with a few specks
   of its colour at its edges.
3. **The block in hand** on tracks: the dip it has today, and a small bob with the steps.
4. **Moved over:** the water's droplets become a row; the screen sparks go (the break's debris replaces
   them).

**A taste call the break needs:** blocks break at one click today, so "cracks while you dig" has nothing
to show yet. *Proposal:* in survival a block takes time by its material and the tool (hands: dirt half a
second, stone several), with cracks spreading over its faces as a track; creative stays instant. That is
a rule of the game (it meets the mining tools the roadmap has "later"), so the developer decides it; until
then the break is the burst alone.

## Later, on the same system
Footstep dust and landing puffs by the block underfoot; rain, snow and blown sand from the air (W3);
smoke and steam from machines (pollution is counted in the air's grid, the smoke is its picture); leaves;
sparks and embers that give light; creatures' effects; the sound that goes with each event, when the
audio step is built, started by the same events.

## Checks
- `data/effects.toml`: a schema test; every row's counts times the settings' scale stay under the cap.
- A test that a burst of a row is the same for the same seed, lands on the ground and is gone after its
  life (no leak: the live count returns to zero).
- A test that a cell's delta from solid to air starts exactly one break, also for another player's edit.
- `run=fx`: thousands of particles alive in `stand`'s view; its row in `docs/benchmarks.md` and in the
  gate's baseline. The other runs' numbers and the six views unchanged with effects off.
- Filmstrips of a break and a place, close and from 20 m.

## Order to build
1. Spike in `prototypes/effects/`: the instanced draw into the G-buffer, lit both ways, at 4,096 and
   16,384 particles; the README has the costs and the choice.
2. The system in the client (`fx.rs`): events, rows, the pool, the draw; `data/effects.toml`; `run=fx`.
3. Tweens and tracks; the block in hand and the placed block on them.
4. The break and the place; the water's droplets moved over; the screen sparks removed.
5. ADR 0031, the benchmark row, the gate's baseline.
