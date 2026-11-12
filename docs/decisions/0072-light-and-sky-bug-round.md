# 0072 – The light and sky bug round (2026-11-12)

- **Status:** accepted
- **Date:** 2026-11-12

## Context
Six of the developer's F9 reports from 2026-11-11 and 2026-11-12 about light, the sky and the air read-out.
The developer had already decided two of them: lamps get about twice the reach, and Luna keeps its true
orbit, so a moonless night should be rich rather than empty.

## Decision, report by report

1. **`2026-11-11_17-43-39` "lamps should give off much more light".** The lamps now reach `LAMP_R` 24 m (it
   was 12). The lamp grid's cells are now 8 m (they were 4), so a lamp still sits in at most 7 cells along
   each axis: the grid's memory and its CPU cost per frame (`gpu.rs lamp_grid`) are unchanged. The light
   now falls off as `2 / (1 + 0.1 d^2)`, faded to nothing at 24 m. Within 3 m that is about as bright as
   before (~1.0 at 3 m, was 0.81), twice as bright at 6 m (0.38), 0.12 at 10 m (was 0.01), and a glow to
   20 m. Stretching the old curve 2x was tried first: it washed a lamp's neighbours out by day (three
   blue lamps turned the grass and dirt mint white). Caves stay dark beyond 24 m. Coloured lamps follow
   the same curve.
2. **`2026-11-12_20-49-17` "dirt blocks should not still cast shadows in between the lamps".** Why:
   not one lamp per pixel. Each of the three strongest lamps at a pixel has its own shadow ray, and the
   rest share their mean visibility. The shadows were hard because every ray aimed at a single point of
   the lamp, the nearest one, so the lamp acted as a point light. With the 12 m reach, the neighbouring
   lamp added almost nothing a few metres away. Now `lamp_target` aims each ray at a random point (per
   pixel) of the lamp's sides that the surface can see, inset 5 cm. So a lamp is a 1 m light and its
   shadows are soft, dithered per pixel the way the sun's are. With the doubled reach, neighbouring lamps
   fill each other's shadows. Two things are left as they are, because they are physically right:
   a one-block dirt row still shades the ground behind it from lamps of the same height, and by day the
   sun's shadow still shows where the lamps add little against the sun.
3. **`2026-11-11_17-38-04` "small white dots between the grass blocks".** The dots in the screenshot are
   the bright lamp face showing through gaps down the middle of the aim outline's lines (pixels x
   1146-1159 and 1233-1235). Those lines run along the grass seam at the lamp's foot. That is the 1440p
   outline gap of bug 2026-11-11_17-38-47, fixed in 02128c8 (ADR 0041), which came after this report's
   build (eb26d01). A night render with a lamp on grass found no light leaking through the seams.
   Nothing to change.
4. **`2026-11-11_17-48-16` "glitched texture".** It does not reproduce on today's build. The report's
   save is from the old noise land, and the day is now 1800 s, so its run line shows another place
   and time. Rendered on land with the sun 7 degrees up and facing it, the open ground averages a
   luminance of 44 against 20.7 in the report's screenshot. The light fixes of ADR 0041 came after that
   build. The "flat dark patches" are the long sun shadows of a 7-degree sun.
5. **`2026-11-11_17-41-28` and `2026-11-11_17-47-11`, the stars.** Luna stays on its true orbit. Moonless
   nights get a galaxy band twice as bright and a little bluer, faint stars a little brighter, and a new
   layer of a few dozen bright stars with a wider glow (`stars()` in `shader.wgsl`). The stars stay fixed
   in the inertial frame.
   **Drift, checked:** the star field turns by `world.pose(frame, time).rot`, the planet's real spin. The
   client's clock moves smoothly toward the server's (`frame.rs`, eased over 0.3 s), so time warp makes no
   jumps. The yearly period wraps continuously (`PERIOD` is a whole number of days and years). The frame's
   rotation is the body's own, and the camera adds nothing to it. So the drift is physical:
   - The planet's spin turns the sky 0.2 degrees a second at 1x (360 degrees in 1800 s); time warp
     multiplies that.
   - Travel round the planet tilts the local horizon against the stars by v / R. At the report's
     328 m/s on the 4245 m planet that is 4.4 degrees a second; at the creative maximum of 1 km/s it is
     13.5 degrees a second.
   - That is the price of a small planet. Earth turns the same sky at 0.003 degrees a second for a jet.
   **It may well still feel sickening when flying fast, and that is not faked away here.** If it is, the
   honest options are the developer's: a lower flying top speed near the ground, or a game setting that
   damps the star field's turning (a fake).
6. **The F3 air of the 2026-11-12 reports: -395.5 C on a temperate forest, -6409 C at 60 km.** This was a
   unit bug in the read-out, not in the grid. Both the server's `ToClient::Air` and F3 took the height
   over the sea as `radius - sea`. But `Body::sea` is a *layer number*, so the height came out about
   4 km on the standard planet, and the 0.1 K/m lapse gave -395 C. The fix:
   - `Body::over_sea` gives metres over the sea, and both read-outs use it.
   - The read-outs now share the grid's two-slope lapse, `air::Params::cool` (0.1 K/m to 150 m, 0.015
     above; the grid already used it). `Weather::here` and F3 used only the first slope.
   - Above the air's top layer (1500 m over the sea) F3 says there is no air rather than reading the
     lapse past absolute zero.

   Nothing else read the wrong value: the grid, the biomes (`bake/life.rs`, the ground's height) and the
   rain use their own correct heights. The server test of the live air now checks the sent value against
   the air at the player's height; it failed (5.7 C sent, 15.4 C true) before the fix.

## Consequences
- CPU: the lamp grid has the same number of entries per lamp, so there is no change. The shader looks
  at about 8 times as many lamps per pixel; it still traces at most 3 shadow rays a pixel and adds the
  rest unshadowed. Measured by the fast gate's perf check (see the merge commit).
- A lamp's light now reaches 24 m, so lamps far down a cave light further along it, but never past
  24 m.

## Calls for the developer to confirm
- The lamp curve: about as bright up close, 0.38 at 6 m, 0.12 at 10 m, nothing past 24 m.
- Same-height blocks still cast soft lamp shadows. A non-directional lamp "fill" would remove them but
  would leak light through walls.
- The star drift when flying is left physical (13.5 degrees a second at 1 km/s). A damping setting
  would be a fake.
- How many bright stars, and how bright the band is.
