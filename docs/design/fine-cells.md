# Fine cells and the reacting world

- **Status:** design, 2026-11-12. Roadmap items 4 and 5. ADR 0078 (fine cells' data and simulation) and
  ADR 0079 (loose blocks and collapse). The spike: `prototypes/fine-cells/README.md`.
- **From:** vision ("blocks are solid and cheap; where something happens they break into fine 12.5 cm cells
  (debris, sand, splashes) and settle back"), fundamentals 2.2 ("this block is fine cells" lives in the
  block's state) and 2.4 (activity, not area), `world-data.md` (one question, one answer; edits sparse until
  dense; the ledger), ADR 0017 (loose blocks fall and slide, unsupported builds collapse).

## Three things, three costs
The spike says what each costs a tick, and that decides where each lives.

| what | where | cost | when |
|---|---|---|---|
| **a loose block falls or slides** (sand, gravel), whole | server, world edits | ~200 ns an awake block | slice (a), built |
| **a dug block's burst** of debris, piling and fading | each client, presentation | none on the server | slice (b), built |
| **material fine cells** (a chiselled block, a blast, a pile that does not fit blocks) | server, world data | ~90 ns an awake cell, a burst of 512 wakes ~5,600 | with chiselling |
| **an unsupported build collapses** | server, world edits | a capped search a tick | slice (c) |

A dig cannot be 512 material cells on the server: 50 bots digging a block a second keep some 400,000 cells
awake, the whole 50 ms tick (spike). So the dig's burst is the client's, and material fine cells come only
where the world genuinely holds less than a block.

## (a) Loose blocks: whole blocks, server-side
- A block row says `falls = true` (`data/blocks.toml`: sand and gravel). Separate from `loose`, which is
  "bare hands take it" (ADR 0034).
- **Awake set:** `engine::fall::Falls`, a `BTreeMap` from (body, face, layer, i, j) to (ticks falling, cells
  fallen). Layer first, so a tick visits bottom up and a stack falls together. Nothing else is visited:
  a sleeping beach costs nothing.
- **Waking:** every changed cell of a tick (a dig, a place, a command, a fall itself) wakes itself, the cell
  over it, and the four sides at its layer and the one over it (ten cells). Across a face edge the sides
  come from `world::fold`.
- **A tick:** each awake cell that holds a falling block, visited in order:
  - the cell under it is open (not solid: air, water's gaps, a plant that is not solid): it falls. Falling
    starts slow: the block moves a cell when g t^2 / 2 passes the next metre (0.45 s for the first, then
    faster), at most one cell a tick (20 m/s). The blocks stacked on it start falling with it, the same
    count, so a column comes down whole.
  - else it **slides** to its angle of rest: a side whose cell and the cell under that are both open takes
    it (a step of two or more is too steep for a block; 45 degrees is what 1 m blocks can show; the fine
    cells give finer angles later). Which side first is a hash of the cell and the tick: deterministic,
    not always north.
  - else it sleeps (leaves the set).
- **A player in the way** (the developer, 2026-11-13): no floor. The block falls through the player's cells
  and settles; a player buried to the eyes holds their breath 10 s, then loses a point a second until they
  dig out (ADR 0079, *Burial*).
- **Bounded:** at most `FALL_BUDGET` = 2,048 visits a tick; the rest wait in the set for the next tick
  (lowest first, so the bottom of a collapse goes first). Never a stall.
- **Wire:** a move is two ordinary edits (`ToClient::Edits`), in the tick's batch with the players' own.
  Nothing new on the wire. The client plays no break or place effect for a pair it recognises as a move (a
  falling block gone from one cell and the same block in the cell under or beside it in one batch).
- **Save:** nothing new. On loading, every edit holding a falling block, and the cell over every dug cell,
  is woken; anything mid-fall carries on (a block that had not moved yet waits again, under half a second).
- **Determinism:** the server's tick is deterministic; the awake set is ordered; edits go into the state
  hash as they do. A replay from the recording comes to the same hash (the soak's check).
- **Water:** a fallen block changes its column's gaps like any edit (`regap`); it falls through water as
  through air (water is not a block, ADR 0020) and the water's ledger stays exact because regap keeps it.
- **The ore ledger:** a move goes through `set_block`, which tallies.

## (b) A dig's burst: debris on the 12.5 cm lattice, on the client
- Today a dig plays `break` from `data/effects.toml`: chips that bounce and lie, then end (ADR 0031).
- Slice (b): chips land **on the fine lattice** (their resting point snapped to 12.5 cm cells) and **pile**:
  a chip that comes to rest over another resting chip's cell lies on top of it, and one that would stand
  more than a cell higher than a side neighbour rolls down to it (the same 45 degree rule as (a), one cell
  down). Then they fade, as now.
- It is presentation only: nothing is saved or sent, the server is not touched, and the ledger is unchanged
  (the dug block went whole to the digger, as before; ADR 0074's item piles take what is picked up). Each
  client throws its own chips (the effects' own random numbers), so two players see piles alike, not equal.
- Cost: a few dozen chips a dig, inside the effects pool's budget (ADR 0031); the pile is a small map of
  resting fine cells, emptied as chips end.

## Material fine cells (designed now, built with chiselling)
- **In the world data:** a block that is fine is an edit whose kind is the reserved kind `fine` (one of the
  4,096), plus a sparse payload keyed by the block: 512 bytes (8 x 8 x 8 cells, one byte each: an index into
  that block's own little palette of up to 255 materials, 0 is air). A chunk keeps its payloads beside its
  edits; `World::block` says `fine`, and `World::cell(b, c)` answers for a cell. The one-question order of
  `world-data.md` gains nothing: an edit wins, and an edit may be fine.
- **In the save:** the payloads, sorted by block, after the edits, run-length packed (a settled pile is
  mostly runs). A save version, when built.
- **Activity, not area:** the server keeps an awake list of cells (sorted, so a fixed order), as (a) keeps
  blocks; a cell falls a cell a tick or slides one diagonally down when both cells are open; each move wakes
  the ten around it. A cap of **20,000 visits a tick (about 2 ms, spike)**. A fine block whose cells all sleep
  costs memory only.
- **Settling back:** a fine block whose cells have slept for 10 s is rounded: all one material and full, it
  is that block again; half or more of one material, that block; less, air (or its other materials'
  majority). Rounding makes or takes cells: the **ledger counts them** per body and material, as an account
  of its own (`rounded`), so made + taken + rounded stays exact and the soak checks it every tick.
- **Wire:** a fine block goes to subscribers as its payload once (an edit `fine` plus 512 bytes, or its runs);
  afterwards the moves of its cells go as cell deltas (block, cell index, new byte), a few bytes each, in the
  tick's batch.
- **Meshing:** the chunk mesher meshes a fine block's cells as quads of 1/8 m, greedy within the block, into
  the same chunk mesh: the ray-traced light sees them in the chunk's BLAS like any face (no separate path,
  so they cast shadows and take bounce light). A block that is fine is not opaque to its neighbours'
  culling.
- **Physics:** the player's box asks `World::cell` for fine blocks it touches (a few at most); step-up eases
  over half blocks (fundamentals).
- **Determinism:** the payload goes into the chunk's hash; awake cells are visited in sorted order; no
  float in the cellular rule.

## (c) Collapse of unsupported builds (designed, built after (a) and (b) are merged)
- A placed (edited, not generated) solid block is **supported** when a path of edited solid blocks through
  their faces reaches a generated solid block, the ground, within `SPAN` = 24 blocks; a block row may say
  `support = n` for a material that carries further (stone) or less (dirt).
- When a cell changes, the server searches from each placed solid neighbour, breadth first in a fixed order,
  for the ground, visiting at most `SPAN`^2 blocks a search and `COLLAPSE_BUDGET` = 4,096 a tick (the rest
  wait). A piece that finds no ground within its span falls: its blocks become falling blocks of (a) (they
  come down as blocks, landing where they land), visited bottom first.
- The generated ground is never searched: it is the support. A search costs only near builds.
- Not built tonight.

## Not in this design
Felled trees, chiselling's tools and W7's plants (they will sit on the material fine cells).
