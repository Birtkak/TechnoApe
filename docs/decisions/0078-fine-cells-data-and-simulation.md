# 0078 – Fine cells: where they live, and the dig's chips on their lattice

- **Status:** accepted (the dig's chips built; material fine cells designed, built with chiselling)
- **Date:** 2026-11-13

## Context
The vision's reacting world: "blocks are solid and cheap; where something happens they break into fine
12.5 cm cells (debris, sand, splashes) and settle back". Roadmap item 4. Design: `docs/design/fine-cells.md`;
spike: `prototypes/fine-cells/`.

## Options considered
- **A dig's burst as material fine cells on the server** (512 cells, simulated, settling back): the spike
  says one burst wakes some 5,600 cells at its peak and 50 bots digging keep about 400,000 awake, about
  35 ms of a 50 ms tick. Too dear, and the ledger would need the dug block split between the hand and the
  ground.
- **The burst as presentation** on each client, on the fine lattice: no server cost, nothing on the wire,
  the ledger untouched (the block goes whole to the digger or ADR 0074's item pile).
- **Material fine cells only where the world holds less than a block** (chiselling, a blast, a pile that
  does not fit whole blocks): a reserved `fine` kind plus a sparse 512-byte payload per block, its own awake
  list capped at 20,000 visits a tick, settling back by a rounding the ledger counts.

## Decision
- The dig's chips: `ground = "pile"` in `data/effects.toml` (the `break` row; the Studio offers the word).
  A chip of such a row bounces as before; once slow it lies in the 12.5 cm cell under it in its column's own
  frame (`fx::FINE`), on any chip already there, and rolls to a side cell lower down when it would stand on
  another (the 45 degree rule of ADR 0079), never out of its column. The cells held are found again from the
  lying chips, one pass over the pool, only in a step where a chip may come to lie; nothing else is kept, so
  a chip that fades or falls through frees its cell by being gone. `break` lives 2.5 to 4 s now (was 1.4 to
  2.2), so the heap is seen.
- Material fine cells: as `docs/design/fine-cells.md` describes, built with chiselling (not tonight).

## Evidence
- Test `chips_pile_on_the_fine_lattice` (seen red with `pile` behaving as `bounce`): four breaks in one
  cell, every lying chip in a cell of 12.5 cm over its floor, no two in one cell, some piled. The old tests
  that a chip never rests inside a block or in the air hold (a chip past its column's edge is kept in the
  column: found by them).
- Filmstrip (`run=stand quick=1 at=-1,1,-0.85 eye=1.6 pitch=-50 yaw=-60 fx=1
  fxedit="0.5:2:0:-1:air,0.6:2:1:-1:air,0.7:2:-1:-1:air,0.8:3:0:-1:air,0.9:3:1:-1:air,1.0:3:-1:-1:air"
  capture_ms=1500 capture_w=960 sheet=out/chips2.png`), read: the dug dirt and grass lie in a heap of
  12.5 cm chips at the bottom of the hole for some seconds, then fade.
  After the review's fixes the same run (`sheet=out/chips3.png`), read: the chips fly, lie on the hole's
  floor, and none is left hanging as the lower ones end.
- **The gate** (`check.sh fast` on d38636e0, main merged): tests, the soak (122 heaps, 107,916 moves) and its
  replay (hash 1f0f6f7c994497cb, 60 checks) pass; the views pass alone (seam 0.002%, lod 0.026%; seam's
  3.1% in the gate was a faint shift, no pixel over 16 levels, while other sessions compiled). The speed
  guard was not held on a quiet PC (sim tick p99 146 ms against 2.4 from compiles beside it); slice (b)
  costs the server nothing.
- Cost: the frame thread's `fx` stage; one extra pass over the pool in a step where a chip of a pile row is
  still flying (the pool holds 1,024 to 16,384), a few hash lookups a chip that lies.

## Review (a fresh reviewer read the diff)
- *A chip piled on one that ended first hung in the air* (lives differ), and the pit test had been loosened
  to let piled chips by: each step a lying chip over a cell that no longer holds a chip falls again, bottom
  first; test `a_piled_chip_falls_when_the_one_under_it_is_gone`, seen red; the pit test asks again for a
  solid block under every lying chip's floor.
- *Two columns could share a fine cell's key* (two of the frame's six numbers): the key is the whole frame.
- *The snap assumed a square frame*: it solves the 2 x 2 for s and t.
- *A set built every step*: one set kept on `Fx` and cleared, filled from the lying chips (a few dozen).
- *A second reader:* the falling-again pass ran every step: it now runs only on the step after a lying chip
  ended, over the chips `pile` put on the lattice (an unsnapped big chip never flips), and the test asks that
  the chips that fell lie again.

## Calls for the developer to confirm
- The chips fade after 2.5 to 4 s. Longer lasting heaps (until swept, or settling into a block of loose
  dirt) would be the material fine cells' job.
- Each client throws its own chips: two players see heaps alike, not equal.
- Chips stay within the 1 m column they landed in (no rolling over a block's edge onto the next).

## Consequences
- Material fine cells reuse `FINE`, the 45 degree rule and the awake-set pattern of `engine::fall`.
