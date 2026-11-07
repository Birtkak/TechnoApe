# ADR 0156: Seasons that matter

Status: accepted (2026-11-07). Runtime only (`scripts/world/season_world.gd`, `SimWorld::season_pass`); no generator
change, GEN_VERSION stays. Materials 145 `snow_layer`, 146 `lake_ice` (reuse atlas tiles 14 snow and 27 ice).

## Context
Fun pass row I: the season shown on the HUD should change play. ADR 0025 (calendar, sky, crops rest in winter) and
ADR 0061 (frost kills crops, wild food by season, root cellars) gave the rules, but a 108-day year (36 h of play) was
never seen turning, a temperate winter (12 +- 7 C) never froze, and snow was only a shader tint.

## Decision
- **A short year:** `Settings.month_days` 3 (was the calendar's 9): a season is 9 days, 3 h at 20-minute days; the year
  still starts on 1 April (`Calendar.with_month_days`). The moon keeps one cycle a month, so it now turns every 3 days.
- **A bigger swing:** `Survival.environment_c` swings `Settings.season_swing_c` 14 C (was 7) about the climate's mean:
  a temperate (climate 0) mid-January means -2 C, July 26 C; nights 4 C under, afternoons 3 C over. Day length was
  already real geometry (ADR 0025).
- **Snow and ice in native code:** `SimWorld.season_pass(chunk_xz, flags, ids, lies_on, seed)` walks one chunk
  column (16 x 16 voxel columns) top down to the first non-air cell: flag 1 lays one `snow_layer` cell on a top in the
  `lies_on` mask (opaque ground and building blocks, not plants, crops, models, machines, liquids, ice) with a
  per-column hashed chance; 2 turns still water under open air into `lake_ice`; 4 thaws (snow layer -> air, lake ice
  -> `water_static`). Cells are static: they are marked dirty for the paste and nothing is woken, so a winter never
  wakes the bubble. `SeasonWorld` runs `Settings.season_columns` (3) chunk columns every `season_pass_s` (0.1 s) in a
  fixed shuffled order: snow builds up patchy over ~20 s sweeps. Mode: thaw when the day's mean outside (the hour's
  swing taken out) is over `thaw_c` 2 C; freeze under `ice_form_c` 0 C; snow while it snows (Weather, below 0 C).
  Only the bubble gets it; what was laid is pasted into the terrain and saved like any edit.
- **Why new materials, not snow / ice:** generated snow ground (14) and ice (30, melts by heat to moving water) must
  not be eaten by a thaw or wake a lake; 145/146 have no heat transitions, only the pass changes them.
- **Shader:** the season's snow tint skips the ice tile, so a frozen river reads as ice under snowy banks.
- **Summer:** Weather multiplies its rolled shower chance by `season_rain_mult` (summer 0.35) and the storm share by
  `season_storm_mult` (summer 2.2): rare rain, mostly thunderstorms whose lightning (ADR 0149) lights fires. While a
  summer is dry, `SimWorld.set_dryness(drought_fire_mult 2.5)` scales the chance a burning cell lights a neighbour,
  and `Farming.drought` stops crops on unwatered farmland (look line "Parched").
- **Farming rhythm:** crops.json `grows_in` (season names) is a crop's window: beans, potatoes and flax grow in
  spring and summer only; others keep the calendar's share (half in autumn). Out of its window the look line says so.
  Beans are tender (`frost_c` 0.5). The first autumn frost night within `frost_warn_days` (3) is announced in chat
  once a year. Winter already rests crops, empties berry bushes and leans animals (ADR 0061), so stored and preserved
  food (ADR 0162) carries the winter.
- **Testing:** `/season [spring|summer|autumn|winter] [now]` shows the state or jumps to the season's first day
  (January, April, July, October); `now` lays or clears snow and ice over the whole bubble within a few frames
  (`season_rush_ms` 1.5 ms of passes a frame; all at once was a 45 ms frame).

## Consequences
- Measured at the default 72 m render distance while snowing at midnight: p99 3.4 ms, max 7.6 ms, budget_ok, pass
  at most 0.9 ms (`loop.season_world` in the JSON line).
- Old saves keep their date: the calendar file now stores `month_days`, and a save without it (9-day months) is
  converted on load to the same year, month and share of the month.
- Snow does not lie on low plants (a thaw clears the layer, it would delete them); instead the terrain shader snows
  in tall grass, ferns, straw grass and flowers on every face but the bottom while the season's snow tint is on.
- Fouled water (ADR 0155) does not freeze, on purpose: ice melts back to clean water (`ice` melts_to water, the
  pass's lake ice to `water_static`), so a winter would launder pollution; open grey water in a frozen landscape is
  also the tell that it is fouled. A fouled ice material would need an id of its own.
- Flowing water (moving cells, rapids, terrace edges) is not frozen by the pass; only the heat sim freezes it, below
  water's own freeze_c.
- In game (2026-11-07): `/date set november 2` brought "first frost in about 1 day"; a 1 m fire ball on plains
  grass at noon grew from 120 to 170 flame cells in 20 s in a dry summer and died to 6 in spring.
