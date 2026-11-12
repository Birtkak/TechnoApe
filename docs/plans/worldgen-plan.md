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
plants. River lines and dry beds (2026-11-12, ADR 0056) with the walk test on the engine's heights; water
in the beds waits for W5. Progress on the title screen too (2026-11-12, ADR 0057: a new world's bake and a
server's map under a bar, on a worker thread). Detail by slope, biome and height (2026-11-12, ADR 0058:
smooth plains and calm coasts, rough craggy slopes; W4's rock plugs into `bake::rough`). Not yet: a bot
walking a river;
`Land` stays for saves from before the bake.
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
**Status (2026-11-12, ADR 0042, 0043, 0055):** the climate and the fifteen biomes are baked; the sky has its
tilt (25 degrees since ADR 0060, open) and seasons over the 54-day year, `/time set summer` and the others, and F3 shows the
air's temperature (the biome's yearly mean with the season and hour on top). **Done (2026-11-12, ADR 0060):**
the tick spike (`prototypes/air`), the bake's climate over a year with the sky's seasons, the map keeping its
climate (each cell's baked temperature, map version 3), the live air grid on the server (10 s steps on a
worker, in the save and the soak's hash, the air at a player's feet over the wire, runoff per column for W5),
seasons through the whole air (the upper air's balance and the wind bands follow the sun; the spawn's noon -2 C in winter, 28 C in summer), the rain check (Terra within 0.3 % per biome, tolerance 5 %) and four season views in `tests/views/`. Waiting:
the developer's look at the views (the old ones moved with the biomes) and the open taste calls below.
Visible weather is the roadmap's next step.
- **First the spike ADR 0010 already asks for:** the coarse air grid's tick cost on Terra (target under
  0.5 ms of the server tick).
- The same model run to a steady year inside the bake (stage 5); biomes, soil and surface blocks from it
  (stages 7 and 8): sand, gravel, clay, snow, ice, the grasses.
- The axial tilt in `sky.rs`, the sun's path and day length by season; temperature in the F3 overlay.
- **Taste calls:** tilt and season strength; the biome list and each biome's look.
- Checks: the bake's mean rain against a year of live weather, per biome (within a set tolerance);
  views at four seasons.

### W3b. Seasonal looks (after W3; `docs/design/seasonal-looks.md`)
**Status (2026-11-12, ADR 0061):** the spike (about 0.03 ms of GPU, no CPU), the season rows in
`data/blocks.toml`, the four-row look table and the shader's mix are built, with colours on the
grasses, broadleaf leaves, shrub and reed that the developer accepted (2026-11-12). In work on
`claude/w3b-season-looks-d39e67`: painted season files, the Studio's season tabs and year slider, the hotbar
and the block in hand.
- The developer's call (2026-11-12): grass and leaves stay one block each, their look follows the season,
  and the Studio paints a block's seasons. Nothing stored, no remesh: the shader mixes four looks by place
  and clock.
- Spike first: the shader's cost on `stand` and `orbit`.
- **Taste calls:** each season's colours from a sheet; how long autumn's colour lasts.
- Checks: a year's filmstrip at the spawn and from orbit; views without a season block unchanged.
- Not in it: snow on the ground (weather, its own design), bare trees (W7).

### W4. Rock, caves and ore
**Status (2026-11-12, ADR 0062):** the first point is on main (ids are 16 bits everywhere but the shader's 8
bits of material; `Body::col_caves` answers the mesher, the physics and the water alike). **Calls (the
developer, 2026-11-12):** the ground goes 192 m deep, hot and undiggable below for now; rock and mineral names
on the "looking at" label, no map, ore found by eye; a palette draft per rock on a look sheet before any is
accepted; `ore-budget.md`'s numbers kept per 1 m block. The rest is in work on
`claude/w4-rock-caves-ore-d4c157`.
- **First:** block ids to 16 bits (its own ADR), and one shared `generated block at (column, layer)`
  for `generate`, `phys::blocked` and `Solid::gaps`, with a test that the three agree on a cave.
- Rock provinces and folded layers (stage 9); caves by rock; the water table; heat with depth.
- Ore bodies as counted objects (stage 10), gravels and surface signs; the **material ledger per body**
  (roadmap) opens with their counts, and the soak checks it every tick as it does the water's.
- **Taste calls:** how deep the ground goes; what the player is told (rock names, a map).
- Checks: per default seed, a report of every mineral's tonnage, depth and distance from the spawn,
  against what `progression.md` needs per tier; a bot digs a body out and the ledger reaches zero.

### W5. Rivers, lakes and tides
**Calls (the developer, 2026-11-12):** rivers and lakes first, tides after; the tide is 1 m low to high at
springs. A reach's runoff is the bake's mean rain over its catchment; W3's air grid is live since ADR 0060,
and feeding the reaches from its runoff is open (ADR 0059 says what it takes).
- Spike: a river reach at rest described by the bake, live water when disturbed, the ledger across the
  hand-over (ADR 0020's still sea is the model).
- Spike: a sea level that moves with Luna, in the shader, the physics and the waterline rule.
- Then both in the engine; rivers fed by the live rain once W3's air grid runs.
- **Taste call:** tide height.
- Checks: the water ledger over a soak with dams built and broken; a tide filmstrip over one day.
- **Status (2026-11-12, ADR 0059): rivers and lakes done; tides open.**
  - Generated water stands in beds and lakes; touched water goes live and is forgotten again.
  - The reaches pass what they take in down to the sea. W3's live runoff feeds them by catchment, taken out
    of the air's sea term (both ledgers exact in the soak); rain in a reach raises its live water on top of generation's dry-weather base (save version 12).
  - Maps are version 4 (climate and lakes; 3, 2 and 1 still read), saves version 9 (8 with its air still reads); bots dam rivers in the soak with the ledger exact.
  - A walker follows a river from spring to sea in water.
  - Open: the look (the developer's call), joining W3's air ledger (both are on main now), and tides.

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
