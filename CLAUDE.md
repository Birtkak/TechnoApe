# CLAUDE.md – TechnoApe

3D first-person voxel game: Minecraft/GregTech building and automation,
Noita-style simulated materials and wand/spell play, procedurally generated
world with 2 km LOD horizons. Windows + Android clients, dedicated Linux
server on Brent's Unraid box over Tailscale. Developer: Brent (solo). Claude
Code does most implementation; Brent steers, plays, decides.

Ground truth: `docs/design/vision.md`. Decisions: `docs/decisions/`.
Research: `docs/research/`. Prototype results: `prototypes/*/README.md`.

## Working principles (agreed 2026-10-21)

1. **Foundation over speed.** Spike alternatives in `prototypes/` and measure
   before committing. Current focus: engine + voxel simulation core.
2. **Every significant choice gets an ADR** (`docs/decisions/NNNN-title.md`).
3. **Prompt Brent only for taste/scope/direction calls.** Routine technical
   calls: sensible default, recorded.
4. **Give yourself a check to run** before asking Brent to look: gdUnit4 for
   logic, headless run for scenes, screenshot for visuals. Show the evidence.
   Commands: see `.claude/skills/godot-cli/SKILL.md`.
5. **Design doc wins over code** when they disagree; fix explicitly.
6. **Data-driven systems:** items, recipes, materials, biomes as data + schema + tests.
7. **Simulation separate from presentation**, deterministic from seed,
   authoritative on the server, deltas over the wire.
8. **GDScript orchestrates, never iterates voxels.** Per-voxel work is C++
   (GDExtension). C# is not used in the shipped game (Android export is
   experimental); it may be used for tooling.
9. **Performance rules (Brent):** multithread everything heavy, budget RAM
   and disk explicitly, always pick the discrete GPU and log it, ship every
   system with a benchmark. See vision.md "Engineering requirements".
10. **Debug tools are for Claude too.** Anything about the running game must
    be reachable without eyes: JSON stats on stdout, log files, state dumps,
    and frame capture. To *watch* motion, capture a filmstrip (thumbnail
    every N ms) and a burst (consecutive frames) and read the sheet; see
    `prototypes/p5-sim-spike/frame_sheet.gd`. Never judge animation from one
    screenshot.

## Stack (ADR 0001 accepted, 0002 accepted, 0003 proposed)

- Godot 4.7.2 .NET at `C:\Users\dev\Tools\Godot\4.7.2-mono\Godot_v4.7.2-stable_mono_win64\`
  (`_console.exe` for CLI). .NET SDK 8.0.426 (tooling only).
- godot_voxel GDExtension (master CI build 2026-10-01 at
  `C:\Users\dev\Tools\downloads\libvoxel-36898667256\`; pin a tag before Phase 2).
  Terrain: `VoxelLodTerrain` + `VoxelMesherBlocky` + `streaming_system = CLIPBOX`.
- Tests: gdUnit4 v6.2.2 (source at `C:\Users\dev\Tools\downloads\gdUnit4\`).
- C++ toolchain: portable MinGW-w64 GCC 16.2 at
  `C:\Users\dev\Tools\mingw-dl\mingw64\bin` (add to PATH), SCons via
  `python -m SCons`, godot-cpp branch 4.5 at `C:\Users\dev\Tools\godot-cpp`.
  Build an extension: `python -m SCons platform=windows use_mingw=yes target=template_debug -j12`.
  MSVC 2022 Build Tools (cl 14.44) are installed too; MinGW stays the default.
- JDK 17 (no admin, zip): `C:\Users\dev\Tools\jdk-dl\jdk-17.0.20.1+1` (set JAVA_HOME
  for Android exports). Not yet installed: Android SDK command-line tools.
- World generator is native (`src/world_gen.cpp`); design in
  `docs/design/world-generation.md`. godot_voxel asks for padded non-cubic
  blocks, so generators take a Vector3i size.

## Game project (`game/`)

- Run / verify: see `game/README.md`. Unattended check:
  `Godot_..._console.exe --path game -- autopilot=1 duration=20 capture_every=1 vsync=0 out=res://out/runN`
  then read `game/out/runN/strip.png` and the JSON line on stdout.
- One world unit = one voxel = 0.5 m. Tunables live in `scripts/settings.gd` in metres.
- Rebuild the sim extension from `prototypes/p5-sim-spike` and copy the DLL to
  `game/addons/technosim/bin/`.
- GitHub: https://github.com/Birtkak/TechnoApe (private). `PC-tuneup/` in this folder
  belongs to another session and is gitignored.

## Phase plan

- Phase 0: research + direction – done 2026-10-21
- Phase 1 prototypes: P1 headless loop ✓, P2 voxel streaming/LOD ✓,
  P5 Noita-model simulation core ✓, P3 crafting core ✓ (17 headless tests)
- Phase 2 (current): `game/` skeleton ✓ (LOD world, textures, player, sim
  bubble, debug capture), native world generator ✓ (splines, caves, ores,
  biomes, trees, ruins, hydrology rivers/lakes/springs, altitude bands,
  seeds), player loop ✓ (ADR 0005: hotbar/inventory/crafting screens,
  item entities, mining table, placing, buckets, hearts, viewmodel; 16
  headless tests + autopilot `mine=1` / `place=1` checks). **Next:**
  crafting table / furnace / chest blocks (materials + tiles + block
  entities), JEI-style browser UI, bubble polish (spikes on shift,
  fire-spread tuning), P4 server headless check.
- A parallel session ("PC testing and optimization") adds biomes.json +
  biome registry/rules/tests/docs as new files only; Biome enum ids 0..12
  in world_gen.h must stay stable; it generates biome_table.gen.h.
- Phase 3: wands/spells, machines, JEI-style browser UI, Android export smoke test
- Phase 4: automation depth, dedicated server on Unraid, auto-update

## Conventions

- Dev box: Windows 11, RTX 3070 8 GB, 32 GB RAM. Shell: PowerShell / Git Bash.
- Commits: imperative subject, body explains *why*.
- Prototypes are throwaway: one folder each, README states question + measured answer.
- Downloads go under `C:\Users\dev\Tools\`, each in its own folder.
