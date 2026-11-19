# 0008 – Game feel and planet rules: brush tools, harsh survival, megaprojects

- **Status:** accepted
- **Date:** 2026-11-09 (developer's calls in a design session; number 0007 is the player movement ADR that
  `crates/engine/src/phys.rs` refers to)

## Context
The vision left survival, progression and the feel of digging open. Two facts forced the questions now:
0.5 m blocks make a walkable tunnel 16 blocks per metre (Minecraft: 2), and the protocol, the simulation and
the content schema all depend on the answers. Background and numbers: `docs/design/fundamentals.md`.

## Options considered
1. **Dig unit:** a 1 m brush with drag-building; single blocks with fast break times; both behind a setting.
2. **Planet rules as mechanics:** sun and time zones; horizon and line of sight; a finite planet; real orbits.
3. **Survival:** dangers of places only; classic health and hunger; harsh.
4. **Long goal:** megaprojects; pure sandbox; a mystery to uncover.

## Decision (developer)
- **Brush + drag-build.** Tools dig and place a 1 m group (2 x 2 x 2 blocks) by default, a held key gives
  single blocks, better tools carry bigger brushes. Building drags out lines, walls and floors.
- **Three planet rules are pillars:** sun and time zones, a finite planet, real orbits. Line of sight over the
  horizon is not one.
- **Survival is harsh:** food, water, temperature and air, a main activity next to automation.
- **Megaprojects pull the ages:** each tech age ends in a planet-scale build that changes the world.
- **The planet's size stays open.** 1 km is the first playable's planet (ADR 0001, 0003); if playtests find
  it limiting it may grow to 1-5 km or more. The engine keeps the radius a parameter and benchmarks a 4 km
  planet as headroom.

## Consequences
- An edit on the wire is a shape, not a block; the server validates the whole shape.
- Harsh survival and automation are joined by one rule: every need starts as a chore and ends as a factory.
  Air and heat need enclosed rooms as a simulated concept.
- A finite planet means deposits are counted, far LOD must show edits, and the late game needs recycling.
- Real orbits mean gravity that falls off with distance and a moon whose rail is a real orbit.
- ADR 0001's "0.3-1 km" is no longer an upper bound; it is amended when a playtest decides the size.
