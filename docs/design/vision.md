# TechnoApe – Vision

- **Status:** living document, the ground truth the code must agree with. Rewritten 2026-11-09 for the
  planets game (ADR 0001-0005); nothing of the earlier flat-world game applies.

## One-liner
A first-person survival and automation game on small spherical planets of 1 m blocks: walk around a planet,
dig into it, build factories on it, and fly up to orbit and over to its moon without a loading screen.

## The idea
- **Planets you can hold in your head.** Terra is 4 km in radius (ADR 0021): you can walk around it in under
  two hours, see its curve, and reach space by climbing. One planet and its moon first, **Terra** and **Luna** (ADR 0026: each simulated once when the world is
  made, `worldgen.md`); a solar system later
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
4. **One clean look.** Every block, item and model sits on one pixel grid of 16 px per 1 m block, with
   16x16 pixel textures, so nothing is ever the wrong size. Far terrain is honestly coarser.
   Real-time shadows, a sun that circles the planet, dark nights and caves, rich 3D sound.
5. **Runs perfectly on the dev PC.** The frame budget of ADR 0004 holds in every run, and no system may stall
   the frame thread; weaker PCs use lower presets.
6. **Built to be tested by machine.** Bots are ordinary clients: tests drive them by script or live and read
   JSON state and filmstrips back, without anyone at the keyboard.

## Fixed decisions
- Perspective: first person. Blocks 1 m, the player one block wide and two tall (ADR 0033); fine cells of 12.5 cm, eight to a
  block's edge, only where needed: gases and finer simulation, crafting in the world, chiselling and
  intricate building.
- Engine: our own, the Pillars of Simulation Engine (`crates/engine`), Rust + wgpu; gameplay in Rust, content as data files (ADR 0002).
- Grid: equi-angular cube-sphere (ADR 0003).
- Budget: p99 <= 8.3 ms, no frame > 16.7 ms, RAM <= 8 GB, VRAM <= 6 GB on the dev PC (ADR 0004).
- Platforms: Windows client, Linux server in Docker on the home server over Tailscale, automatic updates
  (ADR 0005).
- Day about 20 minutes; saves hold only what changed (ADR 0002).
- Feel and rules (ADR 0008, detail in `fundamentals.md`):
  - **Tools are brushes:** a dig or place is one 1 m block (ADR 0033), better tools carry bigger brushes; the
    build hammer drags out lines, walls, floors and shapes (ADR 0017).
  - **Survival is harsh:** food, water, temperature and air. Every need starts as a chore and ends as a
    factory.
  - **The planet's facts are rules:** the sun and time zones (dark, cold night side; solar power follows the
    sun), a finite planet (deposits run out, scars show from orbit), real orbits (things can orbit; cargo can
    be shot to Luna).
  - **Megaprojects pull the ages:** each tech age ends in a planet-scale build that changes the world.
  - **Planet size is a parameter:** 4 km as standard, 8 km for the huge one (ADR 0021).
- Progression (ADR 0009, proposal of the tiers in `progression.md`): **you are an ape** that starts with bare
  hands; a **starter** of stone, fire, steam and cogs (an hour to copper, 7-10 hours to electricity); then
  **electricity in GregTech-style tiers**, quite realistic; **released steam returns as rain**; megaprojects
  are **free-form, judged by a rule**; orbit and Luna are **visited about halfway, lived on late**; after
  the moon base an **exotic resource** feeds the space technology that leaves the planet; late in the game a
  **whole planet can be mined out and drained**.
- The planet's air (ADR 0010, detail in `progression.md`): a **water cycle, weather, pollution and
  radiation** run for the whole planet, not only around players.

## Open design questions (decide when they become relevant)
- The tiers themselves: `progression.md` proposes Starter, LV, MV, HV, EV, IV and an exotic tier, with a
  megaproject each; the exotic resource's name and rule.
- Decided in ADR 0027: the ape learns by doing and seeing (`journal.md`), Terra has animals and hostile
  creatures, the 8 cube corners hold something made. What the creatures and the made things are is open
  (`open-questions.md`).
- AI players as the ape's troop.
- How fast needs drain, and what death costs (proposed in `needs.md`); how the ape carries (`items.md`).
- Magic or wands: dropped for now, may return later.
