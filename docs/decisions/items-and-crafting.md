# Items and crafting

Items, slots and piles, the inventory screens and their look, dropped and held items, tools, hand and grid crafting,
the crucible, bronze, block windows, plant fibre and the shader patterns of a block. What holds today. Numbers in
brackets are the ADRs (`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- Carrying is a hotbar of 9 and rows opened by a worn pack; a dug loose block falls as a pile on the ground, as in
  Minecraft [0074, from 0047]. Death drops everything at the spot on respawn, and the piles stay [0074, decided
  2026-11-13].
- The inventory opens with Tab; Q and E roll again [0075, decided 2026-11-13].
- Knapping by doing: strike flint on rock, no menu [0110].
- A worn-out metal tool gives its stick back, a flint tool vanishes; bronze, not brass; no repair [0139].
- Crafting is a shaped grid: 2 x 2 in the inventory, 3 x 3 at a workbench (a placed, used block); a recipe viewer
  like "Just Enough Items" with Fill [0140, replacing the list of 0139].
- The crucible is one placed block with a plain window (in, fuel, out, heat); 1100 C top; a furnace works only the
  recipes its heat reaches; a furnace's top heat and recipes are data; moulds last forever; wood is fuel enough for
  copper [0141].
- Interacting with a block opens a window apart from the inventory; the recipe viewer sits docked on the right with
  an opaque background [0143].
- Inventory look: nicer UI, a character preview, a better creative menu; grass and the rest keep their real look in
  slots [0113].
- Dropped items always look like their real model; tools painted finer; item models have a size [0114].
- A cheap contact shadow for little things [0114, 2026-11-14]. Dropped tools larger [0114, 2026-11-14].
- When the hand is empty it shows the ape's own hand, as in Minecraft [0115].
- Grass and the other plants get a hit box, can be mined and give plant fibre (for cord and hafting) [0147].
- Block patterns: more of them, with coloured parts and sliders, designed on `tools/review/workbench.html` [0039].

## Items, slots and piles [0074]

- Items (`engine::items`, `data/items.toml`): every block is an item of the same id; other items have ids from
  `0x8000` up in the file's order. A row: `name`, `stack` (1-64), `look` (the block it is drawn as), `pack` (slots it
  opens when worn). Three packs: basket 9, hide pack 18, frame pack 27 (packs stack 1). Tests: names unique and not a
  block's, stack sizes in range, every look a block.
- Stack size: 64 by default; a block row's `stack` (1..64) sets it, rock, rubble, ore and gossan 16 [0074, amended
  0093].
- A stack is (item, n, payload: u64). A slot is `Option<Stack>`. The payload counts a tool's digs had (0 is new) [0110].
- The ape has `items::SLOTS` = 46 slots: 0-8 the hotbar, 9 the worn pack, 10-36 the rows a pack opens (only as many as
  the pack holds are usable), 37..45 (`items::CRAFT_AT`) the crafting cells [0074, 0140]. `give`, `count` and `take`
  keep to the first 37. Taking a pack off while its rows hold things is refused.
- The rule (`items::rule`): a move into an empty slot or onto the same item merges up to the stack size; onto another
  item, a whole stack swaps; anything else is refused. Every move is all or nothing.
- One message: `ToServer::Items { seq, from, to, n }`; a holder is the ape, a pile's cell, creative's grid (slot =
  item id; a stack put there is gone), the ground ahead (a throw), and a furnace (`Holder::Furnace(Block)`, 0141).
  The server checks reach (4 m to a pile), creative for the grid, and the rule, and answers `Done`.
- Piles: an air cell's 4 slots (`items::PILE_SLOTS`), kept in a `BTreeMap` by (body, face, cell). A survival dig of a
  loose block lays its drop where the block was, on the first floor below; a hammer's shape gathers its drops into as
  few piles as hold them, in the cells nearest the ape. A pile falls when the floor under it is dug; a laid pile falls
  at most 64 layers and floats where that ends. Every tick each living player takes what fits from the piles within
  1.5 m of its feet; a throw waits 2 s for its own thrower. Death lays all the ape carried at its feet on respawn.
  Nothing despawns. A pile belongs to an air cell and costs no tick time. (Rubble of a dug rock goes in the cell, not
  to a pile: 0093; also mining-and-digging.)
- Wire: `Inventory { creative, slots, water }` to the owner; `Piles(Vec<(cell, slots)>)` with the piles that changed
  this tick to everyone (every pile on join, split like edits; an empty list: gone). `Welcome` carries the items
  table's hash; a client with other data is refused.
- Saves: version 13 added every player's slots and every pile, with the items table by name (a loaded save's ids found
  by name; an unknown name refuses the save). A version 12 save's counts load into slots in block id order, what does
  not fit lying at the player's feet. A player whose slots do not fit is re-packed (`ape_fits`); a stack over its size
  loads split. A 37-slot save loads with 9 empty cells [0140]. Later versions of the save format: 15 and 16 for
  furnaces (below); see multiplayer-saves-and-data.md for the rest.
- The conservation law (fundamentals 4.8): the server keeps, per item, what digs, places, `/give` and creative's grid
  made or took; `item_ledger()` lists every item whose count over players and piles differs. The soak fails on the
  first one; its bots move, split, throw and take stacks at random. The state hash covers every slot and every pile.
- Pick-up looks at the 27 cells around each player, so a pile nobody is near costs one map entry and no tick time;
  the client draws piles within 64 m; a throw lands 2 m ahead. Items past the top layer are lost (shortcut; the soak's
  ledger would catch it). The littered-face benchmark of 100,000 piles is not built [0074].
- Things got before a player's name, or held by a client that left unnamed, are laid at its feet. A pile just over a
  cube face's edge is not picked up until the player steps across (shortcut).
- Bot commands: `slots` (JSON of what they hold), `move a b n`, `drop a n`, `take dx dy dz k a n`,
  `put a dx dy dz k n`, `grab <item> a n` (creative's grid); later `hold k`, `strike dx dy dz`, `craft [n]`,
  `use dx dy dz`, `close` [0110, 0140].
- `/give <thing>` gives one stack by default, at most 36 stacks; `_` reads as a space in names [0074, 0110].
- Left as shortcuts: tools' food day in the payload, containers as holders, vessels and the bucket as items, the
  planet's material ledger not seeing carried things, piles sent to every client instead of near ones, no float or
  sink in water for a pile, no right-click set-down.

## The inventory screens [0075]

- Tab opens and closes it (`Act::Inventory`, rebindable, `acts=` `inventory`); Esc closes it too. Tab still completes a
  command while the chat line is open. It is a `Menu` (the world runs on, the pointer is free). `settings.txt` version
  2: a version 1 file holding the built defaults (inventory on E, roll right on R) moves to the new ones; a key the
  player chose stays [0075].
- Click lifts a stack and puts it down (onto another item: swap); right click lifts half, or puts one; shift-click
  sends a stack across (hotbar to rows, rows to hotbar, a pile into the ape); a click outside with a stack lifted
  throws it.
- Creative: the hotbar and every block and item in a grid, 9 by 6 (`GRID_ROWS` 6 [0113]), scrolled by the wheel, with
  six tabs and a search. Tabs are derived (lamps: blocks with light; ore: the minerals' blocks; rock: the rock layers;
  made: `data/items.toml`'s `made` list; items: the non-block items; nature: the rest). A click takes a stack, right
  click one, shift-click a stack into the hotbar; a stack put onto the grid is gone.
- Middle click on a block in the world (`Act::Pick`, creative only): takes the hotbar slot that holds it, else a stack
  of it into the slot in hand.
- Prediction: the client applies a move to its own slots with the server's rule at once and keeps the server's
  `Inventory` back until every move it sent is answered; a pile's change comes with the server's answer.
- `hud::RECTS_MAX` is 32,768 rectangles (creative's nature tab is about 11,000).
- Off-screen `acts=`: `inventory`, `point:x:y`, `lclick`, `rclick`, `sclick`, `type:`, `back`, `view:<item>`, `fill`,
  `sslot:<k>`, `cast:<name>` [0075, 0140, 0141].
- Left as shortcuts: drag-spread, number keys sending to a slot, predicted piles, pack rows drawn as that pack.

### Three surfaces [0143]

- The inventory (Tab): the ape, the Pack slot with health and hunger, the pack's rows, the hotbar, "On the ground" and
  the 2 x 2 grid under the pile. Never a furnace's slots.
- A block's window (workbench, crucible): smaller, titled by the block's name, the block's own part on top, the pack
  rows and hotbar under it; no ape, pile or Pack slot. Click rules, shift-click across and the server's Close rules
  are the inventory's; no wire change.
- The recipe viewer: its own opaque panel docked at the right edge, the screen's full height, shown with the
  inventory and any block's window (creative's grid has none). The window is centred in the room left of it; at
  1280 x 720 and 1920 x 1080 neither is drawn smaller nor overlaps. Fill fills the open window's grid; at a crucible
  there is no grid and Fill is not shown, while furnace works are still listed.
- A new window kind adds a `Screen` variant, its arm in `top` (slots, boxes, width, height) and its drawing in
  `App::top_draw`. The layout test covers the five screens at four UI sizes.

## The look of the inventory [0113]

Client only (`crates/client`); behaviour and wire unchanged.

- One window for both modes (`inventory::layout`, pure, drives draw and click): the HUD's glass with the accent's
  corner brackets, a title row, cyan section headings, slots with a 1 px edge, a cyan edge under the pointer, the slot
  in hand framed in cyan, hotbar slots numbered 1-9, the world a little darker under it.
- Survival: the ape | the pack's rows over the hotbar | "On the ground". The ape stands on a dark inset with a soft
  shadow; under it the worn "Pack" slot with health and hunger as numbers. The rows' heading names the pack ("FRAME
  PACK - 27 SLOTS"). With no pack: "No pack worn" and a dim line. The window keeps its size (three rows' room and the
  ground column) so nothing jumps; "Nothing near" when no pile is in reach.
- Creative: the search is an always-visible field in the title row (a click opens search; it is still `tabs[SEARCH]`);
  six tabs, the open one solid cyan with dark ink; a 9 x 6 grid with a scrollbar thumb; "81 items" under it; a
  divider; the hotbar. An empty search says "Nothing named '<q>'". No ape in creative.
- Scale: `layout` gives a scale `k` (at most 1) that fits the window with an 8 px margin; the layout is in its own
  pixels on a canvas 1 / k as large, and the pointer is divided by k. The layout test runs at 213 x 120, 853 x 480,
  1280 x 720 and 1920 x 1080.
- Pictures of items (`crates/client/src/sprite.rs`, a pure CPU helper, made once per model load on the models'
  watcher thread, drawn as rects of one colour run each): orthographic, every visible voxel face and painted box face
  sampled into a depth buffer, shaded by its turn to a light from the upper left, fitted into n x n with a pixel
  margin. Slots 40 px; the view is `picture_view`: an upright item from its front, a flat one from 70 degrees above,
  a lump from three-quarters [0113, amended 0114]. The ape 176 px from 22 degrees round and 8 above. The face toward
  the eye is its own colour and the others no less than 0.75 [0114].
- `icon` uses: the model of the item's name, else the block's `model`, else the cube (`hud::cube`). A cutout block
  (leaves, shrub, reed) is its cube with the painting's empty texels left open, as in the world (`sprite::flat` is
  gone) [0113, amended 0114]. The cyan corner mark is left only on items drawn as a cube.
- A worn pack on the ape: a block of the pack's look colour behind and over the shoulders, wider the more rows it
  opens, and two straps. The item in hand is drawn at its right hand.
- Tooltip: the name and a dim second line (a tool's "34 of 40 uses", a pack's "worn: opens 27 slots", a stack's "12 of
  64", creative's "stacks to 64"); it stands right of its slot (else left), its middle at the slot's, inside the panel
  [0113, amended 0114].
- Fine faces (32 x 32 paintings, ADR 0090): `gpu::painting` gives their first variant halved to 16 x 16 (`gpu::halve`);
  the hotbar's and the hand's cubes of those blocks show their painting. A fine face with fewer than four variants
  gives none.
- The hotbar shows the same pictures: `hud::layer` hands each filled slot's square to its caller, which draws through
  `App::look_of`; counts and wear stay the hotbar's [0114].
- A tool's slot shows a bar of its uses left (green to red) in the hotbar and the inventory in place of the count
  [0110].
- Left as shortcuts: a season's own painting is not drawn in a slot (summer's, tinted); the pack's place on the ape is
  by the ape model's proportions; icon work per frame is for the optimisation batch (`docs/perf-debt.md`).
- The inventory's cost row is in `docs/perf-debt.md` (GPU `screen` stage +0.22 ms).

## Dropped items and the model in hand [0114]

- A pile's stack is drawn as its model when the stack's item or block name is a loaded model; any other keeps the
  cube of its `look`. Past 64 m nothing; no count in the world. A pile's block is the block's own whole-cell look
  (`Look::Block`) shrunk to 0.2 m about the cell's middle and on the floor.
- Models are model instances in `Models::frame`, one per stack, from `piles::lay` (`crates/client/src/piles.rs`) each
  frame from `self.piles`. A model whose instances would pass `MAX_PIECES` (65,536) is left out whole for that frame.
- A standing item (rest bounds taller than wide) lies on its face (+z up); every other lies as it rests in its cell. It
  is centred on its corner of the cell, rests on the floor by its bounds, turned about up by the cell's hash plus 1.9
  rad a slot, and shrunk (`Lying::fit`) so its turned footprint stays inside its cell. All placement is in cells
  turned to metres by the cell's own half widths (`At::half`; a cell is 0.707 to 1 times as wide at a face's corner).
- A stack whose item row has `tool` lies at `TOOL_LYING` = 2 times its item size (the hand axe 42 cm long).
- Contact shadow (`piles::shade`): one flat square under each pile, one round all its stacks, darkening the floor's
  albedo in rings of whole ground texels (half the light at the middle, none at the edge), middle on the 32 a metre
  texel grid, half width the lying footprint's times 1.15 plus a texel, held inside its cell. None on a see-through
  floor (glass, leaves), over air or under water (the block below must be `BLOCKS.opaque` and `solid`, water asked 5
  cm up). It is in the fx buffer after the particles (`Fx::shades`) and drawn by `shade_pipeline` (multiply into the
  albedo's colour only; `vs_shade`, `fs_shade` in `shader.wgsl`); no setting. Clutter needs none (it is in the traced
  scene).
- Item size: a model's optional top-level `item = <metres a cell>` (0.05..1, refused by name outside), read by the hand
  and the pile; absent: 0.35 in hand (`models::ITEM_M`) and in a pile 0.35 for a voxel model, 1 for a painted-box one.
  Not written when absent. The hand axe `item = 0.3`, flint `item = 0.6` (clutter flint lying in the world stays
  19 cm).
- Painted boxes on the instance path (`Bx::pixel_quads`, `Bx::frame`): each painted box face pixel is a one-voxel quad
  of the model's own quads, empty pixels none, a plane flattened to 2 %; one extra part per box. `vs_model` and
  `fs_model` unchanged. A model with painted boxes whose `.png` is missing, unreadable or too small is refused like a
  refused `.toml` (a reload keeps the last good model with one stderr line; a first load draws it not). "Has a model"
  means at least one quad to draw (`drawn`); otherwise the item lies as its cube. Models are drawn within the shared
  `MAX_MQUADS` = 65,536; the first model over (file order) is dropped whole with one stderr line. Boxes over 63 px a
  side are not drawn on this path.
- In hand: a painted-box model is held by its rest bounds' middle; a flat one (height under 0.4 of its width: flint,
  the stick) is tipped 60 degrees (`Loaded::held`); a voxel model is held by its cell. Held light of torches:
  also see player-and-feel.md or the torch's own doc (0117).
- The Studio's model view copies the open model's `.png` beside its `studio.toml` (written as `studio.png.part`,
  renamed; handed over again on a change of the painting's modified time).
- Models are not traced: a lying model casts no real shadow.
- Open: flint's slot picture still reads as a slab (it needs repainting); a box quad per pixel is never merged
  (merge runs of one colour when quads grow tight).

## The bare hand and the swing [0115]

- An empty hand draws the figure's own arm: `data/models/ape.toml` part `arm_r` (and parts hung from it) as one more
  piece of the ape model on the instance path, placed in the eye's frame by `Models::frame` with
  `hand: Option<Hand>` (`Hand { item: Option<&str>, swing }`). With an item or block in hand the arm is not drawn.
- Pose constants in `models.rs`: `ARM_HAND`, `ARM_WAY`, `SWING_REACH`, `SWING_AIM`, `SWING_CHOP`. The hand's end is
  0.55 m ahead, 0.30 right, 0.30 down; the arm uses the ape's voxels (12.5 cm), 0.625 m long. It is drawn at the ape's
  rest pose (a clip never moves it in hand).
- One swing for everything held: `App::swung` starts a pulse of `SWING_S` = 0.25 s on a dig, a place, a knapping
  strike, a pick-up and every hit of a held survival dig (so it repeats). The HUD cube dips by its pixels; a model or
  the bare arm is thrust 45 % of the way toward a point on the look and chopped down 0.35 rad about the hand
  (`models::swing`, eased). Client only, no wire change.
- `frame.rs::hand_shown`: in play, no hammer or bucket up, no F3, not the title's world, not on the death screen (for a
  held item too). The pause menu and the inventory still show the hand.
- No bob for the arm or a model in hand; the zoom (C) grows it on screen (placed in metres).
- The arm is the part `arm_r` (1 x 5 x 2 voxels, the light hand at its bottom layer), drawn on the existing instance
  path in its own colours and lit as every model is: no new pipeline or shader; a pose in constants, not a `hand` clip,
  because the arm is placed in the eye's frame, not the body's. The swing strikes in toward the aim because a first
  try turned down about the shoulder took the arm out of the view for a third of the swing; a test holds the hand
  inside the view all the way. The arm is 12.5 x 25 cm thick, two plain slabs (painting its voxels is the cheap fix
  if it reads bare) [0115].
- Open: the arm's one piece is not counted in 0114's per-model reserve against `MAX_PIECES`; the rest pose is built
  twice a frame while the hand is empty (for the optimisation batch); in the first quarter second of a run the hand
  may strike once at spawn; holding the item in the visible hand is a later step [0115].

### The swing's numbers in a data file [0172, 2026-11-19]
The pose and swing constants of 0115 live in `data/hand.toml` (`seconds` 0.25, `snap` 2, `reach` 0.45, `aim` 1.4,
`chop` 0.35 rad, `arm_hand`, `arm_way`); the maths is `engine::hand` (`Hand::curve`, `Hand::turn`, `RANGES` with
`get`/`set` for the Studio's sliders), so the Studio reads and measures the same code the client draws by. The curve
is `sin(PI * (1 - (1 - x)^snap))`, x = age / seconds; `snap` 2 is 0115's curve. A value outside its range is refused
by name. Presentation only: the client reads the file again within a second of a change (`engine::hand::HAND`, a
`Live` it sets itself); the server never reads it and nothing goes on the wire. Behaviour at the shipped values is
unchanged.
Replaces: 0115's pose constants in `models.rs` and `SWING_S`/`models::swing`.

## Tools and knapping [0110]

- Data: `data/items.toml` has `[[material]]` rows (`speed` over 1 to 20, `grade` 1..4, `uses` 1..10000) and item fields
  `tool` (a kind of `blocks::TOOLS`: axe, pick, shovel, hammer, knife, or `"any"`), `head` (a material; a tool stacks
  by one), `knap = [item, strikes]`, and later `leaves` [0139]. `data/blocks.toml` has `tool` (every solid block),
  `grade` (0..4, default 0) and `later` (no tool of its grade exists yet). Refused at load with the row's name:
  unknown kind, material or knap target, a material named twice, a grade over 4, a grade or `later` on a loose block,
  a grade without a tool, a tool on a block that is not solid, a tool that stacks, a knap into itself.
- The server learns the hand by `ToServer::Hand(slot)`, sent on join and on each change of slot (slot 0 until told);
  `ToServer::Strike(cell)` is knapping. Two variants appended to `ToServer`; no save format change, the wear lives in
  the stack payload.
- Digging (survival only; `edit::dig_time_with`, server `edit` and `dig`, the client's `dig_step`, the bot's `dig`): a
  cell whose block's `tool` is the held tool's kind (or any, for the hand axe) takes `hardness / speed`; any other its
  bare-hand time. A block that is not loose drops itself when the tool takes it (`Tool::takes`: the kind and at least
  the grade); rock with rubble still leaves its rubble (also mining-and-digging.md). Each dug cell the tool fits costs
  one use (`items::wear`; a hammer's shape as many as it digs, stopping at the last). The last use leaves nothing (or
  the row's `leaves`), the ledger count goes down by one, the client plays `tool.break` and the `knap` chips (they take
  the worn item's `look`).
- Knapping (`Server::strike`): survival, alive, within reach, a knapping item in hand, hard rock there
  (`items::knaps`: the block's tool is pick and hands do not take it; not soil, wood, gravel, rubble, ice or coral). The
  server counts strikes per player and item; a strike sooner than `KNAP_WAIT_S` = 0.4 s after the last that counted
  does not count (the client waits 0.45 s); another item in hand starts again. At 4 strikes one flint goes and a hand
  axe comes: into the flint's slot when it empties, else where it fits, else at the feet; the ledger moves one flint to
  one hand axe. The client strikes on the place button over hard rock with a knapping item, plays `knap` (sound and
  chips) at once. `strike` sends `Hand` first. Only the striker sees and hears a strike.
- The hand axe: `tool = "any"` (every kind at grade 1), `head = "flint"`; flint `knap = ["hand axe", 4]`. Material
  flint: speed 2, grade 1, 40 uses. In hand `data/models/hand axe.toml` (voxels, 5 x 7 x 2), later painted (0114).
- Block tools: grasses, dirt, sand, snow, gravel, clay, mud, regolith, loose-ore gravels, bog iron, bauxite, gossan and
  every rubble need a shovel (grade 0); logs, bamboo, cactus, leaves an axe; shrub and reed a knife; stone, every rock,
  dolomite, altered rocks and the hard ores a pick (grade 1); ice, coral, glass and lamps (loose) a pick, grade 0;
  copper block a pick, grade 2 (copper's `later` dropped by 0139); iron-nickel a pick, grade 3, `later`.
- A tool taken up at the end of a bare-hand hold digs at once (shortcut in `Server::edit`). The knap count does not
  decay and is not tied to one rock.
- Open: slot pictures for tools beyond the cube, hafting (needs a process runner), the worn-away row of the material
  ledger for metals.

## Copper tools [0139]

- Materials and items: `[[material]] copper` (speed 4.0, grade 2, 160 uses, prospecting range 24 m; flint 16); items
  `copper ingot` (stack 64), five heads (stack 16: pick, axe, shovel head, knife blade, hammer head) and five tools
  (stack 1, `leaves = "stick"`). All look like the copper block until painted. `items::wear` leaves the row's `leaves`
  in the slot at the last use; the dig and the prospecting strike count it in the ledger. A worn flint tool vanishes.
- The copper hammer fits nothing (no block names `tool = "hammer"`): bare-hand speed, takes nothing bare hands do not,
  never wears, does not prospect (only pick and every-kind tools do). It waits for a use. The hand axe fits every
  block. The build hammer (ADR 0017) is a different thing.
- Head costs when cast: pick 2 ingots, axe 2, shovel 1, knife 1, hammer 3 (see the crucible).

## Crafting grid and workbench [0140]

- Recipes (`data/recipes.toml`, `engine::recipes`, compiled in): `name`, `grid` (rows, a letter a cell, a space empty),
  `key` (a letter's item or a list of alternatives), `out`. A pattern matches anywhere in the grid and mirrored left to
  right, every other cell empty. One item per cell per make. Refused at load with the row's name: an unknown item, a
  key letter unused, undefined or not one letter, ragged rows, over 3 x 3, an empty edge row or column, an amount of
  0, a name twice, an output among its inputs, two recipes one grid could match.
- A recipe needs the bench when wider or taller than 2 (`Recipe::bench`). No data recipe needs the bench yet except the
  crucible's (3 x 3 round an empty middle).
- Data recipes: `workbench` (2 x 2 of any of the six logs; 4 logs to 1), the five copper tools and five bronze tools
  (head above stick), `clay mould` (2 clay side by side), `crucible` (8 clay round an empty middle, at a workbench).
- The workbench (`data/blocks.toml` id 218): axe, loose, planks pattern in wood colours, hardness 2.5 s by hand,
  `window = "workbench"`. `blocks::WINDOWS` lists the windows a row may name; refused otherwise or on a block not solid
  or falling. Use: `Def::window` to the client's `use_target`/`use_block` to `ToServer::Use(Block)` to
  `Server::use_block`, which sets `Client::bench`.
- The place button's order: a block with a `window` within reach first (whatever is in hand), then a pick-up by its own
  faces (0144), then a drink (0145), then a sleeping place (0148); a strike needs hard rock, which no window block is.
  To place a block by a bench, aim at another face.
- Server-authoritative: the 2 x 2's cells always take moves; the other five only while `Server::at_bench` (the bench is
  still a workbench and within `BENCH_REACH` = 6 m of the feet; `crates/engine/src/items.rs`). `ToServer::Craft {
  recipe, n }` takes the grid's output; the server makes only when the cells hold the recipe of that name (a guard
  against a pending move). It matches the 3 x 3 at a bench, else the 2 x 2 alone. Each make takes one item from every
  full cell and gives the output into the usable slots, all or nothing; shift asks for `u16::MAX` and the server stops
  when the cells no longer match or the output has no room. The ledger moves each item. `ToServer::Use(Block)` and
  `ToServer::Close` are appended.
- Never lost: `Close`, a walk out of reach, a death (at once; `uncraft`, so it ends in the pile, 0148), a leave and a
  join put the cells into the usable slots, the rest at the feet. A save keeps what lies in the cells.
- Crafting by name as a make is gone (0139's list and Make column). Bot: fill cells with `move`, take with `craft [n]`,
  `use dx dy dz`, `close`.
- The screen: the grid beside the pile with an arrow and the output (what the cells match, its name under it); a click
  takes one, shift as many as the cells hold. The recipe viewer: a search field (click to type) over every item whose
  name holds it, recipes first; a click shows the recipe (the pattern in a 3 x 3 from its top left, a cell red when
  slots and cells hold none of its items, alternatives shown in turn), what it makes, "needs a workbench" or "by hand,
  2 x 2", and Fill. The viewer shows `VCOLS` x `VROWS` = 5 x 7 items (0143 said 5 x 6; 0140 said 11 x 2) and the wheel
  scrolls (checked in code: `crates/client/src/inventory.rs`). Fill (`inventory::fill_plan`) first moves what lies in a
  cell the pattern does not want back to the slots, then one wanted item into each pattern cell, all by ordinary
  `Items` moves; cells nothing was found for stay empty and red; with the 2 x 2 open and a bench recipe shown Fill is
  dim and the line says it needs a workbench. An item with no recipe says where it comes from when the data knows
  (knapped from, found digging, left by digging), else "no recipe".
- No prediction of the output (one server tick), no drag-to-spread over cells, Fill lays one make's worth. "Hold P to
  analyze" explainer is not built; its place is under Fill.

## The crucible [0141]

- The block `crucible` (`data/blocks.toml` id 219): `window = "crucible"`, loose, drawn by the furnace model; made at a
  workbench from eight clay round an empty middle. Right click opens it; anyone within `BENCH_REACH` may use it, and
  walking out closes it. Digging it lays everything it held as a pile at its cell.
- Slots: `furnace::SLOTS` = 5 [0141, amended 0142]: In (0), Fuel (1), Mould (2), Out (3), In2 (`furnace::IN2` = 4;
  appended so the first four keep their numbers). Server-authoritative; kept in the server's map `furnaces` beside the
  piles, keyed by the block's cell, reached as `Holder::Furnace(Block)`, counted by the items ledger, saved with the
  world. The fuel slot takes only fuel and the mould slot only moulds; the out slot only empties. It ticks while
  there is fire or heat above the air's and is dropped when idle and empty. Several players may share one crucible.
- Fire: a fuel is lit only when the fire is out and there is work; a lit fuel burns its full seconds (even after the
  work ends). Heat moves at `rise` C/s toward min(the fuel's heat, top) and falls at `fall` C/s to the air's heat
  (`furnace::AIR_C` 20 C, one constant). A work's seconds count only while the heat is at or above the work's heat.
- Rows in `data/recipes.toml`: `[[melt]]` (input to output), `[[cast]]` (input and a kept mould to output; needs a cast
  chosen in the window), `[[fuel]]`, `[[furnace]]`. Refused at load with the row's name: an unknown item, an amount of
  0, heat over `HEAT_MAX` 4000 C or seconds over `SECS_MAX` 600, a melt with a mould or a cast without one, a work
  that makes its own input, a name twice, two melts of one input, a fuel twice, a furnace on a block that does not
  open the window, rise or fall not over 0.
- Protocol (appended): `ToServer::Choose { at, cast }`; `ToClient::Furnace { at, f }` to those with it open, only when
  it changed; `ToClient::Fires` to everyone, so the model plays its `working` state (fire, smoke, sound, light).
- The window shows the slots, the fire's seconds left, the heat as a number and as a bar marked at the work's need,
  the work's arrow and percentage, and the row of casts to click. The viewer shows melts and casts "in a crucible, at
  1085 C" with a dim "No fill". Shift-click from the pack: fuel to Fuel, a mould to Mould, the rest to In (to the
  first In that holds the item or is empty, else the second).
- Numbers: crucible top 1100 C, rise 100 C/s, fall 25 C/s; stick 8 s at 700 C (too cool for copper); logs of any kind
  40 s at 1150 C; coal lump 80 s at 1300 C; copper melts to 1 ingot from 4 nuggets, 2 gravel, 1 malachite rubble or 1
  chalcopyrite rubble, 1085 C, 8 s; casts pick head 2 ingots, axe 2, shovel 1, knife 1, hammer 3, 1085 C, 5 s, clay
  mould kept; clay mould 2 clay side by side; crucible 8 clay round an empty middle.
- Save version 15 added furnaces (a 14 loads with none); version 16 gave a furnace five slots (a 15 loads with an
  empty fifth). The state hash of a world without furnaces is unchanged (furnaces hashed only when there are some).
  Cost: about 0.04 microseconds a burning crucible on the tick; no perf-debt row.
- Death only closes a crucible's window: its slots are the crucible's, not the ape's [0148].
- Open: the bloomery, blast furnace and kiln are each another `[[furnace]]` row and window plus rows (nothing built);
  nuggets and rubble smelt but the ore ledger never held that metal; one rubble gives one ingot (a crusher later should
  give more); the copper hammer head has no use; the air's heat is not the biome's.

## Tin and bronze [0142]

- A second input slot on every furnace (`furnace::IN2` = 4). A `[[melt]]` may carry `with = [item, n]`: both stacks are
  needed, one in each input slot, either order, taking exactly both amounts. A work with `with` runs only with both
  input slots full; a work of one input (melt or cast) runs only while the other input slot is empty.
- At load: no two melts of one input or of one pair (either order), `with` only on a melt and not its own first input,
  a cast names a `shape` and a melt none, no two casts of one shape and input.
- Casts are by shape: `[[cast]]` has `shape`; the window offers `Recipes::shapes(top)`, the first row of each shape; the
  metal in the input slot picks the row that runs. `Choose` and `Furnace::choice` still name a cast row, and an old
  `Choose { cast: "cast pick head" }` means "the pick head" for whatever metal is in.
- Numbers: tin melts from 4 tin pebbles, 2 tin gravel or 1 cassiterite rubble to 1 tin ingot, 232 C, 4 s; bronze is 3
  copper ingots with 1 tin ingot to 4 bronze ingots, 1085 C, 12 s; bronze casts use copper's ingot counts at 950 C,
  5 s, clay mould; bronze tools speed 5.0, grade 2, 400 uses, prospect 32 m (`[[material]] bronze`).
- Wire: `ToClient::Furnace` carries five slots; `ToServer::Choose` unchanged, so old recordings play.
- Shortcuts: bronze's items are drawn with `look = "copper"`, the tin ingot as cassiterite, until painted; the bronze
  hammer has no use yet.

## Plant fibre [0147]

- Small plants (clutter rows of kind `plant`, ADR 0091) are hit by a box: the middle of the cell, 0.35 m each way of
  its middle (`PLANT_BOX`), from the ground up to the top of its painting's planes. It does not follow the hashed shift
  or the sway; the outline goes on the box.
- Aim: `clutter::thing_at` asks a cell for the pick-up lying there (not taken); with none, for the plant growing there
  at the Vegetation that is drawn (the client's setting; the server uses the highest, a lower setting's plants being a
  subset). `clutter::aim_thing` tests `thing_faces`, a plant's six box faces.
- Left click cuts a plant, whatever is in hand, if what it gives fits. The right click does not pick up a plant (a reed
  by the water still lets you drink). Only a plant whose row `gives` an item that exists, on a column that holds no
  pick-up in any season, taken or not, can be cut; a plant that gives nothing (wild wheat, flowers, the berry bush) is
  not aimed at and the server refuses it.
- Wire: the same `ToServer::Take { seq, at }`. The server checks reach, finds no pick-up, finds the plant, gives what it
  gives (all must fit) and writes the cell as air: a plain edit sent to every client at once (not through the tick's
  changes, so no water is regapped). A bot's `pick dx dy dz` cuts a plant the same way. No wire or save change.
- Data: `gives` on a plant row (none if left out). Short grass and fern give 1 `plant fibre`; tall grass and reeds 2.
  A data test holds every `gives` to an item of `data/items.toml`. New item `plant fibre` with a draft model
  (`data/models/plant fibre.*` by `tools/paint_clutter.py`, listed in `data/drafts.txt`). No recipe takes it yet
  (`crafting.md` names it for cord and hafting).
- Open (the developer's calls still unanswered): a cut plant never grows back (and clears a decoration on its column);
  each cut is one air edit in the save without bound; whether a cut should need a tool or give more with a knife;
  whether the outline should follow the planes; the client's season clock may differ from the server's at a half-season
  boundary (the Take is refused).

## Block patterns with parts [0039]

- A block's look can be a shader pattern drawn by `texel` in `crates/client/src/shader.wgsl`: plain, grass, stone and
  eight more: pebbles, speckle, frame, weave, planks, tiles, bricks, rings (ids 3-10 in `Look.info`'s low byte). The page
  `tools/review/workbench.html` (`PATTERNS`, `darker`, `lighter`, `part`, `texel`) is the place to design them; a new
  pattern goes into `part` in both with `game: true`.
- Optional block fields (`crates/engine/src/blocks.rs`, `deny_unknown_fields` kept): `colour2`, `colour3` (0..1; default
  the page's `darker` x 0.55 and `lighter` x 1.25 + 0.2 capped at 1 of `colour`, rounded to two places), `size` (1..8
  texels, whole) and `amount` (0..1) (defaults per pattern; plain, grass and stone refuse them), `contrast` (0..2,
  default 1, exactly the old look at 1), `side` (a non-grass block takes the named block's whole look on its four sides
  and foot; grass keeps its fringe rule).
- The `looks` buffer is 64 bytes a block (`Look`: colour, info; colour2, size; colour3, amount; contrast). On a side the
  pattern's x runs along the face and y down from its top edge. WGSL rounds halves to even, so the page's `Math.round`
  is `floor(v + 0.5)`. The registry hash covers the file's whole text.
- A painted texture wins over a pattern: patterns are the fallback for a block or face with no PNG [0039, amended 0045,
  0084; also studio-and-art.md]. The CPU draws the hotbar cubes and the bounce rays with the block's base colour (the
  painting's mean when painted).

## No longer holds

- 0074's "a stack is 64 for every block": rock, rubble, ore and gossan stack 16 (0093).
- 0039's hotbar and "looking at" cubes drawing "8 x 8 noise in the block's colour only": they take the top's and side's
  painting means (0045) and fine faces' paintings (0113).
- 0074's creative grid 9 by 5 and six tabs of "tabs ... and a search": the grid is 9 by 6 and the search a field (0113).
- 0075's first build with the inventory on E: it is Tab (0075 itself, decided 2026-11-13).
- 0075's one-row panel and its clipped hint: replaced by 0113's window.
- 0075's piles drawn as still debris cubes of a quarter metre: replaced by 0114 (models, the block's own look at 0.2 m).
- 0110's line that flint shows its clutter model in hand: it did not until 0114 drew painted boxes on the instance path.
- 0110's "items have no slot pictures; cube with a cyan mark": replaced by pictures (0113, 0114).
- 0113's "cutout blocks as flat paintings": replaced by the cube with open holes (0114).
- 0113's tooltip placement above the slot: replaced by beside the slot (0114).
- 0114's first piles shade of four overlapping squares: replaced by one round shade (0114 review).
- 0115's cube dip of 0.18 s: replaced by the 0.25 s swing in the same ADR.
- 0115's pose constants `ARM_HAND`, `ARM_WAY`, `SWING_REACH`, `SWING_AIM`, `SWING_CHOP`, `SWING_S` and `models::swing` in the client: replaced by `data/hand.toml` and `engine::hand` [0172].
- 0139's recipe list (the Make column, recipes by `in`/`process`, `ToServer::Craft` as a make by name): replaced by
  0140; its copper tools, `leaves` and the hammer rule stand.
- 0140's crafting grid and viewer drawn inside the one inventory screen, the viewer under the hotbar at 11 x 2: replaced
  by 0143.
- 0141's four-slot furnace and cast row of copper only: amended by 0142 (five slots, casts by shape).
