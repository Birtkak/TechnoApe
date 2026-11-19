# Mining and digging

Edit shapes and their validation, the survival dig (time, hold, cracks drawn), mining by reading the rock (cracks,
rubble, undercut), loose blocks that fall, ground clutter and pick-ups, flint. What holds today. Numbers in brackets
are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number. Design: `docs/design/mining.md`,
`docs/design/fine-cells.md`, `docs/design/clutter-and-pickups.md`.

## The developer's calls

- Survival dig time is "the same like Minecraft": a held dig that takes the block's hardness, cracks that spread
  while it is dug, creative instant. Stone by hand is slow and a shape takes its slowest cell's time. Looking away
  keeps the progress 5 s. [0034]
- Mining plays unlike Minecraft and Vintage Story: read the rock, clear the rubble. Ore bodies show in the rock
  around them; rock breaks in chunks along visible cracks or when undercut; broken rock is loose rubble that blocks
  the way; a plain pick always works; no waiting mechanics (no fire-setting, no wedges); no weight system; rock,
  rubble and ore stack to 16; a chunk drop hurts who is under it; blasting comes later; the mine's hazards (water,
  heat, air, weak roof) and washing lean ore are liked as proposed. [0087]
- A granite tunnel must always have a joint to read (any 8 columns, basalt any 6). A bedding line is drawn as level
  lines tilting with the fold, not a brick pattern of square-ended lines. [0094, the overseer's reads]
- Falling sand buries and hurts: a loose block that lands on a player falls through their cells and settles under
  them; the player is held until they dig out. [0079]
- Powders only fall: "just have sand and gravel fall down under them but nothing else for now"; slopes, slides and
  glides stay in the code behind a switch for a better fix later. [0106, replaced 0101's behaviour]
- Flint: "pick ups on the ground or inside a gravel block (you get gravel and flint) when broken". [0097]
- The first age starts with bare hands taking loose things: sticks, flint, loose stones, what grows. [0082]

## Edit shapes

- An edit is one box: `engine::edit::Shape { body, face, min, max }`, always on one face. A drag across a seam ends
  at the seam (`Shape::between` returns `None`). A shape also carries a part of its box (cylinders, cones; ADR 0017,
  not this document). [0014]
- Limits: `MAX_SIDE` = 16 blocks (the 16 m reach) and `MAX_CELLS` = 512 cells. (checked in code: ADR 0014 said 32
  and 2048 for 0.5 m blocks; ADR 0033's 1 m blocks gave 16 and 512.) [0014]
- `edit::changes` is the one rule, shared by the server and the client's prediction: digging clears every non-air
  cell; placing fills every air cell except the cells a player stands in. A wall dragged through a player leaves that
  player's cells open and the client predicts the same. [0014]
- `ToServer::Edit { seq, shape, material }`. The server refuses the whole edit (whole or nothing) unless every cell is
  on the body, the block is registered and every cell is within reach. It answers `Done { seq, ok }` after the tick's
  `Edits`, so the client's prediction has one answer per edit to confirm or revert. [0014]
- Survival: placing costs one block per cell filled; an edit the inventory cannot pay for in full is refused.
  Digging adds to the inventory by the drop rules below. (The count per block id is replaced by slots, stacks and
  piles: also items-and-crafting.md.) [0014, amended 0074]
- Creative counts nothing. `ToServer::Creative(bool)` switches modes; the server takes it from any client (open
  until accounts and permissions exist). The server sends `Inventory` to the owner on join and on change; the state
  hash covers inventories and modes. Players start in survival with nothing; bots and benchmark runs switch to
  creative. [0014]

## Survival dig time and the hold

- Data: `data/blocks.toml` gives each solid block `hardness` (seconds by hand, checked 0..60) and a `loose` flag
  (bare hands take it into the inventory). By-hand times: grass 0.9, dirt and regolith 0.75, lamps 0.45, stone 7.5,
  copper 15. A shape's time, `edit::dig_time`, is the largest hardness of its solid cells. [0034]
- With a tool in hand a cell whose block's `tool` fits takes `hardness / speed` (`edit::dig_time_with`); any other
  block takes its bare-hand time. A tool is not part of this ADR set; see items-and-crafting.md. [0034, amended 0110]
- Rock and ore do not drop by hand in the old way: they leave rubble (see Rubble). A block that is not loose and has
  no rubble drops nothing by hand. [0034, amended 0093, 0110]
- One counter for both sides: `edit::Hold` counts the hold on the server and on the client on each one's own clock.
  A shape left keeps its seconds for `DIG_KEEP_S` = 5 s (at most 8 shapes kept); coming back resumes them. [0034]
- Client: ToServer::`Dig(Option<Shape>)` is sent when what is held changes. In survival, left click held gathers
  time on the aimed shape; the dig goes once the shape has its time, and held on, goes on to the next block under the
  aim. Every 0.25 s held is a hit: a few `chip` particles off the nearest face and the block's dig sound, softer and
  higher. [0034]
- Server: a survival air `Edit` is refused while `held + 0.15 s + 5 % of dig_time < dig_time` (0.15 s for the
  network; the 5 % because the server's tick clock drops a late tick's time). A hold counts only on a shape in
  reach with something solid in it. A player who leaves ends its dig for the others. Creative is unchanged. [0034]
- The server tells the others `ToClient::Digging { id, at: (shape, share done, share a second) }`; they run the cracks
  on from that without a message a frame (one small message per change in what someone digs). [0034]
- Dig cracks are an fx look (`crack`, 3), drawn each frame over the real cell's faces: up to 64 cells, reserved past
  the pool so they never grow the buffer; texel-aligned rays in the block's own texels, darkened, ten stages. Shown
  for the dig held now, the shapes kept, and other players' digs. They go through the effects, so a run with effects
  off shows no dig progress. (also effects-and-sound.md) [0034]
- Bots in survival hold each `dig` for the block's dig time (x 1.1 + 0.1 s), as the server checks the hold. [0097]

## Rubble

- `rubble = <id>` on a block row (`data/blocks.toml`): rock (stone, the 13 rocks of `rocks.toml`, Luna's four) and
  hard ore (malachite, chalcopyrite, cassiterite, hematite, coal, galena, gold quartz, pitchblende). The registry makes
  `<name> rubble` with that id: the rock's colours and sound in pebbles, solid, `loose`, `falls`, 0.5 s by hand
  (`RUBBLE_S`), and `rubble_of` naming its rock. Ids 160..199 are kept for them. [0093]
- Soil, sand, gravel, clay, wood, leaves, ice, made blocks and loose ore (the gravels, bog iron, bauxite, gossan)
  name no rubble: they are taken as themselves (a pile at the cell, ADR 0074's rule; also items-and-crafting.md). [0093]
- A survival dig of rock or ore leaves its rubble in the cell (`edit::dug`, one rule for server, client prediction and
  bots). Nothing goes to the slots and no pile falls. The rubble falls and buries as sand does. `edit::breaks` counts a
  change into `was`'s own rubble as a dig for the effect and sound. [0093]
- Digging rubble puts it into the slots; a dig with no room for all of its rubble is refused and the rubble stays a
  block. Placed, rubble falls again (a spoil heap). Creative digs anything away. [0093]
- Stacks: `stack` on a block row (1..64, default 64); rock, rubble, ore and gossan 16. [0093]
- The ore ledger counts ore in place: a dig of an ore block takes it from its body; rubble is mined ore wherever it
  lies, and `ore_recount` from the edits equals the tally. What rubble of ore is smelted is the items' ledger's
  (also rock-minerals-and-ore.md). [0093, 0063]
- Stone and ore placed back come only from creative or `/give` until tools take whole rock. [0093]
- Open: smelting takes rubble of ore as its input. [0093]

## Cracks and the drop along them

- `engine::crack` is a rule over (rock, place), computed from the rock, the column's folded top (`Rock::top`) and the
  cell, the same on server and clients: no wire, save or protocol change. A change to it is a protocol change like the
  rocks. A block row says `bedding = m` (layered: limestone 2.5, sandstone 3, shale 1.5) or `joints = n` (massive:
  granite 4, basalt 3). Slate, gneiss, schist, the other massive rocks and the halo's altered rock do not crack. [0094, 0087]
- Bedding planes lie every `m` metres under the column's folded top, so they wave with the layers; a plane runs along
  the top face of the cell it crosses. Joints are vertical planes between columns: one in every run of `n` columns at a
  hashed place, so joints stand 1 to 2n - 1 columns apart and any face 2n blocks wide shows one. A joint holds for 16
  columns along and 8 m up, then the next stretch places its own, and every 3 m up it steps a column or not. Placed rock
  cracks by the same rule. The faces of a cell on a crack are 6 bits (`crack::of`). The joints' hash mixes the body's
  size and place, not the world's seed (`Body` keeps none). [0094]
- The drop: a survival dig of a block on a crack (a good hit: a bedding plane through the cell, or a joint on its +i or
  +j side, `crack::HIT`) turns 2 to 5 more of the same rock into rubble (3 to 6 in all, by a hash of the cell): the cells
  on the same plane and side, found breadth first in a fixed order, at most `SEARCH` = 32 cells looked at. They fall
  where open and are dug up as rubble. Off every crack a dig takes the one block. Creative unchanged. Good hits run
  about 2 digs in 3 in shale, 2 in 5 limestone, 1 in 3 sandstone, 4 in 9 granite, 5 in 9 basalt. [0094]
- The look (level 0 chunks only): the mesher marks each face of jointed rock with the joint edges among its four sides
  (bits 20-23 of the quad's high word; lamps' joins use 16-19) and the shader darkens that edge texel to 0.3. A bedding
  plane is a line across each open side face (bits 24-31, `crack::bed`) taken from the folded top at the face's two
  vertical edges, so it runs on through neighbouring faces and tilts with the fold. A marked face never merges. A cell
  with a crack edge draws no slope. The arrays are made only for chunks with cracking rock beside an open cell.
  (also rendering-and-lighting.md) [0094]
- Hurt: a rubble block that falls into a survival player's head cell takes `ROCK_HIT` = 4 points of 20 once, with
  "<name> was hit by falling granite rubble"; burial then holds as for sand. Sand hits nothing. The check is once a
  tick per client's head. [0094]
- Known limits: a crack beside glass or a cutout block draws no line; a far chunk (level > 0) draws none; near a cube
  face's edge the drop's axes can turn and the server alone decides (no desync); a bedding line is drawn from the plane
  at the face's edges while the drop decides by the plane at the column's middle, so a tilted layer can differ by a
  cell. A fold steeper than 1/4 m a metre is drawn at that slope, its line breaking at the face's edge; a joint's jog
  has no line along the step; lines on top faces are joints only. A joint shows as a dark line two texels wide, as
  two faces meet at it. [0094]
- Cost: mesh jobs 359 us against 332 on main (+8 %: the crack arrays and corner tops of exposed bedded rock); the drop's
  search 4.7 us a dig on average, 57.7 us at worst, 3.49 more blocks loosened on average; a crack lookup 0.66 us;
  a row in `docs/perf-debt.md`. [0094]

## Undercut and drop

- A block row may say `span = n` (1..16 columns, bedded rock only, checked by the loader): shale 1, limestone 2,
  sandstone 2. A row without it never drops by undercut (granite, basalt, every other block). [0112]
- `engine::crack::undercut(world, dug, loose)` is read before the dig changes the world; the dug cells and the cells
  `crack::drop` loosened with them count as open. From each dug cell flood its opening at its layer: 4 neighbours
  (across cube-face edges by `fold`), cells that are not solid, breadth first in a fixed order. An opening over `OPEN`
  = 128 cells drops nothing. Each opening cell's `d` is the fewest 4-neighbour steps to a wall (beside a wall is 1).
  The cell over an opening cell holding a rock with a `span` (and rubble) drops when `d > span`: it and the same rock
  over it up to and including the first cell with a bedding plane on its top (`crack::of & PL`), at most `SLAB` = 4
  cells; any other block stops it. In flood order, at most `UNDERCUT_MAX` = 48 cells a dig. [0112]
- The dropped cells go as 0094's loosened ones: the server turns them into their rubble by `leave(old)` (same
  ledger), they fall and hit heads. Survival digs only. Placed rock follows the rule. The client's `fxedit=...:dig`
  mirror calls it too. Computed at the dig: no wire or save change. [0112]
- Shortcuts: an opening over `OPEN` cells (a big cave) never drops; only the dug cell's layer is flooded; a slab is a
  column (no support search); a slot cut into a cliff or bench floods to the cap, so only roofs of closed rooms
  drop; loosened cells count as open though their rubble stays until it falls. [0112]
- Open: the weak roof (props are a wall to the flood) is a later system; mining.md 3. [0112]

## Loose blocks fall

- `falls = true` on a block row (sand, gravel, rubble), apart from `loose`. [0079, 0093]
- `engine::fall::Falls`: an ordered awake set (layer first, bottom up), woken by every changed cell (it, the cell over
  it and under it, the four sides at both layers, across face edges by `world::fold`). A tick: a falling block over an
  open cell falls a cell when g t^2 / 2 (rounded) says, at most one a tick; blocks on it come down with it; one on a
  block that is still waiting waits with it. `BUDGET` = 1,024 visits a tick, the rest carried to the next, lowest
  first: never a stall. Loading a save wakes every edit that is a falling block or a dug cell, and the block over each
  dug cell; a block not yet moved waits its first third of a second again. [0079, 0106]
- Server: after the players' steps and before the water regaps, this tick's edits wake and the falls step; each move
  is two ordinary edits in the tick's `Edits` (moves go in an `Edits` of their own; water regaps their columns, the
  ore ledger tallies them through `set_block`). Save format unchanged. [0079]
- Client: two edits in a row, a falling block gone from one cell and in the next, side-by-side cells, are a move: no
  break, no place effect, no sound. A move across a face's edge plays its effects. [0079]
- A supported loose block never moves: sand and gravel only fall straight down when the cell under is open. A heap
  keeps any shape and a tower stays a tower. `fall::SLOPES_AND_SLIDES = false`. [0079 slides replaced, 0106]
- A falling block is no floor for a player: it falls through the player's cells and settles under them; the player is
  held by the physics until they dig out. [0079]
- Breath: `Vitals::smother`. With the eye's cell solid the breath lasts `BREATH` = 10 s, then a point a second
  (`CHOKE`) until they dig out; a breath of air starts it again; death reads "<name> was buried". Feet only costs
  nothing; creative takes no harm. `Client::buried` is not saved or hashed: a load or a respawn starts anew. An eye
  inside a block aims at that block (`edit::aim`) so a buried player can dig out. The soak passes a player inside sand,
  gravel or rubble that came by an edit only. (also player-and-feel.md) [0079, 0093, 0146 keeps the 10 s]
- A generated block is not disturbed until something next to it changes (a beach over a cave stays until dug). [0079]
- A falling block teleports a cell at a time on the client (the edit stream): no smooth motion. [0079, 0106]
- `run=stand` with `falls=1` sends `fxedit=` cells to the run's server so a filmstrip shows the real fall. [0079]
- Open: collapse of unsupported builds (slice (c) of fine-cells.md) reuses the falls: an unsupported piece's blocks
  become falling blocks. Fine cells (ADR 0078) wake through the same `wake` with their own awake set and cap. [0079]

## Dormant: slopes, slides, glides (behind `fall::SLOPES_AND_SLIDES`)

Switched off by 0106, kept in code and tested with the switch on (`step_as`, `mesh_as`); setting the const to true
brings all three back unchanged. Revisit when a better fix for powders is found (finer cells, a real angle of rest). [0101, 0106]
- Rule: a supported block slides only when it is its pile's top and a side cell and the cell under it are open (stands
  two over the neighbour); a block with anything on it never slides out. [0101]
- Shape: a corner four columns share at the top of layer z sinks a cell when one of the four around it is open, the
  other three loose, and the four over them open; a loose block with open air over it lowers each sunk top corner
  unless all four sink. A quad keeps its 8 bytes: bit 31 of the low word marks a sloped quad, bits 20-23 of the high
  word its lowered corners; a sloped quad never merges. Collision stays the block. `generate_raw`/`skirt` keep chunk
  borders flat. [0101]
- Motion: a moved block glides over `GLIDE_S` = 0.2 s (`fx::Glide`, `vs_fx` bit 20), sheds `grains`
  (`data/effects.toml`); a slump plays `slump` (`data/sounds.toml`) at most every `SLUMP_S` = 0.45 s. [0101]
- Where the switch acts: `Falls::step`, `mesh_cracked`, and the client's glide condition in `frame.rs`; the shader's
  slope path stays and is never taken. [0106]

## Clutter and pick-ups

- Ground small things are a pure function of the seed, the column and the season, drawn as still box models in the
  chunk's mesh; only what was taken is kept. [0082]
- Data: `data/clutter.toml` rows (decoration or pick-up, the ground blocks it lies on, near a tree or in the open,
  seasons, `regrow = "year"`, a pick-up's item), checked by `clutter::parse`; a biome row lists its clutter as
  `[name, per 100 columns]` (held to the rows and to 100 by `biomes.rs`). Rows: grass tuft, fern, fallen leaves, twigs,
  flowers, pebbles (decoration); stick, loose stone, flint, mushroom, berries (pick-ups; items in `data/items.toml`).
  Models are drafts from `tools/paint_clutter.py`, listed in `data/drafts.txt`. The clutter's models are read once at
  the start, not live. [0082]
- The pick: one hashed roll per column. Pick-ups take the bottom of the roll at full weight; decoration the next
  stretch at its weight times the Clutter setting's density (off 0, few 0.25, normal 1, many 4), so a lower setting is
  a subset of a higher one and pick-ups lie the same whatever the setting. A second roll picks the row by weight; `near`
  asks the tile's trunks (within 3 columns). Never on a face's last columns, under the sea, in a river or lake, or on a
  column whose chunk holds an edit; only on the generated top block with air over it. `may` tests a column's roll
  against its biome's rows first (one hash). [0082]
- Seasons: the season nearest the sky's time (`sky::season_at`), the southern half two seasons on; level 0 is meshed
  again when the (season, year) stamp changes or the setting changes. The season is taken per column. [0082]
- Mesh: the chunk holding the ground cell lays the clutter's quads (`clutter::quads`, after the glass, before the
  water). Decoration casts no shadow and has no collision; building on its column removes it. (also
  streaming-and-meshing.md) [0082]
- Pick-up: a left click aimed at the ground under a pick-up sends `ToServer::Take { seq, at }`. The server checks reach,
  that one lies there (`clutter::pickup_at`, the mesher's own rule), that it was not taken this year (ever, for what
  does not regrow) and that it all fits; it fills the slots, counts it in the items' ledger, marks it taken and sends
  `ToClient::Taken` to everyone (all marks on join). The client meshes the chunk again without it and takes only what
  fits (digs otherwise). Save version 14 keeps the taken marks; a 13 loads with none. Aiming rules are ADR 0144's
  (also plants-and-creatures.md; items-and-crafting.md). [0082]
- Settings: Graphics Clutter (off, few, normal, many; normal default); `clutter=` on a run. Pick-ups show and can be
  taken with Clutter off. A regrowing pick-up comes back with the new year. [0082]
- Shortcuts: every taken mark goes to every client and is looked through for every level-0 mesh job; a new season
  meshes every level-0 chunk again; the server does not hide clutter over an edit lower in the column than the ground
  cell's chunk (neither does the mesher). [0082]
- Cost at normal: about 61 us on a level-0 surface chunk's job (816 quads, 6.5 kB a chunk). [0082]

## Flint

- Flint: an item (`data/items.toml`), `stack = 16`, icon a cube of `slate`. [0097]
- On the ground: a clutter row may carry a place rule. `water = n` (1 to 8): the column, or one within n columns on its
  face, has water standing over its ground (`Body::water_at` above the first air). `over = [blocks]`: else the rock under
  the column (`ROCKS` at 6 m down) is one of them. Flint's: `water = 8`, `over` limestone and marble (limestone stands
  for chalk). A row with neither lies wherever its biome rolls it. Flint's `on` is the grasses, dirt, mud, sand, stone
  and gravel; weight 1 per 100 columns in the land biomes (alpine too), 1.5 on beaches. Within 60 m of a spawn the test
  requires 5 flints, and under half of the spawn face's dry land may hold flint. Never regrows (a taken mark is stored,
  ADR 0082). [0097]
- In gravel: a block row may say `holds = ["flint", 0.25]` (an item and a chance, 0 < chance <= 1, only on a `loose`
  block, refused otherwise at load). A survival dig of such a cell drops the block's pile and, if the roll says so, the
  item. The roll is `mix` of the body's baked seed, the body, face and cell (deterministic). No farm: the cell must have
  no edit (`World::edit(b)` is None) and its generated block must be the dug block, so gravel put back or generated
  gravel that fell holds nothing. The flint goes through the same `drops` (`drop_dug`), so `item_made` counts it. A row
  with `holds` is reusable for any loose block. [0097]

## No longer holds

- 0014's inventory as one count per block id: replaced by slots, stacks and piles (0074).
- 0014's `Shape` limits of 32 sides and 2048 cells: 16 and 512 since 1 m blocks (0033).
- 0034's "stone and copper drop nothing, by hand is the only speed": tools and rubble (0093, 0110).
- 0034's table of by-hand hardness stays as data; the tool divide is 0110's.
- 0079's slide to a side at a step of two, and its angle-of-rest default: a supported loose block never moves (0106).
- 0079's "sand falling onto a player rests on the head": decided as burial (0079 Burial).
- 0093's "stone by hand gives nothing" and a dug loose block's pile for rock: rock leaves rubble (0093).
- 0101's calmer slide, slopes and glides as live behaviour: switched off by 0106 (code kept, see Dormant).
- 0094's "slice (c) next": built as 0112.
