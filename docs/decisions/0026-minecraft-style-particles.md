# 0026 – Minecraft-style particles

- **Status:** accepted
- **Date:** 2026-10-23

## Context
The developer asked on 2026-10-23 for "Minecraft alike simple particles to most things
that can use it to lifen up the game". They added: "a lot of them at the bottom of a
waterfall depending on its size as well, making it feel impactful like it's really a
bunch of water colliding".

Before this, the game had three one-off effects: a CPUParticles3D puff when an animal
died, Label3D hearts, and a soft GPU mist at waterfall feet. Breaking a block, landing,
lava, fire and furnaces showed nothing.

Constraints: ADR 0009's frame budget, simulation kept separate from presentation, data
over code, and principle 8 (GDScript never iterates voxels).

## Options considered
1. **One node per effect** (CPUParticles3D / GPUParticles3D per burst, like the old
   poof). Easy, but every burst allocates a node and its first draw compiles a
   pipeline. Hundreds of mining chips a minute would churn the scene tree.
2. **A GDScript particle simulation drawn by a MultiMesh**. Full control (voxel
   collision per particle), but a few thousand particles moved in script costs
   milliseconds.
3. **Two GPUParticles3D pools fed by `emit_particle`, moved by our own particle
   shader**. Chosen. Script only emits. The GPU moves everything. One floor height
   per burst, found with a few voxel reads at emit time, stands in for collision.
   Particles rest on it, skid and stop, or die on it, by motion.

For ambient effects (lava pops, cave drips, furnace flames), Minecraft's `animateTick`
samples random blocks around the player every tick. Doing that in GDScript would
iterate voxels. Instead, `SimWorld.sample_cells` does the sampling natively over the
simulation bubble. It returns only the hits: cells whose material has ambient rules,
or cells that are burning.

## Decision
- `scripts/visuals/particle_fx.gd` (`Main.fx`) owns two pools. **chips** draws a
  random 4×4 pixel square of the block's terrain atlas tile, as Minecraft's breaking
  particles do. **sprites** draws an 8×8 pixel-art sheet that is built at startup from
  `data/particles.json`.
- Every particle is one `emit_particle` call with its whole state: position, size as
  the scale, velocity, colour, and CUSTOM = (sprite code, age, lifetime + 100 × motion,
  floor y). The pools never emit by themselves. Godot's `particles_emit` switches
  `emitting` off and wakes an idle system, so `emitting` stays false.
- `assets/shaders/particle_process.gdshader` applies gravity, drag, sideways sway and
  the floor rule. `particle_draw.gdshader` draws a camera-facing quad cut out at alpha
  0.5, so nothing needs sorting. It is lit as a voxel top face, so particles darken at
  night. Glowing motions (flames, sparks, spores) add emission.
- `data/particles.json` holds the sprites, the motions, the effects (all in metres and
  seconds), the chip tile overrides (grass breaks into dirt), and the ambient rules per
  material. Scripts ask for effects by name: `fx.burst("poof", at)`.
- Effects wired in:
  - Mining: chips off the aimed face at about 10 a second.
  - Breaking: 20-28 chips of the block.
  - Landing: dust of the block landed on, more for longer falls (player and animals).
  - Sprinting: dust at the feet.
  - Water: a splash on entering water, sized by speed, and breath bubbles under water.
  - Eating: crumbs in the food sprite's colour.
  - Hurt: blood drips.
  - Animals: hearts on breeding and love, damage hearts when hit, a white poof on
    death, wool chips on shearing, grass chips when grazing.
  - Work pieces: flint chips per knap, clay per stroke, grit from the sieve, a poof
    when a piece is finished, flames and smoke over lit firepits, smoke over burning
    pit kilns.
  - Ambient: lava pops and smoke, fire smoke and embers, flames and smoke on burning
    cells, furnace flames at a free side, drips under stalactites and under stone
    with water above, glow mushroom spores, falling leaves, bubbles on toxic sludge.
- **Waterfalls**: `WorldGen.waterfalls_near` now reports the sheet's `width`, in voxels
  (60 % of the river's half width on each side). Along the whole width of the impact
  line, `ParticleFx` throws white droplets up and out, slides foam over the pool and
  raises mist. The rates are proportional to width × √(drop in metres), the water's
  speed at the bottom. Throw speed grows with √drop. The soft GPU mist cloud of
  `WaterfallSpray` now also grows with width and drop.
- Budgets (`settings.gd`):
  - Pools of 2048 chips and 6144 sprites.
  - At most 700 particles emitted per frame. The rest of a burst is dropped and
    counted.
  - Bursts beyond 64 m are skipped.
  - Waterfall spray within 90 m, fading out over the far 60 %.
  - Ambient sampling: Minecraft's 667 samples per tick within 8 m, plus 2000 within
    20 m, at 20 Hz.
  - `particles=0` and `particle_ambient=0` turn the system off for benchmarks.
- Debug: the JSON snapshot has a `particles` block (emitted, dropped, culled, bursts
  per effect, ambient samples and hits, waterfalls, worst ms). `full_frames=1` saves
  every filmstrip capture at full size, because small particles are invisible in
  thumbnails.

## Measured (dev box, 2026-10-23)
Benchmark: the largest waterfall within 6 km of spawn on seed 1337, at (-5805, 343, -1428).
Its sheet is 14 voxels wide with a 16-voxel drop, energy 40. The camera is 15 m downstream.
- Lit per pixel, the waterfall's mist doubled p50 (3.4 to 6.5 ms). The quads overlap
  many times and every fragment ran the clustered lighting. With `vertex_lighting`,
  two runs each way:
  - Particles on: p50 3.18 / 3.19 ms, p99 4.09 / 4.18 ms.
  - Particles off: p50 3.19 / 3.21 ms, p99 4.28 / 4.26 ms.
  - Every run is within the ADR 0009 budget.
- `ParticleFx._process` averages 0.10-0.12 ms a frame at that waterfall, about 1250
  particles emitted per second. The native ambient sampling is under 0.01 ms per
  20 Hz tick.
- Mining with bare hands (`mine=1`): chips burst from each broken block and settle in
  the hole. That run found `mine=1` broken (the hand check overwrote the autopilot's
  hold), which is fixed here.

## Consequences
- New effects are JSON plus one `burst` call. New ambient effects are JSON only.
- The process shader has no per-particle voxel collision. A chip flying sideways goes
  through a wall, and particles keep the floor of their burst's centre column. This
  matches what Minecraft mostly gets away with.
- Up to 16 motions (shader uniform arrays), sprites of up to 8 frames, and lifetimes
  under 100 s, from the CUSTOM packing.
- Ambient sampling covers only the simulation bubble (24 m), which also bounds where
  ambient particles can appear.
- Item-sprite particles (Minecraft's item crack on eating or a broken tool) are
  approximated by motes in the sprite's average colour. A third pool on the item
  sprite atlas would make them exact.
