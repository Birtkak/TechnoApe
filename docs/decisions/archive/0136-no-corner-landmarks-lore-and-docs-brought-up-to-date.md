# 0136 – No corner landmarks; the story is written down; the docs brought up to date

- **Status:** accepted (tool tier: docs only, CLAUDE.md principle 13)
- **Date:** 2026-11-16
- **Supersedes:** the corners call of ADR 0027 ("the 8 corners hold something made") and the corner-landmark
  line of ADR 0003

## Context
The developer (2026-11-16): the cube corners as places that hold something made is an old system that was
decided against; the roadmap is behind; go through the docs and put them on track. The same day a story was
laid out in a brainstorm (`docs/design/lore.md`): the Indigenti, a swarm that binds the first animal to touch
its monument, a fuel ladder (chemical, fusion, antimatter) and three systems. It no longer needs the corners
for its structures.

Nothing in the code placed anything at the corners (checked 2026-11-16: only the soak teleports bots onto them
and the grid folds there), so no code changes.

## Decision
- **The 8 corners of each body are ordinary ground.** The bake keeps no sites clear and places nothing there.
  ADR 0003's seam and corner rules (folding, footprints) stay; its "landmarks" line and open question are struck.
- **Where the swarm's monument and other work stand is open.** It is listed in `lore.md` and
  `open-questions.md`.
- **The story is recorded** in `docs/design/lore.md` as a living document. The ending is left open on purpose
  (the developer: decide when the game shows what it has become). `progression.md` is not rewritten yet: it
  still describes one exotic age and changes once the story is settled.
- **The roadmap is brought up to date** (it stood at 2026-11-12; main is at 2026-11-14) and the docs' status lines
  are corrected where the code has moved on.

## What was changed
- ADR 0027 and 0003: notes pointing here. `vision.md`, `worldgen.md`, `open-questions.md`: corners removed, the
  story linked. `lore.md`: written, with the corners removed again.
- `roadmap.md`: Done gets the 2026-11-12 to 14 work (weather, globe map, tides, items and inventory, see-through
  and box models, flora, animals, mining, loose blocks, optimisation batches); In work and Next rewritten; the
  fuel ages and the two new systems are listed under Later.
- Status lines corrected: `clutter-and-pickups.md`, `flora.md`, `see-through-and-boxes.md`, `navigation.md`,
  `rivers-and-tides.md`, `seasonal-looks.md`, `air.md`, `fine-cells.md`, `needs.md`, `effects.md`, `worldgen.md`,
  `world-data.md`, `studio.md`; `worldgen-plan.md` W2b marked built. `README.md`: 1 m blocks, current status.
- `docs/perf-debt.md`: seven rows that were older copies of rows kept beside them were removed. Two features
  still appear twice with different notes (plants of planes, ADR 0091; taller mountains, ADR 0105): merge them by
  hand when their rows are next touched, since the two versions each carry numbers the other lacks.

## Not changed, on purpose
- Historical ADRs that speak of 0.5 m blocks or the old game (0001, 0002, 0003's measurements, 0007, 0031): they record
  what was decided then; ADR 0033 and the fresh start of 2026-11-09 supersede them.
- `progression.md` (see above), and the ADRs' own status lines where a branch's merge was not checked.

## Consequences
- Anything built from here on may not assume structures at the corners. Corners stay a place where the grid
  folds and a test target for the soak and the fixed views.
- The hostile creatures and the exotic resource (ADR 0027's other open calls) now wait on `lore.md`.
