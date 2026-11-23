# Real gravity: every value it changed

What world generation 8 (`world::REAL`, the "Real gravity" entry, ADR 0181, in `docs/decisions/bodies-and-orbits.md`,
under ADR 0180) changes, with how to undo each one. Generations 7 and older keep the "before" column, bit for bit, so
their saves, maps and recordings replay unchanged. Numbers are for the 4 km planet (`planet=6144`). A *choice* is a
value someone set and can set again. A *consequence* follows from the choices and the one G, so it changes only
through a choice. "One-G safe" says whether the revert keeps every pull and period derived from the masses.

| Value | Before (gen 7) | After (gen 8) | Why | Kind | To revert | One-G safe |
|---|---|---|---|---|---|---|
| Constant of gravity `k` | 2.2364e-7, from Terra's row (one value, ADR 0180) | the same, `interior::constant` | Terra's row pulls 19.62 m/s^2 at her surface. It goes as 1/radius, so `planet=` scales the system | consequence | Terra's row (`density`, `iron`) | yes |
| Luna's make-up | `iron` 0.7071, `density` [2700, 3300, 7920], `weight_g` 19.62 | `iron` 0.2, `density` [2550, 3310, 7000] (mean 3340 kg/m3) | the real Moon's | choice | Luna's row in `data/bodies.toml` (the old values sit in `[body.old]`) | yes (any make-up is weighed by `k`) |
| `weight_g` as a row value | read from each row | rejected outside `[body.old]` | no pull set by hand (ADR 0180) | choice | bring back the field in `bodies.rs` `Raw` | **no** |
| Luna's surface pull | 5.314 m/s^2 | 3.597 m/s^2 (0.183 of Terra's) | her mass from the Moon's make-up. Terra's mean (4933 kg/m3) is lighter than the Earth's, so not 1/6 | consequence | Luna's row | yes |
| Luna's mass | 0.0199 Terras | 0.0134 Terras | her make-up | consequence | Luna's row | yes |
| Luna's field (rock weighed) | 0.0119 | 0.0001 | her rock is weighed by her own pull | consequence | Luna's row | yes |
| Sun's density | none (implied 53,794 kg/m3 by the 54-day year) | `sun_density` 1408 kg/m3, Terra's row (`bodies::SUN_DENSITY` the default) | the real Sun's | choice | Terra's row `sun_density`. 53,794 gives the 54-day year again | yes |
| Sun's mass | 1.8555e22 kg (implied) | 4.8565e20 kg, 3.071e5 Terras | its density in the radius the sky gives it | consequence | `sun_density` | yes |
| Sun's size in Terra's sky | 0.500 deg | 0.500 deg | sizes and distances untouched | — | — | — |
| Terra's year | 54 days (`sky::YEAR`, by hand) | 333.85 days (600,927 s) | Kepler's law from the sun's mass and Terra's distance | consequence | `sun_density` (53,794 gives 54 days) | yes |
| Season | 13.5 days | 83.46 days | a quarter year | consequence | `sun_density` | yes |
| Day | 1800 s | 1800 s | kept by the brief | choice | `sky::DAY` | yes |
| Luna's period (sidereal) | 28,800 s (by hand) | 42,669 s (23.705 days) | Kepler's from Terra's and Luna's masses | consequence | Luna's or Terra's row | yes |
| Luna's new-moon month (phase cycle) | 16 days (`sky::MONTH`) | 25.52 days | sidereal month against the year | consequence | rows, `sun_density` | yes |
| New moons a year | 3.38 | 13.08 | month against year | consequence | rows, `sun_density` | yes |
| Luna's day | one new-moon month (locked) | one sidereal month to the stars (locked to Terra) | moons are locked to their own parent (`Sky::against` infinite) | consequence | — | yes |
| Mars's year | 108 days (rounded, 8.5 % bent) | 615.38 days | Kepler's | consequence | `sun_density` | yes |
| Mars's day | 1851 s | 1849 s | rounding to the new repeat | consequence | `sky::REPEAT` | yes |
| The giant's year | 432 days (clamped to the repeat, 0.72 of Kepler's) | 3724.14 days | Kepler's | consequence | `sun_density` | yes |
| The giant's field | 3.7101 | 3.7675 | its own pull with the one `k` | consequence | the giant's row | yes |
| Callisto's period | 2945 s (1.636 days, scaled from Luna's month, against the year) | 4400 s (2.445 days) | Kepler's about the giant, locked to it | consequence | Callisto's or the giant's row | yes |
| Sky's repeat | `sky::PERIOD` 432 days | `sky::REPEAT` 432,000 days | every period a whole fraction of it, within 0.5 % of Kepler's (worst is Jupiter's, 0.16 %; the brief allows 1 %) | choice | `sky::REPEAT` | yes |
| Solar eclipses | 1.88 a year, 0.035 a game day | 6.83 a year, 0.020 a game day | month and year | consequence | rows, `sun_density` | yes |
| Lunar eclipses | 1.38 a year, 0.026 a game day | 5.42 a year, 0.016 a game day | month and year | consequence | rows, `sun_density` | yes |
| Tide range from Luna | 0.75 m (`sky::TIDE_LUNA`, by hand) | 0.396 mm | `sky::tide_rise`: h = (M_k/M)(R/d)^3 R, range 1.5 h | consequence | Luna's row (or a `TIDE_*` constant, which breaks one G) | yes |
| Tide range from the sun | 0.25 m (`sky::TIDE_SUN`) | 0.151 mm | the same | consequence | `sun_density` | yes |
| Tide band `tide_high` (sea quads, `tide_top`) | 1.0 m (`sky::TIDE_HIGH`) | 0.547 mm (`Body::tide_high`) | the derived high at springs | consequence | follows the two above | yes |
| Season, plants (`season_at`, `next_season`, `clutter`), live air, `/time` | over 54 days | over the world's `Sky::year` | the year is derived | consequence | `sun_density` | yes |
| Terra's bake | version 7, its climate over 54 days | version 8, over 333.85 days. Old worlds keep v7 (`bake::get_at`, `Map::bake_v7`) | the climate follows the year | consequence | `bake::VERSION` back to 7 and `bake_with`'s year to `YEAR` | **no** (the climate's year would be the hand-set one) |
| Bake time, climate stage | about 16 s | about 6 times as long (334 days of air instead of 54); 3.1 s to 4.1 s for the whole bake at planet 256 | more days of air | consequence | `sun_density` | yes |
| Water's pull (`liquid.rs`: link speed-up, `fall_cap`, `drag_for`, `steady_speed`) | 9.81 m/s^2 on every body (`liquid::EARTH_G`) | the body's own surface pull (`Ground::g`, `Solid::g`) | water by its body's gravity (liquids.md step 1) | consequence | `Solid::g` returns `EARTH_G` | **no** |
| Water's dead band (`moves`, `CG`) | a 1.6 mm head | the same 1.6 mm head on every body | kept a head, so a lake still lies flat | choice | `CG` | yes |
| Dam break, 4 m deep, front 10 m out | — | 3.45 times slower under 1.85 m/s^2 than under 19.62 (Ritter's 3.26). 3.97 over 20 m, where the fixed friction per step costs the slower front more | water by its pull | consequence | — | — |
| Look table's stagger (client `gpu.rs`) | days of the 54-day year | unchanged: days of `sky::YEAR` (a `shortcut:` there) | the table is built before the world | not changed | — | — |
| Sizes and distances | — | unchanged | the spike found them at real ratios, with the Roche limits inside the orbits | not changed | — | — |
| `phys::pull` / `World::gravity` | every body's interior pull with the one `k`, square law outside, bodies on rails | unchanged | already one G. Luna's 3.60 is above `FREE_G` 2.0, so she is walked, not floated | not changed | — | — |
| `world::GENERATION` (new worlds) | 7 | 8 (`world::REAL`) | the developer, 2026-11-23: make everything real, then pick what to revert | choice | set it back to 7: new worlds are as before, all of the above stays in the code unused | yes |
