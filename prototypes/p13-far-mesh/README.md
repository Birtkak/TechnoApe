# P13 – A far layer like Lay of the Land's: which far mesher, at what cost?

**Question.** ADR 0073 left the world as one 40 m box of full-detail voxels ending in the sky, because
three attempts at voxel LODs with godot_voxel's blocky mesher looked worse than nothing at 12.5 cm
(cubes of cubes, black water trenches, terrace rims, a dither that read as stitching, streaming that
lost a 20 m/s flight). Lay of the Land draws terrain to the horizon with no visible transition
(`docs/research/2026-10-26-lay-of-the-land-distance.md`). Which far mesher gives us that look, what
does it cost next to the 40 m box (`game/out/nolod/stand_40.json`, `fly_40.json`: p99 6.2 / 5.2 ms,
0.9 GB VRAM, load 4.2 s), and does the near box blend into it without a seam?

**Candidates** (hand-off `docs/plans/lotl-mesh-lod-handoff.md`):
- (b) a column height-field skin from the generator's macro lattice, CDLOD rings that morph into
  their parent, flat water sheets, instanced trees, a baked canopy beyond: native `FarMesh`
  (`prototypes/p5-sim-spike/src/far_mesh.cpp`), `game/scripts/visuals/far_layer.gd`, shaders
  `far_ground / far_water / far_tree.gdshader`. Composited behind the near box through its own
  SubViewport, so the near terrain's horizon blend (ADR 0073) fades per pixel into the far layer.
- (a) a smoothed surface (surface nets) over FineGen's coarse voxel data with palette-average vertex
  colours.
- (c) godot_voxel's transvoxel on an SDF channel: not spiked, see the ADR for why.

**Method.** The game itself with `far=1` (`Settings.far_layer`, `far_m=` the reach): the ADR 0072
report pose (seed 272905368, `autopilot=1 speed=0 alt=40 pitch=-0.05`), the altitude pose (`alt=100`),
the budget runs of ADR 0073 (`walk=1 speed=0 duration=20`, `speed=20 alt=30 duration=16`, `vsync=0`,
1280x720, ADR 0014 measurement, no other Godot process). Run JSON key `far` holds the layer's own
numbers (tiles, build ms, triangles, trees). Results in `game/out/far/`.

## Results

### Look (the developer judges; `game/out/far/compare_*.png`, left = the 40 m box alone, right = with the far layer)
- Report pose, 40 m up (`compare_pose.png`): the box alone shows haze and nothing else; the skin shows the
  forest as a field of crown blobs with trunks, the clearing's grass, the river's banks, trees to the horizon.
- 100 m up (`compare_high.png`): forest, river and the coast to the horizon with their shapes; the hills
  behind in the haze.
- Ground level on the beach (`compare_seam.png`): the near sand runs on into the far sand with no line,
  ring or pop at the box's edge; the sea beyond as a flat sheet.
- Surface nets (`pose_nets2/final.png`): lumpy blobs whose colours average leaves, wood and ground, every
  surface doubled by its water skin in the first try; after the fix still 3-4x the triangles.

### Cost (seed 272905368, 1280x720, vsync off, ADR 0014, no other Godot process; `game/out/far/*.json`)
| run | 40 m box (`out/nolod`) | + height-field skin, 1.5 km | + surface nets |
|---|---|---|---|
| report pose alt 40: p99 / max / VRAM | 3.5 / 14.7 ms / 0.56 GB | 4.6 / 9.7 ms / 0.86 GB | 6.8-10.6 / 12-62 ms / 1.1-1.2 GB |
| 100 m up: p99 / max | 2.9 / 7.4 ms | 3.7 / 7.8 ms | - |
| standing 20 s: p99 / max / VRAM | 6.2 / 8.8 ms / 0.9 GB (today 5.0 / 6.8) | 7.2-7.6 / 8.3-12.4 ms / 1.04 GB | - |
| flight 20 m/s 16 s: p99 / max / > 16.7 | 5.2 / 5.7 ms / 0 (today 4.9 / 16-31 / 0-2) | 8.0-11 / 50-90 ms / 14-25 | - |
| first load standing | 4.1 s | 4.9-7.2 s | - |
| tiles / triangles / trees | - | 259 / 2.6-3.0 M / 4.4 k (96 m rings: 634 / 7.5 M) | 634 / 18-26 M / 0 |
| tile build avg / max (worker thread) | - | 14 / 250 ms (6 / 30 before the level-independent slope) | 43-45 / 275-317 ms |

What it took to get there, in order: the ground and water quads wound counter-clockwise (Godot culls
them: the first shot showed the sky's ground band through floating trees); trees built per instance in
GDScript (30 ms frames in flight: now native MultiMesh buffers); a stale-tile scan quadratic in the
tiles (now the quadtree's ancestor / children check, pruned by range); meshes freed on a level change
(RenderingServer frees block on the render thread: now a node pool); the slope that picks stone and the
canopy depended on the lattice stride, so level 3 and 4 met with a 2-voxel crack and different colours
(`test_far_mesh` caught it; the slope now comes from the base columns two metres apart at every level,
from the lattice where its stride divides that, else four extra column reads per node: the 250 ms of
the 8 m level). Then the flight, which never held: run after run of 30-90 ms frames in one cluster
about 12 s in. Eliminated one by one (`out/far/fly_*.json`): own worker threads instead of the
engine's pool (fly5), 2 low-priority threads (fly9), half the rings (fly8), tiles frozen after load
(fly15), tiles built but hidden (fly16), the far view in its own World3D so its camera stops culling
the voxel world (fly17: the view's render-thread CPU 12-16 -> 1.4 ms), half resolution (fly_half),
every other frame (fly14), a 1.5 MB box upload budget (fly_up15): the cluster stays, 14-25 frames over
16.7 ms. Only never rendering the far view (fly_noview: 1 frame) was clean, once. The control flight
without the far layer today: 0-2 frames over, 16-31 ms max (fly_base1/2), so part of it is the box's
own streaming burst (36-48 MB mesh deltas in a frame at a forest edge) that the far layer's load
turns from absorbed into stalls. Unresolved; next step in ADR 0074.

### Answer
The height-field skin (b): the look the developer asked for at the pose and at altitude, a seamless
ground-level join, one to one and a half milliseconds of frame time standing and 0.15 GB of VRAM;
surface nets (a) cost four to seven times the geometry and build for a worse look; transvoxel (c)
would bring back the streaming and hiding problems of ADR 0073. The 20 m/s flight does not hold the
budget with it on (above). Decided in ADR 0074 (proposed, off by default until the developer prefers
it in play and the flight holds).
