# ADR 0063: Ledges, cliff bands, a deeper sea and gentler ravines

Status: accepted (2026-10-26)

## Context
Bug reports of 2026-10-25 (seed 1579829108 unless noted):
- 20-49-22: "oceans should be deeper". The shelf off a beach was 0.75 m deep at the report spot (c -0.157),
  half the sea under 4 m.
- 20-58-48: mountains "feel epic now but still are super easy to traverse". The massif surface was smooth;
  the auto step-up (0.55 m, 4 voxels) climbs any staircase of 1-3 voxel steps, so even 70-degree faces were a
  walk. Of the rises between neighbouring columns around the report spot, 0.01 % were more than 4 voxels.
- 20-18-57 (seed 227694785): trees float over ravines, and ravines are "a tad bit steep". A ravine was a
  slot: the width faded to nothing toward the ends of the mask while the depth stayed 30 m, and the walls'
  upper halves rose 5-13 voxels per voxel. Trees, stamps and sticks stood on the uncarved surface.
- 21-23-02 (a lush cave): "every 1 single voxel step up makes everything feel like lego, these pools are
  super duper tiny". The fine surface and the cave field are interpolated per voxel and rounded, so every
  slope is a staircase of single voxels (93 % of the rises on land, 98 % on cave floors); a cave pool needs
  a flat floor cell with rock on four sides, which a staircase rarely has.

## Decision
1. **Deeper sea** (`column_base` continentalness spline): -1.5 voxels at the beach, -24 (12 m) at c -0.15,
   -64 at -0.3, -120 at -0.45, -175 at -0.6, -220 (110 m) at c -1. The abyss floor is 18 m over bedrock.
2. **Cliff bands and arêtes** on the massif (`column_base`, mountain variants 1 and 2): a knife ridge up to
   25 m along the ridged noise's crest lines, then the relief is cut into tiers 8-18 m tall (n_band_, ~350 m
   patches) whose risers take 6 % of a tier's span of the smooth height, so a riser is ~16x as steep as the
   slope it replaces: walls where the mountain was steep, walkable ledges on the gentle foot. Tier edges
   wobble with the detail noise. The slope test turns the risers to rock.
3. **Ledges on the ground** (`FineGen::ground_f`, used by `surface_f` and `generate_block`): swells, then
   rounding to levels 3, 4, 5, 6 or (rarely) 8 voxels apart (n_terrace_, ~150 m patches), keeping 15 % of
   the slope; hummocks and roughness after the rounding at 40 % on full ground. The ground weight fades it
   all at water, rivers, marsh and ruins (ruin floors and ramps are unchanged; test_ruins passes).
4. **Ledges in caves**: the cave field is read once per 4-voxel band (`cave_band_y`, aligned to the world
   grid), so floors, walls and ceilings change height in 0.5 m ledges with flat treads. Cave pools: n_pool_
   at ~25 m blobs, threshold 0 (was ~11 m, 0.35).
5. **Ravines**: depth and width both fade in with the mask (ends are shallow trenches), 10-40 m deep, the
   width falling as (1 - depth)^2.2 from 1.6x the old top width. `WorldGen::ravine_near(x, z, r)` (the cut
   one voxel down within r macro columns, with a margin) keeps canopy trees (r 5), stamps (3), young trees
   (4), floor features and sticks (3) and clutter (1) off the rim. Tree trunks also reach 10 voxels into
   the ground (was 6), so a trunk beside an 8-voxel ledge meets the lower tread.

GEN_VERSION 20. No material ids, atlas tiles or biome ids added.

## Consequences
Measured with `tools/relief_probe.gd` (seed 1579829108, main's DLL vs this one):
- Sea: depth p50 8 -> 28 m, p90 48.5 -> 77.5 m, max 81.5 -> 103.5 m; 29 -> 64 % of sea columns deeper than
  20 m; the report spot 3.5 -> 12.5 m.
- Mountain (report spot, 48 m radius): rises over the 4-voxel step-up 0.01 -> 6.7 % of neighbour pairs,
  over a jump (10 voxels) 0 -> 1.3 %.
- Land (the cave report's surface): single-voxel share of rises 93 -> 59 %, rises of 4+ voxels 185 -> 57,500
  pairs (of 1.18 M).
- Cave floor (report box): single-voxel share 98 -> 0.5 %; biggest pool 7.4 -> 25 m^2, pool columns about
  the same (3,491 -> 3,312).
- Ravines: floating trunk columns near four ravines 215 -> 0 (forest control 6 -> 4 %, branches the probe
  counts); upper-wall slopes (rise per run) 5-13 -> 1.7-2.3 on most walls, some walls still sheer where a
  cave or a ledge meets the rim; depth 37-58 -> 26-34 m.
- Generation cost: surface blocks (400 land columns, lod 0/1/2/4, min of 4 alternating runs with other
  Godot processes on the machine) within -6 .. +8 % of main per LOD and seed.
- Plains with 8-voxel ledges need jumps; the ledge spacing is a tunable (`ground_f`), as are tier heights.
