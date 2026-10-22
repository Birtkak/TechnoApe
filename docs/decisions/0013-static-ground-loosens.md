# 0013 – Static ground loosens next to changes (Noita), kernel v6

- **Status:** accepted
- **Date:** 2026-10-22 (asked for: "make it so static ground has a chance to
  loosen like Noita")

## Context
Generated ground (dirt, grass, static sand and gravel, snow, generated
water) is "static": it never moved, so craters kept vertical walls, tunnels
never caved in and lakes ignored a broken shore. The micro-resolution study
(`docs/research/2026-10-22-core-architecture-sim.md`) showed the cost risk of
the opposite extreme: a hill that is entirely loose slumps as a whole and
costs 14-36 ms per tick.

## Decision
- A static-ground cell gets a chance per tick to turn loose (flag
  `FLAG_LOOSE`, it then moves as its powder or liquid kind) when:
  - the cell below it opened (unsupported: ceilings and overhangs crumble);
  - for static water, any side or the cell below opened (shores break);
  - for natural sand and gravel (materials with a `loose` kind), the cell
    beside and the one below that opened (walls slump into slopes; dirt and
    grass walls stay upright).
- "Opened" means changed this or the previous tick (a move, a player edit,
  an explosion); a cell that saw such an opening is marked shaken and keeps
  rolling its chance while the opening stays open. A freshly loaded world
  (generated caves and overhangs) never collapses by itself.
- Chances are data: `loosen_chance` (default 0.35 per tick for static
  ground) and `erode_chance` (default 0.04 for static powders with a `loose`
  kind, 0 otherwise); 0 keeps Minecraft behaviour for a material.
- Kernel v6 also: ticks on a background thread (`start_tick` /
  `finish_tick`; the game's bubble uses it), level of detail by distance
  (`set_lod`) and a per-tick cell budget (`set_cell_budget`) that defers the
  furthest chunks instead of dropping work.

## Consequences
- Mining under dirt brings the dirt above down; horizontal tunnels through
  dirt cave in over a fraction of a second; digging next to a lake floods
  the hole. Stone, clay and other solids never loosen.
- Cost: an 8 m crater in static ground now collapses (90 k cells moving per
  tick at 6.25 cm) at 2.8 / 3.4 ms per tick; the worst loose case is held at
  7.0 / 7.9 ms by a 1.5 M-cell budget.
- Loose flags are not saved: a pasted chunk reloads as static ground.
