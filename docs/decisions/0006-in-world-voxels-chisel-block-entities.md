# 0006 – In-world voxels: chiseling every voxel, block entities, decor

- **Superseded** by ADR 0033/0034 (no chisel grid: chiseling is the brush at size 1); the block-entity layer lives on; removed with ADR 0036.
- **Status:** proposed (requested 2026-10-22: "make sure in the future we
  can chisel every voxel so builds will be unlimited possibilities", "get
  inspiration from Vintage Story", "freedom and functionality for chests and
  furnaces etc."). ADR 0007 (station blocks, same day) is the first
  implementation of the block-entity layer described here
  (`game/scripts/blocks/block_entities.gd`, keyed by voxel position, ticked
  at 20 Hz); the `Chisel` behavior joins that registry, no second layer.
- **Date:** 2026-10-22

## Context
A voxel today is one 16-bit id in godot_voxel's `VoxelBuffer`, drawn by the
blocky mesher from a fixed library of models (cubes, and since 2026-10-22
micro-voxel patterns for vegetation). The developer wants three things from in-world
voxels that this alone cannot give:

1. **Every voxel chiselable** into sub-voxel shapes, Vintage Story style, so
   building is not limited to cubes, slabs and stairs.
2. **Functional blocks** (chests, furnaces, crafting tables, machines) that
   hold state, tick, and show that state in the world.
3. **Freedom *and* functionality together:** a functional block should not
   become a dumb cube; a shaped block should be allowed to keep its function.

Reference, Vintage Story (wiki, verified for 1.20): a chisel divides a 1 m
block into **16×16×16 voxels** (6.25 cm). Chiseling a placed block switches it
to a "chiseled" state held by a *block entity*; several materials per block
(added one at a time); tool modes 1/2/4/8 voxels, rotate, flip, copy
(creative), rename; chiseled blocks drop themselves and are not craftable.
Liquids pass only once a layer between two faces has >32 of its 256 voxels
removed. Functional blocks (firepits, tool racks) **cannot** be chiseled.
Chunks store a solid layer, a fluid layer, a **sparse map of block entities**
keyed by position, and a separate sparse **decor layer** keyed by position +
face (paint, plaster, cave art on a 16×16 face grid). Block entities save
through tree attributes and tick on listeners (~50 ms).

Constraints of ours: 0.5 m voxels (ADR 0004), godot_voxel LOD terrain with a
blocky mesher, a native material simulation that sees one material per voxel,
server-authoritative world with deltas over the wire, Android later.

## Options considered
1. **Finer world voxels** (whole world at 6.25 cm): 512× the voxel count;
   generation, meshing, simulation and saves become impossible on the budget.
2. **Enumerate shapes as library models** (slabs, stairs, posts…): fine for a
   fixed set, but "every voxel chiselable" is 2^512 shapes per material set.
   Kept only as a convenience: common shapes may still be library models.
3. **Side store of sub-voxel grids** (Vintage Story's chiseled block entity):
   the terrain voxel becomes a marker id, the shape lives in a sparse map and
   is meshed by our own code into an overlay mesh. Cost scales with the number
   of chiseled voxels, which is what players actually build.
4. Grid per voxel: 4³ (12.5 cm, today's vegetation grid), **8³ (6.25 cm, the
   Vintage Story granularity)**, 16³ (3.1 cm, 4096 cells, 8× the data and
   mesh cost of 8³ for detail below texture-pixel size).

## Decision
**Three layers per chunk, Vintage Story style:**

1. **Terrain layer** – godot_voxel voxel ids, exactly as now. Material/block
   ids are 16-bit and follow `materials.gd` definition order (model id =
   definition index + 1). This layer is what the generator writes, the LOD
   mesher draws, and the simulation copies.
2. **Block-entity layer** – a sparse map `Vector3i -> entity`, grouped per
   16³ chunk, saved with the world, server-authoritative, synced as deltas.
   An entity has a block kind and a list of **behaviors**: `Container`
   (chest), `Process` (furnace: heat, recipe progress), `Chisel` (shape),
   `Multiblock` (one control block for a large machine), `Name`. Block kinds
   are **data** (`game/data/blocks.json`, schema + tests, like items and
   recipes): model (cube | micro pattern | entity-driven), behaviors,
   `chiselable`. GDScript orchestrates entities (inventories and UI are game
   logic); per-cell work is C++.
3. **Decor layer** – later: a sparse map `position + face -> decor` for
   paint, plaster, moss, cave art. Reserved here so nothing assumes faces are
   bare.

**Chiseling** is the `Chisel` behavior:
- **8×8×8 cells per voxel = 6.25 cm**, the same absolute granularity as
  Vintage Story. One material id per cell; up to 16 materials per voxel
  (palette + 4 bits per cell = 256 B + palette on disk and on the wire; 512 B
  of palette indices in RAM). Any solid cubic material may be chiseled.
- The terrain voxel of a chiseled position holds the reserved id
  **`CHISELED`**: an empty, non-culling library model, so the terrain behind
  it still renders and the simulation treats it as a solid.
- **One native micro mesher** (`MicroMesher` in the technosim extension)
  turns a cell grid into a mesh: greedy quads per voxel face, UVs that keep
  the material's tile aligned with the terrain (each cell shows its 1/8 of the
  tile). It builds **both** the library micro models (vegetation, and later
  slabs, posts, machine parts) and the chisel **overlay meshes**, one
  `MeshInstance3D` + trimesh collision per terrain mesh block, rebuilt on
  edit. Overlays live outside the terrain LOD: they are visible while their
  chunk's entity data is loaded (a radius in metres, default 256 m), beyond
  that chiseled voxels are empty. Revisit with a "majority material" fallback
  id bank if distant builds look bitten.
- Tool modes as in Vintage Story: 1/2/4/8-cell brush, rotate, flip, copy
  (creative), name. Chiseling a full voxel first expands it to 512 cells of
  its material. A chiseled block drops itself as an item carrying its data.
- **Freedom and functionality (beyond Vintage Story):** shape and behavior
  are independent. A chest or furnace may be chiseled and keeps its
  `Container`/`Process` behavior as long as at least 1/8 of its cells remain;
  interacting with any remaining cell opens it. Block kinds may still opt out
  (`chiselable: false`) where the shape *is* the function (e.g. a pipe).
- **Simulation:** a `CHISELED` voxel is solid to the sim. Liquids and gases
  pass only along an axis whose every 8×8 layer has more than half its cells
  open (the Vintage Story rule scaled). When the sim destroys the voxel
  (explosion, fire of a burnable majority material) the entity is dropped
  and its items spill. Sub-voxel fluid is out of scope.
- **Mining / raycasts** hit the terrain voxel first and then, for `CHISELED`
  or an entity, refine against the cell grid in C++.

## What must stay true from now on (so the future stays open)
- Voxel ids stay 16-bit and data-ordered; `CHISELED` is reserved in
  `materials.gd` from this ADR on.
- All sub-voxel geometry goes through `MicroMesher` on the 8³ grid; no
  second mesh-building path (the GDScript SurfaceTool model builder is
  replaced).
- No system may assume a voxel is a full cube: anything that reads a voxel
  for gameplay (mining, placing, collision queries, the sim write-back) must
  go through `World` so entities can be consulted.
- The save format carries the entity layer per chunk next to godot_voxel's
  voxel data and has its own version number, separate from `GEN_VERSION`.
- Functional blocks (ADR 0007: crafting table, furnace, chest) are voxel ids
  + entities in this one layer, never special cases in the terrain or the
  mesher. ADR 0007's open points (facing, saving entities) are solved for
  the layer as a whole, so chiseled and station entities share them.

## Consequences
- Builds become unlimited at 6.25 cm; cost is proportional to chiseled
  voxels. P6 (`prototypes/p6-chisel/README.md`) measures the mesher.
- One more layer to save, sync and test; entity ticking needs a budget
  (Vintage Story: 50 ms listeners) and a per-chunk dirty list for deltas.
- Distant chiseled builds are the known gap (overlays only near the player).
- Vegetation gets the finer grid for free (smaller flowers, 6 cm blades).
- The decor layer is reserved but not built; paint/moss wait for Phase 3.
