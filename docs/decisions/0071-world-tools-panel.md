# ADR 0071: The world tools panel

Status: accepted (2026-10-26)

## Context
Bug report 2026-10-26_19-38-35 (creative, beach, brush "flat 25 cm"): "The mining and building system
seems broken now and is clunky overall. Make an easy to access midgame UI where you can select
building/mining size/pattern etc. So all advanced world interaction is accessible, visible and clear
at the touch of a button or hover."

What the player had to know by heart: B and Shift + B cycle five brush shapes, Ctrl + wheel steps six
sizes, Shift + right click paints (creative), holding V opens a ring of build modes whose labels say
nothing, R / Shift + R / X turn and mirror a placed shape, and `/brush`, `/shape`, `/box`, `/prefab`
do the same from chat. The only standing text was one HUD line right of the hotbar. Nothing explained
what "flat" or "column" does, which sizes a tool caps, or why a shape could not be started (no block in
hand).

## Decision
1. **One key, one panel.** G (and `/tools`) toggles `ToolsMenu` (`scripts/ui/tools_menu.gd`), a
   centred panel in the menus' style (`UiStyle`) over the running game with the mouse free, like the
   playtest menu on F4. Esc or G closes it; while it is up other keys stay out of the game, as the
   playtest menu does. It is wired in `Main` with the same overlay signals as the playtest menu.
2. **Every advanced interaction is a button with a tooltip.** Rows: brush shape (`Brush.SHAPES`), brush
   size (`Brush.CREATIVE_SIZES`, greyed outside the mode's sizes, the tooltip naming what the held tool
   digs), build mode (brush, the seven `ShapeBuilder` shapes, box fill, box dig, select; greyed with
   "hold a block to use it" when the mode needs a material), saved prefabs, and a key list. The
   current shape, size and mode show pressed; the panel refreshes every frame while open, so chat
   commands and keys change it live.
3. **Buttons call what the keys call** (`Player.set_brush`, `ShapeBuilder.start_shape`, `start_box`,
   `start_prefab`): the panel holds no logic of its own and never touches a voxel (principle 8).
   Picking a shape, box or prefab closes the panel, since the placement follows the aim.
4. **Data-driven**: the rows are built from the `Brush` and `ShapeBuilder` constants plus two tables
   of labels and tooltips in `ToolsMenu` (`SHAPE_TIPS`, `MODES`, `KEYS`). A new brush shape or
   builder shape needs one tooltip line.
5. **The ghost can be switched off.** The outline and cell shell of what a dig or placement will
   touch (ADR 0065) stay the ghost; a checkbox in the panel (`Settings.brush_ghost`, saved in
   `user://settings.cfg` with the controls) hides both. Asked for in the same report's follow-up.

Defaults taken without asking: the key is G (free; next to F and V), the panel is a flat list of
rows rather than tabs or a ring, Godot's own tooltips (no custom hover card). The HUD's brush line
now also names G. The Settings key list and `/help` (`/tools`) record the key.

## What was broken
A scripted replay at the report's seed and position (creative, `/brush flat 2` then `cube 4`, the
break button held) dug 669 sand and 46 dirt and walked down its own shaft: digging works. The
report's state says what the player did: dug 331 sand on a beach and placed it. The beach is
`sand_static`, a still powder; its drops are `sand`, the live powder, and the brush placed that:
every grain with a free diagonal below slid off, so a placed slab or cube spread into the scatter of
bumps in the screenshot. Building with what you dig was impossible.

Fix: `GameMaterials.still_of` maps a live powder to the still kind the generator lays
(`sand` -> `sand_static`, `gravel` -> `gravel_static`), applied where the held item's material is
resolved for placing, painting and shape building (`Player._use`, `ShapeBuilder._held_material`).
Placed powder keeps its shape until something disturbs it, as the ground does (the still kinds
loosen and erode through `loosen_chance` / `erode_chance`); the pick key already maps a still voxel
back to its loose item. Liquids are untouched. Test: `test_brush.gd`
`test_placed_powders_are_the_still_kind`.

## Consequences
- One place to learn the brush and the builder; the ring on V and the keys stay for players who
  know them.
- A hover tooltip is the only help text; a first-time tutorial is still open (ADR 0028's book could
  carry it).
- Check: `later=/tools;wait 1;/shot tools hud` in an autopilot run saves the panel;
  `test_tools_menu.gd` builds it headless and checks the buttons match the player's brush.
