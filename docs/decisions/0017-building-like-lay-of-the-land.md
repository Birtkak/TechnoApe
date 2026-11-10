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
- ~~The plain dig keeps ADR 0008's 1 m cube~~ **Revised the same day:** bare hands dig and place single
  0.5 m blocks. The good building options belong to a **build hammer**, and better mining comes with later
  mining tools. The 1 m brush stays in the engine (`edit::brush`) for those tools.

## How it plays (first version, on the playtest to-test list)
- **Bare hands:** left click digs the aimed block, right click places the hotbar's block in front of it.
  A white outline marks the aimed block.
- **Build hammer** (H, until items exist): right-drag places the tool's shape, and Q cycles box, cylinder
  and cone.
  - A box spans the drag (a line, wall or floor).
  - A cylinder or cone is centred where the press was, with the drag as its radius. The mouse wheel sets
    its height in blocks (8 by default), and it grows up from the ground.
  - A white ghost shows where the shape would go, and a green ghost shows the shape while dragging.
  - Digging stays one block, even with the hammer.
- **Aim:** 8 m. **HUD:**
  - nine slots, held counts in survival, a gold strip in creative (C switches modes)
  - the height beside the hotbar for cylinders and cones
  - the mode and tool in the window title until the HUD has text
- `acts=` scripts the same inputs for checks without hands, e.g. `acts="1:c,2:h,2.2:q,3:R,3.1:m:-200:0,4:r"`.

## Consequences
- The server accepts any shape from anyone, like the creative switch. It checks for a hammer once items exist.
- `Shape` gains a kind next to its bounding box. The server's whole-shape checks and the client's prediction
  stay as they are, because both ask the shared rule which cells a shape covers.
- The block registry gets loose and support properties when the physics system comes. Until then, ids and
  names are what saves depend on.
- The roadmap's "after the first playable" list gains world physics and blueprints.
