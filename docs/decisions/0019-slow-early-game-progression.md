# 0019 – Slow early game: flint, copper, bronze, iron

- **Status:** accepted (direction); ore veins and ground pickups built 2026-10-23 (GEN_VERSION 9), other systems proposed
- **Date:** 2026-10-22

## Context
The recipes in the game copy Minecraft's opening: punch a tree, 4 planks per
log, wooden then stone then iron pickaxe within minutes. The vision asks for
GregTech depth ("crafting is decisions, not lists"). Asked about bronze's
place, the developer chose copper before iron and a slow-burning early game:
"flint tools to shave off bark and slowly grift for nuggets so you can make
an iron pickaxe after some grinding. Early game GregTech: New Horizons meets
Vintage Story."

## Options considered
1. **Minecraft ladder** (wood, stone, iron, diamond-like). Familiar and quick;
   no reason to explore, sift or build stations early. Rejected by the developer.
2. **Metal ages with a sifting economy** (flint, copper, bronze, iron, steel).
   Every age needs a new station or process; nuggets from sifting give a safe,
   slow route next to mining. Costs new systems: tools that stay in the grid,
   chance-based sifting, chance drops, more station blocks.

## Decision
Option 2, as laid out in `docs/design/progression.md`: five tool tiers
(flint 0, copper 1, bronze 2, iron 3, steel 4), sifting gravel and sand for
copper, tin and iron nuggets, casting copper at a campfire with a crucible
and mold, iron through a bloomery and a hammer, steel in a blast furnace.

## Consequences
- Recipe data gains `keep` (ingredients that stay and lose durability) and a
  `sift` recipe type with `chance`; `recipe_registry.gd`, the crafting grid
  and the recipe browser must support both before those recipes go live.
- `mining.json` gains chance drops, logs need an axe, ore `min_tier` values
  rise and `tool_speed.tier` grows to five entries.
- New station blocks (campfire, sieve, bloomery, blast furnace) follow
  ADR 0007. The atlas is full, so they wait on a bigger atlas.
- The Minecraft openers (wooden/stone pickaxe, 4-plank crafting table,
  furnace iron) are removed when the stone age goes live; tests that rely
  on them change with it.
- Ore generation follows GregTech too (developer, same day): mixed veins on
  a grid with depth bands, small ores and surface indicators, from
  `ore_veins.json`; replaces the ore noise in `world_gen.cpp`.
- Same day, the developer added: Vintage Story smithing (forge, anvil, hammer,
  tool heads), food and cooking with a hunger bar, a bed that skips the night,
  saplings, a sun and moon, and ground pickups. Hunger answers the open
  "survival pressure" question in vision.md for food (temperature stays open).
- Numbers (drop chances, nuggets per ingot, durability) are first guesses,
  tuned in play through the recipe book.
