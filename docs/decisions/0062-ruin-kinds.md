# ADR 0062: Ruin kinds that fit their biome, on level ground, with their own loot

Status: accepted (2026-10-26)

## Context
F9 bug report 2026-10-25_21-20-08: "limit 4 pots per ruin and also add more ruins varieties". The pot
limit landed in 0bbc43a. Every ruin was one shape (ADR 0024 era, `world-generation.md` "Ruins"): a
rectangle of stone walls in segments of random height, a stone floor, corner pots (ADR 0057). Its
ground was flattened inside the footprint and cut down to the floor + 0.5 m two cells round it, so on a
slope a ruin sat in a 1-2 m pit, and on a hollow it stood on a plinth (the old flatness check took the
four footprint corners within 2 m).

## Decision
1. **Kinds are data.** `game/data/ruins.json` lists the kinds; `tools/ruin_codegen.gd` (`RuinTable`)
   validates it and writes `prototypes/p5-sim-spike/src/ruin_table.gen.h` (a test fails when the header
   is stale). A kind names a shape (the geometry, C++ in `ruins.cpp`), a weight, the biomes it may
   stand in, its half size, reach above / below the floor, four materials (wall, trim, floor, extra),
   its pot limit (1..4) and its loot table. New kinds that reuse a shape with other materials or biomes
   need no code.
2. **Seven kinds, each its own shape:**
   - `house` (plains, forests, highlands, cold lowlands): the old stone walls; a stone now always
     rests on the one below (the old walls had stones hanging over gaps).
   - `watchtower` (highlands, plains, shrubland, tundra, taiga): a 2-3 m square cobble tower up to 7 m,
     its wall falling away toward one side in a spill of stones; a door, arrow slits, a plank floor left
     on the standing side. Two pots at most.
   - `stone_circle` (open land, savanna, tundra): 8-12 granite standing stones, 2-3.5 m, mossy tops,
     some toppled inward; an altar slab with two pots beside it.
   - `crypt` (forests, plains, taiga, swamp): headstones and a parapet on the surface, stairs down
     (a voxel down per voxel along, 2 m headroom) into a brick room 3.5 m under the ground with a stone
     coffin and four pots in its corners.
   - `mud_house` (desert, savanna, badlands, shrubland): terracotta-cream walls up to 3 m on a sandstone
     floor, a doorway and windows, acacia roof beams, sand drifted against the walls.
   - `shrine` (forests, jungle, swamp, taiga): a two-step brick plinth, four pillars (the back two
     carrying a lintel, the front two broken), an altar, moss on every top, leaves over the pillars.
   - `wreck` (beaches and stony shores, 0.5-2 m above the sea): a heeled plank hull half sunk in the
     sand with ribs, sprung panels, deck beams and a mast stump; three pots of cargo.
3. **Placement.** One candidate per 80 m cell (now 37/256 of cells, was 31/256: the stricter flatness
   check below rejects more). The kind is a weighted pick among those whose biomes hold the biome at
   the centre (shore kinds on shores only). The centre and eight points a cell outside the footprint
   must lie within 1.5 m of each other; the floor is their middle. FineGen flattens the footprint to
   the floor and ramps the ground round it over 4 cells (2 m), at most a cell up or down per cell out,
   so a ruin neither sits in a pit nor on a plinth. Ground stamps and clutter keep off the ramp; trees
   keep 4 cells further off (a small clearing, and no crown cut by the ruin). Inside its region the
   shape only places its own blocks and air: the ground, and what grows on it, stays FineGen's.
   A test walks every voxel of the kind's blocks to the ground (nothing floats) for 3 seeds x 7 kinds.
4. **Pots** stand at fixed spots of each shape (`technosim_ruin::pot_spots`), each spot taken 3 times
   in 4, at least one, at most the kind's limit and never more than 4, never side by side.
   `ruin_pots_near` reports exactly these spots (with the kind).
5. **Loot per kind.** A pot still carries `"shatter": "ruin_pot"`; when it shatters, `RuinLoot.table_at`
   asks the generator which ruin holds it (`FineGen.ruin_at`) and rolls that kind's table
   (`tower_pot`, `circle_pot`, `crypt_pot`, `mud_house_pot`, `shrine_pot`, `wreck_pot`; the house keeps
   `ruin_pot`). A pot outside every ruin (one the player set down) keeps `ruin_pot`. No new material ids.
6. `find_nearest` takes `ruin_<kind>` and `crypt_room` (a free spot of a crypt's floor), so the
   autopilot can film each kind. GEN_VERSION 18.

## Consequences
- No material ids or atlas tiles appended: the shapes reuse existing blocks (a mud-brick block of its
  own would read better than terracotta cream; a later content pass can add one and change one line of
  `ruins.json`).
- Ruin shapes are code; a new shape needs C++ and a rebuild. Their data (size, materials, biomes, pots,
  loot) does not.
- Wrecks are rare: a cell's candidate point lands on a shore strip only now and then.
