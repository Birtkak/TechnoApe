# CLAUDE.md – TechnoApe

3D first-person voxel game: Minecraft/GregTech building and automation,
Noita-style simulated materials and wand/spell play, procedurally generated
world with 2 km LOD horizons. Windows client (Android dropped, ADR 0009), dedicated Linux
server on the home server (Unraid) over Tailscale. Solo developer; Claude
Code does most implementation, the developer steers, plays, decides.

Ground truth: `docs/design/vision.md`. Decisions: `docs/decisions/`.
Research: `docs/research/`. Prototype results: `prototypes/*/README.md`.
Open jobs for the strongest model: `docs/plans/fable-backlog.md`.

## Working principles (agreed 2026-10-21)

1. **Foundation over speed.** Spike alternatives in `prototypes/` and measure
   before committing. Current focus: engine + voxel simulation core.
2. **Every significant choice gets an ADR** (`docs/decisions/NNNN-title.md`).
3. **Prompt the developer only for taste/scope/direction calls.** Routine technical
   calls: sensible default, recorded.
4. **Give yourself a check to run** before asking the developer to look: gdUnit4 for
   logic, headless run for scenes, screenshot for visuals. Show the evidence.
   Commands: see `.claude/skills/godot-cli/SKILL.md`.
5. **Design doc wins over code** when they disagree; fix explicitly.
6. **Data-driven systems:** items, recipes, materials, biomes as data + schema + tests.
7. **Simulation separate from presentation**, deterministic from seed,
   authoritative on the server, deltas over the wire.
8. **GDScript orchestrates, never iterates voxels.** Per-voxel work is C++
   (GDExtension). C# is not used in the shipped game; it may be used for
   tooling (ADR 0003, 0009).
9. **Performance rules:** multithread everything heavy, budget RAM
   and disk explicitly, always pick the discrete GPU and log it, ship every
   system with a benchmark. See vision.md "Engineering requirements".
   Reference machine = the dev box (ADR 0009): default settings must hold
   p99 frame <= 8.3 ms, no frame > 16.7 ms after load, RAM <= 4 GB, VRAM <= 6 GB.
10. **Debug tools are for Claude too.** Anything about the running game must
    be reachable without eyes: JSON stats on stdout, log files, state dumps,
    and frame capture. To *watch* motion, capture a filmstrip (thumbnail
    every N ms) and a burst (consecutive frames) and read the sheet; see
    `prototypes/p5-sim-spike/frame_sheet.gd`. Never judge animation from one
    screenshot.

## Stack (ADR 0001 accepted, 0002 accepted, 0003 proposed)

- Godot 4.7.2 .NET at `~\Tools\Godot\4.7.2-mono\Godot_v4.7.2-stable_mono_win64\`
  (`_console.exe` for CLI). .NET SDK 8.0.426 (tooling only).
- godot_voxel GDExtension, **our own build** of commit 045ff932 (the 2026-10-01
  master CI build at `~\Tools\downloads\libvoxel-36898667256\`) with the
  patches in `prototypes/p11-lod-horizon/voxel_patch.py` (ADR 0020: LOD
  distance cap, cheap cross-fade through an instance parameter). Source at
  `~\Tools\godot_voxel`; build with `target=template_debug` (the editor target
  does not compile with GCC 16) and copy the DLL to `game/addons/zylann.voxel/bin/`,
  see `game/README.md`. Terrain: `VoxelLodTerrain` + `VoxelMesherBlocky` +
  `streaming_system = CLIPBOX`; never set `terrain.material` (it overrides the
  library's per-surface materials, ADR 0020).
- Tests: gdUnit4 v6.2.2 (source at `~\Tools\downloads\gdUnit4\`).
- C++ toolchain: portable MinGW-w64 GCC 16.2 at
  `~\Tools\mingw-dl\mingw64\bin` (add to PATH), SCons via
  `python -m SCons`, godot-cpp branch 4.5 at `~\Tools\godot-cpp`.
  Build an extension: `python -m SCons platform=windows use_mingw=yes target=template_debug -j12`.
  MSVC 2022 Build Tools (cl 14.44) are installed too; MinGW stays the default.
- World generator is native (`src/world_gen.cpp`); design in
  `docs/design/world-generation.md`. godot_voxel asks for padded non-cubic
  blocks, so generators take a Vector3i size.

## Game project (`game/`)

- Run / verify: see `game/README.md`. Unattended check:
  `Godot_..._console.exe --path game -- autopilot=1 duration=20 capture_every=1 vsync=0 out=res://out/runN`
  then read `game/out/runN/strip.png` and the JSON line on stdout: `budget_ok`
  (ADR 0009, measured as ADR 0014 says) and `other_godot_processes == 0`.
  The project renders on a separate thread (ADR 0016); `--render-thread safe`
  runs the old mode. Never drive the shared RenderingDevice from the main thread.
  A material's first draw compiles its pipeline (20-30 ms, 1 s uncached): new terrain models
  are drawn under the loading screen by `scripts/visuals/material_warmup.gd`, particle pools by
  `ParticleFx._prewarm`; anything else drawn mid-game for the first time needs the same.
  Play loads through the SQLite save; benchmarks do not: measure the save path
  with `persist=1 seed=<unused>` when loading or streaming changes (ADR 0017).
- One world unit = one voxel = 0.5 m. Tunables live in `scripts/settings.gd` in metres.
- The title screen is the main scene (ADR 0032: worlds list, Settings with graphics presets in
  `user://settings.cfg`, Esc pauses with Settings and save-and-quit, the player is saved with the
  world). `autopilot=1` or `play=1` skip it; `menu_test=1` checks the menus.
- Playtest tools (ADR 0018): T chat + slash commands (`/help`), `/gamemode`, F3
  debug screen, F4 playtest menu, F9 bug reports. When the developer mentions a
  bug report, read the newest folder in `game/out/bug_reports/` (report.md,
  state.json, screenshot.png, log.txt).
- Rebuild the sim extension from `prototypes/p5-sim-spike` and copy the DLL to
  `game/addons/technosim/bin/`.
- Item looks: `game/data/item_sprites.json` (16x16 pixel art, shared shapes +
  palettes + per-item layers), generated by `game/tools/make_item_sprites.py`
  (edit shapes there, not the JSON); every item needs a sprite or a block
  material (`test_item_sprites.gd`); `tools/item_sheet.gd` renders them all.
- GitHub: https://github.com/Birtkak/TechnoApe (private).

## Phase plan

- Phase 0: research + direction – done 2026-10-21
- Phase 1 prototypes: P1 headless loop ✓, P2 voxel streaming/LOD ✓,
  P5 Noita-model simulation core ✓, P3 crafting core ✓ (17 headless tests)
- Phase 2 (current): `game/` skeleton ✓ (LOD world, textures, player, sim
  bubble, debug capture), native world generator ✓ (splines, caves, ores,
  biomes from biomes.json, trees, ruins, hydrology rivers/lakes/springs,
  marshes, waterfalls, altitude bands, micro-voxel vegetation and detail
  (pebbles, mushrooms, reeds, lily pads, boulders, cave stalactites/
  stalagmites), seeds; dungeons parked), player loop ✓ (ADR 0005), station
  blocks ✓ (ADR 0007: crafting table, furnace, chest block entities),
  player body ✓ (`ape_model.gd`: micro-voxel monkey in goggles and lab coat,
  F5 cycles first person / behind / front, Minecraft-style arm in first person),
  recipe browser ✓ (ADR 0008: JEI-style search, recipes/uses, R/U keys),
  bubble polish ✓ (ADR 0010: in-place shift, load/push budgets, fire
  pacing per material), voxel-grid collision ✓ (ADR 0011: player and items
  move with `VoxelBoxMover`, terrain mesh colliders off, swimming); 298
  headless tests (2026-10-23). LOD policy and horizon ✓ (ADR 0020: foliage thins per
  LOD ring, 2 km horizon, dithered LOD cross-fade, see-through water,
  loading screen, edit remesh rate; every ADR 0014 scenario within the
  ADR 0009 budget on the dev box; `prototypes/p11-lod-horizon` holds the
  godot_voxel patches and measurements). Animals, farming and furniture ✓ (ADR 0022: sheep and
  pigs with a Minecraft goal brain, herds in plains, tempt/breed/shear; Minecraft farming with
  wheat and carrots, rare wild crops; wool, carpet, bed, chair; `hands_test=1` drives them all).
  Vintage Story early game ✓ (ADR 0023: knapping and clay forming on the ground, pit kiln,
  firepit, crucible melting, casting in molds, sieve; rock drops nothing before copper;
  pieces in `scripts/work/`, `stone_age_test=1` plays flint to copper pickaxe). Progression book ✓ (ADR 0028: FTB-style quest book from the button top left of the inventory, `data/quests.json`, Flint Age chapter ending at the first copper tool, `/quests`). Deeper Flint Age ✓ (ADR 0030: cordage binds flint heads, thrown spear and hides, scraping and drying leather on the ground, worn bag, pan, bowls, cooking pot and stew, storage pot, thatch, wattle and daub, straw bed, torches with lights, an in-world charcoal pit sealed with soil; `flint_age_test=1`, `/kit flint`). Ray-cast near field in the game ✓ (ADR 0021,
  `near_field=1`, off by default until the user calls it: the bubble writes a wrapped
  `BrickStore`, `NearFieldFeed` + `near_scatter.glsl` keep a toroidal 512³ volume on the
  GPU under a byte budget, `near_field.gdshader` marches it with Godot's lighting, the
  terrain shaders hide the meshes inside its valid box). P7 (`prototypes/p7-raycast-near`) measured a Grimorium-style
  GPU ray-cast near field (32 m at 6.25 cm: 3.3 ms p50); ADR 0012 (accepted)
  makes the simulation resolution the next spike and requires per-frame edit
  budgets for mass edits. **Direction (ADR 0012, accepted in principle):
  option B** = the 6.25 cm cell is the world grid, stored as bricks that are
  today's 0.5 m voxels (uniform brick = 1 byte, expanded = 512 cells);
  ray-cast 32 m near field fed by the simulation per brick; godot_voxel
  meshes the uniform view with LOD beyond; chisel = brick expansion; every
  edit path under a per-frame byte budget. Steps, each gated by a spike:
  (1) simulation at micro resolution on expanded bricks (player-loop
  session), (2) brick world store + feed (P9, generator session), (3) the
  game switches its world store to bricks. Step (1) in the game ✓ (ADR 0029, `micro_sim=1`, off by
  default: `scripts/micro_bubble.gd` runs a toroidal 32 x 16 x 32 m `SimWorld` at 6.25 cm inside the
  bubble, whose chunks there are held and get the bricks' majority; changed bricks are published into the
  store as cells; plants are their pattern cells, columns fall together). **Next:** chisel as brick
  expansion (fable-backlog job 2: edits into the micro cells replace ADR 0006's overlay meshes), P4 server
  headless check.
- In-world voxels follow ADR 0006: terrain ids + one block-entity layer
  (`scripts/blocks/`) + reserved decor layer; every voxel chiselable on an
  8³ cell grid (6.25 cm) through the native `MicroMesher`, which also builds
  all micro models (`materials.gd` `micro:` kinds). Material ids are
  definition order: 32..36 stations, 37..43 detail, 44 `CHISELED` reserved;
  append only; 47 = blood (ADR 0015); 48..52 ground pickups (ADR 0019); 57..75 animals/farming/furniture
  (ADR 0022). Atlas 8×12 tiles (ADR 0024, 0030; 84..87 and 93..95 free, grow ROWS for more); 103..107 = Flint Age depth (thatch, wattle, daub wall, torch, storage pot, ADR 0030); 76..87 = content pass (tree kinds, terracotta, basalt, packed ice, fern), 90..101 = content pass 2 (birch, blueberry bush, pumpkin, flowers, bark). 88..89 = work spots (ADR 0023: drawn by `scripts/work/` entities from `MicroMesher.build_arrays` with the winding flipped in the arrays; `MobModel.outward` reads an ArrayMesh back, which waits 4-6 ms for the render thread, so never call it per edit). MicroMesher winds
  triangles counter-clockwise: pass its meshes through `MobModel.outward` (terrain models, mobs, furniture, the ape).
- Two sessions share this checkout (this one and "PC testing and
  optimization"): stage files explicitly, never `git add -A`; Biome enum ids
  0..12 in world_gen.h stay stable; biome_table.gen.h is generated.
- Phase 3: wands/spells, machines, recipe browser depth (machine recipes, bookmarks)
- Phase 4: automation depth, dedicated server on Unraid, auto-update

## Conventions

- Dev box: Windows 11, RTX 3070 8 GB, 32 GB RAM. Shell: PowerShell / Git Bash.
- Commits: imperative subject, body explains *why*.
- Prototypes are throwaway: one folder each, README states question + measured answer.
- Tools live under `~\Tools\` (the user's home), each in its own folder; the repo
  never names a person or a machine, only this home-relative layout.
