# 0075 – The inventory screens

- **Status:** accepted
- **Date:** 2026-11-12

## Context
With slots and piles (ADR 0074) the ape needs a screen to move things, and creative needs every block at
hand to test with (bug report `bugs/2026-11-12_20-58-44`: "a creative menu so u can acces all blocks").
items.md "The screen" asks for Minecraft's mouse habits, since they are free to reuse.

## Options considered
1. A menu page of rows, like the settings (ADR 0023). Already built, but a list of 37 slots and a few hundred
   blocks reads badly as rows, and it cannot show a lifted stack.
2. **Its own screen of square slots drawn with the HUD's rectangles** (`crates/client/src/inventory.rs`),
   reusing the glass, the brackets and the hotbar's block cubes. Chosen.

## Decision
- **Tab opens and closes it** (`Act::Inventory`, rebindable, `acts=` `inventory`); Esc closes it too, and
  so does its key on the search tab unless that key types a letter. Tab still completes a command while the
  chat line is open (the chat takes the keys first). It is a `Menu` (the world runs on, the pointer is free)
  with no buttons of its own. Q and E roll in space, as before.
- **Survival**: the panel holds the hotbar's 9, the worn pack's slot, the pack's rows as far as it opens them,
  and the pile nearest the feet (within 4 m) beside them. Click lifts a stack and puts it down (onto another
  item: they swap); right click lifts half, or puts one; shift-click sends a stack across (hotbar to rows,
  rows to hotbar, a pile into the ape); a click outside the panel with a stack lifted throws it.
- **Creative**: the hotbar and every block and item in a grid, 9 by 5, scrolled by the wheel, under tabs:
  nature, rock, ore, made, lamps, items and a search (typed while its tab is open). The tabs are derived
  (lamps: blocks with light; ore: the minerals' blocks; rock: the rock layers; made: `data/items.toml`'s
  `made` list; items: the non-block items; nature: the rest). A click takes a stack, right click one,
  shift-click a stack into the hotbar; a stack put onto the grid is gone.
- **Middle click on a block in the world** (`Act::Pick`, creative only): the hotbar slot that holds it is
  taken, else a stack of it goes into the slot in hand.
- **Slots show the block's cube** as the hotbar does; an item shows the block it looks like with a cyan mark;
  the name of the stack under the pointer is shown beside it. The hotbar now shows its slots as they lie:
  an empty slot is empty, and a slot holding a pack holds nothing to place.
- **Prediction**: the client applies a move to its own slots with the server's rule at once and keeps the
  server's `Inventory` back until every move it sent is answered; a pile's change comes with the server's
  answer (a tick).
- **Piles in the world** are drawn as still debris cubes, a quarter metre, one a stack, among the effects'
  particles (one more `FxGpu` a stack, no new pipeline).
- Off-screen: `acts=` `inventory`, `point:x:y`, `lclick`, `rclick`, `sclick`, `type:` and `back` drive it.

## Consequences
- The F10 row "64 of each block" became "creative's inventory" (it laid piles of overflow everywhere).
- `hud::RECTS_MAX` doubled to 32,768: the creative screen's 54 cubes are about 11,000 rectangles.
- **Left for later** (each a `shortcut:` comment): a drag that spreads a stack over slots, the number keys
  sending to a hotbar slot, predicted piles, the pack's rows drawn as that pack, containers beside the slots.

## Calls for the developer to confirm
- Decided 2026-11-13: the inventory opens with **Tab** (first built on E with roll right moved to R); Q and E
  roll again. `settings.txt` is now version 2: a version 1 file that still holds the built defaults (the
  inventory on E, roll right on R) moves to the new ones, a key the player chose stays (test
  `the_inventory_is_on_tab_and_an_old_file_moves_to_it`, red without the move).
- The tabs and what falls in each; a 9 by 5 grid.
- A stack put back into creative's grid is gone.
- The hint lines under the panel.
