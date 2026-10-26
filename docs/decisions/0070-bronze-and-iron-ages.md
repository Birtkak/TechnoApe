# ADR 0070: The bronze and iron ages

Status: accepted (2026-10-26)

## Context
The copper age was in the game (ADR 0023, 0042): copper melted in a crucible on the firepit, cast in
clay molds or smithed on a stone anvil. Past it the game still had Minecraft's opening: the furnace
smelted copper, tin and iron ore straight into ingots, bronze was a shapeless furnace "recipe", and the
iron pickaxe and hoe were grid crafts. ADR 0019 and `progression.md` plan copper, bronze, iron with a
bloomery and a hammer. The developer asked to make copper to iron a part of the game now and chose
(2026-10-26):
- bronze: **tin laid into molten copper** on the firepit (over a mixed crucible by ratio, or a grid blend);
- the bloomery: **one station worked by hand**, like the firepit (over a built multi-block structure);
- heat: **a forge for iron**, the firepit stays copper and bronze only, and **the furnace loses ore smelting**.

## Decision
1. **Bronze.** `melt` recipes with the vessel `crucible_copper`: 2 tin nuggets, a tin-bearing stone, a
   block of tin ore or a tin ingot turn it into `crucible_bronze` (15 s). A block of copper ore now melts
   in a crucible like 4 copper-bearing stones. Molten bronze casts in every copper mold and the ingot
   mold; bronze ingots are heatable and smith on the stone anvil. Bronze tools (knife, axe, shovel, hoe,
   pickaxe, hammer, saw) are tier 2; iron ore already wants tier 2, so the bronze pickaxe opens iron.
2. **Smelt recipes take batches and can hand out hot output**: `count` units of input per craft, and
   `hot: true` gives the output a fresh `hot_until`. The firepit's station, fuel (`burns`) and heat
   (`heats`) became overridable, so the forge and the bloomery are `FirepitEntity` subclasses and keep
   its hand use, held right clicks, saves and view hooks.
3. **The bloomery** (8 fired clay bricks, a solid 0.5 m work block drawn as a 75 cm clay shaft): smelt
   recipes with station `bloomery` turn 8 iron nuggets, 4 iron-bearing stones or a block of iron ore
   into one hot `iron_bloom` in 2400 ticks (2 min), burning charcoal or coal only. It holds one bloom;
   an empty hand (a held right click) takes it out, popped clear of the shaft.
4. **The forge** (8 loose stones round a coal, flat like the firepit) burns charcoal or coal and heats
   any heatable item; it holds no vessel and cooks nothing. Items tagged `forge_heat` (iron ingots,
   blooms, iron work pieces) do not go on a firepit ("needs a forge's heat").
5. **Anvil tiers.** `AnvilEntity` has a kind: the stone anvil (tier 0) and the bronze anvil (tier 1,
   7 bronze ingots on a crafting table). Metal whose item has `anvil_tier` above the anvil's (iron) is
   refused, as a hot ingot and as a work piece. The bloom smiths into an iron ingot (`smith_iron_ingot`,
   a two-high bar across the anvil); iron ingots smith into the seven iron heads (two new shapes: shovel,
   hoe). Iron is never cast. Iron tools are tier 3 and assembled on the ground like the others.
6. **Removed:** the furnace's `smelt_iron`, `smelt_copper`, `smelt_tin` and `bronze_alloy`; the grid
   iron pickaxe and hoe (same ids, now `assemble`). The furnace keeps glass, stone, charcoal and food.
   `tin_ingot` stays for old saves and melts into copper like tin.
7. **Book and tools:** Bronze Age and Iron Age chapters follow the Copper Age (quest ids append only);
   `/kit bronze`, `/kit iron`; the recipe browser shows batch counts ("8 x Iron Nugget, 120.0 s").

## Consequences
- `test_metal_ages.gd` covers the data, the alloy, both new stations, anvil tiers, beating a bloom into
  an ingot and saves. `metal_age_test=1` (with `walk=1 speed=0 mob_spawn=0`) runs it all through the
  player's clicks and prints `debug metal {...}`; its hammer blows look almost straight down, because
  from eye height a tall middle hides the bar's low ends (aim at the tall columns first in play too).
- Iron takes real grinding as asked: an iron pickaxe head is 2 ingots = 2 blooms = 16 sifted nuggets
  (about 400 gravel sifts at 4 %) or 8 iron-bearing stones, and a bronze anvil costs 7 molten crucibles.
- Not built: steel and the blast furnace, metal anvils from casting, slag, quality, tongs (hot metal
  still does not burn). Numbers (durabilities, 2 min bloom, tin per crucible) are first guesses.
