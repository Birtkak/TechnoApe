# P2 – Voxel chunk streaming benchmark

**Questions:**
1. Which language can carry voxel hot loops in Godot: GDScript, C#, or native?
2. Is godot_voxel (C++ GDExtension) good enough for streaming and LOD, or
   do we write our own?
3. Can we see very far (LOD) without the frame rate or memory collapsing?

**Machine:** Windows 11, RTX 3070, 32 GB, Godot 4.7.2 .NET, godot_voxel
master GDExtension build from CI run 36898667256 (2026-10-01).

## Part A – generate + mesh throughput, headless, single thread

256 chunks of 16³, heightmap + 3D-noise caves, naive culled mesher, identical
algorithm in both languages (same face count: 231,718). Run twice each.

| Language | gen ms/chunk | mesh ms/chunk | upload ms/chunk | chunks/s |
|---|---|---|---|---|
| GDScript (typed) | 0.81 | 3.96 | 0.20 | 200 |
| C# (Debug) | 0.44 | 0.28 | 0.28 | 1000 |
| C# (optimised) | 0.44 | 0.27 | 0.27 | 1020 |

- Meshing (pure array work) is **14× faster in C#**. Generation is only 2×
  because both pay for engine noise calls.
- Native C++ would be faster still; godot_voxel's mesher is the native
  reference in Part B.

Commands:
```
godot_console.exe --headless --path . --script res://bench_gd.gd
dotnet build p2-voxel-benchmark.csproj && godot_console.exe --headless --path . res://bench_cs.tscn
```

## Part B – rendered streaming, camera flight, vsync off

Frame time stats over the run; `max` is always the first frame (scene setup).

| Test | radius / view | speed | avg ms | p95 | p99 | RAM MB | notes |
|---|---|---|---|---|---|---|---|
| GDScript streamer, WorkerThreadPool | 8 ch (128 bl) | 24 | 0.89 | 1.30 | 1.39 | 70 | 135 chunks/s uploaded |
| GDScript streamer | 16 ch (256 bl) | 60 | 2.59 | 4.55 | 7.58 | 95 | 526 chunks/s, keeps up |
| godot_voxel VoxelTerrain, native gen | 8 ch | 24 | 0.69 | 1.04 | 1.21 | 95 | |
| godot_voxel VoxelTerrain, **GDScript generator** | 8 ch | 24 | 0.69 | 1.04 | 1.21 | 108 | hybrid is free at this scale |
| godot_voxel VoxelTerrain, native gen | 24 ch (384 bl) | 60 | 1.48 | 3.33 | 6.03 | 244 | |
| VoxelLodTerrain blocky, **octree**, 5 LOD, ld 64, view 512 | 8 ch | 24 | 0.80 | 1.18 | 1.39 | 238 | |
| VoxelLodTerrain transvoxel, octree, 5 LOD | 8 ch | 24 | 0.79 | 1.17 | 1.39 | 238 | smooth, for reference |
| VoxelLodTerrain blocky, octree, 6 LOD, ld 128, view 1536 | 24 ch | 60 | 5.30 | 16.7 | 25.0 | 751 | main thread: detect_required_blocks 4 ms/frame |
| VoxelLodTerrain blocky, octree, 7 LOD, ld 192, view 2048 | 32 ch | 120 | 25–35 | 31–40 | 33–42 | 1060–1419 | 3882 blocked LODs, mesh drops |
| VoxelLodTerrain blocky, **clipbox**, 6 LOD, ld 128, view 1536 | 24 ch | 60 | **0.84** | 1.25 | 2.08 | **95** | 0 blocked |
| VoxelLodTerrain blocky, **clipbox**, 7 LOD, ld 192, view 2048 | 32 ch | 120 | **0.90** | 1.39 | 2.78 | **96** | 0 blocked |

Screenshots in `out/`: `stream_gd.png` (GDScript streamer, caves visible),
`stream_voxel_terrain_native.png`, `stream_voxel_lod_native_blocky_clipbox.png`.

## Measured answers

1. **Language:** GDScript is fine as orchestration (a threaded GDScript
   streamer kept up at 526 chunks/s), but any per-voxel loop must be native
   or C#. Given Android (C# export experimental) the hot-loop language is
   **C++ (GDExtension)**; C# stays a fallback for tooling.
2. **godot_voxel:** production-grade. Blocky mesher works in the LOD terrain,
   a GDScript generator plugs in with no measurable cost, and the API has a
   block-level write (`VoxelTerrain.try_set_block_data`) that a native
   simulation can feed.
3. **Far view:** yes, with **`streaming_system = CLIPBOX`**. The default
   octree streamer collapses beyond ~1 km (main-thread cost, memory, blocked
   LODs). Clipbox at 2 km, 7 LODs: sub-millisecond frames, 96 MB.
   Note `lod_distance` sets the full-detail radius (192 blocks here); beyond
   that, detail halves per LOD ring. Player edits inside LOD rings need
   `full_load_mode` (to test in P5).

## Caveats
- Single-colour untextured blocks; no entities, physics, lighting beyond one
  shadowed sun. Real scenes cost more; the ordering of results will hold.
- godot_voxel build is a master-branch CI artifact, not a tagged release;
  pin a tag before Phase 2.
- `--import` with the extension loaded segfaults in `--headless` once; the
  subsequent runs were fine. Watch this in CI.
- The dev PC is far above the Android target; re-run on a phone in P5/P6.

## Still open (P5)
- Falling-sand simulation kernel: cells/ms in C++ vs compute shader; how
  changed blocks flow into VoxelTerrain / VoxelLodTerrain and how fast they
  re-mesh.
- Does godot_voxel run in a headless dedicated-server export?
