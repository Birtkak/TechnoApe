# 0089 – Flora: fluffy crowns, plants of planes, more trees

- **Status:** accepted as direction (the developer, 2026-11-13); nothing built. `docs/design/flora.md`.
- **Date:** 2026-11-13

## Context
Trees are a trunk under a cube crown, small plants a few voxel models, and the land between is bare grass. The
developer wants all flora to look better, with Vintage Story as the reference (five screenshots).

## Options considered
- Crowns: fluffy (planes past the block) / cubes with open holes (ADR 0088) / dense solid cubes.
- Small plants: crossed flat planes / our 1/8 m voxel models / both.
- Trees: branches and more species / the same species better shaped / also giant old trees.
- Extras: wind sway, colour by climate, dense meadows, flowering and fruit.

## Decision
The developer chose: **fluffy crowns**; **crossed flat planes** for small plants; **branches and more species**
(no giant trees for now); **all four extras**.

**Added the same day** (the developer, after the first sheets: "still too low quality of details and colours"):
- **Flora is painted at 32 pixels per metre** (leaves, bark, plants on planes); other blocks stay at 16. The
  developer had no preference between the options; 32 for all flora was the recommendation, since crowns at 16
  over a meadow at 32 would not match.
- **Rich colour, like the references:** six to eight tones with hue shifts (warm highlights, cool shadows),
  saturated, and two or three variants per plant. This replaces "calm" for flora only.

## Consequences
- Drawing see-through plants is the cost: ADR 0088's leaf shells alone took the forest fly-through from 6.3 to
  7.4 ms p99 against a budget of 8.3. Every part starts with a measured spike and the default settings must hold
  the budget; a Vegetation setting scales density.
- ADR 0088's branch stays unmerged: the crowns part takes it as its base and decides its fate.
- Tree rules change what a seed grows: saves' unedited trees and the fixed views change.
- Each part gets its own ADR (0090 crowns, 0091 plants of planes, 0092 trees; paintings under ADR 0084).
