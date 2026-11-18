# draft – A body's interior from its row

- **Status:** proposed
- **Date:** 2026-11-18

## Context
Step 1 of `docs/design/planet-model.md`: a body's row in `data/bodies.toml` states what it is made of, and its
interior is derived from the row, instead of Terra's numbers spread over the engine as constants: one `G`, one
core profile (`CORE = 1/sqrt 2`, `CORE_DENSE = 2.4`, `world/frames.rs`) on both bodies, the crust's and the
mantle's density in `rocks.toml`, surface gravity as `G` times the radius over Terra's, `Body::rock_weight` and
`Body::pressure` with Terra's `G` on any body (marked as a shortcut), the mantle's depth at a hard-coded R/sqrt 2.
Nothing observable may change: generation 3 stays, Terra and Luna must come out as before.

## Options considered
1. **Derive the core's radius from `iron` as a mass share** (`cbrt` of a share). Rejected: Terra's core top is
   R/sqrt 2 because ADR 0152's grid floor is there; a cube root of any share does not give 1/sqrt 2 to the bit,
   and that number decides the mantle's layer count and the weight of rock, so generated blocks.
2. **A general N-zone mass model with each zone's own density** (the crust at 2700 in the pull). Rejected for
   now: it changes the pull's profile by a few per cent, so the pressure, so the mantle's rock boundaries and
   the deep ores (made_hash). That is a new generation's change, not this step's.
3. **The row states the honest inputs and today's simplifications stay explicit** (chosen).

## Decision
**The row's makeup** (`bodies::Makeup`, checked in `bodies.rs`, every message names the body):

| Input | Unit | Terra | Luna | Check |
|---|---|---|---|---|
| `kind` | rocky, icy, gas | rocky | rocky | icy and gas refused: "not built yet" |
| `iron` | the metal core's radius over the body's | 1/sqrt 2 (stated) | 1/sqrt 2 (Terra's) | over 0, under 1 |
| `heat` | 0 dead (core all solid) .. 1 young (all liquid) | 0.65 | 0.05 | 0..1 |
| `density` | crust, mantle, core, kg/m3 | 2700, 3300, 7920 | the same (Terra's) | 500..30000, none lighter than the one over it |
| `weight_g` | m/s^2, optional: the pull rock is weighed with, where not the body's own | none | 19.62 (Terra's) | over 0 |

The crust's and the mantle's densities moved out of `rocks.toml` into the rows (a body's rock is per body by
nature); `rocks.toml` keeps the mantle's rock boundaries and the provinces. `iron` is **stated** as a radius
share, not derived: 1/sqrt 2 is the grid's choice (ADR 0152), parsed to `FRAC_1_SQRT_2` to the bit. The core's
density is stated as 7920 = 2.4 x 3300, whose quotient is the literal 2.4 to the bit.

**Derived** (`crates/engine/src/interior.rs`, `Interior`, computed once per body when the world is made and kept on
`Body::interior`; no per-cell cost, no new transcendental math on a block's path):
- **Zones**, centre out: solid inner core to `(1 - heat)` of the core's radius, liquid outer core to `iron x R`,
  mantle to the crust's floor `R - DEEP` (never under the core's top), crust to R; each with its density.
- **The pull's profile** (`pull`, `pull_integral`): ADR 0152's, now with the row's numbers: core share c =
  `iron`, density ratio k = core / mantle, `norm = k c^3 + 1 - c^3`; inside the core `k x / norm`, over it
  `(k c^3 + x^3 - c^3) / (norm x^2)`, outside `1 / x^2`. The same expressions in the same order as before.
- **Mass** `4/3 pi mantle norm R^3`; `enclosed(r)` and `zone_mass(k)` from the zones (test: they add up).
- **Surface gravity from mass:** the world's gravitational constant `k = G R0^2 / M0` is set by the first body
  pulling `interior::G` (19.62, the developer's tuned feel, ADR 0037: whatever the planet's size), and every
  other body pulls `k M / R^2`. `gravity(r) = surface_g x pull(r / R)`.
- **Weight and pressure:** `rock_weight(ground, r)` = density x `weight_g` x R x `pull_integral`, the crust's
  over its floor, the mantle's under it; `Body::pressure` adds the water with the same `weight_g`.
- **Liquid core**: volume `4/3 pi (rc^3 - ri^3)` and its share of the body's volume.
- **Magnetic field**, one number over Terra's: `field = (liquid share / first body's) x (spin / first body's)`;
  0 without liquid metal. Simple and monotone: more liquid metal and faster spin give a stronger field, as in
  the dynamo scaling laws' leading terms; the share (not the volume) so Terra's planet= size does not change it.
  The spin is the sky's (`interior::spin`: Terra once a sidereal day, Luna once a sidereal month). Value only:
  printed by `report=1`, read by nothing yet.

The readers: `World::gravity` and `surface_g` (`frames.rs`), `Body::rock`, `rock_weight`, `pressure` and
`with_mantle` (the mantle's layer count to the row's core top, the metal sea only with a liquid core) in `body.rs`.
`phys::G`, `fall.rs` and `vitals.rs` (through `phys::G`) read `interior::G`, the one felt-pull constant. Removed:
`world::G`, `CORE`, `CORE_DENSE`, `pull_inside`, `pull_integral` in `frames.rs`, `Rocks::density`,
`Rocks::mantle_density`. Kept as the grid's, not a body's: `CRUST`, `DEEP`, `MANTLE_LIFT`, `MANTLE_STEP`.

**Terra's numbers on Luna, to be made Luna's own at the next generation** (each reproduces today's behaviour):
1. `iron` 1/sqrt 2 (the Moon's core is about 0.2 of its radius): her inner pull profile is Terra's.
2. `density` 2700, 3300, 7920: Terra's rock and metal.
3. `weight_g` 19.62: her rock is weighed with Terra's pull, not her own 5.31 m/s^2 (the old shortcut in
   `rock_weight`), so her pressure is Terra's at the same depth.
4. Falling blocks (`fall::due`), the jump and fall damage use `interior::G` on every body, as before.

**Terra's simplifications on every body**, for the next generation too: the pull reckons the crust at the
mantle's density (ADR 0152's one shell; it weighs at its own 2700); inner and outer core share one density.

## Checks
| Check | Before (main 63c92f37) | After |
|---|---|---|
| `report=1 seed=1 planet=6144` made_hash, mantle=0 / gen=1 / gen=2 / default | 0926f028ddcd85a7, 37b2d1b1006b66f7, a2ef250b3ed4203a, bfb878ce14d32b75 | the same; the whole report identical but the new `interior` block and the timing |
| pressure lines (spawn, hottest, coldest columns) | as recorded | identical |
| the weight of rock at 7 radii, Terra and Luna (`record_tests.rs`) | recorded bits | identical to the bit (asserted) |
| the pull at 12 radii, Terra and Luna | recorded bits | Terra 12 of 12 to the bit; Luna within 1e-12 relative (her surface pull is now `k M / R^2`, a few ulps off `G r / R0`) |
| the mantle's layers, r0, the metal sea | 1280, 2997.758.., Some(0.0) | the same |
| `run=stand quick=1`, `run=fly quick=1` job_us_avg, frame p99 (play build) | see the commit | the same within noise |
| `bash tools/views.sh` | baselines | pass |

## Consequences
- A new rocky body is a row; its pull, weight and field follow. Gas and icy bodies are refused until built.
- Not done here (the design's build order): the geotherm and the regime (heat with depth stays the provinces'
  `heat`), N bodies instead of two (step 2), the field's first effect (step 3), one bake pipeline (step 4), the
  Mars-like planet (step 5); the surface, sky and orbit inputs (water, air, sun, spin, tilt, year, age, orbital
  elements) are later steps' rows.
- The next generation can give Luna her own makeup and the crust its own density in the pull; it must then
  bump `GENERATION` and keep this profile for older worlds.
