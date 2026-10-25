# ADR 0055: Minecraft controls, the offhand, inventory moves, maps, quality of life, graves

Status: accepted (2026-10-25)

## Context
Bug reports of 2026-10-25 (part D of `docs/plans/2026-10-25-bug-report-split.md`):
- 02-47-46: clay knapping placed the clay instead; "make Ctrl sprint and Shift a crouch mode like
  Minecraft, and start clay forming with a right click while crouching".
- 02-18-35: a seashell block could not be built onto a seashell: the right click tried to pick it up.
- 02-27-14: coal could not be laid on a firepit, and a broken firepit left its voxels as ghosts.
- 02-36-31: pick up items on the ground from a bit further.
- 02-56-13: drop items by clicking outside the inventory or with Q over a slot; a trash slot.
- 02-57-10: an offhand like Minecraft and Vintage Story, to make in-world crafting easier.
- 02-23-50: update the kits.
- Later requests the same night (plan items D10-D12): a minimap and a full map with waypoints, quality of
  life (auto-refill, durability warnings, a pickup feed, key remapping, chest sorting / quick-stack /
  labels / favourite slots), and death with stakes (a grave).

Shift used to sprint and to sneak at once (the `sprint` action), Ctrl flew down, F toggled flight.

## Decision
1. **Controls (Minecraft).** Ctrl sprints, Shift sneaks (`sneak` action) and flies down, F swaps the
   hands (`swap_hands`); flight toggles with a double jump only. Sneaking walks at
   `Settings.sneak_speed_m`, lowers the eyes, never walks off a ledge (`Player._hold_edge`), and is what
   `work_use` / `work_hit` / gadgets / stations read (`Player.sneaking()`), never the sprint key. Ctrl +
   wheel still sizes the brush and the laser (the wheel event carries Ctrl; sprinting needs a move key).
2. **Clay.** Shift + right click with a clay block on a top face starts clay forming; a plain click
   places the block. A clay form takes clay blocks as balls: a block's voxels make four balls
   (`WorkSystem._clay_per_ball`).
3. **Pickups vs building.** A held right click picks a ground piece up only with an empty hand, a tool
   or its own non-block item; an item that places a block always builds onto the piece.
4. **Firepit.** A short click that would act (laying fuel, taking off) says "hold right click" instead of
   doing nothing silently; the hold itself is unchanged (ADR 0043). Breaking any voxel of a work piece
   removes the whole piece, its view and its footprint (`Main._on_broken`; it used to clear only the
   aimed voxel, so a firepit broken off its anchor left ghost voxels and a ghost model).
5. **Pickup reach** 1.4 -> 2.2 m (`Settings.pickup_radius_m`).
6. **The offhand.** `PlayerInventory.off`, one slot of an `Inventory`, so per-stack data rides along.
   F swaps it with the held stack; the screen shows it beside the bag slot and the HUD left of the
   thermometer; the viewmodel holds it in the lower left. Anything fits. Who uses it:
   - an empty main hand places the offhand's block, plant or furniture (Minecraft: torches in the left
     hand), but not on stations, work pieces or pickups, which an empty hand works;
   - smithing: a hammer in either hand smiths (left click), and wears where it is;
   - knapping: a knapping stone in either hand;
   - the sieve refills from gravel or sand in the offhand while the right click is held;
   - a clay form takes the offhand's clay when the main hand is empty.
   Saved with the player (`"offhand"` in the player JSON).
7. **Screen moves.** A click on the dimmed screen outside the panel with a stack on the cursor throws
   it (left: all, right: one item or one block of a voxel item); Q over a slot drops one, Ctrl + Q the
   stack. A trash slot left of the hotbar row keeps the last stack thrown in until the next one replaces
   it (a click with an empty cursor takes it back). All of it goes through `Inventory.make_stack`, so
   freshness, wear, heat, work pieces and gadget data survive.
8. **Kits** count blocks for voxel items (they counted voxels: `"stone": 64` gave one block), and every
   kit lists today's items; new `fire` and `clutter` kits.

9. **Maps (plan item D10; developer: shaded relief, coloured pins with a beam, minimap top right, square,
   north up).** `WorldMap` keeps the explored world as 128 m tiles of 0.5 m pixels (RGBA8, saved per
   world as PNGs): alpha 0 unexplored, 255 the generated terrain's colour (`FineGen.map_image` on a
   worker thread, 1 m samples), else the real surface (material, liquid depth, height) from the native
   `SimWorld.surface_map` of the live window (part A adds it; until it is merged the map shows
   generated terrain only, so digs and builds appear once it is in). `map_relief.gdshader` colours
   each material by its atlas tile's average and shades the relief from the north-west; water darkens
   with depth. `MapView` draws tiles, pins and the player arrow; the HUD `Minimap` (corner and size in
   Settings, 10 redraws a second, voxel coordinates and the biome below it, moved down under F3's text)
   and `MapScreen` (M: drag, wheel zoom, Space centres, right click adds / renames / recolours / deletes
   waypoints). `Waypoints` (`main.waypoints.add(name, pos, kind)`) are saved with the player;
   `WaypointMarkers` raise a beam of the pin's colour with the name and distance above it.
10. **Quality of life (plan item D11).** Auto-refill (a hand that runs out or whose tool breaks takes the
    same item from the inventory), a warning sound and toast when a tool enters its last tenth (durability
    bars existed), a pickup feed ("+3 Flint", "Inventory full") bottom right, every gameplay key rebindable
    in Settings (saved under `[keys]` in user://settings.cfg; fixed keys listed below), chests with a
    Sort button, a Stack button (quick-stack into chests within `Settings.quick_stack_m` that already hold
    the item) and a name shown on the screen and when aimed at, and favourite slots (Alt + click, a gold
    corner) that sort, quick-stack, shift-click and refill leave alone.
11. **Graves (plan item D12; developer: nothing stays).** Dying in survival puts everything carried
    (inventory, offhand, bag and its contents, cursor) into a `GraveEntity` at the death spot (the first
    free block upward), with a "grave" waypoint and the coordinates in chat. It never despawns; a right
    click gives it all back (what does not fit pops out) and removes it and its waypoint; breaking it
    spills it. Creative keeps everything.

## Consequences
- Muscle memory from Minecraft works; Shift no longer sprints.
- Work pieces can be fed and worked with two things at hand, without switching the hotbar.
- `test_offhand_trash.gd` covers the moves; `craft_test=1` checks coal on a firepit, the ghost-free
  break, clay from a block, building onto a seashell and the offhand hammer.
