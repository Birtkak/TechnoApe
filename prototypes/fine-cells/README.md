# Fine cells spike (`docs/design/fine-cells.md`, ADR 0078 and 0079)

**Question.** How many awake things can the server tick (50 ms, with 50 bots) afford: (1) whole loose blocks
falling and sliding as blocks, (2) fine cells of 12.5 cm (8 x 8 x 8 a block, one byte each, a block that
is fine keeps a 512-byte payload) falling and piling as a cellular automaton? And how long does a burst take
to settle?

**Answer** (Ryzen 5 5600X, one thread, release, `target-cpu=native`; a flat grid with the edits in a hash
map, as the game's are; a BTreeMap or a sorted list for the awake set, so the order is fixed):

| case | awake visits | ns a visit | peak awake | ticks to settle |
|---|---|---|---|---|
| whole blocks, 1 shaft under 4 m of sand | 489 | 157 | 29 | 81 |
| whole blocks, 64 shafts | 31,168 | 178 | 1,792 | 81 |
| whole blocks, 1,024 shafts | 496,768 | 305 | 27,712 | 81 |
| fine cells, 1 block burst 2 m up | 102,020 | 61 | 5,632 | 29 |
| fine cells, 8 bursts | 816,600 | 101 | 45,056 | 29 |
| fine cells, 64 bursts | 6,533,108 | 90 | 360,448 | 29 |

- **Whole blocks: about 200 ns an awake block.** 1 ms of tick holds some 4,000 awake blocks; the game's
  lookups (a cube-sphere column, the generated ground behind the edits) are dearer than this flat grid, so
  the server caps the visits at **2,048 a tick** and carries the rest to the next. 64 shafts collapsing at
  once (1,792 awake at the peak) fit inside the cap; a thousand spread over a few ticks.
- **Fine cells: about 90 ns an awake cell, and one burst wakes some 5,600 cells at its peak** (every move
  wakes the ten around it). One block's burst is 0.35 ms at its worst tick and settles in 29 ticks (1.5 s,
  at a cell a tick). 50 bots digging a block a second each keep about 70 bursts in the air: some 400,000
  awake cells, **about 35 ms a tick: the whole budget.** So a dig's burst cannot be the server's material.
  It is the client's (presentation, a handful of chips, not 512 cells), and material fine cells are kept
  for the rarer things that need them (chiselling, a blast, a pile that does not fit whole blocks).
- **A burst never refills a block:** the 512 cells of a block spread over 9 fine blocks at their 45 degree
  angle of rest, none of them full again (`full_again` 0 in every case). Settling back into whole blocks
  therefore needs a rounding rule (the design's: a sleeping fine block of half or more one material becomes
  that block, else air, and the ledger counts the cells rounding made or took).
- **The ledger:** every case ends with exactly the cells it began with (an assert).

Throwaway code: its own cargo project.

## Run it
```
cd prototypes/fine-cells
cargo run --release      # one JSON line a case
```
