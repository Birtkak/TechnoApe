# 0014 – Edits as boxes, whole or nothing; survival counts blocks on the server

- **Status:** accepted
- **Date:** 2026-11-10

## Context
ADR 0008: tools dig and place a 1 m brush (2 x 2 x 2 blocks) by default, a held key gives single blocks, and
building drags out lines, walls and floors. On the wire, an edit is a shape the server validates whole. The
first playable has a survival inventory and a creative toggle. Until now the wire carried one block per edit,
and nothing counted blocks.

## Options considered
- **Shape:** a list of cells, a set of kinds (brush, line, wall, floor), or one box of cells on one face.
  Every tool in ADR 0008 is a box: the brush is 2 x 2 x 2, a block is 1 x 1 x 1, a line is n x 1 x 1, and a
  wall or floor is one block thick. A box is 26 bytes on the wire and needs one check.
- **Validation:** drop the cells that fail, or refuse the whole shape. Whole or nothing gives the client's
  prediction one answer per edit to confirm or revert.

## Decision
- `engine::edit::Shape { body, face, min, max }`, with sides of at most 32 blocks (the 16 m reach) and at
  most 2048 cells. A shape stays on one face. A drag across a seam ends at the seam
  (`Shape::between` returns `None`).
- `edit::changes` is the one rule, shared by the server and the client's prediction:
  - Digging clears every non-air cell.
  - Placing fills every air cell, except the cells a player stands in.
- `ToServer::Edit { seq, shape, material }`. The server refuses the whole edit unless every cell is on the
  body, the block is registered and every cell is within reach. It answers `Done { seq, ok }` after the tick's
  `Edits`.
- **Survival:** digging adds each dug block to the player's inventory (a count per block id). Placing costs
  one per cell filled. An edit the inventory cannot pay for in full is refused.
- **Creative:** counts nothing. `ToServer::Creative(bool)` switches modes. Anyone may switch for now: an
  operator role comes when servers go public.
- The server sends `Inventory { creative, counts }` to the owner on join and whenever it changes. The state
  hash covers inventories and modes, so replays check them too.
- Players start in survival with nothing. Bots and the client's benchmark runs switch to creative.

## Consequences
- The soak's bots dig and place blocks, brushes, walls and floors (some over the limits or out of reach) and
  switch modes at random. Its seed-1 10-minute run takes 36 s instead of 16 s, because walls and floors
  leave far more edited cells for 51 world copies to mirror and hash.
- Recordings from before this change no longer decode (the `Edit` message changed shape).
- A wall dragged through a player leaves that player's cells open, and the client predicts the same.
