# 0030 – A deeper Flint Age: cordage, hunting, leather, pottery, shelter, light, charcoal

- **Status:** accepted
- **Date:** 2026-10-23

## Context
ADR 0023 made the road from flint to copper; ADR 0028 put it in a quest
book. Asked what more the Flint Age could hold, Claude proposed eight steps
(cordage, a spear and hunting, hide working, panning, clay vessels,
shelter, torches, a charcoal pit) and the developer answered "all of them",
adding mid-build: "make the charcoal pit an in-world mechanic like Vintage
Story".

## Options considered
For most steps there was one natural Vintage Story / TerraFirmaCraft
shape that reuses ADR 0023's pieces; the choices worth recording:
1. **Charcoal pit as a recipe** (logs in, charcoal out) vs **in the world**.
   The developer chose the world: firewood piles you seal with soil.
2. **More inventory**: a bigger fixed inventory, a bag item that is itself
   a container, or a **worn bag that adds a row** (Vintage Story's backpack).
   The worn bag: it keeps ADR 0005's 36-slot model and every test of it.
3. **Hides**: a crafting step, or **work on the ground** like the pit kiln.
   On the ground, in the same `GroundEntity` as clay ware.
4. **Cordage required** for every flint tool (TerraFirmaCraft-like friction)
   vs only some. All flint heads need a `#binding`; cast copper heads have a
   socket and do not.

## Decision
- **Fiber and cordage.** Tall grass (30 %), ferns (25 %) and reeds (1-2) drop
  plant fiber by hand. Three fibers twist into cordage. Every flint tool is
  head + stick + `#binding` (cordage or a leather strip).
- **Spear and hunting.** Knapped spear head + two sticks + binding. Left
  click stabs (`attack_damage` 4); right click throws it (`throw`: 7 damage,
  16 m/s): `Projectiles` flies it with gravity, sweeps animals and voxels
  each physics frame, and drops it as an item where it stops. Sheep (70 %)
  and pigs (50 %) drop raw hide.
- **Hides.** A hide lies on the ground (`ground_place`). The `scrape` recipe
  type: `strokes` right clicks with its `tool` (a knife). The `dry` recipe
  type: it then lies there `seconds` (90) and turns into leather. A bear hide
  gives two. A knife cuts leather into four strips (the knife stays).
- **The bag.** Six leather and two bindings on a crafting table. It goes in a
  bag slot beside the portrait and adds a row of 9 slots under the hotbar;
  pickups that do not fit go there, and it comes off only when empty.
- **Crafting keeps and leaves.** Shapeless recipes may name `keep` specs (the
  axe that splits a log into four firewood, the knife that cuts strips stays
  in its slot) and a `remainder` (the pot of stew leaves its cooking pot, like
  Minecraft's milk bucket).
- **Pottery.** New clay forms, all fired in a pit kiln: a pan, a bowl, a
  cooking pot and a storage pot. The cooking pot is a firepit vessel: two raw
  meat or three carrots make a pot of stew (the `melt` type with its own
  vessel), which crafts with three bowls into three stews; eating one leaves
  the bowl (`empties_to` on food). The storage pot is a placed 9-slot
  `ChestEntity`.
- **The pan.** Right click with it at water over sand or gravel (or at the bed
  itself), `Settings.pan_shakes` times: the pan's own `sift` table rolls
  (`station: "pan"`; sieve and pan tables no longer mix), and the washed
  voxel becomes water, so a riverbed is dug away as you pan.
- **Shelter.** Thatch (4 dry grass, burns), wattle (4 sticks, a woven micro
  model with full collision), daub (clay, dry grass, dirt) and wattle and daub
  walls. A straw bed (6 dry grass, table) sleeps and sets the spawn like the
  bed.
- **Torches.** A stick and dry grass make one; coal or charcoal and a stick
  make four. They stand on top faces only. Each is a `TorchEntity` (so the
  saved block entities list them); `TorchLights` gives the
  `Settings.torch_lights_max` (24) nearest within 48 m a shadowless
  OmniLight3D, and the flame cells use the fire tile, which glows in the
  terrain shader.
- **The charcoal pit, in the world.** Firewood (from a log and an axe) is
  right clicked onto a top face as a pile: 16 per voxel, a full pile starts
  the next one against the clicked face. Touching piles are one pit. A
  firestarter lights the whole pit; `Settings.charcoal_pit_grace_s` (20 s)
  later, and every second after, every face of every pile must touch another
  pile or a sealing block (solid or powder, not burnable, not transparent, not
  a micro model: dirt, sand, stone, clay, daub). Sealed, it smokes; after
  `Settings.charcoal_pit_burn_s` (120 s) each pile is a charcoal pile (one
  charcoal per two firewood) that you break open. Open after the grace time,
  or opened while it burns, the piles turn into the simulation's fire.
  Soil goes onto a pile with a right click, like against any block.
- **Book.** The Flint Age chapter grows to 35 quests. The new side roads are
  `optional` (drawn round) and only cordage joins the main road. The canvas
  zooms with the wheel and pans with a drag. New quest events:
  `charcoal_pit`, `panned`.
- **Materials** 103..107 (thatch, wattle, daub wall, torch, storage pot);
  atlas row 12 (tiles 88..92: thatch top and side, daub, withy, ember).

## Consequences
- Still no per-stack data: tools keep no wear when kept in a recipe, torches
  never burn out, stew never spoils, and a hide has no wet or dry state
  beyond its item.
- The charcoal pit's times are minutes, not Vintage Story's in-game hours;
  tune them with the calendar when it matters.
- `flint_age_test=1` drives every piece through the player's own clicks and
  prints `debug flint_age {...}`; `/kit flint` hands out the new items.
- Bronze will need the charcoal; the next chapter starts there.
