# 0170: Planets first: small spherical worlds of 0.5 m blocks, fine detail only where it lives

Status: proposed (2026-11-08). The direction is the developer's call (answers below); the open technical
choices (engine, sphere grid, block look) are decided by the spikes in `docs/plans/planet-rebuild.md`.

## Context
Performance has capped the game since ADR 0033: the sim is locked in a small bubble, fast flight breaks
streaming, complex interactions drop frames, and standing still rarely holds 100 fps. The cause is mostly
arithmetic, not the engine: 12.5 cm voxels are 512x the cells of 1 m blocks per volume and ~64x the surface
to mesh, and ADR 0073/0105 rule out LOD, so every metre of view and every metre flown costs full detail.
Small voxels, a long view without LOD and fast travel cannot all be had; games that run fast picked two
(Minecraft-likes: big blocks; Teardown: small maps; PlanetSmith, Space Engineers: LOD).

The grand idea is now a solar system of spherical planets you walk around, dig into and fly between.

## Decision (developer, 2026-11-08)
- **Planets first.** The planet and space are the identity; the Lay of the Land fine look gives way where
  they conflict. Supersedes ADR 0033 (12.5 cm everywhere) and ADR 0073 / 0105 (no LOD) once accepted.
- **Small planets, 0.3-1 km radius** (Outer Wilds scale, 20-80 min walk around). At eye height the horizon is
  sqrt(2 R h): ~29 m at R = 250 m, ~41 m at 500 m, ~58 m at 1 km, so the ground view stays cheap.
- **Seamless travel** ground -> orbit -> other bodies with no cut. Needs a far representation of each planet
  (coarse mesh / LOD), a floating origin or doubles, and radial gravity.
- **One planet (and its moon) first**; the rest of the system later. Bodies on rails when they come.
- **World block = 0.5 m** (`Settings.BLOCK`, so stations, furniture and recipe units keep their size).
- **Fine 12.5 cm detail only where it lives:** a sim bubble where blocks break into fine cells (sand,
  debris, fire) and settle back; chiseled blocks (fine voxels stored only for carved blocks); tree felling and
  cut pieces as fine rigid pieces. Liquids are fill levels with a smooth surface, gases a blended field; neither
  needs fine cells.
- **Old saves break.** No migration; the planet build does not load flat worlds.
- **Engine: lean rewrite.** A custom engine is assumed unless Godot (with our own GPU voxel renderer in the
  native extension) hits the targets on the shared small-planet benchmark. Decided by measurement.
- **Sphere grid and block look: research decides** (hex prisms vs cube-sphere vs rounded cube; patterned
  flat-colour cells vs pixel textures). Grid: ADR 0172 (equi-angular cube-sphere).

## Consequences
- The game systems (machines, networks, crafting, survival, server-owned chunks) keep their design but are
  ported onto the new world data; anything iterating 12.5 cm terrain is rewritten.
- ADR 0009 budgets stay the bar (p99 <= 8.3 ms, no frame > 16.7 ms after load, RAM <= 8 GB, VRAM <= 6 GB),
  now including fast flight and the climb to orbit.
- Becomes "accepted" when the engine and grid ADRs from the spikes land.
