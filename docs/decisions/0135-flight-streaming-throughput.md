# ADR 0135: Flight streaming throughput: a cheaper blocky mesher, folded cave columns, 8 voxel workers

Status: accepted (2026-11-04). No look change, GEN_VERSION unchanged. Builds on ADR 0105 (one box, no LOD), 0106
(merged faces), 0107 (AO pages), 0108 (worker parking, upload order).

## Context
Bug report 2026-11-03_23-05-16: flying fast in creative, "the world still loads slow ... lots of moments where you can
see through the world". Nothing measured what limits the box filling in flight, so first a probe:
`stream_probe=1` (debug_capture.gd) checks 4 m ground patches on an 8 m grid out to the render distance every frame
and logs once a second the holes per 24 m band, the queues, and worker time per task kind and mesher phase
(`voxel_patch_throughput.py` counters). `tools/stream_bench.sh` flies 20 s at the default 72 m render distance,
spectator, follow_ground, from the report's spot (seed 1935341982, voxel 1148 1877 5955: cliffs, savanna, forest,
beach) and waits for a machine without other Godot runs; medians of 3 runs, A and B interleaved.

Before (6 workers, the godot_voxel default of half the cores): at 20 m/s the workers were 91 % busy, meshing
2.64 worker-s/s against generation 2.44. Of ~540 meshes a second ~380 were uniform (free); ~160 surface blocks cost
16 ms each, 9.5 ms in the voxel loop and 6.1 ms in the merge pass, which scanned 6 x 76^3 mask cells to find ~3 000
marked faces. With meshing fixed, generation was 61 % of worker time at 40 m/s with the workers saturated; within it
FineGen's cave prep was 28 %.

## Decision
- **Mesher** (`voxel_patch_mesh_speed.py`): the merge pass walks per-side lists of the faces it marked, sorted into
  the old scan's order, and the mask is only grown; a plain opaque cube between six plain cubes is skipped before the
  six visibility checks. Same triangles (`tools/mesh_hash.gd`: identical hash, 14.7 -> 7.9 ms per block). Merge
  886 -> 27 worker-ms/s, loop 1467 -> 875, a surface block 16 -> 6.7 ms.
- **Cave columns** (fine_gen.cpp): at LOD 0 the cave band caches the folded value (cave, hall, ravine) per row, and
  the hall / ravine columns are computed only when a band needs them. gen_hash identical over 4 000 blocks; generation
  ~9 % cheaper per block.
- **Workers**: `Settings.voxel_threads` = cores - 4 (8 of 12 on the dev box), set in main.gd; the loading cover still
  raises it and parks back (ADR 0108). 10 workers: few fewer holes, higher per-block cost, p99 5.35.

## Results (land path, medians of 3, no other Godot)
| run | holes 0/24/48 m band | gen queue | p99 ms | max ms | near_unloaded |
|---|---|---|---|---|---|
| before 20 m/s | 2.6 / 16.6 / 59.6 | 86 k | 6.13 | 19.45 | up to 8 |
| after 20 m/s | 0 / 2.95 / 12.8 | 4.5 k | 5.14 | 16.36 | 0 |
| before 40 m/s | 4.9 / 45.7 / 114 | 38.8 k | 4.75 | 13.35 | up to 8 |
| after 40 m/s | 5.55 / 19.45 / 42.2 | 14.7 k | 5.46 | 16.98 | 0 |

Sea path (earlier spawn): 20 m/s holes 0/0/9-13 -> 0/0/0; 40 m/s 1/47/111 -> 0/0/11. Persist (save path): p99 5.3,
holes 0/1/7.5. Filmstrips: `game/out/stream_fill_before_after.png` (not tracked; rebuild with stream_bench.sh).

## Consequences
- The box keeps up at 20 m/s and the 24-48 m bands fill 2-3x faster at 40 m/s; p99 stays far under 8.3 ms.
- Open: the worst frame is ~16-17 ms in some runs (render-thread frames once the box is full on the sea path, one
  81 ms physics frame at settle with persist, one 383 ms frame at a run start); the update task's data pass
  (`ta_u_data`) takes 55-125 ms, a latency for the next round; ~5 holes in the 0-24 m band at 40 m/s remain.

## Round 2 (2026-11-04): cave-free skips, the mountain path
stream-forensics (ADR 0136) put 95 % of the visible holes on blocks still generating: a 20 m/s mountain fly at 6 m
asks for ~30 k blocks/s. `game/tools/gen_bench.gd` generates the blocks of that lane as the shell asks for them
(ground_range over the 9 x 9 columns, -4 m / +64 m, whole mesh blocks) with a voxel hash and thread CPU time;
`prof=1` samples call stacks (`prof_sampler.cpp`, `game/tools/gen_prof.py`). Inclusive shares of a block before:
pool capping 11 % (run in every column, cave or not), the per-voxel cave mask 11 %, cave bands 12 %, `cave_at` 5 %,
stringer noise 5 %, libm `floor` calls 7 %, the per-column clock reads of the gen_stats timers 2.5 %.

- **Exact skips** (fine_gen.cpp): the cave field is only ever read as convex mixes of its lattice values, so a block
  with no positive value has no cave voxel: the mask, `cave_at`, pools, crystals and stringers are skipped there; in a
  column, a voxel between two rows whose four corners are all <= 0 skips the interpolation; pools and crystals run
  only in a column with cave air in the block. The per-column timers are gone (block phases remain).
- **`-msse4.1`** (SConstruct): `std::floor` becomes one instruction; every CPU Godot 4.5+ runs on has it.
- Same voxels: gen_bench lane hash `3326b1f6` and gen_hash over 6 000 blocks (two seeds, LOD 0-3) unchanged.
  Generator CPU per block on the lane 123 -> 88 us (median of 3, sky blocks included), -28 %.
- Re-measured on main 7427926 (ADR 0139 smaller veins, ADR 0140 highland moor, GEN_VERSION 36): main's generator
  vs this branch, same lane hash `4a0fa13c` and gen_hash on two seeds; CPU per block 101 -> 60 us (median of 3, both
  rounds together, -40 %).
- `build_quick.sh` (p5): SCons on Windows re-archives godot-cpp one object at a time when it thinks the library is
  stale (20-30 min); the script compiles the named sources and links in under a minute.

Mountain path (`stream_bench.sh <label> alt=6`, before = main DLLs at 6 workers, after = both rounds at 8; medians of
4, **2-4 other Godot runs active in every run**, so frame times are not valid here: p99 11-15 ms for both):

| run | holes 0/24/48 m band | gen queue | near_unloaded |
|---|---|---|---|
| before | 3.3 / 52 / 120 | 128 k | 344 |
| after | 0.9 / 10.6 / 42 | 13.6 k | 0 |

After, the queue drains to near 0 about 9 s into the flight and the workers then have idle time; task pickup is
1-2 % of worker time, so the priority refresh over a full queue that stream-forensics suspected does not show at
this queue size. Filmstrip: `game/out/stream_fill_mountain_before_after.png` (left before, right after).
Open: the frame budget on a quiet machine with 8 workers on this path; cave band builds (12-17 %, real cave work)
are the next generator cost.
