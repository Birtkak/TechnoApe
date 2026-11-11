# 0001 – Planets first: small spherical worlds of 0.5 m blocks

- **Status:** accepted (the block is 1 m since ADR 0033; the rest stands)
- **Date:** 2026-11-08 (proposed), accepted 2026-11-09 with ADR 0002 and 0003

## Context
TechnoApe started (October 2026) as a Godot game on a flat world of 12.5 cm voxels with no LOD. Performance
capped it: 12.5 cm voxels are 512x the cells of 1 m blocks per volume and about 64x the surface to mesh, so
every metre of view and every metre flown cost full detail, and the simulation was locked in a small bubble.
Small voxels, a long view without LOD and fast travel cannot all be had at once.

The grand idea is a solar system of spherical planets you walk around, dig into and fly between.

## Decision (developer)
- **Planets first.** The planet and space are the identity of the game.
- **Small planets, 0.3-1 km radius** (Outer Wilds scale, 20-80 min to walk around). At eye height the horizon
  is sqrt(2 R h): about 29 m at R = 250 m, 41 m at 500 m, 58 m at 1 km, so the ground view stays cheap.
- **Seamless travel** from the ground to orbit to other bodies, with no loading cut: a far representation of
  each planet, a floating origin or double precision, and radial gravity.
- **One planet and its moon first**; the rest of the system later, bodies on rails.
- **World block = 0.5 m.** Fine 12.5 cm cells (4 per block edge) exist only where they live: where blocks
  break into debris, sand or splashes and settle back, in chiselled blocks, and in felled trees.
- **Far terrain is drawn coarser (LOD), visibly but tidily:** distant ground is honestly made of bigger blocks
  with clean seams, no cross-fade. The look is part of the style.
- **The engine and the sphere grid were decided by measured spikes:** ADR 0002 (our own engine in Rust +
  wgpu) and ADR 0003 (equi-angular cube-sphere).

## Consequences
- Nothing of the flat-world game carries over: code, saves and content start fresh (ADR 0002).
- The frame budget of ADR 0004 is the bar, including fast flight and the climb to orbit.
- The plan is `docs/plans/roadmap.md`.
