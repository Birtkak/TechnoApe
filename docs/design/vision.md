# TechnoApe – Vision

- **Date:** 2026-10-21
- **Status:** living document; this is the ground truth the code must agree with.

## One-liner
A 3D first-person open-world game in a fully diggable, buildable blocky-voxel
world, generated procedurally from a seed, where survival and progression run
through a deep, layered crafting system.

## The big idea (2026-10-21)
A voxel game that mixes **Minecraft with GregTech vibes** and **Noita**:
- **Voxels are simulated like Noita.** Materials have physical behaviour:
  sand falls, liquids flow and pool, gases rise, fire spreads, acid eats,
  things react when they touch. The world is a living material simulation,
  not static blocks.
- **Wandcrafting and spell management like Noita.** Wands are containers of
  spell modifiers and projectiles; the player builds spell chains; spells
  interact with the material simulation (burn, freeze, dissolve, explode).
- **Building, automation and crafting are Minecraft-based** (GregTech
  depth): machines, pipes, multi-stage processing, tech tiers, big factories.
- **Crafting, spells and exploration are a mix of both**: materials found by
  exploring feed both the factory and the wand.

### Getting resources (2026-10-21)
- **Mining** yields metals and the materials that drive technology tiers
  (the GregTech side).
- **Noita-like dungeons** yield spell-crafting materials and are where the
  material simulation shines: dangerous liquids, gases, fire, collapsing
  structures.
- **Wands are found, never crafted.** Only spells are crafted. Wand hunting
  is an exploration reward; spell crafting is the player's expression.
- Exploration therefore feeds both loops: deeper/further for better ore,
  dungeons for spell ingredients and wands.

### Interaction model (2026-10-21)
- **Mining, picking up, building and placing are Minecraft-like**, on
  voxels: break a voxel and its matter drops as an item entity on the
  ground that you walk over to pick up; place blocks from the hotbar.
- **Liquids need a container** (bucket, flask) to pick up and carry; you
  cannot "mine" water.
- **In-world behaviour is Noita's:** once matter is in the world it obeys
  the material simulation (falls, flows, burns, reacts). The two models meet
  at the boundary: taking matter *out* of the world and putting it *back*
  is Minecraft; what it does *while in* the world is Noita.

### Inventory, HUD, crafting UI (2026-10-21)
- Inventory, hotbar/HUD, crafting grid and storage (chests) must **feel
  exactly like Minecraft**: same slot layout, drag/split/shift-click
  behaviour, stack sizes, hotbar, crafting table and chest screens. Players
  should need zero learning.
- A **built-in recipe browser like Too Many Items / JEI**: search every item
  and recipe, click an item to see how it is made and what it is used for,
  including machine recipes and spell crafting. Since recipes are data files,
  the browser reads the same data the crafting system does, so it is never
  out of date.

### Engineering requirements (2026-10-21)
- **Multithreaded** wherever it pays: simulation, meshing, generation,
  IO, networking all off the main thread; the main thread only orchestrates
  and renders.
- **Smart RAM and storage:** chunk data compressed in memory and on disk,
  region files with async IO, LOD caches with budgets, nothing loaded that
  is not needed; memory use is a setting-driven budget, not an accident.
- **Always the best GPU:** detect and use the discrete GPU, log the adapter
  on startup, and warn if an integrated GPU was picked.
- **Optimised by default:** every system ships with a benchmark and a
  budget; regressions are caught by the numbers, not by feel.
- **Sturdy debug tools, built for an AI developer as much as a human:**
  machine-readable state dumps (sim stats, chunk flags, memory, frame
  timings) via CLI flags and log files, in-game overlay and console, frame
  capture (single screenshots, filmstrips, bursts) so Claude can "watch"
  the game, deterministic seeds and replays so a glitch can be reproduced
  exactly. Nothing about the running game should be invisible to Claude.

### Focus order (2026-10-21)
Engine and voxel simulation first: sturdy and optimised before any game
content. Everything above is the target; the next prototypes exist to prove
the simulation core can carry it.

### What "simulated like Noita" means for us (assumption, to validate)
Noita simulates pixels only in the chunks around the screen and freezes
everything else. We do the same in 3D: a **simulation bubble** around each
player (and around powered machines) runs the cellular-automaton material
update; everything outside it is frozen and stored. The active volume is the
knob that scales with the machine; the dev PC is the reference (ADR 0009). Simulating every voxel of
an open world at once is not feasible on any hardware and is not what Noita
does either. This is **technical risk #1** for the project and gets its own
prototype (P5, falling-sand-3D spike) before the architecture is locked.

### Consequences for the foundation
- The voxel data layer must support **high-churn updates**: thousands of
  voxel changes per tick near the player, with cheap re-meshing of touched
  chunks. Static-world voxel engines are tuned for the opposite case.
- Material behaviour is **data-driven** (per-material rules), like items.
- The simulation is the authoritative game state on the server; clients
  receive deltas. Determinism matters (seeded RNG per chunk tick).
- The simulation core is **native code** (GDExtension C++ or compute
  shaders); GDScript orchestrates, never iterates voxels.

## Pillars
1. **The world is material.** Every block can be mined, moved, placed. Terrain,
   caves, ore veins and biomes come from a deterministic generator.
2. **Crafting is decisions, not lists.** Layered chains (raw → refined →
   intermediate → product), tag-based ingredients so substitutions emerge,
   stations and processes that matter. Depth over recipe count.
3. **Exploration pays.** Biomes and depth layers gate materials, so going
   further and deeper unlocks crafting routes.
4. **Co-op ready.** Single-player first, but world state is authoritative in a
   simulation layer that will later run as a dedicated server on the
   Unraid box over Tailscale; generation is deterministic and seed-based;
   presentation is separated from simulation.
5. **Runs perfect on ordinary gaming PCs.** One Windows client tuned against
   the dev PC as the reference machine (ADR 0009); the server pushes
   updates so friends never install builds by hand.
6. **Epic horizons.** Distant mountains and player builds are always in
   view through level-of-detail terrain (think Voxy / Distant Horizons in
   Minecraft). Up close, blocks are full quality; far away, the same world
   is drawn coarser but never missing. LOD must include player edits, not
   just generator output.

## Fixed decisions (see ADRs)
- Perspective: 3D first-person.
- World: blocky voxels, chunk-streamed, infinite or very large.
- Scale: **0.5 m voxels**, world 768 voxels high (ADR 0004).
- Look: Minecraft-style 16×16 textures from the first build (ADR 0004).
- Multiplayer: not in scope now; architecture must not block co-op later.
  Target: dedicated Linux server in Docker on Unraid, Tailscale network (ADR 0002).
- Platforms: Windows client, Linux server (ADR 0002, Android dropped in ADR 0009).
- Updates: CI-built, manifest-driven auto-update from the server (ADR 0002).
- Engine: Godot 4.7 (pending prototype confirmation, ADR 0001).

## Open design questions (decide when they become relevant)
- Art direction (textured cubes vs stylised/flat-shaded vs higher-res).
- Survival pressure level (hunger/temperature vs pure progression).
- Automation axis: does late-game crafting include machines/automation?
- World scale: block size, world height, chunk size.
- Combat: present at all, and how central?
