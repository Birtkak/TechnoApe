# 0076 – The weather made visible: clouds, rain, wind, storms, snow

- **Status:** accepted (built overnight 2026-11-12/13; the calls below are the developer's to confirm)
- **Date:** 2026-11-13

## Context
The live air grid (ADR 0060, `docs/design/air.md`) runs on the server and F3 shows its cloud, rain and wind
where the player stands; nothing of it was drawn or heard. Roadmap item 3 asks for clouds seen the same from
the ground, from above and from orbit, rain that stops under a roof, wind you see and hear, storms with
lightning, and snow falling and lying (`seasonal-looks.md` left ground snow to the weather). Two clients must
see the same sky, nothing may stall the frame thread, and the game will be CPU-bound later.

## Options considered
1. **The client runs its own air grid** from the map's climate – no traffic, but it drifts from the server's
   (saves, `/time`, forced weather) and costs a core's share on every client. Rejected.
2. **The server sends the grid's columns as bytes** (chosen) – 13,824 columns of 6 bytes on Terra, whole on
   join, then on each 10 s landing only those that moved more than a byte's step. Everything else follows
   from those bytes, the seed and the world's clock.
3. Clouds as **meshed boxes** in the chunks – real geometry that shadows through the ray tracer, but a
   planet's worth of boxes to mesh and stream and rebuild every 10 s. Rejected for a ray-marched slab.
4. Ground snow as **blocks** placed per column – edits on every column of a winter, saves that grow, melting
   that must remove them. Rejected: snow is a look over the ground, kept by the air grid, which already
   keeps snow per column (`Air::snow`, water units that fall below freezing and melt when warm).

## Decision
- **Wire** (`engine::weather`): `Cell = [u8; 6]` per column: cover, the root of rain over 40 mm/h, wind east
  and north in quarter m/s, lying snow over 10 mm of water, temperature + 128. `ToClient::Sky { t, all,
  changed }`. The lying snow comes from the grid's own `snow` at each landing (not in the saved `View`, so
  saves keep their layout).
- **`/weather clear|cloudy|rain|storm|snow|auto`** (and `weather=` on any run) forces one weather over the
  planet in what is *sent* only: the grid, the rivers' runoff, the ledgers and the soak's hash are untouched.
- **Clouds** (`clouds.wgsl`): a slab 300 to 364 m over the sea, cells of an eighth of an air column (16 m on
  Terra). A cell is filled where the column's cover (bilinear, sampled at the cell's middle) beats its own
  clumped number; its top rises with the margin, its edge is soft where the margin is thin. The cells slide
  with the column's wind in two phases half of a 240 s period apart, so the pattern never shears; the client
  blends the last two landings over the 10 s step so the cover moves instead of jumping. Marched after the
  water (premultiplied) in up to 12 (low) or 24 (high) steps a pixel; lit by Sol by the sun's height where the
  cell is (the night side's are dark from orbit), grey to slate with the rain they hold, fading into the air
  as far ground does. Their shadow: one sample where the sun ray crosses the slab's middle, in `light()`, so
  the ground, the bounce rays' hits and particles all get it.
- **Rain and snow** (`client::weather::Fall`): drops of the effects pool started over the camera, upwind by
  their fall, landing on the floor of the column they are over (the pool's own lookup, re-done as they cross
  columns), so a roof or a cave's ceiling stops them; a small splash where they land, a ripple on water.
  Snow below 0 C at the camera: slow flakes with drag toward the wind. The sky greys with cover and darkens
  with rain. Sound: `rain` through the open sky, `rain.in` (low-passed) for the share of nearby drops that
  land above the head (a roof).
- **Wind:** the effects pool's drag pulls a planet particle toward the wind where the camera is, not to rest
  (leaves, smoke, snow drift); rain is started with the wind's velocity (it leans); the wind's sound rises
  with its speed.
- **Cover:** the grid's `View::cover` was `per_m2 / (per_m2 + cover_half)`, at most 0.01 on Terra while half of
  it rains: `cover_half` (120 units/m²) is about 200 times what a column holds before it rains. The view now
  takes `Air::cloud_share` (the cloud over what its layers hold at `cloud_max`) through a smooth step from
  0.2 to 1.2. F3's cloud reads the same. The sun pass's own shade still uses `cover_half` (so clouds barely
  shade the bake's or the live grid's ground): changing it changes every bake, left for the developer.
- **Storms:** rain over 8 mm/h. Each stormy column within 6 of the camera's strikes in a seeded share of 2 s
  slots (`strikes`: seed, column, slot), so every client sees the same strikes from the bytes alone; a strike
  within 2.5 km flashes (two flickers and a fade that lights every lit pixel by its albedo, weaker far off)
  and its `thunder` plays from the strike, late by distance at 343 m/s.
- **Lying snow:** the terrain shader whitens faces that look up by the column's snow (bilinear over the
  columns), the face's pattern showing through where it is thin. Nothing in the world changes.
- **Settings:** Graphics "Clouds" (off, low, high) and "Rain and snow" (off, light, full).

## Consequences
- A client's weather traffic is the join's 83 kB and then a delta every 10 s.
- The clouds' cost is a full-screen pass that most ground pixels leave after one sphere test; the sky and the
  view from orbit pay up to the steps' count. The frame thread's new work: a 166 kB texture blend at about 6 Hz
  and the drops' spawning (a few block lookups each).
- Clouds cast shadows only on the sun's light, not on the moon's; they are not in the traced scene, so a
  reflection does not show them.
- Rain is not water in the world (that stays the grid's runoff to the rivers, ADR 0059).

## Calls for the developer to confirm
1. Cloud height (300 m over the sea) and thickness (64 m), and the 16 m cell.
2. How much of the sky the grid's cover fills (`cl_number`'s spread: tuned to the live grid's cover so a
   mean sky has broken cloud).
3. Rain density (350 / 1,100 drops a second at 4 mm/h for light and full) and the sounds' levels.
4. Lightning's rate (0.06 % of 2 s slots per stormy column, up to three times that: a strike every few seconds within 2.5 km under a storm from horizon to horizon) and flash strength.
5. Snow fully white from 10 mm of water (about 10 cm of snow).

## Checks
- Tests: a column's bytes round-trip and a force leaves the grid (`engine::weather`); the map blends from the
  last step, winds through calm; lightning only in storms, the seed's own, once each (`client::weather`).
- Filmstrips: ground, 200 m and orbit, clear, cloudy, rain, storm, snow, day and night (`out/weather-*.png`).
- The fast gate against `tests/perf/baseline.json`.
