# ADR 0087: Hot metal is its own state; tongs, a two-ingot forge, heat you can read

Status: accepted (2026-10-27)

## Context

Bug reports of 2026-10-27:

- 18-04-24: one hot ingot merged into a stack of cold ones and made them all hot (ADR 0042 averaged
  `hot_until` on merge). Asked for: hot and cold apart, a cool-down back to cold, and hot metal in a
  bare hand dropped with damage, so it must be held with tongs.
- 18-02-35: the forge took a whole stack of iron ingots and drew one sprite.
- 18-06-58 (second half): the heat system works, but it should show: colours that change with heat,
  and degrees Celsius in the tooltip and the look-at panel.
- 18-01-05: "shift click the workbench puts the max amount in". The player was looking at the
  bloomery (look-at: "On the fire: 50 Iron Nugget"), which loads one item per right click.

## Decision

1. **Hot is a stack state that never merges with cold.** No separate item ids (every heatable metal,
   work piece, recipe and anvil path would need a twin): `hot_until` stays the per-stack timer, and
   `Inventory.same_kind(s, id, data)` (same id, both hot or both cold) guards every merge: `add`,
   `put`, `right_click`, the player's merge/sort/move passes and ground items merging. Hot with hot
   still averages. When `hot_until` passes the stack is cold and stacks with cold ones again.
2. **Tongs.** New item `tongs` (tag `tongs`, durability 400, green wood: 2 sticks + cordage,
   shapeless) so the copper age stays reachable. Every 0.5 s a hot stack held or in the offhand
   without tongs in the other hand falls at the player's feet and deals `Settings.hot_hand_damage`
   (2; damage only where the game mode takes damage). Hot item entities are picked up only with
   tongs in a hand. The bag and main grid may carry hot metal (only hands burn).
3. **The forge holds `Settings.forge_input_max` (2) items** (`FirepitEntity.input_max`, the forge
   overrides it) and draws each one lying side by side on the rim over the coals.
4. **Heat reads.** `Inventory.temp_c` maps heat 0..1 to 20..`Settings.metal_hot_c` (1200) degrees C;
   the tooltip says "Hot: 728 °C (24 s to cool)", the firepit/forge and anvil look-at lines show
   degrees instead of percent. `Inventory.glow_color` gives none below `Settings.glow_from_heat`
   (~500 C), then dull red, orange, yellow, yellow-white. Icons draw the sprite again modulated by it
   (above 1, so it brightens); laid items (forge, firepit) and dropped items get
   `BlockMeshes.heat_material` (the sprite material with emission, 8 steps, cached).
5. **Sneak + right click on a firepit, forge or bloomery loads as much of the held stack as fits**
   (fuel up to `firepit_fuel_max`, the input up to `input_max`). The crafting-table workbench
   already has Fill with Shift (ADR of 2026-10-25) and is unchanged.

## Consequences

- `craft_test` and `metal_age_test` now give the player tongs in the offhand for the hot steps.
- Not done: the first-person held item does not glow yet (the viewmodel dresses by id, not stack);
  the anvil's piece keeps its GLOW/MOLTEN paints. The heat material is a new pipeline variant
  (emission) compiled on first sight of hot metal; add it to `MaterialWarmup` if that hitch shows.
- Check: `test_crafting_in_world.gd` (hot/cold, degrees and glow, tongs), `test_metal_ages.gd`
  (two-ingot forge, sneak loading).
