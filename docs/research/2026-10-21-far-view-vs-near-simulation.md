# Seeing far while simulating near

- **Date:** 2026-10-21
- **Question:** how do we let the player see epic distances while a
  Noita-style material simulation runs, without simulating everything, or
  optimise the simulation so well that we can?

## Short answer
Nobody simulates the whole world, including Noita. Every shipped system
separates three radii that we should treat as three independent knobs:

| Ring | What happens | Typical radius | Who pays |
|---|---|---|---|
| **Simulation bubble** | Full cellular-automaton update of materials | ~2–4 chunks (32–64 blocks) around each player, plus "anchors" (machines, fires) | CPU (or GPU) per tick |
| **Live detail** | Full-resolution voxel data loaded, meshed at LOD 0, editable | `lod_distance` (we measured 192 blocks fine) | memory + mesher threads |
| **Horizon** | Coarser LOD rings up to the view distance; static snapshot of the world incl. player builds | 1–2 km (7 LODs measured at 0.9 ms/frame) | GPU + small RAM |

The trick is that the horizon is a *snapshot* of frozen world state, refreshed
only when a chunk changes, and the simulation only runs where something can
be observed changing.

## How the references do it

### Noita (Falling Everything engine, GDC 2019 talk)
- World in 512×512-pixel chunks; only the chunks around the screen are
  simulated, the rest sit on disk frozen. Players never notice because they
  can't see them.
- **Dirty rectangles:** each chunk tracks the bounding box of pixels that
  moved last frame; settled sand and still water cost nothing. Most of a
  frame's pixels are skipped.
- **Multithreading with a checkerboard:** the active area is split into
  64×64 cells updated in 4 passes so that threads never touch neighbouring
  cells at the same time; no locks.
- **Rigid bodies** are cut out of the pixel world (marching squares) and
  handed to Box2D, then rasterised back.
- Lesson: simulation cost is bounded by *active* pixels near the player,
  not by world size.
  Sources: [talk listing](https://noita.wiki.gg/wiki/Official_Interviews_and_Videos),
  [GDC Vault: Exploring the Tech and Design of Noita](https://www.gdcvault.com/play/1025695/Exploring-the-Tech-and-Design),
  [Road to the IGF interview](https://www.gamedeveloper.com/game-platforms/road-to-the-igf-nolla-games-i-noita-i-).

### Distant Horizons / Voxy (Minecraft LOD mods)
- Minecraft simulates only within the server's simulation distance; the mods
  render far beyond that from **LOD data** built by mip-mapping chunk data
  into a hierarchy (Voxy: 5 LOD levels, GPU-driven indirect rendering, LODs
  stored in a local database).
- Far chunks are either generated once for LOD only and never simulated, or
  copied from the real world when it was last loaded. Player builds show up
  in the horizon because the LOD is rebuilt from real chunk data when a chunk
  unloads.
- Lesson: **LOD data is a byproduct of the live world**, generated at
  unload/change time, never simulated.
  Sources: [Voxy overview](https://deepwiki.com/MCRcortex/voxy),
  [Voxy world-gen notes](https://www.mintlify.com/kamp0010/voxy_worldgen_v2/concepts/lod-system),
  [DH vs Voxy comparison](https://gurugamer.com/pc-console/distant-horizons-vs-voxy-the-ultimate-lod-mod-showdown-for-minecraft-26068/amp).

### godot_voxel (our measurements, P2)
- `VoxelLodTerrain` with `streaming_system = CLIPBOX`, blocky mesher, 7 LODs,
  2 km view: 0.9 ms/frame, 96 MB. Octree streaming: unusable at that range.
- `lod_distance` is the live-detail radius; edits beyond it need
  `full_load_mode`, which keeps edited chunks in memory. That is the
  mechanism for "player builds always in view".

### GPU-driven voxel research (Aokana, 2025)
- Sparse voxel DAG + GPU-driven pipeline, "tens of billions of voxels",
  up to 9× less memory and 4.8× faster rendering than prior work.
- Relevant later if we outgrow mesh-based LOD; not needed now.
  Source: [arXiv 2505.02017](https://arxiv.org/abs/2505.02017).

### 3D falling sand on CPU vs GPU
- GPU approaches use **block (Margolus) partitioning** or a request-then-
  resolve pass to avoid two cells writing the same target; CPU approaches
  use dirty rects + checkerboard threads. Both scale with active cells only.
- No published number for "millions of 3D voxels per frame" was found; we
  measure our own in P5.
  Sources: [sands-of-rust (GPU 2D)](https://github.com/wg-romank/sands-of-rust),
  [sandengine (dirty rects)](https://github.com/ARez2/sandengine),
  [3DCellularWorld (CPU/GPU 3D CA)](https://github.com/ccrock4t/3DCellularWorld),
  [Temple TR-18 3D granular CA](https://cis.temple.edu/tagit/publications/TAGIT-TR-18.pdf).

## Proposed architecture for TechnoApe

1. **Authoritative voxel store** (native): chunks of 16³ (or 32³, to test)
   with per-chunk flags: `active` (has moving material), `dirty_mesh`,
   `dirty_lod`, `edited`. Material id + small state per voxel.
2. **Simulation scheduler**: each tick, collect chunks inside any simulation
   bubble (players, powered machines, burning things) that are `active`;
   update them with Noita-style rules in checkerboard passes on worker
   threads (or a compute shader; P5 compares). A chunk with nothing moving
   goes to sleep until a neighbour writes into it or a player touches it.
3. **Settle-on-unload**: when a chunk leaves the bubble, it freezes as-is.
   Optional "fast-forward" (bounded) when it re-enters so sand that was
   falling finishes falling. Noita does neither and nobody minds.
4. **Render feed**: changed chunks are pushed to godot_voxel via block-level
   writes; it re-meshes on its threads. LOD rings refresh when a chunk's
   `dirty_lod` flag is set (debounced), so builds appear on the horizon.
5. **Server**: same store and scheduler run headless; clients run the
   render feed and (optionally) a predictive local sim for responsiveness.
   Deltas only on the wire.
6. **Budgets as settings**: bubble radius, max active chunks per tick,
   ticks per second, `lod_distance`, `view_distance`, LOD count. The Android
   profile shrinks the first three; the horizon stays.

## "Or optimise so well we simulate everything"
Not a goal worth chasing: a 2 km × 2 km × 256 world is ~1 billion voxels;
even at 1 ns per voxel that is a full second per tick on one core, and most
of it would be rock doing nothing. Activity tracking (dirty flags) gives the
same visible result for a tiny fraction of the cost. The right ambition is
**"everything you can see move, moves"**: a large bubble, high tick rate, and
anchors so machines keep running while you are away.
