# Electrical age: what the book should ask (for the recipe-browser session)

Written 2026-11-02 by the machines session (ADR 0113, 0117). The quest graph is yours; this is what the
age needs the player to find, in order. An `electrical_age` chapter with the first nine of these already
sits in `quests.json` (added before the fence was drawn; keep, move or redraw it as you like).

Chain, each a quest with `{"type": "item", "item": ...}`:
1. `rubber` (from `turbine`): birch tar + sulfur, cured in a furnace.
2. `insulated_wire` and `coated_board` (from rubber).
3. `vacuum_tube` (from insulated wire), `machine_hull` (from insulated wire).
4. `primitive_circuit` (tube + board). The hard step: say "make them in batches".
5. `electric_motor` (hull; first screws sawn by hand from a rod).
6. **final** `first_machine`: item `lathe` (or bending press / wiremill: any machine). Title "The First Machine".
7. optional `electric_furnace`.
Then the second half (ADR 0117), after `first_machine`:
8. `macerator` (ore x3 instead of the crusher's x2), `alloy_smelter` (bronze and steel without the blast
   furnace), `assembler` (circuits two at a time, motors from fewer parts).
9. `battery_box` (power at night), `electric_lamp` (optional, light without fire).
10. `pump` (water without buckets; keeps the simulation awake around it), `coke_oven` + `coal_coke` (the first
    multiblock: a controller plus a hollow 3 x 3 x 3 of bricks, no power; say "26 bricks round a hole").
11. Hoppers and belts feed machines and take from them (ADR 0100 transport works with every machine): a quest
    "feed a machine from a chest" on `hopper` fits here, optional.

Icons: the item ids above all have sprites. Keep LV the end of the tree for now (no MV).
