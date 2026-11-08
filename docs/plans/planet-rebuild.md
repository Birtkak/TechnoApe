# Planet rebuild plan (ADR 0170)

Status: set up 2026-11-08, nothing started. Sessions start from the cards in the 2026-11-08 brainstorm
session; each runs in its own worktree and ends in a prototype folder + README (question, measured answer),
not in changes to `game/`.

## Vision in one paragraph
Small spherical planets (0.3-1 km radius) of 0.5 m blocks you walk around, dig into and build on, with a
seamless climb to orbit and over to a moon. Fine 12.5 cm detail lives only where something happens (sim
bubble, chiseled blocks, felled trees); liquids are fill levels with smooth surfaces, gases a blended field.
One planet + moon first. Old saves break. The engine is a lean rewrite unless Godot proves itself.

## The shared benchmark (every engine spike builds exactly this)
So the numbers compare, both engine spikes implement the same scene and the same JSON output.

- **World:** one planet, radius 500 m, 0.5 m blocks, seeded noise terrain (+-30 m), 3 materials, crust 64 m
  thick (the core below is never meshed). For the benchmark the grid is a plain Cartesian grid with the
  sphere carved into it (the grid spike decides the real one; the cost is the same order).
- **Moon:** radius 150 m, 3 km away, same rules.
- **Runs** (uncapped fps, vsync off, 1920x1080, dev box RTX 3070):
  1. `stand`: 30 s standing on the surface, looking at the horizon.
  2. `fly`: 60 s at 20 m altitude, 60 m/s around the equator (a full lap is ~3.1 km).
  3. `orbit`: from the ground straight up to 3 km, across to the moon, land, back. Seamless, no load screen.
  4. `edit`: 30 s of 2000 block changes/s within 10 m (remesh under churn).
- **Output:** one JSON line per run: `fps_avg`, `frame_p50_ms`, `frame_p99_ms`, `frame_max_ms`,
  `ram_mb`, `vram_mb`, `chunks_meshed_per_s`, `missing_chunks_max` (holes on screen while flying), plus a
  filmstrip of `fly` and `orbit` (CLAUDE.md rule 10).
- **Targets:** ADR 0009 (p99 <= 8.3 ms, max <= 16.7 ms after load, RAM <= 8 GB, VRAM <= 6 GB) in all four runs,
  `missing_chunks_max == 0` in `fly`; stretch: `stand` >= 300 fps.

## Optimization research (both engine spikes, p15 in depth)
The comparison is only honest if each engine is built the best known way. Before writing the renderer, the
spike researches the state of the art (talks, papers, open source voxel engines, their measured numbers) and
records in `RESEARCH.md` what it adopted, what it rejected and why. At least:
- **Meshing:** binary greedy meshing (bitmask per column, the 2023-24 voxel-community technique), face culling
  across chunk borders, meshing on worker threads (rayon) or in compute shaders; measure meshes/s per core.
- **Vertex format:** packed quads (one 8-byte instance per face, expanded in the vertex shader / vertex pulling)
  instead of full vertices; bytes per face and VRAM per chunk recorded.
- **Draw submission:** one big GPU buffer with sub-allocation, multi-draw-indirect, GPU frustum and Hi-Z
  occlusion culling (compute), per-face-direction culling; draws per frame recorded. Mesh shaders if wgpu
  exposes them, else note it.
- **Alternatives to meshing:** ray marching / DDA over a GPU voxel volume or sparse voxel octree / DAG
  (Teardown-style, SVDAG papers), at least a paper comparison with numbers.
- **Data:** palette-compressed chunks, empty/solid-chunk shortcuts (sky and core never stored), cache-friendly
  layout, generation in SIMD-friendly batches; RAM per km of planet recorded.
- **Far planet / LOD:** blocky LOD (coarser blocks per level, never a smooth skin), clipmaps or chunk octrees on
  the sphere, seamless transitions, floating origin; how many triangles a whole planet costs from orbit.
- **Streaming:** priority by distance and view direction, prefetch along the flight direction, budgets per
  frame for upload; how fast you can fly before `missing_chunks_max > 0`.
- **Frame pacing:** pipelined CPU/GPU frames, no stalls on buffer maps, staging-belt uploads.

## Honest numbers (both spikes)
- Release builds (`--release`, LTO, target-cpu=native for the spike; Godot `template_release`), dev box,
  nothing else running, uncapped, vsync off.
- Frame time from both CPU and GPU timestamps; report CPU ms and GPU ms separately.
- 10 s warm-up excluded, each run 3 times, report the median and the worst.
- No shortcuts that do not survive gameplay: no pre-baked meshes, every chunk generated at runtime from the
  seed, the `edit` run must keep its numbers, the planet must be fully diggable.
- An ablation table: each big optimization switched off once, to show what it is worth.
- The same scene, noise, view distance and output in both spikes; differences listed.

## Sessions

### Wave 1 (start together)
| # | Session | Folder | Ends in |
|---|---------|--------|---------|
| 1 | Godot path: profile today + own GPU voxel renderer on the benchmark | `prototypes/p14-godot-planet/` | README + results JSON |
| 2 | Custom engine spike on the same benchmark (Rust + wgpu) | `prototypes/p15-engine-spike/` | README + results JSON |
| 3 | Sphere grid spike: hex prisms vs cube-sphere vs rounded cube | `prototypes/p16-sphere-grid/` | README + recommendation |

1 and 2 are the "lean rewrite" test: custom wins unless Godot meets the targets. 3 is engine independent
(pure math + headless tests + a debug view), so it runs alongside.

#### p15 brief (start a fresh worktree session with this text)
Build `prototypes/p15-engine-spike/`: the best-optimized minimal custom engine you can make in Rust + wgpu,
running the shared benchmark above, so the custom-engine numbers are its honest best.
1. **Research first** (web search, read sources, not memory): every item in "Optimization research", plus
   what the fastest public voxel engines achieve and what wgpu cannot do yet (mesh shaders, bindless) vs raw
   Vulkan/DX12. Write `RESEARCH.md`: technique, links, expected gain, adopt/reject and why.
2. **Build:** Rust under `~\Tools\` (versions in README), winit + wgpu, discrete GPU logged, the benchmark
   exactly as specified, fully diggable, using the adopted techniques.
3. **Measure** by "Honest numbers": release + LTO, CPU and GPU ms, 3 runs per run type, median and worst,
   ablation table, top flight speed with `missing_chunks_max == 0`, filmstrips.
4. **README:** numbers vs the public reference points, and the rewrite cost (UI, audio, input, physics,
   networking, export; reusing `prototypes/p5-sim-spike` and `src/fine_gen.cpp` via FFI or porting).
Done = RESEARCH.md + README + `results.json` + filmstrips on its branch. Stay in the folder; stop after.

### Wave 2 (after wave 1, in the overseer session)
- **Engine ADR** (0171): compare p14 vs p15 numbers, cost of porting (native C++ we keep: technosim,
  fine_gen; GDScript game systems that would be rewritten), decide.
- **Grid ADR** (0172) from p16.
- **Block look spike** (patterned flat-colour cells vs 16x16 pixel textures) on the chosen engine; the
  developer picks from a showcase sheet.

### Wave 3 (implementation, planned after the ADRs)
World data + streaming on the sphere, planet far mesh (LOD) and the seamless climb, radial gravity and
player movement, fine-cell sim bubble (break into 12.5 cm cells, settle back), chiseled blocks, fluids as fill
levels, gases as a field, then porting machines/networks/crafting/survival and the server-owned chunks.

## Rules for every session
- Read ADR 0170 and this plan first; stay inside your prototype folder; do not touch `game/`.
- Results are measured on the dev box with the JSON above; no claims without numbers.
- Godot windows off-screen or headless where possible; kill hung runs.
- Cost cap: finish the brief, write the README, stop. No follow-on work.
