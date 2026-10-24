# ADR 0039: Patterned building materials

Status: accepted (2026-10-24)

## Context
ADR 0036 gave every material a palette: each 12.5 cm voxel shows one texel of its tile, picked by a
hash of its position, so terrain reads as flat-coloured voxels. ADR 0037 decision 8 keeps that for
nature and asks for **patterns** on crafted building materials (plank rows, brick courses, cobble,
bark). With shape building (ADR 0038) a wall is hundreds of voxels of one material, and a hashed
palette makes it a speckled slab. The atlas tiles already hold Minecraft-like pixel art (boards with
seams, bark stripes, thatch weave), drawn at 16 x 16.

## Decision
1. A material with `"pattern": true` draws its tiles as **patterns**: the tile is laid over the world
   grid at one texel per voxel, so it repeats every 16 voxels (2 m). Walls (x and z faces) show it
   upright, floors and roofs from above. Everything else keeps the hashed palette.
2. The shader decides per atlas tile: `GameMaterials.pattern_tiles()` sets bit t for every tile of a
   patterned material, `pattern_tiles` (four ints) carries it to `voxel_sample`. Falling pieces use the
   same rule at the position each voxel broke off, so a toppling wall keeps its pattern.
3. Patterned now: planks (boards three voxels tall with a seam row and staggered joints), every bark
   (and the birch, spruce, acacia and jungle log sides, which are bark tiles), thatch, and two new
   materials: **cobblestone** (109, what the cobblestone item places; stone already drops it) and
   **bricks** (110). Log ends stay hashed: their rings would centre on the 2 m grid, not on the log.
4. Bricks come the Vintage Story way: clay forms two raw clay bricks (`form_clay_brick`), the pit
   kiln fires them (`kiln_clay_brick`), four make a bricks block. Mortar and joints are whole voxels:
   at 12.5 cm a brick is about 0.9 m x 0.4 m (a stylised course, not real brick size), a cobble stone
   about 0.5 m.

## Consequences
- New patterned materials need a seamless tile (`tools/make_atlas.py` wraps them) and the key.
- A fallen log's bark stripes stand upright on its side (no log orientation yet).
- Lighting grades (ADR 0037 decision 8, second half) stay the developer's pick.
