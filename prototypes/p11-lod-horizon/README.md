# P11 – Seamless LOD horizon: how far can godot_voxel carry detail, and can LOD switches be hidden?

**Question.** The user wants seamless travel: no visible pops when terrain
blocks change LOD, detail further out, and a clean horizon. Three knobs sit
inside godot_voxel rather than the game: the LOD0 radius is clamped to
128 voxels (64 m), the clipbox ring size (`secondary_lod_distance`) is
clamped the same way, and the LOD cross-fade copies every fading block into a
new mesh instance plus a per-block ShaderMaterial (its own comment: freeing
one such instance can cost 1 ms). Measure, with the stock extension and a
custom build of the same commit (045ff932), what each buys and costs.

**Method.** Flight at 20 m/s for 16 s after load (`autopilot=1 duration=16
speed=20 alt=30 vsync=0`, 1280x720, separate render thread), ADR 0014
measurement, one run at a time. Screenshots from the far pose
(`speed=0 alt=40 pitch=-0.05`, 1920x1060).

Custom build: `~/Tools/godot_voxel` at 045ff932, godot-cpp 4.5 branch, MinGW:
```
GODOT_CPP_PATH=$HOME/Tools/godot-cpp python -m SCons platform=windows target=editor use_mingw=yes use_static_cpp=yes -j12
```
Patches (`voxel_patch.py`, then `voxel_patch_budget.py`, both in this folder):
`MAXIMUM_LOD_DISTANCE` 128 → 1024; fading copies on transition-mask changes
only when the shader has `u_transition_mask` (blocky meshes never stitch);
the fade value as an instance shader parameter when the terrain has no
ShaderMaterial; `mesh_upload_budget_mb`, a per-frame cap on applied mesh data
(updates over the cap wait a frame, the newest per block wins);
`voxel_patch_remesh.py`: `edit_remesh_rate`, a token bucket over the remeshes
that edits request (streaming is never limited), the rest stay pending.
It also keeps the pending list's indices right when blocks leave it (the
2026-10-25 access violation). `voxel_patch_unloaded.py`, `voxel_patch_fluid.py`
(ADR 0045); `voxel_patch_vertical.py`: `lod_vertical_ratio`, flatter LOD 1+
boxes; `voxel_patch_stream.py`: `VoxelStream.may_have_block`, blocks the SQLite
key cache never saw skip the serial I/O queue (ADR 0048).

The editor target does not compile with this GCC (16.2: template deduction
error in `editor/noise/chart_view.cpp`); `target=template_debug` does, and
`voxel.gdextension` points `windows.debug` at that DLL (the game never needs
the editor classes). Output: `project/addons/zylann.voxel/bin/`.

## Results

### Ring width (stock extension, LOD0 = 64 m, view 1024 m, generator with far canopy cells)
Flight 20 m/s, 16 s after load, 1280x720, separate render thread.

| `lod_ring_m` | LOD1/2/3/4 end (m) | load s | p50 | p99 | max | > 8.3 | backlog max / mean | draw calls | prims | RSS MB | VRAM MB |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 24 (godot_voxel default) | 112 / 160 / 256 / 448 | 2.8 | 1.77 | 2.71 | 6.6 | 0 | 3,318 / 168 | 430 | 3.2 M | 1,077 | 454 |
| 48 | 160 / 256 / 448 / 832 | 4.3 | 2.04 | 3.10 | 8.6 | 1 | 4,576 / 354 | 594 | 4.6 M | 1,191 | 613 |
| 64 | 192 / 320 / 576 / 1088 | 6.2 | 2.17 | 2.98 | 17.6 | 6 | 5,972 / 298 | 618 | 5.1 M | 1,305 | 801 |
| 64 + `terrain_threaded_update=1` | same | 5.9 | 2.15 | 2.96 | 9.3 | 1 | 5,867 / 277 | 622 | 5.3 M | 1,311 | 801 |

Wider rings are cheap per frame (+0.4 ms p50 at 64 m) but the initial load
doubles and the clipbox bursts grow (max backlog 3.3k → 6k blocks). Screens:
`game/out/ring_24`, `game/out/ring_64` (same far pose).

### Custom build: LOD0 radius and the cross-fade (ring 24 m unless noted)

| build / settings | load s | p50 | p99 | max | > 8.3 | > 16.7 | backlog max / mean | draw calls | prims | RSS | VRAM |
|---|---|---|---|---|---|---|---|---|---|---|---|
| stock CI DLL | 2.8 | 1.77 | 2.71 | 6.6 | 0 | 0 | 3,318 / 168 | 430 | 3.2 M | 1,077 | 454 |
| MinGW build, unpatched | 2.9 | 1.77 | 2.76 | 6.6 | 0 | 0 | 2,897 / 170 | 406 | 3.1 M | 1,067 | 454 |
| patched, no fade | 2.8 | 1.69 | 2.29 | 6.9 | 0 | 0 | 3,881 / 175 | 420 | 3.2 M | 1,074 | 454 |
| patched, fade 0.35 s | 2.8 | 1.73 | 2.54 | 6.8 | 0 | 0 | 3,266 / 153 | 416 | 3.2 M | 1,072 | 454 |
| stock, fade 0.35 s (ADR 0017 run) | 2.9 | 1.89 | 9.42 | 61.6 | 90 | – | 4,514 / 241 | | | 1,079 | |
| patched, LOD0 = 128 m | 8.8 | 3.15 | 10.07 | 47.5 | 56 | 17 | 10,854 / 1,412 | 1,169 | 16.4 M | 1,430 | 2,069 |
| patched, LOD0 = 128 m, fade | 8.8 | 3.18 | 11.09 | 54.6 | 67 | 21 | 11,113 / 1,535 | 1,206 | 16.6 M | 1,422 | 2,069 |
| patched, LOD0 = 128 m, ring 64, threaded update | 10.0 (unsettled) | 3.00 | 8.47 | 39.4 | 62 | 15 | 13,798 / 2,340 | 1,257 | 14.3 M | 1,668 | 2,263 |

- The MinGW build behaves like the CI DLL.
- Skipping the fading copies on transition-mask changes (never needed by a
  blocky mesher) turns the fade from 90 frames over 8.3 ms and a 61 ms spike
  into noise: p99 2.54 vs 2.29 ms.
- LOD0 at 128 m is eight times the full-detail volume: 16 M primitives, 2 GB
  VRAM, p99 10 ms, 9 s load. Not an option on this machine; detail further out
  comes from the rings and the generator's LOD policy instead.
- Without `terrain.material` (no per-block ShaderMaterial): 376 draw calls
  instead of 420, and the liquid shader runs on the LOD terrain for the first
  time (water was drawn opaque by the terrain shader override).

### Decision (ADR 0020)
Rings 48 m (64 m gives 20–27 ms frames at 40 m/s flight), fade 0.35 s,
threaded update on, no `terrain.material`, our extension build. The budget
matrix on those defaults is in the ADR; `game/out/mx_d2_*` holds the runs.
The fade is visible as a dithered edge in `game/out/fade_vis2/zoom_far.png`.

### Later the same night (see ADR 0020 for the numbers)
- 2 km horizon at 7 LODs costs nothing over 1 km; an 8th LOD took 22.6 s to
  load until the generator's hydrology tile cache was fixed (4.4 s after).
- LOD0 96 m: fine at 20 m/s, a 19.5 ms frame at 40 m/s. Not default.
- Mesh upload budget at apply time: within noise on the water spill.
