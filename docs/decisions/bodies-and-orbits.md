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
