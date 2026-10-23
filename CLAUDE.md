# CLAUDE.md – TechnoApe

3D first-person voxel game: Minecraft/GregTech building and automation,
Noita-style simulated materials and wand/spell play, procedurally generated
world with 2 km LOD horizons. Windows client (Android dropped, ADR 0009), dedicated Linux
server on the home server (Unraid) over Tailscale. Solo developer; Claude
Code does most implementation, the developer steers, plays, decides.

Ground truth: `docs/design/vision.md`. Decisions: `docs/decisions/`.
Research: `docs/research/`. Prototype results: `prototypes/*/README.md` (p5 holds the native
source of the technosim extension, p11 the godot_voxel patches).
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
- World generator is native (`src/fine_gen.cpp` over `src/world_gen.cpp`); design in
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
- One world unit = one voxel = 12.5 cm (ADR 0033); `Settings.BLOCK` = 4 voxels = the 0.5 m block that
  stations, furniture, work pieces and recipe units keep (ADR 0034). Tunables live in `scripts/settings.gd`
  in metres: never write a distance in voxels as a bare number (convert with `Settings.m2v` or a BLOCK multiple).
- The title screen is the main scene (ADR 0032: worlds list, Settings with graphics presets in
  `user://settings.cfg`, Esc pauses with Settings and save-and-quit, the player is saved with the
  world). `autopilot=1` or `play=1` skip it; `menu_test=1` checks the menus.
- Playtest tools (ADR 0018): T chat + slash commands (`/help`), `/gamemode`, F3
  debug screen, F4 playtest menu, F9 bug reports. When the developer mentions a
  bug report, read the newest folder in `game/out/bug_reports/` (report.md,
  state.json, screenshot.png, log.txt); from the exported build (`build/windows/TechnoApe.exe`,
  export command in `game/README.md`) they are in `%APPDATA%/Godot/app_userdata/TechnoApe/bug_reports/`.
- Rebuild the sim extension from `prototypes/p5-sim-spike` and copy the DLL to
  `game/addons/technosim/bin/`.
- Item looks: `game/data/item_sprites.json` (16x16 pixel art, shared shapes +
  palettes + per-item layers), generated by `game/tools/make_item_sprites.py`
  (edit shapes there, not the JSON); every item needs a sprite or a block
  material (`test_item_sprites.gd`); `tools/item_sheet.gd` renders them all.
- GitHub: https://github.com/Birtkak/TechnoApe (private).

## Phase plan

- Phase 0 (research + direction) and Phase 1 (prototypes) done 2026-10-21; the spikes that shaped
  the retired 0.5 m / brick-world architecture were deleted with it (ADR 0036, git history keeps them).
- Phase 2 (current): **the voxel world (ADR 0033-0036).** One 12.5 cm grid for terrain, simulation,
  collision and rendering. Native `FineGen` draws the voxels over `WorldGen`'s 0.5 m macro layer
  (landforms: ~8 km continents of plains, hill country and mountain ranges; climate, biomes from
  biomes.json, hydrology, caves with pools, GregTech ore veins, ruins); aged trees with bark trunks,
  roots and clumped crowns, young trees, a forest floor of leaf litter, logs, stumps, bushes, ferns and
  mushrooms; far forests as leaf domes. Flat colours: one texel of the material's tile per voxel
  (`voxel_sample`); props use `terrain_prop.gdshader`. `SimBubble` ticks a wrapped 64 x 32 x 64 m window
  of the world's voxels (Noita kernel: sand, water, fire, gas). Brush digging and building
  (`scripts/player/brush.gd`: Ctrl + wheel size, B shape); voxel items count voxels, recipes blocks of 64
  (`ItemRegistry.units`); block entities cover footprints (`BlockEntities.footprint`). The loading screen
  covers the start, respawns and long teleports until the ground is in, with the player and animals
  frozen. Gameplay on top: player loop (ADR 0005), stations (0007), recipe browser (0008), collision and
  swimming (0011), animals, farming, furniture (0022), Vintage Story early game (0023), progression book
  (0028), Flint Age (0030), menus and saves (0032). Probes: `tools/terrain_map.gd`, `fine_gen_bench.gd`,
  `fine_probe.gd`, `face_probe.gd`. Loads take 12-17 s: budget runs use `load_cap_s=30`.
  Full detail is capped at 24 m (`Settings.MAX_DETAIL_M`): 32 m blows the budget in forests.
  **Direction (ADR 0037, 2026-10-24):** rigid voxel pieces (trees, cut pieces, later buildings topple),
  LotL shape building + box mode + prefabs from the start, tools and gear crafted in the world (grid for
  bulk), ages into GregTech, hunger + temperature + spoilage, biting wildlife and dangerous places (calm
  surface, combat a side system), patterned voxels for built materials, structures / dungeons / cave
  biomes / roads / traders, new systems keep state server-side. Magic is decided in Phase 3.
  **Next:** the work order for ADR 0037; still open from before: fire/smoke pacing per cell, fast flight vs
  generation, the water spill / blast / `persist=1` runs; plan in `docs/plans/lotl-overhaul.md`, playtest
  list in `docs/plans/to-test.md`.
- Material ids are definition order, append only (saves and the generator use them): 32..36 stations,
  37..43 detail, 44 reserved (once `CHISELED`), 47 blood, 48..52 ground pickups, 57..75 animals, farming,
  furniture, 76..87 and 90..101 content passes, 88..89 work spots, 103..107 Flint Age, 108 leaf litter.
  Atlas 8x13 tiles (84 leaf litter, 85 oak bark, 86 pebbles, 87 flint, 93/97 twigs top/bark, 94..96 ore-bearing
  stones; 98..103 free). Ground pickups are voxel shapes (stone chunks on a 0.5 m patch, sticks as `FineGen::STICK` stamps). `MicroMesher` builds the
  models of stations, furniture, work pieces, mobs and the ape at 6.25 cm cells; it winds triangles
  counter-clockwise, so pass its meshes through `MobModel.outward` (which reads an ArrayMesh back and
  waits 4-6 ms for the render thread: never per edit).
- When another session shares this checkout, stage files explicitly, never `git add -A`; Biome enum ids
  0..12 in world_gen.h stay stable; biome_table.gen.h is generated.
- Phase 3: wands/spells, machines, recipe browser depth (machine recipes, bookmarks)
- Phase 4: automation depth, dedicated server on Unraid, auto-update

## Conventions

- Dev box: Windows 11, RTX 3070 8 GB, 32 GB RAM. Shell: PowerShell / Git Bash.
- Commits: imperative subject, body explains *why*.
- Prototypes are throwaway: one folder each, README states question + measured answer.
- Tools live under `~\Tools\` (the user's home), each in its own folder; the repo
  never names a person or a machine, only this home-relative layout.
