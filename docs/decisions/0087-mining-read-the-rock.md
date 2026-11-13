# 0087 – Mining: read the rock, clear the rubble

- **Status:** accepted as design (the developer, 2026-11-13); nothing built. `docs/design/mining.md`.
- **Date:** 2026-11-13

## Context
Ore is counted, finite and found by eye (ADR 0063); a block is dug held by its hardness (ADR 0034); loose
blocks fall and bury (ADR 0079). What mining feels like was open. The developer asked for a mix of realism and
fun that plays unlike Minecraft and Vintage Story, and wants the player forced to automate and to watch the
inventory.

## Options considered
1. **Dig any block, ore by luck** (Minecraft). Known and quick; nothing of the worldgen's shapes is used.
2. **A prospecting tool with readings, slow realistic methods** (Vintage Story; fire-setting, wedges).
   Realistic; the developer found the methods not fun to play (waiting).
3. **Weight** to force hauling. Realistic; hard to manage and to keep fun (the developer).
4. **Read the rock, clear the rubble:** ore bodies show in the rock around them, rock breaks in chunks along
   visible cracks or when undercut, broken rock is loose rubble that blocks the way, small stacks.

## Decision
Option 4. The developer's calls:
- Liked as proposed: follow the body by its changed rock; the mine's hazards (water, heat, air, weak roof);
  grade matters after the dig (lean ore pays once washed).
- Breaking rock: cracks and layers on the face, a dig on a crack drops several blocks; undercut and drop;
  blasting later; a plain pick always works. No waiting mechanics.
- **Rubble blocks the way** (real loose blocks, not piles to walk over).
- **Rock, rubble and ore stack to 16.**
- **A chunk drop hurts** who is under it.
- No weight system.

## Consequences
- Hauling (basket to conveyor) becomes needed by the world's own physics, and spoil heaps come for free.
- Natural rock must be able to drop near a dig: ADR 0079 (c) searches edited blocks only, so a capped search
  of generated rock is new work, and a mine full of resting loose blocks must cost nothing a tick. Both are
  spikes in `mining.md` before any build.
- Cracks are a rule over (rock, place), part of what clients derive from the map: a change is a protocol
  change, like the rocks (ADR 0063).
- Left for play-testing: drop sizes, the waste to ore ratio, whether a core-sample drill is wanted.
