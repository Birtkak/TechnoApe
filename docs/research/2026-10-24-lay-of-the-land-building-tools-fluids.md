# Lay of the Land: building, tools and fluids (2026-10-24)

Question: how does Lay of the Land (LotL, the voxel sandbox ADR 0033 takes its world from) build,
use tools in the world and handle fluids, and what would we change to match it?

Sources: a building walkthrough video (youtu.be/HTSj2lgKJBM, its captions read at 2x), the
developer's devlogs (frames of "Simulating Thousands of Voxels", "Meet the Biomes", the trailer),
the 76 Steam announcements and patch notes (Steam news API for app 2776090), the Steam screenshots.

## Building

- **Material in hand, shape from a menu.** Hold a building material (oak planks), press E: a radial
  menu offers shapes (box, cylinder, cone, ...) and prefabs. Since mid 2025 the whole building UI
  is radial.
- **Click the ground, then shape it with gizmos.** A click places a preview (drawn in the world, not
  on top of voxels, so it shows what will be placed). "Physicalized" gizmos, dragged with the mouse:
  - an up/down arrow: height;
  - the outer ring: grows the shape outwards in the direction you look (diameter / size);
  - the inner ring: hollows it (an inner radius: a tower, a well, a ring wall);
  - transform handles: move the whole shape up/down and sideways;
  - a transform UI: rotate on any axis, flip/mirror on any axis (gizmos show rotation and flip).
- **Click away to build.** Left click off the shape finalizes it; the voxels it needs come out of
  the inventory (it checks "enough of the required voxel items").
- **Box mode.** Build mode "box": click a start point, drag the camera to size the box, click to
  finalize. The same box mode on a tool digs: click, drag, click removes that box.
- **Detail mode (C).** Finer voxels for both building and digging, for trim and small cuts.
- **Prefabs.** What you just built becomes a prefab to place again. Custom prefabs: select an area
  with adjustment gizmos, align to centre, save; folders are categories.
- Crouching sticks to ledges, to build on the edge of a build.

## Tools in the world

- Tools dig: each tool has dig modes (normal, box, cube; F changes the mode) and the detail toggle.
  The axe is the tool for wooden builds (box mode carves pieces out of a plank wall).
- Trees fall as voxel physics objects; the axe chops the fallen tree and strips leaves (fewer hits
  since 1.0.12); tools in box or cube mode still harvest physics objects like normal mode.
- Durability: tools wear and break (a soft-lock fix exists for breaking a tool in a hole).
- Physical crafting: work happens on objects in the world, e.g. hit an anvil with a hammer to
  repair an item or take a gem off a weapon; the crosshair shows a hammer icon when the held item
  can work the thing you look at. Ores are melted and cast into shapes (metalworking).

## Fluids

- **Source and flowing water, Minecraft style.** Water voxels are sources; flowing water is derived
  from a connected source and goes away when the source is removed (1.1.9 fixes: "flowing water
  voxels occasionally remaining when their connected source voxel is removed", "ghost flowing water
  effects"). Source and flowing voxels have different colours so the player can find and remove the
  source. Sand, gravel and snow falling into water replace it.
- The look: clear turquoise, the bed visible through shallow water, darker teal in the deep; a
  "shading quality" setting mostly for sky and water (1.3.2). Rivers and lakes are generated; caves
  in the swamp hold pools.
- Other fluids are effects rather than volumes: acid leaves a residue, gas pockets spread and
  suffocate, fire spreads; fire and acid go out in water.

## Against TechnoApe today

| | LotL | TechnoApe (ADR 0034, 0010, 0029) |
|---|---|---|
| Build | shape + gizmos (height, size, hollow, move, rotate, flip), click away to commit, prefabs | brush: cube / sphere / cylinder at 1-8 voxels per click (Ctrl + wheel, B) |
| Dig | tool modes normal / box / cube, detail toggle | the same brush, sized per click |
| Fine work | detail mode | 12.5 cm is the only grid; chisel retired with the 0.5 m era |
| Water | source + flowing, generated rivers | Noita kernel in the 64 x 32 x 64 m `SimBubble` (mass-conserving, levels), static water outside it |

Water look: done 2026-10-24 (depth-tinted, no texture, `terrain_fluid.gdshaderinc`).

## Proposed (needs the developer's go, then an ADR)

1. **Shape building with gizmos** (ADR 0037): a build mode on the held material: pick a shape
   (box, cylinder, cone, sphere, wall, stairs) from a radial menu, click the ground for a preview,
   drag handles for height / size / hollow / move, R rotates, flip; left click away commits and
   takes the voxels from the inventory; the brush stays as "normal" mode.
2. **Box dig and box build**: click start, drag, click; the same handles for fine adjustment.
3. **Prefabs**: the last build is a prefab; save a selected area as a named prefab
   (`user://prefabs/<category>/<name>.json`), place prefabs with the same gizmos.
4. **Tool modes** per tool (normal / box), F cycles; a detail mode at 6.25 cm needs the chisel
   path back and is a separate decision.
5. **Fluids**: keep the Noita bubble (it is the simulation pillar of vision.md) and consider LotL's
   source / flowing split only for generated rivers outside the bubble. A taste and scope call.
