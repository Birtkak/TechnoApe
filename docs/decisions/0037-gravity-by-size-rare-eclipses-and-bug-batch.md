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
