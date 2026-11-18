# 0152 – The deep grid is layered: one grid to the liquid core, plain cubes inside it

- **Status:** accepted (the developer's calls of 2026-11-18: dig through the whole planet; the layered grid over
  shells with seams; the large core). Each build step below gets its own ADR.
- **Date:** 2026-11-18

## Context
The developer wants a planet that can be dug to and through its centre (`docs/design/deep-world.md`). The grid
stops 192 m down (ADR 0063). Its columns point at the centre, so they narrow with depth: no grid of blocks
aligned with "down" keeps its block size all the way. `prototypes/deep-shells/README.md` drew the options;
a research pass (Blocky Planet's shells, the Yin-Yang-Zhong grid, StarMade, Eco) found no game that reaches the
centre of a blocky planet without a visible artefact.

## Options considered
1. **Shells with seams** (`world-data.md`'s sketch): seven places where the columns halve. Floors stay level with
   gravity everywhere, but at every seam four blocks sit on one and block size jumps 2 to 1. The developer saw
   the pictures and found it ugly.
2. **A plain cube grid for the whole deep.** Perfect cubes, but stepped floors from the crust down, at full gravity,
   and a ragged join to the crust in solid rock.
3. **Layered, following the planet's own zones.** No size jump in natural rock; the grid change lies in the
   liquid, which is not drawn as blocks.

## Decision
Option 3, with the large core. For Terra (radius R, 4245 m on the standard planet):

| Zone | Radius | Grid |
|---|---|---|
| Crust and mantle | R down to R/√2 (the top 1243 m) | today's grid, carried on down; blocks shrink smoothly from 1 m to 0.71 m |
| Liquid outer core | R/√2 down to 0.19 R (807 m) | liquid metal, drawn smooth; under it, halving shells carry its columns and what a player builds there once it is drained |
| Solid inner core | the last 0.19 R | a plain grid of 1 m cubes; no shells, nothing special at the centre |

- **Liquid metal never freezes into blocks** in the outer core: it is liquid or gone, so natural rock never
  shows a seam. Seams exist only in what a player builds in the drained core (a wall's width changes by a fifth).
- **Gravity** falls evenly to zero at the centre; on the inner core's surface it is 0.19 of the surface's.
- **New worlds only.** A save from before keeps its 192 m floor.

Build order, one feature a step: (1) the mantle, today's grid down to R/√2; (2) the liquid outer core and its
hidden shells; (3) the inner core's cube grid; then the field, pressure and the caves of `deep-world.md`.

## Consequences
- Blocks at the mantle's bottom are 0.71 m: 2.8 to the cubic metre, so digging is slower there.
- The inner core's surface is stepped away from its six axes (a 1.35 m sawtooth at 45 degrees). How that
  walks at a fifth of gravity is not known until step 3 is played; shells for the inner core are the way back.
- Where the cube ball meets the liquid's columns the join is ragged; it shows only once the core is drained.
- The crust and mantle are 1472 layers deep where 140 were: streaming, saves and the bake's cost per column
  are measured in step 1 (`docs/perf-debt.md`).
- Luna and the small planets: their zones are set when their step comes; Luna has no liquid core.
- Not tested by the spike: meshing and physics at the hidden seams, saves, streaming.
