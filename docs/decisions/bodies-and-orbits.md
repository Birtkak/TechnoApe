# Bodies and orbits

Terra, Luna and any other body as rows of `data/bodies.toml`: parents, poses, the sky's clock, tilt and seasons, tides,
eclipses and the moon's look, and the magnetic field. What holds today. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- **Luna is a fixed part of Terra**: always 3.7 times smaller, 30 Terra diameters away centre to centre, as the Moon to
  the Earth, so a total eclipse on Terra is a perfect one; dynamic with the planet's size (2026-11-11). [0041,
  amending 0027]
- **Totality is widened** to some 115 m across, a few seconds at any spot (the sun is 0.95 of Luna's size seen from
  under her at new moon). [0041]
- **Luna's orbit leans 2 degrees** (chosen from a table of tilts and how often eclipses come); "common eclipses too
  cuz they are cool"; Luna's day of one month (8 Terra days of sun, 8 of night, 16 to the month): "perfect". [0037,
  0041]
- **A 30 minute day** (`sky::DAY` = 1800 s). [0041]
- **Seasons from an axial tilt** were chosen as a live system of the world (2026-11-10). [0026] The tilt is 25 degrees,
  raised from 20 on the developer's trial. [0055, 0060]
- **Tides follow Luna:** 1 m from low to high at spring tides (2026-11-12). [0067]
- **Keep the orbit, glow the night:** an empty moonless sky is not a bug; a faint blue night glow in air instead.
  [0041]
- **The stars are fixed in space and turn as the body turns**: "keep it true". [0041]
- **Seasons stay close to realism** (the seasons reach the whole air). [0060]
- **Open taste calls:** the tilt (`TILT`, 25 degrees; 23.4 is the Earth's), the year's length (54 days of 30 minutes,
  27 hours of play, a season about 7 hours) and the season's strength (`SEASON_K`). [0055, 0060]
- **Bodies are data, any number of them**, a body posed on its parent, the GPU's four bodies a stated limit
  (planet-model.md's order of steps). [0163]
- **The magnetic field is a value of the world** and its first effect is the aurora; draining the core must change the
  sky. [0164]

## The bodies

- A body is a row of `data/bodies.toml`; a row's place is its index, on the wire, in saves and in every frame; never
  reorder rows. Row keys of 0163:
  - `parent`: `"sun"` or the name of a row **above** it (a pose is worked out down the rows, never round a loop).
  - `home = true`: the home world, whose are the spawn, the ore and its ledger, the weather, the tide and the herds.
    Exactly one row, and it must be the **first** (checked): every frame index 0 stays the home's. It circles the
    sun and has a moon (`bodies::moon`, its first moon, which sizes the sun and is the one the sky's eclipses,
    tides and moonlight are of).
  - `bake`: `"terra"` (ADR 0043) or `"luna"` (ADR 0070); none: the seed's noise alone. [0163] (the row later carries
    `stages` in place of `bake`, 0165)
  - `orbit` is in the **parent's** radii (Luna's parent is Terra, so her 60 stands) and never nearer than
    `orbit_min` metres; a body that circles the sun and is not home has `size` and `sun` (its distance in its own
    radii) and no `orbit`.
  - Terra: `parent = "sun"`, `home = true`, `sun = 23500.0` (1 AU in Earth radii); Luna: `parent = "Terra"`,
    `size = 3.7`, `orbit = 60.0`. [0041, 0163]
- **The sun's size** is derived in `Sky::new`: seen from the ground right under Luna at a new moon it is
  `SUN_UNDER_MOON` = 0.95 of Luna's size. On the 4 km Terra: Luna 1150 m in radius, 255 km out; the sun some 435 km in
  radius, 99.7 Mm out. `Sky::new` asserts the moon clears the planet's ground. [0041]
- **Where grids lie:** body i's grid lies at `i * MOON_AT` in the shared coordinates (`MOON_AT` = 10,000 km on the x
  axis; the inertial origin is -10^10 m), so no body's frame (1.5 x its sphere of influence) reaches another's at any
  size. [0041, 0163]
- **World:** `World::of(rows, [(columns, map)], seed, warp, gen)` builds any rows; `with_grid(planet_n, moon_n, ..)`
  stays for tests and the command line (the home gets `planet_n`, its first moon `moon_n`, any other body its row's
  `size`, `bodies::sizes`). `World::moon` is the sky's moon; the client's moonlight index, the HUD and `bench`
  `look=<body>` use it or the row's name. `planet=` and `moon=` are unchanged. [0163]
- **Poses** (`World::pose`, `sky.rs`): the inertial frame stays centred on the **sun** (`Sky::sun` is the origin, the
  home circles it, `Sky::round_sun` places other bodies); the home's grid is at the origin of the shared coordinates.
  The home is posed by the sky (`Sky::planet`). A moon is on `Sky::circle` around its parent's pose at `orbit` parent
  radii, face to the parent (tidally locked), once a month by Kepler's law against the home's first moon for a
  parent as dense (`(q / q_luna)^1.5`, q the orbit in parent radii), rounded to a whole fraction of the sky's
  `PERIOD` (`sky::period`) so the sky still repeats. `Sky::moon` is `circle(planet, moon_orbit, MONTH)`; Luna's
  period comes out `MONTH` exactly (PERIOD / 27), so Terra and Luna pose to the bit as before. [0163]
- **A body that circles the sun and is not home** (`Sky::round_sun`): a circle in the ecliptic at its distance,
  starting where the home starts, once a year by Kepler's law against the home's (rounded to a fraction of `PERIOD`),
  not turning (identity rotation, spin 0). A stand-in for real orbits. Its sphere of influence is the home's; a
  moon's is a quarter of its orbit (Luna's as before). [0163]
- **The GPU's limit:** `engine::bodies::MAX` = 4 (a world of more is refused by `World::of`; `World::gravity` keeps
  its centres in an array of that size); `gpu::MAX_BODIES` stays 4 (shaders mask a body id with `& 3u`). The client
  refuses to start with a message when `data/bodies.toml` has more rows (`gpu::bodies_fit`). The sky shader's
  nearest body looks at all four slots (an unused slot has radius 0). A fifth body needs the GPU's body slots and the
  id bits in the quads widened first. [0163]
- **Formats** (a list instead of two named bodies; every older world keeps generating and replaying bit for bit):
  - Save **version 22**: `Save { bodies: Vec<(i32, Option<u64>)>, .. }` (columns and map hash per row) in place of
    `planet_n, moon_n, map, luna`. A 21 or older is read through `Save21` and becomes `[(planet_n, map), (moon_n,
    luna)]`; it keeps its version until written again. A world whose save lists another number of bodies than the
    rows is refused (`Server::on_map`). [0163]
  - Welcome: `ToClient::Welcome { id, seed, bodies, blocks, items, gen }`; the client, the bot and the title screen's
    download fetch every body's map by its row's bake. No wire version: client and server are one build. A client
    given another number of bodies than its rows refuses with "the server has N bodies, this game M: update the game"
    (`bodies::known_bodies`). [0163]
  - Recordings: `Ev::Start { bodies, seed, dt }` and `Ev::Loaded(Save)` are new variants at the end; the old ones stay
    in place as `Start21 { planet_n, moon_n, seed, dt, map }` and `Loaded21(Save21)`, so an old recording replays as
    before. [0163]
- **Locks:** `made_hash` for seed 1 planet 6144 (`mantle=0` 0926f028ddcd85a7, `gen=1` 37b2d1b1006b66f7, `gen=2`
  a2ef250b3ed4203a, default bfb878ce14d32b75) and the `record_tests` bit lock hold; Terra and Luna pose as the sky
  does (`terra_and_luna_pose_as_the_sky_does`). [0163]
- **Open:** real orbits (planet-model.md "Orbits are real"): the sun-parent pose and the Kepler rounding are
  stand-ins; the home must stay the first row (lifting it means every `bodies[0]` of the home's systems); eclipses,
  tides and moonlight count only the home's first moon; the globe overlay and the waypoint colours know Terra and
  Luna by name; a world made before keeps its bodies, so a new body needs a generation number. [0163]

### Mars is a body in the world [0173, 2026-11-19]
A new universe (world generation 5) has three bodies: Terra, Luna and Mars round the sun; worlds made before keep
two, bit for bit. Mars's row moved from `data/drafts/bodies-mars.toml` (deleted) to the end of `data/bodies.toml`.
- **The `since` rule:** a row may state `since = k` (default 0), the first generation it is in; `bodies::parse`
  refuses a home with `since > 0` and a row older than the one above, so a generation's bodies are always the first
  rows (`bodies::of(gen)`). Mars has `since = 5`; `world::GENERATION` is 5 (`known_generation` takes 5, refuses 6).
  Every place that turns rows into bodies takes the world's generation: `World::with_grid` (and so `baked`,
  `report=1`, sys), `Server::on_map` (a save's body count against its generation's rows, not all rows: a two-body
  version-22 save of generation 4 loads, `tests/fixtures/v22-gen4.sav`), the client's and bot's welcome
  (`known_bodies(n, gen)` after `known_generation`: another count is "update the game"), the soak's bots; saves and
  `Ev::Start` already carry the list (0163). The title menu's "Making the universe" bakes the rows of the new
  world's generation (`menu::world_bakes`: a `gen=4` world bakes no Mars) and shows Mars's stages ("the planet's
  craters", ...) in its bar. A save whose list is empty or short is refused with its count, not an index panic.
- **Its map in a world:** `with_grid` bakes any body after the home's moon by its row's bake when the home is on a map;
  `Body::with_luna` takes a Mars map as `with_map` with no sea and **no caves** (its marked tubes wait for its rock).
  `bake::find` finds a kept Mars map by hash (`mars::find`, by its row's file name), and a map fetched from a server
  is kept under that name (`mars::keep_any`; shortcut: the one planet's row); the server's map hashes in the
  welcome include Mars's. `Map::crust()` (Luna's or Mars's: no sea); the height clamp is `AMP` on a Mars map (its
  352 m summit); river and lake water only on Terra maps of version 4 to 7 (`Map::holds_water`: Mars's beds are cut
  but dry). `Body::rock_body` is Mars's own row on its map.
- **Ground per kind** (`data/biomes.toml`, rows 22..=32 after Luna's; top, band to 4 m, rock below), so that the
  real planet's contrast of bright dust and dark basalt reads from orbit: cratered highland red dust / mars regolith
  / breccia; dust plain red dust / mars regolith / basalt; lava plain basalt sand / basalt / basalt; basalt shield
  basalt sand / mars regolith / basalt (dark as the low shield Syrtis Major; the real tall Tharsis shields are
  dusty, a simplification); canyon floor mars regolith / gypsum / basalt; chaos mars regolith / breccia / basalt;
  outflow channel mars regolith / mars regolith / basalt; sediment basin clay / gypsum / gypsum and delta clay /
  clay / sandstone (light-toned); polar ice ice / dusty ice / basalt; frost frost / mars regolith / breccia. No
  plants, clutter, patches or snow on any.
- **New blocks** (end of `data/blocks.toml`, colours and patterns from the rows, no paintings): 222 red dust (the
  iron-oxide dust: iron and pigment), 223 mars regolith (broken basalt and dust: sintered bricks), 224 breccia
  (rubble 197; the impact-welded old crust: building stone), 225 gypsum (rubble 198; the dried lakes' sulfate:
  plaster, water when heated), 226 dusty ice (the caps' layered ice: water when melted and filtered), 227 frost
  (carbon dioxide frost: carbon dioxide), 228 basalt sand (the dark windblown grains of the young lava: casting and
  glass sand). No recipes. Rubble 199 is the last of 160..199. (Ids past 255 are allowed
  since 0170.)
- **From its row:** its interior (gravity 8.40 m/s^2 against Terra's 19.62, 0.43; field 0.0000: no aurora), no air,
  no sea; its pose is 0163's sun ring at 1.5 AU (year 1.84 of Terra's). **Still Terra's or none:** it does not spin
  (0163's identity rotation: its day is its year, the sun crosses its sky once in 1.84 Terra years); weather,
  clouds, aurora, seasons, tides, plants and herds are the home's (body 0's frame) and none reach it; the sun's
  light is one strength everywhere (its disc is drawn smaller from Mars by distance alone); the globe overlay (M)
  keeps Terra and Luna only (its seen masks are two in its file and GPU buffer); `MOON_AT * 2` places its grid.
- **Left** (each its own run): rock provinces under its ground stack (`rocks.toml` rows with `body = "Mars"`, so
  the band's 4 m is not the whole column's change: the caps are 4 m of ice over basalt); ore (iron oxides,
  sulfates, clays, ice) by those provinces; caves and lava tubes from its 11 marked tubes (`caves.toml`'s round
  tubes in its shield basalt); thin air (about 0.01 atm), its butterscotch sky and dust; real orbits and travel
  time; Luna's row stopgaps (`weight_g`, Terra's densities); a minimum size for a far body seen as a point (from
  Terra Mars is far under a pixel, and Terra from Mars).
- **Locks:** Mars's map seed 1 n 832 552a1c34492c457f, n 3328 8703842819e918b5 (as `bakemap=` made them before);
  Mars's generated blocks at generation 5 (`mars_generates_as_locked`) 3b6b6a314af6caa5; `made_hash` unchanged
  for every generation, Luna's c6a367341bda1227 / cdd810732c670a46 and her blocks b0f00158257f21d2 at 4 and 5.
- **Costs** (play build, seed 1, 4 km planet; `docs/perf-debt.md`): Mars's bake 3.1 s once; the world with its map
  kept 1.7 s against 1.5 s with two bodies; standing on Mars RAM 1.8-2.1 GB, VRAM 1.08 GB; on Terra VRAM 1.34 GB.

### A gas giant, and far bodies as points of light [0174, 2026-11-20]
A new universe (world generation 6) has four bodies: Terra, Luna, Mars and the gas giant, working name **Jupiter**
(the name is the developer's to change), round the sun; worlds made before keep theirs, bit for bit. The developer's
calls: gas planets after Mars; a gas planet "can be entered and has a solid core"; its size a real giant's, about
300 Terra masses and 11 Terra radii (2026-11-18). This run makes it exist, be seen and be approached.
- **The row** (end of `data/bodies.toml`, `since = 6`): `kind = "gas"`, `parent = "sun"`, `size = 1/11` (46.7 km on
  the 4 km Terra: 67,584 columns' worth of radius, no columns of its own), `sun = 10681.8` (5 AU in its radii),
  **`day = 0.4`** (new key: a sun-circler's sidereal day in the home's days, 1080 whole turns a sky `PERIOD`; none:
  it does not turn, as Mars), **`colour`** (new key on every row: its point's colour; Terra blue, Luna grey, Mars
  red, the giant cream; grey if left out), no `ground`, `generator` or `stages` (refused on a gas row; a rocky row
  must have ground and generator). Its makeup keeps the rocky keys with a gas body's meaning: `iron = 0.8` the
  conducting fluid's (metallic hydrogen's) top over the radius, `heat = 0.9` leaves a solid core a tenth of that
  (3.7 km, between Luna's 1.15 and Terra's 4.2 km), `density = [500, 500, 1700]` the molecular gas's twice and the
  fluid's (the core reckoned at the fluid's). No mass key was needed: the densities state the mean (1114 kg/m3, the
  real one's 1326), and the interior derives the mass as it does a rocky body's.
- **From the interior** (`Interior::new`, the same rules): **300.7 Terra masses**, **48.75 m/s^2 at the cloud tops,
  2.48 times Terra's 19.62**; the pull inside the gas by the same profile (rising to the fluid's top at 0.8 R, then
  falling a little through the light gas over it, nothing at the centre); **field 3.71 of Terra's** by 0164's rule
  (liquid share 0.51 over Terra's 0.34, times a spin 2.45 times hers): stronger, not the real one's 14 times, because
  the rule is a share of the volume times the spin and stays one rule for every body. Its aurora by 0164's oval rule:
  a ring at 79.8 degrees latitude, brightness 1.94, drawn on its night side by the giant's own shader (cheap: a term
  in it; not 0164's curtains, which are the home's only).
- **Sphere of influence:** a sun-circler's is the home's (0163) or Laplace's rule from its mass and orbit scaled from
  the home's (`home x a/a_home x (m/m_home)^0.4`), whichever is larger (`World::soi`): the giant's **7.49 Mm, 160 of
  its radii**; Mars's would be 0.65 of the home's, so it keeps the home's, to the bit. A gas body's grid lies its
  index times `GAS_AT` (100,000 km) out, not `MOON_AT`, so that sphere never reaches another body's grid.
- **What a gas body is to the engine** (`Row::gas`, `Body::gas`): a `Body` with no blocks. Its grid numbers exist only
  as its radius's; no map, bake, chunks, streaming, ore, caves or save data (a save and the welcome carry it as
  `(columns, None)`). Guarded where a grid is read: `World::block`, `generated_under` and `generated_solid_under`
  answer air; `World::set_block` does nothing (so no edit, `/place` or structure lands there); `Body::height` and
  `col_height` answer the cloud tops (so `top`, `stand_point`, `/goto`, `run=stand body=3` stand there); the streamer
  makes no roots for it; F3 says "gas" under Size and Feet in. Gravity sums it with the others (a generation 6 world's
  Terra feels it, 4e-7 m/s^2; a world of 5 or before has no row for it, so its bits are as before).
- **Below the cloud tops (not built):** the cloud tops **hold** a player as a floor (`phys::blocked`: inside the
  radius is blocked), so `/goto jupiter` stands on them on the day side and a fall lands there. Chosen over falling
  in because the dive is its own run (pressure, heat and hull limits): a fall would swing a player through the centre
  at some 2 km/s into a place drawn as nothing, with no way out but `/goto`; a held place is valid, saveable and
  reloads in the giant's frame. A player found inside the sphere (a bad save, `at=` under the radius, a teleport)
  is put on the cloud tops along its radius at its next step (`phys::step`), its inward speed gone. What is laid
  there rests where a player stands: `lay` (a death's pile, a throw, a knocked-off torch) takes the cloud tops'
  cell as its floor, never under it; a bucket is not poured on a gas body (nothing would hold the water); no block
  can be set there, so nothing falls; rain feeds the home's reaches only and herds live on the home only.
  Its frame does not turn (0163's ring); its bands turn in the shader by the clock, so the clouds drift under a
  player held there at some 400 m/s at the equator. A frame that turns with its day (as the home's) is not done:
  its sphere of influence is 7.5 Mm, so the frame's own speed at its edge would be some 65 km/s, and a ship coming
  in would be handed that speed. It needs a turning inner frame of a few radii nested in the still sphere
  (`pose` and `reframe` taking a frame inside a frame, the recordings their frame index), or the floor carrying
  a held player round with the clouds (`phys::step` turning a standing player's position by the spin).
- **Drawn by the sky's shader** (`shader_sky_fx.wgsl` `giant`, in `fs_sky`; `frame::far_lights` fills `Globals::gas`):
  a sphere hit analytically per sky pixel, nothing streamed. Belts and zones even in the sine of latitude (16 rows of
  ochre, cream and rust), their edges torn by turbulence stretched along the bands and warped by itself, streaks of
  the next band, a great oval storm 22 degrees south (rust, two wound arms, a pale collar) and white ovals at 33 south;
  all in cells of a power of two metres, about two pixels of its surface and at least 1 m, so the look is the stars'
  and the aurora's blocky texels and holds detail from any distance (the turbulence's octaves reach down to the cell;
  their last five at full weight tear the band edges, shade small eddies and draw thin streaks as the cells shrink
  under some 180 m, from about 10 km over the cloud tops; none from a few radii out).
  Sun-lit with a soft terminator and a darker limb; it turns with its day from the world clock. The disc fades in from
  its point as it grows from 1.5 to 4 pixels, hides the stars, the sun and the points behind it, and is lightened by
  the air when seen through it. Only one gas body is drawn (the first). **Not on screen** (under 1.5 pixels, outside
  the frustum, or its centre under a horizon): its slot is -1 and the shader's only cost is one compare a sky pixel.
- **Far bodies are points of light** (every body, `sky::point_light`, pure and tested): the light it sends,
  `(R/d)^2 x lit share ((1 + cos phase) / 2) x (home's sun distance / its own)^2`, seen as the stars are, `5 x
  (that / 5e-10)^0.4`, at most 8, in its row's colour (each channel 0 to 1, refused otherwise), a glow 1.4 pixels
  wide as the brightest stars'; its brightest channel shows at most 1.0 (the shader's `POINT_CAP`), the others in
  their ratio, and light past that widens the glow by its square root, so a bright point keeps its hue; handed over
  to the disc between 1.5 and 4 pixels across (`disc_share`, so no jump); hidden where another body's sphere is in
  the sight line (`sky::hidden`: the eye's own horizon too), by a light sky (the stars' darkness rule) and by the
  sun's disc where it is farther than the sun (the disc too). From
  Terra at night: the giant at opposition 5.4, Mars at opposition 6.1 and at the far side 2.0, against the brightest
  stars' 1.2 to 4.2. A mesh body's disc (Mars, Luna, Terra) is drawn by its chunks once streamed.
- **Costs:** see `docs/perf-debt.md` ("A gas giant and far points").
- **Left** (each its own run): the dive (pressure, heat, wind and gas mix by depth from the interior; hull limits;
  cloud decks to fly through, drawn as volumes; what replaces the floor); the solid core as a block body (a grid
  under the gas, its own row or a part of this one, its bake and save data); moons of the giant and the body limit
  past four; gas harvesting by depth and floating platforms; the radiation belt (from the field and the spin); real
  orbits (its ring, `day` and grid placement are stand-ins); loading block bodies' chunks and maps on approach (the
  stream keeps Mars's coarse chunks though it is a point: the "Mars a body" row); 0164's curtains on any body but the
  home; a second gas body drawn; the giant casting a shadow on the sky (its eclipses of the sun for its moons).

### Sixteen bodies, loaded on approach [0175, 2026-11-21]
A world holds up to **16 bodies** (`bodies::MAX`, `gpu::MAX_BODIES`, shader_vertex.wgsl `MAX_BODIES`; a 17th row is
refused with "17 bodies; a world holds at most 16"), and a far block body costs next to nothing until it is near.
- **Where a body's number lives now** (no word grew, nothing on disk or the wire changed): a chunk's info word had
  8 bits for it already (`>> 8 & 255`); a particle's info word takes 4 at bits 8-11 (`fx::body_bits`), its look
  moved to 12-13 and a cube particle's face to its own flat varying (`FxOut::dir`, it had borrowed bits 12-14); a water
  quad's word 4 at bits 3-6 (`water::info`), its kind to 7-9; the G-buffer's albedo alpha (8-bit) carries it in
  fifteenths (`gb_alpha`/`gb_body`, exact for 0..15), post.wgsl reads it so. The traced scene's instance mask has
  8 bits, so bodies take **lanes**: the four nearest bodies with instances get one each (`rt::lanes`; sticky, so a
  lane never moves while its body stays among the four, and Terra and Luna alone keep the masks 1|16 and 2|32 of
  before), passed to the shader in `Body.rot1.w`; a body without a lane casts and catches no traced ray (it is the
  fifth out or farther, a few pixels at most). The shaders' loops over bodies walk the set bits of a mask (eclipses)
  or stop at the world's body count (`gas2.y`), not at `MAX_BODIES` (that cost 0.3 ms of GPU at stand). Locked by
  `a_body_past_three_survives_every_packer_and_shader` (the packers' bits for every body 0..15 on the CPU, the
  shaders' lines read from their text, as 0170 does) and `sixteen_bodies_build_and_a_seventeenth_is_refused`.
  Options weighed: 8 (3 bits) costs the same words as 16, so 16; a fifth quad word or a wider mask was not needed.
- **Load on approach** (`World::of_sources`, `Source::Later(hash)`, `world::lazy_row`): a body past the home's first
  moon with a bake (Mars) starts as a **stub**, the body before its map (grid, orbit, interior: no map, heights,
  caves, ores). `World::loader(i)` is a `Send` closure that gets the map of the hash (`Bake::for_hash`: memory, disk,
  or baked again) and runs `World::of`'s own last step on a fresh stub (`Body::on`), so it comes out bit for bit as at
  the start (`mars_loaded_late_is_mars_from_the_start`: the blocks lock 3b6b6a314af6caa5, spawn, layers; out and in
  again the same); `put` takes it in (and splices the server's ore ledger), `unload` goes back to the stub.
  The home and its moon are never lazy: Luna is about 5 pixels across from Terra, so she would stream anyway.
- **The client** streams a body only while it is at least **1 pixel** across (`stream::IN_PX`) and drops it under
  **0.5** (`OUT_PX`, the factor two the hysteresis), or while the camera is in its frame (`Streamer::seen`); the home
  and its moon stream wherever the camera is, as before, unless `drop_home=1`. (Dropping Terra's 18,000 chunks at once
  lost the GPU device: their BLAS were freed under a frame still tracing through the TLAS built before; `rt::Rt::dead`
  now keeps a forgotten BLAS four records. With it the home's drop runs clean, so sizing them out is open to a
  measured run.) Otherwise no root of
  it is walked, and on dropping every slot of it goes, its quad-buffer room and BLAS with it. A body wanted but not
  in is loaded on a thread of its own (`App::bodies_in`), put in when done, and unloaded again (its map and tables
  freed once no mesh job holds them) when the streamer has let it go and the camera is not in its frame. **No frame
  shows neither point nor ground:** the sky hands a point over to its disc only from 1.5 pixels (`sky::HAND_LO`),
  and while a lazy body is not `Streamer::ready` (no drawn walk has yet left no hole in view on it; once ready it stays
  so while it streams) its point is handed over 4 times later, from 6 to 16 pixels (`frame::held_px`): a ground still
  coming in shows the point, and one that never comes whole (a full quad buffer) shows no point on a wide disc.
- **The server** builds a lazy body only when a player is in its frame, a /goto is bound for it, or a client asks for
  its map; on a thread of its own (never the tick, never rayon's pool: `goto_mars_waits_for_its_ground_off_the_tick_and_replays`
  holds the pool), put in at the start of the tick it is done and recorded there (`Ev::Body { tick, body }`; `Ev::Lazy`
  marks such a recording; one without it replays with every body in from the start, `Server::load_all`). It never
  lets one go. **The welcome** carries every body's hash as before (`World::map_hash` answers a stub's); a local
  client makes Mars a stub at once, a remote one checks or downloads its map as every body's (kept under its name)
  and lets it go. The hash is checked when the body comes in: a map this build cannot make leaves it a stub, its
  point and an error on stderr; on the server its waiting /goto's are told so, players near it go to the spawn, a
  /goto there is refused, and the failure is recorded as a load (`Ev::Body`, so its replay fails at the same tick).
  `Ev::Lazy` is recorded only by a world with a lazy body. A map asked for a body not in is queued once a client.
  An old client meets the same welcome.
- **Arriving:** a player within `phys::HOLD_M` (200 m) over a not-loaded body's highest ground (or a step's travel) is
  held where it is on either side (`phys::held_by_a_body`), until the ground is in: a pause, not a brake (its speed
  is kept for when the ground comes, it may look round); one under the ground then (a teleport) is set on it
  (`phys::onto_ground`). /goto to one answers "going to Mars: its ground is loading" and puts the player there when
  it is. Each side holds by its own copy of the body; the server's answers win, so they differ (as corrections) only
  if the server's load is slower than the flight from the body's frame to 200 m. **A stub has no ground**: its cells
  read as air (`World::block`), and the server refuses edits, digs, buckets, pick-ups, strikes, uses and items to or
  from the ground there, saying why; a pile laid there goes to the spawn.
- **Numbers** (play build, `quick=1 seed=1 planet=6144`, interleaved with main's, `slot.sh measure`): Terra stand
  VRAM 1079 -> 823 MB, wgpu's own 776 -> 659 MB, the quad buffer 21.3 -> 14.9 MB, chunks at the load about 21800 ->
  18800, load 2.71-2.75 -> 2.42-2.68 s, frame and GPU p50 the same within noise (3.35-3.56 / 3.23-3.48 ms both);
  fly frame p50 4.23-4.27 -> 3.97-4.03 ms, p99 5.45-5.55 -> 5.03-5.10, GPU 4.17-4.21 -> 3.90-3.96, wgpu 889 ->
  769 MB, load 2.60 -> 2.14 s; orbit max 9.53-9.68 -> 9.02-9.53 ms. Standing on Mars: the same chunks (16350) and
  memory, load 1.31 -> 1.56-1.66 s (Mars built before the stand); its frame numbers are two-state on main and here
  alike (GPU p50 2.24-2.26 or 2.41, RAM about 730 or 1030-1100 MB). Arriving: the server builds Mars in 0.35-0.39 s
  (its map from the disk) off the tick, the client in 0.01 s (the map already in memory); `run=approach` (3,000 km
  to 2 m) frame max 5.8 ms, p99 4.6, no hole; `run=leave` max 5.6 ms. The body limit itself: stand and fly unchanged
  (3.55-3.57 / 3.54-3.59 ms p50).
- **Left:** the home and its moon sized out by default (above); the hold made exact on both sides (a word on
  the wire); the client's first frame of a run that starts on or
  arrives at a body before its ground is in shows its sky without it (`run=leave`'s first frame, about 0.35 s);
  the server never lets a body go; a lost load's bake on rayon's global pool shares it with the mesh workers;
  Luna and a moon of Mars or the giant made lazy too; a load cancelled when the camera turns away before it lands.

### The giant's icy moon [0176, 2026-11-21]
A new universe (world generation 7) has five bodies: the fifth is the gas giant's moon, working name **Callisto**
(the name is the developer's to change), the first body whose parent is neither the sun nor the home and the first
of `kind = "icy"`; worlds of generation 6 or before keep their bodies, bit for bit. The developer's call: "lots of
rocky and gas planets and moons"; step 6 names the giant's moons and icy bodies.
- **The row** (end of `data/bodies.toml`): `parent = "Jupiter"`, `since = 7`, `kind = "icy"`, `size = 4.0` (1536
  columns on the 4 km Terra, 48 chunks, 3 x 16 as Terra's 192 are 3 x 64; 1061 m in radius, 0.92 of Luna's, the real
  pair's 0.91), `orbit = 8.0` (the real one's 26.3 brought in, real-world logic exaggerated for small worlds, so the
  giant hangs large in its sky: 374 km, 7 giant radii over its cloud tops, in the giant's 7.49 Mm sphere), `stages = ["crust", "impacts", "ice"]`, `ground = ["dark lag", "dusty ice", "ice"]` (where no map), `air =
  false`, `colour` dark grey-brown, `iron = 0.5`, `heat = 0.0`, `density = [950, 1700, 3000]`.
- **What icy means to the interior:** the same rules as a rocky body (`Interior::new`), the keys read as a rock core
  (`iron` its radius share, `density[2]`) under a mantle of ice and rock and a crust of dirty ice: mean **1863
  kg/m3** (the real one's 1834), **1.85 m/s^2** at the ground (0.35 of Luna's 5.31, 0.094 of Terra's; a jump some
  13 m), heat 0 so no liquid core and **field 0**. `bodies::check` no longer refuses icy. An icy row with stages bakes
  on a `crust` (refused on plates) and lists no `volcanism` (refused: "not built yet on an icy crust").
- **The pose and period of a moon of any parent** (`World::of_sources`): on its parent's pose (0163's recursion: the
  giant on its sun ring, the moon on `Sky::circle` round the giant), its body -x to the parent (tidally locked: the
  same face to the giant, tested over an orbit), once a period by Kepler's law against the home's first moon, now
  times the root of the home's mean density over the parent's (`mass / radius^3`): for a moon of the home the factor
  is 1 to the bit, so Luna's MONTH and pose and every older world are unchanged (tested). The giant's moon:
  Kepler 2,950 s, rounded to PERIOD / 264 = **2,945 s, 49 minutes of play** (its day: the sun crosses its sky once an
  orbit; at the real 26.3 radii it was 4.9 hours). Its **sphere of influence** is Laplace's rule against its parent
  (`orbit x (m / M_parent)^0.4`, at most a quarter of the orbit): **4.9 km** (4.6 of its radii; the quarter-orbit rule
  would have given 94 km), inside the
  giant's; a moon of the home keeps a quarter of its orbit, to the bit. `World::locate` and `reframe` take the
  **innermost** sphere (the rows backwards: a moon comes after its parent), as spheres now nest; with disjoint
  spheres (every older world) the answer is the same. Gravity is summed as before, its parent's pull cancelled near
  it by 0041's rule.
- **Its parse rules** (`bodies::parse`): a moon's ground must clear its parent's with room for both reliefs
  (`(orbit - 1) x parent radius - its radius > share x (parent radius + its radius)`, in home radii, the share `AMP`
  over the home's radius: touching grounds are refused, "its ground would be inside its parent"); a parent below it
  or itself is refused as before; a **baked moon of a moon** is refused ("not built yet": its polar sun height would
  read its orbit in its parent's radii as the home's). The polar-shadow sun height (`luna::sun_high`) for a moon of a body that is not the home is the home's
  moon's sun disc times the home's sun distance over its system's (the giant's 5 AU: a fifth), not a disc reckoned
  from its own size and orbit; Terra's, Luna's and Mars's are as before.
- **Its bake** (`bake/icy.rs`, map version **301**): Luna's `crust`, `impacts` (no maria) and `ice` stages, then
  the two widest basins made **multi-ring** (their middles 0.9 of the basin flattened 0.7 toward their mean: the
  palimpsest; rings of troughs 2.5 m deep and 3 m to a side with low lips, the first at 1.3 basin radii, one each
  0.3 out to an angle of 1 radian, wandering 4 %), and her kinds turned into its own. Kept by 0167's **name and
  hash rule** (`mars::file` with the version of the row's bake: `seed-S-planet-N-callisto-<hash>-v301.map`; the
  hash takes every input the bake reads: the stage list, the age and, for an icy body, the bits of the sun's height
  its ice stage reads, which come from the home's and the giant's `sun` and the giant's `size`, so a changed row is a
  new file, tested; `bodies::Planet::icy` carries that height), found by hash, a server's map kept under its row's name (`mars::keep_any` by version).
  Only Terra's and Luna's exact lists keep named files; `bakemap=` still pictures a planet's crust only.
- **Its ground** (`data/biomes.toml` rows 33..=36; top, band to 4 m, below): **dark plain** dark lag / dusty ice /
  ice (the old cratered plains, 0.9 of it); **fresh ice** ice / ice / ice (young craters' floors, walls and rays);
  **palimpsest** dusty ice / dusty ice / ice (the flowed-flat middles of the ringed basins); **polar frost** frost /
  dark lag / ice (where no sun reaches). One **new block**, id **240** "dark lag" (past a gap so a row another
  branch adds at 229 does not meet it; colours and a speckle, no painting): the lag left on top as the ice under it
  went off into space, dust, grit and carbon-rich stuff (a grey-brown 0.38 with a sparse blue-white frost speckle,
  under the dusty ice's 0.78 beige with rust, over the blue ice; read on a dug face and from above): sieved for grit and carbon, sintered into bricks, a little
  water when melted. Reused: **ice** (clean water ice: water in bulk away from Terra, for drinking, air by
  splitting and fuel later), **dusty ice** (dirty ice: water when melted and filtered), **frost** (carbon dioxide:
  the real moon has it). No recipes. No plants, clutter, patches, water, caves or ore on it (`rock_field` false).
- **Loaded on approach** (0175): `lazy_row` is "past the home's first moon, not gas, with a bake", which covers it;
  nothing special-cases Mars by index or name in that path (`mars::keep_any` and `Body::rock_body`'s shortcuts
  pick by map version). From Terra it is never streamed or held (tested); `/goto callisto` answers "going to
  Callisto: its ground is loading" and stands the player on it in its frame (tested, saved and loaded again).
- **The sky from it:** nothing new. The giant is **14.4 degrees** across from its near side (28 times Luna's disc
  from Terra), drawn by its shader (lit by the sun, banded), fixed in the sky by the lock; from the far side it is
  under the horizon. Its eclipses of the sun darken the moon's ground through the shaders' per-body eclipse mask
  (`eclipse_masks`, `light`: every body's sphere over the sun's disc): filmed at the sub-giant point when the sun is behind the giant, the
  ground's mean brightness 39 out of 255 a little before and 1.3 at it; the CPU's `Sky::eclipse`
  (the sun's colour in the air, the HUD) and the tides are the home's moon's only.
- **From Terra it is not seen apart from the giant**, and should not be: at opposition (399 Mm) its 374 km orbit is
  0.94 mrad, 0.55 of a pixel at 1280 x 720 (70 degrees high), so its point falls in the giant's; its light is
  (1061 / 46700)^2 = 5e-4 of the giant's, 0.05 of its point's brightness (`point_light`'s 0.4th power). A zoom of
  two or more would part them; the giant's point would still outshine it twentyfold.
- **The stand runs:** `run=stand body=N eye=` far out (past a small moon's sphere) put the eye in its body's grid but
  outside its frame, so it was read as the inertial frame's, 10^10 m off, and the sky showed stars only (since
  be2b1df9, stand on another body; Callisto's 4.9 km sphere showed it): the eye now goes into the frame it lies in
  (`bench::pose`, tested).
- **`Body::rock_body()` is 0 for an icy map:** harmless only while an icy body has no rock field (`rock_field`
  false, nothing asks); whoever adds icy rock provinces must make it name the icy row.
- **Report:** `report=1` has a `callisto` section (kinds' shares, heights, made_hash); the globe overlay's colours
  know its kinds (`globe::ICY`), though the overlay still shows Terra and Luna only.
- **Locks:** its map seed 1 n 384 32ac890dc737b146, n 1536 7984d03437d95554; its blocks at generation 7, n 384
  (`callisto_generates_as_locked`) 4b42aadc955da137; every older hash unchanged.
- **Costs:** its bake 1.9-2.5 s once per seed; on it frame p99 2.4-2.9 ms; on Terra none seen against main (stand
  and sim, two rounds); see `docs/perf-debt.md` ("The giant's icy moon").
- **Left** (each its own run): a subsurface ocean and an ice-shell interior (an `ocean` zone in `Interior`, its
  depth from the heat; needs `heat` to mean tidal heating and a liquid layer that is water, not metal); ice
  tectonics (Europa's or Ganymede's grooved terrain: a stage of its own); rock under the ice (rock provinces with
  `body = "Callisto"`, so the column changes under 4 m), its ore (by those provinces) and caves (ice caves and
  collapse pits); more moons and a volcanic one (Io: `volcanism` on a hot icy or rocky moon, sulphur); the giant's
  radiation belt (from its field and spin, a dose by distance); real orbits (the circle and the Kepler rounding are
  stand-ins; the circle's `1/YEAR` term is the home's year for every moon); the giant lit faintly by the moon and the
  moon by giantshine; the giant's disc popping off at the limb where its centre sets under a far-side horizon.

### Mars turns [draft, 2026-11-23]
A planet round the sun that is not home turns on its pose once its row's `day` (F9 2026-11-21 "mars does not
rotate": 0163's ring never turned, so the sun crossed Mars's sky once in 1.84 Terra years). `Sky::round_sun` takes the
day in seconds (`Body::day`, `sky::period` of the row's, a whole fraction of PERIOD) and turns on the ecliptic's pole, no
tilt. Mars's row has `day = 1.0275` (the real sol, 24.66 h): 420 turns a PERIOD, 1851 s. No generation number: nothing
generated reads it; a save's positions on Mars are in its frame, so a player stays on the same ground. Its spin would
give Mars a field of 0.136 by 0162's rule (it was 0 only for want of a spin), so its row's `heat` goes from 0.15 to 0
(the core all solid, as the real one's dynamo died): field 0 as 0173 states; its pull and mass are unchanged (the inner
and outer core share one density) and no bake reads `heat`.
Shortcut: the giant's pose stays still, as its bands take their own angle in the client (`frame.rs` `far_lights`);
the client should read the pose's angle instead, then the giant turns here too.
Replaces: 0173's "it does not spin (its day is its year)" and 0174's "none: it does not turn, as Mars".
Open: Mars's axial tilt and seasons.

## The sky's clock

- **World time** is `tick / 20 Hz` seconds since the world was made; the sky is kinematic, circular orbits on rails,
  pure functions of the world's time, the same on every machine. [0022] `DAY` = 1800 s, `MONTH` = 16 days, `YEAR` =
  54 days, `PERIOD` = 432 days (it needs the year to divide it, so these are constants, not run-time parameters);
  Luna's orbit `INCLINATION` = 2 degrees. [0041, 0055]
- **Luna's body is locked**: her body -x faces Terra. [0037]

## Axial tilt and seasons

- **The planet's spin axis is leaned and fixed in the inertial frame**: one fixed rotation in front of the spin in
  `Sky::planet`. No body moves, so every eclipse is exactly as before at any season. The frames' velocities are
  numeric differences of the poses (`World::frame_vel`). `TILT` is 25 degrees (`Sky::tilt`, set by `Sky::new`). [0055,
  0060]
- **World time 0 is the northern spring equinox**; a quarter year on, the northern summer solstice (declination
  +tilt), three quarters the winter one. `tilt == 0` is the old sky bit for bit. [0055]
- API: `Sky::sun_dir`, `declination`, `hour_angle(t, up)`, `day_length(lat, decl)` (seconds the sun's centre is up).
  `/time set spring|summer|autumn|winter` (`next_season`) goes on whole days to the day nearest that point of the
  year, so the clock's hour stays. [0055]
- **`temperature(mean, swing, lat, decl, hour)`**: the place's yearly mean plus `SEASON_K` (40 K) x sin(latitude) x
  sin(declination) plus half the day's swing on a cosine warmest 2 hours after local noon. F3 shows it under "Air" at
  the feet, with the year's mean and the biome. [0055] Where the server sends the live air, F3 shows that; otherwise
  the map's mean with these terms. [0060]
- **The bake's air runs a real year with the sky's sun** and the sky's tilt (`Sky::sun_dir`, 25 degrees), at the same
  10 s step as the live grid (see sky-weather-and-water.md). [0060, amending 0055's tilt-free bake]
- The spawn's clock is mean time: the true noon wanders a few game minutes either way over the year; sunrise is 6:00
  only at the equinoxes and on the equator. [0055]

## Eclipses and Luna's look

- **Eclipses follow from the geometry**; with 2 degrees of inclination, over 10 years there are 19 solar eclipses of 34
  new moons (11 total somewhere) and 14 lunar; over 8 years nine blood moons of about two minutes (Luna crosses
  Terra's shadow at about 80 m/s). The eclipse finder steps 10 s. [0041]
- **Blood moons are rare**; a moon is red only in an eclipse. [0037, 0041]
- **The shader lights a body other than the camera's with white sunlight** (`sun_on`; the camera's body index rides in
  `sun_col.w`), because `sun_col` is the sun as seen through the air at the camera, a deep red at night. In an
  eclipse what a body with air hides of the sun comes back as `COPPER` (0.23, 0.07, 0.02) along the sunlight's own
  ray (so hills still shade it): the whole moon glows evenly copper red, about a third as bright as the sun's red
  channel. [0037] (also rendering-and-lighting.md)
- **A lamp's shadow ray aims at the point of the lamp's block nearest the surface** (a block lamp is an area light):
  `lamp_target` (shader.wgsl) works in the grid's own coordinates (the face's two angles and the log of the radius),
  clamps the ray's origin into the lamp's cell plus 2 cm, and the ray stops 1 cm short of it. [0037] (also
  rendering-and-lighting.md)
- **Night glow:** `NIGHT_GLOW` in `sky_colour`, in air at night a faint blue brighter towards the horizon (also in the
  fog's horizon colour). [0041] (also sky-weather-and-water.md)

## Tides

A sea level that follows Luna, a pure function of place and time. Nothing stored, nothing sent; no save or wire
change. [0067]

- **The function** (`sky.rs`, `Sky::tide`, `Tide::at`): `tide(up, t) = 1.5 (A_L (up . luna)^2 + A_S (up . sun)^2) +
  base`, Luna's and the sun's directions in Terra's body frame; `TIDE_LUNA` A_L = 0.5 m, `TIDE_SUN` A_S = A_L / 3, so
  low to high is 1 m at springs, half at neaps. `base` = -1.5 (A_L l^T M l + A_S s^T M s), M the mean of d d^T over
  `Body::ocean` (4096 even directions whose ground is under the sea, at load): the tide's mean over the ocean is 0
  (to 1e-9 m).
- **The water is not created or destroyed**: the bulge under Luna is the water of the low ring 90 degrees off it;
  stored water trades with the open sea only at the stored edge, booked to `sea_extra`; a column forgotten books what
  it holds against what generation gave it (`gen`), so `counted()` stays exact.
- **Two highs a lunar day** (1920 s), 16 minutes apart; Luna's orbit leans 2 degrees, so the two daily highs differ
  mostly by latitude and the sun's season.
- **Asked at:** generation (`Body::water_tide`, through `world::Solid`): the sea's level in a column is the mean plus
  the tide there; the tidal band (columns whose ground's top is the layer over the mean sea's) goes wet and dry with
  no storage; a river's last columns are the sea's while it stands over them (estuaries at high water). Physics
  (`World::water_surface`): swimming and the underwater test follow.
- **Taken once a second:** `Solid::at` takes the tide at the whole second of world time; the water's caches hold for
  that second; `Liquids::step` sees the second change (`Ground::tide_epoch`) and runs `retide`: caches dropped,
  stored water beside the open sea whose level moved a dead band (1/64 block) or more is woken. A loaded save starts
  at its own second (`restore`); the server sets the save's clock before restoring.
- **The sea's quads are meshed once, for the highest tide** (`sky::TIDE_HIGH`, 1 m over the mean): at level 0 the cell
  over the mean's, so the tidal band has water quads; coarser chunks keep the mean's cell and its solid test for the
  shallows. The shader (`vs_sea`) lays each vertex at the mean plus the tide from Luna's and the sun's directions in
  `Globals::tide`; where the ground stands higher it hides the water. **The tide never remeshes the sea.**
- **Waterline rule:** the mean sea lies 1/8 under a layer's top; a moving sea crosses the tops; the shader keeps the
  level 1/64 layer off any top and steps over it (3 cm, at once). The client re-meshes its stored-water mesh's wet
  tiles' unstored sea columns every 4 s of world time, one tile a frame.
- **Shortcuts:** under `/timewarp` `retide` runs every tick, O(stored columns); add a minimum interval if a warped
  server with large floods shows it. A film under the dead band can stay on a beach node above a falling sea.
- **Open for the developer:** a wet-sand look for the tidal band; plants are placed by the mean sea, so grass on the
  band stands in water at high tide.
- A tide mill needs no rule: a pond with a gate fills at high water and drains through a wheel at low.

## The magnetic field

- **Where it lives:** `World::field(b)` (`world/frames.rs`) returns `bodies[b].interior.field` (Terra 1.0, Luna
  0.0119). Every reader asks it (the renderer, F3, the run's JSON line); none reads `Interior::field` directly. It is
  the one place core mining lowers it. [0164] (the number's derivation: deep-world.md)
- **Core mining must add** (not built): a counted amount of liquid metal per body, server-owned, starting at
  `Interior::liquid` (m3); `field` = interior.field x (liquid left / interior.liquid); in the save (a save version up;
  old saves read it as full), in Welcome and a delta message, with a ledger as the ore's (ADR 0063). [0164]
- **The oval rule** (`sky::aurora(field)`, pure): sin2(colatitude) = sin2(20 degrees) / field, at most the equator;
  latitude 70 degrees at field 1, 67.5 at 0.8, 57 at 0.4, 51 at 0.3, 40 at 0.2, the equator under 0.117. **Brightness
  = sqrt((field - 0.02) / 0.98)**: 1 at 1, 0.63 at 0.4, 0.43 at 0.2, none at and under 0.02 (so none on Luna). A
  stronger field draws a smaller, brighter ring. The magnetic poles are the spin poles (both hemispheres). [0164]
- **The look** (`post.wgsl` `aurora`, drawn in the clouds' pass, behind the clouds, in front of the sky and of a
  planet seen from orbit): curtains in a shell 450 to 1150 m over the sea, in a band 4 degrees either side of the
  oval; marched only where the ray is in the shell and the band, up to 48 steps a piece, jittered per pixel; two
  curtains (the second poleward and dimmer); rays of 6 m with their own top, lower edge and beat (52 to 120 s);
  green low, red-violet at the top; every motion is whole turns an hour of the world clock, so every client sees the
  same. It is light only (the lighting stays sun-only). [0164] (also rendering-and-lighting.md)
- **Day and night:** faded out by the daylight in the air at the camera (`day x air` from 0.02 to 0.27, a smoothstep:
  gone with the sun above about 3 degrees under the horizon, whole by about 9 degrees) and per point by the sun's
  height there. **Under the sea level** none is drawn (the G-buffer holds no water). **With the clouds off** it still
  draws. [0164]
- **Not in view, not drawn** (`frame::aurora_seen`): each oval's band is cut into 48 pieces, bounded by a sphere; a
  piece counts only if over the sea's horizon, where its sun is down (`au_glow` 0.05) and in the frustum; none counts:
  the brightness sent is 0 and the pass skips the march. Conservative by construction. [0164]
- **`field=x`** (the game) scales the home's field for this run: held to 0..10; anything not a finite number reads as
  1 (`frame::field_scale`, the one reader). The scaled value is in F3 (World, Field: the value, the oval's latitude
  and brightness) and the run's JSON line (`field`). [0164]
- **Left for later:** the four costs of a weak field in `deep-world.md` (compasses wander under 75 %, sun storms
  under 50 %, air and water leak under 25 %, field-driven machines); tilted magnetic poles and an oval skewed to the
  night side; solar storms; the aurora of a body other than the home (only body 0 is drawn); its sound. [0164]
- **Cost:** GPU p50 +1.9 to 2.0 ms where the oval fills half the screen at night (`docs/perf-debt.md`); none by day
  or at `field=0`. [0164]

## No longer holds

- 0174's "the body limit stays four" (the body id in two bits of every quad, particle and model word): raised to 16
  by "Sixteen bodies, loaded on approach".

- 0026's design in `docs/design/worldgen.md` as a proposal: it was built (the bake, 0043 on).
- 0037 B's blood moons from the 40-degree tilt of Luna's orbit (16 of 432 midnights, 3.7 %, one eclipse about
  every 39 days, kept on the developer's word then): replaced by the 2 degrees of 0041.
- 0037's "Luna's gravity a fifth of Terra's, 'luna gravity at 1/5 is good'": replaced by 0041 (3.7 times less).
- 0055's `TILT` = 20 degrees: raised to 25 by 0060. 0055's "the bake's air stays without seasons" and its to-do (the
  map keeps `temp` and `swing`, F3 reads those instead of `BIOME_CLIMATE`): done by 0060 (map version 3).
- 0022's "no tilt" shortcut: ended by 0055. 0067's design figure of 10.6 minutes between highs: predates 0041's month
and day (16 minutes now).
- 0163's old two-body wiring (`planet_n, moon_n`, `World::pose` matching 0 and 1, `Save`/`Welcome`/`Ev::Start` with
  named bodies): replaced by the list formats above; the old forms are read only (`Save21`, `Start21`, `Loaded21`).
- 0164's "proposed": the field and the aurora are built; core mining's amount is not.
- 0173's "Mars does not spin (its day is its year)" and 0174's "as Mars" for a row without `day`: Mars has a day and
  a sun-circler's pose turns by it (draft "Mars turns").
