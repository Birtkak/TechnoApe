# 0035 – Landforms and living forests

- **Status:** accepted 2026-10-23 (the user's direction; numbers are routine calls, recorded here)
- **Date:** 2026-10-23

## Context
After the 12.5 cm overhaul (ADR 0033) the user asked for a world that "feels already much more
alive and dense", Lay of the Land style: "old growth and new trees, way thicker, branches and big
far reaching leaves", "much more small hills", "huge mountains, plains and small hilly areas", a
forest floor "brimmed with dead logs, leaves, bushes, flowers, grass, mushrooms", and later
"trunks can be much thicker". A reference screenshot showed broad oaks with thick bark trunks,
crowns built from many clumps, light tall grass on a darker meadow and paths in their own colour.

What the generator made (seed 1337, `tools/terrain_map.gd`): an archipelago of ~1 km islands,
each with a peak, almost no plains (continentalness had a 1.5 km period); one tree per 4 m cell
with a 0.2-0.3 m trunk and one round crown; a stamp (bush, boulder, log) per 8 m cell; ground
noise of +-0.25 m.

## Decision
**Landforms** (`WorldGen::column_base`, macro units). Continents ~8 km across (continentalness
period 8 km), the land rising from a few metres above the sea at the coast to ~30 m inland.
A region noise (~3 km) makes each area plains, hill country (rolling 3-11 m hills ~70 m across,
also where a second mask says so) or a mountain range: ridged noise with ~1 km between ridges,
peaks up to ~225 m above the sea (the world is 384 m tall). Erosion still scales relief.
Biomes keep their altitude bands, so lowland forests sit in plains and hills.

**Ground relief** (`FineGen::ground_noise`): 8 m swells (+-0.44 m), 2.5 m hummocks (+-0.19 m)
and 1 m roughness, weighted per lattice point (`MacroPt::gw`) and interpolated: zero on lake,
river and marsh beds and ruins, fading in over the first 3 m above the water, so shores and
channels stay smooth and no step appears at their edge.

**Trees** (`FineGen::build_tree`). Every canopy tree gets an age (`tree_age`: old growth
0.55-1, forests 0.3-1, open land 0.25-0.85, the tree line 0.15-0.25) that scales its height,
trunk and crown. Oaks and jungle trees: trunk radius 0.23 m young to 1 m ancient (+0.19 m in
old growth), tapering to half at the crown, flaring into 4-6 roots; 4-8 limbs that rise in two
segments and end in a cluster of three leaf clumps, plus a crown top, so the crown reaches up
to ~6 m and reads as clumps, not a ball. Birches: slender, small clumps up the top half.
Spruces: a layered cone, wider with age. Acacias: a fork carrying flat plates. Trunks and limbs
are bark outside (a new grey-brown oak bark tile) and wood inside. Young trees: one chance per
3 m cell at 0.55 x the biome's tree density, a tall bare pole with the crown above head height,
never within 1.75 m of the cell's canopy tree.

**Forest floor** (`FineGen::floor_features`, per 2 m cell, scaled by tree density): fallen logs
at any angle (0.25-0.7 m radius, 2.5-7 m long, end grain, a branch stub, moss and mushrooms on
top), stumps with moss, bushes (leafy or blueberry), mossy rocks. Under a crown: leaf litter
(new material 108 `leaf_litter`, atlas tile 84, digs like grass and drops dirt) in patches, ferns,
a few tufts, mushrooms at the foot of trunks, twigs. In the open: tall grass that thickens and
thins in ~6 m patches and flower patches of one kind each.

**Far forests**: at LOD 4+ a tree's cell carries a dome of leaves as tall as its crown instead of
a leaf-coloured ground, so distant forests keep their height.

**Mesh uploads**: `Settings.mesh_upload_budget_mb` 0 (unlimited) -> 4. A forest's first meshes
uploaded 15 MB in one frame (16 ms of main thread); at 4 MB a frame no frame passed 16.7 ms.

## Measured (dev box, 1280x720, vsync off)
| scenario | p50 | p99 | max | > 16.7 ms | triangles | load |
|---|---|---|---|---|---|---|
| standing at spawn (x2) | 2.8 | 3.7 | 4.3 | 0 | 3.2 M | 12 s |
| fly 6 m/s (x2) | 2.9 | 3.9 | 13.7-19.5 | 0-2 | 3.5 M | 15 s |
| fly over a forest, 30 m up | 3.5 | 4.5 | 6.6 | 0 | 5.8 M | 17 s |
| forest, eye level | 5.4 | 6.8 | 8.8 | 0 | 20.5 M | 15 s |
| old growth, eye level | 6.5 | 7.8 | 10.2 | 0 | 21.3 M | 14 s |

Generation (`tools/fine_gen_bench.gd`, one thread, per 16^3 block): plains 0.2-0.4 ms, forest
0.2-0.34 ms, old growth 0.4-0.7 ms at LOD 0-2 (was 0.1-0.2 ms). Of the 20 M triangles inside a
forest ~6 M are the camera pass; the 4 shadow cascades draw the rest (2 cascades: 11 M less).
`tools/face_probe.gd` counts exposed faces per material: forest and meadow ground at LOD 0 is
190-280 faces per m^2 (flat ground 64), mostly leaves, ferns and tufts.

## Consequences
- The world has room to travel: plains to cross, hills to roll over, ranges to climb; forests
  are places, not scatter. Saves from GEN_VERSION 12 do not load (13).
- Loading takes 12-17 s instead of ~10 s; benchmarks need `load_cap_s=30` to measure after the
  world settles. Forests sit close to the frame budget on the GPU (p99 7.8 ms in old growth).
- Levers if the budget gets tight: shadow cascades in forests, leaf clump surface noise only at
  LOD 0, fewer tufts in the meadow pattern. Paths and roads in their own colour (the reference)
  are not generated yet (ADR 0033 lists roads).
- Tests that looked for a biome near the origin search 16 km (some lie a continent away).
