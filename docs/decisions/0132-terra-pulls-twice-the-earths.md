# 0132 – Terra pulls twice the Earth's

- **Status:** accepted (the developer, 2026-11-15: "make gravity on terra heavier", then 2x of 1.5x, 2x, 3x)
- **Date:** 2026-11-15

## Context
`world::G` was the Earth's 9.81 m/s². On 1 m blocks a jump of 1.25 m took a second and falls felt floaty.

## Options considered
1. 1.5x (14.7 m/s²): a jump 0.82 s; mild.
2. **2x (19.62 m/s²):** a jump 0.71 s; what many first-person games use.
3. 3x (29.4 m/s²), about Minecraft's: a jump 0.58 s; very snappy.

## Decision
- `world::G` = 19.62. Every body's pull follows it (ADR 0037), so Luna keeps its share: 5.3 m/s², 3.7 times less.
- **The jump stays 1.25 blocks** on Terra: `phys::JUMP` 4.87 to 6.84 m/s. On Luna about 4.4 m, as before.
- **Fall damage stays a point a metre fallen** (ADR 0040: it is written in metres of a Terra fall). In speed the
  free landing is 10.8 m/s (was 7.7) and 30 m/s kills from full (was 21).
- **Falling blocks follow** (`fall::due`, which had its own 4.905): 6 m in 16 ticks, was 22.
- **The free turn in space** starts where it did: `phys::FREE_G` is G / 9.81, not an absolute 1 m/s².
- **Left on the Earth's 9.81 on purpose:** the particles (`fx.rs`: every row of `data/effects.toml` was tuned by
  eye against it) and the liquid simulation's flow cap (`liquid.rs`: its own tuned numbers, and the lower pull is
  the stable side).

## Tests
- Red first: `phys::tests::terra_pulls_twice_the_earths_and_a_jump_is_quicker` failed on G = 9.81; passes (the
  jump's time in the air 0.71 s within 0.04).
- `a_jump_is_a_block_and_a_quarter_on_terra_and_higher_on_luna` holds with the new push.
- Changed with their numbers: the impact test's kill speed (21 to 30 m/s), the sand fall's ticks (22 to 16).
- engine 330 passed, server 78, bot 7. client 157 passed and 3 failed: `models::tests` on a creature's scale,
  which fail on main too since "Make the animals a quarter larger" (dc057be3) and do not read gravity.

## Review (a fresh reader of the diff)
Found and fixed: `fall::due`'s own constant; `FREE_G` absolute, which moved the free zone from 3.1 to 4.4 Terra
radii and put the orbit run's waypoint inside the held zone; stale numbers in `fundamentals.md`, `survival.md`,
`data/effects.toml` and the playtest menu. Left, to judge by playing:
- **Swimming:** the lift follows G and the drag does not, so bobbing is stiffer and a deep dive rises faster; a
  fall into water goes about 9 m deep before it stops, was 6.6 m, so shallow water breaks a fall less.
- **Sideways hits** cost less: 10 m/s into a wall was a point or two and is free now.
- A fall passes 20 m/s after 10 m, not 20 m: the wind's sound and the streamer's coarse check come in sooner.
- Water and particles fall at the Earth's pace beside a heavier player and sand.

## Consequences
- Orbits are faster: a surface orbit of the 4 km planet 289 m/s, a lap 92 s (was 204 m/s, 131 s).
- Not measured: no speed run; the change is constants only.
- Not played: only tests and one off-screen play run with a jump.
