# ADR 0055: Minecraft controls, the offhand, and inventory moves from the screen

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

## Consequences
- Muscle memory from Minecraft works; Shift no longer sprints.
- Work pieces can be fed and worked with two things at hand, without switching the hotbar.
- `test_offhand_trash.gd` covers the moves; `craft_test=1` checks coal on a firepit, the ghost-free
  break, clay from a block, building onto a seashell and the offhand hammer.
