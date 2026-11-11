# 0041 – The evening play-test batch of 2026-11-11

- **Status:** accepted (review and fast gate still to run, see the end)
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

Still to do before handover: `bash tools/check.sh fast`, the fixed views (expected to move a little: the 1 mm
overlap, the night sky, lamps), and a fresh reviewer on the diff. The usage limit stopped this session first.
