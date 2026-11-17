# draft – The journal's data, state, message and save (journal step 2)

- **Status:** proposed (on a branch)
- **Date:** 2026-11-17

## Context
`docs/design/journal.md`, step 2 of its order of building: the entries as data, each ape's state per entry
(ADR 0047: each ape its own), the message to its client, the save, and the two triggers that need no process
runner, `look:` and `have:`. The book is step 3; `do:`, `see:` and `need:` come with the process runner (step
4); milestones are step 5. The entry format on disk (`data/journal.toml`, the save) and on the wire
(`ToClient::Journal`) is hard to undo, so it is stated here.

## Decision
- **`data/journal.toml`**, a row an entry, built into the binary (`engine::journal::JOURNAL`), as the design's
  example: `name`, `age`, `title`, `question`, `text`, `noticed_by`, `known_by`, `points_at`. Checked at load,
  each failure naming the entry: names unique; `age` one of progression's ages (steam, lv, mv, hv, ev, iv,
  exotic; the design's example said "stone", which is no age the progression has); a title, question and text;
  at least one `known_by` (until milestones bring `measure`); every `points_at` another entry; every trigger
  of a kind built so far and naming what exists:
  - `look:block:<block>` (a block of `data/blocks.toml`; the middle word leaves room for `look:thing:` and
    `look:pile:`);
  - `have:<item>` (a block or item of `data/items.toml`, by its name as written).
  - `do:`, `see:` and `need:` are refused with "comes with the process runner (step 4)", and any other kind.
  The six entries are drafts (flint, hand axe, plant fibre, clay, copper, crucible), marked so in the header.
- **Three states** (`journal::State`): unknown, noticed, known; on the wire and on disk as 0, 1, 2 (pinned).
  A trigger in `noticed_by` raises an entry to noticed, one in `known_by` to known; nothing lowers one but the
  command. An entry that becomes known notices every unknown entry it points at. A table from trigger to rows
  (`look` by block id, `have` by item id) is built at load, as the design asks.
- **Who holds it:** each client a `Vec<State>`, one per entry in the file's order. Not run for a creative ape
  (its slots are a builder's, so `have:` would teach it everything), a dead one, or one before its `Hello`.
- **`have:`** is judged on the server at the end of the tick, after the events, when the slots changed this
  tick (`inv_dirty`, the same flag that sends `Inventory`): every stack in the slots, the crafting grid's
  cells too, raises its rows. Every way an item arrives (a pick-up, a dig, a craft, a take from a pile or a
  furnace, `/give`, a save loaded) sets that flag, so none is missed and nothing is hooked into each.
- **`look:`**: the server runs `edit::aim` from the player's eye (feet + up x 1.6) along the look it holds
  (the body's `fwd` turned by `Player::pitch`, from `Move`, `Input` and `Steps`), 8 m as a client aims, every
  10 ticks (twice a second), and raises the rows of the block it meets. That look lags the client's by the
  wire (ADR 0144); for noticing that is no matter, since a question that comes 50 ms late is the same
  question. Bots set it with the new `look dx dy dz`. *shortcut:* blocks only; things and piles under the
  crosshair need the client's models (`clutter::aim_thing`) and wait for a `look:thing:` row.
- **`ToClient::Journal(Vec<(String, State)>)`**, appended as variant 23, pinned: `[23, 1, 5, "flint", 2]`.
  By name, not index, so a client of another data file never reads one entry as another. Once the player has
  its name, every entry not unknown in one message (empty for a new ape), then at the end of each tick the
  entries that changed, by a diff against what it was last told (as `Marks`), so a command, a trigger and a
  save's restore all reach the client the same way. Its owner only.
- **Save 19:** each saved player gains `journal: Vec<(String, State)>`, the entries not unknown by name in the
  file's order. An 18 or older loads with every entry unknown (the 18 path is a new reader; the 17, 16 and 12
  paths gain an empty journal). An entry the file no longer has is forgotten at load, with a line on stderr,
  rather than refusing the save: the entries are drafts and will be renamed.
- **State hash:** a client's journal is hashed when any entry is not unknown, and an absent player's saved
  bytes leave out an empty journal's byte, so a recording without the journal keeps its hash.
- **Client:** no book; the chat shows "Journal: <question>" for an entry noticed and "Journal: learned
  <title>" for one known, nothing for the first message (the journal it joined with). `/journal` lists the
  entries and `/journal <entry> unknown|noticed|known` sets one, for trying it.

## Measured
- `the_journal_costs_little_a_player` (play profile): 6.7 us a player on its dearest tick, a look tick with
  its slots changed; 50 survival apes, 0.33 ms on one tick in ten, so about 0.03 ms a tick on average.
  `run=sim`'s bots are creative, so the sim benchmark does not run it.
- Soak, seed 1, 50 bots, 1 minute: hash eaa74af9ae874e2f, the same as before the journal (no soak bot met a
  trigger, so every journal stayed empty and is left out of the hash). The recorded soak (warp 32) and its
  replay: 60 checks, no divergence. That the journal is in the hash is shown by the server test, not the soak.

## Options considered
- **Entries on the wire by index** (a u16 into the file): smaller; but a client of another build reads the
  wrong entry and nothing in `Welcome` hashes the file. By name chosen: a change is a few a session.
- **`have:` by hooking each place an item arrives**: exact "came into its slots", but six places and every new
  one to remember. Judging the slots when they changed is the same answer for a deterministic server.
- **`look:` from a client message** (`ToServer::Look`, the crosshair's cell): no lag, the client's exact aim
  with things; but one more message to trust and to record, and bots would need it too. The server's own look
  chosen; revisit when `look:thing:` is needed.
- **Every tick for `look:`**: 20 rays a second a player for a question that does not hurry. 10 ticks chosen.
- **Refusing a save that names a lost entry** (as items do): the save would not load after a draft is renamed.

## Consequences
- The journal works for blocks and items before any process exists; the starter's entries (step 4) replace
  the drafts and add `do:` and `see:` through the event list.
- `/journal` is open to every player, like `/thirst`; it goes to the operator role with them.
- Not live: `data/journal.toml` is read once (recipes are not live either); a reload would need each ape's
  states kept by name and the clients told.
- Instincts (journal.md: drink, eat, pick up, dig, strike two stones known at the start) are not built: no
  entries for them yet.
- The design's "the crosshair rested on it" is read as "is on it at a half-second check"; a rest of some
  time is a taste call.
