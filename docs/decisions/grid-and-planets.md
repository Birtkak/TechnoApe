# Grid and planets

The cube-sphere grid, the conformal warp, 1 m blocks, the planets' sizes and what pulls on a player: gravity and the
player's own orientation. What holds today. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came
from; INDEX.md lists every number.

## The developer's calls

- **The seams are part of the world** ("the planet's grain"): shown in build mode and on the map, networks turn at
  them, a machine footprint never covers a corner cell. [0003]
- **Blocks are 1 m** (2026-11-11: "make the blocks 1 m", "retrofit everything"): a player is one block wide and two
  tall; the 12.5 cm fine cells come after, for gases, chiselling and intricate building. [0033]
- **A ledge one block high needs a jump** ("require jump"); the step-up is removed. "Auto jump" is an off-by-default
  setting ("nice for you to use in tests, or players who want"). [0033]
- **The standard planet is 4 km (Terra, 6144 columns), the huge one 8 km (12288).** About 55 % ocean, 3 to 5
  continents plus islands, hills and some mountains, oceans about 25 m deep with shelves (2026-11-10). [0021]
- **A body is simulated once, when the world is made**, and the result belongs to the save; each seed makes its own
  Terra and Luna, one default seed is curated by hand. Live systems made room for: tides, seasons, plants; active
  geology (eruptions, quakes) was not chosen. [0026]
- **Luna is a fixed part of Terra**: 3.7 times smaller, 30 Terra diameters away, so a total eclipse is perfect
  (2026-11-11). [0041, amending 0027]
- **The ape learns by doing and seeing**: nothing is locked, a journal fills in from what happens. **Terra has
  animals and hostile creatures** (the developer wants danger and a story in the world). [0027] (also
  plants-and-creatures.md)
- **Gravity belongs to the bodies and the orientation to the player**: gravity rights the player gradually, as hard
  as it pulls; the summed pull of all bodies decides down, not the nearest body; in space full rotation (no pitch
  limit, roll on Q and E); creative flight near a body stays upright like walking. [0024]
- **Terra pulls twice the Earth's** (2026-11-15: "make gravity on terra heavier", then 2x of 1.5x, 2x, 3x); the jump
  stays 1.25 blocks. [0132]
- **The grid is the conformal cube-sphere** (2026-11-15, after walking on the trial: "Make it the grid"): square
  block tops everywhere matter more than blocks shrinking near the 8 corners. New worlds only; old saves keep the
  grid they were made on. [0131] The default from generation 13 (2026-11-24, the developer: make it the default):
  every new world is on it. [draft]
- **Corners are ordinary ground**: nothing is placed on the 8 cube corners. [0136 supersedes the corners lines of
  0003 and 0027] (also vision-and-scope.md)

## The grid

- **Equi-angular cube-sphere** with quad-prism blocks, its face coordinates warped first from generation 13 (the
  conformal warp, below): 6 faces of N x N columns. N = 32 * m * 2^L with a small m (the
  chunks are 32 columns wide and the LOD tree halves the resolution per level, so only such an N lets the tree tile a
  face). N sets the radius, R = N * BLOCK / sqrt(2 pi / 3): N = 6144 gives R = 4245 m, 12288 gives 8488 m. [0003, 0033]
- **Radial layers are geometric** (each BLOCK/R thicker than the one below), so blocks stay cubic at every depth.
  Blocks are always level: gravity is radial and every column runs along the radius. [0003] Where layer 0 lies is
  the floor of the world: 192 m down on a world without the mantle, R/sqrt 2 on a new one (see deep-world.md).
- **Fine cells** are 12.5 cm: the same grid at 8N with 8 sub-layers per block, 512 to a block, so they nest exactly.
  [0003, 0033] The look is 16 texels to a block, a texel 6.25 cm, a fine cell two texels wide (a default, not the
  developer's word: 32 texels would keep the old texel size). [0033]
- **World data:** chunk id = (face, ci, cj, layer); each chunk a regular (i, j, layer) array. Neighbours fold over a
  seam only at chunk borders on a seam; the 3-block corners are one special case. Meshing, palettes and the cellular
  automaton work per face unchanged. The vertex shader maps each vertex's global (face, i, j, layer) to the sphere,
  relative to the camera, so quads and chunks share their corners exactly. [0003]
- **Block size varies** across a face and with height; recipes and units count blocks, never volume in m3. [0003]
  On the equi-angular grid a block top is a 1.09 x 0.77 m rectangle at a face edge's middle and a 60 degree diamond
  near a cube corner. [0131]
- **Player movement and collision** run in the local tangent frame; blocks are not axis-aligned in world space.
  Long straight builds turn at seams; build tools show the seam before the player commits. [0003]
- **Seam and corner rules:** the seams are shown (a faint line in build mode, like chunk borders, and on the map);
  conveyors, pipes and cables keep working across a seam and turn there by the seam's angle; a W x W footprint stamps
  as a regular patch everywhere except it may not cover a corner cell; spawn sits far from every seam. [0003, 0136]
- **Seam-fold cost** is paid only by seam chunks (about 0.5 % of cells at R = 300 m). [0003]

## The conformal warp

The conformal grid is the equi-angular grid with its face coordinates warped first. [0131]

    a = (i - n/2) / (n/2),  b = (j - n/2) / (n/2)          // -1..1 across the face
    (a', b') = (a, b) + w * table(a, b)                     // w: 0 today's grid, 1 conformal
    dir = normalize(N + S tan(a' pi/4) + T tan(b' pi/4))    // the equi-angular formula on (a', b')

- `table` is a 193 x 193 lattice of (da, db) f32 pairs, `crates/engine/data/warp_table.bin` (made by
  `prototypes/square-blocks/warp_table.py`), bilinear between its points, the same for every face and planet size.
  It is symmetric under the square's 8 symmetries with da = 0 on a = +-1, so two faces lay a shared edge on the same
  points. CPU and GPU evaluate it on |a|, |b| and put the signs back (`warp.rs::at`, the shader's `warp_at`).
- `engine::warp`: `at`, `warp`, `unwarp` (Newton from one fixed-point step, to 1e-14 of a face). The shader takes
  one fixed-point step and 3 Newton steps (two leave 1.4 columns near a corner; three under 0.005 of a column). The
  table is an rg32float texture at group 0 binding 18; the strength is the body uniform's `rot0.w`.
- `Body::warp: f64` (not a process global, tests run in parallel), set by `World::of_sources` before the body gets
  its map: the generation's (`world::grid_warp`), or a test's `warp` on a world of an earlier one. `Body::dir` warps first, `Body::locate` unwarps after, both
  skipping the table at 0 (0 is the old grid to the bit). `Body::warp_col` / `unwarp_col` convert between block
  columns and the equi-angular grid's columns.
- **The land stays where it is:** every map lookup reads the map at `warp_col` of the column (it read by face column
  before, which would slide the land by up to 0.13 of a face); map cells turned into block columns go through
  `unwarp_col`. What moves with the block lattice: plant spots, biome patches, clutter, caves and ore bodies.
- **Prices:** blocks under 0.75 m on 2.3 % of the ground (within about 450 m of a corner), under 0.5 m on 0.1 %
  (within about 90 m); the last 32 columns before a corner are 0.26-0.32 m with up to 18 degrees of skew. Block tops
  stretched over 1.1 to 1 on 72.7 % of the ground on the old grid and 0.0 % conformal.
- **Left unwarped on purpose:** the bake's own grid (`bake/grid.rs`, `bake/rivers.rs`), the clouds (`clouds.wgsl`
  `cl_locate`), the weather and air grid (`weather.rs`, `bake/air.rs`): each has its own equi-angular map by
  direction. The globe map reads the bake grid.

### The generation carries the grid [draft, 2026-11-24]
Generation 13 (`world::CONFORMAL`, the newest) is generation 12 with every body on the conformal grid (warp 1);
every earlier generation keeps the equi-angular grid (warp 0) to the bit, so old saves, their recordings and hashes
stay as they were. The warp follows from the generation wherever a world is built from it (`World::of_sources`:
a new world, a save's load, a client's or bot's welcome, the soak's bots, a replay, `report=1`), because a save
already stores its generation and the welcome already says it: no save field, no wire message, no argument. The
trial's `warp=` (client, server, bot) and its `save=0` asserts are gone; `gen=12` makes a new world on the old grid
to compare. The developer's call (the default, 2026-11-24); keeping the old grid for old saves is the routine part.
Alternatives rejected: a warp field in the save and the welcome (a second number saying what the generation says).
Replaces: 0131's trial limits (`warp=` by hand, `save=0`) and its open item (making it the default).

## Planets and their sizes

- **Standard planet** `planet=6144` (R = 4245 m; the client, the server and the benchmarks' `small` series), `huge`
  12288 (R = 8488 m), `base` 768, the comparable small planet. The columns per face edge halved with 1 m blocks;
  the planets keep their size in metres. [0021, 0033]
- **Luna** is `size = 3.7` in `data/bodies.toml`: the columns come from Terra's, n / 3.7 up to the next 32
  (`bodies::columns`): 1664 under 6144. [0041, amending 0027] `moon=` overrides it; a save keeps the `moon_n` it was
  made with.
- **Sea level** is 4 m under the radius (ADR 0020), snapped to 1/8 block under a layer top, so a block at the
  waterline stands 6 cm out of the water instead of flickering through it. [0021, 0067]
- **Ocean share** is 55 % (the bake's `ocean_share`, 0.55). 55 % of the planet is sea that costs nothing until
  someone touches it (ADR 0020). [0021]
- **Walking round the standard planet takes about 103 min**, so boats, flight and rails are needed early; a body's
  grid ends at its build headroom, so nothing can be built in high orbit. [0021] (roadmap items)
- **The far sea:** a coarse sea cell that is solid draws its water one layer over the cell's top; merged sea quads
  stay within about 90 m (a flat quad 2 km wide sags 120 m under the sphere). [0021] (also streaming-and-meshing.md;
  the level-0 rule is amended by 0067: the sea's quads are meshed for the highest tide, see bodies-and-orbits.md)
- **A world keeps the bake it was made with**; generator changes only affect new worlds. [0026]

## Bodies are simulated once (the direction)

- At world creation the server runs plates, uplift, erosion, rivers, climate and geology over a coarse map of the
  whole body, saves it and sends it to clients; blocks take their detail from the map. The bake runs on the
  cube-sphere's coarse levels, only the server bakes (deterministic on one machine, not bit-equal across machines),
  rivers are lines beside the grid, the climate stage runs the air model of ADR 0010. [0026]
- A block is a function of column and layer (rock layers, caves, ore) read through one shared function by the mesher,
  the physics and the water. Real rocks and minerals: granite, basalt, limestone; ores as minerals in the rock that
  forms them. [0026] (also bake-and-landforms.md, rock-minerals-and-ore.md)

## Gravity

- **G is 19.62 m/s2** (`interior::G`, `phys::G`): the felt pull, the Earth's twice. [0132] Every body's pull follows
  from its mass (see deep-world.md "A body's interior"): the first body (Terra) pulls G at its radius whatever its
  size, another body k M / R2. Luna, of Terra's makeup and 3.7 times smaller, pulls 5.3 m/s2, 3.7 times less. [0041,
  0162, amending 0037]
- **Jump:** one push everywhere, `phys::JUMP` = 6.84 m/s: 1.25 m on Terra at any planet size, about 4.4 to 4.7 m on
  Luna. [0037, 0132]
- **Fall damage** stays a point a metre fallen (written in metres of a Terra fall): the free landing is 10.8 m/s and
  30 m/s kills from full. **Falling blocks** use `interior::G` (`fall::due`: 6 m in 16 ticks). [0132]
- **Left on the Earth's 9.81 on purpose:** the particles (`fx.rs`, every row of `data/effects.toml` tuned by eye
  against it) and the liquid simulation's flow cap (`liquid.rs`). [0132]
- **Gravity is the sum of every body's pull**, each towards where it is now, in whichever frame the position is
  written (`World::gravity`), so the hand-over between frames changes nothing. Up is against the sum. [0024]
- **Near a moon only the tide is left:** inside a moon's frame the pull of the planet at the moon's centre is
  subtracted, fading out from half to 0.97 of the moon's sphere of influence, so gravity is the same either side of
  the hand-over; a body that circles the sun falls round nothing that pulls. [0037, 0163]
- **Depth:** the pull inside a body follows its interior (rises through the mantle to about 1.135 g at R/sqrt 2,
  falls evenly to zero at the centre); above the surface the square law fades it. [0037 replaced by 0154, 0162]

## The player's own orientation

- `Player::up` beside `Player::fwd`: each step the body's up turns towards the pull's up at `RIGHT` x (pull / G)2
  rad/s, `RIGHT` = 3 (`phys.rs`). While standing, or within `NEAR` (1500 m) of a surface when flying or when the pull is under `FREE_G`, it is held
  at `HOLD` = 8 rad/s: exact while walking, a smooth turn of a few tenths of a second when coming in tilted. Nothing
  snaps. (Falling was left out until bug 2026-11-21_18-02-54: on a moon under `FREE_G` a block's drop down a shaft
  went free, folded the look down into the body, and the landing swung it level.) [0024]
- **Free rotation** (`phys::free`): out of a hold the mouse's pitch turns the body itself (`Input::tilt`) and Q/E roll
  it (`Input::roll`, 1.5 rad/s), where the pull is under `FREE_G` = G / 9.81 (2 m/s2, not an absolute 1) or gravity
  has not yet righted the body to within 0.1 rad. Otherwise the mouse's pitch is the head's, clamped to +-89 degrees
  over the body's horizon. Going free folds the head's pitch into the body, so the view does not move. [0024, 0037,
  0132]
- **Movement:** flight goes along the body's own axes; walking along the ground's, with the look laid level on it.
  A step that changes frames carries the drawn path and any correction left into the new frame's coordinates. [0024]
- **Saves keep only the look**; a loaded player stands upright on the first step. `Player` and `Input` gained fields
  with serde defaults. Physics depends on the moon's place, so steps are taken at their own time (`Input::t`). [0024]
- Keys of the actions live in `docs/playing.md`. Client and server step the same `phys::step`. [0024, 0037]
- **Open:** how gravity shows in the HUD (which body pulls, how hard); whether walking should keep the feet planted
  while the body still turns upright after a landing. [0024]

## Formats on disk and on the wire

- Save version 4 refused older worlds when the block became 1 m (their cells mean other places). [0033] Later
  versions are in multiplayer-saves-and-data.md; those that change the grid: version 20 added `mantle: bool` (a 19 or
  older reads as false, 0154), version 22 replaced `planet_n, moon_n, map, luna` by `bodies: Vec<(i32, Option<u64>)>`
  (0163). See deep-world.md and bodies-and-orbits.md.
- The warp is in no save and no message: the generation, which both carry, says it. [0131, draft]
- Recordings (postcard, positional) from before 0024 no longer load. [0024]

## No longer holds

- 0003's N = 3072 (1 km planet), 1536 (benchmark) and 512 (moon), the 0.5 m mean block, 12.5 cm cells at 4N with 4
  sub-layers, "layer 0 starts at the core 64 m under the nominal surface" and the 0.39-0.55 m block range: replaced
  by 0033's 1 m block and 8N fine cells, and by the floor of deep-world.md.
- 0003's "greedy merging is off": replaced by merging inside a lattice with interpolated corners (0025 I2; see
  streaming-and-meshing.md).
- 0003's corner landmarks (a peak, crater or deep lake on each corner, "what the corner landmarks are" open): dropped
  by 0136. 0027's "the 8 corners hold something made": superseded by 0136.
- 0003's "not taken" conformal cube-sphere: taken by 0131 (the price it named, small blocks near the 8 corners, is
  the one measured above).
- 0021's continent recipe (`Land`, warped low-frequency noise cut at the 55 % quantile, plains, hills, ridged
  mountains, shelf and islands, AMP = 80 m, the spawn as the lattice point nearest the +y pole): replaced by the bake
  (0026, 0043). 0021's moon at 2.83 planet radii: replaced by 0041. 0021's "the moon keeps its old +-30 m hills":
  Luna has her own bake (0070).
- 0021's sizes N = 12288 / 24576 / 1536 as columns per face: halved by 0033 (6144 / 12288 / 768).
- 0024's "the pull is G over all of a body's terrain within AMP of its radius and falls with the square past that":
  replaced by the interior's profile (0154, 0162). 0024's key moves (hammer shape to B, bucket to V): see
  `docs/playing.md`.
- 0026's "land share 45 %" and its plan that 0021's `Land` is replaced: done (the bake). 0026's "no tilt ends with the
  seasons stage": done (0055).
- 0027's Luna at about 1 km (N = 3072, 1,061 m): replaced by 0041 (Terra / 3.7).
- 0033's "a lamp lights 8 m" and its water thresholds in block units: tuning notes, superseded by later lighting
  and water work.
- 0037's gravity of the square root of the radius ratio (Luna 2.0 m/s2), `r` clamped to 0.9 R, `JUMP` 4.87 m/s and
  `FREE_G` 1.0: replaced by 0041 (Luna pulls 3.7 times less), 0154 / 0162 (inside a body), 0132 (G 19.62, JUMP
  6.84, `FREE_G` = G / 9.81).
- 0131's "trial, awaiting the developer": the developer chose it (see the calls), and generation 13 made it the
  default.
- 0131's trial limits (`warp=0..1` on the client, the server and the bot, `save=0` asserted, a remote server's warp
  given by hand) and its open item "making it the default": the generation carries the grid (draft 2026-11-24).
