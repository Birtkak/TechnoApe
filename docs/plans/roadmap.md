# Roadmap

Direction: ADR 0001 (planets first), 0002 (our own Rust + wgpu engine and what it does), 0003 (cube-sphere
grid), 0004 (frame budget), 0005 (platforms), 0006 (client/server split). Read those first.

Brought up to date 2026-11-12. The world's stages are staged and tracked in `docs/plans/worldgen-plan.md`;
this file says what is done, what is in work and what comes next, in order.

## Done
- **Engine choice (2026-11-09):** a Godot spike (p14) and a Rust + wgpu spike (p15) drew the same benchmark;
  Rust won (ADR 0002). Both live on their own branches: p15 on `claude/p15-engine-spike-251147`, p14 on
  `claude/suspicious-taussig-3df52e`.
- **Sphere grid (2026-11-09):** equi-angular cube-sphere, seams shown as the planet's grain (ADR 0003).
- **Engine base (2026-11-10):** `crates/engine` (world, meshing, streaming; no GPU) and `crates/client`
  (window, renderer, benchmark runs). Numbers: `docs/benchmarks.md`.
- **The first playable (2026-11-09 to 11-12)**, every point of it:
  - client and server split, deltas over the wire (ADR 0006); bots on the player protocol (`crates/bot`);
  - gravity, walking and collision in the local frame (ADR 0007, 0024); numbered input steps with
    reconciliation (ADR 0019);
  - the test harness: soak, state hash, flight recorder and replay, the gate (ADR 0011); fixed viewpoints,
    the crack detector and the feel report (ADR 0018); the speed guard (ADR 0025 K);
  - blocks from data (ADR 0013), 1 m blocks (ADR 0033), edits as brushes with prediction (ADR 0014, 0015,
    0017), saves of what changed (ADR 0016), survival dig times (ADR 0034);
  - water with a ledger (ADR 0020); ray-traced light, the sky's mechanics, eclipses (ADR 0022, 0036, 0037,
    0041); sound with our own mixer and cave reverb (ADR 0032, 0048, 0054);
  - health, hunger and death (ADR 0040); the settings menu and key bindings (ADR 0023, 0028); navigation
    parts 1 and 2: coordinates, compass, waypoints (ADR 0030); effects (ADR 0031, 0050); voxel models,
    structures, creatures' walk rig (ADR 0047, 0052, 0053).
- **The optimisation pass (2026-11-10 and 11, ADR 0025):** the traced light's cost brought back inside the
  budget on every run. Not reached: `stand` at 800 fps (it stands at about 580); left open there: `light()`
  as a compute pass (D2) and `fly`'s burst.
- **Terra and Luna, stages W0 to W6 (2026-11-11 and 12):**
  - W1, W2: the planet baked once per seed, heights, mips, river lines and beds, the spawn by score, ground
    detail by slope (ADR 0026, 0043, 0056, 0057, 0058);
  - W3: climate from the air grid, fifteen biomes, the axial tilt and seasons, the air grid live on the
    server (ADR 0042, 0055, 0060);
  - W3b: one block, a look per season, painted season files and the Studio's season tabs (ADR 0061);
  - W4: 16-bit block ids, rock provinces, caves by rock, counted ore bodies under a 192 m crust, and the
    material ledger opened with the ores (ADR 0062, 0063);
  - W5, first half: water in rivers and lakes, booked per reach (ADR 0059);
  - W6: Luna's own bake, with ice in the polar shadow (ADR 0027, 0070).
- **The Studio** (ADR 0044, 0046, 0049, 0051): textures from a palette, models, sounds, effects, structures
  and fauna pages, tied to the running game.
- **An audit of the whole repo (2026-11-12):** fifteen findings in the server, the protocol, saves, the
  engine's core and the client's frame, all fixed with tests.

## In work (2026-11-12)
- **Rain that raises the rivers:** a rain account per reach fed by the live air's runoff (ADR 0059, save
  version 12), on `claude/w5-rivers`.
- **Tides** (W5, second half): 1 m at springs, following Luna, on its own branch.
- **Luna's brightness** in Terra's night sky, and a save naming a deleted version 4 map (it stops the server),
  on `claude/w6-luna-10e10b`.

## Next
In this order unless the developer says otherwise.

1. **What the world's stages left open** (`docs/plans/worldgen-plan.md`):
   - W2b, the globe map (`docs/design/navigation.md` part 3): spike first;
   - the shader's 8 bits of material, before the block list passes 255 (ADR 0062's `shortcut:`);
   - looks the developer makes in the Studio: each rock's and ore's, the rivers' and waterfalls', Luna's.
2. **Items, needs and the journal** (designs of 2026-11-11, calls in ADR 0047; nothing built): items steps
   1-3 (slots, piles, the screen), needs steps 1-3 (thirst, breath, death's cost), the journal's steps 1-3
   (events, `look:` and `have:`, the book); then the process runner of `crafting.md` brings tools, food and
   `do:`/`see:` together. ADR 0027: the ape learns by doing, Terra has animals and hostile creatures, the
   cube corners hold something made.
3. **Visible weather** (ADR 0010; the air grid already runs): clouds, rain, wind, storms; snow on the
   ground with it (`seasonal-looks.md` leaves it to the weather).
4. **Fine cells** (breaking, sand, debris), chiselled blocks, felled trees; then **W7, living plants**, which
   waits for them.
5. **World physics** (ADR 0017: loose blocks fall and slide, unsupported builds collapse) and **blueprints**
   (ADR 0017: capture, name, paste, materials paid from storage).
6. **Crafting and the first machines** (`crafting.md`), then **networks** (`networks.md`: shafts first, then
   pipes and wires on one engine).
7. **The dedicated server on the home server and auto-update** (ADR 0005); AI players; the jetpack and the
   climb to orbit, with the attitude ball (`navigation.md` part 4).

## Later
- **On the material ledger** (the developer, 2026-11-10; opened by W4's ores, exact through every dig and
  build as the water's is):
  - **world miners:** they cut with a real laser where players can see them; with no one near, they remove
    whole chunks at intervals, and what was in them leaves the ledger;
  - **a finite sea:** water taken from a planet lowers its sea level, and later its clouds, until it is gone;
  - **a planet scanner:** it tells how much copper (or anything) a body still holds.
- Gravity that falls off with distance and orbits; the first megaproject.
- **Big builds seen from afar:** a far chunk point-samples edits, so a structure thinner than a coarse cell
  vanishes from orbit; keep a per-level summary of edits.
- **Builds in free space** (a ring around Luna, a structure around the sun) need their own grid, as ships do:
  a body's grid ends at its build headroom (ADR 0021).
- **The planet's air after weather** (ADR 0010): pollution (smoke and waste in air, water and soil; filters
  and scrubbers) with MV; radiation (ore, reactors, waste, fallout; unshielded space and Luna) with EV.
- Telescopes (reflections on metal are built, ADR 0022).

Keep open from now on (ADR 0009): a whole planet can be mined out and drained late in the game, so no world
data or save format may assume the core is never dug or that water is a fixed amount.

Content target (ADR 0009, `docs/design/progression.md`): the starter to HV, which is one planet, the planet's
air up to pollution, and three networks (pipes, shafts, wires) on one network engine.

## The Studio, beside all of the above
Our own program in which the developer makes the game's textures and voxel models by hand, and its effects,
sounds, trees and structures (ADR 0044, `docs/design/studio.md`). **Next: the usability round** (2026-11-12,
the design doc's section of that name): the block always in view, a pattern panel, variants that explain
themselves, the model view's camera and placing, and sounds from outside files.

## The benchmark
The engine keeps one benchmark as its performance check, so numbers stay comparable over time.

- **World:** Terra, the baked planet of 6144 columns a face edge (about 4 km radius), 1 m blocks (ADR 0033),
  a 192 m crust, with Luna 3.7 times smaller and 30 Terra diameters away (`data/bodies.toml`). `base` (531 m,
  comparable with the first spikes) and `huge` (8 km) on demand.
- **Runs** (uncapped, vsync off, 1920x1080, the dev PC), at half length since 2026-11-10:
  1. `stand`: standing on the surface, looking at the horizon.
  2. `fly`: at 20 m altitude, 60 m/s around the equator.
  3. `orbit`: from the ground straight up, across to Luna, land, back, with no load screen.
  4. `edit`: 2000 block changes a second within 10 m.
  5. `sim`: `stand`'s view while the server carries 50 bots (ADR 0025 S).
  6. `fx`: `fx_n=` particles kept alive (ADR 0031).
- **Output:** one JSON line per run (frame times, the frame thread's stages, GPU stages, mesh job time, RAM,
  VRAM, `missing_chunks_max`, `budget_ok`), plus filmstrips of `fly` and `orbit`.
- **Targets:** ADR 0004 in every run, `missing_chunks_max == 0` in `fly`; stretch: `stand` at 800 fps.
- **Two tools:** `bash tools/bench.sh` is the full series (about 8 min; results in `docs/benchmarks.md`);
  the gate's speed guard holds quick runs to `tests/perf/baseline.json` (re-measured 2026-11-12 on a quiet
  PC after W3b to W6).
- **Honest numbers:** release builds, nothing else heavy running, the warm-up excluded, 3 reps per run
  (median and worst), CPU and GPU ms reported separately, an ablation row per big optimisation.
