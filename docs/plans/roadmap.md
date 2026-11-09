# Roadmap

Direction: ADR 0001 (planets first), 0002 (our own Rust + wgpu engine and what it does), 0003 (cube-sphere
grid), 0004 (frame budget), 0005 (platforms). Read those first.

## Done
- **Engine choice (2026-11-09):** a Godot spike (p14) and a Rust + wgpu spike (p15) drew the same benchmark;
  Rust won (ADR 0002). Both live on their own branches: p15 on `claude/p15-engine-spike-251147` (renderer,
  streamer, `RESEARCH.md` with the techniques adopted and rejected), p14 on `claude/suspicious-taussig-3df52e`.
- **Sphere grid (2026-11-09):** equi-angular cube-sphere, seams shown as the planet's grain
  (`prototypes/sphere-grid/`, ADR 0003).

## Next
1. **Engine base:** bring p15's renderer and streamer into a cargo workspace at the repo root (engine,
   client, server as crates), with the headless run that prints the JSON stats of ADR 0004 and a filmstrip.
   Map the chunks onto the cube-sphere grid.
2. **Block look spike:** patterned flat-colour cells against 16x16 pixel textures on the new engine; the
   developer picks from a sheet. It fixes the pixel grid of ADR 0002.
3. **First playable** (ADR 0002), in this order, each with its benchmark and checks:
   - client and local server split from the start, deltas over the wire;
   - test bots on the player protocol, driven by script or live control channel;
   - world data on the cube-sphere, generation from the seed, streaming, visible-but-tidy LOD, the moon;
   - radial gravity, walking and jumping in the local frame, collision;
   - dig and place blocks; saves of only what changed;
   - water as fill levels, with fine 12.5 cm splashes near the player;
   - lighting: block light, the sun around the planet, the 20 minute day, shadow maps;
   - 3D audio with cave reverb and material sounds.
4. **After the first playable:** fine cells (breaking, sand, debris), chiselled blocks, felled trees; the
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
