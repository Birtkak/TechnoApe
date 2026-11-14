# 0109 – Builds seen from afar

- **Status:** accepted
- **Date:** 2026-11-14

## Context

Bug 2026-11-14_02-01-35: a tower of about 60 lamps, one block wide, in a rain forest at night, seen from some
180 m up, was gone but for one bright speck. A far chunk's coarse cell (2^L blocks a side) took the block in
its middle only (`World::edits_for`), so an edit thinner than a cell was skipped unless it crossed a middle.
The roadmap named it ("Big builds seen from afar: keep a per-level summary of edits"). The second half: the
lamps did not glow from afar either, because the cells that held them were air.

What the fix must keep: it is fed from the edits a chunk already keeps (saves store what changed, ADR 0016),
costs nothing where there are no edits, is right after a dig as well as a place, the server and the client
agree (one function in the engine, independent of the order edits come in), and it never stalls the frame
thread (principle 8): mesh jobs are dispatched there.

## Options considered

1. **A per-level summary of edits, kept per edit** (chosen, second round). Every level's chunk keeps, per
   edited cell, the edit at its middle and how many of each counting block it holds, and the block the cell
   shows. `set_block` and `put_edit` add the change up at every level; a mesh job gets the shared maps.
2. **Choose per cell at mesh time from the edits** (the first round, a550035d). No new state, but
   `edits_for` walked and sorted every edit in the coarse chunk's range on the frame thread: a fresh review
   found 5 ms a level 4 or 6 chunk and 41 ms for one edit in a world of 236k edits in one place (Cost).
3. **Majority or density rule** (a cell is solid when, say, an eighth of its blocks are): truer volumes, but
   a 1-wide tower is 1/16 to 1/4096 of a cell and would vanish again, which is the bug.
4. **Draw edits apart from the chunks** (instances or a far "build" mesh): exact sizes from afar, but a second
   renderer for blocks and its own LOD; not worth it for this.

## Decision

Option 1. A coarse cell (edge s blocks, s = 2^level) shows, from the edits inside it:

- the edit at its middle, a placed block or a dig, as before (a model block there counts as no edit);
- else, once the cell holds at least s counting blocks, the most common of them, a tie to the lower id;
- else nothing: the generated cell.

Only opaque blocks that are not models count (`far_counts`): glass and cutouts never colour a far cell, and
`hide_models` can never turn a far ground cell to air. A block placed at the middle keeps showing as before,
so far glass (ADR 0081) still shows where it crosses middles. These rules are the overseer's call after the
review, one the developer may overrule:

- a dig at the middle beats placed blocks elsewhere in the cell: a pit, a quarry or a tunnel mouth with
  rubble, a torch or a lamp in it still reads as a hole;
- s blocks in a cell: a 1-wide tower, wall or bridge passes at every level; a stray block, a pebble of rubble
  or one lamp does not become a 16 m or 64 m cube. The top cell of a tower may hold fewer than s and is left
  out, so a far tower can look a cell shorter;
- a 1-wide tower is drawn as wide as one cell of its level: 2 m at level 1, 16 m at level 4. From a few
  hundred metres it reads as a thicker tower, from a kilometre as a mark.

A removed tower leaves air edits only, which do not count, so its cells fall back to what the ground
generates and it disappears. A placed tower in the ground's own cells (a lamp set into a hillside, s of them)
shows over the generated block.

**The frame thread.** `World::far` holds, per chunk of every level, the cells' sums and an
`Arc<FxMap<cell, block>>` of what each shows (level 0: the edits themselves). `set_block` updates one cell a
level (about 12 levels on the 4 km planet), and the picks map is copied on write only when a mesh job still
holds it. `Streamer::dispatch` takes `far_snap` (the picks of the chunk, its 26 neighbours and, at a seam,
the neighbouring face's chunks: a few dozen lookups and `Arc` clones); the worker gathers them
(`FarSnap::edits`), hides models and holds gliding cells. The seam strips now read the neighbouring face's
picks too, so the first round's seam shortcut is gone.

`stream::sampling` (which chunks to mesh again after an edit) names the coarse cell that holds the block at
every level; a cell off its chunk's sides names only its own chunk (no folds), and the dedup looks only at
the level's own keys. `touch_block` bumps every loaded slot it names; a slot that is loaded but not drawn
(an ancestor of what is drawn) is not meshed for it: only a walk or a cull that draws it again requests the
job, so edits under a far chunk cost nothing until it is shown.

**Lamps from afar.** The glow is free: a far cell whose block is a lamp is drawn with the lamp's material,
and the shader's emissive path (`looks[m].info` bit 24, `terrain_lit`) shows its own light, so no shader edit
was needed. Its light on the ground around it is not: the traced lamp light reads the lamps near the camera
(`LAMP_R` 24 m, the lamp grid of ADR 0025 H); a far lamp lights nothing but the bounce rays that happen to
meet it. The glow alone reads well at night (stills below).

## Checks

Each seen red first (the first two against the code before this ADR, the next three against the first
round's rule, the order and sampling tests against a mutation: a tie to the last edit, no neighbours):

- `world::tests::a_thin_tower_shows_in_coarse_cells_and_goes_when_dug`: every cell holding s of a 60-high
  1-wide tower shows it at levels 1-4, the rest stay air; dug, all air again.
- `stream::tests::a_block_off_the_middle_is_sampled_at_every_level`.
- `world::tests::a_dig_at_the_middle_beats_placed_blocks_in_the_cell`.
- `world::tests::a_coarse_cell_shows_placed_blocks_from_its_edge_in_number`: s in a line show, s - 1 and one
  do not, s glass do not, levels 1-4.
- `world::tests::model_and_see_through_blocks_are_never_picked_for_a_far_cell`.
- `world::tests::a_coarse_cell_does_not_depend_on_the_order_of_its_edits` (a tie, four orders).
- `stream::tests::every_chunk_an_edit_changes_is_sampled`: levels 0-4, both corners of a chunk, the eight
  chunks whose edits change are all in `sampling`.
- The existing `padding_across_a_seam_matches_the_neighbouring_chunk` caught a missing chunk layer in the
  first `far_snap`.
- Stills of the bug's own view on a copy of its save, play build: `out/bugs-1114/tower-before.png` (`build/`
  of ffc9f557), `tower-after.png` (first round), `tower-after2.png` (this rule), `tower-zoom.png`; approach
  flights at 60 m/s, a frame every 250 ms: `tower-fly.png`, `tower-fly-strip.png` (first round),
  `tower-fly2.png`, `tower-fly2-strip.png`.

## Cost

**The benchmark** `stream::tests::far_edit_cost` (ignored; `cargo test -p engine --profile play --lib
far_edit_cost -- --ignored --nocapture`): 235,805 edits in a 160 x 160 x 12 place on the 4 km planet (a quarry
of digs and gravel, granite, lamp posts), then one edit, then a 20k-cell batch, on the calling thread:

| | first round (a550035d) | this ADR |
|---|---|---|
| setting the 236k edits | 20 ms | 91-150 ms |
| `edits_for`, level 0 chunk (2558 cells) | 787 us | 172 us, on the worker; the frame thread's `far_snap` is a few us |
| `edits_for`, level 4 chunk | 5272 us | 1 us |
| `edits_for`, level 6 chunk | 4969 us | 0 us |
| one edit: set, touch, dispatch of its 11 chunks | 40.8 ms | 2 + 2 + 32 us |
| 20k-cell batch: set / touch / dispatch (55 chunks) | 24.9 ms set and touched / 80.2 ms | 9.6 / 9.7 / 0.7 ms |

(the first round's numbers without a warm worker pool, which added about 0.3 ms to its one edit; machine
shared, about 30 % noise.) What is left on the frame thread is a few microseconds a dispatched chunk and
about 1 us an edited cell (set and touch), so a 20k-cell batch in one frame still costs some 20 ms: as much as
before ADR 0109's first round, whose touch skipped most levels but whose set was not cheaper. Memory: the
picks repeat the edits at level 0, and each coarser level adds about an eighth: some tens of MB for 236k
edits, on the server too.

**The runs** (first round, one measurement each, play build, `bash tools/slot.sh measure`, against
`tests/perf/baseline.json`):

| run | frame thread work (ms p50) | GPU (ms p50) | mesh job (us a chunk) | wgpu MB |
|---|---|---|---|---|
| `fly quick=1` | 2.069 (base 2.004) | 4.989 (5.033) | 619 (612) | 974 (974) |
| `edit quick=1` | 3.585 (3.607) | 3.777 (3.726) | 1041 (1003) | 834 (834) |

This ADR, the same runs:

| run | frame thread work (ms p50) | GPU (ms p50) | mesh job (us a chunk) | wgpu MB |
|---|---|---|---|---|
| `fly quick=1` | 1.839 | 4.941 | 608 | 974 |
| `edit quick=1` | 2.956 | 3.734 | 954 | 834 |

Faster than the first round and the baseline on the frame thread (`dispatch` 0.007 ms p50 at 1267 edits a
second, `edits` 0.027 ms) and in the mesh job, within noise elsewhere. Nothing for `docs/perf-debt.md`.

**Approach flight:** with the s-block rule the tower is in every frame of a 60 m/s flight toward it and grows
steadily (`tower-fly2-strip.png`), as in the first round; one frame where the level changes shows a short
piece of a coarser cell over its top, a smaller step than before ADR 0109, where the tower appeared from
nothing. In the bug's own view (`tower-after2.png`) it stands a little shorter than in the first round: its
top cell holds fewer than s lamps.

## Consequences

- An edit now meshes again the chunk that holds it at every level that is drawn (before: only where it hit a
  middle, one edit in 2^3L at level L).
- A save of many edits loads more slowly (about 0.5 us an edit for the sums).
- Not changed: the save format, the wire (the client builds its far cells from the same edits), old saves
  (they hold edits as before and now show them from afar).
- The roadmap's "Big builds seen from afar" is done for builds on a body's grid; builds in free space are not,
  and a far lamp's light on the ground is not.
