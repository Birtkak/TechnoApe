# Core mechanics: TechnoApe vs Lay of the Land vs Vintage Story (2026-10-24)

Purpose: put every core system side by side, from how the world is simulated to the art style,
with pros and cons, so the developer can pick the new direction. Nothing here is decided.

Sources: Lay of the Land (LotL): Steam store page, all 76 Steam announcements and patch notes
(2025-01 to 1.3.4, 2026-10-22), 20 Steam reviews, a beginner guide, a building walkthrough video,
devlog frames; see also `2026-10-24-lay-of-the-land-building-tools-fluids.md`. Vintage Story (VS):
vintagestory.at, the official wiki (first day, smithing, world generation, temporal stability,
mechanical power, chisel, farming, ore deposits, temperature, combat, water), forum threads and
review round-ups. TechnoApe: `vision.md`, ADRs 0001-0036, the code. Marked *(est.)* where a fact is
my estimate rather than a source.

## At a glance

| | Lay of the Land | Vintage Story | TechnoApe today |
|---|---|---|---|
| What it is | Physics-first voxel action RPG: explore, fight bosses, loot, build | Slow, realistic survival sandbox with deep crafting and lore | Voxel sandbox: Noita materials + GregTech crafting + LotL world (vision) |
| Engine | Unreal Engine 5 (5.8), solo developer | Own C# engine (OpenTK), small studio, mod API | Godot 4.7 + godot_voxel + our C++ (FineGen, sim) |
| Grid | small voxels, ~10 cm *(est.)* | 1 m blocks; chisel to 16³ microblocks | 12.5 cm voxels; 0.5 m block for stations/recipes |
| Released | 1.0 April 2026, 79 % positive (~1.1k reviews) | Since 2016, mature, "mostly very positive" | Phase 2 prototype |
| Price | €20 | ~€20 *(est.)* | — |

## 1. Simulation: how the world behaves

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Powders | sand, gravel, snow fall and pile; burying hurts (1.1.17) | sand and gravel fall like Minecraft | Noita kernel: sand, gravel, dirt walls slump (ADR 0013) |
| Liquids | Minecraft-style source + flowing water; flowing water dies with its source; colours tell them apart | Minecraft-style: flows 6 blocks, sources form from 3, salt and fresh water | Noita kernel in a 64 x 32 x 64 m bubble: mass-conserving water with levels; static water outside |
| Fire | spreads and burns foliage, smoke; fire and acid go out in water | campfire/forge fuel; wildfire limited *(est.)* | cellular fire, smoke, burnables per material (ADR 0010) |
| Gas | pockets of gas suffocate; poison gas bombs; gas mask | — | gases rise (kernel); no breathing yet |
| Other reactions | acid residue, frost residue, explosions (Detonate spell, gunpowder barrels) | none to speak of | data-driven reactions (acid, lava, toxic sludge, blood stains: ADR 0015) |
| Collapse | trees fall as **voxel physics objects** (rigid bodies) that crush; levitate rips chunks loose; bosses fracture voxels. Builds do not topple ("can't knock over huge towers") | **cave-ins**: unsupported rock collapses a few layers; wooden beams shore tunnels | ceilings crumble into powder cell by cell (ADR 0013); rigid falling pieces not built ("structural collapse" is on the plan) |

**LotL** pros: the world fights back, emergent kills (trees, cave roofs), it reads as physical.
Cons: physics produce bugs and exploits (objects falling through the world, levitating bosses out
of arenas, soft locks), simulation was its main performance pain point for months.
**VS** pros: predictable, cheap, cave-ins add real mining tension. Cons: static; nothing surprising.
**Ours** pros: the deepest material sim of the three (true Noita kernel, mass-conserving liquids,
reactions, stains), deterministic and budgeted. Cons: only inside the bubble; no rigid-body pieces,
so a felled tree or a cut pillar cannot fall as one piece yet.

Choices for us: (a) keep the Noita kernel as the heart and add rigid voxel pieces for trees and
cut structures (LotL's best moment: "the tree fell on me and I died"); (b) add VS cave-ins and
shoring as a mining rule on top; (c) a cheaper source/flow model for rivers outside the bubble.

## 2. World and generation

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Size | finite world, generated up front (40 s - 2 min, 1.1.15) *(est.: finite)* | ~1,000,000 km² | infinite, streamed, 2 km LOD horizons |
| Landforms | "layered simulations": water carves channels, roads and bridges wind between locations | landforms + realistic climate by latitude and altitude | continents, plains / hills / mountain ranges, hydrology, waterfalls (ADR 0035) |
| Biomes | ~7 hand-tuned biomes (meadow forest, mystical forest, outback canyons, mushroom swamp, snowy mountains, volcano...), each with its **own cave biome** and boss | no discrete biomes: climate, rock and forest density blend | 13 biome ids from biomes.json, latitude climate |
| Geology | ore in caves per biome (copper, silver, titanium...) | **22 rock types** in strata; ores only in host rocks; disc deposits; surface nuggets; prospecting pick | GregTech ore veins with indicator stones; strata bands |
| Places | ~6 ruin types per biome with loot and enemies at the doors | ruins with loot, traders, story locations | ruins (walls and rubble) |
| Roads | generated roads and bridges | trader paths *(est.)* | none (planned) |

LotL pros: every view is composed (hand-designed feel), places to go. Cons: reviewers call it empty
and repetitive ("same 3 or 4 buildings", caves "explored in 2 to 5 minutes"), the world runs out.
VS pros: geology makes prospecting a real skill; climate drives everything. Cons: big and quiet.
Ours pros: the scale and horizons of both, deterministic. Cons: few places to go yet; no roads,
no structures worth visiting, no cave biomes.

## 3. Art style and rendering

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Voxel look | small voxels, each one texel of a **patterned** material texture (bricks, planks, bark read as patterns across voxels) | 1 m blocks with 32 px textures *(est.)* and modelled shapes (JSON shapes) | small voxels, each one colour of a **shuffled palette** (no pattern), ADR 0036 |
| Light | UE5: global illumination, volumetric clouds, fog, hardware ray tracing option, DLSS/FSR, warm grade | soft block light, sun shadows, bloom, god rays *(est.)* | Godot Forward+: AO, sky, sun; look pass pending |
| Water | clear turquoise, bed visible, deep teal | Minecraft-like translucent water | depth-tinted turquoise (2026-10-24) |
| Seasons and weather | dynamic weather (storms, willy-willies), time of day | **seasons** change foliage, snow accumulates, hail, local weather | real calendar and sky, seasons tint leaves (ADR 0025); weather minimal |
| Creatures and items | modelled animals and bosses, item meshes | modelled creatures, item shapes | micro-voxel models at 6.25 cm, pixel-art item icons |

LotL pros: the most praised thing in every review ("gorgeous", "stunning"). Cons: "identifying
materials can be tiring on the eyes... you hunt for subtle colour variations at the pixel level";
16 GB RAM minimum, memory crashes at launch. VS pros: instantly readable blocks, runs on anything.
Cons: looks like Minecraft. Ours: the LotL look at a fraction of the cost (4 GB RAM budget), but the
same readability risk, which we just hit with the pickups: small things vanish into palettes.

Choices: (a) keep shuffled palettes (soft, natural); (b) **patterned voxel textures** like LotL
(bricks, planks and stone courses readable at a glance); (c) palettes for terrain, patterns for
built materials. Plus the lighting grade (taste call already on the plan).

## 4. Digging, tools and building

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Digging | tools dig blobs; dig modes normal / box / cube; detail mode for finer voxels; tool tiers remove more voxels | one block per break, tool tiers and speed | brush 1-8 voxels, cube / ball / cylinder (ADR 0034) |
| Tools | durability, break (soft locks reported) | durability, quenching, tiers | tool tiers flint to steel planned; durability is in items.json and the browser, but tools never wear |
| Building | **shapes with gizmos** (box, cylinder, cone, walls), hollow ring, rotate and flip, click away to commit; **prefabs**, custom prefabs with categories; terrain sculpting; draw paths onto the ground | place blocks; **chisel** any block into 16³ microblocks, mix two materials, rotate, flip | place with the brush |
| Furniture | many sets, useables (campfire, kiln, anvil, smeltery, stove) | many functional blocks | stations, furniture, pots (ADR 0007, 0022) |

LotL pros: building is its strongest system after the looks ("level of control is very high,
prefabs help a lot"); materials are not locked behind progression. Cons: gizmos are fiddly for
some; box modes needed patching. VS pros: chisel gives unlimited detail with 1 m simplicity. Cons:
slow. Ours: simple and fast, but no shapes, prefabs or precision tools.

## 5. Crafting and progression

| | LotL | VS | TechnoApe |
|---|---|---|---|
| How you craft | **physical**: lay sticks, rope and flint on the ground and assemble; put food in a campfire; hit an anvil with a hammer; a recipe guide lists recipes | a 3x3 grid plus in-world crafts: **knapping**, **clay forming**, **casting**, **smithing** (voxel work piece, heavy hit / upset / split) | Minecraft grid + JEI browser, plus VS knapping, clay forming, pit kiln, firepit, casting (ADR 0023, 0030) |
| Metal | melt ore or scrap in a smeltery, pour into clay moulds (metal counted in units) | crucible casting, alloys (bronze ratios), bloomery iron, cementation steel, quenching | casting copper (ADR 0023), iron/steel planned (ADR 0019) |
| Tiers | flint → copper → ... → silver → titanium; gems upgrade weapons | stone → copper → bronze → iron → steel, slow | flint age → copper built; bronze, iron, steel planned; GregTech depth planned |
| Structure | biome loop: gather → caves for ore → craft gear → beat the biome boss (sigil) → upgrade smeltery → next biome | open: handbook, lore chapters, traders, classes | progression book with quests (ADR 0028) |
| Automation | none | **mechanical power**: windmills, axles, gears, quern, helve hammer, pulverizer | GregTech machines planned (Phase 3-4) |

LotL pros: crafting "is unique"; the world is the workbench. Cons: "not enough to craft", "confusing,
the crafting interface too annoying", little explanation, a thin loop repeated four times.
VS pros: the depth everyone praises, every step a small skill game. Cons: grind, steep start.
Ours pros: Minecraft familiarity + JEI clarity + VS hand crafts, data-driven. Cons: the grid and
the world crafts are two styles side by side; the long tail (GregTech) is still paper.

## 6. Survival pressure

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Food | cooking gives timed buffs (campfire 10 min, stove 30 min); composting | satiety + **nutrition groups**, cooking, **spoilage**, cellars | hunger bar, cooking, stew (ADR 0030) |
| Climate | weather hazards | **body temperature**, wetness, clothing warmth, rooms | none (open question in vision) |
| Threat clock | monsters in structures, bosses | **temporal stability** and **temporal storms** every 10-15 days | night and bears |
| Death | soft locks reported (stuck in holes, broken tools) | respawn at spawn, keep the world | respawn under the loading screen |

VS's temporal storms are its most criticised system ("a day-wasting annoyance"); temperature and
spoilage are praised as what makes shelter and farms matter.

## 7. Combat, creatures and magic

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Combat | melee, bows (elemental arrows), throwables, shields, armour, artifacts; AI strafes and dodges since 1.1 | melee, spears, bows, timing-based aim, armour tiers | spear stab and throw, bears bite (ADR 0027, 0030) |
| Enemies | ~4 per biome + 1 summoned boss per biome | drifters (temporal), wolves, bears, locusts, deeper monsters | bears |
| Magic | **wands with levels**, spells found and learned (Lightning, Acid Ball, Frost Ray, Shield, Detonate, Leap, Levitate, Summon), runes | none | Noita wand building planned (vision: wands are found, spells crafted) |
| Animals | many per biome, horses (tame, name, ride, jump) | husbandry, breeding, elk riding | sheep, pigs, chickens, bears, ocean life; breeding |

LotL pros: spells that use the physics (levitate a chunk of world and throw it; detonate a wall).
Cons: "combat is VERY basic", levitate breaks progression and bosses. VS cons: combat is its other
weak spot. Ours: combat barely exists; the Noita wand system would be our differentiator.

## 8. Interface and feel

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Inventory | small inventory (criticised), stacks 50 (150 plants) | slots + bags/baskets | Minecraft exactly, bag slot (ADR 0005, 0030) |
| Interaction | radial menus, physical interaction with useables, hammer icon on the crosshair | right-click blocks, handbook | Minecraft screens + world work spots |
| Help | tutorial tasks, recipe guide with search | the handbook (searchable, everything) | JEI browser + quest book |
| Controller | full support, split screen | partial | keyboard and mouse |

## 9. Multiplayer, tech, modding

| | LotL | VS | TechnoApe |
|---|---|---|---|
| Multiplayer | split screen and Remote Play only (1.3, 2026-10); its absence was a top complaint | dedicated servers, land claims, reinforcement | dedicated server on Unraid planned (Phase 4) |
| Modding | custom prefabs only | **core of the game**: mod API, JSON content, mod database | data-driven JSON, no mod API yet |
| Performance | 16 GB RAM min, launch memory crashes, sim and world creation were the pain points | low requirements | budget: p99 ≤ 8.3 ms, RAM ≤ 4 GB, VRAM ≤ 6 GB, loads 12-17 s |

## What players say

- **LotL praised:** visuals, the physics ("Minecraft and Terraria had a Teardown baby and named it
  3D Noita"), building control and prefabs, unique crafting.
- **LotL criticised:** thin content and an empty, repetitive world; basic, clunky combat; tools
  breaking too fast and a small inventory; soft locks; physics exploits; hard-to-read materials;
  confusing crafting UI; no online co-op.
- **VS praised:** depth and realism of crafting and survival, slow-burn progression, geology and
  prospecting, modding, performance, world settings to tune difficulty.
- **VS criticised:** temporal storms, combat, early grind and learning curve.

The pattern: LotL won on **feel** (looks, physics, building) and lost on **depth**; VS won on
**depth** and loses on **feel**. Our vision already aims at both: the LotL world with VS/GregTech
depth and Noita systems. The risk is doing both halves at LotL's breadth and none deeply.

## Decided (2026-10-24, one by one)

1. **Simulation core: A.** Keep the Noita kernel as the pillar and add rigid voxel pieces: an
   unsupported group of voxels (felled trunk, cut pillar, undermined ledge) breaks off as one body,
   falls, crushes, settles back into voxels. Trees and cut pieces first; whole buildings may topple
   too (later milestone). Rivers outside the bubble stay static for now.
2. **Building: A, unlocked from the start.** LotL shape building: radial menu of shapes (box,
   wall, cylinder, cone, sphere, stairs/slope), a preview on the ground with drag handles (height,
   size, hollow, move), rotate and mirror, click away to commit and take the voxels from the
   inventory; box mode for digging and building; prefabs (the last build, saved selections in
   categories). The brush stays as "normal" mode. No progression gate on building.
3. **Crafting style: B.** Physical for tools and gear: knapping, assembling head + stick + binding
   on the ground or a workbench surface, the anvil, moulds, firepit, pottery, hides. The grid stays
   for bulk and simple recipes (planks, sticks, torches). Machines get their own screens later. The
   recipe browser shows for every recipe whether it is a grid or a world craft and how to do it.
4. **Progression spine: A.** Ages (flint, copper, bronze, iron, steel) into GregTech machines,
   power and automation; exploration gates materials (regional tin, deeper rock for better ore,
   dungeons for spell ingredients). No boss loop as the spine.
5. **Survival pressure: A, not adjustable.** Hunger plus VS body temperature (winter, altitude,
   night, rain and swimming chill; fires, enclosed rooms and clothing warm; freezing hurts,
   overheating does not) and food spoilage (storage pots, cellars and cooking slow it). No
   nutrition groups. One rule set for every world, no world setting to turn it off.
6. **Threat: A; combat a side system for now.** Wildlife that bites (packs of wolves, boars,
   bears, predators per biome with reasons to attack) and hostile creatures in places: Noita-like
   dungeons and deep caves (monsters, traps, gas, lava, collapsing floors), guarded ruins. The
   surface stays mostly calm; no world clock, no night monsters. Dungeons hold wands and spell
   ingredients. Combat is a side system you can mostly avoid, not a pillar (revisit later).
7. **Magic: D, later.** Stays in Phase 3; the details (Noita wand building vs fixed spells, how it
   meets the tech tree) are decided when we get there. The vision's Noita wand idea stands until then.
8. **Art: A.** Nature (stone, dirt, grass, sand, leaves, ore) keeps shuffled palettes; crafted
   building materials get patterns across the voxels (plank rows and seams, brick courses, cobble,
   bark grooves and log rings), one colour per voxel from a tile repeating every few voxels. Then a
   sheet of three lighting grades for the developer to pick from.
9. **Places: C.** Generated structures per biome (ruined towers, cottages, keeps, shrines, mines,
   camps) built from voxels with loot and sometimes guardians, with variety (modular rooms,
   procedural layouts) so they do not repeat; Noita-like dungeons; a cave biome per region; roads
   and bridges routed between places; plus traders (at road crossings or camps) to buy and sell.
10. **Multiplayer: A.** Co-op ready, dedicated server in Phase 4. Every new system keeps its game
    state in a server-side layer; the client draws it and sends inputs.

## Decisions for the developer

Each line is a direction call; the first option is what this research suggests.

1. **Simulation core:** keep the Noita kernel as the pillar; add rigid voxel pieces (felled trees,
   cut structures) / or stay cellular only.
2. **Building:** LotL shape + gizmo building and prefabs, the brush as "normal" mode / brush only /
   VS chisel-style detail editing.
3. **Crafting style:** one style. Physical in-world crafting everywhere (LotL ground crafting + VS
   knapping, smithing on an anvil) with JEI as the guide / keep the Minecraft grid plus world crafts
   / grid only.
4. **Progression spine:** VS ages (flint, copper, bronze, iron, steel) into GregTech machines /
   LotL biome-and-boss loop / both (bosses gate biomes, ages gate tech).
5. **Survival pressure:** hunger (have) + VS temperature and spoilage / hunger only / buffs like LotL.
6. **Threat:** creatures and dungeons (LotL) / a world clock like temporal storms (VS) / none.
7. **Magic:** Noita wand building (vision) / LotL found spells with wand levels / later.
8. **Art:** palettes for terrain + patterned textures for built materials / palettes only /
   patterns everywhere; then the lighting grade.
9. **Places:** generated structures with loot and roads (LotL) / VS-style ruins and traders / later.
10. **Multiplayer timing:** keep "co-op ready, later" / pull the dedicated server forward (LotL's
    biggest complaint was co-op).
