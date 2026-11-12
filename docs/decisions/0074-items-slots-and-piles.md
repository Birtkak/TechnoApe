# 0074 – Items, slots and piles

- **Status:** accepted
- **Date:** 2026-11-12

## Context
The ape owned a count per block id with no limit (ADR 0014). `docs/design/items.md` asks for slots, stacks,
items that are not blocks, and things lying on the ground. The developer's calls are already made (ADR 0047,
`open-questions.md`): carrying is B (a hotbar of 9 and rows from packs worn), and a dug block falls to the
ground as a pile, as in Minecraft. Bug report `bugs/2026-11-12_20-58-44` asks for an inventory. This ADR is
items.md steps 1 and 2 and the move message of step 3; the screens are ADR 0075.

## Options considered
1. Keep the counts and add a cap per block. Small, but nothing but blocks could be carried, no packs, no piles.
2. **A list of slots for everything that holds things, one move message.** Chosen: items.md's one idea.
3. Piles as entities with a position and a velocity. Free to roll and float, but each one ticks. Rejected:
   items.md wants a pile nobody touches to cost nothing, so a pile belongs to an air cell.

## Decision
- **Items** (`engine::items`, `data/items.toml`): every block is an item of the same id; other items have ids
  from `0x8000` up in the file's order. A row has `name`, `stack` (1-64), `look` (the block it is drawn as) and
  `pack` (slots it opens when worn). The file starts with three packs: basket 9, hide pack 18, frame pack 27.
  Tests: names unique and not a block's, stack sizes in range, every look a block.
- **A stack** is (item, n, payload: u64); the payload means nothing yet. **A slot** is `Option<Stack>`.
- **The ape**: 37 slots. 0-8 the hotbar, 9 the worn pack, 10-36 the rows a pack opens (only as many as the
  pack holds are usable). Taking a pack off while its rows hold things is refused.
- **The rule** (`items::rule`): a move into an empty slot or onto the same item merges up to the stack size;
  onto another item, a whole stack swaps; anything else is refused. Every move is all or nothing.
- **One message**: `ToServer::Items { seq, from, to, n }`, `from` and `to` a (holder, slot) where a holder is
  the ape, a pile's cell, creative's grid (slot = item id; a stack put there is gone) or the ground ahead (a
  throw). The server checks reach (4 m to a pile), creative for the grid and the rule, and answers `Done`.
- **Piles**: an air cell's 4 slots, kept in a `BTreeMap` by (body, face, cell) so every visit has one order.
  A survival dig of a loose block lays its drop where the block was, on the first floor below; a hammer's
  shape gathers its drops into as few piles as hold them, in the cells nearest the ape. A pile falls when
  the floor under it is dug. Every tick each living player takes what fits from the piles within 1.5 m of
  its feet; a throw waits 2 s for its own thrower. **Death** lays all it carried at its feet on respawn.
  Nothing despawns.
- **Wire**: `Inventory { creative, slots, water }` to the owner; `Piles(Vec<(cell, slots)>)` with the piles
  that changed this tick to everyone (every pile on join; an empty list: gone). `Welcome` carries the items
  table's hash, so a client with other data is refused like one with other blocks.
- **Saves** (version 13): every player's slots and every pile, with the items table by name. A loaded save's
  ids are found by name, an unknown name refuses the save. A version 12 save's counts load into slots in
  block id order; what does not fit lies in piles at the player's feet.
- **The conservation law** (fundamentals 4.8): the server keeps, per item, what digs, places, `/give` and
  creative's grid made or took; `item_ledger()` lists every item whose count over players and piles
  differs. The soak fails on the first one. Its bots move, split, throw and take stacks at random, 1 % of
  their actions, some out of reach, some from an unknown item.
- **The state hash** covers every slot and every pile, so the soak's replay checks them.
- **Bots**: `slots` (a JSON line of what they hold), `move a b n`, `drop a n`, `take dx dy dz k a n`,
  `put a dx dy dz k n`, `grab <item> a n` (creative's grid).

## Consequences
- The gate's soak hash changes (new state hashed).
- **Left for later, each a `shortcut:` comment where the code meets it:** tools' wear and food's day in the
  payload (items.md steps 4 and 6), containers and machines as holders (step 5), vessels and the bucket as
  items (the bucket's water stays its own number), the planet's material ledger (`ore-budget.md` does not
  see carried or lying things), piles sent to every client instead of those near them, a pile that does
  not float or sink in water, and no right-click set-down (the screen's throw is the one way to lay a thing).
- Not built: the littered-face benchmark of 100,000 piles (items.md: "Tests"). Piles are a `BTreeMap` and
  the pick-up looks at the 27 cells around each player, so a pile nobody is near costs one map entry and no
  tick time; the client draws the piles within 64 m.
- Red then green: the stack-rule, move, pile and save tests were written beside the code, not seen failing
  first; the drop on death was. They were each run against hand-made cases whose answers were worked out
  before the run.

## Calls for the developer to confirm
- A stack is 64 for every block; packs stack 1.
- The packs' sizes (9, 18, 27) and what they look like (reed, mud, log) until they have models.
- The reach to a pile 4 m, pick-up within 1.5 m, a throw 2 m ahead that waits 2 s for its thrower.
- `/give <thing>` gives one stack by default, at most 36 stacks.
- Death drops everything at the spot, on respawn.
