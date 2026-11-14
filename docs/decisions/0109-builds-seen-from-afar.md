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
costs nothing where there are no edits, is right after a dig as well as a place, and the server and the
client agree (one function in the engine, deterministic, independent of the edits' hash order).

## Options considered

1. **A per-level summary of edits** (the roadmap's words): a map per level from coarse cell to its chosen
   block, kept up to date on every `set_block`. Fast to read, but a second store beside the edits that has
   to stay right through digs, undo (`put_edit`), saves and the wire; more code than the problem needs now.
2. **Choose per cell at mesh time from the edits already gathered** (chosen). `edits_for` already walks every
   level 0 edit chunk that overlaps the coarse chunk; it now keeps every edit in range (not only those at a
   middle) and `pick_cells` chooses one block per cell. No new state, nothing in the save or on the wire.
3. **Majority or density rule** (a cell is solid when, say, an eighth of its blocks are): truer volumes, but
   a 1-wide tower is 1/16 to 1/4096 of a cell and would vanish again, which is the bug.
4. **Draw edits apart from the chunks** (instances or a far "build" mesh): exact sizes from afar, but a second
   renderer for blocks and its own LOD; not worth it for this.

## Decision

Option 2. A coarse cell's block, from the edits inside it:

- a placed (non-air) block at its middle: that block (as before);
- else the most common placed block in the cell, ties to the lower id (so the order never matters);
- else a dig at its middle (as before: a dug shaft shows where it crosses middles);
- else nothing: the generated cell.

A dig off a middle never shows from afar (the brief: a shaft need not), and a removed tower leaves only air
edits, so its cells fall back to what the ground generates: it disappears. A placed block wins over a
generated solid cell too: a lamp set into the ground glows from afar.

`stream::sampling` (which chunks to mesh again after an edit) now names the coarse cell that holds the block
at every level, not only when the block is a middle: any edit can change a far cell now.

**Lamps from afar.** The glow is free: a far cell whose block is a lamp is drawn with the lamp's material,
and the shader's emissive path (`looks[m].info` bit 24, `terrain_lit`) shows its own light, so no shader edit
was needed. Its light on the ground around it is not: the traced lamp light reads the lamps near the camera
(`LAMP_R` 24 m, the lamp grid of ADR 0025 H); a far lamp lights nothing but the bounce rays that happen to
meet it. The glow alone reads well at night (stills below).

**Sizes.** A 1-wide tower is drawn as wide as one cell of the level it sits in: 2 m at level 1, 16 m at
level 4. From a few hundred metres it reads as a thicker tower, from a kilometre as a mark. That is the call
of this ADR the developer may overrule (option 3 is the other side of it).

## Checks

- `world::tests::a_thin_tower_shows_in_coarse_cells_and_goes_when_dug` (red first: level 1 cell air, the
  tower placed at columns off every middle; green after; then dug, every cell air again).
- `stream::tests::a_block_off_the_middle_is_sampled_at_every_level` (red first at level 1).
- Stills of the bug's own view on a copy of its save, play build, before (`build/` of ffc9f557) and after:
  `out/bugs-1114/tower-before.png`, `tower-after.png`, `tower-zoom.png` (before and after side by side).
- An approach flight filmed on the same save: `out/bugs-1114/tower-fly.png` (the sheet, every 250 ms) and
  `tower-fly-strip.png` (the tower cut out of its first 26 frames).

## Cost

One measurement each, play build, `bash tools/slot.sh measure`, against `tests/perf/baseline.json`:

| run | frame thread work (ms p50) | GPU (ms p50) | mesh job (us a chunk) | wgpu MB |
|---|---|---|---|---|
| `fly quick=1` | 2.069 (base 2.004) | 4.989 (5.033) | 619 (612) | 974 (974) |
| `edit quick=1` | 3.585 (3.607) | 3.777 (3.726) | 1041 (1003) | 834 (834) |

All within the gate's tolerances; the edit run's mesh job is 4 % slower (the sort in `edits_for` and the
coarse chunks meshed again), and it remeshed 327 chunks a second at 1267 edits a second. Nothing for
`docs/perf-debt.md`.

**Approach flight:** the tower is in every frame of a 60 m/s flight toward it and grows steadily
(`tower-fly-strip.png`); one frame where the level changes shows a short piece of a coarser cell over its top,
a smaller step than today's, where the tower appears from nothing.

## Consequences

- An edit now meshes again the chunk that holds it at every level that is loaded (before: only where it hit a
  middle, one edit in 2^3L at level L). The slots that are not loaded cost nothing.
- `edits_for` sorts the edits in range per coarse chunk; with no edits it returns at once as before.
- Shortcut: the padded strips past a face seam still take a coarse cell's middle block only, so a thin build
  in a coarse cell right on a seam may miss from the neighbouring face's padding (one cell's side face).
- Not changed: the save format, the wire (the client builds its far cells from the same edits), old saves
  (they hold edits as before and now show them from afar).
- The roadmap's "Big builds seen from afar" is done for builds on a body's grid; builds in free space are not.
