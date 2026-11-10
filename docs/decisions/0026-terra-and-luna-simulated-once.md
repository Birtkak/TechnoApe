# 0026 – Terra and Luna, simulated once when the world is made

- **Status:** accepted (the direction); the design in `docs/design/worldgen.md` is a proposal
- **Date:** 2026-11-10 (developer's calls in a design session)

## Context
ADR 0021 gave the planet continents from one warped noise field and named plates and erosion as "later,
if the biomes ask for it". The developer now wants realistic, in-depth world generation and simulation
for the planet and its moon, after the optimisation pass, and named them **Terra** and **Luna**.

## Options considered
1. **Simulated once.** At world creation the server runs plates, uplift, erosion, rivers, climate and
   geology over a coarse map of the whole body, saves it, and sends it to clients. Blocks take their
   detail from the map.
   - Pros: rivers run downhill, rain shadows, soils and ore have causes; ore bodies are counted, which the
     material ledger needs; far chunks read the map and need no noise.
   - Cons: a wait when a world is made; the map is world data to save and send; much more code.
2. **Noise only, richer.** More layers on today's approach.
   - Pros: instant, simple.
   - Cons: nothing has a cause; rivers and deposits are decoration.
3. **Simulated, and kept running in play** (live erosion, moving plates).
   - Cons: far more work and risk for effects rarely seen in 150 hours.

## Decision
The developer's calls (2026-11-10):
- **Option 1.** A body is simulated once; the result (the bake) belongs to the world's save.
- **Each seed makes its own Terra and Luna; one default seed is curated** by hand, shared by new players
  and the tests.
- **Live systems the world design makes room for,** beyond the water cycle and weather of ADR 0010:
  **tides from Luna, seasons (an axial tilt), and living plants.** Active geology (eruptions, quakes) was
  not chosen.
- **Real rocks and minerals:** granite, basalt, limestone; ores as minerals in the rock that forms them.

Routine technical defaults, recorded in `docs/design/worldgen.md`: the bake runs on the cube-sphere's
coarse levels; only the server bakes, so it must be deterministic on one machine, not bit-equal across
machines; rivers are lines beside the grid; the climate stage runs the air model of ADR 0010 so biomes
match the live weather.

## Consequences
- ADR 0021's `Land` is replaced when the bake lands; its land share (45 %) and planet sizes stay.
- ADR 0022's "no axial tilt" shortcut ends with the seasons stage.
- The air grid's spike (ADR 0010) moves earlier: the climate stage needs it.
- A block becomes a function of column and layer (rock layers, caves, ore), which the mesher, the physics
  and the water must read through one shared function.
- Block ids probably widen to 16 bits before the rock and ore content exists; that gets its own ADR.
- Saves grow by the bake (target under 10 MB compressed) and joining a server downloads it.
- A world keeps the bake it was made with; generator changes only affect new worlds.
- Staged plan with its spikes and open taste calls: `docs/plans/worldgen-plan.md`.
