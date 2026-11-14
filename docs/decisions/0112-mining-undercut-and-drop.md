# 0112 – Mining (c): undercut and drop

- **Status:** accepted (slice (c) of `docs/design/mining.md`, ADR 0087; extends ADR 0094)
- **Date:** 2026-11-14

## Context
mining.md 2: dig out the soft layer under a hard one and the hard layer comes down under its own weight. Granite
"stands by itself" (mining.md 3); a weak roof in loose rock and timber props are a later system. ADR 0094 made a
survival dig on a crack loosen rock into rubble by a capped search of generated ground; ADR 0079 makes loose blocks
fall; ADR 0094's `rock_hits` hurts a head a falling rock passes.

## Options considered
- **Support by a real structural search** (a roof cell's path to the ground through rock: unbounded on generated
  ground, spike 3 of mining.md) or **a span rule at the dig's layer** (how far an opening cell is from its nearest
  wall, against what the rock over it carries). The span rule was the design given; it needs only a capped flood.
- **Stored state** (a roof "cracked" flag) or **computed at the dig** from the world as it is: computed, so no wire or
  save change, as in 0094.

## Decision
- **Data:** a block row may say `span = n` (1..16 columns, bedded rock only, checked by the loader): shale 1,
  limestone 2, sandstone 2. A row without it never drops by undercut (granite, basalt, every other block).
- **The rule** (`engine::crack::undercut(world, dug, loose)`, read before the dig changes the world; the dug cells
  and the cells `crack::drop` loosened with them count as open):
  1. From each dug cell, flood its **opening** at its layer: 4 neighbours (±i, ±j, across cube-face edges by
     `fold`), cells that are not solid, breadth first in a fixed order. Over `OPEN` = 128 cells: that opening drops
     nothing (a cave or open sky beside: too big to judge cheaply).
  2. Each opening cell's `d`: the fewest 4-neighbour steps to a wall (a cell beside a wall is 1), breadth first from
     the cells beside one.
  3. The cell over an opening cell holding a rock with a `span` (and rubble) drops when `d > span`: it and the same
     rock over it up to and including the first cell with a bedding plane on its top (`crack::of & PL`), at most
     `SLAB` = 4 cells; any other block stops it.
  4. In flood order, at most `UNDERCUT_MAX` = 48 cells a dig.
- The dropped cells go the way of 0094's loosened ones: the server turns them into their rubble by `leave(old)` (the
  same ledger), they fall by ADR 0079 and hit heads by `rock_hits`. Survival digs only; creative unchanged. Placed
  rock follows the rule as generated rock does. The client's `fxedit=...:dig` mirror calls it too, so a stand run
  shows it.

## Evidence
- Tests, each seen red first (the function a stub returning nothing; the loader check switched off):
  `crack::an_undercut_drops_the_roof_past_its_span` (3 x 3 under shale: the centre column's slab to its bedding
  plane, nothing else; a 3-wide gallery 20 long under limestone: nothing; 5 x 5 limestone: the centre; 5 x 5 granite:
  nothing; 13 x 13 shale, over the cap: nothing; two runs the same list; a pillar beside the centre holds it, the same
  pillar passed as loosened does not), `blocks::rock_and_ore_dig_into_rubble_and_stack_by_16` (span parsing and the
  three rocks' spans), and the server's `digging_out_a_room_under_shale_drops_its_roof` (a survival dig of a room's
  last cell: the dug block, its crack's cells and the slab as rubble, ledgers equal; creative: no rubble, the roof
  stays; red with 1 rubble where 2 were due).
- **Cost** (`crack::undercut_cost`, the 4 km planet, play build, job slot, one run): digs in buried rock (an opening
  of the one cell, the common dig) **3.4 us mean, 196 us worst** over 161,278; digs at a column's top cell (open
  sky beside: the flood runs to its cap) **81.8 us mean, 1.97 ms worst** over 26,879. No drop came of either (natural
  ground has no closed rooms). 50 bots digging a block a second at the surface: some 4 ms of server time a second.
  A row in `docs/perf-debt.md`; not tuned (ADR 0096).
- **Still** (read, `out/mining-undercut/rooms.png`, `pair.png`; made by `out/mining-undercut/shot.py`): from 15 m
  over two roofs one block thick, each over a 3 x 3 room with walls 3 thick, dug out but for a corner; the corner is
  dug by `fxedit` at 1.5 s and 2.0 s. The shale roof shows 3 rubble cells in an L about its middle after the dig
  (`4 more loosened, 3 by undercut`: the corner's bedding plane loosened 4 wall cells, which widened the room so two
  more cells reached `d` 2); the limestone roof stays whole (`5 more loosened, 0 by undercut`). With walls one
  thick the shale room dropped nothing: the loosened wall cells opened the room onto the air beyond, the flood ran
  to its cap.
- **Soak** (`technoape-server soak=1 bots=50 minutes=2 seed=1`, play build, measure slot): **ok**, 2,400 ticks in
  45 s, hash `c8e4025fd9882a0d`, 265 heaps fell; the bots' digs are on generated ground, so the flood runs and drops
  nothing there.
- `cargo test -p engine -p server -p client`: 320 + 75 + 139 passed, none failed.

## Calls for the developer to confirm (tuning, by play)
- Which rocks drop: shale, limestone, sandstone; not granite, basalt or the rest.
- Spans 1, 2, 2; a roof drops at `d > span` (shale over any room 3 wide, limestone and sandstone over 5).
- The slab: to the first bedding plane, at most 4 cells.
- Caps: an opening of 128 cells is judged, a larger drops nothing; 48 cells drop a dig at most.
- Loosened cells count as open though their rubble stays in the cell until it falls or is dug: a good hit in a
  thin wall can join a room to the open beyond and so stop an undercut (seen in the still).

## Shortcuts
- An opening over `OPEN` cells drops nothing: a big cave's roof never comes down by a dig at its edge.
- Only the dug cell's layer is flooded: a room two high is judged by the layer the dig was in.
- A slab is a column: the cells beside a dropped column stay, held by nothing but the rule (no support search).
- Several dug cells (the hammer) flood each opening once; a capped opening drops nothing but the others still
  judge.
- Near a cube face's edge the flood's axes turn (as 0094's drop); the server alone decides, so no desync.

## Review

## Consequences
- The weak roof (mining.md 3, props) can build on `span`: a prop is a wall to the flood.
- The surface digs pay the capped flood every time; the perf-debt row names how to stop it early.
