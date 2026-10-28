# 0103 – Falling water draws as cubes, connected surfaces, the waterfall sound counts falls

- **Status:** accepted (built 2026-10-28)
- **Date:** 2026-10-28

## Context
Bug reports 2026-10-28 08-21-48 ("the water has lots of tears inside it when moving") and 08-21-26 ("a lot of water
falling here should make a sound, also it falls still pretty slow"), with 2026-10-27 17-59-05 ("water should update
faster").

- **Tears.** The fluid height field (ADR 0069 A) took a column's vote as its *highest* cell of the fluid within the
  mesher padding (6 cells up and down). A drop or a heap in the air above a surface became that column's surface,
  and two top cells at different heights that share a corner looked through different windows and computed different
  corner heights: thin slits through moving water where the bed shows. The same rule pulled the corners of every
  falling cell toward whatever water lay within 75 cm, so a falling stream drew as long shards; a falling cell with no
  water beside it was a "lone" film of zero height; and the A3 rule hid the side faces of stream cells, so streams
  looked hollow.
- **Sound.** The waterfall loop sampled 600 random cells in a 16 m sphere for water with air below. A stream is a few
  hundred cells out of ~9 M, so it was almost never hit (the report reads `waterfall 0.0`).
- **Slow.** The kernel's fall is faster than real gravity (3 g, up to 7 cells a tick: 3 m in ~0.47 s). At the
  report's waterfall the whole river had woken (`still_woken` 130 k, 322 active chunks), the push queue peaked at 366,
  over `remesh_busy_queue` (256), so terrain remeshes ran at the busy rate of 60/s and the waterfall's blocks
  updated rarely.

## Decision
- **Connected column tops** (`fluid_column_top` in `voxel_patch_fluid.py`): a column's surface, seen from the meshed
  cell's height, is the top of the run of fluid at that height; over open air the first fluid straight below; inside
  solid (water over a shelf) the first fluid straight above the solid. A run that rises past the padding (a pillar or
  a stream) gives no vote. Used by the 2x2 votes, the A2 ring and the A3 face rule.
- **Falling water is a cube** (`fluid_falls`): a cell whose run of fluid ends in open air within the padding draws a
  full-height box with all its side faces, whatever its fullness (a falling eighth is spray through the cell; slabs at
  its level drew a curtain of specks). A3 hides faces only under a sheet in reach, not on a falling or pillar column.
- **Falls are counted** (`SimWorld::get_falls(slot)`): each tick sums the liquid cells that fell per fluid slot
  (0..3) and their positions; `SimBubble.falls(slot)` returns `[count, mean world voxel]`. A loop with
  `"where": "falls"` in `data/sounds.json` (the waterfall: water, full at 40 falls a tick) takes its level and
  position from that instead of samples.

## Consequences
- Moving water shows no slits in the flood and stream scenes (filmed 2026-10-28); a waterfall is a curtain of
  falling water cubes with gaps between them, the LotL voxel look, instead of a sheet of shards. A stream has gaps
  because cells spread apart as they speed up (a lip feeds about one cell a tick per column).
- Frame times unchanged within noise (flood scene p99 11.0 -> 7.7 ms, stream 5.4 -> 4.9 ms).
- One centre per fluid slot for the whole sim window: two waterfalls sound from between them (shortcut, noted in
  `sim_bubble.gd`).
- Open: the remesh rate when a whole river wakes (the "slow" part of 08-21-26) is not changed here; nearer blocks first
  or a higher busy rate are the options, both cost frame time.
