# 0143 – Block windows apart from the inventory, the recipe viewer docked right

- **Status:** accepted
- **Date:** 2026-11-17

## Context
Using a workbench or a crucible (ADR 0140, 0141) opened the whole inventory screen with only the crafting grid
swapped for the block's slots, and the developer mistook the crucible's window for the inventory. Their call
(2026-11-17): "when interacting with any entity make their UI a different window then my inventory. Also make
the JEI alike system we have sit on the right on the screen with an opague background".

## Options considered
1. Keep one screen, swap more of it per block – little code, but the inventory and a block still look alike.
2. Three surfaces, Minecraft's container layout – what the developer asked for; the client only, no wire change.

## Decision
Option 2, in `crates/client/src/inventory.rs` (`Screen`, `layout`, `top`):
- **The inventory** (Tab): the ape, the Pack slot with health and hunger, the pack's rows, the hotbar, On the
  ground, and the 2 x 2 under the pile. Never a furnace's slots.
- **A block's window** (workbench, crucible): smaller, titled by the block's name, the block's own part on top
  and the player's pack rows and hotbar under it; no ape, pile or Pack slot. The click rules, shift-click across
  and the server's Close rules (ADR 0140) are unchanged; the server and the wire did not change.
- **The recipe viewer**: its own opaque panel docked at the right edge, all the screen's height, shown with the
  inventory and any block's window (creative's grid has none). The window is centred in the room left of it;
  at 1280 x 720 and 1920 x 1080 neither is drawn smaller nor overlaps. Fill fills the open window's grid; at a
  crucible there is no grid and Fill is not shown, while furnace works are still listed.

A new block window kind adds a `Screen` variant, its arm in `top` (its slots and boxes, its width and height)
and its drawing in `App::top_draw`; the pack rows, hotbar, viewer and click rules come with the frame.

## Consequences
The layout test covers the five screens at four UI sizes: slots apart and inside the window, each found by a
click on its middle, the viewer docked and apart. The look (colours, spacing) is left for a visual pass. The
viewer shows 5 x 6 items; the wheel scrolls it as before.
