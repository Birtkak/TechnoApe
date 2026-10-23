# Progression – slow early game

Direction set by the developer on 2026-10-22: **"early game GregTech: New
Horizons meets Vintage Story."** The early game burns slowly. Players start
with flint, shave bark and sift gravel for nuggets, and copper comes before
iron. An iron pickaxe costs real grinding. Decision record: ADR 0019.

The working recipe list lives in the recipe book artifact (see
`game/data/recipes.json` for what is in the game today). Recipes there carry
an `age` and are marked `idea` until promoted; this document is the plan
they follow. Numbers below are first guesses to tune in play.

## The ladder

| Age | Unlocked by | Tool tier | Tools | Key stations |
|---|---|---|---|---|
| Stone | bare hands | 0 | flint knife, hatchet, shovel, pickaxe; stone hammer | 2×2 hand grid, crafting table (flint + logs), campfire |
| Copper | sifted copper nuggets cast at the campfire | 1 | copper pickaxe, axe, shovel, knife, saw, chisel, prospecting pick | sieve, furnace |
| Bronze | copper + sifted or mined tin, alloyed in the furnace | 2 | bronze pickaxe, axe, shovel, saw, hammer | bloomery |
| Iron | iron blooms (from sifted nuggets or iron ore) hammered into ingots | 3 | iron pickaxe, axe, shovel, saw, hammer, knife | blast furnace |
| Steel | iron + coal in the blast furnace | 4 | steel pickaxe | (machines, later) |

Spells run beside the ladder: mana crystals from crystal caves, crafted at
the spell altar. Wands stay found-only (vision.md).

## Stone age: the first hour

1. Break **leaves** by hand: sticks drop (about 20 %).
2. Dig **gravel** by hand: flint drops (about 10 %), otherwise gravel.
   Pick up **loose stones** (the pebble detail voxels).
3. **Flint knife** = flint + stick (hand). Cut **tall grass** with it for
   **plant fiber**; 3 fiber = **twine**.
4. **Flint hatchet** = 2 flint + stick + twine. Logs can't be broken by hand,
   only with an axe.
5. **Shave bark** off a log with a knife or hatchet; 2 bark = twine.
   (Better later as an in-world action: use the tool on a log, it becomes a
   stripped log and drops bark.)
6. **Crafting table** = 2 flint + 2 logs. Planks: 2 per log by hand, 3 with
   a hatchet, 4 with a saw.
7. **Flint pickaxe** (table): mines stone and coal slowly, not ores.
   **Stone hammer** (loose stones) crushes cobble to gravel and gravel to sand.
8. **Campfire** fires clay: clay balls → raw crucible and raw ingot mold →
   fired at the campfire.

## Copper age: sifting

- **Sieve** (table: sticks, twine, planks). Sifting is a chance table:
  gravel gives flint 25 %, copper nugget 18 %, iron nugget 4 %;
  sand gives tin nugget 10 % (placer tin).
- **Copper ingot** = 7 copper nuggets + crucible + ingot mold at the
  campfire. The crucible and mold stay in the grid and lose durability.
  About 40 sifts per ingot.
- Copper tools. The copper pickaxe is the first that mines ore
  (copper and tin ore need tier 1).
- **Furnace** (cobble) smelts copper and tin ore, sand to glass, logs to
  charcoal, clay to bricks.

## Bronze age

- Bronze = 3 copper + 1 tin in the furnace (in the game today).
- Bronze tools. **Iron ore needs tier 2** (bronze pickaxe).
- **Bloomery** (clay bricks): iron nuggets ×4 + coal ×3, or iron ore + coal ×2,
  make an **iron bloom**. Furnaces no longer smelt iron.
- **Bronze hammer** beats a bloom into an iron ingot (tool stays in the grid).

The grind the developer asked for: 3 iron ingots for a pickaxe = 3 blooms =
12 iron nuggets ≈ 300 sifts of gravel at 4 %, unless the player reaches
iron ore with a bronze pickaxe first. Both routes are valid; ore is faster
once found, sifting is safe.

## Iron and steel

- Iron tools (iron pickaxe is in the game today, tier 2 → becomes tier 3).
- Fire clay → fire bricks → **blast furnace** (wraps a furnace).
  Steel = iron + 2 coal there. Steel pickaxe, tier 4: what deep ores require.
- Machines (GregTech side) start here; not designed yet.

## Smithing: Vintage Story style (developer, 2026-10-22)

Metal tools are not crafted from ingots in a grid. A tool is a **head** on
a stick, and the head is smithed:

1. Heat an ingot (or iron bloom) in a **forge** (loose stones around coal;
   copper also works hot off the campfire). Metal carries a temperature and
   cools in the hand and on the anvil.
2. Put it on an **anvil**: the stone anvil works copper and bronze, the
   bronze anvil every metal. Iron and steel need the bronze anvil.
3. Hit it with a hammer. The anvil shows the target shape on the 8×8 chisel
   grid (ADR 0006: one layer for now); each strike moves metal cells, and
   the work is done when the cells match. Too much metal must be chiselled off.
4. Fit the head on a stick by hand.

One piece of metal gives 24 cells. Heads: pickaxe, axe, shovel and hammer
cost 2 pieces; knife, saw, chisel and prospecting pick cost 1. An iron
bloom refined on the anvil gives one iron ingot. Shapes are recipe data
(`type: "smith"`, `input`, `input_count`, `shape`), drawn and edited in the
recipe book. The stone hammer can work copper, so there is no chicken and egg.

## Food, hunger and cooking (developer, 2026-10-22)

- **A hunger bar** (20 points, like Minecraft's 10 shanks) drains with time
  and work; at zero you lose health. Items carry
  `food: {hunger, saturation, spoils_days}`. Spoiling is a later step.
- First foods: **blueberries** and **cranberries** (bushes in worldgen later),
  **raw meat** (placeholder until animals; 30 % food poisoning when eaten raw),
  **cooked meat** (campfire or furnace).
- **Cooking pot** (fired clay) on a campfire makes meals that fill more than
  their parts: meat stew, berry compote. Meals come in **clay bowls**; the
  bowl comes back when you eat (`empties_to`).

## World and survival basics (developer, 2026-10-22)

- **Ground pickups, Vintage Story style:** loose stones, flint and sticks
  lie on the surface to get started. **Ore-bearing stones** (copper-, tin-,
  iron-bearing) lie above ore veins: picking them up and crushing them with
  a hammer gives nuggets, and they show where to dig. They spawn with the
  world (worldgen detail pass), not over time.
- **Bed:** a straw bed (plant fiber + sticks) lets you sleep at night to
  skip to morning and sets the respawn point. A wool bed comes with sheep.
- **Saplings:** leaves drop saplings (about 5 %); planted on dirt or grass
  they grow into a tree after a while, so forests can be replanted.
- **Sun and moon:** a visible sun and moon cross the sky with the day cycle
  (the moon with phases), lighting follows them.

## Systems this needs (not in the game yet)

1. **Tools that stay in the grid** (GregTech crafting tools): a recipe lists
   `keep` ingredients; crafting leaves them in place and costs 1 durability.
   Also buckets: `empties_to` should come back when a full bucket is used.
2. **`sift` recipe type**: `input`, `output`, `chance`, station `sieve`.
   All sift recipes with the same input form one loot table.
3. **Chance drops** in `mining.json`: leaves → stick, gravel → flint,
   tall grass with a knife → plant fiber, pebbles → loose stone.
4. **Mining changes**: logs require an axe; ore `min_tier` copper/tin 1,
   iron 2; tier speeds grow to 5 entries (flint … steel).
5. **New station blocks** (ADR 0007 pattern): campfire (smelt-like, slow,
   low heat), sieve, bloomery, blast furnace.
6. **Remove the Minecraft openers**: wooden and stone pickaxes, 4-plank
   crafting table, 4 planks per log by hand, iron straight from the furnace.
7. Item textures: the atlas is full (8×6), so new blocks need a bigger atlas.
8. **Smithing**: item temperature, forge, anvil block entities with a
   work grid, a `smith` recipe type, tool heads on sticks.
9. **Hunger and food**: hunger bar on the HUD, eating, saturation, meals with
   `empties_to`, food poisoning; spoiling later.
10. **Sleeping**: bed block, skip night when the player sleeps, respawn point.
11. **Saplings**: leaf drops, planting, growth into a generator-style tree.
12. **Sky**: sun and moon meshes or sky-shader discs following the day cycle.
13. **Ground pickups**: worldgen places loose stones, flint, sticks and
    ore-bearing stones (above veins) as pickable surface detail.

## Ores: GregTech-style veins (developer, 2026-10-22)

"Have ore generation be like GregTech as well." **Built 2026-10-23
(GEN_VERSION 9)**: veins, layers, depth bands, surface indicators, data in
`ore_veins.json`, `/locate <type>_vein`; numbers and measurements in
`world-generation.md`. Not built yet: small ores and the prospecting pick.
The design:

- **Veins on a grid.** One vein candidate per 96 m × 96 m cell (192 voxels),
  jittered, about half the cells filled. A vein is a flat lens about
  32 m across and 8 m thick (64 × 16 voxels). Finding one is an event; a
  vein holds hundreds of ore blocks, so one mine lasts a long time.
- **Mixed veins, not single ores.** Each vein type has a primary ore (top
  layers), a secondary ore (bottom layers), a between ore (middle) and a
  sporadic ore (scattered throughout), like GregTech's vein layers.
- **Depth bands and biome bias.** Each vein type has a y range (voxels,
  sea level = 256) and a weight, optionally per biome group.
- **Small ores** scattered between veins: single blocks that drop 1–2
  nuggets, never full ore. A third nugget route beside sifting and veins.
- **Surface indicators.** Loose ore rocks (pebble-style detail voxels)
  above a vein's cell hint at what lies below; picking one up gives a
  nugget. The copper prospecting pick reads vein types within a radius.
- **Data, not code** (principle 6): veins live in `game/data/ore_veins.json`
  with a schema and tests, read by the native generator like `biomes.json`.

First vein table with today's ores (more ores need new materials and tiles):

| Vein | y range (voxels) | Primary | Secondary | Between | Sporadic | Weight |
|---|---|---|---|---|---|---|
| Coal | 200 – 320 | coal | coal | stone gaps | iron | 30 |
| Copper | 160 – 280 | copper | copper | tin | iron | 25 |
| Cassiterite (tin) | 120 – 240 | tin | tin | copper | coal | 15 |
| Iron | 40 – 180 | iron | iron | coal | copper | 20 |
| Mixed copper-iron | 80 – 200 | iron | copper | copper | tin | 10 |

Copper and tin sit higher than iron, so the ladder follows depth: shallow
veins in the copper age, deep iron veins with a bronze pickaxe.
Generator cost: one vein lookup per column (3×3 cells), cheap next to the
caves; ores stay LOD 0 only, as today.
