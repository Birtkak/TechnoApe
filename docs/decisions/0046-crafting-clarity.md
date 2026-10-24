# ADR 0046: Crafting you can read (any layout, real amounts, hints)

Status: accepted (2026-10-25); recipe balance open

## Context
Playtest 2026-10-25: "cannot make crafting table", then: "do this with any crafting recipe. its also
difficult to see how much is what, what it needs etc.. crafting needs to be balanced and more clear all
together", "the way we need grid recipes with bigger numbers now".

What made it hard:
- **Shapes nobody knew.** The crafting table was shaped (two flint over two logs). Flint beside the
  logs, or under them, made nothing, and nothing said why.
- **Two number systems.** Since ADR 0034 voxel items (logs, planks, dirt, cobble, ores) count
  12.5 cm voxels: the inventory shows 126 oak log. A recipe takes one *block* of them (64 voxels) per
  slot, but the recipe browser drew the log cell with no number and the planks output as "2", which
  is really 128 plank voxels. A slot holding less than 64 silently counted as empty.

## Decision
1. **Any layout.** A grid recipe matches its ingredients in any arrangement. A shaped recipe keeps
   its shape only as a tie-breaker: when two grid recipes for the same station take the same
   ingredients (`RecipeRegistry.any_layout` is false), the exact shape decides, and a loose layout
   that fits both makes nothing. The browser says which: "Any layout" or "Lay it out as shown".
2. **One number system: the inventory's.** The recipe browser shows each input cell with the amount
   it takes as the inventory counts it (64 for a voxel item, 1 otherwise) and the output likewise
   (2 planks = 128). Recipes in `recipes.json` stay in units; only the display multiplies.
3. **Have / need.** The recipe view lists each distinct ingredient as "Oak Log 126/128", green when
   carried, red when short (inventory and bag).
4. **The grid says why.** A slot short of one block is tinted red and marked "/64". Under the output
   a line names the problem: "Each slot needs 64 Oak Log (one block)" or "Needs a Crafting Table".
   Tooltips of voxel items read "126 voxels = 1 block + 62".
5. The crafting table is shapeless (2 flint + 2 logs).

`open_inventory=1 craft_grid=flint:1,oak_log:10,oak_log:64,flint:1 browse=crafting_table` shows it
(prints the hint).

## Open: balance
Counted in voxels, several grid recipes make no sense as amounts: splitting a log by hand turns 64 log
voxels into 128 plank voxels (the saw: 256), and a chest (one 0.5 m block) eats 8 blocks (512 voxels) of
planks. A volume rule (a recipe takes roughly the voxels its result is made of, hand work loses some)
would fix that but changes most recipes; it is the developer's call.

## Consequences
- Old tests that pinned "a horizontal pair of planks is not sticks" and "the pickaxe needs its exact
  shape" now pin the opposite.
- New shaped recipes that share ingredients with another must be laid out exactly; `any_layout`
  tells which.
