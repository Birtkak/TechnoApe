# 0037 – Direction: Lay of the Land feel, Vintage Story and GregTech depth

- **Status:** accepted 2026-10-24 (the developer, decision by decision)
- **Date:** 2026-10-24

## Context
With the voxel world in place (ADR 0033-0036), the developer asked for a comparison of our core
mechanics with Lay of the Land (LotL) and Vintage Story (VS), from simulation to art style, and
then decided the direction one system at a time. Research and the full option lists:
`docs/research/2026-10-24-core-mechanics-lotl-vintage-story.md` (and
`2026-10-24-lay-of-the-land-building-tools-fluids.md`). The pattern it found: LotL wins on feel
(looks, physics, building) and loses on depth (thin loop, empty world, basic combat); VS wins on
depth and loses on feel. We aim for both, so each system had to pick its side.

## Options considered
Per system, see the research doc (four options each, pros and cons there).

## Decision
1. **Simulation:** the Noita kernel stays the pillar. Add **rigid voxel pieces**: an unsupported
   group of voxels (a felled trunk, a cut pillar, an undermined ledge, later whole buildings) breaks
   off as one body, falls, crushes and settles back into voxels. Rivers outside the bubble stay
   static.
2. **Building:** LotL **shape building**, unlocked from the start: a radial menu of shapes (box,
   wall, cylinder, cone, sphere, stairs/slope), a preview on the ground with drag handles (height,
   size, hollow, move), rotate and mirror, click away to commit, voxels taken from the inventory;
   box mode for digging and building; **prefabs** (the last build, saved selections in categories).
   The brush stays as "normal" mode.
3. **Crafting:** physical for tools and gear (knapping, assembling head + stick + binding on the
   ground or a workbench, anvil, moulds, firepit, pottery, hides); the grid for bulk and simple
   recipes; machines get their own screens. The recipe browser says which and how.
4. **Progression:** ages (flint, copper, bronze, iron, steel) into GregTech machines, power and
   automation; exploration gates materials. No boss loop as the spine.
5. **Survival:** hunger plus VS **body temperature** and **food spoilage**; no nutrition groups;
   the same rules in every world (no setting to turn them off).
6. **Threat:** wildlife that bites (wolf packs, boars, bears, predators per biome) and hostile
   creatures in **places** (Noita-like dungeons, deep caves, guarded ruins). The surface stays
   calm: no world clock, no night monsters. Combat is a side system for now.
7. **Magic:** later (Phase 3); the vision's Noita wand idea stands until then.
8. **Art:** shuffled palettes for nature; **patterned** voxels for crafted building materials
   (plank rows, brick courses, cobble, bark, log rings); then three lighting grades to pick from.
9. **Places:** generated structures per biome with loot and guardians and enough variety (modular
   rooms, procedural layouts) not to repeat, Noita-like dungeons, a cave biome per region, roads and
   bridges between places, and **traders**.
10. **Multiplayer:** co-op ready, dedicated server in Phase 4. Every new system keeps its game state
    in a server-side layer; the client draws it and sends inputs.

## Consequences
- The vision (`vision.md`) is updated to these calls; where older ADRs disagree (ADR 0034 brush as
  the only building tool, ADR 0036 palettes for every material, ADR 0019's grid-assembled tools),
  this ADR wins.
- New technical risk: rigid voxel pieces (splitting, physics, settling, the budget) get a prototype
  in `prototypes/` before they enter the game, like the sim kernel did.
- Temperature needs room detection (what counts as enclosed); spoilage needs per-stack timestamps
  in inventories, chests and pots.
- Building with shapes needs gizmo UI, previews that cost nothing until committed, and a prefab
  file format (`user://prefabs/<category>/<name>.json`).
- Places need a structure generator that composes rooms, plus road routing in `FineGen`/`WorldGen`
  and a `GEN_VERSION` bump.
- Traders need an economy (what they buy, sell and at which rates): its own ADR when it starts.
- Combat stays minimal on purpose; revisit when dungeons exist.
- Open: the order of the work, the lighting grade, and the magic design in Phase 3.
