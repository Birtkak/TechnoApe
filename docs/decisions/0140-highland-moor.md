# ADR 0140: Highlands as open moor: ice-worn hills, thin soil over rock, tors, scree, peat, heather

Status: accepted (2026-11-04). Touches the generator only (`world_gen.cpp` column_base and tree_at_cell,
`fine_gen.cpp` macro_point and the column pass, `biomes.json`). No new materials. GEN_VERSION is bumped at merge.

## Context
The developer: "Make highlands more realistic." Captured first (three seeds, fixed spots, four cameras each, 24 m
view): seed 1337 (-384, 12288), seed 42 (-19968, -19200), seed 7 (-3840, 2304), all inside the highland band.
What read as fake:
- **Woodland, not upland.** Oak and birch at 0.18 plus bushes: 27 % of the surface seen from above was leaves,
  seed 42's spot was a closed oak wood. Real uplands (Scottish Highlands, Pennines, fjell) are open moor with trees
  only in sheltered glens.
- **A flower meadow.** A green grass carpet with clutter flowers of every colour; heather only in a few patches.
- **No rock.** Bare rock was 4 % of the surface; slopes were turf down to a metre of dirt, so ledge risers and
  steep banks showed dirt walls. Real hills are thin soil over rock: crags on the steep sides, scree under them,
  tors on the knolls.
- **Noise humps.** The ~70 m hills were `pow(noise, 1.3)` bumps, and on the massif apron (where most highland lies)
  they faded out, leaving one smooth ramp.

At the player's scale (12.5 cm voxels, a 72 m box) the cover and the 1-30 m forms decide the look; kilometre glens
cannot be seen, so the changes go to the surface and the 70 m hills.

## Decision
1. **Ice-worn hills** (`highland_hills` in world_gen.cpp): on high ground (50-75 macro voxels over the sea and up,
   faded on the steep massif core and in hot lands, so badlands and lowland biomes keep their forms) the hill noise
   takes a glacial profile: flat floors below 0.32, convex rounded knolls (`u(2-u)`) on top; the hills also stand on
   the massif apron. Same noise sample as before.
2. **Thin soil, crags, scree, peat** (`FineGen::highland_ground`, highlands and frozen highlands): soil 4-6 fine
   voxels on flats and 2-4 on slopes (every ledge riser shows rock under the turf); slope > 0.65 scree (gravel with
   rock patches), > 0.9 bare crag; flat hollows (macro detail noise dn < -0.42) are peat: dark earth and turf,
   12-14 voxels deep. Bare rock on top is the local rock kind (`rocks_.at`), not grey stone.
3. **Tors** (`FineGen::highland_tor`, in macro_point): where dn > 0.55 (a knoll's crown) the ground rises 1-4 m in
   1 m joints as bare local rock (`col.rock`, the monolith path) with a step at its edge; ground relief there is
   damped so the faces hold. Never by water, ruins or a ruin's ramp; no trees (dn > 0.42) or flower stamps on them.
4. **Moorland plants** (`FineGen::highland_plant`): heather over most of the open ground (n_meadow < 0.15, 62 % of
   those columns, some knee high), bracken on lower slopes (n_meadow > 0.38), coarse grass between; sphagnum moss
   and cotton-grass tufts on the peat. Flower stamps one in eight as often as on other grassland.
5. **Few trees, in shelter** (`highland_shelter`, biomes.json): highlands 0.06 birch with 35 % spruce (the pine of
   the glens), frozen highlands 0.04 spruce; x2.5 in hollows, x0.25 and stunted on exposed knolls, none on tors.
   Boulders (erratics) 0.15 -> 0.3 (frozen 0.1 -> 0.25), bushes 0.05 -> 0.02.

Cost: everything reads values the column already had (dn, slope, the 25 cm patch hash, n_meadow_); fewer trees and
flower stamps make highland blocks cheaper.

## Measured
- Surface census (top voxel over 32 m round the three spots, `game/out/census.gd`): leaves 27.5 % -> 2.5 %, heather
  8.7 % -> 20.6 %, bare rock (granite, stone, gravel) 5.8 % -> 24.5 %, peat moss 2 %, bracken 3.4 %.
- Generation, highland lane (960 LOD 0 16^3 blocks over the three spots, one thread, 5 passes on fresh generators
  per run, A/B interleaved, FineGen gen_stats): median ms per block 0.354 / 0.315 / 0.356 before, 0.224 / 0.221 /
  0.315 after (median of 3 runs 0.354 -> 0.224 ms, 2825 -> 4464 blocks/s, -37 %).
- `test_highlands_are_open_moor` (heather outnumbers leaves, bare rock shows) fails on the old generator
  (heather 16997, leaves 383357) and passes now; the generator suites (fine_gen_lod, fine_gen_top, biomes,
  content_pass, ruins: 63 cases) pass.
- Evidence: `game/out/highlands_before_after.png` (rows before/after per spot).

## Not done
- Bog pools and tarns with water (needs a level rim test; finite water, ADR 0068), streams cut into gullies, and
  U-shaped glens at the kilometre scale (the massif's ridged valleys stay V-shaped).
- A peat material (dark earth is dirt for now) and wind-bent tree shapes (exposed trees are only stunted).
