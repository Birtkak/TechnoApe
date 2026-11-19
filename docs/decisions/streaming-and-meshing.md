# Streaming and meshing

How chunks are chosen, meshed and drawn at every distance: the LOD walk, flight, the window between worlds, the far
forest, face seams, builds seen from afar, and the mesher's rules for leaves, glass, still models and joined lamps.
What holds today. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every
number. Light and the shader side are in `rendering-and-lighting.md`.

## The developer's calls

- Fast flight must never show holes: "flying quickly now make it so everything goes invisible" (F9 2026-11-11) [0038].
- One process, one window, a loading screen: no swap of windows between title and world (F9 2026-11-11) [0073].
- Creative flight goes up to 10 km/s [0073].
- Lamps next to each other glow as one surface, the frame only on the outer edge (2026-11-13) [0085].
- Fast leaves draw their holes open: "do the shader change here", after bare winter crowns showed as black cubes
  (2026-11-13) [0088].
- Glass is see-through and tinted: "make it a big nice upgrade" [0081].
- A forest must show in the far chunks, and a face seam must not show as a bare strip (F9 2026-11-13) [0100].
- Big builds are seen from afar (bug 2026-11-14_02-01-35; roadmap item) [0109].
- The overseer's call on 0109's far-cell rules, which the developer may overrule: a dig at the middle beats placed
  blocks elsewhere in the cell; a cell needs s counting blocks; a 1-wide tower is drawn one cell wide [0109].

## The LOD walk and the cover

- A coarse chunk covers its children while they load. A child that splits counts as done only when its own children
  are, recursively, and a stand-in draws as deep as the finer chunks go (`Streamer::complete`, `kids_below`,
  `emit_below`). The recursion stops at the first chunk with a mesh [0038 B].
- A child wholly out of view (`Streamer::hidden`) no longer holds its parent as cover in a drawing walk. A frame that
  only culls marks the streamer dirty, so frames walk again until the hidden child's mesh is in [0073].
- `Streamer::too_coarse` counts drawn, visible chunks the split rule would split. A run's JSON has `coarse_slow_s`,
  the longest time it stays non-zero while the camera moves under 20 m/s; the gate fails a benchmark run over 1 s
  [0073].
- Tests: `a_fast_flight_never_leaves_a_hole_where_a_coarse_chunk_was_drawn` (flies 6 s at 1000 m/s, 200 m up, meshing
  slower than the flight; 96 holes before, none after) [0038 B]; the cover case and a scripted fast flight that
  settles at its level [0073].
- Accepted: ray-traced shadows lose the parent as a caster where a hidden child is not meshed yet; the shadow pops in
  when the child lands [0073].

## Flight

- `phys::FLY_MAX` is 10 000 m/s; the wheel reaches it (its steps are a factor per notch). No server check limits
  speed [0073].
- Over a body's top layer (radius + `AMP` + `BUILD_HEADROOM`, plus the step's length, with 1 m margin) a flying step
  is one move; near the ground the 0.25 m substeps stay. Client and server differ only by rounding at the switch
  [0073].
- The `fly:<m/s>` act scripts flights [0073].

## One window, loading screen

- `main` makes one event loop for the program; the title and each world are sessions run on it
  (`run_app_on_demand`). The next session takes the window (`App::carried`), sets fullscreen, frame and size from its
  settings and makes its own GPU state after the old one is dropped. Each world still gets a fresh server, world,
  streamer and GPU state [0073].
- Until a world is in, play draws a loading screen: the chunks still missing against the most there were ("Loading
  the world: N chunks to go", dark, a green bar). The bake's and the map download's bars stay on the title [0073].
- Statics (sizes, the audio folder, the bake caches, the contrast flag, `sound_dir=` as a `OnceLock`) carry from one
  world to the next. Off-screen runs print one JSON line per session [0073].
- (also player-and-feel) The outline's Accessibility row **Outline colours: moving | still** (`outline_colours`, bit 16
  of `opt.x`) holds the hues where they are; "still" keeps the rainbow, frozen [0073].

## Fixed-view determinism (also process-gate-and-performance)

- A shot holds the sky's clock at `time=` (`bench::sky_time`); `Gi::drop_history` (called at load) restarts the
  bounce frame counter. Every fixed view runs under `weather=clear`, and a shot shows the newest weather map whole
  [0073].
- A shot fires at exactly 400 frames after loading and waits up to 180 s for the whole world (a run 60 s);
  `views.sh` kills one at 240 s [0073].
- Left: water's ripples run on the process's clock (`sun.w`), so a view of a flowing river would vary [0073].

## Far chunks: forest, seams

- (also grid-and-planets) **Plants over face seams.** `Body::candidate` takes any foot inside its face. A column
  within `REACH` (6) columns of an edge also reads the neighbouring faces' spots, located by `unfold` (the inverse of
  `fold`). `plants_with(face, i, j, cand)` reads faces in index order, so both sides of a seam pick the same plant.
  Round a cube corner `unfold` does not reach, so a crown is cut at the corner's diagonal (eight corners). Clutter
  (`clutter::ground_under`) lies up to the edge; the river check reads across a seam through `fold` [0100].
- **Far forest** (`world.rs`, `build_tile` for level > 0): in a coarse tile the canopy is the ground. The trees
  (plants with a trunk) at the coarse column's middle (level 1) or its four quarter points (level 2 and up) are read
  with `plants_with` and the same spot cache as level 0; the highest top, if over the coarse ground, becomes the
  column's height and its block the column's three materials (rock none). A column whose middle is under water keeps
  its ground. The work is on the workers [0100].
- The canopy is a ground, not leaves: from far off a forest edge is a 10-20 m step, its sides leaves to the ground
  [0100].
- (also plants-and-creatures) `herds::stand_clear` is `stand` whose whole box is clear (`phys::blocked`); herd
  generation and drift use it and try their next spot otherwise. `stand` is unchanged (`move_on`'s climb-out and
  `/spawn <x> live` use it) [0100].
- The bake and `saves/bakes/` are unchanged by this; saves' edits win over trees along seams [0100].

## Builds seen from afar

A coarse cell (edge s blocks, s = 2^level) shows, from the edits inside it [0109]:
- the edit at its middle, a placed block or a dig (a model block there counts as no edit);
- else, once the cell holds at least s counting blocks, the most common of them, a tie to the lower id;
- else the generated cell.

Only opaque blocks that are not models count (`far_counts`): glass and cutouts never colour a far cell, and
`hide_models` never turns a far ground cell to air. A removed tower leaves air edits only, which do not count, so it
disappears. A 1-wide tower is drawn as wide as one cell of its level (2 m at level 1, 16 m at level 4); its top cell
may hold fewer than s and is left out [0109].

The structure [0109]:
- `World::far` holds, per chunk of every level, the cells' sums and an `Arc<FxMap<cell, block>>` of what each shows
  (level 0: the edits themselves). `set_block` and `put_edit` update one cell a level (about 12 levels on the 4 km
  planet); the picks map is copied on write only when a mesh job still holds it.
- It is the client's: `World::far` is an `Option` the client opens (`open_far`, which also builds it from a loaded
  world's edits). The server keeps none; without it `edits_for` reads level 0 from the edits and coarse levels see no
  edits. The state hash is the same with or without it.
- `Streamer::dispatch` takes `far_snap` (the picks of the chunk, its 26 neighbours and, at a seam, the neighbouring
  face's chunks); the worker gathers them (`FarSnap::edits`), hides models and holds gliding cells. The seam strips
  read the neighbouring face's picks.
- `stream::sampling` names the coarse cell that holds the block at the levels asked; a cell off its chunk's sides
  names only its own chunk. `far_edit` keeps per edited block the levels whose pick its last edit changed;
  `touch_block` asks `sampling` for those and level 0 only (`World::far_levels`; all levels when it does not know,
  as after a take-back). A slot loaded but not drawn is not meshed for an edit: only a walk or cull that draws it
  requests the job.
- The result does not depend on the order edits come in or on loading (`open_far` after loading gives the same
  summary).
- Far lamps: a far cell whose block is a lamp is drawn with the lamp's material, and the emissive path (`looks[m].info`
  bit 24, `terrain_lit`) shows its glow. Its light on the ground is not drawn: lamp light reads only the lamps near
  the camera (`LAMP_R` 24 m).
- Cost: about 1 us an edited cell in `set_block` on the client; a 236k-edit load 0.2 s more; memory for the summary
  some tens of MB for 236k edits. One edit's dispatch is about 25 us. Server pays nothing. Not changed: save format,
  wire, old saves.
- Done for builds on a body's grid; builds in free space are not [0109].

## The mesher's rules

- **Cut modes** (`mesh::Cut`) [0088, 0069]: `Solid` (coarse levels: the old mesher; a cutout block is one colour),
  `Shell` (fast leaves on level 0), `Full` (fancy leaves). Culling reads one bit per block id (`mesh::CUT`).
  - `Shell` culls a cutout block's face against a cutout block (a crown has no inner faces) and keeps an opaque
    block's face behind one (the ground under a shrub, a trunk in its crown).
  - `Full` culls a cutout face only against an opaque neighbour.
  - Cutout quads go last among the land quads (`Mesh::cut`) in both. A chunk with no cutout block is the old mesher
    bit for bit. Tests hold each mode to a face-by-face reference (`faces_are_culled_only_against_opaque_neighbours`).
  - The ray split travels with the mesh (`Mesh::ray_cut`, `DrawChunk::ray_cut`), so a chunk meshed before a change of
    the Leaves setting agrees with its BLAS [0088].
  - `meshbench` measures `Shell`; its numbers do not compare with older ones [0088].
- **Glass** [0081]: a glass face is culled against an opaque block or the same glass block (a wall has no inner
  faces); never against other glass kinds. Glass quads go last among the land quads, after the cutout ones
  (`Mesh::glass`), with either Leaves setting. A chunk with no block seen through runs the old loops plus one bit
  test per solid cell (`SEE`); one with glass or fancy cutout blocks makes a second pass for the opaque and glass
  columns. Far levels draw glass in its one colour at alpha 0.45.
- **Still models in the chunk** [0068, 0081]: what does not move is put in the chunk's mesh. A still model's quad is 8
  bytes, as a terrain quad: each box face is one quad of dir 7 (no block face has it) with the cell, a turn in
  sixteenths, a place in the cell, a flag bit, and an index into the **face table** (origin, two edges, the face's
  painting rectangle: 48 bytes per model face, held once). Nothing else is stored per placed model. Still quads lie
  after the glass and before the water (`Mesh::still`). They are in no BLAS, so they cast no shadow and hold no
  light. The face table and paintings are bindings 16 and 17, read once at the start.
- **Joined lamps** [0085]: `joins = true` on a block row (lamp, blue lamp; `blocks::Def::joins`). The quad's high word
  is `id_bits(id) | ao8 << 8 | join4 << 24` (the id at 0-7 and 16-19, the sides at 24-27 since "Block ids past 255",
  multiplayer-saves-and-data.md); `join_sides` sets bits 0..3 (u-, u+, v-, v+) where the in-plane neighbour
  is the same block and the cell in front of it does not hide its face by the mesher's own culling rule (air, glass,
  fancy cutout blocks). A joining block's faces never merge (also on coarse levels). Quad bits 20..31 stay free for
  other uses (0090 took 20..25 for cutout quads' wood bits).
- **Ambient occlusion** is per corner in the mesher: two sides and the diagonal, 2 bits a corner in the quad's spare
  high bits; greedy merging joins only open faces [0022].
- Clutter default: 1 per 4 columns of its ground, on level-0 chunks (the range follows Detail); every column as the
  "many" setting. Clutter is not in the BLAS; a block row's lone model or machine may be [0068]. The scan should read
  the mesher's occupancy bits instead of every cell (open) [0068].
- Block-light and mesh CPU: the mesher costs nothing extra without glass (`mesh_cut` looks at kinds only where glass
  can be); a joining test costs about 1.4 us a chunk (2 %) [0081, 0085].

## Open

- Whether to skip `leaf_hole` when no cutout pipeline draws (fast is not quite free on the GPU: +0.32 ms p50
  standing) [0069].
- A 1-wide far tower stays as wide as a cell; a far lamp lights nothing on the ground [0109].

## No longer holds

- 0038 A's window hand-over between processes (`sys::wait_shown`, `shown=<file>`, the old window waiting 5 s then
  30 s): replaced by 0073's one process and one window.
- 0068's "fancy leaves are the default": replaced by 0069 (fast is the default; fancy is the Leaves choice).
- 0068's "coarse levels draw cutout blocks opaque in their mean colour" as the fast leaves' look on level 0: fast
  leaves now draw level 0's `cut` quads through `fs_cut` with open holes [0088]; coarse levels still draw one colour.
- 0069's "fast = the old mesher, opaque, one BLAS geometry" for level 0: now `Cut::Shell` through the cutout pipeline;
  the BLAS keeps one opaque geometry for fast [0088].
- 0069's spike removal of clutter models is moot: still models (0081) are built from 0068's format.
- 0109's first round (choose per cell at mesh time from the edits in `edits_for`): replaced by the kept per-level
  summary.
