# ADR 0099: The steam age, stage 2, the turbine, and the helper items

Status: accepted (2026-10-27)

## Context
ADR 0097 built stage 1 (the small boiler, pipes, the steam hammer and sieve). The developer asked for
"the rest of the steam age like you did here", then for two items per stage that directly help the
player (a miner's helmet that gives light, a jetpack that rises slowly and hovers, a traveler's backpack,
"some more you come up with"), intermediate parts for flavour, a steam-powered compressor that refills
the jetpack, a lenient dig brush, and the rule that **from the steam age on tools are made at the
crafting table: ground assembly is over**.

## Decision
1. **Fire bricks.** `fire_clay` (two clay balls and sand, shapeless) fires in the pit kiln into a
   `fire_brick`. Eight round a bronze pipe make the **blast furnace**.
2. **The blast furnace** (`BlastFurnaceEntity`, a `BloomeryEntity` with station `blast_furnace`): coal
   under it, an iron ingot in, lit; the charge only cooks while a boiler on its pipes blows it (it draws
   `steam_per_use` a second) and comes out a hot `steel_ingot` in 1200 ticks. Steel smiths on the bronze
   anvil or the steam hammer: `steel_plate`, `steel_pickaxe_head` (tier 4, the deep ores to come).
3. **The steel boiler** is the `BoilerEntity` with `item = "steel_boiler"` (`BlockEntities` keeps one
   kind): it builds pressure `steel_boiler_x` (2.5) times faster, and its **safety valve** blows off at
   `STEEL_VALVE` (2 bar) so it can be left running; rusted past half the valve sticks and it bursts at
   `boiler_burst_at` times its strength. The brighter drum, a valve on top, the hiss when it blows off.
   The boiler's `contents()` now drops its own item (stage 1 dropped a non-item "boiler").
4. **The ore crusher** (`OreCrusherEntity`, a `SteamSieveEntity`): loads blocks of ore (the ore item's
   `crushed` field names what it gives), crushes one per `ore_crusher_crush_s` into **two** crushed ore,
   each smelting like a block: the bloomery takes crushed iron, a crucible crushed copper, crushed tin
   alloys bronze. The 2x ore reward of ADR 0096.
5. **The water tank** (`TankEntity`, `tank_buckets` 16): water buckets in, an empty bucket draws one out,
   and `BoilerEntity.feed` lets every boiler whose pipes reach a tank drink a bucket a second while low.
6. **The steam turbine** (`TurbineEntity`): on the pipes it draws steam and spins at `turbine_rpm` times
   the pressure; nothing takes its power. `copper_wire` (four from a copper ingot on any anvil) is its
   coil and the first part of the electrical age.
7. **Helpers, two a stage** (intermediates: `iron_gear` smithed two from an ingot, `glass_lens` from glass
   and sand, `copper_coil` from four wires on a rod, `steam_cell` from steel plates round a gear and pipe):
   - Stage 1: the **miner's helmet** (head, `light` 12 m: `TorchLights` lights from the worn head piece when
     neither hand holds a light) and the **traveler's backpack** (`bag_slots` 18: `PlayerInventory.bag`
     is rebuilt to the worn bag's size, the inventory screen shows two rows).
   - Stage 2: the **steam drill** (tier 4 pickaxe, `speed` 2.5) and the **boiler suit** (body, warmth 6,
     armor 4, steam-proof).
   - Stage 3: the **steam jetpack** (a new clothing slot `back`; hold jump to climb `jetpack_rise_m`, release
     to sink `jetpack_sink_m`; the fall peak is reset so no landing hurts) flown on compressed air: its stack's
     `air` (0..1) drains over `jetpack_air_s` of climbing (a quarter as fast hovering) and the **compressor**
     (`CompressorEntity`, on the pipes, fills in `compressor_fill_s`) moves its air into a jetpack held
     against it. And the **surveyor's stake** (`waypoint` field): a right click plants a map waypoint.
8. **Tools at the table.** Tier 4 tools (steel pickaxe, steam drill) are shaped recipes; the grid-tool test
   lets tier 4 and up through.
9. **The brush.** Creative sizes go to 64 voxels; survival caps are hands 4, tools 6, iron and steel 8
   (the native fill above 8 stays creative-only).
10. `/kit steam` carries all of it; sheet kinds and sprites added (`make_item_sprites.py`).

## Consequences
- `test_steam_age_2.gd` covers the data, the furnace under and without steam, the valve and its rusty
  burst, the crusher, the tank, the turbine, the compressor charging a jetpack, saves.
- Not built: hoppers, chutes and conveyors, rails and the minecart (item transport and vehicles are
  projects of their own), pipes that climb, deep ores that need tier 4, the steam-age book chapter, and
  any in-game check of the jetpack, helmet light and backpack rows (unit tests only).
- The jetpack's air is a stack field, so it survives saves like a hot ingot's heat.
