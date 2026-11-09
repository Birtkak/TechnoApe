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
   - radial gravity, walking and jumping in the local frame, collision;
   - dig and place blocks; saves of only what changed;
   - water as fill levels, with fine 12.5 cm splashes near the player;
   - lighting: block light, the sun around the planet, the 20 minute day, shadow maps;
   - 3D audio with cave reverb and material sounds.
2. **After the first playable:** fine cells (breaking, sand, debris), chiselled blocks, felled trees; the
   Earth-like biomes and the moon's ores; crafting and the first machines (designed fresh); AI players;
   jetpack and the climb to orbit; the dedicated server on the home server and auto-update (ADR 0005).

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
