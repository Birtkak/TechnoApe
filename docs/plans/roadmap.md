# Roadmap

Direction: ADR 0001 (planets first), 0002 (our own Rust + wgpu engine and what it does), 0003 (cube-sphere
grid), 0004 (frame budget), 0005 (platforms), 0006 (client/server split). Read those first.

## Done
- **Engine choice (2026-11-09):** a Godot spike (p14) and a Rust + wgpu spike (p15) drew the same benchmark;
  Rust won (ADR 0002). Both live on their own branches: p15 on `claude/p15-engine-spike-251147` (renderer,
  streamer, `RESEARCH.md` with the techniques adopted and rejected), p14 on `claude/suspicious-taussig-3df52e`.
- **Sphere grid (2026-11-09):** equi-angular cube-sphere, seams shown as the planet's grain
  (`prototypes/sphere-grid/`, ADR 0003).
- **Engine base (2026-11-10):** a cargo workspace with `crates/engine` (world, meshing, streaming; no GPU)
  and `crates/client` (window, renderer, benchmark runs), from p15's code. The world is on the cube-sphere
  grid: chunks are (face, column, column, layer) boxes, seams fold in generation, edits and meshing, the
  vertex shader maps the grid to the sphere. Numbers: `docs/benchmarks.md`.
- **Block look (2026-11-10):** 16x16 pixel textures per block, over flat colours and 32 px textures, picked
  from a sheet (`prototypes/block-look/`, ADR 0002).

## Next
1. **First playable** (ADR 0002). Settled 2026-11-09:
   - the planet has a **1 km radius** (N = 3072 per cube face, R = 1061 m, ADR 0003; about 22 min to walk
     around, 60 m horizon at eye height). The benchmark runs on a 531 m planet (`base`) and on this one
     (`big`); both must hold ADR 0004;
   - the moon is in the sky and **reachable with a creative fly mode**, seamlessly; the jetpack comes later;
   - **survival-style inventory:** dug blocks go to a hotbar and inventory and are placed from it; a creative
     toggle for testing;
   - **no playtest builds before the whole first playable is done.** Until then each step is checked with
     tests, bots and headless runs; the developer gets one build with a to-test list at the end.

   In this order, each with its benchmark and checks:
   - ~~client and local server split from the start (a `server` crate), deltas over the wire~~ done
     2026-11-10 (ADR 0006);
   - ~~test bots on the player protocol, driven by script or live control channel~~ done 2026-11-10
     (`crates/bot`, `technoape-bot`);
   - ~~world data on the cube-sphere, generation from the seed, streaming, visible-but-tidy LOD, the moon~~
     done 2026-11-10: the engine base had all but the LOD seams; skirts in `generate` close them;
   - radial gravity, walking and jumping in the local frame, collision. **In progress, uncommitted in the
     working tree (2026-11-09):** `crates/engine/src/phys.rs` plus the server, client (`run=play`) and bot
     (`walk`) wiring; the workspace compiles and all 20 tests pass. Left to do: write ADR 0007 (the code
     refers to it), run the benchmark, commit. Check first: `blocked` samples the box at heights 0, 0.9 and
     1.8 m, so a lone block at 1.0-1.5 m above the feet looks walkable-through (found by reading, not run);
     sample every block layer and add the test;
   - **test harness I** (`docs/design/fundamentals.md` section 4, steps 1-3): invariants with a fast-forward
     soak of random bots biased to seams, a state hash per tick, the flight recorder with replay; one gate,
     `tools/check.sh`. Before digging and saves, which it is there to catch;
   - **cheap speed levers**, one spike on the benchmark (fundamentals section 3): mimalloc, a fast hasher,
     worker priority below the frame thread; a `huge` row for the 4 km planet (N = 12288);
   - dig and place blocks **as brushes and dragged shapes, predicted on the client** (ADR 0008); a block
     registry from data instead of `MATERIALS`; saves of only what changed;
   - **test harness II** with the first visuals that can break: fixed viewpoints compared by number, the
     crack detector, the feel report;
   - water as fill levels, with fine 12.5 cm splashes near the player; the total volume is counted from the
     start (the water cycle needs it, ADR 0010);
   - lighting: block light, the sun around the planet, the 20 minute day, shadow maps;
   - 3D audio with cave reverb and material sounds.
2. **After the first playable:** fine cells (breaking, sand, debris), chiselled blocks, felled trees; the
   Earth-like biomes and the moon's ores; crafting and the first machines (designed fresh); AI players;
   jetpack and the climb to orbit; the dedicated server on the home server and auto-update (ADR 0005).
   From ADR 0008: the survival needs and rooms (air, heat); counted deposits; gravity that falls off with
   distance and orbits; the first megaproject. The planet's size is decided after the first playtest.
   From ADR 0010, **the planet's air:** one coarse grid for the whole planet. Spike it first and measure the
   tick cost on the 1 km and 4 km planets. Then, in this order:
   - temperature and the **water cycle** (steam and evaporation return as rain), with the survival needs;
   - **weather** (clouds, rain, wind, storms, lightning), with LV;
   - **pollution** (smoke and waste in air, water and soil; filters and scrubbers), with MV;
   - **radiation** (ore, reactors, waste, fallout; unshielded space and the moon), with EV.

   Keep open from now on (ADR 0009): a whole planet can be mined out and drained late in the game, so no
   world data or save format may assume the core is never dug or that water is a fixed amount.

   Content target after that (ADR 0009, `docs/design/progression.md`): the starter to HV, which is one
   planet, the planet's air up to pollution, and three networks (pipes, shafts, wires) on one network engine.

## The shared benchmark
The engine keeps the spikes' benchmark as its performance check, so numbers stay comparable over time.

- **World:** one planet, radius 500 m, 0.5 m blocks, seeded noise terrain (+-30 m), 3 materials, crust 64 m
  thick (the core below is never meshed). Moon: radius 150 m, 3 km away, same rules.
- **Runs** (uncapped, vsync off, 1920x1080, reference machine):
  1. `stand`: 30 s standing on the surface, looking at the horizon.
  2. `fly`: 60 s at 20 m altitude, 60 m/s around the equator.
  3. `orbit`: from the ground straight up to 3 km, across to the moon, land, back, with no load screen.
  4. `edit`: 30 s of 2000 block changes/s within 10 m.
- **Output:** one JSON line per run: `fps_avg`, `frame_p50_ms`, `frame_p99_ms`, `frame_max_ms`, `ram_mb`,
  `vram_mb`, `chunks_meshed_per_s`, `missing_chunks_max`, plus filmstrips of `fly` and `orbit`.
- **Targets:** ADR 0004 in all four runs, `missing_chunks_max == 0` in `fly`; stretch: `stand` >= 300 fps.
- **Honest numbers:** release builds, nothing else heavy running, 10 s warm-up excluded, 3 reps per run
  (median and worst), CPU and GPU ms reported separately, an ablation row per big optimisation. `quick=1`
  gives quarter-length runs for a smoke check in about a minute.
