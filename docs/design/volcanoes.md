# Volcanoes and vents

- **Status:** design proposal, 2026-11-23. The developer's call is ADR 0188
  (`docs/decisions/bake-and-landforms.md`); the rest is a routine default until built and measured. It builds on
  the bake's rock provinces (the volcanic arc), `Body::heat`, the fluid rows and lava of ADR 0186 and the phases
  of ADR 0183.
- **Built (2026-11-23, generation 10, map version 9):** steps 1 and 2 of "What is made": the cones, the heat field,
  chamber, conduit and crater lake of generated lava, the burn (draft entry in `bake-and-landforms.md`). Not yet:
  layered basalt and tuff of the cone's own (it is the arc's tuff with its basalt flows), vents,
  new rock; a dead crater holds no rain lake.
- **Built (2026-11-24): eruptions** (draft entry "Eruptions: a chamber fills, bursts its plug and pours counted lava"
  in `bake-and-landforms.md`): the chamber's fill, its plug, the pour of counted lava from the crater's lip, the
  ledger's `erupted`, the save's tail, `/erupt`, the report's chambers, chat lines as the warning.

## The rule

**A volcano is a place where the planet's heat reaches the surface.** Nothing is switched on by name: a chamber
of molten rock sits under it, the rock around it is hot by conduction from that chamber, and whatever that heat
does (keeps lava liquid, boils ground water, warms a spring) follows from the temperature at the place.

- **The chamber** is held at the mantle's heat by what rises under it (the arc's subduction, a hotspot's plume).
  That supply is the one thing given, as the sun's light is.
- **Around it** the rock's temperature is the rest state of conduction from a hot ball: the far temperature
  (`Body::heat`: the year's air plus the geotherm, `Body::geotherm`, a sphere's steady conduction from the
  province's gradient at the top from generation 11) plus `(T_chamber - T_far) * R / r` at distance r from a chamber of radius R, and the
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

## Eruptions (the developer, 2026-11-23: "yes eruptions too")

The same heat, not at rest. Melt keeps arriving in a live chamber from below at a rate (its row); the chamber's
pressure rises with what it holds; when it passes what the plug of frozen rock in the conduit bears, the conduit
opens and the surplus leaves as **counted units of lava from the crater**, the ordinary fluid of ADR 0186: it
runs downhill by its viscosity, pools, and freezes to basalt where it has left the heat (the first rock phase
change, on the world's heat with lava's own loss at half its pace: draft "Heat runs on two constants", 2026-11-24). The pressure falls, the plug freezes, the volcano rests.
So a cone grows by its own flows, a flow can bury a base, and what erupted is in the ledger.

- **Server-side and deterministic:** each live volcano's chamber is a few numbers in the save, stepped slowly;
  an eruption is an `Event` (journal, sound, ash and glow for clients) and edits through the ordinary queue.
- **Warning before harm:** pressure shows first (fumaroles strengthen, the lake rises, rumble), so a player can
  read a volcano and leave or build a channel.
- **Order of building:** after the landform, the inside and lava's freezing; before ash and bombs.
- **As built:** the overpressure is `bulk` x fill / volume (1 GPa), the plug bears 10 MPa, so an eruption is 1 %
  of the chamber; melt arrives at 1/300 of the chamber a game year, so one comes every 3 game years (`data/rocks.toml`
  the arc's `eruption`). It pours at 1 m3/s on the outer lip of the rim's lowest column, where a risen lake would
  spill, and ends when the melt over the chamber's rest is spent. Each chamber starts a seeded share of the way
  full, so they do not erupt together. It is poured whether a player is near or not.
- **Open:** how often and how much (the numbers above are defaults, not the developer's call); the lake's level
  following the chamber's pressure (the lake is still generation's formula, so it does not rise before an eruption);
  fumaroles strengthening, rumble, ash and glow (the warning is a chat line); explosive eruptions (gas in the melt:
  ash, pumice, bombs) after gases.

## Open

- Quakes: still out (`worldgen.md`, the developer's earlier call; only eruptions were taken back).
- Mars's shields are extinct (their chambers cold): landforms only, as today. Io-like moons later.
- The geotherm (draft "The geotherm: a sphere's steady conduction from the province's gradient", generation 11):
  built as the real curve over today's gradients (the developer's choice, 2026-11-24); a gradient from the body's
  real interior temperature over its depth in blocks was rejected for now: it boils water 40 m down.
