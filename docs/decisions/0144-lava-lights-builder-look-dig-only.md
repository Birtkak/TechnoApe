# ADR 0144: Lava lights things, fire and boiling are counted for sound, the builder's look, dig only

Status: accepted (2026-11-07)

## Context
Six bug reports of 2026-11-07 and a Lay of the Land building guide as the reference for the look.
Fire and steam had sound loops, but they found their cells with 600 random samples in a 257^3 box: 31 000
burning cells gave one hit, so a big fire whispered and a hiss was silent. Lava never lit anything, and a
burning cell beside lava went out as if it touched water. A 1 M voxel ghost cost 700 ms per drag step in
`ShapeTool.cells` on the main thread, and in aim mode a mere move rebuilt it.

## Decision
1. **Flowing lava lights what it touches** (`SimWorld::hot_liquid_ignite`, a liquid with `heat_c` >= 500) at
   the burnable's `ignite_chance`, and stays awake until nothing beside it is left to light. A hot liquid no
   longer douses. Generated still lava lights nothing: every lake would burn its shore as it streams in.
2. **The sim counts what sampling cannot find:** `get_falls(4)` burning cells, `get_falls(5)` cells that just
   became a gas (boiled or a reaction's product), with their mean position. `sounds.json` loops take
   `"where": "burning" | "boiling"`; a loop's `"burst"` plays over it, paced by its cooldown.
3. **Ghost:** a depth pass (`brush_ghost_depth.gdshader`) then a colour pass, so only the surface nearest the eye
   shows, for the brush and the builder alike. It is the held material itself, see-through (`GHOST_OPACITY` 0.8) and
   barely tinted (`GHOST_TINT` 0.1), red when material is short; blue at 0.3 opacity hid what was being placed. Frames and
   prefabs stay a plain colour. Depth at priority 0 and colour at 1 keep water behind a ghost drawn.
4. **A big shape previews coarse while it changes** (at most `PREVIEW_CELLS` cells, stretched to its box); a move
   only moves the mesh. After 0.15 s of rest a worker builds the full mesh and its bill.
5. **Handles after Lay of the Land** (`GizmoMesh`): chunky white blocks drawn over everything, an arrow with a fat
   head up, a four-way cross at the foot's centre, rings of square pixels; the one under the cursor turns cyan.
   Replaces ADR 0143's point 5 (tori, x-ray pass, colours).
6. **The hint is a key list at the bottom right**, one fact or key per line, and its left-click line names what
   the handle under the cursor does.
7. **The rings replace the G panel (ADR 0071 retired, `tools_menu.gd` gone):** small tiles spread wide over the running
   game, `BuildIcon` line icons with their names underneath, a pointer at the centre. G or V opens the build ring, C or
   its Brush tile the brush ring (the wheel sizes it). A tap keeps a ring up until a click, a hold picks on release.
   Dropped with the panel: the "Now" lines and the tooltips. The ghost toggle moved to Settings.
8. **Dig only:** Shift + middle click toggles the aimed material on a list the brush, big brushes and box dig
   keep to; aimed at nothing it clears. Shown beside the hotbar, `/digonly`. It stays out of the
   way: no mode, no menu, and clicks elsewhere do nothing while it is on. Not saved with the world.
9. **Armour:** helmet, chestplate and greaves sprites (metal had the hood and the cloak). On the body the brim
   sits above ears and goggles, the arms step out a cell under body armour so sleeves clear the vest, and boots
   widen only outward so they no longer overlap. Plate pieces get a ridge, bands and pauldrons.

10. **The brush and a build mode are never on together:** a build mode hides the brush's ghost, outline and HUD line and
   ignores its keys (B, Ctrl + wheel); a brush choice on a ring leaves the build mode. A click that closes a ring
   digs and places nothing (`Player.click_swallowed`).
11. **Fire, steam and smoke look like it:** fire cells glow in fire's colours (`flame` in `gas_looks.json`, the gas shader)
   instead of drawing as pale lit cubes, and live 14 ticks (was 24), so flames stay near their fuel; slowing them instead let them hop water. The gas noise is
   about a voxel across, so a lone cell is a wisp. Steam condensed on a canopy or ceiling as a full cell of water per
   steam cell, columns of tall teal blocks raining over every boiling pool: now one steam cell in eight makes a one-eighth
   drop and the rest vanish, and it no longer condenses on a liquid, where one drop turned the column above it to water.

## Consequences
- Lava next to placed wood in a generated pool does not light it; only flowing lava does. Worth revisiting if a
  playtest misses it (a placed block wakes still lava for one tick only).
- The coarse ghost is blocky for 0.15 s after each change; the bill of a big shape lags as long.
