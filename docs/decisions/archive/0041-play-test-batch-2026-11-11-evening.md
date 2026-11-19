# 0041 – The evening play-test batch of 2026-11-11

- **Status:** accepted (the gate is run by the overseer after the merge)
- **Date:** 2026-11-11

Seven F9 reports, `bugs/2026-11-11_17-37-12` to `17-48-16`. The earlier 12:xx and 13:xx reports were all
answered already (ADR 0032, 0036, 0037, 0038 and their commits).

| Report | What it was | What changed |
|---|---|---|
| 17-38-04 white dots by a blue lamp | Pixel holes of sky where a merged quad's edge meets its neighbours' corners: both are worked out apart at f32's grain at the body's radius (a quarter millimetre at 4 km). Placing a lamp re-meshes its chunk and changes the merges, so more holes. Measured at 2560x1440 in the report's view with the lamp: 82 sky pixels in rows; 13 without the lamp. | The drawn quads grow by 1 mm (`DRAW_GROW`, shader.wgsl `vs`), as the traced copy already grows by 1 cm. After: 1 pixel each, a real gap between far hills. |
| 17-48-16 glitched texture | On block sides facing ±j the texel's u axis is the height, so the grass fringe was hashed per row, not per column: random green rows down the dirt. Found with a temporary per-pixel probe (one grass cell's side read green, dirt, green, dirt). | `texel()` hashes the fringe by the column along the side. The probe now reads green, green, dirt, dirt; the fringe is ragged as on the other sides. |
| 17-38-47 selection box | (1) The mesh puts vertices between the exact points of each 8-cell lattice cell (ADR 0025 I2), up to 2 mm under the sphere; the outline used the sphere, so it stood a few pixels off its block close up. (2) Its lines were spaced for 1080 lines, so at 1440 each line was hollow. (3) The overlay's copy of the uniform struct lacked `ctr`, so every body after the first was misread. | (1) `hud::ghost` places corners as `place` does; test `a_ghost_sits_where_the_mesh_draws_its_block` (red 2.14 mm, now under 0.5). (2) The real frame size rides in `opt.zw`. (3) The overlay's structs match shader.wgsl. RGB: a keyboard wave, saturated, a turn every 3 s flowing along the edges (was 12 s and paler). Taste default. |
| 17-43-39 lamps brighter | Light was 1.5/(1+d²) to 8 m: a seventh at 3 m, nothing past 6. | 3/(1+d²/4) to 12 m (`LAMP_R` 12 in shader.wgsl and gpu.rs, grid list 7³ cells a lamp): 7 times at 3 m, 20 at 6. In the report's save the ground near the lamp went from mean (21,22,14) to (30,34,17) and the wall 10 m off is lit. Taste default. |
| 17-47-11 empty sky | Not a bug. Luna orbits at 2.83 Terra radii, so from any spot it is up 37 % of the time at the equator, 30 % at 39° N (Earth's Moon: about half). Sol and Luna are both down about a third of the time. | Developer: keep the orbit, glow the night. `NIGHT_GLOW` in `sky_colour`: in air, at night, a faint blue brighter towards the horizon. A moonless night went from (2,3,6) to (5,7,15) up high and (9,13,29) low. |
| 17-41-28 stars too fast | Not a bug: the stars are fixed in space. Flying 328 m/s round a 4 km planet turns the whole sky 4.5°/s; standing, 0.3°/s. | Developer: keep it true. No change. |
| 17-37-12 loading screen | Each world is a new process and window (ADR 0038 A). | Developer: a separate task next (one process, one window, a loading screen, its own ADR). No change here. |

Also: a run other than play reads `settings=` when given, so a shot can be taken at another screen size.

Cost: on the GPU a few more lamp-lit cells per pixel (still 3 shadow rays); on the CPU the lamp grid lists a
lamp in up to 343 cells instead of 125 each frame (nothing without lamps). Not yet measured against
`tests/perf/baseline.json`.

Review (a fresh reviewer on the batch's diff): the 1 mm grow also moved the texture coordinates, so textures
slid at block edges; fixed, `o.local` keeps the exact corner. The fog's horizon colour (frame.rs `haze`) lacked
the night glow, so far hills stood out against a glowing horizon; fixed, it adds the same glow. The lamp grid's
longer lists were noted as a cost, above.

## Luna: a fixed part of Terra, and perfect eclipses

The developer (2026-11-11): Luna always 3.7 times smaller than Terra, 30 Terra diameters away centre to
centre, as the Moon to the Earth, so that a total eclipse on Terra is a perfect one; dynamic with the planet's
size; only Terra needs it.

- **Data.** `data/bodies.toml`: Luna has `size = 3.7` and `orbit = 60.0` instead of `n` and `orbit_min`;
  Terra has `sun = 23500.0`, the sun's distance in its radii (1 AU over the Earth's radius).
  `engine::bodies::columns(i, n)` gives a body's columns from the first body's: n / 3.7 to the next 32, so
  1664 under the standard 6144 (ratio 3.692), 416 / 832 / 3328 / 6656 under 1536 / 3072 / 12288 / 24576. The
  client, the server, the new world page (`moon=`) and the system checks all take it; `moon=` still
  overrides it, and a save keeps the `moon_n` it was made with.
- **The sun.** `Sky::new` derives the sun's radius: seen from the ground right under Luna at a new moon, the
  sun is `SUN_UNDER_MOON` (0.95) of Luna's size, as the Moon on a total eclipse is 3 to 6 % bigger than the
  Sun. On the 4 km Terra: Luna 1150 m in radius 255 km out, the sun some 435 km in radius 99.7 Mm out. Test `luna_is_a_fixed_part_of_terra_and_just_covers_the_sun` over the five sizes
  (red first: the old sun was not covered on the 1 km planet; red again at 0.995, whose totality was too
  narrow): the ratio within 1 %, the orbit 60 radii, total 3 % of Luna's radius off the middle of the
  shadow, and a sun Luna's size not total.
- **Where Luna's grid lies.** `MOON_AT` moved from 1,000 km to 10,000 km and the inertial origin to
  -10^10 m, so no body's frame (1.5 x its sphere of influence) reaches another's at any size. Saves are
  version 6: in an older one a player on Luna is moved with its grid, and one in space (whose place round the
  old sun means nothing now) goes to the spawn, keeping what it held (test
  `players_of_a_save_before_the_frames_moved_are_moved_with_them`, red first: the guess by distance moved a
  player of this version too). A recording begun from an older save still replays.
- **Tilt.** With 40° and a 54 day year, totals would come once in about 8 years. Measured by tilt (per year,
  somewhere on Terra): 40° 0.12 solar, 0.12 total; 5.1° 0.62 / 0.38; 3° 1.12 / 0.63; 2° 1.88 / 1.12;
  1° 3.38 / 2.62. The developer chose 2°. The sky tests now count, over 10 years, 19 solar eclipses of
  34 new moons (9 total somewhere) and 14 lunar; over 8 years 9 blood moons, about 75 s each (Luna crosses
  Terra's shadow at some 80 m/s).
- **Gravity.** By ADR 0037's square-root rule the bigger Luna would pull half as hard as Terra. The developer
  reasoned that a 3.7 times smaller Luna gives a 4 to 6 m jump; `surface_g` is now G times the radius over
  Terra's (one density): Luna pulls 3.7 times less, a jump there is 4.66 m (test
  `a_jump_is_a_block_and_a_quarter_on_terra_and_higher_on_luna`, now at the real Luna size). Terra stays G
  on every size, so a jump on Terra is 1.25 m whatever the planet. F10's row says so.
- **F10.** The eclipse finder steps 10 s instead of 60 s, as Luna now moves 80 m/s rather than a few.

Totality: at first the sun was 0.995 of Luna, so the full shadow was some 10 m across and passed in 0.2 s.
The developer chose to widen it: at 0.95 it is some 115 m across, a few seconds at any spot; the partial
phase lasts about a minute.

## A 30 minute day

The developer (2026-11-11): time a bit slower, a 30 minute day. `sky::DAY` is 1800 s (was 1200). The month
(16 days), the year (54 days) and the sky's period (432 days) are counted in days, so they stretch with it:
a month is 8 hours of play, a year 27. A full stomach still lasts a day (ADR 0040), so hunger drains a third
slower in real time. The clock's hours, `/time set` and F10 follow from `DAY`. Red first:
`a_day_is_thirty_minutes_and_starts_in_the_morning`; the chat test's `/time set 10` now reads 09:08. The
fixed views stand at time 0, 9 am either way. vision.md, air.md, progression.md, open-questions.md, the
roadmap and CLAUDE.md say 30 minutes. Over 10 years now: 19 solar eclipses, 11 total somewhere; over 8, nine
blood moons of about two minutes.

Review of the Luna part (a fresh reviewer on the diff):
1. Luna's chunk tree. Its roots come from the power of two in its columns: 1664 is 13 x 128, so 13 x 13 roots a
   face, and under the 1 km Terra (416) level 0 roots that never coarsen. Measured, `run=orbit quick=1`: on the
   4 km planet the frame thread's p99 3.5 ms with the old Luna (`moon=256`) and 3.9 to 4.0 ms with the new,
   draws 41 against 422; on the 1 km planet p99 3.7 ms, 641 draws. Inside the budget; one run in four had a
   24 ms frame that did not come back. Answer: kept, as no column count near 3.7 has a small odd part (1536 is
   4.0, 1792 3.43); a root grid that can coarsen past the power of two is a follow-up.
2. Older saves' players in space were lost: fixed by the save version (above).
3. Totality a fraction of a second: widened, the developer's call (above).
4. An older save keeps its small Luna, so its sun, sized from it, is small too: as asked (a save keeps its
   `moon_n`).
5. The `orbit` run means something else now: noted, the baseline is for the gate to judge.
6. On the 768 planet (`bench.sh base`) Luna is 192 columns, 4.0 times smaller: the rounding to 32 columns.
7. `Sky::new` asserts the moon clears the planet's ground, as a bad row would make the sun's size NaN.
8. Two stale comments: fixed.

Still to do: the fixed views with Luna in them, the benchmark baseline (`orbit` now flies 250 km between the
bodies in its 10 s leg), the gate.
