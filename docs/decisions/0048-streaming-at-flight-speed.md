# ADR 0048: Streaming at flight speed, and saving what is still loaded

Status: accepted (2026-10-25)

## Context
Playtest 2026-10-25, flying at the default 20 m/s: "lod" (a bush a few metres away drawn with 0.5 m
voxels, 3.9 m above the ground) and "load way too slow" (33 m up, everything coarse). Measured with a
ghost flight 3 m above a forest (`autopilot=1 speed=20 alt=3 follow_ground=1 target=forest`, new: it
moves the player itself through trees and hills) and a new JSON field `detail` (how often the ground
18 m around the player had no LOD 0 mesh):

| 20 m/s | queued tasks (mean) | detail missing | p99 |
| --- | --- | --- | --- |
| no save (benchmarks until now) | 51k generation | 0-1 % | 8.8 ms |
| with the SQLite save (play) | 48k, 87k streaming peak | 10 %, up to 0.75 s | 15.1 ms |

Generation itself is cheap (0.16-0.45 ms a 16^3 block). Play differs from the benchmarks: every
block of every LOD is first asked from the save as a *serial* I/O task, and while one runs every
worker looking for work walks past the queued ones. 33 m up is simply above the 24 m detail box.

While testing a save round trip: terrain edits were written only when their blocks unloaded. The
game never called `save_modified_blocks`, so save-and-quit (and the autosave) lost everything dug or
built near the player.

## Decision
1. **Skip the save for blocks it never held** (`voxel_patch_stream.py`): `VoxelStream.may_have_block`
   (default true); `VoxelStreamSQLite` answers from its key cache once loaded; `request_block_load`
   generates such a block at once, as without a stream. Saved blocks still load through the stream.
2. **Deep rock is one fill** (FineGen, `set_uniform_blocks`): from LOD 1 on there are no ores and
   caves only reach the cave mouths, so a block wholly 24 m+ under the lowest carved surface of its
   columns is stone; FineGen returns one voxel and `TerrainGenerator` fills the buffer (stored
   uniform). Probes and tests keep the full arrays (off by default).
3. **Clutter stamps only at LOD 0** (they are not drawn coarser): the LOD 1-2 cost returns to what it
   was before ADR 0047.
4. **`lod_vertical_ratio`** (`voxel_patch_vertical.py`, Settings): LOD 1+ boxes flatter than cubes.
   0.35 cut the 20 m/s backlog ~10x but from 120 m up the trees under the player fell out of the
   detailed rings; it stays at 1.0 (cubes), the knob kept for later.
5. **Save what is loaded**: `main.save_terrain()` flushes the sim bubble and calls
   `save_modified_blocks` (~9 ms on the main thread: it walks every loaded block); quitting and
   quitting to title wait for it (5 s at most), the 30 s autosave runs it only when the terrain was
   written since (`World.edits`). `probe_voxels=` prints voxels after load for round-trip checks.

## Measured after (20 m/s, with the save)
Streaming queue 0, generation mean 11k, detail missing 0 %, p99 8.3 ms headless / 5.7 ms windowed;
default autopilot budget_ok with load 10.2 s (was ~11.6 s). Still open: a few 19-25 ms main-thread
frames during fast flight (seen before these changes too), and the 24 m detail box when flying high.
