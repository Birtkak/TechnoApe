# ADR 0130: Quality of life: toggles, inventory gestures, status at a glance

Status: accepted (2026-11-02)

## Context
The developer asked for "more quality of life features" of the kind a Minecraft/GregTech modpack
player expects. Much already exists (ADR 0055: sort, quick-stack, auto-refill, minimap, waypoints,
graves, key rebinding; pick block, wheel moves, chat history and Tab completion, F2 screenshots,
grid Fill / Clear, the Jade-style look-at panel, durability tooltips). This ADR adds what was missing.
Each feature whose taste differs has its own switch in Settings (`user://settings.cfg`).

## Decision
1. **Toggle sprint, toggle sneak** (Settings > Controls, off by default): Ctrl / Shift flip a latch
   instead of being held, with a toast. `Player.sprint_held()` and `Player.sneaking()` read it.
2. **Alt + wheel turns the inventory rows** into the hotbar like a drum
   (`PlayerInventory.cycle_rows`); a column holding a favourite slot stays put.
3. **Double click** in the inventory gathers the cursor's item from every slot, the open container
   first (`collect_to_cursor`), favourites keep theirs; **Shift + double click** shift-clicks every stack
   of the clicked item in that slot group (`shift_all`). Vanilla behaviour, no switch.
4. **Status at a glance** (Settings > Interface, a new tab):
   - *Hurt, cold and heat effects* (on by default): a red screen edge for 0.6 s after a hit, a frost
     edge while the body is below `cold_c` (strongest at `freeze_c`), an orange one above `hot_c`;
     the lost hearts blink white, hearts shake at 2 hearts or less and the food row shakes at 3 or less
     (vanilla). `GameHud.edge_color()`.
   - *Coordinates and compass* (off by default): "X Y Z m  NE 45°" top left; hidden while F3 shows its own.
   - "Saving world..." bottom left for a moment when the 30 s autosave runs (no switch).

## Consequences
- `test/test_qol_controls.gd` covers the toggles, rows, gestures, the coordinates text and the edge;
  `menu_test=1` adds `qol_hud` (coordinates, edge alpha after a hit) and `interface_tab`, shots
  `hud_qol` and `settings_interface`.
- The Interface tab is where later HUD switches go.
