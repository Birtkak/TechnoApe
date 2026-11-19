# NNNN – Any number of bodies, each posed on its parent

- **Status:** accepted
- **Date:** 2026-11-19

## Context
`docs/design/planet-model.md`, build order step 2. The world was exactly two bodies wired by index: body 0
Terra, body 1 Luna. `World::with_grid` asserted two rows and took `[planet_n, moon_n][i]`; `World::pose`
matched 0 to the planet and 1 to the moon; the save (`planet_n, moon_n, map, luna`, version 21), the welcome
(`planet_n, moon_n, map, luna`) and the recording's `Ev::Start { planet_n, moon_n, map }` named the two. The
Mars-like planet (step 5) and a gas giant's moons need more rows, and every old world must keep generating and
replaying bit for bit (the locked `made_hash`es, the bit lock on Terra's pull).

The developer's calls for this step (the brief): a row names its parent; what an index decided becomes a row
value; the formats carry a list; a body is posed on its parent's pose; the GPU's four bodies stay as a stated
limit; old worlds keep their ground to the bit. No new body in `data/bodies.toml`.

## Options considered
1. **A list in the formats, rows with a parent (chosen).** Small: each format gains one `Vec<(columns, map
   hash)>` in row order, the pose recurses down the rows.
2. **A general orbit per row now** (eccentricity, inclination, period from the masses) – planet-model.md's
   later step; rejected here, it would reopen Luna's month and the eclipses before a ship needs it.
3. **Keep `planet_n`/`moon_n` and add the extra bodies beside them** – no format break, but two ways to say a
   body's size forever; rejected.

## Decision
**Row keys** (`data/bodies.toml`, `bodies.rs`):
- `parent`: `"sun"` or the name of a row **above** it (so a pose is worked out down the rows, never round a loop).
- `home = true`: the home world, whose are the spawn, the ore and its ledger, the weather, the tide and the herds.
  Exactly one row, and it must be the **first** (checked): every frame index 0 stays the home's, so the home
  world's systems keep `bodies[0]` and nothing else changes for them. It circles the sun and has a moon (its
  first moon, `bodies::moon`, sizes the sun and is the one the sky's eclipses, tides and moonlight are of).
- `bake`: `"terra"` (Terra's map bake, ADR 0043) or `"luna"` (Luna's, ADR 0070); none: the seed's noise alone.
  `Bake::get`, `for_hash`, `file` are the two pipelines behind one name.
- `orbit` is now in the **parent's** radii (Luna's parent is Terra, so her 60 is unchanged). A body that circles
  the sun and is not home has `size` and `sun` (its distance in its own radii) and no `orbit`.
Terra: `parent = "sun"`, `home = true`, `bake = "terra"`; Luna: `parent = "Terra"`, `bake = "luna"`.

**World.** `World::of(rows, [(columns, map)], seed, warp, gen)` builds any rows; `with_grid(planet_n, moon_n,
..)` and the constructors above it stay (tests, the command line), giving the home `planet_n`, its first moon
`moon_n` and any other body its row's `size` (`bodies::sizes`). Body i's grid lies at `i * MOON_AT` in the
shared coordinates (Luna's place as before). `World::moon` is the sky's moon; the client's moonlight index,
the HUD and `bench` `look=<body>` use it or the row's name instead of 1.

**Poses by parent** (`World::pose`, `sky.rs`). The inertial frame stays centred on the **sun**, as before
(`Sky::sun` is the origin, the home circles it, `Sky::round_sun` places other bodies round the origin): that is
what keeps Terra's and Luna's poses to the bit. The brief's "centred on the home body" holds for the shared
coordinates only, where the home's grid is at the origin. The home is posed by the sky (`Sky::planet`, unchanged); a moon on
`Sky::circle` around its parent's pose now, at `orbit` parent radii (never under `orbit_min`), its face to the
parent, once a month by Kepler's law against the home's first moon for a parent as dense (`(q / q_luna)^1.5`,
q the orbit in parent radii), rounded to a whole fraction of the sky's PERIOD (`sky::period`) so the sky
still repeats. `Sky::moon` is now `circle(planet, moon_orbit, MONTH)`, and Luna's period comes out MONTH
exactly (PERIOD / 27), so Terra and Luna pose to the bit as before (`terra_and_luna_pose_as_the_sky_does`).
**A body that circles the sun and is not home** (`Sky::round_sun`): the simplest deterministic pose, a circle
in the ecliptic at its distance, starting where the home starts, once a year by Kepler's law against the
home's (also rounded to a fraction of PERIOD), **not turning** (identity rotation, spin 0). To be replaced by
real orbits. Its sphere of influence is the home's; a moon's a quarter of its orbit (Luna's as before). Its own
fall round the sun is not subtracted from gravity (nothing pulls there), as the home's.

**Formats.**
- Save **version 22**: `Save { bodies: Vec<(i32, Option<u64>)>, .. }` in place of `planet_n, moon_n, map,
  luna`. A 21 or older is read through `Save21` (the old generic layout, every older reader kept) and becomes
  `[(planet_n, map), (moon_n, luna)]`; it keeps its version until written again.
- Welcome: `ToClient::Welcome { id, seed, bodies, blocks, items, gen }`; the client, the bot and the title
  screen's download fetch every body's map by its row's bake. There is no wire version: the client and the
  server are one build (the friends' client updates itself). A client given another number of bodies than its
  rows refuses with "the server has N bodies, this game M: update the game" (`bodies::known_bodies`, as
  `known_generation` refuses). No protocol number was added: an old client could not read it. An old client
  meeting a new server reads the new welcome in the old layout and stops on whatever it misreads, most
  likely the blocks hash ("the server's build has other blocks") or "no welcome from the server"; not tried.
- Recordings: `Ev::Start { bodies, seed, dt }` and `Ev::Loaded(Save)` are new variants at the end; the old
  ones are kept in their places as `Start21 { planet_n, moon_n, seed, dt, map }` and `Loaded21(Save21)`, so an
  old recording reads and replays as before (a `Start21`'s Luna is this build's bake whenever Terra is baked,
  as it always was).
- `planet=` and `moon=` are unchanged (the home's and its first moon's columns); no new argument.

**GPU limit.** `engine::bodies::MAX` is 4 (a world of more is refused by `World::of`, and `World::gravity`
keeps its centres in an array of that size); `gpu::MAX_BODIES` is that constant and stays 4 (the shaders mask a body id with `& 3u`). The client refuses at start
with a message when `data/bodies.toml` has more rows (`gpu::bodies_fit`), and again on a world of more. The
sky shader's "nearest body" looks at all four slots instead of bodies 0 and 1 (an unused slot has radius 0).

## Consequences
- A body is a row: a test builds a world of four (home, moon, the moon's moon, one round the sun).
- Old worlds: `made_hash` for seed 1 planet 6144 unchanged (`mantle=0`, `gen=1`, `gen=2`, default), the
  `record_tests` bit lock untouched and green; a version-21 save and two old recordings (`crates/server/tests/
  fixtures`, written by the build before) load and replay to the same world hashes.
- Left for later: real orbits (planet-model.md "Orbits are real"), so the sun-parent pose and the Kepler
  rounding are stand-ins; the home must stay the first row (lifting it means every `bodies[0]` of the home's
  systems: ore, ledger, weather, tide, herds, spawn, the client's map and globe); eclipses, tides and moonlight
  count only the home's first moon; the globe overlay and the waypoint colours know Terra and Luna by name;
  `Luna`'s bake reads its own row by `bake = "luna"`; a world whose save lists another number of bodies than the
  rows is refused (`Server::on_map`), which the Mars step must turn into "the new rows at their defaults".
- A fifth body needs the GPU's body slots and the id bits in the quads widened first.

## Checks
| Check | Result |
|---|---|
| `world::tests::a_world_of_four_rows_builds_and_poses_each_body_on_its_parent` | red with the old two-body `pose` (body 2's gravity off: posed at the sun), green |
| `world::tests::terra_and_luna_pose_as_the_sky_does` | green before and after (the bit lock on the old poses) |
| `bodies::tests::bad_rows_are_refused` (parent, home, bake, a sun-parent row) | written with the keys, green |
| `server::tests::a_version_21_save_and_recordings_load_as_terra_and_luna` | red with the 21 reader cut off ("only version 22"), green |
| `world::tests::the_homes_moon_may_come_last` (Terra, Mars round the sun, Phobos, Luna) | red (index out of bounds in the Kepler ratio), green |
| `bodies::tests::the_data_file_has_terra_and_luna` (`known_bodies`) | red with the check stubbed, green |
| `v21-moving.rec` (main's build: a player walks, jumps, goes to Luna and falls; 25 checks) | replays to the same hashes; red with a moon's orbit off by 1e-3, still green with Luna's month off by 1e-4 (25 s of world time is too short to show it) |
| `save::tests::a_save_of_three_bodies_reads_back` | green |
| `net::tests::tcp_round_trip` (a welcome of three bodies) | red as a compile error on the old `Welcome`, green |
| `client::gpu::more_bodies_than_the_gpu_holds_are_refused` | red with the check stubbed to Ok, green |
| `report=1 seed=1 planet=6144` made_hash, mantle=0 / gen=1 / gen=2 / default | 0926f028ddcd85a7, 37b2d1b1006b66f7, a2ef250b3ed4203a, bfb878ce14d32b75: unchanged |
| `tools/views.sh` (play build) | every view within the limit, no cracks; the views off their baseline (orbit 0.26 %, winter 0.24 %, lod 0.13 %) are pixel for pixel main's build's shots too |
| `run=stand look=luna time=1100` | Luna a small disc at the centre over Terra's night horizon, pixel for pixel main's |
