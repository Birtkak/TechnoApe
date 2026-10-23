# Lay of the Land overhaul (ADR 0033)

- **Written:** 2026-10-23
- **Goal:** the whole world at 12.5 cm voxels, flat-coloured, fully
  destructible and simulated, dug and built with brushes. Built behind
  `world_v2=1`; the 0.5 m game keeps running until the new one beats it.
- **Rule:** each step lands only when its check passes (ADR 0009 budget on the
  dev box, measured as ADR 0014 says; screenshots or filmstrips for looks).
  Steps 1-2 gate the architecture; later steps can split across sessions.

---

## 1. P12 spike: can godot_voxel carry a 12.5 cm world? (gate)

`prototypes/p12-fine-voxels/`. Question: with a 2 km horizon, what LOD 0
radius holds p99 <= 8.3 ms, RAM <= 4 GB, VRAM <= 6 GB, and is the near ground
better meshed (a) or ray-cast over 64 m (b)?

- `WorldGen` gets a voxel size (metres per voxel). Heights, noise periods and
  feature sizes are already in metres or convert through it; the spike may
  sample today's 0.5 m fields at fractional coordinates where the real port
  is not done yet.
- Terrain: `VoxelLodTerrain` + `VoxelMesherBlocky`, one cube model per
  material, baked AO on, no atlas. A flat-colour shader picks each voxel's
  colour from its material palette by a hash of the voxel position (the face
  normal nudged inward gives the voxel).
- Measure fly 6 m/s, fast flight 40 m/s, a 1 m dig per 0.2 s and a 4 m
  blast, at LOD 0 radii of 16, 32 and 64 m; generation time per block, mesh
  time, triangles, RAM, VRAM. Variant (b): `near_field` reading the same
  voxels, meshes from LOD 1.
- **Done:** README with the table and the chosen radius and variant; a
  screenshot strip next to the 0.5 m game at the same seed and spot, shown to
  the user for the look.

## 2. The vibe pass: colour, light, atmosphere (gate for the look)

- `data/palettes.json`: palette per material (3-6 colours, weights), biome
  tints for grass and leaves; schema and tests.
- Light: mesher AO, sky ambient, sun shadows, SSAO/SSIL, volumetric or depth
  fog, AgX or ACES tonemap and a warm grade; test SDFGI cost against the
  budget. Water: flat colour with depth tint, no texture.
- **Done:** the user picks between two or three grades from a screenshot
  sheet (taste call).

## 3. Generator at 12.5 cm

Port `world_gen.cpp` features to fine voxels (see `docs/design/world-generation.md`):
- Terrain surfaces that read smooth at 12.5 cm (no 0.5 m steps): the height
  and density fields sampled per voxel; soil depth, strata bands, scree.
- Hydrology stays; channels and banks shaped at the finer grid. Roads between
  ruins and settlements are new (Lay of the Land routes them; path cost over
  slope and water).
- Trees as voxel shapes: trunk taper, roots, branches, leaf clusters per
  species, deterministic per tree. Boulders, logs, bushes, grass tufts and
  flowers become 12.5 cm voxel stamps (the 6.25 cm micro models halve).
- Caves, ores, ruins at the new scale. `GEN_VERSION` bump.
- **Done:** `gen_probe.gd` timings per block within the streaming budget,
  seam probe clean, a biome screenshot tour.

## 4. The simulation on the voxels

- The bubble ticks 12.5 cm voxels directly (P10 v6 kernel; window about
  64 x 32 x 64 m under the 1.5 M cell budget, or sparse). Remove ADR 0029's
  micro window, majority publishing, held chunks.
- Retune materials per cell size: fire spread and burn time, sand angles,
  water flow, gas spread (ADR 0029 consequences).
- **Done:** the ADR 0014 matrix (fire, water spill, blasts) in budget;
  filmstrips of sand, water, fire.

## 5. Player and gameplay on the fine grid

- Collision (`VoxelBoxMover`) at 12.5 cm, step-up 0.5 m, swimming.
- Brush digging: tool shape and size by tier, yields material amounts with
  loss rules; mining speed by hardness x volume. Inventory stacks as amounts
  (supersedes part of ADR 0005; ADR of its own).
- Placing with shapes: cube, cylinder, cone, slope, path; ghost preview;
  sculpting (smooth, flatten).
- Structural collapse: detached islands become falling voxel bodies (tree
  felling, cave roofs). ADR of its own; native island search under a budget.
- Port block entities, stations, furniture, crops, work spots (ADR 0007,
  0022, 0023, 0030) to 12.5 cm models; animals and the ape redrawn at 12.5 cm.
- **Done:** `stone_age_test=1`, `flint_age_test=1`, `hands_test=1` pass on
  `world_v2=1`; a playtest build for the user.

## 6. Saves, streaming, switch-over

- SQLite saves at the new scale (`persist=1` measured, ADR 0017); edits
  cascade to far LODs.
- Flip the default after the user has played it; delete the 0.5 m path, the
  brick majority view, the terrain atlas; mark ADR 0004, 0012, 0029, parts of
  0005/0006 superseded; update `vision.md` and `CLAUDE.md`.

## Open taste calls (ask when the step reaches them)
- Colour grade and palette mood (step 2).
- How much material a dig loses, and whether loose dirt piles keep their
  shape or slump (step 5).
- Tree felling: does the whole tree fall as one body, or break apart (step 5).
