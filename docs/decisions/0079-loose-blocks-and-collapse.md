# 0079 – Loose blocks fall and slide; collapse designed

- **Status:** accepted (slice (a) built; collapse, slice (c), designed in `docs/design/fine-cells.md`)
- **Date:** 2026-11-12

## Context
ADR 0017 wants sand and gravel to fall and slide and unsupported builds to collapse. Roadmap item 5. The
design and its spike are `docs/design/fine-cells.md` and `prototypes/fine-cells/` (ADR 0078 holds the fine
cells' half).

## Options considered
- **Where:** whole blocks on the server as edits (nothing new on the wire or in the save), or as fine cells
  (the spike: a burst of 512 cells wakes some 5,600 and 50 bots would fill the tick), or as falling entities
  with positions (smooth, but a new kind of state, its own wire and its own landing rule).
- **Angle of rest:** 45 degrees (a step of two slides), all that 1 m blocks can show, or a material's own
  angle by a random chance per tick (a finer slope on average, but a heap that never quite settles).
- **A save mid-fall:** keep the awake set in the save (a save version bump, while other branches bump it
  tonight), or wake from the edits on loading.

## Decision
- `falls = true` on a block row (`data/blocks.toml`: sand, gravel), apart from `loose` (bare hands take it).
- **`engine::fall::Falls`:** an ordered awake set (layer first: bottom up), woken by every changed cell (it,
  the cell over it, the four sides at both layers, across face edges by `world::fold`). A tick: a falling
  block over an open cell falls a cell when g t^2 / 2 (rounded) says, at most one a tick; the blocks on it
  come down with it in the same tick; one on a block that is still waiting waits with it; on solid ground
  it slides to a side whose cell and the cell under it are open (first side by a hash of cell and tick),
  else it sleeps. Never into a cell a player's box holds: it waits on the head.
- **`BUDGET` = 1,024 visits a tick**, the rest carried to the next, lowest first: never a stall.
- **Server:** after the players' steps, before the water regaps: this tick's edits wake, the falls step, and
  each move is two ordinary edits in the tick's `Edits` (the water regaps their columns, the ore ledger
  tallies them through `set_block`).
- **Client:** two edits in a row, a falling block gone from one cell and in the next, are a move: no break,
  no place effect, no sound.
- **Save:** unchanged. Loading wakes every edit that is a falling block or a dug cell; a block that had
  not moved yet waits its first third of a second again.
- **Stand runs:** `falls=1` sends `fxedit=` cells to the run's server, so a filmstrip shows the real thing.

## Evidence
- Tests seen red (the step switched off) then green: `fall::tests` (falls to the floor in its time, a
  column comes down whole, a tower spreads to no step of two with no block made or lost, waits on a
  player's head, the budget holds and the rest carry on, two runs end in the same world) and the server's
  `sand_dug_from_under_falls_and_every_player_sees_it`.
- **Cost** (`fall_cost`, play build, the noise land whose every generated lookup is noise): 2,744 sand
  blocks dropped onto a 7 x 7 floor and spread over the ground, 132,060 moves in 624 ticks: **517 ns a
  visit, 6.9 ms in the worst tick at 2,048 visits**; hence the budget of 1,024 (about 3.5 ms at worst). A
  quiet world costs nothing (nothing awake: one `len` a tick).
- The soak's bots heap sand and gravel and dig the ground from under it (`heaps`, `moves` in its line); the
  soak's bots' worlds match the server's chunk by chunk, its replay meets every checkpoint and a save and
  load give back the same state.

- **Soak** (`technoape-server soak=1 bots=50 minutes=2 seed=1`, play build): ok, 265 heaps dug out from
  under, 209,535 blocks moved; the bots' worlds equal the server's, a save and a load give back the state.
- **Filmstrip** (`technoape.exe run=stand quick=1 at=-1,1,-0.85 eye=2 pitch=0 yaw=-45 fx=1 falls=1
  fxedit="0.2:4:0:3:stone,0.2:4:0:4:sand,0.2:4:0:5:sand,0.2:4:0:6:sand,0.2:4:0:7:sand,0.2:4:1:3:stone,0.2:4:1:4:gravel,0.2:4:1:5:gravel,1.5:4:0:3:air,1.5:4:1:3:air"
  capture_ms=700 capture_w=480 sheet=out/falls-c.png`), read: the shelves are dug (grey chips), the sand and
  gravel come down and lie on the ground, spread to one block of step; no break effect while they fall.
  That run's server tick: p99 0.07 ms.

## Review (a fresh reviewer read the diff)
- *The soak's heap branch never ran* (it sat behind the items' share after main's merge): fixed; the soak
  line above counts its heaps.
- *A fresh block on one still waiting to start slid off it* (a heap in the air lost its edges): fixed, test
  `a_heap_in_the_air_comes_down_whole`, seen red.
- *Loading woke ten cells of every edit* (millions of keys for a big save): it now looks at each falling
  block of the edits, and at the block over each dug cell when it falls, once.
- *A move's two edits split over two messages played a break and a place*, and an unrelated dig and place
  could pass for a move: moves go in `Edits` of their own, and the client pairs only side-by-side cells
  (shortcut: a move across a face's edge plays its effects).
- *Answered, not changed:* the budget visits the lowest face and layer first, so a big collapse holds back
  blocks in flight elsewhere for a few ticks (bounded; they carry on), and a budget cut between a block and
  the one riding on it parts them for one tick.

## Calls for the developer to confirm
- 45 degrees for sand and gravel alike (gravel would be steeper, sand flatter, once fine cells give finer
  slopes).
- A falling block teleports a cell at a time on the client (the edit stream): no smooth motion yet. A
  follow-up draws a falling block as a moving cube (the effects' `block` instance) between its cells.
- Sand falling onto a player rests on the head rather than hurting or burying them.
- Generated sand is not disturbed until something next to it changes: a beach over a cave stays until dug.

## Consequences
- Collapse (slice (c)) reuses this: an unsupported piece's blocks become falling blocks.
- Fine cells (ADR 0078) will wake through the same `wake` and keep their own awake set and cap.
