# Rock, minerals and ore

Rock by province, the ore bodies and their ledger, the changed rock around ore, ore samples and prospecting,
pressure and the rocks and minerals that form by it, and the generation numbers. What holds today. Numbers in
brackets are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number. Caves: `caves.md`.

## The developer's calls
- The ground goes 192 m deep (the crust), hot and undiggable below for now (later: the mantle, below). Real rock and
  mineral names on the "looking at" label; no map; ore is found by eye from gravels, stains and plants. One palette
  draft per rock, shown on a look sheet before any is accepted. `ore-budget.md`'s numbers are kept per block. [0063]
- Every rock and mineral must be something to find, mine and craft with; real-world logic decides where it is. [0063,
  0158, 0166]
- Nothing in generation is placed for where the spawn is, except copper (2026-11-18): from generation 2 only the
  copper kinds keep a ring round the spawn. [0160]
- Ore must be something to look for: loose samples on the ground above every body and a prospecting strike that says
  what is near and how strong (2026-11-16). [0138]
- A body dug almost empty must stop ringing rich from inside its own void (the developer's report, 2026-11-16). [0138]
- Pressure is part of the world's model (2026-11-18); rocks and ores form at depths set by pressure and heat. The
  content (rocks, colours, hardness, diamond) is a draft for the developer; the lower mantle (bridgmanite) is `later`
  (no pick can dig it yet): a taste call. [0158]
- Open calls the halo ADR left for the developer: new block rows over a look on the host (the host is not seen in a
  halo); two grades a mineral and the reaches (tuned by playing); coal without a halo; whether the halo should also
  show where the 4 m soil band is thin (cliffs, river cuts); the quartz-veined rock's painting should carry its veins
  (the placeholder reads as mottled rock). [0095]
- The rocks' and ores' looks are drafts, back to the developer. [0063]

## Rock
- **The deep crust.** `world::deep_layers(n)` adds whole layers under the old 64 m crust until it is at least 192 m
  (none under 768 m of radius, so the small planets keep theirs). The surface keeps its radius; every layer index
  moves up by that many. Below layer 0 nothing is diggable (`Shape::fits`). [0063] (The mantle later put diggable
  ground below this floor on bigger worlds: `deep-world.md`, ADR 0154.)
- **Provinces** (`bake::rock`, stage 9): shield, basin, folded belt, volcanic arc, rift, ocean floor, from the plates
  (distance to collisions, arcs, rifts; old sutures are folded belts too). The map carries a `rock` byte per cell;
  a map without one has no rock, no caves, no ore. [0063] (Luna's provinces: Luna section below.)
- **The rock rule** (`data/rocks.toml`, `rock::ROCKS.at`): each province's layers top down from a datum 60 m over
  the sea, folded by noise of the province's amplitude and wavelength. The mesher's fill (`generate`, from the tile's
  `rock` per column) and `generated_under` ask the same rule under the 4 m soil band. `Body::rock` is the one rule
  the mesher, `generated_under`, physics and the ledger ask (ADR 0062's one generated block). [0063, 0158]
- **Basalt flows** (generation 2): a province row may state `flow = { rock, share, thick, width }`
  (`rock::Flow`). The volcanic arc does: `basalt`, share 0.5, 30 m thick, fields some 700 m across, lying from the
  ground down (not from a datum). `Body::rock_at` puts the flow's foot radius in `Rock::flow` where a slow noise of
  the seed is over a cut that leaves `share` of 4096 directions over the sphere (worked out once a map load);
  `Rocks::at` returns the flow's rock over it. Every reader already asks `Rocks::at`. The fields are isotropic
  (stretching them downslope was left out). No mineral is hosted in tuff, andesite or basalt, so the arcs' ore is
  where it was; gravel and rubble go by the block dug, so a flow gives basalt's. [0160]
- **Heat** (`Body::heat`): the bake's year mean at the ground plus the province's `heat` a metre down (13.6 C at the
  spawn's ground, 42.8 C at the crust's floor, seed 1337). [0063]
- **The mantle's rocks (pressure and heat).** `data/rocks.toml`: `density` (the crust's, 2700) and a `[mantle]` table
  with `density` (3300), `wobble` (metres) and `[[mantle.rock]]` rows top down, each `block`, `pressure` (MPa),
  `heat` (C), `slope` (MPa a degree). A row takes over, going down, where the column's pressure reaches
  `pressure + slope * (T - heat)`. The first row ends the crust. Checks at load: pressures rising row by row,
  0..1000 MPa, slope within +-0.04 MPa a degree (so a column crosses each boundary once), heat -50..2000 C, densities
  1000..20000, wobble 0..20 m. `Body::rock` asks it under R - 192 m + 64 m (`MANTLE_LIFT`) on a body with a mantle.
  The pressure there is the sea-level column's (`rock_weight(R - 4 m, r)`: isostasy), the heat the province's
  gradient under the column's year air (`Rock::air`, filled with `Rock::wobble` by `rock_at` on a mantle body only);
  each boundary moves by the wobble (fold noise at 600 m width, +-4 m). A boundary steps where a province does. [0158]

  | Rock | Block, rubble | Where | Hardness, grade |
  |---|---|---|---|
  | peridotite (upper mantle) | 58, 191 | 10.07 MPa at 42 C, slope -0.04: the crust's floor, thinner under hot rifts | 10, 1 |
  | ringwoodite (transition zone) | 59, 192 | 33.0 MPa at 95 C, slope +0.03 | 12, 2 |
  | bridgmanite (lower mantle) | 60, 193 | 56.7 MPa at 148 C, slope -0.03 | 15, 3 (`later`) |

  Seed 1, 4 km planet, under the spawn: crust floor 192.9 m, ringwoodite 544.9 m, bridgmanite 898.0 m, mantle floor
  1246.2 m. The crust's floor is no longer one radius: anything that assumed exactly 192 m asks `Body::rock`.

## Pressure
- `Body::pressure(face, i, j, z) -> Option<f32>` (beside `Body::heat`, None without a map), MPa: one atmosphere
  (0.101325) at the top, plus fresh water (1000 kg/m3, at the surface g) from the water's top down to the ground
  (`water_at`), plus the rock from the ground down: the crust's density above R - 192 m, the mantle's under it, with
  the gravity inside the body of ADR 0152 integrated in closed form (`world::frames::pull_integral`). In the game's
  doubled gravity (19.62 m/s2): about 0.053 MPa a metre in the crust, 0.065 to 0.073 in the mantle; 10.35 MPa at the
  crust's floor and 81.7 MPa at the mantle's floor under the 4 km planet's spawn. Generated only: digging does not
  change it. The planet's surface g is used on every body (a shortcut until Luna's own deep). [0158]
- Nothing reads the field yet except the mineral windows (air, liquids, collapse, suits are the next steps). [0158]

## Ore bodies
- Ore is counted objects placed from the map and the seed when the planet loads (`ore.rs`, `data/minerals.toml`),
  the same on the server and every client. Kinds have a shape (lump, vein, seam, layer, blanket, cap, bog, gravel),
  the budget's count and blocks per body (scaled by area on other planets, at least one), depth, provinces, host
  rock, biomes, a ring from the spawn (copper only from generation 2, below), a gravel kind in the stream bed below
  and a gossan blanket over deep ones. A body is an ellipsoid solved from its blocks; grade (ingots per block) is
  3, 2, 1 from its middle out. **One rule** `Body::ore_at` (ground, not a cave, first body wins) is what
  `generated_under`, `generate` at level 0 and the count ask. [0063]
- Ore blocks are their own blocks (malachite, chalcopyrite, ...), not flecks over the host rock; the sign blocks
  (copper, tin and gold gravel, gossan) are loose. A `host` on a ground-following kind is refused. [0063]
- Caves are made before ore; a cave cell is never ore. [0063, 0159]
- **Minerals with a window.** Optional `pressure = [min, max]` (MPa) and `heat = [min, max]` (C): a body is placed
  only where `Body::pressure` and `Body::heat` at its middle's column and layer lie inside both; its host rock is
  checked by `Body::rock` (the mantle's rocks). Such a kind may lie 1400 m deep on a planet with a mantle only, is not
  cut at the crust's old floor (`OreBody::span`), and has no halo, gossan or gravel. Kinds with a window come last in
  the file (and the gossan's kind is placed before them), so the crust's kinds keep their indices. [0158]
- **Diamond** (ore, block 61, rubble 194): 8 lumps of 60 blocks, 560-900 m down, 36-56 MPa and 0-110 C, host
  ringwoodite; no rift or arc; ingots in the report are gems by the ore grade rule. No item, recipe or use beyond its
  rubble. Seed 1, 4 km: 8 bodies, 481 blocks, 595.6-695.0 m down, nearest 985 m from the spawn. [0158]
- **Only copper near the spawn (generation 2).** `near` stays on copper gravel and deep copper; the rings of tin vein,
  bog iron, banded iron, coal seam and lead pocket are `near_before_2` (a row has one or the other), which
  `ore::Kind::ring(gen)` applies below generation 2 only. The spawn keep-out (`ore.rs` `place`'s `fits`, `SPAWN + a`),
  `caves.rs` `Probe::off` (64 columns) and the older worms' 100/40 columns stay. [0160]
- **Ore per body.** A kind's body (`ore::Kind::body`) is its provinces' (`rock::Province::body`); a row whose
  provinces are two bodies' is refused; a kind is made only on its own body. The spawn keep-out and rings apply to the
  home's kinds only (another body's row with a ring is refused). Counts are on the kind's body's standard size:
  `count * (n / bodies::columns(body, 6144))^2`. Order: another body's mineral rows come after the home's (after the
  deep kinds); the parser refuses a home row after another body's. Gravel kinds need rivers (none on a body without),
  deep kinds need a mantle. [0166]
- The budget lives in `report.rs` (`BUDGET`) until `data/budget.toml` exists with the recipes. [0063]

## The ledger
- `World::ore_left` is one list: every world body's ore bodies in body order (the home's first, so its indices are
  as before); `World::ore_base(body)` is where a body's part starts. The server opens it with the exact count before
  a save's edits (`open_ledger` counts every body), and `set_block` / `put_edit` move it when a body's own ore leaves
  or comes back to one of its cells (`tally` and `ore_recount` book at `ore_base + k` against that body's own ore
  block; `ore_cell` answers on any body). The soak checks every tick that it equals a recount from the edits
  (`Server::ore_ledger`). Clients keep none. The ledger is never saved or sent: no format or version change. [0063,
  0166]
- A dig of an ore block takes it from its body; ore in rubble is counted as the body's until carried off (ADR 0093,
  outside this doc's list).
- `technoape-server report=1 seed= planet=` prints provinces, caves, heat, every element's and kind's bodies, blocks,
  ingots, budget, nearest distance and depth, and `nearest_at` (the nearest body's direction, for `run=stand at=`);
  each kind with a halo lists `halo_blocks` per grade; each kind with a sample gives `"samples": n`; the `deep` field
  gives the mantle's depths; Luna's part follows Terra's as `"luna": {n, made_hash, provinces, baked_tubes, caves,
  minerals}`; a kind's cave line also gives `longest_m`. `made_hash` and `made_hash_crust` (caves and the crust's
  kinds' bodies) are printed. [0063, 0095, 0138, 0158, 0166]
- Cross-platform risk (not proven): caves and bodies are made with the platform's `sin`, `cos`, `exp`, `ln`; one
  ulp can add or drop a cave or body between a Linux server and a Windows client. `made_hash` is the check (run it on
  the server before a remote game); the fix, if they differ, is a pure-Rust `libm` for these paths or the server
  sends the lists. Halo cells share this risk. [0063, 0095]

## Alteration halos
- Six block rows (ids 100 to 105): iron-stained rock, copper-stained rock, quartz-veined rock, greisen, bleached
  rock, dolomite. A new row, not a look on the host: an altered rock is a different rock. [0095]
- Per mineral (`halo`, `halo_m` in `data/minerals.toml`, grades outermost first):

  | Kind | Host | Outer grade | Inner grade | Reach |
  |---|---|---|---|---|
  | deep copper (chalcopyrite lump) | arc, belt | iron-stained rock | copper-stained rock | 8 m |
  | tin vein (cassiterite) | granite | quartz-veined rock | greisen | 4 m |
  | banded iron (hematite layer) | shield | bleached rock | iron-stained rock | 6 m |
  | lead pocket (galena) | limestone | dolomite | iron-stained rock | 4 m |
  | gold vein (gold quartz) | belt | bleached rock | quartz-veined rock | 3 m |
  | uranium (pitchblende) | granite | bleached rock | iron-stained rock | 3 m |

  Coal and the bodies under the ground (gravel, cap, bog, blanket, gossan) have none. [0095]
- **The rule** (`OreBody::halo_grade`, `Body::halo_at`): a cell is in a halo when it lies inside the body's ellipsoid
  with each half axis grown by `halo_m` and not inside the body; grade g reaches `halo_m (1 - g / grades)` out, so
  the richest grade is next to the ore. Each cell's reach is frayed by a hash of the cell (`FRAY`, 15 % either way);
  where two grades meet they mix over 30 % of the reach. Where two halos meet the higher grade wins, the first body on
  a tie. A cell goes to the body's frame once (`OreBody::local`). [0095]
- Asked only of rock under the soil band (4 m) that is no ore and no cave: `World::generated_under`, the mesher's
  fill (`generate`, level 0, before the ore), `ore::count_halo`. Halo rock holds no metal and is never `ore_cell`;
  the ledger is untouched. `World::set_ores` registers a body as wide as its halo reaches (`HALO_MAX` 12 m caps
  `halo_m`). `count_halo` and the mesher's padding ask a face's own columns only (bodies keep 6 columns from a seam,
  halos reach up to 9 m). Halo rock has its row's hardness (these 6 to 9; granite 9) and drops itself when dug. [0095]
- The halo lies under the soil band, so it shows in a dug face, a cave or a cut; at the surface a deep copper or
  banded iron body is signed by its gossan. [0095]
- The rule is part of what a client derives from the map: client and server must be one build (no protocol number).
  Placement of bodies did not change (`made_hash` did not move). [0095]

## Ore samples and prospecting
- **Samples.** A `[[kind]]` of `data/minerals.toml` may name `sample = "<item>"`: copper cap and deep copper the
  copper nugget, tin vein a tin pebble, banded iron an iron pebble, coal seam a coal lump, lead pocket a lead pebble,
  gold vein a gold nugget, uranium a pitchblende pebble (stack 16, `data/items.toml`). Each has a pick-up row in
  `data/clutter.toml` (`on = []`) and a model in `data/models/` (`tools/paint_clutter.py`). `clutter::sample` rolls per
  column each body in the column's tile whose spread covers it: the body's footprint from above, each half axis at
  least `SAMPLE_SPREAD_M`, with chance `(1 - q) / SAMPLE_EVERY_M2` (q: 0 at the middle, 1 at the rim, squared), so
  densest over the middle, none at the rim, however deep the body. `pick` asks it before the biome's roll: a sample
  is a pick-up like flint (deterministic from seed and column, never under water, taken once, drawn by the mesher,
  checked by the server). A sample holds no metal: the ledger, `made_hash` and ore counts do not move
  (`shortcut:` nuggets become smeltable with the copper age's recipes). `items::Items::parse` refuses a sample that
  names no item. A pick-up taken at a column before this now hides the sample there. [0138]
- **Prospecting.** A `[[material]]` has `prospect = <metres>` (flint 16, 0 to 64, checked at load). In survival the
  place button with a tool of kind `pick` or `any` whose head reaches, on hard rock (`items::knaps`: rock, ore, the
  altered rocks), is a prospecting strike: at most one a second a player, a use of the tool; it reuses
  `ToServer::Strike` (flint in hand knaps, a prospecting tool prospects). The server answers that player only
  (`ore::prospect`, new `ToClient::Prospect(Vec<(element, band)>)`): per element with ore left in the ground on the
  struck face, the distance d from the cell to its nearest body's ellipsoid surface (exact, `ore::to_surface`; 0
  inside) gives a band against the range R: rich under R/8, strong under R/3, some under 2R/3, traces under R; the
  three strongest, no direction. The client shows "copper: strong, tin: traces" or "the rock rings empty" in the chat,
  chips the face and plays the knock, higher the stronger the best band. The bot's `prospect dx dy dz` prints
  `{"bot":..,"prospect":[["copper","strong"]]}`. [0138]
- **A mined body fades.** The distance is to the body as generated, but its band drops with the share of its ore
  still in the ground (ledger `left` over the opening count `ore_open`): at 1/2 or more unchanged, under 1/2 one band
  down, under 1/8 two, under 1/32 three; a band below traces is no answer. No wire or save change. [0138]
- A strike hears only the struck face's bodies (bodies keep 6 columns from a seam); a body across the seam is not
  heard. [0138]

  | Number | Value | Where |
  |---|---|---|
  | Samples a square metre at a body's middle | 1 / 40 | `ore::SAMPLE_EVERY_M2` |
  | Least half width of a body's sample spread | 6 m | `ore::SAMPLE_SPREAD_M` |
  | A flint head's prospecting range | 16 m | `data/items.toml` `[[material]] prospect` |
  | Longest range a material may have | 64 m | `items::PROSPECT_MAX` |
  | Seconds between answered strikes | 1 | `items::PROSPECT_WAIT_S` |
  | Band edges | R/8, R/3, 2R/3, R | `ore::band` |
  | Share left under which a body drops one, two, three bands | 1/2, 1/8, 1/32 | `ore::FADE` |
  | Elements an answer names | 3 | `ore::PROSPECT_NAMES` |
  | The knock's pitch | 0.85 empty, 1 + 0.12 (band + 1) | `crates/client/src/frame.rs` |

## Luna's rock and ore
(also `bodies-and-orbits.md`, `bake-and-landforms.md` for the rock field itself, ADR 0165.)
- From generation 4 Luna has a rock field and `Body::rock_at` answers there (before 4 it answers `Rock::NONE`).
  `Body::rock_field()` (a map with a rock layer, or Luna's map from generation 4) is what `ore::place`, `caves::make`
  and `caves::systems_of` ask; `Body::rock_body()` names the row whose provinces the field holds. [0166]
- A kind's body is read from its provinces (no `body` key on mineral and cave rows). Terra never runs Luna's rows and
  the reverse. Nothing on Luna depends on her landing point. [0166]
- No sea, no shore, no water table on Luna: `with_map` lays no sea on a Luna map before her caves and ores are made;
  `ore::place` and the cave walks keep their `DRY`/2 m over the sea datum only on a body with a sea (`caves::shore`);
  `caves::water_table` is minus infinity on a body without a sea; a karst kind is not made there. [0166]
- **Luna's minerals** (`data/minerals.toml`, the last three rows). Blocks 220 and 221 are new rows at the end of
  `data/blocks.toml` (rubble 195, 196), picks of grade 1, a speckle pattern, no texture yet. [0166]

  | Kind | Block | Where | Depth | Standard Luna (seed 1) |
  |---|---|---|---|---|
  | ilmenite bed (layer) | ilmenite (220) | mare, host mare basalt | 8 to 30 m | 10 bodies, 57,876 blocks |
  | merrillite pocket (lump) | merrillite (221) | kreep terrane, host kreep | 6 to 16 m | 6 bodies, 2,998 blocks |
  | meteoritic iron (lump) | iron-nickel (204, ADR 0070's) | highland crust, under the `metal crater` kind, below its 3 m lens | 6 to 9 m | 3 bodies of 12 asked, 907 blocks |

  Ingots on the standard Luna (seed 1): ilmenite 119,970, merrillite 6,320, meteoritic iron 1,932 (the metal craters
  are few, so 3 bodies of 12 asked). [0166]

  Uses (named, no recipes): ilmenite titanium and iron and oxygen when reduced with hydrogen; merrillite thorium and
  the rare earths; iron-nickel nickel and iron. The KREEP block stays the terrane's rock (not counted ore). [0166]
- Luna's lava tubes: a `caves.toml` row; see `caves.md`. [0166]

## Generation number
`world::GENERATION` is **5** (checked in code, `crates/engine/src/world/mod.rs`; `known_generation` takes up to 5 and
refuses 6 with "this world is generation N, this build knows up to M: update the game"). `Body::gen` is set on every
body (before 4 only the home's was). A rule asks `b.gen >= k`; a future rule bumps `GENERATION` and keeps the old
profile for older worlds. [0160, 0165]

| Gen | What it is | What it changed |
|---|---|---|
| 0 | worlds before the mantle (`mantle=0`, a save of version 19 or older, small planets) | the crust only: 192 m, one rock in a layered province, caves and ore of ADR 0063 |
| 1 | the mantle's worlds (ADR 0154, 0158, 0159) | the grid goes down through the mantle; mantle rocks by pressure and heat; diamond; the cave kinds of `data/caves.toml` (lava tube, fissure; the old tube worm is not made on them) |
| 2 | ADR 0160 | basalt flows on the volcanic arcs; round lava tubes; only copper (rings) near the spawn |
| 3 | ADR 0161 | karst (the limestone worm is not made where karst replaces it); `Body::candidate_of` plant check cheaper (same answer) |
| 4 | ADR 0165, 0166 | Luna's rock field; her ore and caves by her rock (0166 redefines generation 4's Luna: no gen-4 world had been handed over); no change to Terra |
| 5 | bodies-and-orbits.md "Mars is a body in the world" | Mars, the third body (a row's `since = 5`, `bodies::of`); Terra and Luna as at 4 |

How it is carried (ADR 0160):
- In the save: `Save::gen: u8`, `VERSION` 21 at the time (a 20's `mantle: bool` is the same one byte: it reads as
  generation 1 with the mantle and 0 without; a 19 or older reads as 0). The save is now version 22 (ADR 0163,
  `Save { bodies, .. }`; see `multiplayer-saves-and-data.md`).
- On the wire: `ToClient::Welcome { id, seed, bodies, blocks, items, gen }`; the client and the bot build their
  world of it; refused when newer than the build (`world::known_generation` on a save read, `Server::from_save`, the
  client's and bot's welcome). `gen=` with `mantle=0` and a generation over 0: `gen=` wins, and a line says so.
- In recordings: `Ev::Gen(g)` after `Start` for a world from the seed of generation 2 or later (generation 1 still
  writes `Ev::Mantle`); a replay takes it; one from a save: the save says it.
- By argument: `gen=k` on the game, the server and `report=1` (`server::gen_arg`; `mantle=0` is `gen=0`);
  `report=1` prints `gen`.
- A cave row says the generations it is made in (`generations = [least, most]` in `data/caves.toml`); a row's place
  in the file seeds it, so rows are appended and an old row is given an end, never changed.
- The mantle itself stayed on the flag `b.mantle > 0` (the same set of worlds as `gen >= 1`; the comments point at
  `gen`), because that code is locked by hashes.

Locked hashes (`technoape-server report=1 seed=1 planet=6144` `made_hash`):

| `gen` | `made_hash` (Terra) | Source |
|---|---|---|
| 0 (`mantle=0`) | 0926f028ddcd85a7 | 0158, 0159, 0160 |
| 1 | 37b2d1b1006b66f7 | 0159, 0160 (f283474fb9f15012 before the cave kinds, with diamond) |
| 2 | a2ef250b3ed4203a | 0160 |
| 3 | bfb878ce14d32b75 | 0161 |
| 4 | bfb878ce14d32b75 (Terra the same at 3 and 4) | 0165, 0166 |
| 5 (default) | bfb878ce14d32b75 (Terra the same at 3, 4 and 5); Mars's `made_hash` cbf29ce484222325 (nothing made) | Mars |

- `made_hash_crust` (caves and the crust's kinds' bodies on a mantle world): 9b209f3e64fcf265. [0158]
- Luna's `made_hash` (report's, seed 1): gens 0 to 3 c6a367341bda1227 (her baked tubes, no ore); gen 4
  cdd810732c670a46 (and 5). Luna's blocks, small Luna (`luna_blocks_hash`): gens 0 and 3 a22a376de6be21ba; gens 4 and 5
  b0f00158257f21d2. Mars's blocks at generation 5 (`mars_generates_as_locked`, n 832): 3b6b6a314af6caa5. [0166, 0167]
- The regression tests `generations_0_and_1_generate_as_before` (extended to 0, 1, 2 by `generations_0_1_and_2_..`
  in ADR 0161) lock caves, ores and rock of the 1536 planet from main's code before each change. [0160, 0161]
- Also held by other ADRs: ADR 0062 (one generated block), 0070 (Luna's bake), 0162 and 0163 (checked: they keep gen
  3 and the same hashes, no new number), 0165 (gen 4), 0167 (the Mars bake: its map is version 201), Mars in the world
  (bodies-and-orbits.md: **generation 5** adds the third body). Grep result: no ADR outside 0160, 0161, 0165 and
  Mars's changes `GENERATION`.
- Old ADR 0063 recorded `made_hash` 8ecb5dadddbbf9b5 (seed 1337, 4 km), and ADR 0138 recorded 2a79455bb6161781 on
  seed 1337; both are earlier numbers of that seed and not locks.
- Saves of version 20 made with the mantle changed under their owners' feet where not dug (the mantle's rocks, the
  crust's floor, the diamond bodies appended to the ledger) with no new version; accepted. [0158]
- Old saves of 0063's time: save version 10 (an older save's edits and water gaps move up by `deep_layers`, test
  `a_save_from_before_the_deep_ground_moves_up`) and map version 5 (`rock` byte per cell; older maps load with none).
  Both have been superseded by later versions (`multiplayer-saves-and-data.md`). Recordings from before the deep
  crust do not replay. [0063]

## Open
- A rule change to rock, caves or ore is a protocol change (the map hash covers the map, not the rules): client and
  server must be one build. [0063, 0095, 0138]
- Rock is not read by `bake::rough` yet (hard rock in steps); indicator plants over bodies (stage 10's third sign) are
  not done; Luna's row stopgaps (`weight_g`, Terra's densities in her `pressure`); no textures for ilmenite and
  merrillite; prospecting across a face's seam; samples into the copper age's recipes; better heads with longer
  `prospect`; a dowsing kit is a separate call; smoothing the heat field to soften mantle boundaries; pressure
  changed by digging, Luna's own g, kimberlite pipes, magma and any use of diamond. [0063, 0138, 0158, 0166]
- Coal seams lose some blocks where valleys cut them (seams are 190 m long at a fixed layer). [0063]
- Cost rows (`docs/perf-debt.md`): the halo's chunk generation (+81 us a chunk near a body, ADR 0096 allows it),
  `Body::rock` 18 ns a mantle cell against 2 ns, the flow noise per arc column; cheaper: the column's boundary radii
  once per tile. [0095, 0158, 0160]

## No longer holds
- 0063's `made_hash` 8ecb5dadddbbf9b5 as a lock: replaced by the table above.
- 0063's "Luna has no minerals until her own budget" and "the ledger counts the planet only": replaced by 0166.
- 0063's "caves by rock" worms as the only limestone and tube generators: see `caves.md` (0159, 0160, 0161).
- 0063's one exact 192 m crust floor for rock: replaced by 0158 on mantle worlds.
- 0158's "f283474fb9f15012" as the generation-1 hash: replaced by 37b2d1b1006b66f7 once the cave kinds came (0159).
