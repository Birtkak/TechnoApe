# The bake, the climate and the landforms

What holds today for the planet bakes (Terra's, Luna's, the Mars-like planet's), the air grid and climate, biomes, rivers
and lakes, landforms, the polar caps, ground detail, and the bake's progress on the title screen. Numbers in brackets
are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- **Biomes from a real climate model, not noise** [0042]: the air grid first, as the design says; one model for the bake
  and the later live weather, not a separate simple climate. Not chosen: a latitude-and-noise climate, Minecraft-style
  noise biomes.
- **Biome list** [0042]: land: ice, tundra, cold forest, temperate forest, grassland, dry scrub, desert, savanna, rain
  forest, wetland, alpine, beach. Sea: shelf, deep, warm reef. Each had to be shown on a look sheet before it was
  accepted. Per biome: its ground blocks and still plants made as plain blocks (trees, shrubs, cacti) [0042, 0043].
- **Looks accepted 2026-11-11** [0043]: "Accept both", then four changes: warmer plain sand; fewer dirt and gravel patches
  on dry scrub; fewer, less boxy trees; less neon meadow grass.
- **Stay close to realism for the seasons** [0060]: the seasons reach the whole air (upper layers, wind bands), not only
  the surface.
- **Rain raises live rivers** [0059, 2026-11-12]: rain is a flow on top of a dry-weather base.
- **Rivers and lakes first, tides later** [0059, 2026-11-12].
- **Realistic-ish, exaggerated for small planets** [0105, 2026-11-13]: landforms follow real-world logic (how and where
  they form); relief and contrast are exaggerated so they read on a 4 km radius world.
- **Taller mountains, clouds back up** [0105]: "the highest mountain peak [should] be able to reach the clouds". The
  developer chose taller mountains over the clouds-at-100 m of ADR 0098.
- **Polar ice caps** [0107]: the bug "How come there are no polar ice caps?" (2026-11-14). The rule and numbers are
  Claude's and open to taste.
- **Luna's calls, 2026-11-12** [0070]: ice under the dust in polar shadow; Luna from orbit accepted as it is; a brighter
  Luna in Terra's night sky (`MOON_NIGHT_GAIN`, the developer's to tune).
- **The Mars-like planet** [0167]: "rocky, cold, thin air, a dead core and so no field, old craters and dry river beds,
  giant dead volcanoes with long lava tubes". Its name, Mars, is the developer's call (2026-11-19).
- **Not chosen** [0042]: seasons and axial tilt in the first air grid (later added by ADR 0055 and carried into the
  bake by 0060).

Open taste calls (the numbers are defaults, not accepted looks):
- The tilt (25 degrees), how strong the seasons are (the live air gives about +-15 K at the spawn; `SEASON_K` +-7 K in
  F3's fallback; `season_shift` and `season_wind` set the winds'), each biome's look after the seasonal shift [0060].
- How a river, a lake and the rapids look; whether real waterfalls (a drop of a block or more) should exist. The smooth
  generation has none: steep reaches are rapids [0059].
- How rough the ground is: the 0.1 base and the 0.8 grassland factor are the knobs if plains read too flat [0058].
- Peak height (`peak_extra_m` 230, highest land 350 m on the 4 km planet), the landforms the survey proposed (snow line
  by temperature with a rock band, valleys and canyons incised in ranges, sea cliffs and fjords, mesas and volcano
  cones, a deeper ocean floor, island arcs broken up) are taste or scope calls, none built [0105].
- The cap rule's `polar_k` = 20 K (goes to 0 when the air model gets cold poles) [0107].
- Desert is the largest biome (22 to 42 % of the land), wetland the rarest (about 1 %); both are thresholds if the
  developer wants otherwise [0042].

## Versions on disk: the map, the bake, the save

The map is `bake::Map`, kept in `saves/bakes/` under a file named for its seed, planet size and version. A save names its
map by hash. A map is kept in the file of its own version, in the bytes of its own layout, so its hash holds [0056].

| Map version | What it is | ADR |
|---|---|---|
| 1 | the land (stages 1 to 4) and the biomes: per 8 m cell the height over the sea and the biome, plus spawn and hash | 0043 |
| 2 | adds `rivers: Vec<Reach>` (cell, the cell it drains into, area), downstream first, land cells only | 0056 |
| 3 | adds `bake::Climate` per air column (128 m): the ground and water share, the year's three means, the grid's state at the year's end; `Map::temp` | 0060 |
| 4 | adds the lakes (cell, level) and each river cell's flow | 0059 |
| 5 | adds the `rock` field (rock provinces) | 0063 (rock-minerals-and-ore.md) |
| 6 | the peaks stretched (`land::stretch`), the air's own lapse in the cells' temperature | 0105 |
| 7 | same layout and ground as 6, only the biomes differ: the polar caps. `bake::VERSION` is 7 | 0107 |
| 9 | the arcs' volcanoes raised before the climate, kept after the map's bytes; `bake::VERSION` is 9 (8: real gravity's year, the layout of 7) | draft 2026-11-23 |

- Luna's maps count from 101 (now `luna::VERSION` 102: the `rock` field is empty on Luna; a 101 file is baked again)
  [0070]. A planet's crust's maps count from 201 (`mars::VERSION` 201, Luna's layout: the map, then the tubes) [0167].
  The file name, the in-memory cache and the hash all key on the version, so the bodies' maps never meet [0070].
- A new game takes only the current version (7 for Terra); `bake::get(seed, n)` takes the map from memory, else from
  `saves/bakes/seed-<s>-planet-<n>-v<VERSION>.map`, else bakes and keeps it [0043, 0056].
- A save names its map by hash. `for_hash` reads an older map, or bakes it back from a fresh bake when its file is gone:
  `Map::bake_v6` (the rules before the caps, `LifeParams::capless`), `Map::bake_v5` (no stretch, the old lapse, same
  under 150 m), from which layouts 4 to 1 are made as before; Luna's 102 is baked back by its hash. Luna's 101 is not
  rebuilt (no save names it) [0059, 0060, 0070, 0105, 0107]. `every_version_a_save_names_is_baked_back_without_its_file`
  holds this (it caught `from_bytes` refusing a 6, and was red on 4 first).
- Old saves on a v1 or v2 map need their bake file while it is in `saves/bakes/`, which a bake never deletes: the bake's
  climate changed (0060), so a fresh bake no longer gives their hash. Without it the game stops, as for any map that
  comes out different [0060]. Likewise a save whose rebaked map comes out different stops the game [0043].
- Maps of versions 3, 2 and 1 still read, with dry beds and no lakes [0059]; a v2 map is read as it was and gives back
  its own bytes, so its hash holds [0060]. Bytes from a server are checked before the lines are built: a river cell
  drains into a neighbouring cell (across a seam too), its area is a positive number; a climate whose state does not fit
  its grid, or holds NaN or impossible amounts, is refused (a cell over 2^28 units of water is refused); the range of
  `m` is checked first [0056, 0060].
- **Save versions the bake owns** (the save format itself is multiplayer-saves-and-data.md): a save of version 6 names
  its map's hash; versions 4 and 5 load on the noise land [0043]. Version 8 keeps the air grid's state (4 to 7 load as
  before, their air starts from the map's); 9 is 8 plus `reaches`; 11 carries `luna` (a 10 or 9 keeps Luna on her hills);
  12 appends `rain`. An 11 or older loads with none of the rain state [0059, 0060, 0070].
- **Generation 4** (`world::GENERATION` 3 to 4; `known_generation` takes 4 and refuses 5): every body carries its
  world's generation (`Body::gen`); from 4, `Body::rock_at` answers on Luna, before 4 it answers `Rock::NONE` there.
  Terra is the same at 3 and 4 [0165] (also rock-minerals-and-ore.md, which holds the generation number).
- **Old worlds keep the noise land.** `World::new` is the noise land (tests, saves from before the bake); new games use
  `World::baked` and `Server::baked` [0043]. A world saved on a v2 map before ADR 0058 moved under its edits (the hash is
  unchanged so it is not detected; accepted, v2 was a day old) [0058].

### Hill country, and Mars's shields as domes: world generation 15 [0207, 2026-11-25]
The developer: "make planets more irregular and hilly all over the board so our huge volcano and mountains don't
look so much out of place". Measured first: the bigger the planet, the flatter its land. The ranges and the peaks'
stretch (ADR 0105) are in metres, the continents grow with the planet, and the heights are scaled by the one
highest peak: on the 8 km planet 99 % of the land lay under 47 m beside peaks of 350 m (52 m on the 4 km planet),
and nothing under 45 m was stretched. From map version 12, world generation 15 (`world::HILL_COUNTRY`; the build
handed over on the morning of 2026-11-25 made worlds of generation 14, which keep their map of version 11, the
coarse year on the land before the hills, `Map::bake_v11`):
- **The shields rise as high as a range** (`shield_uplift` 1.0, was 0.35), so the rivers cut uplands inside the
  continents instead of plains.
- **Hill country** (`hills` 2.0, `hill_m` 700): uplift in ridges 700 m apart, in metres on every planet, stronger
  in some regions than others. It goes in before the erosion, so every hill drains.
- **The stretch starts at 10 m and eases in over 90** (was from 45 over 20): the same order of heights, the same
  top; the foothills rise into the peaks instead of standing as a wall on a plain.
- **The tree line is at 150 m** (`alpine_m`, was 58): at 58 m a sixth of the land was bare alpine ground; at 150
  it is 3 % (1 % before), and the forests keep their shares within a tenth.
What it gives, seed 1337: on the 8 km planet half the land is over 25 m (was 12), a tenth over 97 m (was 31), one
cell in a hundred over 171 m (was 47); on the 4 km planet 25, 109 and 207 m (were 14, 33 and 52). The relief
inside 130 m at the middle cell goes from 12 m to 33 m on the 8 km planet. The land is colder for it: tundra 2.4 %
of the planet from 0.3, ice 8.0 from 6.3, wetland 0.4 from 1.6; the spawn is elsewhere. Small planets were rugged
already and change little (the 1536 one: a tenth of the land over 108 m, was 104).
**Mars's shields are domes** (the developer, the same day: "mars it's mountains look not right too"). His four
shields were round cones, smooth, 27 degrees steep, each ending in a circle, laid over one another like discs.
From his map of version 202 (the same generation 15) a shield is a dome: flat on top, steepest two thirds out,
level where it meets the plain and lying on the ground as it is there; a third wider (the giant a quarter) for
the same height; its foot runs out in lobes of a few hundred metres and tongues of 90 m; flows 2.5 m high run
down the flank. Summits, calderas, the giant's scarp and the tubes are where they were. A world before
generation 15 keeps the cones (`mars::CONES`, version 201, `bodies::Planet::cones`, the existing pinned hashes).
Seen on the bake's height picture (`out/mars/shields-cmp.png`), not in the game: that side of Mars was in its
night for a stand shot.
Older maps are baked as before (`land::Params::before_hills`, pinned to the bit). With it, a fix: a game
joining a server from the title checked the fetched maps under today's file names, so a world of an older
generation was refused as "could not be kept" (`Bake::file_at`, by the welcome's generation). Looked at in shots from the
ground and from 350 m up at two places (`out/hills/`), not yet in play.
Alternatives not taken: noise laid on the eroded land (rivers would run uphill and every dip would be a pond); a
lower `mountain_m` (the peaks are the developer's call of 2026-11-11). Open: the strength is taste (the four
numbers above are the knobs); the steepest tenth of the land is now steeper than 136 % between 8 m cells on the
8 km planet (was 42 %), which is climbing, not walking; the 4 km planet has 113 lakes where it had 30 (their cells 39,543 against
6,721, about 2.5 % of the land): hill country holds water in more hollows; a walker following the longest river of the 1536 test planet is stopped a
third of the way down, where the bed is two blocks wide and a tree stands at its edge (that test runs on the
land before the hills); Mars's highlands are as flat as they were (35 m, craters only); Luna and the icy moon
are as they were.

### The bake's year on an air grid twice as coarse [0204, 2026-11-25]
A new world's minutes were its climate's year of weather: real gravity's year (ADR 0181) is 334 days on the 4 km
planet and 472 on the 8 km one where the hand-set year was 54, and the air model stepped every one of them on the
live grid's own columns, 147 s of the 4 km planet's 187 and 531 s on the 8 km one (play build, 2026-11-25). From
map version 11 (world generation 14) the bake runs its year on a grid twice as coarse (256 m cells; 8 a face edge at
least, so a planet of 1024 columns or fewer keeps the live grid as before, and the 1536 one runs 12 cells on 8, in
the live grid's steps) in steps twice as long, the same share of a cell a step: an eighth of the work. The year's means go from that grid to the cells and to the live grid's columns by the same
smoothing as before. The live grid's state at world time 0 is each of its columns laid from its coarse column (the
air by its share of the area; the ground's heat as it is; soil, snow and dirt come down by its share of the land;
the sea takes what the whole units' cut leaves, so the water's count is the year's), then two days at its own size into the
year's turn, so its winds and its own columns' land and water are its own. The 4 km planet bakes in 64 s (climate
24). Worlds of generations 12 and 13 keep their map of version 10, baked as before (`Map::bake_v10`).
What it changes, one seed baked both ways on the 4 km planet: the land, the lakes and the river network are the
same; the sea-level temperature differs 0.84 C on average (3.8 at most), the planet 0.7 C colder; rain 0.30 mm a
day on average of a mean of 0.65, 6 % drier; the biome on 80.7 % of the land's cells is the same, on 19.3 % another
(borders moved); the spawn 296 m away. The cold, dry lean is not explained.
The developer's call (2026-11-25, "Yes, coarser climate"), asked after a like-for-like speed-up of the step found
nothing (the carry pass with each edge worked out once: 8 % faster on one thread, slower on the pool).
Alternatives not taken: the year sampled every few days with the seasons run faster (soil and snow would need
their rates scaled); the air model on the GPU. Left: the erosion (27 s) and the water table (8 s) are now as much
as the climate.

### Volcanoes and vents, fed by the planet's heat [0188, 2026-11-23]
The developer: "it's time to add volcanoes and volcanic vents ... The heat from the planet itself will sustain".
Terra's arcs get volcanoes with a chamber, a conduit and a crater lake of lava, and vents where the heat reaches
the ground; all of it follows from one temperature field around the chamber, not from rules per feature
(`docs/design/volcanoes.md`). Replaces `worldgen.md`'s "lava only as still pools deep under volcanic provinces".
**Eruptions too** (the developer, the same day): a live chamber fills from below, and past the plug's strength
its surplus pours from the crater as counted lava that freezes to basalt. Still standing: no quakes.

### Volcanoes on Terra's arcs: cone, conduit, chamber, a lake of generated lava [0192, 2026-11-23]
- **Context:** ADR 0188 asks for volcanoes fed by the planet's heat (`docs/design/volcanoes.md` steps 1 and 2).
- **Decision:** map version 9 (`bake::VERSION`) places the cones after the erosion and before the climate
  (`bake/volcano.rs`): on the volcanic arc, the crater and upper cone on land (the lower flank may stand in the sea
  and ends at the shore), no lake or river under the foot, clear of the face seams, eight foot radii apart, picked
  by a hash of the seed alone (nothing by the spawn). Defaults, taste calls open: six on the 4 km planet (by area
  elsewhere, one at least), every third dead; foot 4.5 % of the radius (191 m on the 4 km planet, 48 m at least),
  the cone 0.65 of it plus 10 m high on ground levelled to the summit's foot, a concave flank, a crater a fifth of
  the foot wide (three map cells at least) and 0.3 of the height deep (18 m at least); the chamber 0.7 foot radii
  under the foot, 0.18 of it across; the conduit 1.5 % (2 m at least). The map keeps each (cell, foot, rim, crater,
  depth, chamber's height and radius, conduit, alive) after its bytes, as Luna's tubes are. Generation 10
  (`world::VOLCANOES`) plays on it; older generations on version 8 (`Map::bake_v8`, the same bytes as before).
  In the world (`world/volcano.rs`): `Body::heat` is the far heat plus `(T_chamber - T_far) * R / r` from a live
  chamber and from its conduit (the larger), `T_chamber` the province's `chamber` row in `data/rocks.toml` (1150 C,
  refused unless over lava's freeze point in `data/fluids.toml`); a dead volcano adds nothing. A live one's chamber
  and conduit are caves (a ball, and a ball a layer up to the lake), the foot's other caves left out, no plant in the
  crater; where the heat there is over lava's freeze point they hold generated lava (`Ground::generated_other`):
  a closed hollow full and at the lake's head under pressure, the open one up to the lake, which stands a third of
  the crater's depth over its floor and 2 m under the lowest rim, a whole layer. So it all stands at one head and
  lies still untouched; touched it is stored as counted units by the ordinary path, `counted_of` per kind net of
  what generation gave. The client draws a live lake's tiles as it draws stored fluid. Feet in lava burn by the
  metal sea's rule.
- **Not chosen:** cones added in the world's heights only (the air, the biomes and the snow would not see them); the
  server storing the lakes at load (every client and save would carry them; the tests that compare a client's
  copy with the server's would see it).

### Eruptions: a chamber fills, bursts its plug and pours counted lava [0196, 2026-11-24]
- **Context:** ADR 0188 ("eruptions too"), `docs/design/volcanoes.md` "Eruptions", the world queue's step 5; lava is
  the counted fluid of 0186 and 0193 and reacts by 0194.
- **Decision:** each live crater's chamber (`World::chamber`, `World::erupt`, once a second with the heat step, by
  the world's clock) holds one number, the units of melt over its rest, and whether its conduit is open. Melt
  arrives at the arc's `eruption.supply` (0.003333 chamber volumes a game year; `shortcut:` for the mantle's supply
  rule); the overpressure is `bulk` (1000 MPa: a melt with gas bubbles and its walls) times the fill over the
  chamber's volume; past `plug` (10 MPa, rock's tensile strength) the conduit opens and the fill pours as stored
  lava at `effuse` (1 m3/s) until less than a unit is left, then it closes. So an eruption is plug / bulk = 1 % of
  the chamber (64 m3 for the small planet's, seeds 3 and the tests', about 1,650 m3 on the 4 km planet; real
  chambers give some 0.1 %, exaggerated as the landforms are) every plug / bulk / supply = 3 game years (162 days,
  81 hours of play at real time). A chamber not yet stepped starts a hash of the seed and its cell of the way full,
  so a planet's volcanoes do not erupt together. The lava is poured on the outer lip of the rim's lowest column
  (`Crater::lip`; `shortcut:` where a risen lake would spill, the lake staying generation's formula), at its row's
  pour heat, and then runs, lights, reacts and freezes to basalt by the rules already there. Its units are booked
  in `Liquids::erupted`, so lava's `counted_of + froze + vapour` is `erupted` plus what buckets brought. It is
  poured whether a player is near or not (the liquid step runs on every stored column of a loaded body; held while
  the home is not loaded). Warnings: a chat line to everyone when a chamber passes nine tenths of its plug, when it
  erupts and when it is quiet again; no lake rise, fumarole, sound, ash or glow event yet. `/erupt` fills the
  nearest live chamber to its plug, so it erupts by the rule the next second; the server's report lists each
  live chamber's fill, what its plug holds, its pressure and the years to its next eruption. The save keeps
  `(chambers, erupted)` as a fourth tail after the heats' fluids, written only when there is any: a world with no
  live volcano writes the bytes it did; a save without it loads with every chamber where the seed starts it.
  Generation 10 worlds change by it (they erupt now); older ones have no crater and step as before.
- **Limits:** any client can run `/erupt`, as `/fluid` and `/timewarp`; while a conduit is open it does nothing and
  says so. A pour refused while the lip's gap is full leaves the chamber filling, and the backlog pours later (nothing
  lost). Frozen lava takes the first free cell at or above its floor up to its gap's ceiling, so a coarse step
  (`/timewarp`) sets every basalt block. A flow's momentum is not saved (links restart at rest on load), so a
  reloaded server's flow may differ by a unit from one that kept running; lava at rest above its freeze point is
  not given pour heat again on load. A recording begun from a save logs the save's tails (heats, booked, kinds,
  chambers) beside it, and `Server::hash` takes the chambers only when there is one. Skipped warning signs: the
  lake's rise, fumaroles, quakes, sound, ash and glow. The frequency and the volume are defaults awaiting the
  developer.
- **Not chosen:** an eruption on a timer or a random draw (the design: from the supply, ended by the chamber
  emptying); raising the crater's lake to spill (1 % of a chamber raises it centimetres: no flow down the cone, and
  the lake is a formula); holding an eruption until a player is near (more state for the same lava).
- **Open:** the frequency and the volume are defaults, not the developer's; a lake that follows the chamber's
  pressure; an `Event` with sound, ash and glow; explosive eruptions after gases.

### Locked hashes and byte-identity

- Terra's map bytes at seed 5, n 256 (v6 and v5 of it): `16ee32bb94308b73`, `065e789cbd17a892`, `5bfb7a403005f7d7`;
  Luna's map (seed 7, 416) `cb20ef2abc8ed3a7`; the stage lock `the_stages_bake_terra_and_luna_to_the_bit_as_before`
  holds them [0165, 0167].
- Luna's generated blocks, small Luna (`luna_blocks_hash`): `a22a376de6be21ba` at generation 0 and 3,
  `ca3af00891b21215` at generation 4 [0165].
- `made_hash` (Terra's caves and ores, seed 1 planet 6144): `bfb878ce14d32b75` (generation 4; mantle=0 `0926f028ddcd85a7`,
  gen=1 `37b2d1b1006b66f7`, gen=2 `a2ef250b3ed4203a`, gen=3 `bfb878ce14d32b75`); Luna's, generation 4:
  `cdd810732c670a46` [0165, 0167]. Before the stretch it was `8ecb5dadddbbf9b5`, after `3b563610bd1d358f` for version 6
  maps [0105].
- Seed 7, 4 km planet map hash `6b835875f1871d10` and seed 3 / 256 `1bf96be37cccebe6` were unchanged by the progress
  reporting [0057].
- Cached `seed-1-planet-6144-v7.map` and `seed-1-planet-1664-v102.map` are byte-identical to a fresh bake under the
  stage pipeline, so the caches stay valid [0165].
- Every map cache writes through `<file>.<pid>.tmp`, so two processes baking one map never rename each other's
  half-written file [0167].
- Windows and Linux must bake the same bytes: maps are never sent, each side bakes its own and `state.rs` compares
  hashes. Platform `sin`/`cos`/`acos`/`powf` and float thresholds could differ in the last bit (the Mars bake adds
  some: the canyon's width, the valley depth law `powf(0.3)`, the bulge's azimuth, the frost line `|z| > 0.87`, lake
  water `>= 2 m`, `SHORE` -12 m, crater age against the ground's age, the cover test of seen craters). A Linux bake of seeds 1 and 2 must be compared with Windows' hash first [0167, 0060].

## The bake in the engine

- **`engine::bake`** (`crates/engine/src/bake/`): grid, land (stages 1 to 4), life (5 to 8), air. A `Map` keeps per 8 m
  cell the height over the sea (a lake cell at its level), the biome, the spawn and its hash; mips of the heights
  (2 x 2 means) are built on load [0043].
- **Bake time** (4 km planet, 8 of 12 threads, quiet PC): about 36 to 40 s, once per seed and planet; the map is about
  19.2 MB (peak RAM 349 MB in the prototype) [0043, 0060, 0105].
- **Heights:** a Catmull-Rom spline through the 4 x 4 cells around a column, folded over seams, plus the detail below
  (ADR 0058 replaced the uniform 1 m). Chunks of level L read mip L - 3, so a mip cell is about as wide as their
  columns. The sea floor is clamped to 56 m (CRUST stays 64). `AMP` is 400 m (was 140, from 80 on the noise land) for
  the stretched peaks [0043, 0105]. The noise land keeps its 80 m bound and the clamp of 140 stays on maps of version
  5 and older and Luna's, so old worlds keep their ground to the bit; the clamp of baked heights on version 6 and 7 is
  378 m [0043, 0105].
- **Ground per column** from `data/biomes.toml` (schema and checks in `crates/engine/src/biomes.rs`): top, band and rock
  blocks; patches (a share of the surface, clumps of about 3 columns); snow above a height. The biome lookup and patches
  are moved by noise of about 6 columns so borders interfinger. The height cache tiles hold each column's ground
  [0043].
- **Biome ground, plants** (the table of looks) [0043, with the later changes]:

  | biome | ground (top / band / rock; patches) | plants |
  |---|---|---|
  | ice | snow / ice / stone, 20 % patches of bare ice (0107) | none |
  | tundra | grey-green grass, gravel patches / dirt | rare shrubs |
  | cold forest | blue-green grass, snow patches / dirt | spruce 8-12 m, shrubs |
  | temperate forest | grass / dirt | broadleaf 6-9 m, shrubs |
  | grassland | yellow-green grass / dirt | lone broadleaf, shrubs |
  | dry scrub | olive grass, dirt 0.1 and gravel 0.04 patches / dirt | shrubs 1-2 blocks |
  | desert | sand / sand / stone | rare cacti 2-4 blocks |
  | savanna | straw grass / dirt | flat-topped acacias far apart, shrubs |
  | rain forest | deep green grass / dirt | dense jungle trees 12-16 m, undergrowth |
  | wetland | mud, dark marsh grass patches / clay | reeds, shrubs |
  | alpine | gravel, stone patches / stone; snow over 100 m | none |
  | beach, shelf | sand / sand | none |
  | deep sea | mud / clay | none |
  | warm reef | sand, coral patches / sand | none |

  Sand is a warmer yellow, plain with soft grain. Broadleaf crowns have radius 2 and every round crown loses about a third
  of its outer shell to a hash; the share of spots that grow trees is broadleaf 0.25, jungle 0.4, spruce 0.38 [0043].
  (The look of trees was later changed by ADR 0092, plants-and-creatures.md.)
- **New blocks of the bake** (ids 8 to 29): sand, snow, ice, gravel, clay, mud, log, four leaves (spruce, broadleaf,
  jungle, acacia), cactus, shrub, reed, coral, seven tinted grasses. The tints are separate blocks so far chunks show each
  biome's colour with no renderer change [0043].
- **Still plants** as plain blocks through one function, `Body::plants_with`, the only source: the mesher reads it through
  the tile, physics through `col_plants`, the water through `Solid::gaps`. One candidate spot every 4 x 4 columns,
  hashed from the seed; the biome's rows give the share that grows each plant; shapes are data: cone, round, flat,
  column, bush. A plant stands at least 3 columns inside its face. Plants stand only where the ground leaves air; an
  edit wins over them; wood wins over leaves. Level 0 only: far chunks show the biome's grass colour. `PLANT_TOP` is 42 m.
  The spot cache per body keeps the server's tick low [0043]. No plant grows in a river or a lake [0059].
- **The spawn by score:** grassland or temperate forest, slope under 0.08, within 160 m of water, temperature nearest
  14 C, then moved to a column with no plant around it. The search keeps within 26 degrees of longitude of the +y
  meridian because the sky's clock puts 9 am at world time 0 there (`sky.rs`) [0043]. (Spawn rules: nothing near
  spawn but copper, see the world's memory notes; Luna's landing point is the map's `spawn`: gentle ground nearest +y.)
- **The download:** `Welcome` names the map's hash. A client with the map neither in memory nor on disk sends
  `ToServer::Map` and gets `ToClient::Map` pieces of 1 MiB (18 MB for the 4 km planet); the server makes the pieces once.
  `net::fetch_map` keeps whatever else arrives meanwhile and hands it on in order. `ToServer::Map(hash)` asks for either
  map (Terra's or Luna's); `Welcome` carries `luna` [0043, 0070]. (also multiplayer-saves-and-data.md)
- **The soak** runs on a baked planet and its bots share the server's map; its Luna grows from 32 to 128 columns [0043].
- **Where it was left:** `meshbench` (`crates/client/src/sys.rs`) stays on the noise land [0043].

## The air grid and the climate

- **The air grid** (`prototypes/air`, `air.md`): columns of 128 m, four layers, water and dirt in integer units (an exact
  ledger), heat and wind f32 in a fixed order. The bake's step is 10 s and matches the live step; 30 s does not. Every
  threshold is in `LifeParams`, every air constant in `air::Params` [0042].
- **Stages 5 to 8** (`life.rs`): climate (the air model settles on air columns built from the bake; its means come to the
  8 m cells smoothly, with the lapse by each cell's height); water on land (rain less evaporation routed down receivers as
  discharge; wetlands where ground is flat, fed and humid); soil (sand, loam, clay, gravel, peat, rock, from slope over 24 m,
  coast, wetness, floodplains, aridity); biome (a Whittaker table by temperature band and rain, after ice, alpine, tundra,
  beach and wetland; the sea's three by depth and warmth). Inputs are jittered by 40 m noise so neighbours interfinger
  [0042].
- **The bake runs a real year with the sky's sun at the same step as the live grid** (`bake::air`): the sun is
  `Sky::sun_dir` with the sky's tilt of 25 degrees (ADR 0055); `Air::year` runs the last 18 days of the year before, then
  one year of 10 s steps from world time 0, returns the year's means (potential temperature, the day's swing, rain), and
  ends at the year's turn so its last state is the weather at world time 0. The warm-up must not hold the sun at the
  equinox (the next year rained up to 36 % more in dry biomes). The bake, not a tilt-free day or four held days, is
  chosen: the bake is 0.1 % off the live year at the same step; a 5 s live step rained 10 to 20 % less [0060].
- **The seasons reach the whole air** [0060]: the upper layers cool toward the day's mean sunlight at their latitude and
  the sun's declination (`balance`); the wind bands move toward the sun by 0.4 of its declination (none at the poles) and
  blow `1 - sin(lat) sin(decl)` as hard; `season_lag` can delay both and is 0. Spawn noon in Terra's seed 1337: spring
  9.4 C, summer 27.8, autumn 16.7, winter -2.2 (+-15 K).
- **`Map::temp`** is a cell's baked temperature: the bake's own smoothing (`land::upsample_at`, which `land::upsample`
  calls), less the lapse by the cell's height; it equals `life::Life::temp` at every non-lake cell. The cells'
  temperature takes the air model's own lapse (`air::Params::cool`: 0.1 K/m to 150 m, 0.015 above) in `life` and
  `Map::temp` alike [0060, 0105]. F3 shows the live air where the server sends it, else the map's mean with ADR 0055's
  season and hour terms, else the biome's band on an older map [0060].
- **The live grid** (`engine::weather::Weather`, on `Server::weather` when the map has a climate) [0060]:
  - One step is `DT` = 10 s of world time (a step every 200 ticks). When the grid is a step behind the clock, a worker
    thread takes it for up to 8 steps; the server takes it back `LAG` = 20 ticks later and waits if it is not done.
  - A job's work is fixed when handed over and what the game reads changes only on the tick it comes back, so thread
    timing never shows. Under a time warp it keeps up to 80x, then jumps. A clock set back moves the grid's clock with it.
  - Its view per column: surface air, rain, cloud cover, wind, runoff over the job. The view's hash is in
    `Server::hash`, so the soak and the replay check the weather.
  - Each player gets `ToClient::Air(Here)` every 40 ticks (about 24 bytes); bots report it too.
  - Cost: a live step on Terra 4.43 ms on one thread, 1.27 ms on 12; the 8 km planet 17.5 and 4.4 ms; on the server's
    own thread under 10 µs. About 0.022 ms a tick on one thread (target 0.5) [0042, 0060].
- **The rain check** (`weather::tests`): the bake's mean rain per biome against the next year of live weather, each
  smoothed to the map's cells the same way; `RAIN_TOLERANCE` is 5 %. Terra (seed 1337,
  `cargo test --release -p engine -- --ignored terra_rain`) is within 0.3 % in every biome [0060].
- **Tests run the engine at opt-level 2** (`[profile.test.package.engine]`): a small planet's bake runs a year; the bake
  tests take 31 s instead of 2.5 minutes [0060].
- **Views:** `season-spring`, `-summer`, `-autumn`, `-winter` look south from the spawn's forest at noon on the four
  seasons' days (not accepted: no baselines at the time) [0060].
- **Survival's cold, the snow line by season, crops and solar power** can read `Weather::here` [0060].
- **Not built** [0042, 0043, 0060]: seasons in the first air grid; deltas of the grid to clients; rooms, fires, chimneys;
  rain on awake chunks; air.md's yearly swing, rain per season and snow days kept by the bake; the globe map's W2b was
  separate. For live weather, the sea breeze is weak at 128 m and first-order carry smears a plume over about 3 columns a
  minute (knobs for when weather is played) [0042].
- **Planet size:** at 12 km the bake's climate needs a coarser grid, a longer settle or more time (+66 s, 2.7 GB, and the
  air does not settle within its 45-day cap); the game stays at `planet=6144` unless the developer decides [0042].
  (The 8 km planet, `planet=12288`, is a supported size by later ADRs; this is the 12 km case.)

## Rivers and lakes

- **Lines and beds** (`bake/rivers.rs`, map v2) [0056]:
  - `land::bake` finds every cell draining at least `river_area_m2` (1.5e5 m^2) and its receiver; the map keeps these as
    `rivers`; `Lines` is built on load (0.05 s). One height function carries the bed: `World::height_at` cuts it, so the
    server, height cache and far chunks all get it.
  - Banks: the lowest map height at or upstream of each river cell, one pass upstream first, so a bank never rises
    downstream.
  - Half width `(area / 1.5e5)^0.4` clamped to 1-6 m (a 2 m stream to a 12 m river); depth `0.6 + 0.4 * half width`
    (1-3 m) under the bank; both interpolated along a line.
  - Profile: a flat bottom out to 0.8 of the half width, a rounded side up to the bank at the half width, then a bank that
    cuts at slope 1 for 6 m more (`BANK`).
  - In `World::height_at`, after the detail noise: `h = min(h, cut).max(floor)`; `cut` is the lowest profile of the
    lines near the column, `floor` the nearest line's bed while inside that channel. On a line the ground is the bed
    exactly.
  - Lines run through column centres: each node is moved half a column to the centre of the column just past it
    (a cell's centre is a corner of the columns under it, 8 columns a cell). Without this the walk test found 6 one-block
    steps up in 17.6 km.
  - Far chunks: a line is cut only for columns no wider than the river. Level 0 and 1 chunks show every river, level 2
    rivers from 4 m wide, level 3 from 8 m. Mips are unchanged.
  - The index: a bit per map cell says whether a cut reaches it; behind it a sorted list of (cell, segment) pairs from the
    cells around three points of each segment, padded for the narrowest cell (0.7 of the nominal edge); a test checks it
    against every segment.
  - Walk test: a river followed from spring to sea never climbs (0 climbs, 0 one-block steps up on the line; every river
    ends under the sea level). The test steps on the game's own height function and column tops.
  - Known shortcut: a bank's cut ends 6 m past the half width; where ground stands more than 6 m over the bank it shows a
    step down to the cut. Two diagonal reaches that cross give two beds where they meet; steepest descent cannot make
    one, measured 0, and the lines test asserts none.
- **Hold water** (map v4, `Lines` per segment: level at each end, fill, flow, reach) [0059]:
  - The map keeps the lakes (cell, level) and each river cell's flow; the ground under a lake is its bed. The level never
    falls under the lake or sea it runs into and never rises downstream.
  - Fill: the bed's depth x min(0.7 (runoff ratio)^0.3, 0.85), dry under 0.15 m. Flow: Manning's v = fill^(2/3) sqrt(slope)
    / 0.035, clamped to 0.3-2 m/s. Reaches are cut at springs, confluences and lake outlets, plus one reach per lake. A
    reach holds what it takes in for its length over its speed; a lake holds it 600 s. `Map::water` gives a point's
    river or lake water.
  - Generation (`Body::water_at`, the liquid's `Ground`): each column gets its water top, its account (a reach, or `SEA`),
    and for a river its flow per column of width, its fall and its lateral runoff. The sea wins wherever it stands higher.
  - The model is the sea's (ADR 0020): generated still water costs nothing, only disturbed water is simulated. A river
    column's friction (`drag_for`) is chosen so steady live flow equals generation; a woken column starts at the steady
    speed; open ends are booked to the reach's account (`reach_extra`): upstream the river pours in its flow, downstream
    it holds the generated level, a lake holds its level. Every 15 steps a whole body within U/64 of generation is
    forgotten, and so is any body that fell asleep since the last pass (`slept`).
  - `run_reaches` once a second: each account passes on extra/hold, at least a unit a second, to the reach below and
    finally to the sea. `Liquids::rain(reach, units)` is the entry for live runoff, booked as `rained`; the water's count
    is `counted() - rained`. `reach_flow` is a wheel's read: one lookup. Luna has no reaches (`Ground::down` and `base`
    read Terra's reaches only).
  - Drawing: `world::river_quads`, `vs_sea`, `fs_water`. A chunk holding a wet column's level gets a quad of kind 5 at
    that level, a plane through the column's middle with its fall along i and j (18 + 7 + 7 bits in q.y); a lake's
    columns merge as the sea's. The fragment shader runs ripples downstream at 0.5-3 m/s by steepness and whitens rapids.
    None past chunk level 6. `Lines::wet` is a mask of cells within a wet channel's half width (sampled every quarter
    cell) and lake cells with their ring; `Map::water` returns early outside it; `Map::wet_near` checks a chunk's
    footprint.
  - Physics: in water, jump held with a one-block ledge ahead climbs out.
  - The soak: bots dam rivers (a creative plug of stone across the channel, dug out 2-6 s later) on planet 1536, the
    smallest with wet rivers; the gate's soak stays on planet 256 (no rivers).
  - Known shortcuts: a sloped river quad steeper than 0.25 layer a column is drawn in steps; lake outlets can stay live;
    a lake crossed by a river is a flat channel; rapids, no waterfalls; the dam chain accounting (rain poured into a live
    patch returns as returned water to the reaches below).
- **Live rain** (`weather::Weather::feeding`, `Weather::tick`) [0059]:
  - A river cell's own catchment (its drained area less what flows into it) is shared out of its air column's land (the
    column's area less its water), capped at all of the land, kept as 1/65536 parts per reach, built once at server start.
    What no river drains stays in the sea term.
  - When an air job lands (at a fixed tick), each column's runoff over the job goes to its reaches by those shares, in
    whole units rounded down; the sum comes out of the air's `sea` term. The server rains it into the reaches
    (`Liquids::rain`). The air's ledger plus what it handed is constant (`Weather::ledger`), and the water's
    (`counted() - rained`); the soak checks both every tick (`Server::air_ledger`).
  - A rain account per reach, `Liquids::rain_held`, kept apart from `reach_extra` (which carries the generated flow's
    circulation); only `rain` writes it, and once a second it runs down the reaches' linear stores to the sea.
  - `BASE` = 0.5 of a river's flow at rest is the dry-weather base (`weather.rs`); a column hands its reaches 1 - `BASE`
    of its land's runoff (0.5 is a typical baseflow index). Live water upstream pours in its share of the base to
    `reach_extra`, and on top the rain's flow (`reach_flow` = held over the reach's hold) at the same share
    (`Ground::base`: the reach's outflow at its last segment; 0 for a lake); that part is taken out of `rain_held`, capped
    at what is held and rounded up.
  - View: `View::runoff` counts water units per air column over the last job; `Weather::column(dir)` finds a cell's
    column.
  - Cost on the 4 km planet: the split 11.5 ms once at server start (298 reaches over 13,824 air columns); a landing's
    hand-over 36 µs once per air step (200 ticks); `run_reaches` under 50 µs a second.
  - Save version 12 appends `rain`: each reach's rain, the air's job back but not shown yet, and the water's steps.
    A save mid-rain and loaded carries on to the same water and air as a server that kept running. Not the same yet: up to
    20 ticks of the shown weather after such a load; which ticks step the water while the water budget is overloaded.
    The wire is unchanged.

## Ground detail by slope

[0058] (`bake::rough`, map v2 and later; v1 maps take the old uniform 1 m detail to the bit at every level.)
- `bake::rough(slope, biome, h)` is the one function for how rough a cell's ground is (metres of detail). Slope is m/m
  from the four neighbouring cells (across seams), `h` metres over the sea.
  - By slope: `0.1 + 0.4 s(g / 0.2) + 2.5 s((g - 0.25) / 0.6)` (`s` smoothstep): about 0.15 m on a plain, 0.5 m at the
    4 km planet's median land slope (0.21), 1.3 m at the 90th percentile (0.47), 2.8 m at the 99th (0.75).
  - Height stands in for hard rock: x1 up to 50 m over the sea, rising to x1.5 at 100 m. The sea floor is calm by the
    coast: x0.1 from the sea level to 4 m down, back to x1 at 16 m down.
  - Biome: beach 0.3, wetland 0.4, desert 0.6, grassland and savanna 0.8, ice and tundra 1.1, alpine 1.3, the rest 1.
    Capped at 3 m; a cell with no slope at all (a lake's flat) gets 0, so lake beds stay calm. The sea floor under sea ice
    takes the ice biome's 1.1 (left so, as `rough` runs on old maps too) [0107].
  - Where rock plugs in: `rough` takes the cell's rock as one more argument and its hardness replaces the height factor
    (`Map::roughs` passes it); nothing else changes.
- `Map::rough` (serde-skipped) is built in `Map::finish` with the mips and river lines, v2 and later: 3.5 M cells and 14
  MB on the 4 km planet (about 56 MB on the 8 km one). `Map::height_rough` is `height` at mip 0 with the roughness on the
  same Catmull-Rom weights (one generic `spline`).
- Character in `world::detail(a, n)`: below 1 m of roughness the noise scaled (rounded swells); from 1 to 2.5 m it blends
  into the noise folded at 0 (`0.25 - |n|`, scaled 1.6), sharp crests: crags on steep ground. No second noise.
- Far columns of a v2-or-later map get no detail where columns are wider than a map cell (`k = level - log2(n / m) > 0`,
  mip alone). On the 4 km and 8 km planets (8 columns a cell) levels 0 to 3 read mip 0 and get the detail.
- The river cut is applied after the detail, so a bed's line is the bed to the bit. Known: up to about 2 m of mismatch
  where near and far chunks meet on crags (level 3/4); on a steep bank a crag dip can lie lower than the bed's floor.

## Taller mountains and the landform principle

[0105] (also rendering-and-lighting.md, sky-weather-and-water.md for the cloud base)
- `land::stretch`: under `STRETCH_FROM` (45 m) a height is unchanged; above, the height over 45 m is multiplied by a
  factor that eases from 1 to its full value over 20 m (smoothstep), the full factor such that the highest land
  (120 m, `mountain_m`) stands `peak_extra_m` higher. Order of heights is kept, so every downhill, river and lake holds.
  Lakes are found and puddles filled before the stretch (the lake cells are the unstretched bake's); then the ground, the
  filled surface and lake levels are stretched; climate, soil and biomes are made from the stretched heights.
- `peak_extra_m` = 230 (in `land::Params`): the highest land is 350 m over the sea on the 4 km planet; the extra scales
  with the planet `n / 6144` at most 1 (768's highest land 149 m, 1536's 178 m, the 8 km planet 350 m). Luna's bake keeps
  its own relief. All land over 150 m is alpine; trees stop at the alpine line (58 m).
- Cloud base back at 250 m (`CL_BASE`, the lightning's height follows); the slab is 64 m thick, top at 314 m.
- `world::AMP` 400 (was 140); the clamp of baked heights is 378 m on maps of this version only.
- Bake version 6. New worlds get it; old worlds keep their terrain (see Versions on disk).
- Consequences that hold: mountain slopes up to about 4x steeper over the old 65 m line (more bare rock and gravel by the
  slope rules); AMP 400 adds about 260 layers of air to every body (small bodies, radius under 400 m, never take
  `generate`'s quick "solid" exit); the crack counter's "surely solid" sphere keeps the old 141 m; the LOD walk passes
  over a node's children when all are empty leaves (cost row in `docs/perf-debt.md`).
- Seed 1337 4 km, highest column: cell (face 2, i 673, j 660), 120.0 to 350.0 m; all 3,506,215 cells under 45 m are
  identical to the bit; 2.0 % of the lowland's land cells changed biome (rain shadows). `bake::tests::the_peaks_rise_and_the_lowland_stays_to_the_bit`
  holds it (768 and 1536 planets).
- **The landform principle** [0105, the developer 2026-11-13]: realistic-ish, exaggerated. Landforms follow real-world
  logic; relief and contrast are exaggerated for small planets.
- **The landform survey** (state of the 4 km bake, none implemented, each a taste or scope call): ranges are good; snow
  begins at 100 m everywhere (just over the tree line, no rock band, no change by latitude); no gorges in ranges; almost
  all coasts are forest down to a thin beach (19 % of seed 1337's land is under 5 m), no cliffs; half the land is under 14
  m (plains with low hills read well); no plateaus, mesas or volcano cones; the sea floor is 30-40 m almost everywhere
  (shelf and abyss barely differ); seeds 7 and 641653600 have long, thin, almost straight island chains.

## Polar ice caps

[0107] (`bake/life.rs` stage 8, `world.rs` `Body::ice_sheet`, `client/src/globe.rs`; also sky-weather-and-water.md)
- **Bake rule:** `cold = tt - polar_k * max(0, (lat - polar_from) / (90 - polar_from))`, with `polar_k` = 20 K, nothing
  equatorward of `polar_from` 55 degrees, linear between. Land is `ice` where `cold < t_ice` (-13 C, as before); the sea is
  `ice` (the same biome row reused) where `cold < t_sea_ice` (-8 C). The air model keeps its poles a few degrees under 0 and
  has no ice albedo, so the count stands for the long polar night and the ice's own albedo. `shares` keeps sea ice out
  of the land's shares. Seed 933105400 planet 6144: ice on 25 % of the land and 12 % of the sea at 60-70 degrees, 98-100 %
  poleward of 70, about 6.5 % of the planet. A small planet (768) has no cold poles.
- **Ground:** the ice biome's top is snow, its band ice, with 20 % patches of bare ice (`data/biomes.toml`).
- **Sea and lake ice** (`world.rs`, `Body::ice_sheet`): where a level-0 column's biome (jittered as the ground's) is ice
  and water that is not a river stands over its ground, one block fills the layer the water's surface lies in (the sea's
  surface is 7/8 into its layer, so the sheet stands an eighth of a block proud, open water under it). The block is the ice
  biome's top: snow, with patches of bare ice. Laid through `plants_with`, so the mesher, physics and water read it. A
  cheap test first (no ice cell within the jitter's reach) keeps it off other columns. Far tiles (levels 1+): a coarse
  column over frozen water stands at the sheet's top, in ice, reaching at least the middle of the coarse cell. The sheet is
  only laid on maps of version 7 (`ice_sheet`'s gate).
- **Globe map** (`client/src/globe.rs`): ice cells are drawn in the ice colour over land, lake and sea alike.
- **Version:** `bake::VERSION` 6 to 7; `Map::bake_v6` bakes a 6 again; `for_hash` tries it; `find` and `from_bytes` read 6.
  `bake_v5` and older layouts derive from the capless rules too.
- **An existing save keeps its map** (a save names its map by hash): `world-1.sav` (seed 933105400) carries on on its v6
  map with no caps. Moving it to v7 would need a one-off rewrite of its map hash; not done, the developer's call.
- Sea ice is solid: a boat or swimmer stops at its edge; a player walks on it and digs through to water. Digging the
  sheet gives snow.
- Known and open: the tide under a sheet is not verified (ADR 0067); the far sheet stands up to half a coarse cell over
  the sea (a step at the cap's edge from far away); anything keyed on the biome (herds, F3 line, rain per biome) sees ice
  over the sea there; `plant_top_with` does not see the sheet. Revisit: when the air model has cold poles, `polar_k` goes
  to 0.
- Tests: `both_poles_carry_an_ice_cap_on_land_and_sea` (standard planet of two seeds, ice on 4 to 14 % of the planet,
  over 90 % of it poleward of 55 degrees, over 80 % of each pole's last 10 degrees) and
  `the_polar_sea_carries_a_sheet_of_ice_over_open_water`.

## Bake progress on the title screen

[0057] (also multiplayer-saves-and-data.md, the join path)
- `engine::bake::Progress` is a `Mutex<(&'static str, f32)>` whose fraction never falls. `bake::reporting(p, || ...)` is a
  thread-local sink set around the work, written by the existing `lap` closures, the erosions' and air model's step loops,
  and `fetch_map`'s piece loop: no signature changes, no cost when nobody listens. `STAGES` names the bake's laps in order
  with each one's seconds in a 4 km bake (a lap not in the table is passed over, so the bar stays monotonic, only less
  even). `reporting` ends at 1. `net::fetch_map` reports `DOWNLOAD` with pieces received over total.
- Client: `menu::Loading` (a worker thread below normal priority, its `Progress`, the arguments to start again with). The
  new world page's Create bakes there when `bake::file(seed, planet)` is missing (and, per ADR 0070, when either Terra's
  or Luna's map is missing), else starts at once. The long work is done in the title process that already has a window.
  A `connect=` play run whose map is not kept starts the title screen with `join=host:port` (and every other argument),
  which fetches the map under the bar and then starts the game with `connect=` and `joined=1` (which never starts a title
  again; the join goes through the title from any play run, not only on screen, and the title's own world then is the 768
  planet with no save). `menu::server_addr` adds `:7777` once, for both uses. `menu::join_args` / `joined_args` pass every argument
  through. The title checks `bake::file` after the download and shows an error note if the file is missing. The bar reads
  "Joining the server: connecting", then `downloading the map`; the frame polls `is_finished` and draws the stage and a bar
  at the foot of the screen; while it runs the menu answers only Quit.
- Tests: `bake::tests::a_bake_reports_progress_and_makes_the_same_map`, `net::tests::a_map_download_reports_its_pieces`.
- Known: a saved world whose map is missing (deleted, or a new map version) still bakes in `main()` with no bar; so does
  the title's own background world on a first start. A `connect=` join that needs the map connects three times (probe,
  title download, game). Stage weights are one machine's 4 km bake; other sizes move a little unevenly. Luna's bake has no
  stage of its own on the bar (2 s).

## One bake of named stages

[0165] (the table of stages; Luna's rock part also rock-minerals-and-ore.md)
- A row of `data/bodies.toml` lists `stages`, in the order they run (it replaced `bake = "terra" | "luna"`, ADR 0163).
  `bake::Stage`, `bake::check`, `bake::run`; `bodies::Bake` is the list and its body's sun at its highest
  (`luna::sun_high` of the row's size and orbit) which a crust's `ice` reads.
- Terra: `["plates", "erosion", "climate", "ice", "life"]`. Luna: `["crust", "impacts", "ice", "volcanism"]`.
  - `plates` (`land::crust`: grids, plates, boundaries) and `crust` (`luna::crust`: the old crust without plates) are the
    two crusts; one must be first.
  - On plates: `erosion` (`land::erosion`: coarse and fine erosion, provinces as the grid is refined, the sea, rivers and
    lakes), `climate` (`life::climate`: the air model's year on the cells), `life` (`life::life_of`: water on land, soil,
    biomes), `ice` (the polar caps, ADR 0107, listed between `climate` and `life`; without it the bake is the capless one:
    map versions 5 and 6 are Terra's list without `ice`, `Stage::TERRA_ICELESS`). A bake on plates ends with `life`.
  - On a crust: `impacts` (`luna::impacts`: basins and craters oldest first; the maria flood among them at `T_MARE` when
    `volcanism` is listed), `ice` (`luna::ice`: the polar shadow), `volcanism` (`luna::volcanism`: the thorium province and
    the lava tubes). The `ice` before `volcanism` order commutes with Luna's old order (checked by the lock).
- `check` refuses with a message: an unknown name, a list not starting with a crust, a crust past the first place, a stage
  twice, a stage without what it needs before it. (The crossings "not built yet" were lifted for the planet's crust by
  0167; see below.)
- Terra's and Luna's lists keep their maps' files, versions and old layouts (`bake::get`, `for_hash`, `luna::get`);
  `luna::bake` is the home's moon's (`bodies::moon`). `Map::bake`, `bake_with`, `bake_v5`, `bake_v6` go through `run_with`
  (plates), `luna::bake` and `bake_stats` through `luna::run` (a crust), and `run` picks by the first stage. A reordered
  crust list (`volcanism` before `ice`, or no `volcanism`) bakes: it is another body. Shortcut: a list of neither kind is
  baked on every call and kept under a name nobody writes; the Mars-like planet's step gave such a body its cache name
  (0167).
- **Luna's rock field** (generation 4): `Map::rock` on a Luna map is `luna::province(kind)` of the cell's kind (maria and
  fresh mare: `mare`; the thorium kind: `kreep terrane`; every other kind: `highland crust`). No new bytes: her map file
  and hash stay. Rows in `data/rocks.toml` after Terra's (ids 6, 7, 8; `bake::rock::NAMES`): `highland crust` (anorthosite
  to the bottom), `mare` (`mare basalt` 60 m from the datum, about 40 m under a mare's ground, over anorthosite), `kreep
  terrane` (KREEP 45 m from the datum, about 20 m under the dust, over anorthosite); fold 2 m over 400 m, heat 0.02 C a
  metre, caves `none`. The biomes' dust (4 m on highlands, top metre on maria) lies on top. No new blocks: anorthosite
  (200, rubble 182), mare basalt (202, rubble 183), KREEP (205, rubble 185) came with ADR 0070, each mined with a pick,
  stacking to 16.
- **The tables' body key:** a province row may say `body = "<row name>"` (`rock::Province::body`, a row index; the home's
  when not given); a name that is no row is refused, and so is a key that disagrees with the province's id (ids to the
  ocean floor are the home's, those after it the home's moon's). Terra's six provinces say none, Luna's three
  `body = "Luna"`. A Terra map whose cells name another body's province is refused on read. A world without a mantle
  (generation 0) never gets Luna's rock field.
- Uses named, not built (the developer's rule: every rock is something to find, mine and craft with): anorthosite for
  aluminium, ceramics, glass, refractory bricks; mare basalt for ilmenite (titanium, iron, oxygen), basalt fibre and cast
  basalt; KREEP for thorium and rare earths.
- `place=`, `fxedit=` and `structure=` in a stand shot now use the stand's body (they built on Terra whatever `body=` said).
- Open: what ore and caves on Luna need (the second half) was listed as a work list for ADR 0166 (`ore::place` and
  `caves::make` return nothing when `map.rock` is empty; the ledger counts `bodies[0]` only; Luna's row stopgaps
  `weight_g`, Terra's densities).

## Luna's bake

[0070] (`crates/engine/src/bake/luna.rs`; size and orbit are ADR 0041 and bodies-and-orbits.md)
- Luna is 3.7 times smaller than Terra (1664 columns under the standard 6144, 1150 m radius) and 60 Terra radii out
  (ADR 0041, `data/bodies.toml`). The same `bake::Map` under its own version numbers (101 then 102); her kinds of ground
  are rows of `data/biomes.toml` after Terra's 15 so `world.rs` draws them with no new code. Her relief is her own and
  unaffected by the stretch (ADR 0105).
- Cells of 2 m (832 cells per face edge; the map is 20.8 MB). Crust: noise of +-8 m around 40 m over the world's datum.
- Basins: four 0.5 to 0.9 Luna radii wide within about 45 degrees of body -x (the face toward Terra, `sky.rs moon`), and two
  anywhere; shallow for their width (2.5 % of the diameter, worn as the oldest).
- Craters: N(>D) = K A / D^2 with K = 0.2, from 6 m to 0.13 radii (150 m), each with an age `t` (0 oldest, 1 now), stamped
  oldest first: parabolic bowl, depth 0.15 D capped at 3 (D/20)^0.45 m; 30 to 80 m a flat floor; from 120 m terraces; from
  100 m a central peak; debris falling as x^-3 to 3 radii; older ones shallower and lower-rimmed (sharpness 0.35 + 0.65 t).
- Maria flood at t = 0.35: each basin facing Terra fills with basalt to half its rim's height over its floor, with +-0.4 m
  of wrinkles; older craters are drowned or left as ghost rims. If no crater came after their time (a Luna under about 64
  columns), the maria flood after the craters.
- Dust is in the kind of ground: highland 4 m of regolith on anorthosite; mare 1 m of dark regolith on mare basalt; craters
  younger than t = 0.96 have bright fresh dust to 1.3 radii, 1 m on the rock under; craters over 30 m and younger than
  t = 0.995 throw 12 rays up to 8 radii.
- Ores are what each rock is (counted ore bodies are W4's, now ADR 0166): mare basalt ilmenite; anorthosite aluminium; 1 %
  of craters over 15 m are iron impactors (a 3 m lens of iron-nickel under their floor); a KREEP (thorium) province, a
  sector of highland round the largest mare; helium-3 in the old highland dust.
- Lava tubes: in each mare one or two wandering chains of balls, radius 3.5 to 5.5 m, roofs 5 to 9 m thick, two pits each
  where the roof fell in; kept off face seams; stored in the map as (cell, depth, radius) and made `Body::caves` through
  `set_caves` (bucketed per tile like Terra's).
- Polar shadow: computed on 8 m cells within 20 degrees of either pole; the sun's highest is the orbit's 2 degree tilt plus
  the disc's 0.25 degree; a cell is shadowed when 32 rays toward that sun, marched 700 m over the ground, are all blocked;
  shadowed cells become the kind `polar shadow`: 1 m of regolith on ice, the ice to 4 m down, then anorthosite.
- Blocks 200 to 205 (anorthosite, mare regolith, mare basalt, fresh regolith, iron-nickel, kreep); W4's rocks take the ids
  30 to 56 and no name is shared (W4's `basalt` is Terra's, Luna's is `mare basalt`).
- Plumbing: `World::with_maps` and `World::baked` stand Luna on her map with no sea; `World::with_map` keeps the hills.
  The new-world page bakes both maps. `run=stand` takes `body=1` and `look=terra|luna`. A 10 or 9 save must leave no bytes
  over, so a pre-merge branch build's 10 (with `luna` after it) is refused.
- Bake time 2.0 to 2.5 s for the standard Luna; the map adds 57 MB of RAM; meshing her ground costs about 10 % more per
  job than her hills.
- **A brighter Luna in Terra's night sky:** `shader.wgsl` `moon_gain` multiplies the sunlight on Luna (an eclipse's copper
  with it, so an eclipse keeps its ratio to a full moon) by **`MOON_NIGHT_GAIN` = 2.6**, the developer's to tune, only when
  the camera is in another body's air and that body's night (faded in by the sun's height from 0.05 to -0.1). By day, from
  space, in orbit round Luna and standing on it the gain is 1. Luna's size, phases, the CPU's `moon_col` moonlight on
  Terra's ground and Terra's own light are untouched. (also rendering-and-lighting.md)
- Open: `Ev::Start` should name Luna's map when recordings next change (a recording from a seed replays on this build's Luna;
  a `shortcut:`); a ragged shoreline of the maria would come from more crust noise at the basins' rims; the ambient term
  and the metal glint are not raised with the moon gain.

## The Mars-like planet's baked map

[0167] (planet-model step 5a; not a body in any world yet: no row in `data/bodies.toml`, no change to saves, the wire,
the sky, the renderer or world generation)
- **The row** was `data/drafts/bodies-mars.toml` (since Mars joined the world it is the last row of `data/bodies.toml`); then not loaded
  by the game: `name = "Mars"`, `parent = "sun"`, `stages = ["crust", "impacts", "volcanism", "erosion", "ice"]`,
  `size = 1.85` (3328 columns, 104 chunks = 13 x 8; 6144 / 1.9 would be 101 chunks, prime; 2300 m radius), `sun = 65212.5`
  (1.5 AU in its own radii), `age = 1.0`, `generator = "hills"`, `ground = ["regolith", "regolith", "stone"]`, `air = false`
  (stand-ins read by no stage), `kind = "rocky"`, `iron = 0.5`, `heat = 0.15`, `density = [2900.0, 3500.0, 6700.0]`. Thin air
  (about 0.01 atm), no liquid water and the sky colour are not keys yet. `age` is the one new key: the share of the impacts'
  power law its surface has collected; `parse` refuses it on any row that does not bake a planet's crust and outside
  0 < age <= 1.
- **By the row, not the list** (real-world reason: Luna's maria flood the basins of the side facing her planet, a planet's
  own volcanism is a hot spot under one plate; a list-based meaning would give `["crust", "impacts", "volcanism"]`
  maria on a planet and shields on nothing): a moon's crust (a body whose parent is not the sun) is Luna's code; a planet's crust (a body
  that circles the sun) is this bake. `bodies::Bake` gains `Option<Planet>` (`name`, `age`); `get`, `for_hash`, `file`,
  `version` go to `bake::mars` for it. `bake::check(stages, moon)`: on a planet's crust `erosion` needs `impacts`; on a
  moon's crust `erosion` is refused ("not built yet on a crust without plates of a moon"); `climate` and `life` on any
  crust, and `impacts`/`volcanism` on plates, stay refused.
- **The stages** (`crates/engine/src/bake/mars.rs`; 4 m cells, m = 832, 4,153,344 cells):
  - `crust`: the dichotomy. The plains are a cap of 38 % of the surface round a centre within about 25 degrees of the north
    pole (the seed tilts it), lobed by noise (+-0.12 rad), smooth at -25 m; the highlands about +35 m, rough, rising 24 m
    away from the plains; a scarp of about 65 m over about 160 m between. The plains are "made" at `T_PLAINS` = 0.8.
  - `impacts`: two giant basins on the highlands, far from each other (1100-1650 m wide, 0.045 d deep but at most
    `BASIN_DEP` = 66 m so a larger planet's basins stay in the world's columns, flat floored, the planet's lowest ground, their own profile); then Luna's craters: her
    power law `K` times `age`, D from 12 m (the dust filled smaller ones) to 0.1 r, her crater shapes and her `Land::stamp` (without her kinds). A crater
    is kept only where the ground it fell on is older than it; with `volcanism` listed those younger than `T_VOLC` = 0.9
    wait for it.
  - `volcanism` (one plate over a hot spot, dead now): a bulge (0.6 rad, +70 m) on the dichotomy's edge, highland side;
    lava floods its low ground (dome + 44 m, to -20 m at its lobed edge, so it spills onto the plains); three shields in a
    row (NE-SW, 0.13-0.16 rad, 150-190 m over their foot) and the giant past its NW edge (0.3 rad, summit 348-360 m, a 35 m
    basal cliff, two nested calderas), each a near-linear flank `(1-x)^1.15` with a flat caldera (7 % of its height). Lava
    tubes down the flanks (2 a shield, 5 on the giant; balls of r 4-6.5 m every 1.2 r under a 5-9 m roof, two pits each),
    in Luna's tuple and `Map::tubes`. The canyon along the dichotomy's edge (0.24 rad inside it, away from the giant,
    0.9-1.05 rad long, floor falling from +45 to -8 m eastward, half width 35-100 m, a second trough beside its middle).
    Then the young craters.
  - `erosion` (long ago, dry): three chaos patches at the canyon's end (floor -10 m with knobs) and an outflow channel from
    each toward the plains (half width 20-30 m, floor -10 to -29 m) until 60 m out on them. Then Terra's drainage
    (`land::drain`: Priority-Flood and D8) with the old shore at -12 m as its sea, on the regional ground 8 cells wide
    (`land::upsample` of the cell means, not the cratered ground: networks died in the first crater when tried there):
    valleys where 6e4 m^2 drain, not on lava or shields; beds carved (1.2 (A/A0)^0.3 m, to 7 m, banks widened past 4 A0) and
    made to fall from the springs down (a rim in the way is cut through); lake beds (water 2 m or more over the regional
    ground) where valleys end, flattened 3 m under the spill; the basins' floors under the shore are old sediment; deltas
    (fans of 25-90 m) where valleys over 4 A0 met standing water. The beds are the map's `rivers` (flow 0: the water is
    gone), so `Map::river` will cut them; the caps' valleys are dropped (buried).
  - `ice`: layered caps, north 0.2 rad (45 m thick), south 0.15 rad (32 m), lobed, terraced in 3 m steps, three spiral
    troughs; frost on the floors of craters of 30 m and more poleward of 60 degrees.
  - Nothing reads where a player lands: the map's `spawn` (Luna's rule, gentle ground nearest +y) is picked after the last
    stage.
- **The map's kinds** (biome ids after Luna's; the `data/biomes.toml` rows come with the run that puts the body in the
  world): 22 cratered highland, 23 dust plain, 24 lava plain, 25 basalt shield, 26 canyon floor, 27 chaos, 28 outflow
  channel, 29 sediment basin, 30 delta, 31 polar ice, 32 frost. `Map::rock` answers None on it (no provinces yet);
  `Map::from_bytes` refuses a version 201 map with a kind outside 22..=32.
- **The file:** version 201, Luna's layout. Cache name
  `saves/bakes/seed-<seed>-planet-<n>-<row name, lower case, other characters '-'>-<low 32 bits of FNV of "<stages:?> <age:?>">-v201.map`,
  e.g. `seed-1-planet-3328-mars-89b8b05e-v201.map`: another list, age or bake version is another file, a renamed body bakes
  once more. Baked once, kept in memory by path (`mars::get`, `keep`), read back from the file; a file that is no map of that
  version, seed and n is baked again.
- **Seeing it:** `technoape-server bakemap=data/bodies.toml body=Mars seed=1 [n=] [out=prefix]` bakes it, keeps
  it as the game would find it, writes `<prefix>-height.png`, `-kinds.png`, `-features.png` (equirectangular 2400 x 1200
  centred on the canyon, `bake/pictures.rs`) and prints one JSON line of the numbers. `bakemap=` refuses an n that is not
  whole chunks (n=0 exits 2).
- **Numbers** (full size n = 3328, seed 1): bake 2.7-4.3 s; map 21.0 MB; lowland share 0.372; highland / plains mean height
  35.2 / -24.6 m; height range -41.1 .. 352.0 m (the world's columns: -56 .. 378); 64,653 craters made, 46,538 seen; 2
  basins; 3 shields plus the giant (690 m radius, 29 m foot, 352 m summit, caldera 23.6 m); canyon 2234 m; 3 outflow
  channels; 18,199 valley cells; 136 deltas; 11 tubes, 533 balls, 21 pits. What the row keeps across seeds: plains' share
  and heights, one bulge on the edge with three shields and the giant past its NW side, the canyon along the edge ending
  in chaos and three channels, two basins, caps at both poles, the crater densities. What the seed moves: the plains'
  centre and lobes, the basins, every crater, the valley networks and lakes, the shields' heights within their ranges, the
  tubes' paths.
- **Tests that hold it:** `bodies::the_mars_draft_row_parses_and_its_list_is_accepted`, `bodies::bad_rows_are_refused`,
  `bake::mars::a_planets_bake_is_the_same_twice_and_round_trips_through_its_file`, `bake::mars::the_map_has_what_the_row_calls_for`
  (seeds 1 and 2 at n = 1664), `bake::mars::heights_stay_in_the_worlds_columns_at_every_size` (n = 832 and 6144).
- **Open (the body in the world and what it needs):** the row moves into `data/bodies.toml` (a third row; `MAX` bodies is
  4); **generation 5** adds the third body (a world made before keeps two); the sky's pose; the wire sends its map (version
  201 accepted by `net`, `bake::find` by the planet's file name); `World::with_grid` reads its map by `Bake::get`;
  `world::AMP` holds its 352 m summit. Places that would meet a Mars map wrongly once it is a body:
  `crates/client/src/globe.rs` (`version > 100` takes it for Luna's), `crates/engine/src/biomes.rs` (22 rows registered,
  kinds 22..=32 read past the table), `crates/engine/src/world/body.rs` (tests `== luna::VERSION`), `Bake::plates()`.
  Rock provinces and ground (`Map::rock` from the kinds as Luna's: cratered highland basaltic breccia, shields and lava
  plains basalt, sediment in lake beds, basins and deltas clays and sulphates, canyon floors layered deposits, caps water
  and dry ice; `rocks.toml` rows with `body`, `biomes.toml` rows 22-32, red dust over everything), the air and sky colour keys,
  ore and tubes by `caves.toml`/`minerals.toml` are all later steps. Looks to judge on the pictures: most valleys end in
  highland lake beds (237 of 282 mouths), the caps are thin bands in the projection, smooth shield cones, no fretted
  terrain along the scarp.

## No longer holds

- 0042's air grid with no year and no tilt in the bake; its "20 day repeat, no drift": replaced by 0060's year with the sky's sun
  (25 degree tilt, seasons through the whole air).
- 0042's "Phase 3 waits for the developer's look sheet": done by 0043.
- 0043's uniform 1 m detail noise on heights and its `DETAIL_M`: replaced by 0058's roughness; `AMP` 140: raised to 400 by 0105.
- 0043's "existing saves keep the noise land" for saves from before the bake still holds; its "first start waits about 25 s
  with no progress shown": replaced by 0057's bar.
- 0043's bake times (25.0 s; v1 layout and 17.7 MB): now 36 to 40 s and about 19.2 MB.
- 0056's "beds stay dry" and lakes flat at their level: replaced by 0059 (water in beds and lakes).
- 0056's "VERSION 2" and 0059's, 0060's, 0105's, 0107's intermediate versions: stand as rows of the table above; the
  current is 7.
- 0057's new-process hand-over (`sys::wait_shown`, `shown=`): gone (0073).
- 0057's "a lap not in the table is passed over" stays; its seed 7 and 3 hashes stand for the progress test only.
- 0059's first live pour design where `reach_extra` carried rain (a dam's wave never settled): replaced by `rain_held`.
- 0059's air-grid-less runoff ("the bake's mean rain over the catchment") until W3: replaced by the live rain section.
- 0060's note that the map's version stays 3: version 3 stands as in the table.
- 0070's "Luna's maps 101": now 102; her rock "left to W4": done by 0165 (generation 4); `bake = "luna"` row key: replaced by `stages`
  (0165).
- 0105's cloud base at 100 m (ADR 0098's): the cloud base is 250 m.
- 0165's "not built yet on plates: impacts, volcanism" and "not built yet on a crust without plates: erosion, climate, life":
  `impacts`, `volcanism` and `erosion` on a planet's crust lifted by 0167; a moon's crust still refuses `erosion`, and `climate` and
  `life` stay refused on any crust.
- 0167's "no row in `data/bodies.toml`" holds until its next step.
