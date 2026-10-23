# P12 – Rigid voxel pieces: can a cut tree break off, fall and settle back into voxels within budget?

**Question.** ADR 0037 decided that unsupported groups of voxels (a felled trunk, a cut pillar, later
whole buildings) break off as one body, fall, crush what they hit and settle back into voxels, like
Lay of the Land's trees. Nothing in the game does this yet: the Noita kernel moves single cells. Can
we (1) find the piece that a dig cut loose, (2) simulate it as a rigid body against the 12.5 cm grid,
(3) write it back into the grid when it comes to rest, and (4) draw it with the terrain's own
colours, cheaply enough to keep the frame budget (ADR 0009: p99 <= 8.3 ms) in a forest?

**Pass criteria (set before measuring).**
- Detection after a dig, on the main thread: <= 3 ms for a full-size old oak, <= 0.5 ms for an
  ordinary dig in the ground (the common case must stay free).
- Physics: <= 1 ms per simulation tick (30 Hz) with up to 4 pieces in flight.
- Settling (writing the voxels back): <= 4 ms once per piece.
- The fall looks like a tree falling (topples away from the cut, does not stand balanced on the
  stump, does not sink into or bounce off the ground), judged from a filmstrip.
- The game holds `budget_ok` while trees are being felled.

**Design (the prototype's answer to each step).**
1. *Support.* Materials get a `rigid` class in `materials.gd`: `wood` (logs, bark, planks, built
   wood) carries support, `leaf` hangs on wood but carries nothing, everything else solid is ground
   (anchors). After a dig, a capped flood fill through `wood` starts at every wood face of the hole;
   a fill that touches ground, an unloaded chunk or the window edge, or grows past the cap, is
   supported. A fill that ends first is a free piece. Leaves join a piece when they are closer
   (through leaves) to its wood than to any other tree's wood, so a felled crown does not steal its
   neighbour's.
2. *Physics.* Native rigid body per piece in voxel units: mass and inertia from its voxels, contact
   points on the wood surface tested against the grid as half-voxel spheres, sequential impulses with
   friction, sub-steps. Leaves do not collide; wood moving through foreign leaves and plants crushes
   them. A cut gives the piece a small push away from the player so it topples.
3. *Settling.* Once still, every grid cell inside the piece's bounds maps back into the piece
   (inverse transform, so a rotated log has no holes) and takes its material where the grid is
   empty. The Noita kernel and the terrain paste then treat it like any edit.
4. *Drawing.* One mesh per piece (faces between piece voxels culled), drawn by
   `terrain_piece.gdshader`, which hashes each voxel's position at the moment it broke off, so the
   colours do not change when it moves.

Code: `prototypes/p5-sim-spike/src/rigid_pieces.{h,cpp}` (the technosim extension), bench
`game/tools/piece_bench.gd`, in-game hook behind `rigid_pieces=1`.

## What changed on the way (2026-10-24)
- **Trees welded together.** The first cuts broke nothing off: in old-growth forest a crown's limbs
  grew straight into the neighbouring trunks (a traced support path: up the cut trunk, along a limb,
  down the next tree). `FineGen` now leaves a one-voxel gap where a later tree's wood would touch an
  earlier tree's wood (no `GEN_VERSION` bump: only voxels inside crowns change).
- **Strength (max flow).** Supported wood above a cut must also be held strongly enough: separate
  face paths to the ground are counted as a max flow, each path holds `strength` (600) voxels, and if
  the paths carry less than the wood above the narrowest cut, that side breaks off there. So a
  trunk chopped ~80 % through falls on its hinge, and a limb grown into the next tree snaps.
- **Hardness (the developer's call).** Every material has a hardness (`hardness`, else `hp`, else
  wood 4000, leaves 20, plants 5; ground never breaks). A piece breaks what it hits when
  `impact_k * speed^2` (20 x (voxels/s)^2) reaches it: leaves and plants give way from 0.12 m/s, limbs
  (wood whose shortest run along x, y or z is under 0.75 m) snap from 1.8 m/s or under too much load,
  trunks and ground hold. (Wood at 400 snapped at 0.56 m/s: a post dropped 12 cm onto its own stump
  broke it, caught by `test_rigid_pieces.gd`.) The bench and game numbers below ran at 400. The
  piece's own leaves shred against what they hit, so a crown hitting the ground bursts. Snapped
  limbs are checked after the tick and fall as their own pieces (under 64 voxels they drop as crumbs).
- **From Grimorium** (`docs/research/2026-10-24-grimorium-rigid-bodies.md`): contact normals from the
  3x3x3 occupancy (their marching-cubes trick, without triangles) and no pieces under 8 voxels.

## Results

Bench: `game/tools/piece_bench.gd` (headless; a 48 m cube of the real generator around the thickest
standing trunk, cut 2 m up, 30 Hz until settled). Game: `/fell` and `/fell pillar` with
`rigid_pieces=1`, filmstrip 0.2 s.

| Case | Piece (wood + leaves) | Check | Step p50 / max | Settle | Written / lost |
|---|---|---|---|---|---|
| Dig in the ground (bench) | none | **0.04 ms** | | | |
| Old-growth oak (bench, 4 directions) | 11 117 + 30 676 | 68-80 ms (fill 2.7, strength 21, leaves 41, make 3) | 1.6-4.6 / 24-41 ms | 4.6-5.1 ms | 40 550 / 87-123 (first version, before snapping) |
| Forest oak (bench) | 7 957 + 103 315 | 24 ms | 0.5 / 1.3-15 ms | 0.4-15 ms | |
| Log pillar + crown, plains (game) | 4 096 + ~6 000 | 1.8 ms | max 1.5 ms (tick incl. meshes 5.5) | 1.1 ms | 1 304 leaves shed |
| `/fell` in forest (game) | 46-94 trunk columns | 33-43 ms | max 10-14 ms (tick 22-41) | 7-14 ms | 5 000-29 000 voxels broken, 5-27 limb pieces |

Against the criteria:
- Ground dig <= 0.5 ms: **pass** (0.04 ms; the ground anchor ends the fill at once).
- Tree detection <= 3 ms: **fail** (24-80 ms). Leaves (hash-map Voronoi over 30-100k voxels) and
  the strength flow are the cost; the wood fill itself is 2.7 ms.
- Physics <= 1 ms per tick: **pass for one clean piece** (pillar 1.5 ms max), **fail in a forest**
  (snapped limbs re-checked inside the step, dozens of pieces).
- Settling <= 4 ms: pillar **pass**, a big oak borderline (4.6-5 ms), a crown full of leaves fails (15 ms).
- Looks like a tree falling: **pass for the pillar** (see the filmstrip crop
  `game/out/fell4/fall_rows.png`: topples away from the cut in ~2 s, the crown bursts into leaf
  particles on impact, the log settles); in a forest the view was blocked by crowns and not judged.
- `budget_ok` while felling: plains pillar p99 ok (one frame over 16.7 ms, likely the debug command
  writing 30 000 voxels from GDScript); **forest fail** (p99 35 ms: detection on the main thread and
  thousands of broken voxels re-meshing).

**Answer.** Rigid voxel pieces work end to end on our grid: find, fly, crush, settle, draw with the
terrain's colours. The mechanism is sound; the forest cost is not yet. Before it leaves the flag:
1. Run `check` (and settling) on the sim driver thread between ticks, never on the main thread.
2. Leaves on a dense local grid instead of hash maps (expect 5-10x), and the strength flow only
   when the cut plane leaves a thin hinge (count the wood left in the cut's layer first).
3. A break budget per tick (crowns break thousands of leaves; each is a terrain edit and remesh),
   and snapped limbs re-checked in one batch every few ticks.
4. Then a forest filmstrip from a clearing, and the budget run with `budget_ok`.
