# ADR 0121: Things to make and use (overnight content pass)

Status: accepted (2026-11-02). Follows ADR 0112 (per-wood planks, saplings).

## Context
The developer asked for a big leap in content across the board. This lane covers things to make and use,
pre-electric or decorative: food and cooking, tool and armour tiers, furniture and storage. Material ids
216..227 and atlas tiles 180..191 are this lane's; items need no material id, so food, tools and armour are items.

## Decisions
- **Metal tiers.** Steel knife, axe, shovel, hoe, hammer and saw: smithed steel heads (iron's shapes on a
  steel ingot) put together at the crafting table like the steel pickaxe (ADR 0099); tier 4, 1.6x iron's uses.
  Helmet, chestplate and boots in bronze, iron and steel with copper's patterns and rising armour
  (bronze 2/6/2, iron 3/7/3, steel 4/8/3). Bronze, iron and steel spears hit 5/6/7 and throw 9/11/13 (flint 4/7); like every metal-age tool their heads are smithed (a leaf blade on one ingot), then hafted: bronze and iron on the ground with two sticks, steel at the crafting table.
  Written by `game/tools/content_tiers.py`. Eight bronze ingots now make a chestplate (layout-free crafting,
  ADR 0046), so the bronze anvil test checks nine for "nothing".
- **Meals with buffs.** The firepit cooking pot (stew's path) makes porridge (wheat, einkorn or emmer x3),
  fish soup (raw fish x2), mushroom soup (brown mushrooms x3) and berry compote (wild strawberries or
  blueberries x4); a pot serves three bowls. A food's `buff` {kind, amount, s} is kept by `Survival.buffs`
  (saved): warm / cool shift the felt temperature, regen heals one health every `amount` s, haste speeds the
  walk. The same buff again keeps the longer time; it never stacks. Porridge warms 6 C for 5 min, stew 4 C
  for 4 min, fish soup +15 % speed for 2 min, mushroom soup heals every 5 s for 1 min, compote cools 6 C
  for 5 min. Eating shows the buff as a toast; `/survival` and the snapshot carry `buffs`.
  Written by `game/tools/content_meals.py`.
- **Saplings in the world.** Checked in game (`later=` with `/sapling ... grow`, saplings 6 m apart): oak,
  birch and acacia grew, a spruce next to a monolith pillar waited. The trunk's lowest 0.5 m may grow into soil
  (slopes); `/sapling grow` says why the last sapling is blocked. Scripted `later=` runs print each command's
  reply on stdout.
- **Furniture and storage (batch 2).** Door, trapdoor, fence, fence gate, table, bench, shelf and sign in each
  of spruce, acacia, jungle and birch: `<wood>_<kind>` is the oak model with its planks repainted
  (`FurnitureEntity.WOODED`, 8 new paints), made from that wood's planks; the oak pieces now take oak planks.
  New: a two-seat bench, a tallow candle (lights like a lantern), red and blue wool rugs (dyed with a
  strawberry or a cornflower) and a straw mat, flat with one voxel of collision. Signs keep a line of text
  (60 characters, saved): a right click opens chat with `/sign `, the look-at panel shows it. The barrel
  (material 216, a StationEntity model) is an 18-slot ChestEntity between the storage pot and the chest.
  No stairs or slabs per wood: the shape builder cuts any block. `/furniture <kind>` places one for checks.
  Written by `game/tools/content_furniture.py` and `content_rugs.py`.
- **Crops.** Potatoes, onions and beans grow on farmland, each its own seed; wild ones drop from tall grass
  (1.5-2 %). A baked potato (firepit or furnace) and a vegetable soup (any of the four vegetables x3, warms
  like porridge). Written by `game/tools/content_crops.py`.

## Consequences
- The new crops borrow the carrot's growth looks; their own looks need material ids from 217..227 and
  atlas tiles 180..191 (both still free).
- Buffs are one per kind; a status HUD row for them is left to the HUD owner (the toast and `/survival` show them).
