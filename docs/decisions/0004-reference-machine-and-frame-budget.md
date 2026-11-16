# 0004 – The dev PC is the reference machine; the frame budget and how it is measured

- **Status:** accepted
- **Date:** 2026-11-09 (budget first set 2026-10-22, RAM raised to 8 GB 2026-10-27, carried over unchanged)

## Context
The game should run perfectly on the developer's PC, a 2020-era mid-range gaming PC, and with lower settings
on weaker ones. A budget is only worth something if it is measured honestly: the old game's first
numbers were optimistic (engine-smoothed frame times, capture stalls counted, load not finished, native heaps
missing from the RAM figure).

## Decision
- **Reference machine = the dev box:** Ryzen 5 5600X (6c/12t), RTX 3070 8 GB, 32 GB RAM, Windows 11.
- **Budget at default settings, vsync off, after load, in every standard run:**
  - p99 frame time <= 8.3 ms (120 fps class), and no single frame > 16.7 ms (no visible hitch at 60 Hz);
  - process RAM <= 8 GB (working set), VRAM <= 6 GB;
  - the server's tick (added 2026-11-11, ADR 0025 S), measured in `run=sim` (the `stand` view with 50 players
    at work and water running): p99 <= 25 ms, half its 50 ms, and no tick over 50 ms; and the run still holds
    the frame budget above, so the simulation causes no frame over 16.7 ms.
  - A regression past the budget is a bug. Every system ships with a benchmark that reports these numbers.
- **Weaker PCs** get lower presets of the same settings (render distance, LOD, shadows, simulation rates);
  there is no second budget.
- **Measuring honestly:**
  - frame time = wall clock between frame starts, never an engine-smoothed delta;
  - "after load" starts when streaming, meshing and the simulation have had no backlog for 30 frames in a
    row (capped, and reported if the cap was hit);
  - frames that a debug capture (screenshot, filmstrip readback) stalled are excluded and counted;
  - RAM is the process working set, VRAM from the driver;
  - every frame over 8.3 ms is listed with its own phase times (main thread per system, render CPU, GPU), so
    a spike names its cause;
  - a run that shared the machine with another heavy process (a build, another benchmark) does not count:
    the run checks for them and repeats.

## Consequences
- Settings knobs exist from the start for every expensive system.
- The JSON stats of every headless run carry `frame_p99_ms`, `frame_max_ms`, `ram_mb`, `vram_mb`, the spike
  list and `budget_ok`.
