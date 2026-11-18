# The deep world: the grid to the centre, the core, its field, and caves

- **Status:** direction set by the developer 2026-11-18; numbers marked *proposal* are open. The grid's spike:
  `prototypes/deep-shells/README.md`. Each engine step gets its own ADR.
- **Replaces** "hot and undiggable below 192 m" (ADR 0063) and answers `lore.md` ("A real core"),
  `world-data.md` ("Digging to the centre") and `progression.md` ("Eating a planet": the grid's part).

## The developer's calls (2026-11-18)
1. **A planet can be dug through or around, all the way.** No floor under the crust.
2. **The core holds liquid metal that can be mined, and mining it weakens the planet's magnetic field.**
3. **A weak field costs four things:** the aurora fades; radiation reaches the surface; air and water leak
   away; compasses and field-driven machines fail.
4. **Caves are realistic and follow rock and formation:** karst systems, lava tubes and magma, faults, fissures
   and sea caves, and rare deep caverns.
5. **Order:** the grid to the centre first, then the layers and the liquid core, then core mining and the field,
   then the caves across the whole depth.
6. **Pressure is part of the model** (the developer, 2026-11-18), beside heat.

## Pressure
All five below are the developer's call (2026-11-18); the numbers and rules of each are open. *Proposal:*
pressure rises with depth from the weight above (rock, liquid metal, water, air), so it is one number per
depth, like heat, until something is dug.
- **Air:** thicker in a deep shaft, thin on a mountain and on a planet that has lost its field (section 3).
- **Liquids:** water and liquid metal under pressure rise in a bore or burst into a tunnel cut below their level.
- **Rock:** deep openings need support, more with depth (ADR 0079's collapse rule, scaled by pressure).
- **The ape and machines:** a pressure limit per suit and machine tier, gating the dig with heat.
- **Rock and ore kinds:** which form at which depth (diamond only under high pressure). *Built*
  (`docs/decisions/0158-pressure-and-formation.md`): `Body::pressure` is the generated weight above a cell
  (rock by zone density, water, one atmosphere at the top, the gravity inside the body), about 10 MPa at the
  crust's floor and 82 MPa at the mantle's under the 4 km planet's spawn. The mantle's rocks take over by
  pressure and heat (`data/rocks.toml` `[[mantle.rock]]`, each boundary moving with the province's heat), and a
  mineral may name a pressure and heat window (`data/minerals.toml`): diamond, in the transition zone under the
  cold provinces. Digging does not change pressure yet; air, liquids, collapse and suits do not read it yet.

## 1. The grid to the centre
**Layered** (ADR 0152; the developer chose it over shells with seams, which they found ugly, and chose the
large core): today's grid carried down through the mantle with no seam, blocks shrinking smoothly from 1 m to
0.71 m; the grid change hidden under the liquid outer core; a plain grid of 1 m cubes for the inner core.
Liquid metal never freezes into blocks, so natural rock never shows a seam.

Rules:
- **One generated block for every reader** (ADR 0062) at any depth: mesher, physics and server ask one rule.
- **New worlds only.** A save from before keeps its 192 m floor, as with the conformal grid.
- **The deep costs nothing until someone is there.** Nothing is stored for untouched rock; chunks below are
  streamed only near a player or a hole that shows them.
- **Gravity grows as you go down toward the mass** (the developer, 2026-11-18), as inside a planet with a
  dense core: it rises through the mantle to about 1.13 of the surface's at the liquid's top (the core is taken
  2.4 times as dense as the mantle), then falls evenly to zero at the centre, where no mass is left below and a
  player floats. About 0.3 on the inner core's surface.
- **Heat rises with depth** (`Body::heat` carried on down) and is what gates the dig by age, with the rock's
  hardness: cooled suits and machines, not a wall.

## 2. What the planet is made of
A large core (the developer, 2026-11-18), the crust exaggerated as `worldgen.md` does to relief. Terra
(radius about 4.2 km); the rocks are a *proposal*:

| Zone | Radius | What it is |
|---|---|---|
| Crust | top 192 m, its floor waving by a few tens of metres (thinner under hot rifts and arcs) | today's provinces, rocks, ore and caves (ADR 0063) |
| Mantle | down to 3.0 km from the centre (1.2 km deep) | three rocks by pressure and heat, each boundary moving with the province's heat (built, draft): peridotite (upper mantle, dark olive), ringwoodite (the transition zone, deep blue; thinner under hot provinces) and bridgmanite (the lower mantle to the floor, dark brown), harder going down; diamond in the ringwoodite under the cold provinces. *Proposal still:* rare deep caverns, magma under volcanic provinces |
| Outer core | 3.0 km to 0.8 km | **liquid iron and nickel**: a sea of metal, the field's source |
| Inner core | the last 0.8 km | solid metal; the heavy metals of `lore.md` (osmium, iridium) |

Luna is cold: no liquid core, no field, which is already why its surface is irradiated (`worldgen.md`).

## 3. The liquid core and the field
- The outer core is **counted liquid** (`progression.md`, "Water and rock are counted"), but as **one level for
  the whole planet**, like the open sea's account: a connected liquid shell finds one level, so nothing is stored
  per column; metal taken out lowers that level and a ledger on the server counts it, as for ore (ADR 0063).
  Today it is generated at the mantle's floor and nothing can take it yet (`docs/decisions/0156-the-metal-sea.md`).
- **Field strength = liquid metal left / liquid metal at the start.** Pumping it out, or letting it cool solid,
  lowers it. Whether it can be poured back is open.
- What a weak field costs, in the order it is felt (*proposal* for the thresholds):

| Field | Effect |
|---|---|
| 100 % | aurora rings at both poles at night, seen from the ground and from orbit |
| under 75 % | the aurora thins and drifts toward the equator; compasses wander |
| under 50 % | sun storms hurt in the open: apes, crops, animals; rock or shielding overhead protects |
| under 25 % | the counted air and water leak to space: thinner air, less rain |
| 0 % | no aurora, no compass, field-driven machines stop, storms are deadly, the planet dries |

- It is the same trade as `progression.md`'s "Eating a planet": the mass the Ark asks for is paid with the world.
- The crater of `lore.md` reaches the core: the swarm has already taken some. *Proposal:* a new world starts
  with the field under 100 %, and the aurora shows it.

## 4. Caves by rock and formation
Extends ADR 0063's limestone systems and lava tubes. Each kind is cut only in the rock that makes it:

| Kind | Where | Shape |
|---|---|---|
| Karst | limestone, down to the water table | sinkholes and shafts from the surface, halls with dripstone, underground rivers and lakes that come out at springs |
| Lava tubes | basalt of rifts and arcs | long tubes downhill a few metres under the ground, skylights, collapsed pits |
| Magma | under volcanic provinces, deep | chambers and lava lakes; heat source and hazard |
| Faults and fissures | hard rock at province borders | narrow, tall cracks along the fault |
| Sea caves | cliffs at the sea | cut by the surf at sea level, overhangs |
| Deep caverns | the mantle, rare | crystal geodes, gas pockets, flooded aquifers |

Granite and the old shields keep almost none.

## Open
- How the inner core's stepped surface walks at a third of gravity (ADR 0152).
- The zones' rocks, their look and what the mantle is worth mining for.
- How liquid metal is lifted 2 km (pumps, pipes, heat) and at which age.
- Whether the field can be restored.
- Radiation as a need (`needs.md`) and its shielding rule; the leak's rate against the air and water budgets.
