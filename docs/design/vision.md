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

### The world looks and plays like Lay of the Land (2026-10-23, ADR 0033)
- **12.5 cm voxels everywhere**, one grid for gameplay, simulation,
  collision and rendering; fully destructible; the Noita simulation runs on
  the voxels you see. Trees are cut voxel by voxel and fall; undermined
  roofs collapse.
- **Flat-coloured voxels** (per-material palettes, jittered per voxel), soft
  light, AO and atmosphere; no terrain textures.
- **Brushes and amounts:** tools dig blobs, the inventory holds material
  amounts in Minecraft-style slots, building places shapes (cube, cylinder,
  cone, slope, path).
- Plan: `docs/plans/lotl-overhaul.md`, behind `world_v2=1` until it beats
  the 0.5 m world. Where this section disagrees with older text below
  (blocky 0.5 m voxels, 16x16 textures, mining single blocks), this wins.

### Direction: LotL feel, VS and GregTech depth (2026-10-24, ADR 0037)
Decided system by system after comparing Lay of the Land and Vintage Story; where this section
disagrees with older text, this wins.
- **Simulation:** the Noita kernel plus rigid voxel pieces: felled trees, cut pillars, undermined
  ledges and in time whole buildings break off, fall, crush and settle.
- **Building:** LotL shape building from the start (shapes from a radial menu, a preview with drag
  handles, rotate/mirror, click away to commit), box mode, prefabs; the brush is "normal" mode.
- **Crafting:** tools and gear are made in the world (knapping, assembling on the ground or a
  workbench, anvil, moulds, firepit); the grid stays for bulk and simple recipes.
- **Progression:** ages into GregTech; exploration gates materials; no boss loop.
- **Survival:** hunger, body temperature and food spoilage, the same in every world.
- **Threat:** biting wildlife and dangerous places (dungeons, deep caves, guarded ruins); a calm
  surface; combat is a side system for now.
- **Magic:** decided in Phase 3.
- **Art:** palettes for nature, patterned voxels for crafted building materials, then a grade.
- **Places:** structures with loot, dungeons, cave biomes, roads and bridges, traders.
- **Multiplayer:** co-op ready, server in Phase 4; new systems keep state server-side.

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

### Progression feel (2026-10-22)
- **Slow-burning early game: GregTech: New Horizons meets Vintage Story.**
  Flint tools first, shave bark, sift gravel for nuggets; copper before
  iron; an iron pickaxe takes real grinding. Ages: flint, copper, bronze,
  iron, steel. Plan: `progression.md`, ADR 0019.
- Vintage Story smithing (heat, anvil, hammer to a shape), food and cooking
  with a **hunger bar** (survival pressure: yes, hunger), beds that skip the
  night, saplings, a sun and moon, and loose pickups on the ground
  (stones, flint, sticks, ore-bearing stones above veins).

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
6. **A horizon that never looks worse than the world.** The voxel world is
   one box of full-detail voxels, 40 m around the player, and ends in the
   sky (ADR 0073): everything drawn is the real thing, editable and
   simulated, and the last quarter blends into the sky's own colour. There
   is no level of detail: the 2 km LOD horizon of the early plan (ADR 0020,
   "Voxy / Distant Horizons") and the 200 m one of ADR 0072 looked worse
   than nothing at 12.5 cm voxels, streamed too slowly in flight, and hid
   the detail under the player behind coarse parents. Distant mountains
   and far builds come back only with a technique that draws them well (a
   painted far layer or impostors), decided then.

## Fixed decisions (see ADRs)
- Perspective: 3D first-person.
- World: blocky voxels, chunk-streamed, infinite or very large.
- Scale: **12.5 cm voxels**, world 384 m high (ADR 0033, replacing ADR
  0004's 0.5 m once `world_v2` goes default).
- Look: flat per-voxel colours, Lay of the Land style (ADR 0033, replacing
  ADR 0004's 16×16 textures for terrain; item icons stay pixel art).
- Multiplayer: not in scope now; architecture must not block co-op later.
  Target: dedicated Linux server in Docker on Unraid, Tailscale network (ADR 0002).
- Platforms: Windows client, Linux server (ADR 0002, Android dropped in ADR 0009).
- Updates: CI-built, manifest-driven auto-update from the server (ADR 0002).
- Engine: Godot 4.7 (pending prototype confirmation, ADR 0001).

## Open design questions (decide when they become relevant)
- Art direction: decided 2026-10-24 (ADR 0037: palettes for nature, patterns for built
  materials); the lighting grade is still a taste call.
- Survival pressure: decided 2026-10-24 (hunger, temperature, spoilage).
- Automation axis: decided 2026-10-24 (ages into GregTech machines and automation).
- World scale: decided 2026-10-23 (12.5 cm voxels, 384 m high, ADR 0033).
- Combat: decided 2026-10-24 as a side system for now; revisit when dungeons exist.
- Magic: the wand design and how it meets the tech tree, in Phase 3.
