# ADR 0064: Bunched ore lumps with patterned ore tiles; the dry savanna floor

Status: accepted (2026-10-26)

## Context
Bug reports of 2026-10-25:
- 21-05-13 and 21-06-48: ore veins should look nicer, "a bit more bunched up with a shared connected
  textures thingy so we can have modular detail / more color". A vein was a lattice: every 0.5 m cell of
  the lens held a 3x3x3 nugget (ADR 0047), evenly spread, and every ore voxel showed one texel of its
  tile picked by a hash; the tiles mixed host-rock grey into the ore colours, so a wall of ore read as
  salt and pepper.
- 20-37-22: the savanna should not look like a lush forest floor. It used the meadow's grass, tall grass
  and flowers, with an acacia every ~20 m.

## Decision
1. **Ore lumps** (`FineGen::vein_voxel`). The lens is cut into Voronoi lumps around jittered points 5
   voxels apart, squashed 1.35x in y. A seam of host stone (where the two nearest points are within 1.0
   of each other) parts neighbouring lumps, so a lump still digs out as one piece (ADR 0047; ~20-80
   voxels, under `GroundPickups.MAX_CELLS`). A lump is ore or stone as a whole: the chance is the vein's
   density times the host rock's bias times a pocket field (two octaves of value noise, ~3 m pockets,
   flatter in y) that is 3.0 in pockets and 0.2 between them, plus a boost toward the lens centre (a
   prospector who finds the centre finds ore). The GregTech layer follows the lump point's height, the
   pocket field wobbling the borders; 12 % of lumps are the sporadic ore. Measured on the report's
   cassiterite vein (seed 1579829108, 6x6x4 blocks): ore share 33 % before, 39 % after; a lens block
   takes ~2.2 ms instead of ~1.7 ms (27 hashes per lens voxel; blocks outside lenses are unchanged).
2. **Patterned ore tiles** (tiles 16-19, `"pattern": true`, ADR 0039). Each ore tile is one 16x16 picture
   laid over the world grid (`make_atlas.py ore_vein_pattern`): crystals 4-8 texels long along a
   diagonal, a bevel of light and shade, a thin dark matrix, one or two wandering veinlets of a second
   mineral and a few glints, 8 colours each (coal: black with pyrite; iron: hematite red with specular
   grey; copper: native copper with malachite green; tin: cassiterite brown with quartz veins). Adjacent
   ore voxels show adjacent texels, so a lump and its neighbours read as one textured vein; the tile is
   the "modular detail". No shader change.
3. **The savanna floor** (materials 127-129, tiles 109-111, from the world generation reserve 121-129):
   `savanna_grass` (straw top), `savanna_tall_grass` (golden tussocks, a column plant that cuts for dry
   grass like tall grass), `red_earth` (bare laterite, also the soil under the savanna). FineGen lays
   red earth in patches a few metres across (meadow noise), tussocks on 3-20 % of the grass columns (clumped by the meadow noise), 1-5
   voxels tall, and almost none on bare earth; no flowers, ferns or litter. Acacias at density 0.022
   (was 0.04), bushes 0.06 (was 0.12), no blueberries. `GameMaterials.is_grass` / `is_tall_grass` make
   farming, grazing, animal spawns and the survival commands treat the new grass like grass.

## Consequences
- Saves keep their voxels; newly generated vein and savanna chunks look different from old ones beside
  them (generator change, as every look pass before).
- True connected textures (outlines where a lump meets stone) would need neighbour data in the shader;
  not done. Crater walls (stepped surfaces) still mix top and side texels of the pattern; flat tunnel
  walls show it best.
- Evidence: before/after shots at the report positions, `game/out/{ore,sav}_{before,after}/shots/`
  (scripted: `/tp`, `/blast 8` x3 to open the vein, `/cam`, `/shot`).
</content>
</invoke>
