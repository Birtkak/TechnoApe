# TechnoApe – Vision

- **Status:** living document, the ground truth the code must agree with. Rewritten 2026-11-09 for the
  planets game (ADR 0001-0005); nothing of the earlier flat-world game applies.

## One-liner
A first-person survival and automation game on small spherical planets of 0.5 m blocks: walk around a planet,
dig into it, build factories on it, and fly up to orbit and over to its moon without a loading screen.

## The idea
- **Planets you can hold in your head.** Each planet is 0.3-1 km in radius: you can walk around it in under
  an hour, see its curve, and reach space by climbing. One planet and its moon first; a solar system later
  (ADR 0001).
- **Building and automation with depth.** Minecraft-style building; machines, conveyors, pipes and power with
  GregTech-like depth: multi-stage processing, real choices, factories that grow. Content is designed fresh for
  planets (ADR 0002).
- **A world that reacts.** Blocks are solid and cheap; where something happens they break into fine 12.5 cm
  cells (debris, sand, splashes) and settle back. Water and lava are fill levels that flow and pool. The whole
  loaded area keeps running, not just a bubble around the player.
- **Never alone.** Multiplayer from day one on the home server, and AI players that live on the planet on
  their own (gather, build, settle).

## Pillars
1. **The planet is real.** One continuous sphere (cube-sphere grid, ADR 0003) with radial gravity: every
   block can be dug and placed, terrain comes from a deterministic generator, and the grid's seams are part of
   the world, not hidden.
2. **Crafting is decisions, not lists.** Layered chains, tag-based ingredients so substitutions emerge,
   machines and processes that matter. Depth over recipe count.
3. **Exploration pays.** Biomes, depth and other bodies hold different materials, so going farther and deeper
   opens new routes.
4. **One clean look.** Every block, item and model sits on one pixel grid (16 px per 0.5 m block unless the
   block look spike changes it), so nothing is ever the wrong size. Far terrain is honestly coarser.
   Real-time shadows, a sun that circles the planet, dark nights and caves, rich 3D sound.
5. **Runs perfectly on the dev PC.** The frame budget of ADR 0004 holds in every run, and no system may stall
   the frame thread; weaker PCs use lower presets.
6. **Built to be tested by machine.** Bots are ordinary clients: tests drive them by script or live and read
   JSON state and filmstrips back, without anyone at the keyboard.

## Fixed decisions
- Perspective: first person. Blocks 0.5 m, fine cells 12.5 cm only where needed (ADR 0001).
- Engine: our own, Rust + wgpu; gameplay in Rust, content as data files (ADR 0002).
- Grid: equi-angular cube-sphere (ADR 0003).
- Budget: p99 <= 8.3 ms, no frame > 16.7 ms, RAM <= 8 GB, VRAM <= 6 GB on the dev PC (ADR 0004).
- Platforms: Windows client, Linux server in Docker on the home server over Tailscale, automatic updates
  (ADR 0005).
- Day about 20 minutes; saves hold only what changed (ADR 0002).

## Open design questions (decide when they become relevant)
- Block look: patterned flat-colour cells or 16x16 pixel textures (a spike on the new engine).
- Survival: which needs (hunger, temperature, air in space) and how hard.
- Progression: the tech ages and how far they go before space travel.
- What the 8 cube corners hold as landmarks (ADR 0003).
- Combat and creatures.
- Magic or wands: dropped for now, may return later.
