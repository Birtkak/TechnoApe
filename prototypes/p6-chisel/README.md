# P6 – Chisel spike: one native micro mesher for every sub-voxel shape

**Question (ADR 0006):** can one greedy micro mesher on an 8×8×8 cell grid per
voxel (6.25 cm, Vintage Story granularity) build both the library micro models
(vegetation, slabs, machine parts) and chisel overlay meshes for whole 16³ mesh
blocks cheaply enough, with texture tiles staying aligned to the terrain?

**Code:** `prototypes/p5-sim-spike/src/micro_mesher.{h,cpp}` (`MicroMesher`,
static `build`, `build_arrays`, `fill_box`, registered in the technosim
extension). Bench + sanity checks: `game/tools/chisel_bench.gd`:

```
Godot_..._console.exe --headless --path game --script res://tools/chisel_bench.gd
```

**Measured 2026-10-22 (Ryzen 5600X, template_debug MinGW build, best of 5):**

| case | cells | quads | mesh | trimesh shape |
|---|---|---|---|---|
| one chiseled voxel (cube minus a 4×6×4 bite) | 512 | 14 | 0.04 ms | 0.04 ms |
| typical fully chiseled 16³ block (2/3 voxels solid, 1–3 bites each, 2 materials) | 2.1 M | 51 166 | 118 ms | 123 ms |
| 3D checkerboard of cells (pathological worst case) | 2.1 M | 6.3 M | 2 660 ms | 22 700 ms |

Sanity: full voxel = 6 quads, two adjacent voxels = 10 quads (shared face
culled, no merging across the voxel border), top face UV spans exactly one
atlas tile, all triangle windings face their normal.

**Answer:** yes.
- A single voxel edit costs tens of microseconds; the mesher is not the
  bottleneck for chiseling, library models or vegetation patterns.
- A whole 16³ block solid with chiseled voxels is ~120 ms mesh + ~120 ms
  collision shape: fine on a worker thread, too slow on the main thread. The
  overlay rebuild must run on the thread pool (like terrain meshing) or keep
  one sub-mesh per chiseled voxel and only rebuild the edited one. Real builds
  are far sparser than this case.
- Quads never cross voxel borders by design, so each cell shows its 1/8 of the
  material tile and chiseled stone stays aligned with neighbouring stone.
- The checkerboard row is the reason to cap cell edits per voxel per tick on
  the server, not a real risk from players.

**Done the same day:** `materials.gd` builds every micro model through
`MicroMesher` on the 8³ grid (`micro_pattern`, `micro_paint_tiles`), the
`CHISELED` id (44) is reserved, and `game/tools/model_sheet.gd` renders all
models to one PNG for checking. ADR 0006 is reconciled with ADR 0007: the
station block entities are the entity layer the chisel joins.

**Next (not in this spike):** the `Chisel` block-entity behavior, overlay
`MeshInstance3D` + trimesh per mesh block on a worker thread, chisel tool
modes, save format for the entity layer.
