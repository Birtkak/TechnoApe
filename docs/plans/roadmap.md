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
   - ~~the planet has a 1 km radius (ADR 0003)~~ since 2026-11-10 a **continental planet of 4 km**, 8 km
     for the huge one (ADR 0021: 55 % ocean, 3-5 continents and islands, hills and ranges to 50 m). The
     benchmark's `small` series runs it, `base` (531 m) and `huge` (8 km) on demand; all must hold ADR 0004;
   - Luna is in the sky and **reachable with a creative fly mode**, seamlessly; the jetpack comes later;
   - **survival-style inventory:** dug blocks go to a hotbar and inventory and are placed from it; a creative
     toggle for testing;
   - **no playtest builds before the whole first playable is done.** Until then each step is checked with
     tests, bots and headless runs; the developer gets one build with a to-test list at the end.

   In this order, each with its benchmark and checks:
   - ~~client and local server split from the start (a `server` crate), deltas over the wire~~ done
     2026-11-10 (ADR 0006);
   - ~~test bots on the player protocol, driven by script or live control channel~~ done 2026-11-10
     (`crates/bot`, `technoape-bot`);
   - ~~world data on the cube-sphere, generation from the seed, streaming, visible-but-tidy LOD, Luna~~
     done 2026-11-10: the engine base had all but the LOD seams; skirts in `generate` close them;
   - ~~radial gravity, walking and jumping in the local frame, collision~~ done 2026-11-09 (ADR 0007):
     `crates/engine/src/phys.rs`, shared by the server, the client's `run=play` and the bots' `walk`;
   - ~~test harness I: soak, state hash, flight recorder with replay, one gate~~ done 2026-11-10 (ADR 0011):
     `tools/check.sh`. Its first runs moved collision into grid space (ADR 0007, revised);
   - ~~cheap speed levers and a `huge` row~~ done 2026-11-10 (ADR 0012): a column cache in the physics, Fx
     for edits, mimalloc, low-priority workers; the soak 2.8x faster, the 4 km planet holds ADR 0004;
   - ~~dig and place blocks **as brushes and dragged shapes, predicted on the client** (ADR 0008)~~; a block
     registry from data instead of `MATERIALS`; saves of only what changed. Done so far (2026-11-10): the
     registry (ADR 0013), box edits with a survival inventory (ADR 0014), prediction (ADR 0015), saves
     (ADR 0016), single-block hands and a **build hammer** with Lay of the Land's shapes (ADR 0017: box,
     cylinder, cone, with a ghost and a hotbar). Done 2026-11-10. Survival digs take time by hardness, as in Minecraft (ADR 0034, 2026-11-11); mining tools come later;
   - ~~**test harness II** with the first visuals that can break: fixed viewpoints compared by number, the
     crack detector, the feel report~~ done 2026-11-10 (ADR 0018): `tools/views.sh`, `shot=`, the `feel`
     field, all in the gate. Found a pinhole crack at the LOD view and a start-up correction, both fixed
     (the top padding's skirt; numbered input steps with reconciliation, ADR 0019);
   - ~~water, counted from the start (the water cycle needs it, ADR 0010)~~ done 2026-11-10 (ADR 0020):
     shallow water with momentum on the columns' air runs, a generated sea that is stored only where it
     moves, a ledger the soak checks every tick, tiles to the clients, a smooth water mesh with splash
     particles, the bucket and swimming. The 12.5 cm splash cells come with the fine cells, basin lakes
     with the biomes;
   - **lighting and the sky's mechanics** (the developer's calls, 2026-11-10; spike in `prototypes/lighting/`):
     ray-traced light with the sun as the only source, a real sphere. The planet rotates (the 20 minute day
     comes from its spin) and Luna orbits on a tilted orbit, so its phases and moonlight follow from
     where it is. Eclipses are a real blockage of the light, each place on the planet getting its own share
     of totality. Soft but fairly sharp shadows, ambient occlusion, smooth coloured block light, and
     reflections on metal (later telescopes). Hardware ray queries on Vulkan: about 1.8 ms per 1080p frame
     on the RTX 3070 with a reflection on every pixel;
   - 3D audio with cave reverb and material sounds.
2. **Before the rest (the developer, 2026-11-10), in this order:**
   - **the optimisation pass** (`docs/plans/optimization-handoff.md`, ADR 0025): the lighting took the
     frame from 0.4 ms to about 4 ms; target `stand` at 800 fps or more;
   - **Terra and Luna** (the planet's and the moon's names; ADR 0026, `docs/design/worldgen.md`,
     staged in `docs/plans/worldgen-plan.md`): each body simulated once when the world is made (plates,
     erosion, rivers, climate, real rocks and minerals, counted ore bodies), then tides, seasons and
     living plants. This replaces "the Earth-like biomes and Luna's ores" below and brings the air
     grid's spike and the material ledger forward.
   Designs written 2026-11-10 for the steps after it, all proposals until built (`docs/design/`):
   `world-data.md` (what a block is: runs per column, 16-bit ids, regions, the ledger), `ore-budget.md`,
   `air.md` (the grid ADR 0010 asks to spike), `rivers-and-tides.md`, `networks.md` (shafts, pipes,
   wires, belts on one graph), `crafting.md` (processes the world or a machine can meet; the starter),
   `settings.md` (2026-11-11: every page of the settings menu, whose each setting is, and the order to
   build them), `navigation.md` (2026-11-11: latitude and longitude, the compass and waypoints, built in
   ADR 0030; the globe map after the bake, the attitude ball with the jetpack), `items.md` (2026-11-11:
   slots in place of counts, tools from the materials, piles on the ground; crafting's first engine need),
   `needs.md` (2026-11-11: thirst, breath, warmth, food as items and what death costs, after ADR 0040),
   `journal.md` (2026-11-11: ADR 0027's learning by doing and seeing, on a list of events out of the tick),
   and `open-questions.md`; their four taste calls are ADR 0043. **A build order across the three**, each step small and checked alone: items
   steps 1-3 (slots, piles, the screen), needs steps 1-3 (thirst, breath, death's cost), the journal's
   steps 1-3 (events, `look:` and `have:`, the book); then the process runner of `crafting.md` brings
   tools, food and `do:`/`see:` together. ADR 0027: Luna grows to about 1 km, the ape learns by doing, Terra has
   animals and hostile creatures, the cube corners hold something made.
3. **After the first playable:** fine cells (breaking, sand, debris), chiselled blocks, felled trees; **world
   physics** (ADR 0017: loose blocks fall and slide, unsupported builds collapse); **blueprints** (ADR 0017:
   capture, name, paste, materials paid from storage); the
   Earth-like biomes and Luna's ores; crafting and the first machines (designed fresh); AI players;
   jetpack and the climb to orbit; the dedicated server on the home server and auto-update (ADR 0005).
   From ADR 0008: the survival needs and rooms (air, heat); counted deposits; **a material ledger per
   body** (the developer, 2026-11-10): every planet, moon and star knows how much of each material it holds,
   counted from the seed's generation and kept exact through every dig, build and machine, as the water's
   ledger already is (ADR 0020). It is the base for:
   - **world miners:** they cut with a real laser where players can see them; with no one near, they remove
     whole chunks at intervals, and what was in them leaves the ledger;
   - **a finite sea:** water taken from a planet lowers its sea level, and later its clouds, until it is gone;
   - **a planet scanner:** it tells how much copper (or anything) a body still holds.
   The coarse generated totals come from integrating the generator over each body; gravity that falls off with
   distance and orbits; the first megaproject. **Big builds seen from afar:** a far chunk point-samples
   edits, so a structure thinner than a coarse cell vanishes from orbit; keep a per-level summary of edits
   (a coarse cell is built when its blocks are). **Builds in free space** (a ring around Luna, a
   structure around the sun) need their own grid, as ships do: a body's grid ends at its build headroom
   (ADR 0021).
   From ADR 0010, **the planet's air:** one coarse grid for the whole planet. Spike it first and measure the
   tick cost on the 1 km and 4 km planets. Then, in this order:
   - temperature and the **water cycle** (steam and evaporation return as rain), with the survival needs;
   - **weather** (clouds, rain, wind, storms, lightning), with LV;
   - **pollution** (smoke and waste in air, water and soil; filters and scrubbers), with MV;
   - **radiation** (ore, reactors, waste, fallout; unshielded space and Luna), with EV.

   Keep open from now on (ADR 0009): a whole planet can be mined out and drained late in the game, so no
   world data or save format may assume the core is never dug or that water is a fixed amount.

   Content target after that (ADR 0009, `docs/design/progression.md`): the starter to HV, which is one
   planet, the planet's air up to pollution, and three networks (pipes, shafts, wires) on one network engine.

   **Beside all of the above, the Studio** (ADR 0044, `docs/design/studio.md`): our own program in which the
   developer makes the game's textures and voxel models by hand, later effects, trees and structures. Its
   seven steps are in the design doc; the first is a measured spike of painted 16 x 16 textures against
   ADR 0039's patterns, the second lets a PNG in `data/textures/` replace a block's pattern.

## The shared benchmark
The engine keeps the spikes' benchmark as its performance check, so numbers stay comparable over time.

- **World:** one planet, radius 500 m, 1 m blocks (ADR 0033; 0.5 m until then), seeded noise terrain (+-30 m), 3 materials, crust 64 m
  thick (the core below is never meshed). Moon: radius 150 m, 3 km away, same rules.
- **Runs** (uncapped, vsync off, 1920x1080, reference machine), at half length since 2026-11-10 so a series
  takes minutes, not half an hour (`length=0.5`, 5 s warm-up; rows before that date are full length):
  1. `stand`: 15 s standing on the surface, looking at the horizon.
  2. `fly`: 30 s at 20 m altitude, 60 m/s around the equator.
  3. `orbit`: from the ground straight up to 3 km, across to Luna, land, back, with no load screen, in 32 s.
  4. `edit`: 15 s of 2000 block changes/s within 10 m.
- **Series:** `small` (the standard 4 km planet) and the filmstrips by default, about 8 min; `base` (531 m,
  comparable with the spikes) and `huge` (8 km) on demand, about 6 min each.
- **Output:** one JSON line per run: `fps_avg`, `frame_p50_ms`, `frame_p99_ms`, `frame_max_ms`, `ram_mb`,
  `vram_mb`, `chunks_meshed_per_s`, `missing_chunks_max`, plus filmstrips of `fly` and `orbit`.
- **Targets:** ADR 0004 in all four runs, `missing_chunks_max == 0` in `fly`; stretch: `stand` >= 300 fps.
- **Honest numbers:** release builds, nothing else heavy running, the warm-up excluded, 3 reps per run
  (median and worst), CPU and GPU ms reported separately, an ablation row per big optimisation. `quick=1`
  gives quarter-length runs for a smoke check in about a minute.
