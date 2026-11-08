# 0165: World sync: block entities, live fire, world ops

Status: accepted (2026-11-08)

## Context
The first multiplayer playtest (2026-11-08, bug reports 15-59-33, 16-03-27, 16-14-20) found that stations
and laid things did not reach the other player, fire did not either, and the Gelvin monolith was gone after
a rejoin. ADR 0145 shares only voxel edits made inside `Net.record`, and three gaps followed from that:
- **Right clicks were never recorded.** Placing a station, laying an item on the ground, fuelling and
  lighting a firepit and flint and steel all run from `Player._use`, outside any `Net.record`, so their voxels
  stayed local. The block entity itself (kind, state, contents) has no channel at all, and a client keeps no
  block save (`persist_world` is off), so a rejoin lost every station too.
- **Fire** is a sim material written by `Weather.ignite`. Journaling it would light the flame again at every
  replay (a late join, a regenerated column).
- **The monolith** is raised by each client from the seed, outside the journal, and is saved as "up" in the
  player save. Terrain a client regenerates (a rejoin, or walking 72 m away and back) came back without it.
  At the next join it was never raised again.

"Simulation updates slowly" was not measured before this change. The proof now measures it (below).

## Decision
- **A right click and a hold are player actions:** `Player._use` and the hold's `hold_done` run inside
  `Net.record`. `Net.record` nests and passes back the callback's result.
- **Block entities go as their saved dict** (`to_dict`, the same one the single-player save writes).
  `BlockEntities` touches an entity's anchor when it is registered, removed or changed while recording, and
  the inventory screen touches the container it closes. At the end of the frame `Net` asks
  `Main.net_block_out(p)` and sends `{k:"block", pos, d}` (d = {} when removed) in the frame's batch. Signs,
  notes and trading posts stay on the PostLedger (ADR 0164). The server keeps the newest dict per anchor
  (`_keep_block`: a Vector3i pos, a Dictionary under 64 KB, the key written into the dict), saves it as
  `net_<seed>_g<gen>_blocks.json` and sends the whole registry to a joining player after the items.
  `Main.net_block` replaces or removes the entity with `BlockEntities.load_one` (the code path `load_dict`
  uses) and redraws its view. Ticking (a firepit burning down, a furnace smelting) runs on each client from
  the same start; the next player action on the entity sends its state again, and the last one sent wins.
- **Live ops:** `Net.send_live(op)` marks an op `live`. The server forwards it and does not journal it, and
  clients apply it without indexing it. `Weather.ignite` writes its flame with recording off and sends it live.
  A one-cell `voxels` op is applied through `SimBubble.set_voxel` (the sim's own write, which wakes the cell).
- **World ops:** `Net.add_world_ops` puts edits that every client makes from the seed at the front of the
  client's journal index (`_base_n` of them). Main adds the monolith's boxes at setup, so the column replay
  rebuilds it under the players' own edits whenever its terrain regenerates.
- **Delay:** every batch event carries `ts` (unix ms) and the receiver keeps `lat_ms_max`, `lat_ms_sum`
  and `lat_n`. Copies in the server's item registry drop `ts`, so a late joiner's old items do not count as
  delay.

`tools/net_proof.sh` proves it: alice runs `/ignite` and places a firepit with `/use`. Bob gets the flame
into his sim (`live_ok`) and the firepit (`blocks_recv`) with a mean op delay under 100 ms. Carol (a late
join) gets the firepit from the registry, and alice's rejoin gets the monolith from the world ops
(`base_replayed`). Measured 2026-11-08 on the dev box: bob's delay was 15 ms on average (73 ms over 5
events), 18 ms at most; the monolith is 625 world ops, of which 323 to 348 replay per client.

## Direction: single-player on the same logic
The developer wants single-player to run the logic LAN runs, as Minecraft's integrated server does, so that
what works offline works online. Today the reverse holds: everything runs on the client and only what a hook
sends is shared, so a feature that works offline can still miss its hook. Getting there takes three steps:
1. (this ADR) Hooks sit in the shared code paths (`Player._use`, `BlockEntities`, `Weather.ignite`) and run
   the same way offline (`Net.active` false: one boolean each).
2. A loopback server in the client process: single-player starts `Net` as a server on localhost and joins it,
   so every edit, item and block entity travels the same path. Its save would be the journal plus the block
   registry instead of the SQLite terrain stream, which needs the journal compaction ADR 0145 defers, plus a
   save migration.
3. Server-side simulation (ADR 0145's "additive" step): the server needs the voxel and technosim extensions
   and a bubble per player. Fire, water and machines then run once, on the server, the same way for
   single-player and LAN. Only this step removes divergence between clients.
Steps 2 and 3 are their own jobs and change the save format and the server image.

## Consequences
- What diverges now is only ticking state between two actions and the sim's spread: a fire spreads
  separately on each client, and what it burns is not journaled.
- A world system that writes voxels from the seed (like the monolith) belongs in `add_world_ops`.
- Two proofs on one machine need different ports (`bash tools/net_proof.sh <seed> <port>`). On the
  default port, another session's client joined this proof's server and its edits were lost.
