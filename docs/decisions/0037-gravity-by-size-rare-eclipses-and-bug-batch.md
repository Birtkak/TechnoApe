# 0037 – Gravity by size, rare blood moons, and a batch of play-test fixes

- **Status:** accepted
- **Date:** 2026-11-11

## Context
A play-test batch of F9 reports (2026-11-10 and 11): gravity was G on every body and a jump rose about
1.2 m; the moon was red most nights; lamp light made hard shapes on walls; flying fast made the ground
vanish; the level-of-detail seam on Terra seen from Luna was a hard band; leaving or entering the menu
flashed the window. Each part below records what we found and chose.

## A. Gravity follows a body's size (bug 2026-11-11_13-08-58)

Options for Luna's pull: (1) a `gravity` row in `data/bodies.toml`: free but not what the developer asked
("based on the size"); (2) proportional to the radius (same density as Terra): Luna 0.4 m/s², a 30 m jump,
too floaty for 1 m blocks; (3) the square root of the radius ratio: Luna 2.0 m/s², a fifth of G, close to
the real Moon's sixth. **Chosen: (3).** `World::surface_g`: G at the first body's (Terra's) sea-level radius
whatever its size option, `G * sqrt(R / R_terra)` on another.

Towards the core: the developer wants gravity "a bit stronger" deeper in. **Chosen:** the square law
`g_s * (R / r)^2` everywhere, with `r` no less than 0.9 R, so it grows to at most 1.23 times at 0.9 of the
radius and stays there (real physics would weaken inside; this is the asked-for feel). Above the surface the
same law fades it as before (it used to be flat G up to R + 80 m). On Terra's ground (within 80 m of sea
level) it is 0.96 to 1.04 G.

A jump is one push everywhere, `JUMP = 4.87 m/s`: 1.25 m at G at the client's 60 Hz step (it was 4.8, about
1.21 m). Measured (`a_jump_is_a_block_and_a_quarter_on_terra_and_higher_on_luna`): 1.265 m at Terra's
spawn (27 m over sea level, 9.69 m/s²), 6.8 m on a Luna hill (1.82 m/s²).

With Luna this weak Terra's own pull there (1.2 m/s²) mattered: summed as before it would pull a player on
Luna's near side up. Near a moon only the tide is left: its frame subtracts the planet's pull at the moon's
centre, fading out from half to 0.97 of its sphere of influence, so gravity is still the same either side of
the hand-over between frames. `FREE_G` (where the mouse turns the body itself) went from 2.0 to 1.0 m/s², so
Luna's surface is not deep space.

Tidal lock: Luna already keeps its body -x to Terra (`sky::Sky::moon`, test
`the_moon_keeps_its_face_and_its_orbit`); nothing changed.

Client and server step the same `phys::step`, so they agree.

## B. The red moon (bug 2026-11-11_13-17-26)

Cause: the sunlight colour the shader lights every surface with (`sun_col`) is the sun as seen through the
air at the camera. At night the camera's sun is under the horizon, so that colour is a deep sunset red, and
the moon, lit by it, was red every night. Eclipses were never the cause: `a_blood_moon_is_rare` counts the
midnights over the sky's whole 432-day period with the moon more than half in Terra's shadow: 16 of 432
(3.7 %), in 11 eclipses, one about every 39 days. That frequency comes from the 40-degree tilt of Luna's
orbit (ADR 0022) and is kept; even a polar orbit would only bring it to about a quarter of full moons,
because Luna orbits so close that Terra's shadow is 41 degrees wide, so an eclipse also lasts about one and
a half nights.

**Chosen:** the shader lights a body other than the camera's with white sunlight (`sun_on`; the camera's
body index rides in `sun_col.w`). In an eclipse, what a body with air hides of the sun comes back as `COPPER`
(0.23, 0.07, 0.02) along the sunlight's own ray (so hills still shade it), and the whole moon glows evenly copper red, about a third as
bright as the sun's red channel. Measured in the report's view (`run=stand ... time=124231.8`, a 64 % moon):
the moon's pixels were red over blue 2.4, now 0.9 (grey). An eclipse (`at=0.2046,0.96321,-0.17425 pitch=89
time=4350 seed=1337`): an even copper disc, red over blue 3.5. Cost: one compare and one max per light()
call, nothing on the CPU.

## C. Lamp light in shapes (bugs 2026-11-11_13-00-53 and 2026-11-10_19-45-23)

Cause: a lamp's shadow ray aimed at the middle of its block and stopped 1 m short. For a lamp set into a
floor or wall (the usual way to light a dug room) the middle is half a block inside; a ray from a wall that
came in flatter than about 30 degrees passed through the blocks beside the lamp before its stop and counted
the lamp as hidden. So each lamp lit the walls only inside a cone, whose edge drew hard arcs and slanted
lines, and with several lamps those overlapped into the polygon patches of the report. A lamp standing on
the ground shadowed the ground by its own corners the same way (the black spikes of 2026-11-10_19-45-23).

Options: (1) soft lamp shadows by aiming at a random point of the lamp: noisy, and the same cone; (2) aim at
the point of the lamp's block nearest the surface (a block lamp is an area light): exact, one ray as before;
(3) trace every lamp: more rays. Also tried: easing the third traced lamp over to the untraced lamps' mean
visibility where they swap places; it changed nothing visible, so it was dropped. **Chosen: (2).**
`lamp_target` (shader.wgsl) works in the grid's own coordinates (the face's two angles and the log of the
radius), where a block is a box, clamps the ray's origin into the lamp's cell plus 2 cm, turns that back,
and the ray stops 1 cm short of it. Faces the surface is not beyond are not pushed out, so a ray along a
floor to a lamp on that floor stays over it.

Evidence (`run=stand at=0.038284,0.999241,0.007201 eye=-5 pitch=-30 time=900 seed=443734500 planet=6144
place=` a 5x5 pit 7 deep with 8 lamps): before, arcs and slanted edges across the walls and the lower walls
dark; after, the walls evenly lit, falling off with distance only. The 2026-11-10 view with one lamp on the
grass: no spikes, and the grass around it now glows (mean red 35-44 near it against 22-24 away; it was
darker near the lamp than away from it with the first try of this fix). Fixed views (`tools/views.sh`):
unchanged. Cost: on the GPU a few atan/tan per traced lamp (at most 3 a pixel); nothing on the CPU.

Review (a fresh reviewer read the diff): found that the first version of `lamp_target` projected onto the
block's two side normals as if they were square, which they are not near a face's corners (fixed: grid
coordinates), and that the copper glow skipped the body's own horizon and terrain (fixed: it shares the
sunlight's ray); gravity's centres were worked out twice per step in a moon's frame (fixed). `FREE_G` 2 to 1
is deliberate (part A).
