# Roadmap

Direction: ADR 0001 (planets first), 0002 (our own Rust + wgpu engine and what it does), 0003 (cube-sphere
grid), 0004 (frame budget), 0005 (platforms), 0006 (client/server split). Read those first.

Brought up to date 2026-11-16 (it stood at 2026-11-12 before; main is at 2026-11-14). The world's stages are
staged and tracked in `docs/plans/worldgen-plan.md`; this file says what is done, what is in work and what
comes next, in order. How work is done: features first, speed in batches (ADR 0096, CLAUDE.md principle 14);
the debt is `docs/perf-debt.md`. The story is `docs/design/lore.md`.

## Done
- **Engine choice (2026-11-09):** a ready-made-engine spike (p14) and a Rust + wgpu spike (p15) drew the same benchmark;
  Rust won (ADR 0002). Both spikes lived on their own branches, deleted on 2026-11-16 (the tips: p14 `61fa82d0`, p15 `6a442e9d`).
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
  - W5, first half: water in rivers and lakes, booked per reach, and rain from the live air that raises
    them (ADR 0059, save version 12);
  - W6: Luna's own bake, with ice in the polar shadow, and a full Luna that reads bright from Terra
    (ADR 0027, 0070).
- **Looks and the living world (2026-11-12 to 14)**, all merged to main:
  - **Weather made visible** (ADR 0076): clouds, rain, wind, storms and snow from the live air grid; the
    water's look (ADR 0071) and beds under water (ADR 0086); taller mountains and clouds at 250 m (ADR 0105);
    sky, shadow and hand fixes (ADR 0072, 0098).
  - **Navigation part 3** (ADR 0077): the globe map, with waypoints in survival.
  - **Tides** (ADR 0067), built after the rivers and lakes of W5.
  - **Items and inventory** (ADR 0074, 0075): slots, packs, piles and the survival and creative screens;
    pick-ups and clutter on the ground (ADR 0082); flint (ADR 0097).
  - **See-through blocks and box models** (ADR 0068, 0069, 0081): cutout leaves, glass, still models in the
    chunk mesh; joined lamps (ADR 0085).
  - **Flora** (ADR 0088 to 0092, 0099, 0100): fluffy crowns, plants of crossed planes, trees with branches,
    denser forests, forests in the far chunks and over the face seams; painted at 32 pixels per metre.
  - **Peaceful animals** (ADR 0083): herds computed from the seed, drifting while nobody is near.
  - **Mining** (ADR 0087, 0093, 0094, 0095): rubble that blocks the way, stacks of 16, cracks and the drop along
    them, the changed rock around ore bodies. Slice (c), undercut and drop, is not built.
  - **The reacting world, first parts** (ADR 0078, 0079, 0101): the dig's chips on the fine lattice, loose
    blocks that fall and slide, powders as slopes. Chiselling and collapse are not built.
  - **Painted block textures** (ADR 0084), **the Studio's usability rounds and a palette that grows**
    (ADR 0064 to 0066, 0080), the light bug round (ADR 0072), streaming and LOD fixes (ADR 0073).
  - **Optimisation batches** (ADR 0102 to 0104): the CPU batch, the G-buffer batch, rays and clouds. The
    speed of the 11-13 round is the baseline now; open rows are in `docs/perf-debt.md`.
- **The Studio** (ADR 0044, 0046, 0049, 0051): textures from a palette, models, sounds, effects, structures
  and fauna pages, tied to the running game.
- **An audit of the whole repo (2026-11-12):** fifteen findings in the server, the protocol, saves, the
  engine's core and the client's frame, all fixed with tests.

- **2026-11-12 to 11-14** (each row's ADR says what was built and what it left open):
  - tides (0067); items' slots, piles and the inventory screens (0074, 0075); visible weather: clouds, rain,
    snow (0076, 0105); the globe map (0077); fine cells (0078); loose blocks and collapse (0079);
  - see-through and cutout blocks, glass and box models (0068, 0069, 0081); clutter and pick-ups (0082);
    peaceful animals in herds (0083); painted block textures (0084); joined lamps (0085);
  - mining: read the rock, rubble, cracks and drops, the ore halo (0087, 0093, 0094, 0095); flint (0097);
  - flora: fluffy crowns, plants of planes, trees with branches, forest in the far chunks (0088 to 0092,
    0099, 0100); taller mountains (0105);
  - features first, speed in batches (0096, `docs/perf-debt.md`); the first batches (0102, 0103, 0104).

- **Merged 2026-11-14:** the bug round of six F9 reports: powders only fall (0106), polar ice caps (0107),
  shadows snapped to texels, no rain or snow above the clouds, looser clouds (0108), builds seen from afar
  (0109); the first tools from flint (0110); the 1440p speed batch (0111: the forest at 2560 x 1440 is still
  GPU-bound at 12 to 17 ms); mining slice (c), undercut and drop (0112: roofs of closed rooms only).

## In work (2026-11-16)
- **The story** (`docs/design/lore.md`): the Indigenti, the uplifted ape and the fuel ladder, set down on
  2026-11-16. The ending is open on purpose; the corner landmarks of ADR 0027 are dropped (ADR 0136).
  `progression.md` is rewritten when the story is settled.
- Nothing else is known to be in work on a branch; this file is updated when a session starts one.

## Next
In this order unless the developer says otherwise (the developer's order of 2026-11-14 first).

0. **The developer play-tests** the flint tools (ADR 0110) and the undercut (ADR 0112: its spans and the
   room-only rule are calls to confirm). Open beside it: the inventory's new look (ADR 0113), the crack paintings
   (`docs/design/mining.md`), an overhang open to the sky does not drop. Then the list below, skipping what
   the rows above already built.

   **Merged 2026-11-14:** the inventory's look (ADR 0113) and dropped items as their models, painted tools,
   pictures in the hotbar and contact shadows under piles (ADR 0114); pick-ups thinned by 30 %.
   **In work:** the bare hand when nothing is held (`claude/bare-hand`), then an optimisation batch down
   `docs/perf-debt.md` in its own session (`claude/opt-1114`).
   **Queued by the developer, 2026-11-14:**
   - **Things on the ground are aimed at themselves** (stones, sticks, flint, mushrooms, berries; ADR 0082 takes
     them by a click on the ground under them): a hit test on the thing's own boxes, the aim's outline on it,
     a left click mines it, and a held right click picks it up. *Done 2026-11-17 (ADR 0144).*
   - **Grass and the other plants get a hit box** so they can be mined, and give **plant fibre** (a new item; the
     developer, 2026-11-14: it is for cord and for hafting tools. Write it into `crafting.md` when built).
     *Done 2026-11-17 (ADR 0147).*

1. **What the world's stages left open** (`docs/plans/worldgen-plan.md`):
   - the shader's 8 bits of material, before the block list passes 255 (ADR 0062's `shortcut:`; block ids reach 217
     of 255);
   - looks the developer makes in the Studio: each rock's and ore's, the rivers' and waterfalls', Luna's;
   - the Studio's box tools (step 5 of `see-through-and-boxes.md`).
2. **Needs and the journal** (designs of 2026-11-11, calls in ADR 0047; items are built, these are not):
   needs steps 1-3 (thirst, breath, death's cost; step 1, thirst and drinking, is done: ADR 0145; step 2, breath, is done: ADR 0146; step 3, death's cost (the death pile's waypoint, the sleeping place), is on a branch: `draft-death-cost.md`; today also
   health and hunger and the breath held when buried),
   the journal's steps 1-3 (events, `look:` and `have:`, the book); then the process runner of `crafting.md`
   brings tools, food and `do:`/`see:` together. ADR 0027: the ape learns by doing, Terra has animals and
   hostile creatures. What the creatures are waits on the story (`lore.md`, `open-questions.md`).
3. **Mining's slice (c)**, undercut and drop, and blasting later (`docs/design/mining.md`).
4. **Fine cells, chiselled blocks and collapse** (`docs/design/fine-cells.md`; chips and falling blocks are
   built), felled trees; then **W7, living plants**, which waits for them.
5. **Blueprints** (ADR 0017: capture, name, paste, materials paid from storage) and building collapse.
6. **Crafting and the first machines** (`crafting.md`), then **networks** (`networks.md`: shafts first, then
   pipes and wires on one engine).
   With crafting: the torch's recipe (ADR 0117; the torch is only `/give torch` and creative until then).
7. **The dedicated server on the home server and auto-update** (ADR 0005); AI players (the troop); the jetpack
   and the climb to orbit, with the attitude ball (`navigation.md` part 4).
8. **Optimisation batches** whenever `docs/perf-debt.md` grows past what playing can bear (ADR 0096).

## Later
- **On the material ledger** (the developer, 2026-11-10; opened by W4's ores, exact through every dig and
  build as the water's is):
  - **world miners:** they cut with a real laser where players can see them; with no one near, they remove
    whole chunks at intervals, and what was in them leaves the ledger;
  - **a finite sea:** water taken from a planet lowers its sea level, and later its clouds, until it is gone;
  - **a planet scanner:** it tells how much copper (or anything) a body still holds.
- Gravity that falls off with distance and orbits; the first megaproject.
- ~~**Big builds seen from afar**~~ done (ADR 0109): a coarse cell takes a placed block anywhere in it, and a
  far lamp glows. Left: a far lamp's light on the ground around it.
- **The fuel ages and two more systems** (`lore.md`): chemical, then fusion, then antimatter; Alpha Centauri
  and the Indigenti's star as places to go. This changes `progression.md`'s exotic age and shortens the
  early game to make room; it needs its own plan once the story is settled.
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
themselves, the model view's camera and placing, and sounds from outside files (built and merged
2026-11-12; its second round adds a palette that grows where you paint).
**After that: see-through blocks and textured box models** (`docs/design/see-through-and-boxes.md`): cutout
leaves, glass, small models of painted boxes at 32 pixels per metre, still models meshed into the chunk, and
later forest floor clutter and pick-ups (`docs/design/clutter-and-pickups.md`: computed from the seed, only
what was taken is stored; pick-ups wait for items). Seven steps, the first a measured spike (done, ADR 0068).

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
