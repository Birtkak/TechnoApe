# Items: what the ape carries, wears out and leaves lying

- **Status:** design proposal, 2026-11-11. It is the first thing `crafting.md` needs from the engine
  ("items and inventories beyond blocks") and the tools ADR 0034 left open ("the tool divides the time and
  decides the drop"). Lines marked *taste call* are the developer's; the rest are routine defaults, each
  with its ADR when built.
- **Today:** the inventory is a count per block id with no limit, plus the bucket's water
  (`ToClient::Inventory { counts, water }`, ADR 0014). Nothing but blocks can be owned, and nothing can
  lie on the ground.

## One idea: an inventory is a list of slots, and everything that holds things is one
The ape, a basket, a chest, a machine's input, and a heap of things on the ground are the same structure
with the same rules, the same message to move things and the same test. Only the number of slots and who
may open it differ.

| Holder | Slots | Lives in |
|---|---|---|
| the ape | the hotbar's 9, more with what it wears (below) | the player, in the save |
| a **pile** | up to 4 | a block cell that holds air: things lying on the ground |
| a container (basket, pot, chest, tank) | by its row of data | a block entity (`world-data.md`) |
| a machine | its ports' slots | the same block entity |

## What an item is
- **Every block kind is an item** with the same id, so nothing changes for what is dug and placed. Other
  items take ids above the block kinds, from two sources: material x form (`crafting.md` idea 2: a bronze
  gear is the bronze row and the gear row) and `data/items.toml` for the rest (food, tools, parts).
- Ids are 16 bits and given at load; the name-to-id table goes into saves and `Welcome`, as the blocks'
  does (ADR 0013). Adding an item never breaks a save.
- A **stack** is (item, count, payload). The payload is 8 bytes whose meaning the item's kind gives:

  | Kind | Payload | Do two stacks merge? |
  |---|---|---|
  | plain (blocks, ingots, parts) | none | yes |
  | tool | uses left | no, a stack of one |
  | food | the day it spoils, a whole game day | yes: the merged stack takes the earlier day |
  | vessel (pot, bucket, tank item) | a fluid and its amount in the liquid's units (ADR 0020) | only when empty |

  Food that merges to the earlier day loses a little and never gains: no stack of forty apples that will
  not join because they were picked a minute apart. Today's bucket becomes a vessel item, and
  `Inventory.water` goes.
- **Every item knows what it is made of** (elements, `crafting.md` idea 3), so what apes carry and what
  lies in piles and chests is part of the body's material ledger. Carried things count for the body the
  ape stands on; a trip to Luna moves them from one ledger to the other.

## Carrying (decided: B, ADR 0043)
The chore-to-factory rule covers hauling too: carried by hand, then a cart, a belt, a railway.

| Option | For | Against |
|---|---|---|
| A. 36 slots from the start, stacks of 64, as Minecraft | known to every player; no design risk | hauling is never a problem to solve, so carts and belts are only a convenience |
| **B. The hotbar's 9, and rows earned by what the ape wears** (*chosen*): a woven basket +9, a hide pack +18, a frame pack +27 | the first hour has a carrying problem a craft answers; the screen and the stack rules stay Minecraft's | nine slots can annoy in the first minutes: tune the basket to be a ten minute craft |
| C. Weight in kilograms from the materials' density | realistic; stone is heavy, feathers are not | a number to watch instead of a grid to see; every item needs a weight that feels right |

Stack size is a column of the item's row: 64 by default, 1 for tools and filled vessels, 16 for bulky
things. A block is a cubic metre, so 64 of them in a hand is not real under any option; it is Minecraft's
feel, which the developer chose for digging (ADR 0034).

## Tools
- A tool is an item with a **kind** (axe, pick, shovel, hammer, knife, later saw and wrench) and a
  **head material**. Its numbers come from the material's row, not from a list of tools:
  - **speed:** the dig time of ADR 0034 is `hardness / speed` when the tool's kind is the block's `tool`,
    and the bare-hand time otherwise;
  - **grade:** a block drops when it is `loose`, or when the right kind of tool has at least the block's
    `grade` (0 hands, 1 stone, 2 copper and bronze, 3 iron, 4 steel). Otherwise it breaks and gives
    nothing, as stone does today;
  - **uses:** from the material's strength. A dig costs one.
- A tool head is a **form**, so a new metal arrives with its pick, axe and shovel (one row, as its gear
  and wire). A head and a handle make the tool.
- **A worn-out tool leaves its head as scrap** of the same metal, to melt and cast again (*taste call*,
  against tools that vanish or never wear). The cost of wear is the trip back to the furnace, the ledger
  loses nothing, and recycling exists from the first copper axe.
- `data/blocks.toml` gains `tool` and `grade` beside `hardness` and `loose`. A data test: every block
  that is not loose names a tool, and some tool of its grade can be made by its tier (the reachability
  test of `crafting.md`).
- The build hammer (ADR 0017) stays what it is; powered tools (the MV drill) later carry a larger brush,
  the "better tools, bigger brushes" of `fundamentals.md`.

## Things lying in the world: the pile
- A **pile** is the inventory of an air cell, drawn as its items lying on the block below. There is one
  per cell at most, and it holds up to 4 stacks; a fifth kind dropped there goes to the nearest cell with
  room.
- Three things make piles: the ape **sets an item down** (right click with it in hand on a block's top),
  **drops** it (a key), or a dig gives more than the ape can carry (the rest lies where the block was).
  Dug blocks still go straight to the inventory when there is room, as today: fewer things to simulate
  and fewer clicks (*taste call*, against Minecraft's blocks that pop out and are walked over).
- **Nothing despawns.** The planet is finite and counted, so a thing that vanishes would be matter lost.
  Piles merge by cell, so their number is bounded by the cells an ape has littered, and a pile nobody
  touches is plain data in its region: it costs nothing in the tick (fundamentals 2.4).
- **The pile is what crafting in the world runs on.** `crafting.md`'s process runner checks piles and
  open vessels against the conditions at their cell: wet clay pots set down by a fire dry and then fire;
  a pan of gravel set in a stream washes. No second kind of "placed item" is needed until the fine cells
  bring shaped things.
- A pile falls with the block under it and floats or sinks by its density when water comes (with world
  physics, ADR 0017; until then it stays where it is and drops when its floor is dug).
- **Death** makes a pile of what the ape carried (`needs.md`).

## The screen
- A key opens the ape's slots in the HUD's glass (ADR 0030), and beside them whatever is open: the
  container or machine under the crosshair, or the pile at the ape's feet. No crafting grid
  (`open-questions.md`: hands craft in the world).
- The mouse works as in Minecraft, since those habits are free to reuse: click lifts a stack, right
  click half, shift-click sends it across, a drag spreads it. The number keys send to a hotbar slot.
- Each row a pack adds is drawn as that pack's row, so a glance says what dropping the pack would cost.
- Off-screen it is reachable by `acts=` (`point`, `click`, a key through the bindings) and the JSON of a
  play run lists the slots. Bots get `take`, `put`, `drop` and `slots` (a report line).

## Wire, saves, determinism
- One message moves things: `ToServer::Move { seq, from, to, count }`, where `from` and `to` are (holder,
  slot) and a holder is the ape itself, a block entity or a pile's cell. The server checks reach and the
  merge rule, applies it whole or not at all, and answers `Done { seq, ok }`; the client predicts it, as
  it does edits (ADR 0015).
- `ToClient::Inventory` becomes slot deltas to the owner; piles and open containers go to the clients
  subscribed to their region, as entities.
- Sim code visits holders in a fixed order (block entities by their stable id, piles by chunk then cell).
  The state hash covers every slot.
- Saves: a version bump. An old save's counts load into slots in block id order; what does not fit lies
  in a pile at the player's feet.

## Tests
- Data alone: every block has its item; ids and names are unique; every tool kind a block names exists;
  a stack size is 1..64.
- **A conservation law for the soak** (fundamentals 4.8): the count of each item over all apes, piles and
  containers changes only by a dig, a place, a recipe or a spoiled food, and each of those reports its
  change. Bots move, split, drop and take stacks at random, some out of reach and some to full slots.
- Two answers must agree: the client's predicted slots against the server's after every `Done`.
- A named worst case for the benchmark: a face littered with 100,000 piles, asleep. It must cost no tick
  time, and a region of them must mesh and draw inside the budget.

## Order of building
1. The item registry and slots in place of counts: the hotbar is slots 0-8, digging fills them, the save
   bumps, bots get `slots`. No new content: the gate's soak hash changes and nothing else.
2. The screen and `Move`, with prediction.
3. Piles: set down, drop, overflow, pick up; the conservation check in the soak.
4. Tools as data: `tool` and `grade` on blocks, speed, uses, scrap. This needs the first recipes
   (knapping a hand axe), so it lands with `crafting.md`'s process runner.
5. Containers as block entities, and packs that add rows.
6. Payloads: food's day, vessels and the bucket.
