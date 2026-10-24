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

**Answer (first pass).** Rigid voxel pieces work end to end on our grid: find, fly, crush, settle,
draw with the terrain's colours. The mechanism is sound; the forest cost was not.

## Cost pass (2026-10-24)
What changed, in the order it paid off:
1. **Strength flow grows its region lazily.** The max flow no longer builds the whole connected
   wood network (93k voxels in old growth) before its first search: a node's faces are read when a
   search first reaches it, so a dig by the roots meets the ground at once. Strength 21 -> 4 ms; an
   ordinary dig next to a big tree 17 -> 0.4 ms (it ran the flow over the whole forest before).
   Same pieces found (11 117 wood + 30 676 leaves, as before).
2. **Leaves on a dense box** (calloc'd distance bytes around the wood, plus leaf_reach) instead of
   hash maps: 41 -> 11 ms (old growth), 13 -> 6.6 ms (forest).
3. **Settling scans world rows once**, clipped to the piece's rotated box, instead of a small cube of
   world cells around every piece voxel: a leafy crown 15 -> 5.6 ms.
4. **The work runs on the sim's driver thread** (`RigidPieces.set_async`, `SimWorld::pre_tick_`):
   the main thread queues checks and steps and collects results between ticks. Single-cell reads
   (`get_voxel`, `get_flags`) no longer wait for that phase, only for the tick itself: the first
   async run stalled the main thread for the whole job through the player's and the particles'
   per-frame `get_voxel` (a read now sees a wood or leaf cell just before or just after a piece takes
   it; writers still wait).
5. **Meshes in 16^3 blocks, built on the driver**: a crown that sheds leaves rebuilds only the blocks
   it lost them from, and the main thread uploads blocks within 1.5 ms a frame (one 100k-voxel mesh
   cost 15 ms to upload, every few ticks while the crown shed).
6. **Budgets:** at most 800 world voxels broken per tick (over it, wood holds and leaves wait),
   snapped limbs re-checked together every 3 ticks, 8 break particles per tick (24 cost 4.4 ms).
7. `/fell` scans every 4th column first: the debug command itself cost a 574 ms frame (now 110 ms,
   still GDScript; a player's chop does not go through it).

| Case | Check (driver) | Job max (driver) | Main thread max | p99 / frames > 16.7 ms |
|---|---|---|---|---|
| Old-growth oak (bench, sync) | 19.7 ms (fill 2.6, strength 3.8, leaves 10.8, make 2.4) | | | |
| Forest oak (bench, sync) | 14.7 ms | | | |
| `/fell` in forest (game) | 10.5-11 ms | 15-17.6 ms | 2.4 ms (`pieces.tick`) | **7.19 ms** / 1 (the `/fell` command frame) |
| `/fell pillar` on plains (game) | 1.5 ms | 2.5 ms | 1.6 ms | 4.45 ms / 1 (the command placing 30k voxels) |

Against the criteria now: detection costs the main thread nothing (it delays one sim tick by up to
~18 ms on the driver, the sim runs at 33 ms a tick); physics and settling likewise; the forest
felling holds p99 (7.19 <= 8.3 ms). `budget_ok` still reads false in both runs, only for `max`:
the one frame over 16.7 ms is the debug command's own GDScript (scan and cut, or placing the
pillar), not the falling. The pillar filmstrip (`game/out/fell12/fall_sheet.png`) looks as before:
topples away from the cut, crown bursts on impact, the log settles.

Still open:
- ~~A forest fall has not been judged on camera.~~ Done with a free camera (`/cam`, `DebugCamera`;
  `/fell cam` picks a spot 14-30 m off with clear raycasts to 4, 10 and 16 m up the trunk; the
  player, terrain detail and sim window stay put). Seen (`game/out/fell17`): the old oak (5 868 wood +
  40 066 leaves) tips away from the cut and comes to rest after ~3 s leaning at ~50 deg on its
  neighbours' wood, breaking leaves on the way, and is written back leaning. A snapped twig with a
  bunch of leaves (15 wood + 2 273 leaves) rocked for 15 s without sleeping: only wood holds a piece
  up, so it pivoted on its few wood voxels. Snapped bits now drop as crumbs under `min_limb` wood
  voxels (not wood + leaves); every piece settles, p99 6.47 ms.
- **Push through (the developer's call, 2026-10-24).** A felled tree should not hang up on its
  neighbours. Three things were in the way, found with the per-piece contact log (`get_contact_info`,
  `rest`, `contact_log` in the stats, `track_m` in FallingPieces):
  1. Only wood under 0.75 m could break. Wood now breaks at `hardness * (run / thick)^2` (thickness
     factor, capped at a 6 m run): twice as thick needs twice the speed or load.
  2. Old-growth crowns interlock one voxel apart: a tree touched its neighbours' limbs on the first
     tick, before it moved fast enough to break anything, and stood (0-1 deg for 4 s in the bench). A
     heavy piece (`heavy_mass` 1000) now breaks any wood thinner than `limb_run` (1.5 m) it touches,
     at any speed; trunks hold.
  3. Cut off its stump, a trunk slid about on its base and rocked (3-28 deg for 6 s). Felled trunks
     now pivot on the edge of their cut that faces the fall (`hold_hinge`, a ball joint) until they
     tip 45 deg or 4 s pass, like the hinge wood a feller leaves.
  Also: a strength split tears the wood left face to face with the piece (it would jam on it).
  Result in the game, the same old oak felled three ways: it tips to ~50 deg in 2-3 s and rests on
  its own crown limbs on the ground (3 ground contacts, no wood), breaking 6 000-8 000 voxels; job
  max 18-19 ms on the driver, p99 6.0-6.6 ms. A broad oak crown props its trunk off the ground; its
  own limbs do not break under it (they are part of the piece).
- Leaves (7-11 ms) are now the biggest part of a check; reading cells through `cls_at` (bounds and
  loaded check per neighbour) is most of it.
- A frame over budget in a real chop could still come from the terrain remesh of thousands of broken
  leaves; the break budget bounds it per tick but it was not measured with a player's chop.

## Out of the flag (2026-10-24)
- `/chop [cam]` chops like a player: 1 m brush cubes (the biggest brush) over the trunk's cross-section
  1.5 m up, one every 0.3 s, through `Main._on_dug`. The old oak falls after 4 digs, the same piece as
  `/fell` (5 868 wood + 40 055 leaves), rests at 49 deg on its crown.
- A 1 m dig into wood cost **16-63 ms** on the main thread before P12 ever ran: `GameMaterials.defs()`
  rebuilt the whole material table (about 110 dictionaries) on every call, and `_on_dug` called it per
  cell through `is_plain_voxel`. It is built once now (a static cache, like `_content_pass2_defs`): a 1 m
  dig costs 1.0-1.8 ms, `/chop`'s trunk scan 30 ms (was 110 ms for `/fell`), and every other per-voxel
  `defs()` caller got the same speedup.
- Chop run in the forest: p99 6.55 ms, one frame over 16.7 ms (the `/chop` command's own GDScript
  trunk scan). Standard budget run (`autopilot=1 duration=20`) with pieces on: `budget_ok` true,
  p99 3.35 ms, max 12.35 ms.
- `Settings.rigid_pieces` is on by default; `rigid_pieces=0` turns it off.
