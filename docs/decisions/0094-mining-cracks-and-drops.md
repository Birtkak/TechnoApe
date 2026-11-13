# 0094 – Mining (b): cracks in the rock and the drop along them

- **Status:** accepted (slice (b) of `docs/design/mining.md`, ADR 0087); slice (c), undercut, follows here
- **Date:** 2026-11-13

## Context
ADR 0087: layered rock shows bedding lines, massive rock a few joints, both a rule over (rock, place) the same on
server and clients; a dig on a crack loosens its neighbours along it (a good hit 3 to 6 blocks, a bad one 1); a
plain dig always works; a drop hurts who is under it. ADR 0093 made a dug rock its rubble.

## Options considered
- **Where cracks live:** stored per block (a save and wire change, and a world of state for what is a rule), or
  **computed** from the rock, the column's folded top (`Rock::top`) and the cell, like the rock itself.
- **What a good hit is:** a hit placed well on a face (needs the aim on the wire), a chance per dig (luck, not
  reading the rock), or **any dig of a block that lies on a crack** (read the face, dig on the line).
- **The look:** a painted texture (the textures session's; a picture per rock cannot follow planes that cross
  blocks anywhere), or **a line drawn by the shader** on the face edges the mesher marks.
- **Hurt:** burial only (ADR 0079), or burial and **a hit** when rock falls through a head.

## Decision
- **The rule** (`engine::crack`): a block row says `bedding = m` (layered rock: limestone 2.5, sandstone 3,
  shale 1.5) or `joints = n` (massive rock: granite 4, basalt 3). Bedding planes lie every `m` metres under the
  column's folded top, so they wave with the layers; a plane runs along the top face of the cell it crosses.
  Joints are vertical planes between columns: one in every run of `n` columns, at a hashed place in it, so joints
  stand 1 to 2n - 1 columns apart and any face 2n blocks wide shows one (granite: any 8, basalt: any 6; the overseer:
  a granite tunnel must always have a joint to read). A joint holds for 16 columns along and 8 m up, then the next
  stretch places its own (a few joints that end, not a grid across the face: the review), and every 3 m up it steps a
  column or not, so it climbs crooked. Placed rock
  cracks by the same rule as generated rock. The faces of a cell on a crack are 6 bits (`crack::of`).
- **The drop:** a survival dig of a block on a crack (a good hit: a bedding plane through the cell, or a joint on its
  +i or +j side, `crack::HIT`; one side of each plane, so about one dig in `bedding` metres, or two in `joints`) turns 2 to 5 more of the same rock into rubble
  (3 to 6 in all, by a hash of the cell): the cells on the same plane, on the same side, found breadth first in a
  fixed order, at most `SEARCH` = 32 cells looked at. They fall where open (ADR 0079) and are dug up as rubble
  (ADR 0093). Off every crack a dig takes the one block (a bad hit). Creative is unchanged.
- **The hit:** a rubble block that falls into a survival player's head cell takes `ROCK_HIT` = 4 points
  ("<name> was hit by falling granite rubble"), once (a block passes a head once); burial then holds as for sand.
  Sand hits nothing.
- **The look:** a joint: the mesher marks each face of jointed rock with the joint edges among its four sides (bits
  20-23 of the quad, as lamps' joins use 16-19) and the shader darkens that edge texel to 0.3; two faces meet at a
  joint, so it reads as a dark line two texels wide. A bedding plane: each open side face carries the plane's line
  across it (bits 24-31, `crack::bed`: its texel row at the face's low end and the rows it climbs to the high end),
  from the folded top at the face's two vertical edges, so the faces beside it along a wall share their edges' rows
  and the line runs on through them and tilts with the fold (the overseer: a brick pattern of square-ended level
  lines read wrong). A marked face never merges. Level 0 chunks only; the arrays are made only for a chunk with
  cracking rock beside an open cell, and each column's and corner's top is looked up once a chunk.
- No wire, save or protocol change: clients compute the same rule from the map they derive already.

## Evidence
- Tests: `crack::bedding_runs_under_the_folded_top_and_joints_between_columns` (any 2n columns hold a joint: seen
  red on the old hash, an 8-column stretch with none), `crack::a_bedding_line_runs_on_across_faces_and_follows_the_
  fold` (seen red), `stream::exposed_rock_gets_its_joints_and_bedding_lines`,
  `crack::a_dig_on_a_crack_drops_its_neighbours_along_it` (new code), `mesh::a_crack_marks_the_edges_of_the_faces_
  beside_it_and_stops_their_merge` (seen red with the merge rule taken out), and the server's
  `a_dig_on_a_crack_drops_rubble_along_it` and `falling_rock_hits_the_head_it_falls_through`, both seen red.
- **Spike 3, the drop's search on generated rock** (`crack::drop_cost`, the 4 km planet, play build, measure
  slot): 3,000 digs on cracks, 3.49 more blocks loosened on average, **4.7 us a dig, 57.7 us at worst**; a crack
  lookup 0.66 us. 50 bots digging a block a second: some 0.25 ms of server time a second.
- Stills (read, `out/mining-cracks/`): `limestone-wall.png` (seams run on through the blocks and step with the
  fold), `granite-wall.png` (two joints crossing a wall 15 blocks wide top to bottom), `dig-pair.png` (the face; a
  dig on a bedding plane, 5 more loosened, a band of 6 rubble blocks where the line was; a plain dig a row lower, the
  one block). The digs are a stand run's `fxedit=t:di:dj:dl:dig`, the server's rule on the client's world (the
  stand's placed blocks are the client's alone); it prints what each dig loosened. In a wall the rubble is held by
  the rock under it and stays in place; it falls where the ground is open (ADR 0079).
- **Soak** (`technoape-server soak=1 bots=50 minutes=2 seed=1`, play build, measure slot): **ok**, 2,400 ticks in
  58 s, the bots' worlds, replay, save and ledgers equal (their digs are on generated ground, cracks included).
- **The mesher's cost** (`run=fly quick=1`, play build, three runs in the measure slot): mesh jobs 359 us on average
  against 332 on main (+8 %: the crack arrays and the corner tops of exposed bedded rock); GPU p50 2.86 ms, frame
  p99 6.2 ms, unchanged. A row in `docs/perf-debt.md`.

## Review (a fresh reviewer read the diff)
- *Almost every dig was a good hit* (a cell counted a plane on its top or its foot, a joint on any side: shale every
  dig, basalt 68 %): a good hit is now a plane through the cell, or a joint on its +i or +j side (`crack::HIT`).
- *A mutex lock per cell* (`col_rock` on every exposed cracking cell of a chunk): each column's and corner's top is
  looked up once a chunk.
- *`rock_hits` looked up every client's head per falling block:* once a tick.
- *Joints a grid across a whole face:* a joint holds for a stretch (16 columns, 8 m up) and jogs every 3 m.
- *A test compared a value with itself:* fixed.
- *Answered, not changed:* near a cube face's edge the drop's axes can turn, so it may take cells off the plane, and a
  joint at the edge draws on one side (the server alone decides the drop: no desync); a column's top from a cached
  tile and from `rock_at` can differ in the last bits, so very rarely a drop and the drawn line disagree by a cell;
  rubble sliding sideways into a head hurts as one falling does.

## Calls for the developer to confirm (tuning, by play)
- Which rocks crack and how: bedding in limestone, sandstone, shale; joints in granite and basalt; spacings above.
  Good hits: about 2 digs in 3 in shale, 2 in 5 in limestone, 1 in 3 in sandstone, 4 in 9 in granite, 5 in 9 in
  basalt.
  Slate, gneiss, schist (foliated), the other massive rocks and the halo's altered rock do not crack yet.
- A good hit is any dig on a crack; drop sizes 3 to 6; `ROCK_HIT` 4 points of 20.
- The look: a dark line, 0.3 of the texel, on both faces of the crack (for the textures session to replace).

## Shortcuts
- The joints' hash mixes the body's size and place, not the world's seed (`Body` keeps none): every world of a
  planet size has the same joints.
- A crack beside glass or a cutout block draws no line; a far chunk (level > 0) draws none.
- A bedding line is drawn where the plane crosses the face at its edges, a drop decides by the plane at the column's
  middle: on a tilted layer the two can differ by a cell. A fold steeper than 1/4 m a metre is drawn at that slope,
  its line breaking at the face's edge. A joint's jog has no line along the step. Lines on top faces: joints only.

## Consequences
- Slice (c), undercut and drop, uses the same capped search over generated rock.
- Hauling gets busy: a good hit fills a pack's slot in two or three digs.
