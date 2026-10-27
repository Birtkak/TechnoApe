# ADR 0083: Station ghosts, a chest lid that opens, and a firepit laid on the ground

Date: 2026-10-27. Status: accepted.

## Context

Three bug reports from one survival playtest (2026-10-27_15-31-39, 15-31-56, 15-33-14): holding a chest showed
the brush's 25 cm cube, not where the 1 m chest would go; chests had no open/close animation; and the firepit
was a crafting-grid recipe while flint tools are put together on the ground (ADR 0042). The 1 m chest also grew
off one corner of the aimed 0.5 m block, the player checked only a 0.5 m cube before Main checked the whole
footprint (an item spent, then "No room"), and any pebble or flower in the footprint refused it.

The four 2026-10-26 reports in the same batch were already handled: 19-38-35 by the world tools panel (ADR 0071)
and still-powder placing (0648d8d); 20-25-14, 20-26-24 and 20-26-39 by ADR 0073 (placed sand falls, the
brush is flush and centred, one box of full-detail voxels).

## Decision

1. **Station ghost.** Holding a station item (crafting table, chest, furnace, storage pot) shows its own model,
   see-through (`StationEntity.ghost_mesh`, one shared unshaded alpha material, warmed under the loading
   screen) plus the footprint outline where a click puts it: blue when it fits, red when not. Aiming at
   something a right click opens hides it (the click opens, places nothing). `Player.station_spot` is the one
   rule for the ghost and the placement: the footprint centred on the aimed 0.5 m block, standing on the
   aimed face, lifted up to one voxel over bumpy ground.
2. **Clutter gives way.** `PlantStamps.is_clearable_cell`: a free cell or passable ground clutter (pebbles,
   flowers, grass). Stations and a built firepit flatten it; Main's station check uses the same rule.
3. **Chest lid.** A chest view is a body mesh and a lid mesh on a hinge node (`chest_body`, `chest_lid`, lid
   from `CHEST_LID_Y`). Opening it swings the lid about 70 degrees in 0.25 s, hinged on the side away from the
   player (chests have no facing); closing the screen swings it shut.
4. **Firepit laid in the world.** The firepit is an `assemble` recipe: right click the ground with a loose stone
   (any `#rock`), lay three more in a ring, then two sticks crossed inside it. The last stick turns the parts
   into a firepit where they lie (`WorkSystem._build_in_place`, centred on them; its bottom layer may cut
   into plain ground, a dug-in pit). With no room it drops the firepit item as before. The Hearth quest
   completes on the `assembled` event. The firepit item stays (kits, breaking a firepit gives it back).
5. **`/use [aim|close]`** right-clicks with the held item, shows the aim ghost under autopilot, or closes the
   open screen, so scripted runs can film placement; `/shot` now also hides the inventory screen.

## Consequences

- A rock right-clicked on a top face now starts a firepit (it did nothing before).
- A station can flatten a flower whose head stood above its footprint, which then floats; rare at 75 cm.
- Checks: `test_crafting_in_world.gd` (firepit laid stone by stone), `test_quests.gd`, `test_survival_loop.gd`;
  evidence `game/out/placement/placement_sheet.png` and `chest_strip.png` (beach target, `/use`, `/cam`).
