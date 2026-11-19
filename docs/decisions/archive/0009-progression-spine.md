# 0009 – Progression spine: an ape, a steam starter, electricity in tiers, free-form megaprojects

- **Status:** accepted
- **Date:** 2026-11-09 (developer's calls in a design session; the power decision was revisited and replaced
  the same day, before anything was built on it)

## Context
ADR 0008 fixed harsh survival and megaprojects as what pulls the ages. Who the player is, when space is
reached, how power works and what form a megaproject takes were still open, and they decide the shape of the
simulation (how many network systems) and of the content. The proposal built on these calls:
`docs/design/progression.md`.

## Options considered
1. **Player:** an ape with bare hands; a castaway astronaut with a wreck; no framing.
2. **Space:** visit early and live late; space as the finale; space within hours.
3. **Power:** three kinds (shafts, steam, wires); one unit with tiers; a new kind every age. The developer
   first picked a new kind every age, then replaced it with the decision below.
4. **Megaproject form:** free-form and judged by a rule; a blueprint to feed; free-form early, blueprint late.

## Decision (developer)
- **The player is an ape** that starts with bare hands and climbs to orbit.
- **Visit early, live late:** orbit and the moon are first reached about halfway; surviving there and hauling
  in bulk is the second half.
- **Power:**
  - **A starter** groups the stone age and the steam age: about an hour from bare hands to copper, then a
    hand crank drives a pump, the pump feeds a fire-heated boiler, and steam turns cogs and shafts.
  - **Rotation** powers everything from steam until LV, which is reached after 7-10 hours.
  - **Metals:** copper in the first hour, bronze through the steam age, iron at its end, steel as the gate
    to MV. About 110-150 hours to the moon base.
  - **From LV on, electricity is the one base system, in GregTech-style tiers** (LV, MV, HV, ...): better
    cables, machines and sources, and quite realistic.
  - **Released steam returns as rain,** so ponds refill naturally.
  - **After the moon base milestone an exotic resource** (warp fuel, a quantum liquid) feeds exotic space
    technology. Power stays a main concern to the end.
- **Megaprojects are free-form:** the player designs them and the planet judges the result by a rule, not
  the shape.
- **A whole planet can be mined out and drained** late in the game; it must be physically possible.

## Consequences
- Water and rock are counted, not fixed: nothing appears or vanishes by itself, but the player can remove
  all of it. A save must be able to store "this volume is gone", and the grid, which stops 64 m under the
  surface today (ADR 0003), must later reach the centre. Each is its own ADR and spike; until then nothing
  may assume the core is unreachable.
- Three network rule sets on one engine: pipes (water, steam, later fuels and the exotic liquid), shafts,
  wires. Each gets its own ADR and benchmark when built.
- The planet's water cycles through the air; the air needs a coarse water store and rain must cost nothing
  where no chunk is awake (ADR 0010).
- "Quite realistic" electricity means voltage, current and cable loss are simulated, so voltage tiers have a
  physical reason (distance), and generators are shafts turning dynamos.
- A megaproject's rule is a measurement the simulation already makes (power, temperature, an orbit, tonnage)
  with a threshold and a duration, as data; bots can test it.
- The start has no tools, no suit and no map: the first hour is hands, fire and the night.
- Space content cannot be left to the end of development: the first visit sits in the middle of the game.
