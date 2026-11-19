# 0134 – Blocks, items and creatures reload in a running game

- **Status:** accepted (the developer, 2026-11-15: "can't we have something like NBT data ... and reload all the
  properties everywhere instantly?", then "Do the first 2")
- **Date:** 2026-11-15

## Context
Textures, models, sounds and effects reload within a second of a save (ADR 0045, 0047, 0048, 0050). The rows
that say what a block, an item or a creature *is* did not: `data/blocks.toml` and `data/items.toml` were compiled
in (`BLOCKS`, `ITEMS`), the creatures' speeds and the herds too, so a changed hardness, light or model needed a
rebuild. The developer asked for Minecraft's NBT; what "reload everywhere at once" needs is the other half of
that: the table of kinds made live. Data per instance (a chest's contents) is another feature, not built here.

Asked for: the first two of four classes of property.
1. **Read when used** (hardness, tool, drops, stack, a creature's speeds): in force at the next read.
2. **Baked into meshes or light** (a block's model, see-through or solid, light, colours, pattern): derived again.
3. Baked into the world (biomes, ore, where plants grow, the ground's layers): not here.
4. Identity (taking rows away, reordering): refused, see below.

## Options considered
1. A lock around the tables (`RwLock<Arc<Registry>>`): a lock on 350 call sites, some in the mesher's and the
   physics' inner loops.
2. **A pointer swapped whole** (`live::Live<T>`): a reader pays one atomic load, as `LazyLock` did, and keeps
   the reference it took; the replaced table is never freed.
3. Passing the tables down as arguments: every signature in the engine.

## Decision
- **`engine::live`:** `Live<T>` is made on first use and replaced whole by `set`. `BLOCKS` and `ITEMS` are
  `Live`; every call site is as it was. `live::apply(blocks, items)` parses both, puts them in force and counts
  `live::rev()`; the mesher's five bit tables are made again there.
- **Refused, by name, nothing changed:** a file that does not parse; a block gone or renamed (a world holds it
  by id, the code asks for some by name); an item or a creature gone, renamed or moved (their ids are their
  rows' places). Rows are changed in place and added at the end.
- **The server** (the game's own, local or dedicated; never a soak, a replay or a test: `Server::live_data`)
  looks at `data/blocks.toml`, `items.toml`, `creatures.toml` and `herds.toml` once a second (four stats). A
  changed file is put in force, `World::refresh_defs` derives the lights and models of placed blocks again,
  `Herds::reload` takes the speeds and shares and keeps the animals alive, and `ToClient::Defs` carries the two
  texts to every client; a client that joins later gets them before its `Welcome`, whose hashes then agree.
- **The client** follows `live::rev()`, whoever applied the rows (the server's thread in single player, the
  message otherwise): `Streamer::set_defs` gives every slot a new version, so every chunk is meshed again while
  the old mesh stays drawn; its own world's lights and models are derived again; its watchers (`gpu::watch`)
  count the rev among what they watch, so the looks table, the models and the effects are made again.
- **At its start** a game's server reads `data/` too: a build no longer has to be newer than its data.

## Tests and evidence
- Red first, `crates/engine/tests/live_defs.rs` (a process of its own: it swaps what every other test reads):
  it did not compile before `live`, then failed on an item renamed in place (taken, ids are places), on the
  mesher's tables (16 against 17 blocks seen through: stale) and passes: a changed hardness at the next read, a
  placed lamp's light after `refresh_defs`, the mesher's tables, a new row, and each refusal leaving the tables
  as they were.
- `live::tests::a_live_value_is_made_once_and_replaced_whole`,
  `herds::tests::a_reload_changes_the_speeds_and_keeps_the_animals`.
- `python -I tools/live-check.py`: a play run off-screen, `data/blocks.toml` changed under it: the server had it
  in force 0.21 s after the save, the client's looks were made again 1.0 s after (the watchers' second).
- Filmed once by hand (a stand run, a lamp on a stone floor from above): the lamp's light turns from warm to
  blue between two frames with no restart.
- Suites: see the commit; the first commit's were engine 354, client 175, server 83, bot 8, all passing.

## Review (a fresh reader of the diff)
Found and fixed: the mesher's `SEE`, `GLASS`, `JOINS`, `WOOD`, `FALLS` tables were made once, so a row whose
`see`, `joins`, `wood` or `falls` changed was meshed as before; a renamed block passed and `BLOCKS.id("stone")`
would then panic on 180 call sites; the title menu's join took `Defs` for a missing `Welcome`; every server
watched `data/`, so a soak or a replay could run on other rows than the build's. Checked and left: `Live`'s
pointers (all leaked, the loser of a first-use race is never handed out); between the blocks' swap and the
items' a reader sees new blocks with old items, which hold ids that cannot go.

## Shortcuts and limits
- `shortcut:` a replaced table is never freed (about 100 kB a reload).
- `shortcut:` the server reads and parses on its tick thread (about a millisecond); the client walks every edit
  on the frame thread once a reload (about a millisecond per 50,000 edits).
- A file refused, or caught half-written, is looked at again only when its time or size changes again.
- A recording keeps no reload: a session that reloaded replays with the build's rows.
- Class 3 stays baked: a changed biome, ore or plant rule needs new terrain, and tables that name blocks
  (`biomes`, `clutter`, `bodies`) keep the ids they found, which holds because rows keep their ids and names.
- A creature's look (scale, clips) was live on the client already (`data/creatures.toml` is among the models'
  watched files); its speeds are the server's.
- Not measured: speed (a game was running on the PC). The hot path is unchanged in kind: one atomic load a read.
- Not filmed: an unpainted block's colour changing (the log shows the looks made again).
