# Lay of the Land: how it draws distance (2026-10-26)

Question (hand-off `docs/plans/lotl-mesh-lod-handoff.md`, step 1): what does Lay of the Land (LotL,
Steam 2776090, Southern Cross Interactive / Tooley1998, Unreal Engine) do beyond the full-detail radius,
and what of it should our far layer copy? Nothing published explains the technique in words; this note
is the developer's few public sentences plus what the footage shows, each marked as such.

## Sources read

- The devlog clip "Adding Seamless LOD Transitions to my Voxel Game" (YouTube WfxuCEyK16I, 2025-08-22,
  1:09, no narration). Description, in full: "I've added smooth level of detail transitions to Lay of the
  Land. There is now almost zero pop-in when moving around the world making traversal look and feel much
  better." Stepped through at 10 % intervals (jungle road, desert mesas, a bridge, a stone church).
- The same day's X post: "Spot the LOD transition. Hint: it's much harder now in Lay of the Land."
- The developer on the Steam forum ("Why system requirements so high?", 2026-03-26): "It's mostly due
  to with generating and rendering the huge number of voxels the game has. Each voxel in the game is
  10cm. There are 1000 voxels occupying the same volume of space as a single voxels in games that use
  1m voxels."
- Changelogs (Ko-fi / Steam): 0.5.0 "Foliage draw distance now changes with view distance"; 0.7.0
  "Improved render thread performance by dramatically reducing number of draw calls with a voxel mesh
  merging system"; 0.12.3 "Spread out loading/unloading of vegetation instances over multiple frames";
  1.3.2 (2026-10-15) "New graphics settings intended for handhelds", "a shading quality graphics setting
  that primarily effects sky and water rendering"; 1.3.5 "Improved voxel query performance". A 2024 X
  post: voxel data generation moved to Intel ISPC, "2-3x speedup", less caching of voxel data. No
  changelog ever names a render-distance figure or an LOD level.
- "Meet the Biomes of Lay of the Land" (YouTube uT8xprIJR6o): the opening shot is from ~150 m up over
  forest, a volcano and red mesas 1-2 km off, the only altitude footage with a clear horizon.
- Steam store page: DirectX 12, hardware ray tracing on the recommended tier (RTX 3060 Ti for 1080p60
  "High"), 16 GB RAM, world generated up front; its ten screenshots are all at ground level.
- Reviews: Summit Reviews (2026-05): "a very high render distance that allows you to witness large
  swathes of land in their most minute detail, especially at higher altitudes". ProtonDB (Steam Deck
  class APU): "view distance above Low will cause an instant crash when loading in a world"; forum
  users quote the menu tiers Low / Medium / High.
- Comments under the LOD clip (183): nobody can see a transition; one viewer "could spot when it
  happens only on a tree"; several ask for a before/after. The developer answered none of the
  technical questions.

## Answers

**Full-detail radius, number of levels, far voxel size.** Not published. Inferred: the view-distance
menu has three tiers and the lowest already crashes an 8 GB handheld, so the horizon is kilometres,
not hundreds of metres; the altitude shot shows ground to the atmospheric horizon (several km). With
10 cm voxels, a kilometre horizon at any sane triangle budget means at least 5-6 power-of-two levels
(10 cm, 20, 40, 80 cm, 1.6 m, 3.2 m); a 2 km view at 3.2 m cells is ~400 k columns, the usual size for
a clipmap. Nothing in the footage contradicts plain power-of-two voxel levels.

**What a far level looks like.** The footage shows: hills, mesas and the volcano as *smooth masses with
their own colour bands* (the mesa strata are the generator's layers, visible as bands, not as 2^n cube
stacks); forests at 500 m-1 km as a *speckled canopy of individual tree blobs* with their own shading,
trunks gone; the mid-distance ground (100-300 m) still shows *per-voxel colour speckle*, softened, not a
flat average. No ring, rim, trench or cube-of-cubes edge is visible in any frame at any distance, at
1080p. Reading: whatever the far mesher is, its silhouette is the terrain's silhouette and its colour is
the voxel colour noise filtered by the screen (TAA is on: the frames have its softness and ghosting),
plus haze. Whether the far levels are merged voxel cubes with averaged *geometry* (a 1.6 m cell is 1-2
pixels at 500 m, so cubes would be invisible anyway) or a smoothed surface cannot be told from the
footage; the point is that nothing coarser than a pixel is ever shown, and that the near-to-far
*colour* is continuous because the voxel colour noise is kept, not replaced by a flat tile average.

**How transitions are hidden.** The clip hides them so well that a frame-by-frame step at 10 %
intervals shows nothing. The one tell a viewer reports is "on a tree": vegetation instances swap. With
TAA on, a per-pixel stochastic (hashed) cross-fade between two LOD meshes over a distance band resolves
to a true alpha blend over a few frames, which is the standard Unreal way (dithered LOD transitions +
TAA) and matches "almost zero pop-in" exactly; a hard geometric switch of 1-2 pixel cells would also be
invisible at these distances. Nothing suggests geomorphing or skirts. Our earlier 4x4 Bayer fade read
as a "stitching" pattern because we have no TAA to integrate it (ADR 0073); the lesson is a per-pixel
*continuous* blend (ADR 0073's sky blend, now to a far layer) rather than any ordered pattern.

**Water, cliffs, trees, buildings at distance.** Water: flat sheets at their level in every far shot
(a lake at 200 m in the biomes clip, the river at 100 m in the LOD clip), never a dark step. Cliffs: the
mesa walls keep their vertical faces and strata bands to 1 km; the volcano a smooth cone. Trees:
individual instances to the horizon ("vegetation instances", "foliage draw distance now changes with
view distance"): the canopy reads as many blobs, so a forest at 1 km is still a forest, not a green
field. Buildings: the church at 150 m in the LOD clip is full voxel detail; nothing further away is
shown, but player builds are voxels like the terrain and the same merged meshes carry them.

**Player edits in the far terrain.** Not published. The "voxel mesh merging system" and ISPC-generated
data suggest far meshes are built from the same voxel data (regenerated or cached), so edits would be in
them after a remesh; no footage shows a far edit.

## What we take from it

1. The far look to match is: silhouette of the terrain, flat sheets of water at their level, trees as
   individual blobs to the horizon, colour continuity with the near voxels (keep the speckle, let the
   screen filter it), haze doing the rest. Not: cubes of cubes, averaged flat tiles, canopy as ground.
2. Cell size at distance only has to stay under about a pixel: 25 cm at 40-80 m, 50 cm to 160 m, 1 m to
   320 m, 2 m to 640 m, 4 m beyond (1280x720, 70 deg FOV: 1 px = distance / 1000).
3. Transitions: a continuous per-pixel blend over a band, never an ordered dither without TAA.
4. Trees are an instancing layer, not terrain cells.
5. Their voxel is 10 cm to our 12.5 cm; their minimum GPU is a GTX 1660 Ti at 30 fps on Low, ours an RTX
   3070 at 120 fps with 8.3 ms p99: we have roughly a third of their frame budget for everything, so
   the far layer has to be cheaper than theirs, which rules out drawing far voxel volumes as merged
   cubes at 10 cm granularity and points at a height-field skin plus instanced trees (ADR 0074).
