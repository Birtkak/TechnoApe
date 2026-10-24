# ADR 0040: Hunger, body temperature and food spoilage

Status: accepted (2026-10-24)

## Context
ADR 0037 decision 5: hunger plus Vintage Story's body temperature and food spoilage, no nutrition
groups, the same rules in every world. None of it exists yet: food only heals ("no hunger yet").
The world already knows what it needs: a climate temperature per column (-1..1, latitude and altitude
in it, `FineGen.column_info`), a calendar with seasons and 20-minute days, fires and lit entities
(`is_lit()` on firepits, furnaces, torches, kilns), the sim bubble's cells for shelter.

## Decision
All numbers are tunables in `Settings` (seconds of real time, game days, degrees C).
1. **Hunger** (0..20, Minecraft's bar) with a **saturation** buffer that drains first. It drains one
   point per 40 s, faster when sprinting or swimming (x2.5) and when cold (x1.5). Eating works whenever
   the bar is not full: food gives its `hunger` points and `saturation`; stale food half. Full (17+)
   regenerates one health per 6 s; empty hurts one health per 8 s **down to 1** (hunger alone does not
   kill: a default the developer may change).
2. **Environment temperature** where the player stands: climate (-1..1 to -14..38 C) plus the season
   (+-7 C over the year), the hour (-4 C at night, +3 C at noon); 4 m and more under the surface
   blends to 10 C (caves, cellars); water is 8 C colder. **Shelter:** a native flood fill through open
   cells from the player's head (`ClimateProbe`, capped at a 16 x 8 x 16 m box and 64k cells): if it
   closes before the cap, the player is in a **room**, which pulls the temperature halfway to 18 C.
   **Heat:** fire and lava cells and lit entities (firepit, furnace, kiln, torch) add heat falling off
   with distance, 1.5x inside a room, 0.7x outside.
3. **Body temperature** (starts 37 C) eases toward a target over ~90 s: 37 inside the comfort band
   (10..30 C without clothes; clothing will widen it), lower below it (a quarter degree per degree of
   cold), higher above (a fifth per degree of heat). Below 35.5 C the player is cold (hunger x1.5),
   below 34 slowed (x0.85), below 33 freezing (one health per 6 s). Above 38.5 hot, above 39.5
   heatstroke (one health per 8 s).
4. **Spoilage:** food items get `spoil_days` (game days at 16 C). A perishable stack carries `spoil`
   (0 fresh .. 1 rotten) through every inventory move; merging stacks averages it by count (Vintage
   Story does the same). A clock ages every perishable stack in the player's inventory and in
   containers, faster when warm and slower when cold (rate = temperature / 16 C, 0.1..2), half in a
   storage pot, and in containers 3 m or more under the surface at 8 C (a cellar). Past 0.5 it is
   stale, at 1 it becomes **rot**.
5. Creative mode keeps the readings but never hurts or slows; the same rules in every survival world.
6. Server-ready: `Survival` holds the state and the rules (snapshot, save with the player); the HUD
   only draws it.

## Consequences
- Every place that builds a stack dictionary must keep `spoil` (Inventory, saves, item entities);
  tests cover moves and merges.
- Clothing, wetness from rain and per-room temperature memory (a heated house staying warm) are left
  for later; the comfort band is where clothing will plug in.
- Rot is an item (compost later).
