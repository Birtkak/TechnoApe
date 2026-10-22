# Foundation research – engines, AI workflow, procgen, crafting

- **Date:** 2026-10-21
- **Goal:** pick a foundation for an AI-assisted ("vibe coded") open-world game
  with in-depth crafting and procedurally generated worlds. Favour long-term
  solidity over speed; test alternatives with prototypes before committing.
- **Dev machine:** Windows 11, RTX 3070 (8 GB), 32 GB RAM. Installed: git, gh,
  Node 22, Python 3.12, .NET 8. Not installed: Godot, Unity, Rust.

---

## 1. Engine options for AI-assisted development

What matters for an AI-driven workflow (consistent across the sources):

1. **Text-based project files.** The AI must be able to read and write scenes,
   not just scripts. Binary scene formats (Unity, Unreal) leave the AI blind.
2. **Headless verification.** The AI needs a CLI it can run: build, run tests,
   launch the game, capture a screenshot, read the log. Without this, the human
   becomes the only feedback loop.
3. **Training-data depth.** Models write C# and GDScript well. Rust/Bevy and
   fast-moving APIs cause hallucinated or deprecated calls.
4. **Editor awareness.** MCP servers that expose the scene tree reduce invented
   node paths.

| Engine | AI fit | Open-world/procgen fit | Notes |
|---|---|---|---|
| **Godot 4.7** (GDScript + C#, MIT) | **Best.** Text `.tscn`/`.gd`, headless CLI, 5+ MCP servers, two mature test frameworks (GUT, gdUnit4). | Good. Terrain3D (heightmap, v1.0.2, 4.4k stars) and godot_voxel (voxel/SDF, 4k stars, active Oct 2026) are both C++ and production-grade. | Script perf is the weak spot: GDScript roughly 4x slower than C#, C++ (GDExtension) far faster for hot loops. Plan: GDScript/C# glue, native for generation. |
| **Unity 6** (C#) | Good for code, poor for scenes (YAML but noisy; prefabs/meta files). Models know C# very well. | Very good. DOTS/ECS, mature terrain and asset ecosystem. | Licensing churn history; heavy editor; harder headless loop. Reasonable fallback. |
| **Unreal 5** (C++/Blueprints) | Weak. Blueprints are binary, C++ compile loop is slow, PCG framework is graph-based. | Excellent tech (Nanite, PCG, World Partition). | Overkill for a solo AI-assisted project; AI can't edit Blueprints. |
| **Bevy 0.20** (Rust, ECS) | Weak-to-medium. API breaks every ~3 months, thin corpus so models hallucinate. | Good architecture for simulation-heavy worlds; voxel crates exist. | No stable editor. Only sensible if you *want* Rust. |
| **Web stack** (TypeScript + Three.js/Babylon) | Excellent. AI writes TS fluently, instant reload, trivially headless. | Weak for a serious 3D open world: no built-in physics/nav/terrain, browser memory limits, perf ceiling. | Great for 2D or for throwaway procgen visualisers. |
| **Custom engine** (Zig/C + SDL, MonoGame) | Full control, AI writes plain code well. | Everything hand-built: renderer, physics, tooling. | One cited dev tried this route and came back to Godot. Not recommended. |

**Recommendation:** Godot 4.7 as primary candidate. Validate with prototypes
(see section 5) rather than assume. Unity is the fallback if Godot's performance or
tooling disappoints in the spike.

Sources:
- [AI coding tools for game dev, first-principles analysis](https://chierhu.medium.com/ai-coding-tools-for-video-game-development-a-first-principles-analysis-of-what-actually-works-90dfa10edd13) – ranks Godot + Claude Code headless as strongest pairing; notes Bevy hallucination problem.
- [Unity vs Godot 2026 with AI tools](https://ziva.sh/blogs/unity-vs-godot-2026-ai) – C# training depth vs Godot text formats.
- [Best AI tools for Godot 2026](https://www.summerengine.com/blog/best-ai-tools-for-godot) (vendor; grain of salt).
- [Bevy indie guide 2026](https://www.strayspark.studio/blog/bevy-rust-game-engine-2026-indie-guide).
- [GDScript vs C# perf (Godot forum)](https://forum.godotengine.org/t/state-of-gdscript-vs-c-performance-in-godot-4-0/5875), [Godot scripting-languages doc](https://docs.godotengine.org/en/4.x/getting_started/step_by_step/scripting_languages.html).

### Godot tooling worth adopting
- **MCP servers:** [Coding-Solo/godot-mcp](https://github.com/Coding-Solo/godot-mcp) (6.2k stars, Node; run project, capture output, edit scenes; last push Apr 2026), [beckett-godot-mcp](https://github.com/beckettlab/beckett-godot-mcp) (in-editor, no Node dep, Oct 2026), [Godot Catalyst](https://godotengine.org/asset-library/asset/5042) (240+ tools).
- **Tests:** [gdUnit4](https://github.com/MikeSchulze/gdUnit4) (GDScript + C#, very active) or [GUT](https://github.com/bitwes/Gut). Both run headless via CLI.
- **Headless loop:** `godot --headless` for tests/exports, a screenshot script for visual QA, GitHub Actions for CI.
- [cc-godot plugin for Claude Code](https://github.com/zate/cc-godot) and [godogen](https://gigazine.net/gsc_news/en/20260317-godogen/) (two-skill plan/execute pipeline with screenshot QA) show the pattern.

---

## 2. AI-assisted ("vibe coding") workflow – what actually works

From Anthropic's [Claude Code best practices](https://code.claude.com/docs/en/best-practices) and game-dev field reports:

1. **Give the AI a check it can run.** Tests, build exit code, a screenshot
   diff. The most-cited failure: "tests green, game unplayable." Pure logic
   (worldgen, crafting, inventory) must be unit-tested; feel/fun is the
   human's job and needs a playable build each milestone.
2. **Explore, plan, implement, commit.** Use plan mode for multi-file or
   uncertain work; skip it for one-line changes.
3. **Written design is the ground truth.** One dev's Claude built a wrong
   character controller from contradictory notes; pointing it at the design doc
   fixed it. Keep `docs/design/` authoritative and the code consistent with it.
4. **Lean CLAUDE.md.** Only what the AI can't infer from code: commands,
   conventions, gotchas, decisions. Prune when rules get ignored. Domain
   knowledge goes in `.claude/skills/` loaded on demand.
5. **Precise prompts, one task each.** Vague prompts make the AI over-infer
   from surrounding code.
6. **Definition of Done includes self-verification** before asking the human.
7. **Fresh-context review.** A subagent reviewing the diff catches what the
   author-context misses.
8. **Stay in touch with the code.** The RTS experiment was scrapped because the
   dev lost the ability to fix things themself. Mitigation: ADRs, short
   architecture notes, and the human reading/playing every milestone.

Sources: [Building an RTS in Godot with Claude](https://dev.to/datadeer/part-1-building-an-rts-in-godot-what-if-claude-writes-all-code-49f9),
[Claude Code for game development](https://chierhu.medium.com/claude-code-for-game-development-7a88fcd19992),
[Decrypt vibe-coded game experiment](https://decrypt.co/311183/we-built-game-vibe-coding-ai-claude).

---

## 3. Procedural world generation – approaches

### 3a. World representation (the big fork)
| Representation | Pros | Cons | Godot tooling |
|---|---|---|---|
| **Heightmap terrain** | Huge view distance, cheap, pretty; easy biomes/rivers/roads | No caves/overhangs, limited terraforming | Terrain3D, SimpleXTerrain (node-graph streaming) |
| **Blocky voxels** (Minecraft-like) | Fully editable/destructible, caves, building and crafting synergy, simplest mesher | Distinct look; LOD and far distance harder | godot_voxel VoxelMesherBlocky |
| **Smooth voxels / SDF** (Transvoxel) | Natural look plus editable plus caves | Most complex: LOD stitching, physics, texturing | godot_voxel VoxelLodTerrain + Transvoxel |
| **2D tilemap / top-down** | Cheapest, fastest iteration, massive content density per effort | Not 3D | Godot TileMapLayer + FastNoiseLite |

This choice drives everything (rendering, physics, save format, how crafting
interacts with the world). It should be the first ADR after the engine.

### 3b. Generation techniques (apply to any representation)
- **Layered noise with splines** (Minecraft 1.18 model): continentalness,
  erosion, peaks/valleys, weirdness mapped to height via spline curves;
  temperature + humidity mapped to biome via nearest-match in parameter space.
  Gives varied, explorable terrain rather than uniform hills.
- **Red Blob fundamentals:** octaves, redistribution exponents, elevation x
  moisture biome grid, Poisson-disc object placement, independent seeds per
  layer. "Start extremely simple, then replace."
- **Chunked streaming:** chunk = load/unload unit; macro-cell = biome unit;
  generate on worker threads; cap chunks applied per frame; biome cleanup
  pass merges tiny fragments.
- **Deterministic seeds everywhere** so worlds are reproducible and testable.
- **Noise can't do global features** (rivers, roads, settlements). Plan a
  second pass: graph-based rivers, POI placement, structure stamping.

Sources: [Red Blob – terrain from noise](https://www.redblobgames.com/maps/terrain-from-noise/),
[How Minecraft generates worlds (Kniberg talk write-up)](https://dawnosaur.substack.com/p/how-minecraft-generates-worlds-you),
[Godot forum: infinite procedural world scheduler](https://forum.godotengine.org/t/infinite-procedural-world-how-to-implement-a-high-performance-map-with-pan-zoom-interactive-pois/135066),
[godot_voxel docs](https://voxel-tools.readthedocs.io/en/latest/overview/),
[Terrain3D](https://github.com/TokisanGames/Terrain3D), [SimpleXTerrain](https://store.godotengine.org/asset/prajwal-m/simplexterrain/).

---

## 4. In-depth crafting – design and architecture

### What makes crafting "deep" (community and design consensus)
- **Layered chains:** raw, refined, intermediate, end product, with
  intermediates reused across many recipes (Factorio, modded Minecraft).
- **Multiple sources per ingredient** so routes differ between playthroughs.
- **Mechanical consequences:** crafted items change survival/progression, not
  just cosmetics (Valheim, Vintage Story).
- **Tactile process** where it matters: stations, multi-step forging, timing
  (Vintage Story anvil). Depth is not more recipes; depth is decisions.
- **Automation axis** is separable from survival axis (Factorio vs Valheim).

### Architecture that supports depth and AI editing
- **Data-driven everything:** items, recipes, stations, loot tables in
  JSON/TRES/YAML files, validated by a schema and tests. Minecraft's JSON
  recipe system is the reference model.
- **Tag-based ingredients** (ECS-inspired): recipes require tags
  (`metal.ingot`, `wood.plank`) rather than specific items, giving emergent
  substitutions and far fewer recipes to author.
- **Item properties, not item types:** quality, durability, material carry
  into the product (composition over inheritance).
- **Recipe graph as a first-class object:** enables tech-tree UI, balance
  analysis (cost-to-craft), and automated "is everything reachable?" tests.
- **Separate pure logic from engine:** crafting/inventory rules as plain
  classes with no scene dependencies, so they run in fast headless tests.

Sources: [GDC: Astroneer crafting overhaul](https://gdconf.com/news/go-behind-scenes-astroneers-big-crafting-overhaul-gdc-summer),
[gamedev.net: tag-based crafting](https://gamedev.net/forums/topic/715034-designing-a-data-driven-crafting-system-using-tags/),
[Minecraft Forge recipe docs](https://docs.minecraftforge.net/en/latest/resources/server/recipes/),
[Steam thread on crafting depth](https://steamcommunity.com/discussions/forum/12/601891467285260592),
[Destructoid survival crafting list](https://www.destructoid.com/survival-games-with-the-best-crafting-systems/).

---

## 5. Proposed validation prototypes (before any ADR is "accepted")

| # | Prototype | Question it answers | Budget |
|---|---|---|---|
| P1 | Godot 4.7 headless loop: gdUnit4 tests + CLI run + screenshot, wired into Claude Code | Can the AI verify its own work end-to-end? | half day |
| P2 | Chunk-streaming spike in chosen representation; GDScript vs C# vs GDExtension hot loop benchmark (chunks/sec, frame time) | Which language for generation? Is Godot fast enough? | 1–2 days |
| P3 | Pure-logic crafting core (tags, recipe graph, tests), no scenes | Is the data model right? Does it test well? | half to 1 day |
| P4 (optional) | Same streaming spike in Unity for comparison | Only if P2 disappoints | 1 day |

## 6. Open decisions (taste/direction – see ADR 0001 draft)
1. Perspective and dimension: 3D first-person / 3D third-person / 2D top-down.
2. World representation: heightmap / blocky voxel / smooth voxel / tilemap.
3. Multiplayer: never / maybe later / core. (Changes architecture from day 1.)
4. Art direction target (low-poly, voxel, stylised, pixel) – affects asset pipeline.
5. Engine shortlist to actually prototype: Godot only, or Godot + Unity.
