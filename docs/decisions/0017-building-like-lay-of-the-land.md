# 0017 – Mining and building like Lay of the Land: shape tools, world physics, blueprints

- **Status:** accepted (amends ADR 0008)
- **Date:** 2026-11-10 (developer's calls)

## Context
The developer wants mining and building to feel like *Lay of the Land* (Southern Cross Interactive, 1.0 on
2026-04-08). From its store page and guides, it has:
- fine voxels
- procedural shape tools: walls, a cylinder for round towers and pipes, a cone for roofs and spires, each
  with a size to set
- terrain sculpting
- a fully destructible world in which sand collapses and water flows
- blueprints that capture a volume and paste it elsewhere, taking the materials from storage

## Options considered
Each of the four, and for the plain dig, a round brush with a radius or ADR 0008's 1 m cube.

## Decision (developer)
- **Shape tools: yes.** You pick a shape and set its size, a ghost shows it, and a click digs or fills it.
  The first shapes are the box (block, brush, line, wall, floor), the cylinder and the cone. Shapes go on the
  wire as shapes (ADR 0014), and the shared rule decides their cells.
- **Physics on the world: yes.** Loose blocks (sand, gravel) fall and slide, and unsupported builds
  collapse. This is a simulation system of its own, after the first playable, with the fine cells.
- **Blueprints: yes.** Capture a volume, name it and paste it elsewhere, with materials paid from storage.
  This comes after the first playable, once there is storage to pay from.
- **Terrain sculpting: no.** Digging stays block-based.
- **The plain dig keeps ADR 0008's 1 m cube** (2 x 2 x 2 blocks on the 1 m grid), with Shift for single
  blocks.

## Consequences
- `Shape` gains a kind next to its bounding box. The server's whole-shape checks and the client's prediction
  stay as they are, because both ask the shared rule which cells a shape covers.
- The block registry gets loose and support properties when the physics system comes. Until then, ids and
  names are what saves depend on.
- The roadmap's "after the first playable" list gains world physics and blueprints.
