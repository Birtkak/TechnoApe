# 0169: The server owns the edited chunks

Status: accepted (2026-11-08)

## Context
Under ADR 0145 and 0165 the server keeps only the edit journal, and each client rebuilds the world from the
seed plus the journal, inside its 64 m sim bubble. Terrain between the bubble and the 72 m view distance is
drawn straight from the generator, so a reload there came back without anyone's edits. Measured 2026-11-08 at
the default view (seed 7337, `/monolith` after a 2000-voxel trip and back): the multiplayer client found 1 of
625 monolith boxes in place, the single-player run 580 (its SQLite save holds the edited blocks). That is
bug report 2026-11-08_16-03-27, "the monolith disappeared", and it would have hit every built thing seen
from a distance. The developer asked for the Minecraft model: the server stores the world and the clients
load it from there, so what is built is wherever you stand and whoever built it.

Sending generated terrain is out: a 72 m box of 12.5 cm voxels is ~370 000 chunks (hundreds of MB), and
the server image runs the stock Linux Godot without the voxel extension. Generated terrain is also
reproducible from the seed. What has to travel is what differs from the generator: the edited chunks, the
same set the single-player SQLite stream saves.

## Decision
- **The server stores every edited chunk** as the client saves it: `{k:"chunk", p: Vector3i, b: bytes, n}`
  in the frame's batch, `b` the chunk serialized by `VoxelBlockSerializer` (LZ4), `n` the number of journal
  ops the saving client had, which the snapshot bakes in. The newest `n` wins (`_keep_chunk`, 64 KB at most).
  Saved as `net_<seed>_g<gen>.chunks` (zstd `store_var`) next to the journal. The server needs no voxel
  code: it keeps bytes.
- **The client's terrain stream is the server's copy.** `NetStream` (`scripts/net/net_stream.gd`, a
  `VoxelStreamScript`) mirrors the store: a joining player gets every chunk before the journal, later
  snapshots arrive live, and `VoxelLodTerrain` asks it before generating a block.
- **The sim is the only source of snapshots.** The terrain's own save (`_save_voxel_block`) is ignored: a
  terrain block can unload before the bubble's budgeted pushes reached it, and a snapshot of it would claim
  ops it does not hold. Instead `SimBubble` serializes a chunk from its own cells (`_snapshot`,
  `Net.snapshot_chunk`) when the chunk leaves its window and, for every chunk the sim changed since, at
  `flush_all` (the 30 s save, a quit). Those go out in the next batch (`flush_chunks`, and `flush_chunks(true)`
  at the end of `save_terrain` so a quit's chunks are on the wire first). Edits outside the bubble and the
  simulation's own results (water that flowed, what fire burnt) persist with them.
- **A chunk arriving live** is pasted into the terrain when it is drawn here and the sim does not hold it
  (the gap above); when the sim holds it, the op already landed there and the sim's own snapshot follows.
- **The replay skips what the snapshot holds:** `replay_column` applies an op only if some chunk of it in
  that column has no snapshot or one older than the op. World ops (the monolith) are baked into any snapshot
  of their chunk. (The first cut compared against -1 for "no snapshot" and skipped every world op: the
  monolith went from 321 boxes to 5 before the proof caught it.)
- The wire changed (`_c_chunks`, the chunk event): `Net.PROTO` is 7.

`bash tools/reload_proof.sh` (24 m view) and `VIEW=72 bash tools/reload_proof.sh` (the default view) prove it:
both clients fly 250 m away and back and must find the bricks box, the monolith boxes (none lost) and the
block entities, and must have sent and received chunks. Measured 2026-11-08 (seeds 7319 and 7331):
- 24 m: monolith boxes in place 389 -> 389 on both clients (the single-player SQLite run 321 -> 321: the
  snapshots also carry what the sim did after the raise); bricks 27 / 0 away / 27; 17 block entities both
  times; alice sent 573 chunks and received 442, bob 442 and 610; 1707 and 825 ops skipped as baked in.
- 72 m: in place 495 -> 575 (alice: the mirror filled in boxes her own bubble never reached) and 575 -> 575
  (bob); from 250 m away the ground shell (ADR 0124) shows 575 of 614 boxes, where the old code showed
  1 of 625 (generated ground): the shell loads through the mirror too. 851 / 587 chunks sent.

## Consequences
- Single-player is unchanged (its SQLite stream). The direction of ADR 0165 (single-player on the
  multiplayer logic) now has the server-side store it needs; a loopback server could use `NetStream` too.
- The journal still grows; it can now be compacted by dropping ops older than every snapshot of their
  chunks (reindexing `n`), which ADR 0145 deferred. Not done yet.
- What still diverges: an op on a chunk nobody has in a bubble, and that no client saves afterwards, lives
  in the journal only until a bubble passes over it; terrain outside a bubble shows the last snapshot, not
  the ops since. Both are the old gap narrowed to seconds.
- The ground truth for a chunk is the last saver's copy: two bubbles simulating the same chunk (two
  players standing together) save alternately, and the later save wins. The sim is the same on both, so
  they agree except for in-flight water and fire.
