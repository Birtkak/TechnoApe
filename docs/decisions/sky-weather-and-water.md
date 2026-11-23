# Sky, weather and water

How water is simulated, drawn and bedded; the seasons' looks; the weather the player sees and hears (clouds, rain,
snow, wind, storms); the light snapped to texels; the globe map. What holds today. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- Water has momentum and waves; a generated sea now, basin lakes with the biomes; splashes as client particles now,
  12.5 cm cells later; a bucket and simple swimming; creative pours freely but counted; no endless sources [0020].
- Grass and leaves stay one block each and their look follows the season; nothing stored, nothing meshed again per
  season [0061].
- Water "still looks ugly": the look is traced and clear, still on the pixel grid [0071].
- The water's clock steps 4 times a second so ripples, foam and falls jump like pixel art instead of gliding [0071].
- Blocks set into water must not make water behave strangely [0071].
- River, sea and lake bottoms are clay, sand or stone, whatever is geologically right; dirt too, never grass [0086].
- Rain must not fall above the clouds ("its snowing while i am above the clouds") [0108].
- The weather's cloud shadow is soft and cloud-shaped, not the cells' hard squares [0076].
- Lightning is rarer than the first version: every 20 to 40 s under a storm [0076].
- Clouds feel bunched and too dense: scattered cumulus with blue between in fair weather (a look call left to
  Claude, [0108]).
- The light is snapped to the block texture's texels, so shadows read as pixel art, not grain [0108].
- Creative sees the whole globe map, survival only what it has seen; survival has waypoints on the globe too [0077].
- **Liquids and gases are one system, and nothing is faked** [0177, 2026-11-23]: an untouched fluid may follow a
  formula as long as it is the state the simulation settles to; every fluid answers to its body's gravity and to
  the tides of its moons and sun; the first liquids are water, lava, liquid metals, oil and toxic sludge, and they
  mix and react in the open world, not only in machines; gases follow the same logic, with atmospheric tides where
  they change play or the simulation. Tide sizes are derived from the bodies' masses and distances, with no fixed
  amplitude and no scale: the simulation is to be as close to real astrophysics as possible. The design and its steps: `docs/design/liquids.md` (a proposal; each step is
  spiked and measured before it is built). Replaces, when step 2 is built: 0067's tide as a level only (`retide`,
  the shader's offset on a sea meshed once). Open: equilibrium tides have no lag; the layer cap; gases near the ground.
- Open taste calls (colours and amounts are first guesses): every seasonal colour and how long autumn lasts [0061];
  the water's colours (`WATER_FOG`, the absorption) and how fast the underwater fog closes [0071]; foam as a thin
  broken line, not surf over the whole shelf [0071]; the Low preset does not set Water [0071]; cloud height, cell
  size, how much sky the cover fills, rain density and sound levels, snow white from 10 mm of water [0076]; the bed
  numbers (35 degrees, 1 %, 8 m, 1.5 m, 0.5 m), deep sea as mud over clay, alpine lakes as bare rock [0086].

## The water's simulation (`crates/engine/src/liquid.rs`)

The server simulates; clients show it [0020].

- **Model.** Shallow water on the columns' air runs ("gaps"): one node per gap holding water, a velocity per link
  (virtual pipes). Water is in integer units, 4096 to a block (`U`), so counts are exact [0020].
- Speed carries on from link to link; friction, a thin-sheet drag and a stop speed slow it. A fall into a gap
  passes at most Torricelli's flow. A full gap under a ceiling holds 1/16 block extra per block of head, so closed
  tunnels pass pressure on [0020].
- A step is 1/60 s; the server runs as many steps as the physics does [0020].
- **Closed gaps.** A link into or out of a closed gap passes at most what would level the two heads (half the
  difference, or the room plus the rest over K + 1 once the gap is full, K = 16), shared over the gap's links; the
  gap settles in a step, full, with the pressure share that matches the sea [0071].
- **Activity.** Only awake nodes are stepped; a node sleeps after 30 quiet steps. Every 15 steps a calm body is laid
  exactly flat (binary search for its level, every unit kept) and put to sleep [0020].
- **Budget.** At most 60 000 node-steps a tick (about 7 ms). Past that, water slows down instead of the server
  lagging. The count of skipped steps goes on the stats line [0020].
- **Blocks set into water.** Water a block displaced whose gap above stands over the column's generated open water
  joins that water's account (`book(reach)`), only the part under that level and not in a river, so no puddle stays
  on the block and the ledger stays exact. After a dig the column closes and is forgotten (generation's sea again).
  Test: `a_block_placed_in_the_sea_and_dug_leaves_the_sea_as_it_was` [0071].
- **Play.** B takes the bucket: left click scoops up to a block, right click pours it. Creative pours and scoops a
  full block every time, counted. Swimming is in `phys::step`: buoyancy holds a body 80 % under water, drag, jump
  held swims up at 2 m/s, walking is slower by how deep you are [0020]. (also player-and-feel: the sprint's `sub`
  share and the dive key are in 0118.)

### Weather-driven heat runs at the calendar's pace [0187, 2026-11-23]
The developer's call, on the water-phases spike's rate question (`prototypes/water-phases/README.md`, its option 3):
the game's year is 52 times shorter than a real one, so **heat exchanged with the air and the sky** (a pond under
winter air, an ice block in a warm biome, snow) runs faster by that same factor (the real year over the game's,
derived from the sky's rows, not a typed 52), and a winter grows a real winter's ice. **Heat a source drives**
(a fire, a furnace, lava, the rock's own heat) stays at real rates. Known cost, accepted: against the air a small
thing cools faster than a fire warms it, so a pot's loss to the air is taken at the real rate while a source
heats it.

### Fluids are rows; lava, with no mixing [0186, 2026-11-23]
`data/fluids.toml` (`crates/engine/src/fluids.rs`, built in): a row a fluid, its place its kind (row 0 water,
rows only added at the end): `name`, `density` (kg/m3), `viscosity` (a whole number), `colour`, `light` (as a
block's, 0..4; a fluid with one glows), and `freeze`, `boil` ({ at = C, into }) and `reacts` ([{ with, gives,
heat }]) parsed but unused until liquids.md steps 4 and 5. First rows water (viscosity 1) and lava (2600 kg/m3,
viscosity 64, glowing orange). Not reloaded live (a kind is a row's place; a reload must reach the clients).
- **A node holds one kind** (`Node::kind`, 0 when empty). Two kinds never share a gap: a link into a gap holding
  another fluid passes nothing (a plain wall), and when two kinds flow into one empty gap in a step it takes the
  first link's and the others stay where they were. A pour into another fluid is refused. Where an edit joins two
  gaps of different fluids, the denser keeps the gap and displaced water joins its account (as 0071). Only water
  trades with the open sea, rivers and rain, is settled to the sea's level or forgotten as generation's; another
  fluid is always stored. `counted()` stays water's; `counted_of(kind)` is each fluid's, exact.
- **Viscosity** multiplies a link's friction a step: `v -= (v * visc >> 9).max(1)`, and the thin-sheet drag by the
  same factor; water's 1 is today's to the bit. Steady flow on a slope runs at 1/visc of water's; a lava dam break
  is overdamped (friction-led), so its front's time for a distance grows with the viscosity, linearly in that
  limit. Head, gravity (`Ground::g`) and every other rule are water's.
- **Wire and save** carry the kind in the top byte of a gap's units (`liquid::pack`: units | kind << 56). Water is
  kind 0, so a water-only world sends and saves the very bytes it did, every older save and recording loads and
  replays unchanged, and no save version or generation moves.
- **Look**: a glowing fluid's quads in the water mesh carry its light (7 bits a channel in `WQuad.info` bits 10..)
  and are drawn as the metal sea is (`metal_sea`, opaque, its own light); no new effect.
- A thick fluid stays awake only while the speed a head difference gives a link in a step outlasts that step's
  friction (`thick_moves`, under its body's pull); water keeps `moves`. Without it lava held still by friction
  never slept. On a slope lava still creeps for minutes before it sleeps (water runs off in seconds).
- **Play**: `/fluid <name>` sets what creative's bucket pours and scoops; survival's holds water. A stand run's
  `pour=t:di:dj:dl:fluid` pours through its server (docs/playing.md).
- **Fixed on the way:** the water mesh packed absolute layers while the GPU counts from the floor of the worlds
  before the mantle (`frame::gpu_r0`, ADR 0152), so every stored column (water too) was drawn `mantle` layers up,
  out of sight, since the mantle. Its quads are now in the GPU's layers.
Replaces: 0020's "a gap holds one number of water" (a gap holds one number of one fluid).
Open (step 4): layers by density in a gap instead of the wall, reactions (lava and water to rock and steam), the
look per fluid, lava's light into the block light and its heat on a player (it neither holds up nor hurts yet), a
server ledger for creative's other fluids.

## The sea and the ledger

- **The sea** is generated from the seed: every column whose ground is under the sea level holds water up to it.
  The sea level is 4 m under the planet's radius [0020] (also grid-and-planets: snapped to 1/8 block under a layer
  top, ADR 0021). The moon has no sea [0020].
- **Tide** (also bodies-and-orbits, ADR 0067 is another topic's number): the level in a column is the mean sea plus
  the tide there; the shader and the physics (`World::water_surface`) ask the same function, so generation, swimming
  and the underwater test agree [0020, amended 0067].
- A column is stored only once water near it moves. The edge of the stored region is open sea: a stored column next
  to unstored sea, within 1.5 cm of the sea's level, is held at that level and the difference goes to the counted
  reservoir `sea_extra`. A calm body touching the open sea is laid at the sea's level; columns that are as
  generation gives them are forgotten again [0020].
- **Conservation.** `counted() = stored - generated + sea_extra` changes only when water is carried in or out. The
  server's ledger adds the buckets and what creative made or removed; it must be 0 and the soak checks it every
  tick. The save keeps stored columns, `sea_extra` and the ledger (save version 2; version 1 still loads) [0020].
- Open (shortcut): the sea's level does not rise with what it takes in; revisit when the water cycle moves sea
  water [0020].

## Water on the wire and in the client

- **Tiles.** Water goes out in tiles of 16 x 16 columns (`TILE`), each sent whole. A tile is sent when it changed by
  more than a dead band (1/64 block), or settled, since this client last had it. Tiles go to a client within 128 m,
  nearest first, about 16 KB a tick [0020].
- A tile goes out as last listed: each column's water as of its last listed change (`tile_sent`), not its current
  water; otherwise a neighbour's change within the dead band leaks out and a later undo is never heard [0020].
- Settled tiles reach a client exactly; the soak compares them with every bot [0020].
- Shortcut: far floods are not sent, so they are not seen from orbit [0020].
- **Meshing** (`crates/client/src/water.rs`). The generated sea is the terrain chunks' trailing quads at the exact
  sea level (terrain LOD and streaming for free). Tiles holding stored water are left out of those quads (down to
  level 4) and drawn by the water mesh: a top per column with corners shared with wet neighbours (never below its
  floor), and sides over lower water or floors (fronts, falls). Heights move from the last state to the new one over
  one tick on the GPU [0020].
- Droplets fly off sides that drop over half a block and splash where they land [0020].
- The sea's quads are meshed once for the highest tide (`sky::TIDE_HIGH`); the shader's `vs_sea` lays each vertex at
  the mean plus the tide, and the client re-meshes its wet tiles' unstored sea columns every 4 s of world time, one
  tile a frame. The tide never remeshes the sea [0020, amended 0067].

## The water's look (`fs_water`, `shader_water.wgsl`, post.wgsl)

Traced and clear, still on the pixel grid [0071].

- **Ripples.** Three octaves of value noise over the face's grid coordinates (columns, which are metres), passed
  from `vs_sea` and `vs_water` (`WOut.grid`, `face`), evaluated at the middle of each 1/16 m texel. They run on the
  sky's clock (`Globals.water.x`, world time mod 3600 s, so a `run=view` shot is the same every time); a river's run
  along its flow (`WOut.gflow`, columns a second, from the reach's fall). The clock steps 4 times a second
  (`WATER_STEPS`). Ripples calm where a pixel covers more than 4 cm of water and are gone at 40 cm: far water is
  flat, from orbit one blue with no seam between levels [0071].
- **Clear.** A ray down the view (up to 16 m; nothing met counts as its reach) gives the way through the water; it
  takes `exp(-k len)` of the light, k = (0.45, 0.12, 0.075) a metre (red goes first). Blended over the bed already
  drawn: alpha is what of the bed does not show; the colour is the water's own light (`WATER_FOG` lit by sun and
  ambient) plus the reflection [0071].
- **Reflection.** Fresnel with water's 2 % head-on. Within 250 m, and only where the Fresnel term is over 8 %, the
  terrain is traced up to 150 m and lit without shadow rays (`seen_lit`), fading into the sky over the last 50 m and
  between 8 and 16 %; past it the sky alone (`sky_colour`, with the sun's disc). Luna is added where the reflected
  ray meets its sphere, lit by its phase. The Reflections setting off leaves the sky only [0071].
- **Glint.** The sun's specular: exponent 900 on near ripples, 60 where they have calmed [0071].
- **Foam.** A broken line where the depth at the view ray's hit (way through times the cosine) matches a level that
  laps in and out between 0.05 and 0.35 m, near water only; on every front and fall (`side` quads) in streaks
  running down; on rapids by speed. (The first try, four short rays across the surface to the nearest bank, failed
  on stepped beaches.) [0071]
- **The sea behind the horizon** is not drawn when the camera is out of the water (water writes no depth). A fall seen
  from behind is drawn as its face; the discard comes before any ray [0071].
- **From under the surface:** the sky through a window overhead (Snell's), the deep around it, fogged [0071].
- **Under water** (post.wgsl composite): with the eye in water (`World::water_surface`) everything dims to
  (0.55, 0.8, 0.9) and fades into `WATER_FOG` times the day's light at 0.09 a metre (most of it by 12 m). The old
  flat blue veil (play.rs) is gone [0071, replacing 0020's veil].
- **Setting.** Graphics, Water: off (the old sheet, `water_old`), simple (ripples, Fresnel, the sky in it, a guessed
  3 m depth; no rays of its own), traced (the default); `g.opt.x` bits 4-5 [0071].
- Shortcuts: ripples and foam use per-face grid coordinates, so a cube face's edge has a pattern mismatch (calm there
  at any distance); the ripple clock is world time mod 3600 s, so the pattern jumps once every two game days; far
  water is not hazed into the air as far ground is [0071].
- Known limits of the mechanics: falls inside a column land at once; waves in deep water run at most at the speed of
  8 m deep water; a cube edge has a seam in the water mesh; one thread for the simulation; the dead band means a
  client's moving water can be up to 1/64 block off until it settles [0020].

## Beds under water (`Body::bed`, engine `world.rs`)

Chosen where the ground is chosen (`Body::ground_biome_at`, the one place the mesher, `generated_under`, physics and
the water read a column's blocks from); nothing is baked, no map version changed [0086].

A column is under water when the water's level (`water_at`) stands over its ground. The bed then depends on:
- **Slope** of the map's cell (`Map::slope`, four-neighbour rise over run) over `BED_STEEP` = 0.7 (about 35 degrees):
  bare rock, the province's (`ROCKS.at`), under any water; a lake in the alpine biome is rock too [0086].
- **Rivers**, by the reach's fall (m a metre) and flow: falling 1 % or more, gravel; under 0.2 % or 2 m deep or more,
  clay; a small stream (under 0.05 m2/s a metre of width), under 0.5 m deep, over land whose band is dirt, dirt;
  else sand [0086].
- **Lakes:** under 0.5 m over land whose band is dirt, dirt; to 1.5 m sand; deeper clay [0086].
- **The sea:** to 8 m sand; deeper mud on a band of clay; a warm reef keeps its row (sand and coral) [0086].
- Grass, snow and ice never: no rule returns them. The band under a bed is the bed's block (clay under mud), the rock
  below as before. Banks above the waterline keep the biome's ground [0086]. (Polar ice caps lay their sheet over
  open water by their own rule, ADR 0107, owned elsewhere.)
- Blocks used: `sand`, `clay`, `mud`, `gravel`, `dirt`, the provinces' rocks. Kept bakes stay valid; existing saves
  keep their edits and every unedited bed generates by this rule on the next load. Test:
  `world::baked_tests::the_beds_under_water_are_sand_clay_gravel_stone_or_dirt` (1536 planet, every second column)
  [0086].
- The bed is chosen per column; a stream narrower than the map's cell takes the cell's slope. `Map::roughs` takes
  its slope from the same `cell_slope`, bit-identical to before [0086].
- Sand and gravel fall (ADR 0079), so a bed of them over a cave falls when an edit wakes it [0086].

## Seasonal looks

- **Data.** A row of `data/blocks.toml` may name `[block.spring]`, `[block.autumn]`, `[block.winter]`, each with
  `colour`, `colour2`, `colour3`, `amount`, `contrast` and nothing else; the row's own look is summer's and a season
  it leaves out is that too. A season's colour brings its own darker and lighter parts unless named. `stagger` is 0
  to 10 days, refused on a row without seasons (`Registry::parse`). `blocks::Def::tone(season)` gives a season's
  look [0061].
- **First rows with seasons:** grass, the tundra, cold, meadow, scrub and marsh grasses, broadleaf leaves, shrub,
  reed. Spruce, jungle and acacia leaves, the savanna and jungle grasses and the cactus have none [0061].
- **Look table.** Four rows a block in the one look table (`looks[season * 256 + block]`, 64 bytes a row, 64 KB); no
  second table [0061].
- **The season at a place** is `sky::season_at(up, t)`: the year's phase in quarter turns (the south half a year
  on) and a strength, a smooth step on the latitude's sine from 10 to 35 degrees. The shader's `season` is its
  port [0061].
- **In the shader.** Each body carries its year's phase in the free float of `Body` (negative: no seasons; only the
  planet has them). `look_in` returns a block's row untouched when the block has no seasons (bit 8 of the row's
  `size`) or the place has none; else the two nearest seasons' rows mixed by the phase, mixed with the row's own by
  the strength. Terrain fragments, effects' pieces of blocks and a ray's seen colour all take it [0061].
- **Stagger.** A column's phase shifts by up to the row's `stagger`, two thirds by its patch of 4 x 4 columns and a
  third by the column, hashed on level-0 columns. A ray's seen colour takes no stagger [0061].
- **Painted seasons.** `data/textures/<block>[_top|_side|_bottom]@spring|@autumn|@winter.png` beside the base files
  (ADR 0045's rule each); the file without one is summer's. A face a season does not paint is the base's; a summer
  or unknown season is refused by name (`textures::Set::seasons`). Each season's paint goes in its look row's `tex`,
  `tex2` and `mean`; a season's paint alone gives a block seasons (bit 8 of `size`); bit 9 says a season paints its
  own. The shader mixes in the two nearest seasons' rows that paint this face only under bit 9; the seen colour
  mixes the rows' means. `blocks::SEASONS` and `blocks::ROW` name the seasons for files and tabs [0061].
- **Hotbar, "looking at" cube and block in hand** show the season where the player stands (`gpu::season_here`:
  `sky::season_at`, none off the planet), mixed as `look_in` mixes (no stagger); painted blocks by their rows'
  means. The cubes show summer's paint only (ADR 0098 on the cubes) [0061].
- **The Studio** (also studio-and-art): the Textures page has four season tabs (summer is the base files; Add season
  copies the base's files unsaved and writes a `[block.<season>]` with the base colour, Remove deletes both), a Year
  slider (day 0 to 53, each tab sets the day nearest its start) and North, Equator, South chips. The game view takes
  `season north|equator|south`: the seasons whole everywhere, as at that latitude (`Body.year` 8 and on is the phase
  at full strength), or none. `check=1` takes the season files, `shot=` takes `season=`. `data/palette.toml` has the
  ramps `autumn` and `dry grass` [0061].
- Check tool: `python -I tools/year_sheet.py out/x.png <run=stand's arguments>` makes the 12 frames of a year
  (`textures=dir` for scratch paint) [0061].
- **Cost:** about 0.03 ms (2 %) of GPU on `stand` and `orbit`; 48 KB more in the look table; nothing on the frame
  thread or workers [0061].
- Ground snow and bare trees were left outside 0061; lying snow is the weather's (below) [0061, 0076].

## Weather on the wire and in the world

- **Wire** (`engine::weather`): `Cell = [u8; 6]` per column: cover, the root of rain over 40 mm/h, wind east and
  north in quarter m/s, lying snow over 10 mm of water, temperature + 128. `ToClient::Sky { t, all, changed }`: 13,824
  columns on Terra whole on join (83 kB), then on each 10 s landing only those that moved more than a byte's step.
  The lying snow comes from the grid's `Air::snow` at each landing, not from the saved `View` (saves keep their
  layout) [0076]. The client never runs its own air grid [0076].
- **`/weather clear|cloudy|rain|storm|snow|auto`** (and `weather=` on any run, which also takes a number as a forced
  cover) forces one weather over the planet in what is sent only: the grid, the rivers' runoff, the ledgers and the
  soak's hash are untouched. The fixed views run under `weather=clear` (ADR 0073) [0076, 0108].
- **Cover.** The grid's `View::cover` takes `Air::cloud_share` (the cloud over what its layers hold at `cloud_max`)
  through a smooth step from 0.2 to 1.2 (it was `per_m2 / (per_m2 + cover_half)`, at most 0.01 on Terra). F3's
  cloud reads the same. The sun pass's own shade still uses `cover_half`, so clouds barely shade the bake's or the
  live grid's ground; changing it changes every bake [0076].
- **Rain is not water in the world;** that stays the grid's runoff to the rivers (ADR 0059) [0076].
- **Lying snow.** The terrain shader whitens faces that look up by the column's snow (bilinear over the columns),
  the face's pattern showing through where thin; fully white from 10 mm of water. Nothing in the world changes
  [0076].
- **Wind.** The effects pool's drag pulls a planet particle toward the wind where the camera is (leaves, smoke,
  snow); rain starts with the wind's velocity; the wind's sound rises with its speed. Fluffy crowns and plants of
  planes sway from the same wind at the camera (ADRs 0090, 0091; in a shot `g.water.w` is 0) [0076].
- **Settings:** Graphics "Clouds" (off, low, high; low by default) and "Rain and snow" (off, light, full) [0076].

## Clouds (`clouds.wgsl`, post.wgsl)

- **Slab.** 250 to 314 m over the sea on every planet (`CL_BASE` 250, `CL_THICK` 64; also in `weather.rs`, a test
  holds the two to each other), cells of an eighth of an air column (16 m on Terra, `CL_SUB`). The highest summits
  (350 m) stand above the cloud sea [0105, replacing 0076's 300 m and 0098's 100 m].
- A cell is filled where the column's cover (bilinear, sampled at the cell's middle) beats its own clumped number
  (0.7 x clump + 0.3 x hash + `CL_OPEN`); its top rises with the margin, its edge soft where the margin is thin.
  `CL_OPEN` = 0.06: a cell under it cannot fill [0076, 0098, 0104].
- **Cover mapping.** `cl_cover` maps the air grid's share c to the cover the cells fill to, `c - 1.4 max(c - 0.3, 0)
  (1 - c)` (0.3 gives 0.3, 0.6 gives 0.43, 0.95 gives 0.90), in the clouds, their shadow field and the sky's grey
  (`weather::cl_cover`, a test holds it to the shader's line) [0108].
- **Edges and light.** A cell's edge fades over a margin of 0.15; the light falloff `CL_K` is 0.09. The shadow is
  `1 - 0.8 smoothstep(field)`: a thin edge lets most of the sun through, a thick body takes 80 % [0108].
- **Motion.** The cells slide with the column's wind in two phases half of a 240 s period apart; the client blends the
  last two landings over the 10 s step so cover moves instead of jumping. A step landing mid-blend goes on from what
  showed. A joining client sees the current step at once [0076].
- **March.** Marched after the water (premultiplied) in up to 12 (low) or 24 (high) steps a pixel, 4 m steps near the
  camera (still capped by the setting), start by interleaved gradient noise, ends at the sea's sphere (no cloud
  under the sea level, no cloud through the planet). Lit by Sol by the sun's height where the cell is (the night
  side's are dark from orbit), grey to slate with held rain, fading into the air as far ground does [0076, 0098].
- **Shadow.** One sample where the sun ray crosses the slab's middle, in `light()` (so ground, bounce hits and
  particles get it). The soft shadow `cl_soft` reads the same field as the cells but smooth; the clouds themselves
  stay blocky. Clouds shadow only the sun's light, not the moon's; they are not in the traced scene, so reflections
  do not show them [0076].
- Shortcut: the cells' pattern and the cover step at the cube's twelve edges (each face's texture is sampled alone)
  [0076].
- **Cost accepted:** GPU median frame +0.8 ms with clouds low, +1.5 ms high (about 0.35 of it the shadow in
  `light()`); the frame thread gains a 166 kB texture blend at about 6 Hz and the drops' spawning [0076].

## Rain, snow, storms and sound

- **Rain and snow** (`client::weather::Fall`): drops of the effects pool started over the camera, upwind by their
  fall, landing on the floor of the column they are over (re-done as they cross columns), so a roof or a cave's
  ceiling stops them; a small splash where they land, a ripple on water. Snow below 0 C at the camera: slow flakes
  with drag toward the wind. Spawning stops at two thirds of the pool's cap [0076]. Rain density 350 / 1,100 drops a
  second at 4 mm/h for light and full [0076].
- **Only under the clouds.** `weather::from_cloud(height over the sea)` is 1 under the slab's base falling to 0 at its
  top; a drop's spawn point draws against it (no draw under the base) and the rain's sound at the head is scaled by
  it (weighed where drops start, `RAIN_UP` + 2.5 m) [0108].
- **The sky** greys with cover and darkens with rain [0076].
- **Sound:** `rain` through the open sky, `rain.in` (low-passed) for the share of nearby drops that land above the
  head (a roof) [0076].
- **Storms:** rain over 8 mm/h. Each stormy column within 6 of the camera's strikes in a seeded share of 2 s slots
  (`strikes`: seed, column, slot), so every client sees the same strikes from the bytes alone; under a storm over all
  13 x 13 columns a strike comes every 40 s, every 20 s in rain twice as heavy [0076]. A strike within 2.5 km flashes
  (two flickers and a fade that lights every lit pixel by its albedo, weaker far off); its `thunder` plays from the
  strike, late by distance at 343 m/s, in the head (no position; gain by distance, the sound rows' reach caps at
  200 m). A later strike's flash keeps the latest; thunder's queue clears on a clock jump. The lightning's height is
  the slab's base (`CL_BASE`) [0076, 0105].

## Light snapped to texels [0108]

- **Where.** `terrain_lit` (every block face, cutout blocks and the crowns' fluff cards) takes its direct light at
  `lit_at`, the middle of its painting's texel (16 a cell edge; a coarse LOD level's as large as it draws them).
  Fine 32 px paintings (ADR 0090) are lit 2 x 2 painting texels a light texel. The functions sit between
  `// ---- the light's texels` markers in shader.wgsl (`tx_grid_of`, `tx_point`, `tx_snap`, `tx_seed`, `tx_k`).
- **On the cube-sphere grid itself:** a body-frame point to (face, column i, column j, layer), the two in-face
  coordinates to their texel's middle, back to the body frame, the face's own axis untouched. The fragment uses
  `local` (the painting's own point, so a swaying crown's light rides with its texels); `gi_trace` uses the
  G-buffer's point and normal.
- **Past a texel a pixel.** `tx_k`: where one pixel spans more than a texel (about 54 m at 1440p, 40 m at 1080p
  facing a face), the light's texels double until one spans a pixel again, on the same grid of whole texels. The step
  stops at a block (or a coarse level's painted texel); past that it returns 0, the pixel's own point.
- **Sun and lamps without random numbers.** `sun_seen` and `lamp_seen`: 2 rays, 4 in a penumbra (a penumbra is flat
  steps of a quarter); the sharp-shadow setting keeps one ray to the sun's middle; the moon is one ray to its
  middle. The sun's pairs lie on the disc's diagonals in a frame from the local up. A repeated lamp end takes the
  earlier ray's answer; the glass tint is the mean of the rays that got out. Up to 2 to 4 rays a lamp for the three
  strongest. At a bounce's hit (`seen`): one ray to the sun and one a lamp (`one_ray`).
- **Bounce.** `gi_trace` leaves from the texel's middle, its direction from the texel's seed and the frame, so every
  pixel of a texel shares one sample a frame; it snaps only where the normal is within about 8 degrees of a grid axis
  (no slope, plant, model or effect snaps), else keeps the pixel's point and seed.
- **No texel grid** (plants' stills, models, particles, water, glass panes' own pixels): their own pixel's point, but
  the same noiseless `sun_seen` / `lamp_seen`. Sloped tops keep the pixel's point. `fs_still` passes a footprint of
  -1 to `terrain_lit` ("light where you are"); a test holds it.
- **`direct_smooth`** (post.wgsl, ADR 0098's 3 x 3 smoothing of the direct light, albedo put back) is kept; it now only
  softens a texel step's edge by a pixel.
- **Cost accepted** (not optimised, ADR 0096): about +0.5 ms GPU at 1080p, +0.7 to +0.9 ms at 1440p, mostly
  `gbuffer` near the ground. Ideas: trace the second sun ray only where neighbouring texels disagree; skip penumbra
  rays past `tx_k`'s first doubling; cap the cloud march at a lower transmittance where the slab is thin.
- Known: a face seen exactly edge-on may pick the wrong axis for the bounce (smoothed anyway); `seen()` still takes a
  `px` it no longer uses past the bounce direction.

## The globe map (`crates/client/src/globe.rs`) [0077]

- **Opening.** M (rebindable, "Globe map", Keys page) opens `Menu::Globe`: an orthographic globe, north up; a drag
  turns it, the wheel zooms 1 to 64 times. A click drops a waypoint on the ground there, a click on a waypoint opens
  its page, a right click removes it. Buttons: Terra, Luna, Back. Latitude and longitude under the pointer use
  `nav::geo_text`. Holding M does not flicker it (key repeat does not reach the menu's close).
- **Picture.** The bake's map at mip 0, resampled once on a worker into latitude-longitude RGBA8 (Terra 1024 x 512,
  Luna 512 x 256): biome colours, sea by depth, lakes, every river cell, hillshade from the NW (ice cells in the ice
  colour over land, lake and sea alike, ADR 0107). About 82 to 109 ms, off the frame thread.
- **GPU.** One storage buffer (binding 20, 2.70 MB VRAM) holds a 128-byte view, both pictures and both seen-masks. The
  UI shader draws the globe in one rectangle (glyph 200): 3 x 3 pixel blocks, night side where Sol really is, the
  unseen bare, a line every 30 degrees. CPU per open frame: the view write; masks only when they change.
- **Seen-mask.** A bit per texel, marked out to the horizon from the eye's height over the ground (row spans by the
  law of cosines; twice a second at most, after a move), kept as `<save>.seen` (80 KB), written to `.tmp` and renamed
  every 60 s and on exit. Creative shows everything; survival shows what was seen. It is client-side beside the
  save, like waypoints; a client joined with `connect=` has no save, so it lasts a session there (shortcut until
  per-server files). Luna is told by its map's version (Luna's count from 101, `bake::luna`).
- **Waypoints in survival** are on the globe and in the same places as N and the compass strip, in both modes; where
  you are in numbers stays creative's (the compass strip names only the body, the chat's line for a new waypoint has
  no latitude and longitude, the globe shows none under the pointer); survival can drop a waypoint only on land it
  has charted. Defaults, open to change.
- **Markers** are ordinary UI rectangles: you (ring and facing tail, filtered by id, not position), other players
  (white, named, shown everywhere), waypoints (icon, colour).
- **Server.** The player list goes out once a second as well as on joins and leaves, so positions are current (about
  150 KB/s at 50 bots; kept) (also multiplayer-saves-and-data).
- Edits do not show (a texel is 24 m); a "what you built" layer is for later.

## No longer holds

- 0020's blue veil over the screen under water: replaced by 0071's composite dimming and fog.
- 0020's "the sea is one level": the level is the mean plus the tide, 0067.
- 0020's water mesh drawn without tide re-meshing: amended by 0067 (every 4 s of world time for wet tiles).
- 0071's first foam by four rays across the surface: dropped before the build for depth at the view ray's hit.
- 0076's cloud slab of 300 to 364 m, and its 8 m march step: replaced by 0098 (100 m, 4 m near the camera), then the
  base back at 250 m by 0105 (steps stay 4 m near the camera).
- 0098's cloud base of 100 m, `CL_K` 0.14: replaced by 0105 (250 m) and 0108 (`CL_K` 0.09).
- 0076's hard-edged cloud shadow of the 16 m cell: replaced in 0076 itself by the soft `cl_soft` shadow (0073's note
  that the shadows are hard by design is out of date).
- 0076's rain that spawns wherever the column's air says it rains, even above the slab: replaced by 0108's `from_cloud`.
- The sun's shadow by a dithered point of its disc (`in_disc`), the lamps' `lamp_target` random point and the
  bounce's per-pixel seed (0022, 0072, 0098's interleaved-gradient disc point): replaced by 0108's noiseless rays
  and texel light.
- 0077's status "not merged": it is merged (the globe is in `crates/client/src/globe.rs` and used by 0107).
