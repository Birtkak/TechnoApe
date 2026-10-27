# ADR 0093: Information systems: a picture and mining line in the look-at panel, R / U everywhere, station tabs, Fill on the furnace, recipe bookmarks

Status: accepted (2026-10-27)

## Context

Bug report 2026-10-27_18-08-02 asked for an overhaul of "what am I looking at", the recipe browser
(JEI) and the work stations. The developer's answers to the scope questions:

- Look-at panel: a small picture of the block or item next to the text (insisted on), temperature in
  °C, mining info (hardness, tool needed, what it drops), and what a station holds (contents,
  progress, heat).
- Recipe browser: R (recipes) / U (uses) on any hovered item anywhere; station tabs showing which
  station makes it; a click on a recipe fills its ingredients into the open station from the
  inventory; bookmarks of recipes at the side of the screen, saved.

Much of this already existed: the panel had the tool line with a tick or cross and "Drops nothing
with this" (ADR 0078), firepit, forge, bloomery and anvil contents with °C (ADR 0087); R / U / A worked
over any slot of the inventory screen, station screens and the browser (ADR 0008); Fill filled the
crafting grid; item bookmarks were saved in `user://bookmarks.json`; Pin put a recipe's checklist on
the HUD, saved with the player (ADR 0061). What was missing is below.

## Decision

1. **Picture.** The look-at panel draws an `ItemIcon` left of its text: the block's item, or for a
   block without one its material's cube (`ItemIcon` takes a `"mat"` key), or a work piece's station
   item (the forge, the anvil; flint for knapping, clay for clay forming). No new art: item sprites
   and block tiles as everywhere else (ADR 0084).
2. **Mining line** from `data/mining.json`: "Hardness 3.0 · drops Iron Ore" (no drops part when the
   entry has none: crops and stations drop their own way). It shows in creative too.
3. **Contents of chests and furnaces** join the work stations': a chest's name, "Holds 12 Coal, 3 Iron
   Ore and 2 more" (counts in items, not voxels), a furnace's "Burning, smelting 40%". °C stays the
   ADR 0087 lines (hot metal on a fire or anvil); nothing else in the world has a temperature yet.
4. **R / U in the world.** With no screen open, R / U open the inventory on the recipes / uses of the
   aimed block's item (a station: the station), else the held item (`RecipeBrowser.lookup`); a toast
   when neither has any. Not while the shape builder owns the keys (its R turns the shape).
5. **Station tabs.** The recipe view's category row is a row of tabs, one per category, each the
   station's (catalyst's) icon; a click opens it, the open one is framed, `<` `>` page when there are
   more than 7. Smelt recipes split by station: "smelting" is the furnace's, the firepit, forge and
   bloomery each get their own tab (`station:<id>`), so the tab says where it is made.
6. **Fill on the furnace.** On a furnace's screen Fill puts a smelt recipe's input from the inventory
   into the input slot: one item, Shift as much as the slot takes (`PlayerInventory.fill_container`;
   a hot stack never merges with a cold one). The crafting table's grid Fill is unchanged. Forges and
   firepits are worked by hand and have no screen to fill.
7. **Recipe bookmarks.** The recipe view's Mark button bookmarks the shown recipe (`recipe:<id>` in
   the same `user://bookmarks.json`); it lists with the item bookmarks at the left of the inventory,
   with a gold corner, and a click opens that recipe. Recipes of `recipes.json` only (the mining
   pseudo-recipes are added after the bookmarks load). Pin (HUD checklist) stays as it is.

## Consequences

- `test_recipe_browser.gd` (lookup, show_recipe and tabs, recipe bookmarks persist) and
  `test_grid_fill.gd` (furnace Fill) cover the logic; `info_test=1` films the look-at panel on ore and on
  a loaded forge, R in the world, U with station tabs and the bookmarks into one sheet `info.png`
  (`scripts/ui/info_test.gd`, JSON line "debug info").
- The furnace's uses (its catalyst category) no longer include firepit cooking.
- Not built: stains next to the character and an info line under the minimap (another change), and
  sneak-click fill on the work block (station code, another change).
