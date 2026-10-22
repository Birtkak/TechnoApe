# 0008 – JEI-style recipe browser

Status: accepted (2026-10-22)

## Context
vision.md asks for a built-in Too Many Items / JEI browser that reads the same
data files as crafting, so it can never drift. The registries already had
`makes`, `uses_of` and `search`.

## Decision
- `RecipeBrowser` (scripts/ui/recipe_browser.gd) is pure logic: item list in
  items.json order, search where every term must match id or name and `#tag`
  matches a tag prefix, paging that wraps, recipes / uses views grouped by
  category (Crafting, Smelting, Spellcraft, then other stations), a history
  stack, per-recipe layouts and tag members for cycling.
- Categories come from recipe type + station; a station item's uses include its
  whole category (JEI catalysts: crafting table, furnace).
- `RecipeBrowserView` draws the item list right of every inventory screen (hidden
  when narrower than two columns; R/U still work) and the recipe view over the
  panel; the item list stays clickable above it.
- Controls follow JEI: LMB recipes, RMB uses, R/U over any slot or cell,
  Backspace back, Esc closes the recipe view first, wheel turns pages, tag
  ingredients cycle once a second, names show as tooltips.
- Lambdas that capture `self` are no longer connected to a child's signals
  (PlayerInventory, ChestEntity, BlockEntities): they formed reference cycles
  that leaked at exit; method callables do not.

## Consequences / revisit
- No cheat mode (click to give items) yet; it belongs with a creative/debug mode.
- No bookmarks, no recipe transfer ("+" fills the grid) yet.
- Non-block items still show placeholder tag icons.
- Debug: `open_inventory=1 browse=<item>[:uses] search=<text>`; `hover_test=1`
  drives hover + R + RMB through the real input path and prints the result.
