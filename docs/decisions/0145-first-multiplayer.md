# 0145 – First playable multiplayer: a thin server that owns the edit journal

- **Status:** accepted
- **Date:** 2026-11-07 (the developer: "multiplayer tackled"; the server is a Docker on the Unraid box
  over Tailscale like the other game's; updates come through it later; offline play stays)

## Context
Single-player only so far; no networking code. The vision (ADR 0002) wants a dedicated server,
server-authoritative simulation, deltas over the wire, deterministic worldgen. Two facts shape the first
slice:
- Every voxel change is already a **replayable intent** against a deterministic world: a brush job
  (`ShapeTool.fill`: cells + material + mask), a sphere (`SimBubble.blast`), a box (`fill_box`), single
  voxels (`set_voxel`). Replaying the same intent on another client with the same seed gives the same
  cells, as long as the ops land in the same order.
- The simulation runs in a native kernel, so a server that simulates needs godot_voxel and technosim
  built for Linux before it can run in Docker. A server that only relays and records needs nothing native.

## Decision
**Transport:** Godot high-level multiplayer over `ENetMultiplayerPeer` (UDP, port 7777). RPCs live on
one autoload, `Net` (`scripts/net/net.gd`), so node paths match on every side. A protocol number is
checked at hello. Over Tailscale the address is the tailnet IP or MagicDNS name; nothing is exposed
publicly.

**What the server owns (`server=1`, headless, no world loaded):**
- the seed and game mode (`seed=`, `gamemode=` on its command line);
- the **edit journal**: every op in arrival order, `user://worlds/net_<seed>_g<gen>.ops`
  (zstd, `store_var`), written every 30 s and on quit; this is the world's save;
- the **player saves** by name, `user://worlds/net_<seed>_g<gen>_players.json` (the same dictionary
  single-player writes, sent by the client every 30 s and on quit);
- the peer list and the position relay.

**What clients do:** generate the world from the seed, run their own SimBubble, apply their own edits at
once (no round trip, no rollback), send each op to the server, apply every op the server forwards, and
keep the whole journal (welcome + live ops) indexed by chunk: when the bubble loads a chunk column
from the terrain (`SimBubble._load_pending`), the ops touching those chunks are replayed. That one
mechanism covers late join, edits made far away while connected, and terrain that streamed out and
regenerated. Ops are applied through the same functions that made them (`ShapeTool.fill`, `blast`,
`fill_box`, `set_voxel`), with `Net.applying` set so they are not sent again.

**Op format** (a Dictionary, `var_to_bytes` on the wire, one RPC each; single voxels are batched per
frame into one `voxels` op):
- `{k:"fill", lo:Vector3i, cells:PackedInt32Array (relative x,y,z triples), m:int, mask:PackedByteArray}`
- `{k:"voxels", cells:PackedInt32Array (world triples), mats:PackedByteArray}`
- `{k:"box", lo:Vector3i, hi:Vector3i, m:int}`, `{k:"sphere", c:Vector3, r:float, m:int, only:PackedInt32Array}`
Each op carries `box:[lo, hi]` (world voxels, inclusive) for the chunk index. `NetOps` (`scripts/net/net_ops.gd`)
encodes, decodes and indexes them; `test_net_ops.gd` covers the round trip.

Everything a client sends in a frame (ops, item events) goes as **one reliable batch**: the first proof run
sent the monolith's ~330 box fills as separate RPCs in one frame and the server dropped that client.

**Dropped items (shared items):** an item spawned by a player action (`Net.recording`: a dig's drops, a
throw, `/drop`) gets a uid (peer id, counter) and goes to everyone as `{k:"item", uid, stack, pos, vel}`;
every client runs the same `ItemEntity` from the same start, so it lands about where the dropper's did,
and the server keeps a registry for late joiners (dropped where it was spawned) that expires at
`item_despawn_s`. A pickup is **arbitrated**: the client in range sends `pick(uid)` once, the server grants
it to the first asker and sends `picked(uid, by)` to all; the winner takes it into the inventory (what does
not fit stays as that player's own item), the others see it go. Shared items never merge. Items that
world systems spawn without a player action (farming, mobs, fires) stay local to each client and are
picked up as in single-player. Inventories ride in the player save (every 30 s and at quit).

**Players:** 20 Hz unreliable position + yaw + pitch + speed to the server, relayed to the others;
a `RemotePlayer` (an `ApeModel`) eases toward the last state (exponential, ~80 ms) and animates by the
sent speed.

**Join:** the title screen's Multiplayer page lists every server that answers a UDP probe on `port + 1`
(`ServerFinder`: localhost, the LAN broadcast and every online Tailscale peer from `tailscale status
--json`; a server answers with name, seed, mode, players and an id so one server reachable by several
routes is one row); one click joins. An address field and `connect=host[:port] name=<n>` on the command
line remain for anything the probe cannot reach. Tailscale carries no broadcasts, which is why the peer
list is asked for. The handshake happens before the game scene loads, so `main.gd` starts with the server's
seed, the saved player and the journal; `Settings.persist_world` is off on a client (the server holds
the save). Disconnect saves nothing new; the last 30 s save stands.

**Single-player:** unchanged. `Net.active` is false, the hooks are one boolean each, `Net` never
processes.

**Explicitly single-player for now** (each client runs its own copy, nothing crosses the wire):
simulated materials (water, sand, fire, gas, heat), block entities
(stations, chests, furniture, machines, networks), mobs, farming, weather, rigid pieces (a felled tree's
cut and settled voxels do replicate as voxel ops, the flight does not), survival state (saved per player),
the calendar. Op ordering: a client applies its own op before the server orders it, so two clients
editing the same cells at once can end in different states until they rejoin.

## Consequences
- The server is a Godot headless process of the same project; it loads the GDExtensions but never calls
  them. The deployment the developer wants (2026-11-07): a Docker container on the Unraid box like the
  other game's, reachable only over Tailscale (the box is not on the home network), and later the
  container also serves the client updates (ADR 0002's manifest) so a player who joins is brought up to
  date, while single-player keeps working offline with whatever build is installed. Linux build, Docker
  image, Tailscale service and auto-update are the next job; until the native libs are built for Linux
  the server runs on Windows.
- The journal grows for the life of a world (one brush op ≈ 1-4 KB). A compaction (fold ops into chunk
  snapshots) comes when a world's journal passes a few MB.
- Server-side simulation (the vision's authoritative sim) is additive: the server gets a bubble per
  player and sends dirty boxes; clients then stop simulating. Not needed to play together.
- Measured on one machine, two clients, one digging: see the report in `tools/net_proof.sh`'s JSON.
