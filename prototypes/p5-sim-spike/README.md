# P5 – Falling-sand voxel simulation spike (native)

**Questions:**
1. How fast can a Noita-style 3D material simulation run in C++ inside Godot
   (GDExtension), and does it scale across threads?
2. How much does it cost to push the changed chunks into godot_voxel for
   rendering every tick (the "hybrid" of ADR 0003)?
3. Is GDScript even in the running for this? (No, but by how much.)

**Setup:** Godot 4.7.2, godot-cpp branch 4.5 (an extension built against 4.5
loads on 4.7), MinGW-w64 GCC 16.2 (portable, no admin needed), `-O2`.
World: 128³ voxels = 2.1 M cells in 512 chunks of 16³. Materials: air,
stone (static), sand (falls, slides), water (falls, spreads), gas (rises).
Chunks are updated in 8 parity passes on a persistent thread pool; only
chunks flagged active are touched; flags are accumulated per chunk and
flushed once (v2; v1 wrote shared atomics per cell and scaled badly).
Machine: Ryzen 5 5600X (6c/12t), RAM at 2666 MHz.

Build: `python -m SCons platform=windows use_mingw=yes target=template_debug -j12`
with `C:\Users\dev\Tools\mingw-dl\mingw64\bin` on PATH and godot-cpp at
`C:\Users\dev\Tools\godot-cpp`.

## A. Simulation throughput (headless, `bench_sim.gd`)

| Scenario | active chunks/tick | moved cells/tick | 1 thread | 6 threads | 12 threads |
|---|---|---|---|---|---|
| Sand column 64×48×64 collapsing | 136 | 64 k | 2.38 ms | 0.69 ms | **0.67 ms** |
| Water block 48³ released into basin | 100 | 30 k | 2.93 ms | 0.90 ms | **0.89 ms** |
| Worst case: 30 % of the world is loose sand | 405 | 400 k | 9.95 ms | 2.17 ms | **1.72 ms** |
| Settled world, nothing moving | 0 | 0 | 0.001 ms | 0.001 ms | 0.001 ms |

- Peak throughput ≈ **965 k active cells per ms** (12 threads, worst case).
- Material is conserved in every run (no cell lost or duplicated across
  threads), which validates the parity-pass scheme.
- Idle chunks cost nothing: the active-flag tracking works.
- v1 (per-cell atomic flag writes) was 9.75 ms at 6 threads on the worst
  case; batching the writes gave 4.5× at 6 threads. False sharing matters.

**GDScript reference** (`sim_gd.gd`, sand only, 32³, single thread):
12.6 ms per tick, 2.6 k cells/ms. The native kernel is **~90× faster per
thread** and ~370× with threads. GDScript must never iterate voxels.

## B. Push path into godot_voxel (rendered, `stream_push.gd`, 6 threads)

Each frame: tick the sim once, then for every dirty chunk build a
`VoxelBuffer` from `get_chunk_bytes()` and call `VoxelTerrain.try_set_block_data()`.

| Scenario | chunks pushed/frame | sim ms | push ms | avg frame | p95 | p99 |
|---|---|---|---|---|---|---|
| Mixed (sand + water + gas + pillars), sim only, no push | 0 | 0.56 | 0 | **0.87** | 0.97 | 1.12 |
| Mixed, push, mesh block 16 | 109 | 0.95 | 1.18 | **14.1** | 18.1 | 29.0 |
| Mixed, push, mesh block 32 | 109 | 1.02 | 1.07 | 14.5 | 22.2 | 35.4 |
| Water only | 80 | 0.78 | 0.64 | 7.8 | 13.0 | 14.8 |
| Sand only (settles quickly) | 2.9 avg | 0.04 | 0.04 | 1.3 | 9.1 | 15.3 |

- The push itself (buffer build + block set) is cheap: ~10 µs per chunk.
- The cost is godot_voxel **re-meshing** every changed block: about
  **0.11 ms of frame time per changed 16³ chunk**, independent of mesh block
  size. 109 churning chunks per frame is an extreme event (a 40³ sand block
  and a 40³ water block collapsing at once); it still runs at ~70 FPS.
- Zero failed pushes: all data blocks inside the viewer range were loaded.

Screenshot: `out/push_mixed.png` (sand piled around pillars, water pooled,
gas cloud at the top). Chunk byte order (y fastest) matches godot_voxel.

## C. Watching it move: filmstrips, and a renderer bug found and fixed

Single screenshots hid two kinds of problems, so `frame_sheet.gd` now tiles
a thumbnail every 0.5 s (plus a burst of consecutive frames) into one PNG.

**Rule problems (fixed in v2/v3):** water piled like sand and fizzed;
gas exploded into a permanent speckle cloud. Fixes: Noita-style lateral
dispersion for liquids (slide up to 7 cells to a ledge, cohesion so films
don't fragment, calm pools), gas rises calmly and dissipates (lifetime).

**Renderer bug (found with the debug queries):** after a collapse, a lump
of sand stayed floating on screen. The sim's own queries
(`count_unsupported`, `layer_histogram`, `count_in_box`) proved the sim had
no sand above y=60, and a hash check proved every chunk's final state had
been pushed. Reading godot_voxel's source: `try_set_block_data` swaps in a
*new* voxel buffer, mesh tasks already in flight keep the *old* buffer, and
results are applied in whatever order the thread pool finishes them (the
code even notes "there could be more than one update in progress"). Under
load (60 ticks/s, 15–100 changed chunks per tick) the backlog grows to
seconds and the last result to land is often stale.

| Push path | Stale meshes at end | avg frame ms |
|---|---|---|
| `try_set_block_data` | yes | 2.6–4.9 |
| `try_set_block_data` + re-push 0.25 s after settle | still yes | 4.9 |
| `try_set_block_data` + re-push 3 s after settle | no | 2.6 |
| **`VoxelTool.paste`** (writes into the existing buffer) | **no** | **1.5** |

Decision: push through `VoxelTool.paste`. Keep the hash desync check and
the unsupported-material query as permanent debug tools.

## D. v4: the Falling Everything model, data-driven (Brent's direction)

The kernel was rewritten around Noita's model (`docs/design/material-simulation.md`):
materials are rows in a table with Noita's attribute names (`cell_type`,
`liquid_sand`, `density`, `liquid_flow_speed`, `gas_*_speed`, `lifetime`,
`burnable`, `fire_hp`, `autoignition_temperature`, `temperature_of_fire`,
`on_fire_*`...), each voxel carries `{material, flags, aux}` packed in 4
bytes, and a reaction table (`input_cell1/2 -> output_cell1/2`, probability)
drives chemistry. `materials.gd` defines stone, sand, water, steam, oil,
lava, wood, fire, smoke, ash and four reactions (lava+water → stone+steam,
fire+water → air+steam, lava ignites oil and wood).

| Scenario (128³, 12 threads) | v3 hard-coded | v4 first | v4 packed cells + LUTs |
|---|---|---|---|
| Sand column | 0.67 ms | 1.57 ms | **1.05 ms** |
| Water release | 0.89 ms | 2.54 ms | **1.95 ms** |
| Worst case 30 % loose sand | 1.72 ms | 3.12 ms | **2.48 ms** |
| Wood house + oil pool on fire | – | 0.46 ms | **0.37 ms** |
| Lava block into a lake | – | 2.55 ms | **1.99 ms** |
| Settled, idle | 0.001 ms | 0.001 ms | 0.001 ms |

Generality costs ~1.5× over the hard-coded rules (per-cell table lookups,
per-cell RNG, 4-byte cells). Packing the three per-voxel arrays into one
struct and replacing the density comparison with a 256×256 lookup won back
a third. Further headroom if ever needed: SoA by chunk, SIMD column scans,
chunk-local material cache. Not worth it now.

Filmstrips: `out/strip_lava_close.png` (lava hits the lake, stone crust and
steam cloud form), `out/strip_fire_v4d.png` (oil pool burns, fire spreads
through the wooden house, ash remains). First fire version did not spread
between burnable cells (only flame cells could ignite, and they drift away);
Noita-style direct neighbour ignition fixed it. Spread rate needs tuning.

Push-path cost scales with churn: the lava scene pushes 92 chunks per frame
at 60 Hz (a whole lake reacting) and averages 21 ms/frame; at 30 Hz it
halves. Simulation bubbles, not world size, must bound this.

## Measured answers

1. **Native simulation is fast enough with a large margin.** A 2 M-voxel
   active region with 400 k moving cells ticks in under 2 ms. Real bubbles
   will have far fewer moving cells; the budget knob is the bubble size.
2. **Hybrid push path works** (ADR 0003 option 3). Frame cost scales with
   *changed chunks per frame* at ~0.11 ms each, not with simulation size.
   Design rules that follow:
   - Tick the simulation at a fixed rate (20–30 Hz), not per frame, and push
     once per tick: see the `tick_hz` run below.
   - Keep the live bubble a few chunks across; outside it, nothing changes
     and nothing re-meshes.
   - If churn ever dominates, the active bubble can get its own lightweight
     mesher later; godot_voxel still owns everything static and all LOD.
3. **GDScript is out** for voxel loops, by two orders of magnitude.

## Caveats
- MinGW `-O2` debug template build; MSVC/release may differ slightly.
- Rules are the simplest possible (no density swaps beyond sand>water,
  no reactions, no temperature). Real rules cost more per cell but the
  active-chunk tracking still bounds the work.
- Not yet measured on Android or in the headless server export.
