# Plan: building Terra and Luna

For future Opus coding sessions. The design is `docs/design/worldgen.md`, the decisions ADR 0026; read both
first. One stage per session. Starts after the optimisation pass (`optimization-handoff.md`), whose
package E (the height cache) every stage here builds on.

## Rules for every stage
- **Spike first** (CLAUDE.md, principle 1): a throwaway in `prototypes/`, its README stating the question
  and the measured answer, before any engine code.
- **Show before building on it:** sheets of images for anything with a look. The developer picks.
- Each stage ends with its ADR, its tests, a benchmark row, and `tools/views.sh` accepted after the
  developer has seen the new views.
- The budget of ADR 0004 holds after every stage, and so does the optimisation pass's result for `stand`.
- The soak stays deterministic: same seed, same bake, same final hash, twice in a row.
- Stop for the taste calls listed in `worldgen.md`; each stage below names the ones it needs.

## Stages

### W0. Names (an hour; any session)
- Terra and Luna in the docs, the bodies' data, chat and commands (`/goto terra`, `/goto luna`), the F3
  overlay and bug reports. The block "moon dust" becomes "regolith" by name only: its id stays.
- Bodies become rows of data (fundamentals, section 2: "bodies are data") if they are not yet: name,
  size, generator, air, orbit.

### W1. Prototype: the bake of the land (`prototypes/world-bake/`)
- Stages 1 to 4 of the design on the cube-sphere's coarse grid: plates, boundaries, uplift against
  stream-power erosion, sea level, the river network and lakes.
- Output: an image sheet per seed (height, plates, rivers, from six sides) and an orbit view.
- Questions for the README: seconds for Terra and for the huge planet on the dev PC, per stage; memory;
  the same hash twice; do the seams and the 8 cube corners show in the map?
- **Taste calls:** relief (how high, how deep), the wait, and the default seed from a sheet of 12.
- Done when the developer has picked a look and the time fits the budget, or the README says what does
  not.

### W2. The bake in the engine
**Status (2026-11-11, ADR 0043):** the map, its mips, heights from it, the height cache, far chunks, the
spawn by score, the save (by hash) and the download in pieces are built, with W3's biome ground and still
plants; progress on the title screen too (2026-11-12, ADR 00XX: a new world's bake and a server's map
under a bar, on a worker thread). Not yet: river lines and beds, detail by rock and slope, the walk
test; `Land` stays for saves from before the bake.
- `engine::bake`: the map with its mips, the river lines, the save format, and its download in the
  protocol (compressed, in pieces, with progress on the title screen).
- Heights from the map: smooth interpolation plus detail driven by rock and slope; river beds cut from
  the lines. `Land` goes.
- The height cache's tiles are filled from the map. Far chunks read the mips.
- The spawn by score (stage 11).
- Checks: a walk test (a bot follows a river from spring to sea and never climbs); `fly` and `orbit`
  with no holes; mesh job time per level against the optimisation pass's numbers.

### W2b. The globe map (after W2; `docs/design/navigation.md` part 3)
- The bake's coarse map drawn as one textured sphere the player turns, zooms and clicks to set a waypoint
  (ADR 0030 has the coordinates, the compass and the waypoints it builds on).
- It fills in as the player sees the world: a seen-mask per coarse cell, kept beside the save.
- Spike first: the globe from the bake's mips in a frame, and what the seen-mask costs to keep.
- **Taste calls:** is the map a tool to be made; does one player's map show to friends; its look.
- Checks: a filmstrip of the globe turning; a bot walks a line and only that band is drawn.

### W3. Air, climate, seasons, biomes
- **First the spike ADR 0010 already asks for:** the coarse air grid's tick cost on Terra (target under
  0.5 ms of the server tick).
- The same model run to a steady year inside the bake (stage 5); biomes, soil and surface blocks from it
  (stages 7 and 8): sand, gravel, clay, snow, ice, the grasses.
- The axial tilt in `sky.rs`, the sun's path and day length by season; temperature in the F3 overlay.
- **Taste calls:** tilt and season strength; the biome list and each biome's look.
- Checks: the bake's mean rain against a year of live weather, per biome (within a set tolerance);
  views at four seasons.

### W4. Rock, caves and ore
- **First:** block ids to 16 bits (its own ADR), and one shared `generated block at (column, layer)`
  for `generate`, `phys::blocked` and `Solid::gaps`, with a test that the three agree on a cave.
- Rock provinces and folded layers (stage 9); caves by rock; the water table; heat with depth.
- Ore bodies as counted objects (stage 10), gravels and surface signs; the **material ledger per body**
  (roadmap) opens with their counts, and the soak checks it every tick as it does the water's.
- **Taste calls:** how deep the ground goes; what the player is told (rock names, a map).
- Checks: per default seed, a report of every mineral's tonnage, depth and distance from the spawn,
  against what `progression.md` needs per tier; a bot digs a body out and the ledger reaches zero.

### W5. Rivers, lakes and tides
- Spike: a river reach at rest described by the bake, live water when disturbed, the ledger across the
  hand-over (ADR 0020's still sea is the model).
- Spike: a sea level that moves with Luna, in the shader, the physics and the waterline rule.
- Then both in the engine; rivers fed by the live rain once W3's air grid runs.
- **Taste call:** tide height.
- Checks: the water ledger over a soak with dams built and broken; a tide filmstrip over one day.

### W6. Luna
- **First:** Luna at N = 3072 (1,061 m, ADR 0027) and its orbit moved out (proposal 30 km): `sky.rs`,
  the `orbit` benchmark's trip, the views. **Taste call:** the orbit's distance, by how Luna looks from
  Terra and how wide a total eclipse's band is.
- Its own bake (the design's Luna table): highlands, maria toward Terra, craters by size and age,
  regolith depth, lava tubes, its ores. Shadow at the poles computed, then the ice decided.
- **Taste call:** Luna's ice; the look from Terra's surface (the maria are its face) and from orbit.
- Checks: views from Terra at full moon and from low Luna orbit; crater counts by size against the power
  law the README states.

### W7. Living plants
- After fine cells and felled trees (roadmap). Spike the coarse plant grid's tick cost for the whole
  planet, then growth and spread, then plants as blocks near players.
- Checks: a soak where a bot clear-cuts a patch; cover returns within the set time and the ledger of wood
  holds.

## Designs each stage reads
| Stage | Design |
|---|---|
| W2, W4 | `docs/design/world-data.md`: the column record and runs, 16-bit ids, the three readers' test, the ledger |
| W3 | `docs/design/air.md`: the grid, its passes, the spike's questions |
| W4 | `docs/design/ore-budget.md`: stock per mineral, distances from the spawn, the per-seed report |
| W5 | `docs/design/rivers-and-tides.md`: the reach graph, a reach at rest, the tide function |
| all | `docs/design/open-questions.md`: the taste calls, decided and open |

After the world: `docs/design/networks.md` (shafts first) and `docs/design/crafting.md` (the starter).

## Order and why
W0 any time. W1 and W2 first: everything reads the heights. W3 before W4 because bauxite, soils and
gravels follow climate and rivers. W5 needs W3's rain. W6 is independent of W3 to W5 and can run beside
them in its own worktree. W7 last.
