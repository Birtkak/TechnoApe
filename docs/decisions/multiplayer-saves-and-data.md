# Multiplayer, saves and live data

What holds today for the client and server split, the protocol, input steps and edit prediction, the block
registry and its ids, saves, recordings, live reload of definitions, the game reading Studio maps, platforms,
hosting and the home server. Numbers in brackets are the ADRs (`archive/NNNN-*.md`) a decision came from;
INDEX.md lists every number.

## The developer's calls

- Targets are a Windows x86_64 client and a Linux x86_64 dedicated server, hosted in a container on the household
  Unraid box and reached over Tailscale. No mobile or console targets (Android was dropped) [0005].
- Players always run the version the server runs, without installing anything by hand; the friends' client
  updates itself [0005, 0130].
- Multiplayer from day one: singleplayer is a local server, bots are ordinary clients, only changes cross the wire
  [0005, 0006].
- A running game should reload what a block, an item or a creature is within a second of a save, the way
  textures, models, sounds and effects do. Asked for as "something like NBT data ... reload all the properties
  everywhere instantly"; the developer said "Do the first 2" of four classes of property [0134].
- The Studio's maps drive the game's plant wind and particle turbulence, and the developer wants both drawn in the
  Studio; a row's `[sway]` is the developer's switch between map and the old sines [0157].
- A higher `rise` on the smoke row (the map smoke stays lower than the old value noise) is left to the developer
  [0157].
- Disk and wire size matter little; time and CPU are the limits (so saves are written whole, simple formats)
  [0016, 0062].

## Client and server split

- One process, the server on its own thread, connections as channel pairs; TCP for remote players. The same
  `Server` code runs inside the singleplayer client and as the dedicated binary `technoape-server`; the client does
  not know which [0006].
- `crates/server` holds `Server`: it owns the `World`, ticks at 20 Hz (`net::TICK_HZ`), applies the edits players
  send, and each tick broadcasts the changed blocks and the player list [0006].
- A connection is a `Link` (a sender and receiver pair, `engine::net`): in memory for the local server, from a TCP
  stream for `connect=host[:port]` (default port 7777, `net::DEFAULT_PORT`). The dedicated binary accepts TCP and
  hands each stream to the same channel [0006].
- Messages are `serde` enums in `postcard` frames with a u32 length (`ToServer`, `ToClient` in
  `crates/engine/src/net.rs`) [0006].
- Singleplayer and multiplayer share one code path; the dedicated server is a build flag, not another program
  [0005].
- The client's `World` is a cache of the server's and may diverge only until the next delta [0006].
- Two windowed clients on one PC throttle each other through the Windows compositor; load tests use windowless bots
  [0006].
- Identity is a name; nothing else (the server sits on a private Tailscale network) [0006, 0005].
- Trust boundary: the server drops an edit unless the block exists, the material is known, and it is within
  `REACH` of the player's reported position (`net::REACH` = 16.0 m) [0006]. The server trusts input values (any
  look, any move in -1..1), not positions, so a modified client can only walk as a player could [0019].
- Positions of `Move` (fly, teleport) are taken as sent in creative; anyone may switch mode (`shortcut:` an
  operator role when servers go public) [0006, checked in code: `ToServer::Move`, `Creative`].
- TCP carries everything; a lost packet delays the stream. Switch to QUIC when playtests show it [0006].
- Wire version: there is no protocol version number. A client and server agree through `Welcome`'s hashes and
  generation (see below); the client refuses a server whose block or item registry hash differs [0013, 0134].
  (checked in code: no `PROTOCOL` or wire-version constant in `crates/engine/src/net.rs`.)
- `Welcome { id, seed, bodies, blocks, items, gen }` is the first message: `bodies` is every body of
  `data/bodies.toml` in row order as (columns per face edge, hash of the baked map or None); `blocks` is
  `BLOCKS.hash`, `items` `ITEMS.hash` (FNV of the file text); `gen` the world's generation. The client rebuilds the
  terrain from them, asks for a baked map it lacks with `Map(hash)`, and then `Edits` brings what changed [0006,
  0013; fields as in `net.rs`].
- A joining player receives every edit in one message (`shortcut:` chunked sync when worlds grow) [0006].

## Input steps and reconciliation

- Numbered steps, each taken once, in order (the Quake/Overwatch model) [0019].
- Wire: `ToServer::Steps { first, inputs }` carries the inputs not sent yet, one per 1/60 s physics step, about
  three per 20 Hz message. Each tick every client gets `ToClient::You { step, me }`: the number of the last step
  taken (0 for none) and the whole `phys::Player` (position, velocity, look, ground contact) [0019].
- `Input` (one input held every tick) stays for bots and the soak, which do not predict [0019]. Sprint rides as
  `Sprints` and `Sprint`, which add each input's sprint bit so recordings from before still read; `ToServer::steps`
  and `ToServer::input` pick the form and `unpacked` returns the old one (ADR 0118, in `net.rs`).
- The server queues new steps in order and drops repeats and old ones. It takes at most as fast as real time: three
  a tick, plus up to 12 saved up when a batch comes late. The queue is `QUEUE_MAX` = 60 steps (1 s); a client
  further ahead loses the rest, so speeding is impossible. A `Move` clears the queue [0019; checked in code:
  `QUEUE_MAX` 60 in `crates/server/src/lib.rs`, credit in `tick.rs`].
- The client does not move before the first `You`, which places it (a saved player starts where it left). On each
  `You` it drops the steps the server has taken and replays the rest from the server's state. A result over 1 mm
  from the prediction counts a correction (`feel.corrections`). The drawn position keeps the old place as an offset
  that eases out with a 100 ms time constant. A jump of 2 m or more (a teleport) is not eased [0019].
- Step sending runs by the wall clock, not by when the world loaded [0019].
- The look is the player's own: a step's input carries `turn` (the mouse's turn about the player's up since the
  last step) and the pitch, no direction. `phys::step` carries `Player::fwd` to the step's up, turns it, and
  `reframe` rotates it with a frame change. A non-zero `fwd` still sets the look outright, for bots and the soak
  [0019].
- A clean local run has 0 corrections and `drift_max_m` exactly 0.0; the gate asks for 0 [0019]. Anything that
  changes the walk must be a bit of the numbered input (ADR 0118's rule).
- Other players are drawn at their last received position; interpolating them comes with the dedicated server
  [0019].

## Edit prediction

- A dig or a place shows in the frame of the click; the server confirms or reverts it [0015, from ADR 0008].
- `engine::edit::Predictor` keeps the server's latest word on every cell a pending edit covers. A cell shows the
  newest pending edit's block; when its last pending edit is answered, refused or not, the cell takes the server's
  word. Exact because the server's `Edits` for a tick arrive before its `Done` [0015].
- Three calls: `edit` applies the shape at once and returns the `seq` to send; `server` takes a cell from `Edits`;
  `done` puts back every cell the server did not confirm and returns them for remeshing. A cell already showing
  the server's block is left alone [0015].
- A shape that does not fit the world is sent but not predicted. A put-back cell with no server edit goes back to
  "generated" (`World::put_edit(b, None)`), so a predicting client's world hashes as the server's [0015].
- Rejected: an undo log per edit (it goes wrong when another player's edit lands on the cell in between, or an
  accepted edit differs on the server). The `edit` benchmark run remeshes in the frame of each edit: 2670 -> 2480 fps
  [0015].
- The soak's bots predict like clients; its check every 100 ticks (each bot's world equals the server's) covers the
  predictor. A predictor that never puts back fails it at tick 100 [0015].
- Movement prediction: snap over 1 m is replaced by reconciliation above (ADR 0007, 0019).

## Block registry and ids

- `data/blocks.toml` lists `[[block]]` entries; `engine::blocks::BLOCKS` parses the file once, built into the
  binary with `include_str!` and now a `Live` table (see Live definitions) [0013, 0134].
- Core fields from the first version: `id`, `name`, `solid` (default true), `colour`, `pattern`, `side`. The
  registry has since grown many more fields in `crates/engine/src/blocks.rs` (the file's schema and its tests are
  the reference, ADR 0039 and later). A new block is one TOML entry using an existing pattern; a new pattern is
  shader code [0013].
- A broken file panics at start-up. Unknown fields, duplicate ids or names, a missing `side`, colours outside 0..1,
  and an id 0 that is not a non-solid `air` are refused, each with a test [0013].
- Ids are stable numbers in saves and recordings; an id is never reused for another block [0013].
- The registry feeds generation (by name), `phys::blocked` (`solid`), the server (accepts any registered id), the
  soak (`placeable()`), and the client's block-looks storage buffer (binding 3), which the shader reads [0013].
- `Welcome` carries the block file's FNV hash (`BLOCKS.hash`); the client and bot refuse a server whose blocks
  differ [0013].
- Ids are `u16` everywhere a block id is held (registry, edits, mesher cells, protocol, saves, inventory,
  structures, effects) [0062]. The 12 kind / 4 state split of `world-data.md` stays the plan; nothing reads state
  yet, so there is no newtype or mask [0062].
- Block ids run to 4095 (see the entry below) [draft].
- Saves and the wire kept their version when ids went to u16: postcard writes a `u16` under 128 as the same single
  byte as a `u8`, so every save, recording and old peer reads the same bytes (test
  `a_save_with_8_bit_block_ids_loads`). A save holding an id of 128 or more is new data an old build could not read
  anyway. A save's edits and inventories must name known blocks (an id past 255 would index past the inventory)
  [0062].
- The edits' chunk hash (part of the server's hash in recordings) hashes an id under 256 as its one byte, so
  recordings made with 8-bit ids still replay; a larger id hashes both bytes [0062].
- One generated block for every reader: the mesher, `phys::blocked` (through `World::generated_under`) and the
  water (`Solid::gaps`) ask `Body::caves` / `Body::col_caves(face, i, j)`: the layers a cave hollows in one column,
  at layer 0 and up. Under the ground those layers are air for `generated_under`, for `generate` at level 0 (a
  chunk a cave touches is never shortcut to `Solid` or `Empty`) and for `Solid::gaps`; over the ground a cave
  changes nothing. Caves are carved at level 0 only; a cave across a face seam is cut at the seam. The check is
  `the_mesher_physics_and_water_see_the_same_cave` [0062] (also caves.md).
- `Body::col_caves` allocates and scans every cave per call; with real cave counts it needs bucketing by face and
  tile and cached spans per column [0062; open, see below].

### Block ids past 255 [0170, 2026-11-19]
Block ids run 0..4095, 12 bits (`blocks::IDS`); the registry refuses a row's or a rubble's id over 4095 by name.
The quad stays 8 bytes: its second word carries the id's low 8 bits at 0-7 (where the material was) and its high 4
at 16-19 (`mesh::id_bits`, `mesh::quad_id`; WGSL `id_bits`, `quad_id`), the occlusion stays at 8-15 and bits 20-31
keep their meanings (crack or slope, twigs and bit 25 on a cutout, bedding). A joining block's four sides moved
from 16-19 to 24-27, a bedding line's place, so the registry refuses `joins` on a cutout block or on rock with
bedding or joints, and the look row flags a joining block (`info` bit 27) for the shader to tell the two apart.
The look table is 4 x 4096 rows of 80 bytes (1.3 MB, was 80 KB), a season IDS rows on (`WINTER_ROW`); a look's
side block is its own word (`Look::side`, was `info` bits 8-15). A particle's block rides at bits 0-7 and 20-23 of
its `info` (`fx::block_bits`, WGSL `fx_id`); its "offset in cells" flag moved from bit 20 to 19. A client test
holds the WGSL helpers and `IDS` to the engine's letter for letter. Nothing on disk or the wire changed: ids were
`u16` there already (postcard writes 128..16383 in two bytes either way), the edits' chunk hash already hashes an id
over 255 in two bytes, and every existing id keeps its number; saves, recordings and fixtures load and replay as
before. An old build given data with an id over 255 refuses the file by name (its registry's 255 check), so a newer
server's rows are refused by its client and the blocks' hash in `Welcome` stops the join.
Measured (play build, `run=stand` and `run=fly quick=1`, two each, interleaved with main, `slot.sh measure`): no cost
past the noise, so no `perf-debt.md` row: stand frame p50 3.47/3.99 ms main against 3.51/4.00, GPU p50 3.34/3.61 against
3.37/3.59, mesh jobs 910/890 us against 923/906; fly frame p50 4.21/4.19 against 4.13/4.22, GPU 4.03/4.00 against 3.91/4.02,
jobs 598/563 against 574/611; VRAM 821 and 1079 MB both; the same quad counts. The look table's 1.2 MB is under the VRAM
reading's step.
Alternatives rejected: one more word per quad (12 bytes; the load's 11.5 M quads take 46 MB more VRAM and every
upload 50 % more bytes, for bits a repack gives free); a per-chunk palette (a lookup per fragment and per ray hit,
and a chunk could hold more than 256 kinds); repacking the first word (a cell's 5-bit place and the face's far
side would free 3 bits, but every reader of the place changes).
Replaces: 0062's 255-id limit and its `shortcut:` in `blocks.rs`.

## Saves

- A save holds only what differs from the seed, written whole: postcard in one file per world [0016].
- Version: `server::save::VERSION` = 22 (`crates/server/src/save.rs`). Version 22 keeps every body's columns and map
  as a list in `data/bodies.toml`'s row order (`bodies: Vec<(i32, Option<u64>)>`) in place of the planet's and the
  moon's; a 21 or older lies on disk as a `Save21` and reads as its two bodies, Terra (`planet_n`, `map`) and Luna
  (`moon_n`, `luna`) (0163-n-bodies) [0016; checked in code].
- What each version added (from the comment on `VERSION`; versions are by those ADRs, not 0016): 2 water (ADR
  0020); 3 the world's time (0022); 4 the world of 1 m blocks (0033); 5 every player's health and hunger (0040);
  6 and 7 the shared coordinates (0041) and the name of the baked map (0043); 8 the live air grid; 9 the river
  reaches' open water (0059); 10 the 192 m crust (0063), a 9 or older moves its cells and water up by the bodies'
  `deep_layers` on reading; 11 Luna's baked map (0070); 12 the rain each reach holds and a pending air job; 13
  slots in place of counts, the items' names and the piles on the ground (0074); 14 the pick-ups taken (0082); 15
  the furnaces (0141); 16 the furnace's second input, five slots (0142); 17 each player's thirst (0145); 18 each
  player's death mark and sleeping place; 19 each player's journal; 20 whether the planet's grid goes down through
  the mantle (0154, 0152); 21 the world's generation (`Body::gen`, 0160); 22 the bodies list (above).
- Reading: a save of versions 1 to 3 holds a world of 0.5 m blocks and is refused. Versions 4 to 21 load and are
  carried over by `Save21` (postcard keeps no field names, so each layout change is another type; `()` is no
  bytes). A save keeps the version it was read as until it is written again. A save on a map of version 1 or 2
  needs that map's file in `saves/bakes/` (since map version 3 a bake makes another climate) [0016; checked in
  code].
- A save holds, at version 22: version, seed, bodies, every registered block as (id, name), every edited cell
  sorted, every named player (position, look, mode, inventory, vitals and the later fields) sorted by name, the
  water, `made`/`removed`/`sea_extra`, the tick, the air state, reaches, rain, the items' names and piles, and
  `gen` [0016; fields as in `save.rs`]. Edits sorted, so one state always gives the same file.
- Loading checks the version and that every saved id still has its name in `data/blocks.toml`. New blocks leave
  old saves readable; a renamed or removed block refuses them. A save that does not load stops the start, so it is
  never overwritten [0016]. The generation must be known (`engine::world::known_generation`) [0160].
- Players are saved by name, as `Hello` gives it; accounts come with public servers. Players who leave stay in the
  server by name, and a `Hello` with that name brings them back with what they held [0016].
- How saving runs: the tick thread takes a snapshot every 30 s; a thread serialises it, writes `<file>.tmp` and
  renames it over the save, so a crash keeps the last good save [0016].
- Files: singleplayer for `run=play` saves to `saves/seed-<seed>-planet-<n>.sav` (`save=file` names another,
  `save=0` none) and saves again on quit; the dedicated server saves to `saves/server.sav`. Benchmark runs keep no
  saves [0016].
- A server that began from a save records the save after `Start`, so replays begin from it too [0016].
- The soak ends with a save and a load that must give back the same world hash and the same save; it runs in the
  gate [0016].
- A clean stop: a line `stop` on the server's stdin saves and ends it (end of stdin does not) [0130].

## Recordings

- A dedicated server records to `out/rec/` with `record=1` (or `record=file`), else not; `technoape-server
  replay=file until=tick` replays one headless [CLAUDE.md; 0016, 0062].
- A recording keeps no live reload: a session that reloaded replays with the build's rows [0134].
- The edits' chunk hash keeps old recordings replayable (see ids) [0062].

## Fine cells

- The vision: blocks are solid and cheap; where something happens they break into fine 12.5 cm cells (debris, sand,
  splashes) and settle back. Design: `docs/design/fine-cells.md`; spike: `prototypes/fine-cells/` [0078].
- A dig's burst is presentation on each client, on the fine lattice: no server cost, nothing on the wire, the ledger
  untouched (the block goes whole to the digger or the item pile) [0078]. Material fine cells on the server for
  every burst were refused: about 400,000 awake cells with 50 bots digging is about 35 ms of a 50 ms tick [0078].
- The dig's chips: `ground = "pile"` in `data/effects.toml` (the `break` row). A chip bounces as before; once slow
  it lies in the 12.5 cm cell under it in its column's own frame (`fx::FINE` = 0.125 in `crates/client/src/fx.rs`),
  on any chip already there, and rolls to a side cell lower down when it would stand on another (the 45 degree rule
  of ADR 0079), never out of its column. A lying chip over a cell that no longer holds a chip falls again, bottom
  first. `break` lives 2.5 to 4 s [0078].
- Each client throws its own chips: two players see heaps alike, not equal [0078].
- Material fine cells: only where the world holds less than a block (chiselling, a blast, a pile that does not fit
  whole blocks): a reserved `fine` kind plus a sparse 512-byte payload per block, its own awake list capped at
  20,000 visits a tick, settling back by a rounding the ledger counts, reusing `FINE`, the 45 degree rule and the
  awake-set pattern of `engine::fall`. Designed, not built; built with chiselling [0078] (also mining-and-digging.md).

## Live definitions

- `engine::live`: `Live<T>` is made on first use and replaced whole by `set`; a reader pays one atomic load. `BLOCKS`
  and `ITEMS` are `Live`. `live::apply(blocks, items)` parses both, puts them in force and counts `live::rev()`; the
  mesher's five bit tables (`SEE`, `GLASS`, `JOINS`, `WOOD`, `FALLS`) are made again there [0134].
- Four classes of property: read when used and baked into meshes or light are live; baked into the world (biomes,
  ore, where plants grow, the ground's layers) and identity (taking rows away, reordering) are not [0134].
- Refused, by name, nothing changed: a file that does not parse; a block gone or renamed (a world holds it by id and
  the code asks for some by name); an item or a creature gone, renamed or moved (their ids are their rows'
  places). Rows are changed in place and added at the end [0134].
- The game's own server (local or dedicated; never a soak, a replay or a test: `Server::live_data`) looks at
  `data/blocks.toml`, `items.toml`, `creatures.toml` and `herds.toml` once a second. A changed file is put in force;
  `World::refresh_defs` derives the lights and models of placed blocks again; `Herds::reload` takes the speeds and
  shares and keeps the animals; `ToClient::Defs { blocks, items }` carries the two texts to every client. A client
  that joins later gets `Defs` before its `Welcome`, whose hashes then agree [0134].
- The client follows `live::rev()`: `Streamer::set_defs` gives every slot a new version (every chunk is meshed
  again, the old mesh stays drawn); its world's lights and models are derived again; its watchers (`gpu::watch`)
  count the rev, so the looks table, the models and the effects are made again [0134].
- At its start a game's server reads `data/` too; a build no longer has to be newer than its data [0134].
- Tables that name blocks (`biomes`, `clutter`, `bodies`) keep the ids they found, which holds because rows keep
  their ids and names [0134].
- Timing: server has a change in force about 0.2 s after the save; the client's looks are made again about 1.0 s
  after (`python -I tools/live-check.py`) [0134].
- Textures, models, sounds and effects reload on their own watchers (ADR 0045, 0047, 0048, 0050); a creature's look
  is live on the client, its speeds are the server's [0134].
- `shortcut:` a replaced table is never freed (about 100 kB a reload); the server parses on its tick thread (about a
  millisecond); the client walks every edit on the frame thread once a reload (about a millisecond per 50,000 edits)
  [0134].
- A file refused, or caught half-written, is looked at again only when its time or size changes again [0134].

## The game reads Studio maps

- One evaluator, `engine::map` (moved from `crates/studio/src/map.rs`): the format, nodes, graph and their tests.
  The Studio re-exports it and keeps only what it does (`check=1` over the folder, the PNG bytes, export, `map=`)
  [0157].
- `engine::map::Field`: a direction map's whole loop (side, frames, seconds, the push per pixel per frame),
  `at(u, v, t)` bilinear in space, linear between frames, wrapping in all three. A number or colour map is refused
  ("map 'x' gives a number, not a direction: wire a Slope or Curl node before its Out"). Worked out on a watcher's
  thread, never the frame thread [0157].
- Particles: `noise = { map = "smoke swirl", strength, scale, speed }` (`scale` the metres a tile covers, `speed`
  times the map's pace, left out: 1). The map lies on the plane of the particle's dominant axis (body frame), its
  part along up taken out (`shortcut:` a seam at the cube's edges; a chart when it is seen). A row without `map`
  moves bit for bit as before (a golden test). A row naming a missing, refused or number map is refused with the
  row and the map named. Maps are read from `data/maps/` of the working folder, else the two shipped ones compiled
  in [0157] (also effects-and-sound.md).
- The plants' wind: `[sway] map = "wind gusts", tile = 48.0` in `data/effects.toml`; `plant_sway` samples a 3D
  texture (frames along the third axis, `Rg8Snorm`, linear, repeating) at the plant's column / tile and the sky's
  clock, played `round(3600 / seconds)` loops an hour so the clock's wrap has no jump; the bend is `amp * (dir *
  (0.4 + 0.8 m.x) + cross * 0.8 m.y)`. Without `[sway]`, or with its map refused (said on stderr), the two sines of
  ADR 0091. A shot holds the wind at 0 [0157] (also sky-weather-and-water.md, studio-and-art.md).
- The `plant_sway` gate reads the body as `(c.info >> 8) & 255`, since `frame.rs` packs log2(lattice) << 16 and the
  shell << 18 into `info` [0157].
- Live: the effects watcher watches `data/maps/`, the effects file and the palette; the sway has a watcher of its
  own (the GPU's). A field is worked out again only when its map's text changed; the new wind map is uploaded and
  the terrain's bind group made again on the frame it arrives. Both watchers' first loads are waited for at
  loading; a map saved refused in `data/maps/` is named on stderr and cannot stop the game starting [0157].
- Shipped: `[sway]` names `wind gusts`; the `smoke` row names `smoke swirl` (strength 0.8, scale 4 m) [0157].
- The sway's 3D texture is 1 MB for `wind gusts` (128 x 128 x 32, two bytes a texel); a 512 px map of 64 frames
  would be 32 MB of VRAM [0157].
- Leaves (`sway` in `shader_vertex.wgsl`) keep their own motion. No Studio UI yet for a row's `map` and `[sway]`:
  written by hand; the Effects page keeps them on Save [0157].
- Presentation only: nothing on the server, the wire or a save changes [0157].

### A link's limits and its ping [0168, 2026-11-19]
From the audit of 2026-11-19 (a sleeping laptop stayed online for ever; one client could fill the server's memory).
- **Ping:** a peer with nothing to send for 5 s sends an empty frame (length 0); the reader skips it. It lives on
  the socket only: never reaches the tick, never recorded. A link with nothing heard for 30 s is closed (std read
  and write timeouts, the same on Windows and Linux; TCP keepalive would need a new dependency and its timings
  are the OS's). Both directions, so a client notices a dead server too.
- **Take-over:** a `Hello` whose name belongs to a client the server took nothing from for 100 ticks (5 s) takes
  that player over, place and slots kept, instead of becoming `name-2`. Silence is counted in ticks, so a replay
  decides the same. Names are not checked, so a frozen client's name can be taken while its link is open.
- **Bounds** (`net::Limits`): inbound 4096 messages waiting (twice that through a map download), outbound 128 MiB
  waiting; past either the link is closed with a log line. A join's backlog is two maps (about 23 and 21 MB) plus
  12 bytes an edit. The server sends each map hash once a connection.
- **Client and bot** drop a server message naming a cell or block id the world lacks (`net::fits`).
- Wire: an old server cannot decode the empty frame and drops a new client idle 5 s or downloading a map, so
  server and client are deployed together. A recording made before this that holds a same-name `Hello` after 100
  silent ticks now replays as a take-over.
- Open: the Linux side of the timeouts and a join over Tailscale against the bounds are not run; what a survival
  player may do (move, creative, dig size, names) is the developer's call.

## Platforms, hosting and the home server

- Targets: Windows x86_64 client; Linux x86_64 dedicated server [0005].
- Server: the same Rust code built without a window, in a Docker container on the Unraid box, reached over
  Tailscale; deterministic from its seed, so only changes cross the wire [0005].
- Updates: the client updates itself before joining. Implemented as the launcher's update (below), not an updater
  inside the game [0005, amended 0130]. Nothing yet refuses a client older than the server except the registry hash
  check [0130].
- Option chosen for the page: a small wrapper as the container's command. `deploy/Dockerfile` builds
  `technoape-server` from the committed source on the box itself (release, cargo caches kept between builds) and
  runs it under `deploy/web.py` (Python standard library), host network, world and bake in the `/data` volume. The
  server's code does not change [0130].
- The page on port 7780 shows the last 5000 lines of output, the 5 s stats line apart as a status, and hands out
  `/data/client/TechnoApe.zip`: the client, `data/` and a `Join Tower.bat` that connects to the server's Tailscale
  address. The log is in memory: a restart empties the page (`docker logs` has the same lines) [0130]. The page is
  `http://100.124.84.125:7780` (CLAUDE.md).
- `bash tools/deploy.sh [all|server|client]` does it; `all` keeps client and server on one commit. `python -I
  deploy/web_test.py` checks the page [0130].
- The friends' client is its own build (`target/friends`, `play` profile): `-C target-cpu=x86-64-v3` (any CPU with
  AVX2, 2013 on) and `-C target-feature=+crt-static`, so nothing is installed first. `build/TechnoApe.exe` stays
  this PC's own [0130; checked in `tools/deploy.sh`].
- The client updates itself in its launcher: `Join Tower.bat` compares the page's `version.txt` with its own and,
  when they differ, takes the zip and unpacks it over its folder (Windows' own curl and tar) before it joins. The
  game has no other way to join a server than the launcher's `connect=` yet [0130].
- The container carries Unraid's WebUI label, so its menu opens the page [0130].
- A clean stop: the wrapper writes `stop` on the server's stdin on `docker stop` (30 s allowed); no signal crate
  [0130].
- The client needs a GPU with hardware ray queries on Vulkan (ADR 0022); no build setting widens that [0130].
- No sign-in: whoever reaches the box on the tailnet or the LAN reads the log and downloads the build [0130].
- The dedicated server prints a JSON stats line every 5 s [CLAUDE.md, 0130].

## Open

- `Body::col_caves` cost at real cave counts: bucket caves by face and tile and cache spans per column; sea over open
  caves (the top gap starts below `gen_floor`, where `Solid::generated` starts the generated sea) needs its rule
  and the water's ledger settled [0062].
- A placement flake from ADR 0019: about 1 run in 4 the played run shows corrections of 0.1-0.7 m right after a
  placement. A likely cause: the client applies its own placement only when the server's echo returns (against ADR
  0008 and 0015), so replayed steps meet a different world than the server's. The fix starts with a failing test
  (a bot that places a block under its own feet and steps in the same tick, expecting 0 corrections) [0019; not
  checked whether since fixed].
- Chip heaps: longer lasting heaps (until swept, or settling into a block of loose dirt) would be the material fine
  cells' job; chips stay within the 1 m column they landed in [0078].
- Snapshot cost: the save snapshot copies every edit on the tick thread (tens of ms at a million edits every 30 s);
  regions or a database when a world outgrows one file [0016].
- Predictor `done`'s check whether a newer edit covers a cell is a linear scan; make it a count per cell if a
  profile shows it [0015].
- Switch to QUIC (and chunked join sync) when playtests show TCP's cost [0006].
- A server older or newer than the client: nothing refuses a client older than the server beyond the registry hash
  check; the launcher's update is the only guard [0130].

## No longer holds

- 0005's "CI builds the client and the server image; the server publishes a version manifest; the client checks it
  at start": replaced by 0130 (the box builds from committed source; a page `version.txt`; the launcher updates).
- 0006's `ToServer`: Hello, Move, Edit and `ToClient`: Welcome (seed and planet sizes), Edits, Players as the whole
  message set: both enums are much larger now (`net.rs`); `Welcome` carries `bodies`, hashes and `gen`.
- 0006's "positions are client-reported until gravity and collision move to the server": replaced by 0019 (the
  server steps the numbered inputs and answers `You`); `Move` remains for creative fly and teleport.
- 0006's "Welcome (seed and planet sizes)": replaced by the `bodies` list and `gen` of the current `Welcome`.
- 0013's `id: u8` and `MATERIALS = 4`-style limits: replaced by 0062 (u16, registry limit 255 only for the shader).
- 0013's "built into the binary with `include_str!`" as the only source: amended by 0134 (the files are `Live`, read
  from `data/` at start and watched).
- 0013's "the mesher draws every non-air block as an opaque cube": later blocks carry `see`, `joins`, `wood`, `falls`
  and models (0134 names the tables).
- 0015's "Movement prediction is unchanged (snap over 1 m, ADR 0007)": replaced by 0019.
- 0016's save fields (a version, seed and planet sizes, block table, edits, players): amended by the versions above;
  the planet sizes are now the `bodies` list at version 22.
- 0016's "Ctrl-C loses up to 30 s for now": replaced by 0130's `stop` line (a shutdown command).
- 0019's `Steps` as the only walk message: `Sprints` and `Sprint` added by ADR 0118; `Steps` and `Input` remain.
- 0019's "Other players are drawn at their last received position" is still so until interpolation is built (kept
  above).
- 0062's "Saves and the wire keep their version": true at the time; the save version is 22 now (above).
- 0062's "the registry still refuses an id over 255" (the shader's `q.y & 255u`, a 256-entry looks buffer, the
  `shortcut:` in `blocks.rs`): replaced by "Block ids past 255" (12 bits, 4095 the top).
- 0078's `break` lives 1.4 to 2.2 s: now 2.5 to 4 s.
- 0134's "Not measured: speed", and its first-commit suite counts: measurements, not decisions.
- 0157's "No Studio UI yet": still so; the Studio's Maps page (ADR 0153) makes the maps only.
