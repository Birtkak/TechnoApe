# 0010 – The planet's air: water cycle, weather, pollution and radiation

- **Status:** accepted (the systems); the design in `docs/design/progression.md` is a proposal
- **Date:** 2026-11-09 (developer's call in a design session)

## Context
ADR 0008 made the planet's facts rules (sun, finite planet, orbits) and survival harsh. ADR 0009 added that
released steam returns as rain. The developer then asked for the water cycle, weather, pollution and
radiation as systems of the game and on the roadmap. They decide what the simulation must carry for the
whole planet, not only around players.

## Options considered
1. **Four separate systems**, each with its own grid and rules.
2. **One coarse air grid** around the whole planet whose cells carry heat, water, pollution and wind;
   weather is what the cells show; radiation uses it for fallout and for the shielding the air gives.
3. **Scripted effects** (random rain, a pollution number per base) with no simulation behind them.

## Decision
- **Developer:** the game has a water cycle, weather, pollution and radiation, and they are planned work.
- **Routine technical default:** option 2, one coarse grid, always running for the whole planet inside the
  deterministic tick. Scripted effects cannot refill a pond honestly or carry smoke to a neighbour; four
  grids cost four times the code and the memory for the same data.

## Consequences
- A spike in `prototypes/` measures the grid's tick cost on the 1 km and 4 km planets before anything is
  built on it. Target: under 0.5 ms of the server tick.
- The temperature field that survival needs (ADR 0008) is part of this grid, so it is built first.
- Water must be counted from its first version (the first playable's water step), or the cycle cannot
  conserve it later.
- Each substance gets a conservation law in the soak test. Counted is not fixed: the player can take water
  out of the cycle, down to a drained planet (ADR 0009); the law only says none appears or vanishes by
  itself.
- Order of building follows the tiers: heat and water with the survival needs, weather with LV, pollution
  with MV, radiation with EV.
- Open taste calls: how hard pollution and radiation punish, and whether damage can be permanent (proposal:
  no).
