# 0025 – Calendar, seasons and a real sky

- **Status:** accepted (built 2026-10-23)
- **Date:** 2026-10-23

## Context
Requested 2026-10-23:
- seasons and months;
- a moon and stars that match real life, with the moon going through its
  phases by the day of the month;
- a bigger harvest moon "etc.";
- real constellations;
- a rare eclipse.

The game had a time of day and nothing else. The sun went round a fixed
circle (sunrise always 06:00), the moon was a disc opposite the sun, and the
stars were hash noise.

The developer chose:
- months of 9 days (one moon phase per night, about 36 h of play a year);
- realistic day lengths at 45°N;
- all four season effects: autumn leaf colour, winter snow cover, crops
  that follow the seasons, and season-based animal spawns.

## Decision
- **`Calendar`** (`scripts/world/calendar.gd`) is a pure, tested model.
  - The date: day counter, 12 months, 4 seasons.
  - The sky's geometry, all real astronomy run on game time:
    - **Sun:** ecliptic longitude once per year, obliquity 23.44°,
      equatorial → horizontal for latitude 45°N.
    - **Days:** 15.6 h in June, 8.8 h in December. Sunrise and sunset come
      from the almanac formula (−0.833°).
    - **Time of day** is local solar time (0.5 = noon, sun due south).
      North is −z, as in the generator.
    - **Moon:** one lunar cycle per month. New moon is at noon on the
      1st, full moon at midnight before the 6th. An odd month length
      makes both instants observable.
    - **Moon position:** the sun's longitude plus the moon's age, with
      latitude 5.145° · sin(λ − Ω). The nodes regress once per 18.61
      game years, so the phase, the rise times and the crescent's
      orientation are all real.
    - **Moon size:**
      - the perigee cycle (±5.5 %, real anomalistic/synodic ratio);
      - Harvest Moon (September) ×1.3 and Hunter's Moon (October)
        ×1.15, larger when near full and tinted orange;
      - the traditional full-moon names per month;
      - "Super …" at perigee;
      - "Blood Moon" in a total lunar eclipse.
    - **Eclipses:** real limits on the moon's latitude at syzygy: solar
      0.55°, lunar 0.96°, total lunar 0.44°. This gives about 0.8 solar
      and 1.4 lunar eclipses per game year, roughly one solar eclipse
      per 45 h of play.
    - **Eclipse duration:** each eclipse plays over ±0.06 day (about
      2.5 min at 20-minute days). The honest contact time at this month
      length would be half a minute.
- **Stars:**
  - Data: 5044 real stars to magnitude 6 and the 88 constellation figures
    (Yale Bright Star Catalogue via d3-celestial, BSD-3; licence kept in
    `data/sky/`).
  - `tools/make_sky.py` bakes them into an equirectangular RA/Dec texture:
    colour from B−V, sqrt-encoded brightness, constellation lines in alpha.
  - It also bakes a Milky Way texture from galactic coordinates, with a
    bulge and the dark rift.
  - The sky shader turns each view direction into RA/Dec with the
    sidereal basis. Stars turn through the night and the seasons, and
    Polaris stands at 45°.
  - Constellation lines are off by default: `constellations=1` on the
    command line, or `/constellations`.
- **Sky shader:**
  - The moon is lit by the real sun direction, so its terminator is
    physically right. It has maria roughly where the real ones are, plus
    earthshine.
  - **Solar eclipse:** the drawn moon crosses the drawn sun at the real
    closest approach. The sky darkens with the covered fraction and the
    corona shows near totality.
  - **Lunar eclipse:** the umbra crosses the moon copper-red.
  - Discs are drawn about 8× real size (sun 2.4°). Eclipse geometry uses
    real limits, and the drawing maps them onto the drawn discs.
- **Light:** one DirectionalLight plays the sun by day and the moon by
  night. Moonlight scales with the phase and the moon's height, so
  new-moon nights are dark.
- **Seasons in the world** (terrain shader, no voxel edits):
  - autumn reds, oranges and yellows on oak leaves, varying per voxel;
  - straw-coloured grass, tall grass and ferns, and brown oak leaves, in
    winter;
  - snow on top faces from December to early March.

  A native `WorldGen.climate_map` (256² temperatures, 16 m texels, 4 km
  across, built on a worker when the player moves 512 m) keeps snow to
  climates no warmer than temperate and keeps hot biomes green.
- **Crops** (ADR 0022 random ticks) grow at the season's rate: spring and
  summer 1, autumn 0.5, winter 0 (`Farming.season_growth`).

## Consequences
- `test_calendar.gd` checks:
  - date arithmetic;
  - sun altitudes and day lengths at the solstices and equinoxes;
  - sunrise in the east;
  - Polaris at 45°;
  - Orion high in a winter midnight and gone in summer;
  - the moon's phase by date and the crescent in the west after sunset;
  - the harvest moon's size;
  - eclipse rates over 60 years;
  - crops resting in winter;
  - saving.
- `time_of_day` keeps its meaning (0 midnight, 0.5 noon), but sunrise
  moves. `/time set` and sleeping move forward to the next occurrence, so
  the date never goes back.
- The calendar is saved with the world (`calendar.json` beside the
  others).
- The ray-cast near field (ADR 0021, off by default) does not draw season
  colours yet.
- Snow is a look, not a material: it does not block placing and the
  simulation does not see it.
