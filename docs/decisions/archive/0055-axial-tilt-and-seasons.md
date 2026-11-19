# 0055 – Terra's axial tilt, the seasons and the air's temperature in F3

- **Status:** accepted (the tilt and the season's strength are open taste calls, below)
- **Date:** 2026-11-12

## Context
W3's sky side (`docs/plans/worldgen-plan.md`): its bake side is built (ADR 0042, 0043), but the sky had ADR
0022's "no tilt" shortcut: the sun stood over the equator all year and every day was 12 hours everywhere.
`docs/design/worldgen.md` ("Climate, seasons and biomes") proposes a 20 degree tilt over the 54-day year;
seasons are the developer's call (open question 5). The eclipses of ADR 0041 must stay perfect, and the sky
must stay a pure function of the clock, the same on the server, the client and the bots.

## Options considered
1. **Lean the planet's spin axis, fixed in the inertial frame** – one fixed rotation in front of the spin in
   `Sky::planet`. No body moves: the sun, the planet's and Luna's centres and Luna's turn are what they were,
   so every eclipse is exactly as before at any season. The frames' velocities are numeric differences of the
   poses (`World::frame_vel`), so physics and reframing need nothing.
2. **Tilt the ecliptic instead (the orbits about a leaning plane)** – the same sky seen from the ground, but it
   moves Luna's orbit and the eclipse geometry with it; more to re-prove for nothing.
3. **Fake the season in the lighting only** – the sun's place would disagree with the bodies; no.

## Decision
Option 1, in `crates/engine/src/sky.rs`:
- `TILT` (20 degrees, worldgen.md's proposal) is `Sky::tilt`, set by `Sky::new`; the axis leans toward the
  sun's place a quarter year on, so **world time 0 is the northern spring equinox**, a quarter year the
  northern summer solstice (declination +tilt), three quarters the winter one. `tilt == 0` is the old sky
  bit for bit (`tilt_zero_is_the_old_sky`). The year stays `YEAR`, 54 days (worldgen.md; ADR 0041's
  `PERIOD` of 8 years needs it to divide 432 days, so it stays a constant, not a run-time parameter).
- `Sky::sun_dir`, `declination`, `hour_angle(t, up)`; `day_length(lat, decl)` (seconds the sun's centre is up).
- `/time set spring|summer|autumn|winter` (`next_season`): whole days on to the day nearest that point of the
  year, so the clock's hour stays; Tab completes the names.
- `temperature(mean, swing, lat, decl, hour)`: the place's yearly mean, plus `SEASON_K` (40 K) times
  sin(latitude) sin(declination) (some ±10 K at 45 degrees with this tilt, nothing on the equator or at an
  equinox), plus half the day's swing on a cosine warmest 2 hours after local noon. F3's World section shows it
  under "Air" at the feet, with the year's mean and the biome.
- The mean and the swing come from the biome (`BIOME_CLIMATE`): the baked map keeps the biome but not the
  `temp` and `swing` of `bake::life::Life` it was chosen by. Storing those in `bake::Map` is a bake format
  change and `bake/` was out of this change's scope (a parallel session works there): see Consequences.

Checks (`cargo test -p engine --lib sky::`): the noon sun's height by latitude (0, 30, 60, -45) at both
solstices and both equinoxes within 0.2 degrees of 90 - |lat - decl|, and the midnight sun past the polar
circle; the counted day length against `day_length` at five dates and four latitudes; tilt 0 equals the old
rotation exactly; Luna's pose and the planet's centre are identical with and without tilt over the whole
`PERIOD`; the existing eclipse and blood-moon tests unchanged; `next_season` keeps the hour and lands within
half a day; the temperature's season and day terms. `chat_reaches_everyone_and_commands_answer_their_sender`
covers `/time set summer`.

Open taste calls (the developer): **the tilt** (`TILT`, 20 degrees proposed; 23.4 is Earth's), **the year's
length** (54 days of 30 minutes: 27 hours of play, a season about 7 hours), and **the season's strength**
(`SEASON_K`).

Review (a fresh subagent, read-only): nothing serious; the tilt's sign, the hour angle, `day_length`,
`next_season`'s range, the eclipses and the other users of the planet's pose checked by hand. Fixed: a raw
newline in `/time`'s usage text, `next_season` tested at the year's turn and exactly on a season (and its dead
`max` dropped), the Studio's `game_time` comment. Answered: `tilt_zero_is_the_old_sky` passes by its own branch
(the tilted branch's sign is held by the noon-height and polar tests); the bake's tilt-free climate (below); the
day swing still applies in polar night (harmless for F3).

## Consequences
- The fixed viewpoints have no `time=` and stand at world time 0, the equinox: the sun's direction in the body
  frame is exactly as before there (the lean is about the sun's own direction), but Luna's and the stars'
  places in the sky turn by the tilt, so a viewpoint that sees Luna or the stars may differ by pixels. Not run
  here; `tools/views.sh` decides.
- The spawn's clock is mean time: with the tilt the true noon wanders a few game minutes either way over the
  year (the equation of time), and sunrise is 6:00 only at the equinoxes and on the equator.
- The bake's air grid (`bake/air.rs`, its own `SPIN0` copy and "no tilt" sun) stays without seasons: the map's
  climate is a tilt-free yearly mean, a little warmer at the equator and colder at the poles than a tilted
  year's mean would be. Left alone (bake/ was out of scope); worth aligning when the map keeps temperature.
- CPU: `Sky::planet` gains one quaternion product (a few ns; it is called a few times a frame and per tick);
  the F3 reading is one biome lookup and a few trig calls when the overlay draws. Nothing on a hot path.
- To do: the map keeps `temp` and `swing` per cell (W2/W3 bake format change, a version bump), and F3 reads
  those instead of `BIOME_CLIMATE`; a seasonal lag (the warmest day a few days after the solstice); snow lines,
  crops and solar power by season (worldgen.md); W3's live air grid replaces the season and day terms.
