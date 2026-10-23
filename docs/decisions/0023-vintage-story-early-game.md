# 0023 – Vintage Story early game: knapping, clay forming, pit kiln, casting

- **Status:** accepted
- **Date:** 2026-10-23

## Context
ADR 0019 set the ladder (flint, copper, bronze, iron, steel) but left the
first steps as grid recipes. The developer sharpened it on 2026-10-23: "when
you spawn, get flint tools, then be able to break rocks but not get anything
out of them. You will need to make crucibles out of clay and heat them so you
can collect surface copper and make your first metal tools. For now the
early game will basically have Vintage Story progression and mechanics. When
we add or unlock electricity, that's where the game switches to GregTech-type
automatic machines that craft for you."

Taste calls the developer made the same day:
- Knapping and clay forming are **Vintage Story minigames in the world**:
  the work piece lies on the ground and you click its cells.
- Clay is fired in a **pit kiln**; a **firepit** cooks and melts the crucible.
- The GTNH **sieve stays** as a second route to nuggets beside surface copper.

## Options considered
1. **Grid recipes** (flint + stick = knife). Fast to build, no Vintage Story feel.
2. **A knapping screen** (a crafting-table-like UI with an 8×8 grid).
   Easy to read, but the piece isn't in the world.
3. **Work pieces in the world** on the ADR 0006 chisel grid (8 cells per
   voxel edge, 6.25 cm). Chosen by the developer. It reuses MicroMesher and the
   furniture pattern of ADR 0022: a block entity owns the state, and an
   invisible voxel takes the aim ray.

## Decision
Option 3. The rules, as built:

- **Work spots.** Two new materials, appended after the content pass:
  88 `work_spot` (invisible, only the aim ray hits it, you walk through it)
  and 89 `work_block` (invisible, solid). Both have `interact: "work"`. A block
  entity in `scripts/work/` owns each one and draws its own micro mesh.
- **Knapping** (`type: "knap"`, 8 rows of 8 characters, `#` = keep): right click
  the top of a solid voxel with flint. A flint slab appears and a recipe
  picker opens. The cells to chip off are drawn dark; left click chips one, and
  holding repeats. Only marked cells can be chipped, so a wrong click costs
  nothing. When the last one goes, the head drops. Breaking an unfinished
  piece gives the flint back.
- **Clay forming** (`type: "clayform"`, a list of 8×8 layers, bottom first):
  right click a top face with clay. The active layer's missing cells show as a
  ghost. Right click adds a 2×2 brush, clipped to the target and the active
  layer, and holding repeats. One clay ball fills 32 cells. The finished
  raw item drops.
- **Ground storage and the pit kiln** (`type: "kiln"`): right click a top face
  with an item tagged `ground_place` (raw clay ware, molds). It lies there,
  drawn from its clay recipe's cells. Add 4 dry grass, then 8 sticks, then
  light it with a firestarter. After `Settings.pit_kiln_burn_s` (60 s) the
  ware is fired.
- **Firepit** (furnace screen plus a vessel slot): it needs a firestarter to
  light, then burns its fuel like a furnace. Cooking uses `smelt` recipes with
  `station: "firepit"`. **Melting** (`type: "melt"`): a crucible in the vessel
  slot plus `count` ores in the input turns into a molten crucible.
- **Casting** (`type: "cast"`): right click a mold lying on the ground with a
  molten crucible. The crucible comes back empty (`returns`). After
  `Settings.cast_cool_s` (10 s), right click takes the casting, and the
  mold stays.
- **Sieve** (`type: "sift"`, `chance`; all sift recipes with the same input
  are one loot table): right click with gravel or sand to load it. Right
  click 8 more times to shake it, and the loot pops out on top.
- **Rock.** Stone-like materials get `drop_tier: 1`. Any pickaxe breaks them
  (the flint pickaxe at normal speed), but they drop nothing below copper.
  Logs need an axe (`requires`). The knife cuts tall grass into dry grass
  (drop entries may carry `tool`).
- **Tiers.** Flint 0, copper 1, bronze 2, iron 3, steel 4. Copper and tin
  ore need tier 1, iron ore 2, obsidian 3. The wooden and stone pickaxes and
  hoes are removed.

## Consequences
- No per-stack data: molten metal, heat and tool wear stay out for now.
  A molten crucible is its own item and never cools in the hand.
- New recipe types reach the browser (`layout` kinds `pattern`, `row`) and
  `ingredient_specs`, so "is everything craftable" and the uses lists cover them.
- Smithing (ADR 0019), bloomery iron and charcoal pits are the next ages.
  GregTech machines start at electricity, not before.
- Numbers (cells per clay ball, ores per crucible, burn and cool times, sift
  chances) are first guesses to tune in play.
