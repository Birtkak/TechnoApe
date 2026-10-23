# Fable backlog

- **Written:** 2026-10-23
- **Purpose:** the jobs that need the strongest model (Claude Fable 5.1): a
  wrong call costs weeks, threads and budgets interact, or the job ends in an
  ADR. Everything that ends in JSON, a GDScript UI, or a script following an
  existing pattern goes to a cheaper model (list at the bottom).
- **How to start one:** "Read `docs/plans/fable-backlog.md` and do job N."
  Each job lists what to read first, what is already known, and what done
  looks like. Tick a job off here (and in CLAUDE.md's phase plan) when it lands.
- Next free ADR number: **0028** (0022 = animals, farming and furniture; 0023 = Vintage Story early game; 0024 = content pass; 0025 = calendar and sky; 0026 = particles; 0027 = wildlife).

Recommended order: 1, 2, 3 (ADR 0012's remaining steps, they depend on each
other), then 4, then 5-7 as Phase 3/4 start. 8 is ongoing.

---

## 1. Micro-resolution simulation on expanded bricks (ADR 0012 step 1 in the game)

**Read first:** ADR 0012, ADR 0013, ADR 0021, `docs/design/brick-world.md`,
`docs/design/material-simulation.md`, `docs/research/2026-10-22-core-architecture-sim.md`,
`prototypes/p10-micro-sim/README.md`, `prototypes/p5-sim-spike/src/sim_world.h`
and `brick_store.h`, `game/scripts/sim_bubble.gd`.

**Where it stands (checked 2026-10-23):**
- The game's bubble ticks at **0.5 m**: one sim cell = one brick.
  `SimBubble._collect_dirty` calls `SimWorld.write_dirty_to_store(store, ..., 8)`
  (scale 8). The scale-1 path (one sim cell = one store cell) exists in the
  kernel but nothing in the game uses it.
- P10 kernel v6 passed gate 1 **with conditions** at 6.25 cm: background tick,
  distance LOD, per-tick cell budget (1.5 M cells: 7.0 / 7.9 ms worst case),
  static ground that loosens (ADR 0013), liquid fullness levels. Bulk loose
  material is the expensive case (14-19 ms/tick before the budget).
- ADR 0012 conditions still to meet in the game: dense sim window 24-32 m
  wide by 16 m tall inside the 32 m render window (6 B/cell), uploads by dirty
  box or brick, and a **brick-sparse kernel** (only expanded bricks resident)
  as the later step.
- `BrickStore` exposes `raw_cells_mut` / `raw_cells` / `raw_ref` for native
  sim access. Uniform bricks are walls or air to the sim; a moving cell at a
  uniform brick's border must expand it first; a settled homogeneous brick
  collapses (`collapse_dirty`).
- Micro vegetation cells in the store hold `128 + atlas tile` (paint, not
  material, ADR 0021): the sim must treat ids >= 128 as decoration.

**Open design questions for the ADR:**
- Dense 6.25 cm window vs brick-sparse kernel straight away. A dense 32 m
  cube is 134 M cells; at 6 B/cell that breaks the 4 GB RAM budget, so the
  dense window must be the smaller 24-32 m x 16 m box, or go sparse.
- What the far field (godot_voxel, 0.5 m) gets from a micro sim: majority per
  brick, and how often (push budget, ADR 0010 / ADR 0020 remesh rate).
- Fluid levels (`FLUID_LEVEL_BASE` eighths) at micro scale: still needed when
  cells are 6.25 cm?
- Player, items and collision (ADR 0011, `VoxelBoxMover`) read 0.5 m voxels:
  which view of an expanded brick is solid?

**Done looks like:** a spike (`prototypes/p12-...` or in the game behind a
flag such as `micro_sim=1`), ADR 0028 (or the next free number), every ADR 0014 scenario (fly, fast
flight, walk + mine + place, forest fire, water spill, 8 m blast, 50 craters)
inside the ADR 0009 budget on the dev box with the table in the ADR, gdUnit
tests for the kernel, a filmstrip of falling sand and flowing water at micro
resolution in the near field. If the budget cannot be met: ADR 0012's
fallback is 12.5 cm cells.

## 2. Chisel as brick expansion (replaces ADR 0006's overlay meshes)

**Read first:** ADR 0006, ADR 0007, ADR 0012, ADR 0021 (its "Open" bullet),
`docs/design/brick-world.md` ("Building" and migration step 4),
`prototypes/p6-chisel/README.md`, `game/scripts/blocks/`, `game/scripts/player.gd`,
`game/scripts/world.gd`, `prototypes/p5-sim-spike/src/micro_mesher.*`.

**Where it stands:**
- ADR 0006 is still **proposed** and describes chiseling as a `Chisel` block
  entity + `CHISELED` terrain id (44, reserved in `materials.gd`) + overlay
  meshes from `MicroMesher`. ADR 0012 and brick-world.md replace the overlay
  part: a chiseled voxel is an **expanded brick**; the entity layer stays for
  function only (chest, furnace).
- The store is a wrapped 128^3-brick window written by the bubble. Its write
  rule already protects chisel detail: a brick is rewritten only when its
  material (majority, for an expanded brick) differs.
- The store is **not saved** and does not exist outside the window; the far
  field keeps godot_voxel's own data. A chiseled brick needs a persistent home
  (brick-world.md: saved regions keep expanded bricks as 512-byte blobs behind
  a brick index) before it survives leaving the window or a reload.

**Pieces:**
1. Edit path: chisel tool -> `World` -> store cells (`set_cell`, `fill_cells`),
   brush 1/2/4/8 cells, Vintage Story modes (ADR 0006).
2. Picking: ray hit on the 0.5 m voxel, then refine against the brick's 512
   cells in C++ (the near-field shader's DDA is the reference).
3. Far field and gameplay views: the brick's majority material (or `CHISELED`
   when mixed) for godot_voxel, collision and the 0.5 m sim. Decide what is
   solid for `VoxelBoxMover`.
4. Persistence: expanded bricks saved per chunk, versioned apart from
   `GEN_VERSION`, loaded back into the store when the window reaches them.
5. Beyond the near field (32 m) chiseled bricks show as their majority voxel:
   check distant builds do not look bitten (ADR 0006's known gap).
6. Drop and pick up a chiseled block as an item carrying its cells.
7. Rewrite or supersede ADR 0006 so doc and code agree.

**Done looks like:** chisel works in play inside the near field, save and
reload keep the shape, a screenshot pair (near and beyond 32 m), gdUnit tests
for edit path, picking and persistence, a budget check of a mass chisel edit
under ADR 0012's mass-edit rule (per-brick dirty tracking, per-frame byte budget).

## 3. The far field reads the store (brick-world.md step 3b)

godot_voxel still keeps its own voxel data, fed by the bubble's pushes. Step
3b makes it read the store's majority view, so LOD 0 can stop at the near
field's edge (today meshes inside the near-field box are still built and drawn,
then discarded per pixel). Job 2 needs it for persistence; it also saves VRAM
and meshing time. Done: LOD 0 radius at the near-field edge, ADR 0014 matrix
in budget, no seam visible at 32 m (debug tint off and on).

## 4. Near field on by default (ADR 0021 follow-ups)

- Pack uploads on a worker thread (the first fill costs 6-7 ms per frame on
  the main thread, hidden by the loading screen today).
- Stains (ADR 0015) and animated water / lava / fire tiles in
  `near_field.gdshader` (shader work on `surface_stain` / `terrain_sample`).
- Water tint across the 32 m seam: **taste call for the developer**. Show a
  screenshot pair and ask.
- Flip `Settings.near_field` to true only after the developer has played it.
- Remove the unused `game/shaders/occ.glsl` and `scatter.glsl` copies.

## 5. P4 server headless check and the delta protocol (Phase 4 groundwork)

ADR 0002 (Linux dedicated server, Docker on Unraid, Tailscale). Design first:
server-authoritative sim of bricks, dirty bricks as deltas (4 B uniform, 512 B
expanded, compressed), what clients predict. Spike: a headless server ticking
the bubble, one client receiving deltas, bandwidth and CPU measured. The
Unraid container and auto-update plumbing afterwards is cheaper-model work.

## 6. Wand and spell system design (Phase 3)

Vision: Noita wandcrafting, wands found never crafted, spells crafted, spells
act on the material simulation. Fable part: the ADR (a wand as a container of
modifiers and projectiles, cast order, mana and recharge, how a spell edits the
sim under the mass-edit rule), the native evaluator, and the data schema with
tests. Individual spells afterwards are data for a cheaper model.

## 7. Machine, energy and fluid network architecture (Phase 3/4)

GregTech depth: machines as block entities (ADR 0007 layer), power networks,
pipes, multiblocks. Fable part: the ADR and the network solver (native if it
iterates per block), tick budget, how machines keep a sim bubble alive, save
and sync. Each machine afterwards is data.

## 8. Budget regression hunts (ongoing)

Whenever an ADR 0014 scenario leaves the ADR 0009 budget. Mind the benchmark
pitfalls: wall clock not delta, no console prints, one Godot on the machine
(`other_godot_processes == 0`).

---

## Not Fable: hand to a cheaper model

- Recipe browser depth (machine recipes, bookmarks).
- ADR 0019's remaining progression systems once its rules are written.
- More content passes (ADR 0024 did tree kinds, strata, sea ice, ice spikes,
  basalt, ferns; atlas 8x10 with tiles 73..79 free): per-wood planks, saplings,
  birch, item sprites for new tools.
- New spells and machines once jobs 6 and 7 exist.
- Playtest tools, chat commands, bug-report triage.
- Server deployment on Unraid, auto-update, Linux CI.
- Dungeons generator (parked), following the world-gen patterns.
