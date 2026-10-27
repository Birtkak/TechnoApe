# 0095 – Tools and ingots set down in the world

- **Status:** accepted (built 2026-10-27)
- **Date:** 2026-10-27

## Context
Requested 2026-10-27: "Make it so u can set tools and ingots down on the floor/lean against the wall
like vintage story". Clay ware already lies on the ground as a `ground` work piece (ADR 0023), firewood
piles grow one log per click (ADR 0030), and work pieces draw item sprites (`view_parts().items`,
ADR 0042). Sneak + right click already means "do the bulk or the alternate thing" (firewood, the forge,
clay blocks), and Vintage Story sets things down with sneak + right click.

## Decision
One block entity class, `LaidEntity` (`scripts/work/laid_entity.gd`), in three work kinds:

- **`laid_tool`**: sneak + right click on a top face with any `tool` item lays it flat (a one-voxel
  high `WORK_SPOT`), turned to the player's facing in 90° steps (+45° so the sprite's diagonal handle
  runs along the look).
- **`leaning_tool`**: sneak + right click on a wall face leans the tool on it: a block-sized `WORK_SPOT`
  flush with the wall face (natural walls are off the block grid, so the anchor follows the face,
  `LaidEntity.lean_anchor`), snapped to the block grid along the wall, standing on solid ground. Drawn
  upright, turned 45° in its own plane so the handle stands, tipped 18° back onto the wall.
- **`ingot_pile`**: sneak + right click on a top face with an `ingot` starts a pile (`WORK_BLOCK`, solid
  like a firewood pile). A right click holding the same ingot adds one, up to `Settings.ingot_pile_max`
  (64, Vintage Story's), drawn as cast ingots in crossing layers of 8 on a 3.125 cm grid (the solid
  view part may name its `grid`). A sneak click on a full pile starts the next one on top. Hot and cold
  ingots do not mix (`Inventory.same_kind`). Metal colour by `metal.*` tag (new paints BRONZE, TIN).

An empty hand takes the tool or one ingot back; breaking the piece drops all of it. The stack keeps its
per-stack data (wear, `hot_until`) and is saved with the block entities. The rule is the item's tags,
so new tools and ingots need no extra data. Items whose own sneak click comes first (the prospecting
pick's menu, playtest gadgets, buckets) keep it.

## Consequences
- Any tool or ingot can be stored in the world; `showcase=<tool>,<ingot>` films a flat tool, a leaning
  tool and two piles. `test/test_laid_items.gd` covers kinds, anchors, the pile, saving and the clicks.
- Not done: hot ingots on the ground do not glow or cool visibly; other items (plates, food) cannot be
  set down yet; no Ctrl + sneak to add five ingots at once.
