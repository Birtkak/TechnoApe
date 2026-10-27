# Progression – slow early game

Direction set by the developer on 2026-10-22: **"early game GregTech: New
Horizons meets Vintage Story."** The early game burns slowly. Players start
with flint, shave bark and sift gravel for nuggets, and copper comes before
iron. An iron pickaxe costs real grinding. Decision record: ADR 0019.

The working recipe list lives in the recipe book artifact (see
`game/data/recipes.json` for what is in the game today). Recipes there carry
an `age` and are marked `idea` until promoted; this document is the plan
they follow. Numbers below are first guesses to tune in play.

## Three eras (developer, 2026-10-27, ADR 0096)

| Era | Ages / stages | Feel |
|---|---|---|
| **Material age** | stone → copper → bronze → iron | Vintage Story: everything by hand, in the world, one piece at a time |
| **Steam age** | low pressure → high pressure → steam works | steampunk: machines you build slowly take the old chores off your hands |
| **Electrical age** | LV onward | GregTech: machines that craft for you, automation |

There is no mechanical power (water wheels, windmills): iron leads straight into steam.

## The ladder

| Age | Unlocked by | Tool tier | Tools | Key stations |
|---|---|---|---|---|
| Stone | flint picked up from the ground, knapped | 0 | flint knife, axe, shovel, hoe, pickaxe | 2×2 hand grid, crafting table (flint + logs), firepit, pit kiln |
| Copper | surface copper melted in a fired crucible and cast in clay molds | 1 | copper pickaxe, axe, shovel, knife, hoe | sieve, furnace |
| Bronze | tin laid into molten copper | 2 | bronze pickaxe, axe, shovel, hoe, knife, saw, hammer | bloomery, bronze anvil |
| Iron | iron blooms hammered into ingots | 3 | iron pickaxe, axe, shovel, hoe, knife, saw, hammer | forge |
| Steam: low pressure | iron plates + bronze pipe → small boiler | 3 | — | small boiler, steam forge hammer, steam sieve |
| Steam: high pressure | steel from the blast furnace | 4 | steel pickaxe, excavator | blast furnace, steel boiler, ore crusher, pipes, tanks |
| Steam works | belts and hoppers | 4 | steam drill | conveyors, rails, steam turbine |

The material age plays like **Vintage Story** (developer, 2026-10-23): work
by hand, in the world, one piece at a time. Mechanics and numbers as built:
ADR 0023, 0070. The steam age hands those chores to machines; the
electrical age is GregTech.

Spells run beside the ladder: mana crystals from crystal caves, crafted at
the spell altar. Wands stay found-only (vision.md).

## Stone age: the first hour (ADR 0023)

You spawn with nothing.

1. **Pick up** what lies around: flint on gravel and stony ground, loose
   stones, twigs (sticks) on forest floors, and copper-bearing stones above
   copper veins. Leaves drop a stick now and then.
2. **Knap flint** in the world: right click the top of a solid block with
   flint, pick a head (knife blade, axe, shovel, hoe, pickaxe), then chip off
   the marked cells. Head + stick in the hand grid = the tool.
3. Logs need an **axe**. Planks are 2 per log by hand. The **crafting table**
   is 2 flint + 2 logs.
4. The **flint knife** cuts tall grass into **dry grass**. A **firestarter**
   is 2 sticks + dry grass.
5. The **flint pickaxe** breaks rock, but **rock drops nothing** until
   copper. You can dig into a hill, but you cannot collect stone.
6. **Clay** lies in river beds, lake beds and marshes, and digging it gives
   clay balls. **Form clay** in the world the same way you knap: pick a
   crucible or a mold and fill the ghost cells layer by layer.
7. **Pit kiln:** put the raw clay ware on the ground, add 4 dry grass and
   8 sticks, and light it with the firestarter. A minute later it is fired.
8. **Firepit** (2 sticks + 2 loose stones): light it with the firestarter.
   It cooks food and melts metal.

## Copper age: surface copper

- Put a **crucible** in the firepit's vessel slot and **4 copper-bearing
  stones** in its input. When they melt, the crucible holds molten copper.
- **Cast:** right click a fired **mold** lying on the ground with the molten
  crucible. You get the empty crucible back. When the metal has cooled, take
  the **copper head** out (the mold stays) and put it on a stick.
- The copper pickaxe makes rock drop cobblestone, and it mines copper and tin
  ore (tier 1). The furnace needs that cobble.
- **Sieve** (crafting table: sticks, dry grass, planks) is the slow, safe
  route next to surface copper. Load gravel or sand and shake it. Gravel
  gives flint 25 %, a copper nugget 18 % and an iron nugget 4 %; sand gives
  a tin nugget 10 %. 8 copper nuggets melt like 4 copper-bearing stones.

## Bronze age

Built as ADR 0070 (2026-10-26); the ADR has the numbers.

- **Bronze:** lay 2 tin nuggets (or a tin-bearing stone, a block of tin ore) into a crucible of molten copper
  on a lit firepit. Molten bronze casts in every copper mold. The furnace smelts no ore.
- Bronze tools are tier 2. **Iron ore needs tier 2** (bronze pickaxe).
- **Bloomery** (8 fired clay bricks, worked by hand): 8 iron nuggets, 4 iron-bearing stones or a block of
  iron ore plus charcoal or coal make one hot **iron bloom** in 2 minutes.
- **Forge** (loose stones round a coal): the only fire that heats iron. On the **bronze anvil** a hammer
  beats the bloom into an iron ingot; the stone anvil refuses iron.

The grind the developer asked for: 2 iron ingots for a pickaxe head = 2 blooms =
16 iron nuggets ≈ 400 sifts of gravel at 4 %, unless the player reaches
iron ore with a bronze pickaxe first. Both routes are valid; ore is faster
once found, sifting is safe.

## Iron

- Iron tools are tier 3, smithed from iron ingots on the bronze anvil (never cast).

## Steam age (developer, 2026-10-27, ADR 0096; stage 1 built as ADR 0097, stage 2, the turbine and the helpers as ADR 0099)

A slowly improving steampunk era. Each stage needs parts from the one
before, and each gives back a chore the player has done by hand many times.
Stage 1 machines get a steel version in stage 2, so what you own keeps
getting better instead of being thrown away. Steam, water and fire are
simulated materials: a boiler can leak, overheat and flood.

**Boilers explode.** A boiler that runs dry while hot blows up: a blast
crater, scalding steam and fire, like the crater rod and lava bomb do. Keep
the water coming (by hand, then by pipe) or lose the room.

1. **Low pressure (iron and bronze).**
   - **Small coal boiler**: iron plates, bronze pipe, a firebox. Burns coal or
     charcoal, takes water poured in, gives a slow stream of steam.
   - **Steam forge hammer**: finishes a head shape the player has smithed by
     hand once; also makes **plates and rods**, the parts every steam machine
     is built from.
   - **Steam sieve**: runs the gravel and sand grind unattended.
2. **High pressure (steel).**
   - Fire clay → fire bricks → **blast furnace** (steam bellows):
     steel = iron + 2 coal. Steel pickaxe, tier 4: what deep ores require.
   - **High-pressure steel boiler**: 2–3× the steam, several machines at once.
   - **Ore crusher**: crushed ore smelts to **2× the metal per ore block**.
   - **Pipes and tanks** for water and steam: one boiler feeds a row of machines.
3. **Steam works.**
   - **Hoppers, chutes, conveyor belts**: ore in one end, ingots out the other.
   - **Steam drill / 3×3 steel excavator**, **rails and a steam minecart** for
     hauling from far veins.
   - Ends with a **steam turbine** and copper wire: the first generator, the
     door to the electrical age.

Built so far (ADR 0099, 0100): all of stage 2, the turbine and copper wire, the compressor, and the
stage 3 transport: hoppers and chutes, conveyor belts, rails and a rideable minecart (flat rails, the
rider carries the ore). **Helpers, two a stage** (developer,
2026-10-27): stage 1 the miner's helmet (light) and the traveler's backpack (18 bag slots); stage 2 the
steam drill (2.5x a steel pick) and the boiler suit; stage 3 the jetpack (compressed air from the
compressor; hold jump to climb, release to hover) and the surveyor's stake (a map waypoint). Intermediates
for flavour: iron gear, glass lens, copper coil, steam cell. **From the steam age on, tools are made at
the crafting table; ground assembly ends with the iron age.** The dig brush is lenient: creative to 64
voxels, survival hands 4, tools 6, iron and steel 8.

## The GregTech mindset (late steam age onward)

From steam works on, and fully in the electrical age, recipes follow
GregTech: **many intermediate products, and lots of each.** A machine is
built from plates, rods, gears, pipes, wire and circuits, each made by its
own machine from its own material. Every ore the world holds is needed for
something sooner or later, and in quantity.

What this asks of the player: **plan expeditions.** Find a vein with the
prospecting pick, go out equipped, and bring back as much of it as you can
carry, because it will all be used. Storage, hauling (minecart, rails) and
the 2× crusher are what make those trips pay. Recipe design rule: no ore is
a dead end, and mid-game parts should cost enough that a single small trip
never covers a machine.

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

## Systems this needs (not in the game yet; ADR 0023 built 2, 3, 5 in part and 6)

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
