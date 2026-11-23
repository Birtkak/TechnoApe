# Volcanoes and vents

- **Status:** design proposal, 2026-11-23. The developer's call is ADR 0188
  (`docs/decisions/bake-and-landforms.md`); the rest is a routine default until built and measured. It builds on
  the bake's rock provinces (the volcanic arc), `Body::heat`, the fluid rows and lava of ADR 0186 and the phases
  of ADR 0183.

## The rule

**A volcano is a place where the planet's heat reaches the surface.** Nothing is switched on by name: a chamber
of molten rock sits under it, the rock around it is hot by conduction from that chamber, and whatever that heat
does (keeps lava liquid, boils ground water, warms a spring) follows from the temperature at the place.

- **The chamber** is held at the mantle's heat by what rises under it (the arc's subduction, a hotspot's plume).
  That supply is the one thing given, as the sun's light is.
- **Around it** the rock's temperature is the rest state of conduction from a hot ball: the far temperature
  (`Body::heat` as today) plus `(T_chamber - T_far) * R / r` at distance r from a chamber of radius R, and the
  same along the conduit. One formula, read by everything.
- **Lava is rock's liquid** (`data/fluids.toml`): where that temperature is over lava's `freeze` point the rock
  is lava, so the chamber, the conduit and the crater's lake are generated lava, a fluid at rest held by a
  formula like the untouched sea (`liquids.md`, the rule at the top). Touched, it becomes counted units.
- **Kept liquid by the heat:** once phases reach rock (`liquids.md` step 5), lava carried away from the conduit
  cools and freezes to basalt, and the lake stays liquid because the conduit under it is over the freeze point.
  Until then lava never freezes anywhere, which is wrong away from a volcano and right at one.

## What is made

1. **The landform** (bake): on Terra's volcanic arcs, cones with a summit crater, a few a planet, spaced as real
   arcs are; some dead (cold chamber, a crater lake of rain water or none), some alive. Kept in the map with what
   the world needs: centre, radius, summit, crater, chamber depth and radius, alive.
2. **The inside** (world): layered basalt and tuff, a conduit from the chamber to the crater, generated lava in
   chamber, conduit and lake. A player in lava burns as in the metal sea.
3. **Vents** (after 1 and 2): where the formula puts the ground over water's boiling point at that pressure, a
   fumarole (steam, sulfur crust); where ground water runs through warm rock, a hot spring; under the sea on a
   rift, a black smoker. Their places follow the heat field and the cracks (faults already made), not a list.
4. **New rock as data:** obsidian (lava frozen fast, by water), scoria and pumice on the cone, sulfur at
   fumaroles: something to find and craft with.

## Open

- **Eruptions.** `worldgen.md` records "no eruptions and no quakes" (the developer, earlier). This design keeps
  volcanoes at rest: a lake, a glow, vents. An eruption would be the chamber's level rising (lava as counted
  units pouring from the crater) as an event; not built until the developer asks.
- Mars's shields are extinct (their chambers cold): landforms only, as today. Io-like moons later.
- A real geotherm from the interior (ADR 0162 left it open): today `Body::heat` is a gradient a province.
