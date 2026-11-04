# ADR 0137: Fluid lag: generated ice stays frozen, the sim stops paying for chunks that only loaded

Status: accepted (2026-11-04). Builds on ADR 0069 B (liquid substeps), 0104 (endless world water) and 0119 C (heat,
state changes). No material ids, atlas tiles or save format changed.

## Context
Bug report 2026-11-03_23-05-16 (seed 1935341982, voxel 1148 1877 5955, creative flight): "how fluids react and fill
spaces etc is still very junky and laggy". Its state.json had a **60 ms sim tick** (the period is 33 ms), 416 active
chunks, 1.44 M cells scanned in one tick (the budget is 1.5 M), a paste queue of 209 (max 493), 629,121 still cells
woken and 613,615 chunk pastes for 554,622 chunk loads: nearly every chunk the flight loaded was pasted back and
remeshed. Single frames spent 296-512 ms in pastes.

`fluid_lag=<case>` (`scripts/debug/fluid_lag_test.gd`) times a fluid change in the running game: frame p99/max over
the 25 s after it, where each frame over 16.7 ms went (the bubble's split, the process phase per node, godot_voxel's
main-thread steps), sim tick ms, cells moved, pastes, remeshes of the 8 m mesh blocks around the spot (godot_voxel's
`mesh_state`), the wait from a paste to its remesh, the latency to the first remesh and to rest. Cases: `lake` /
`river` (a 1.5 m hole dug into the bank under the surface), `bomb` (a 1.5 m water ball into a 2.5 m pit), `pour` (a
2 m ball let go in a 4 m cave room), `fly` (spectator flight at 15 m/s, 4 m over the nearest lake; `fluid_lag_from=x,z`
picks where to look). Windowed runs, one of ours at a time; other sessions ran Godot beside them.

**Standing cases** (quiet machine, before): frames were fine (p99 4.7-6.0 ms, max <= 10 ms, none over 16.7 ms), the
first remesh came one sim tick after the change (23-57 ms), a paste waited a median 5-17 ms for its remesh, the paste
queue never built up (0-5). The paste and remesh path is not the lag while the player stands still.

**Flight over the developer's lake** (before): the generated ice of the frozen lake **melted** although the outside
was 3.6 C: the sim kept the outside temperature as a rounded integer (3.6 -> 4 = ice's `melt_c`), and a loaded chunk
kept the heat its recycled slot had before (the field starts at 18 C, and relaxing toward the outside moves a cell
only once it is 64 degrees off). 141k cells of moving water in the window before the flight; during it 9,359 still
cells woken, 18,228 pastes, a paste queue of 402, sim ticks of 32 ms on average and up to 561 ms. Over a lake without
ice nothing woke, but 100-370 chunks were awake every tick with no water moving: a loaded chunk is awake for its first
tick, and each of water's 8 liquid substeps rescanned it whole (`fluid_bench` `reload`: 400 freshly loaded lake chunks,
an 18 ms tick; the next tick 0.02 ms). Below 0 C, `relax_heat` called every heat cell "extreme" and woke its chunk
whole every 64 ticks: 512 chunks a tick in the game's window, about 2.1 M cells, over the cell budget, with nothing
to freeze. The thread pool woke every thread for each of a tick's 64 passes (8 substeps x 8 parities), even for two
chunks.

## Decision
- **The outside temperature is floored, not rounded** (`set_ambient_c`): 3.6 C is under ice's melt point.
- **A loaded chunk's heat starts at the outside** (`SimWorld::on_loaded`, chunk and column loads); SimBubble hands
  the sim the outside before it loads chunks, not only when a tick starts (a pin takes its window's), and
  `Survival` measures again at once after the player moved over 32 m (`PROBE_MOVE_M`), so a teleport or a fast
  flight does not load the next place at the last one's temperature.
- **A liquid substep scans a chunk only when it or a face neighbour held moving liquid with substeps left** at its
  last scan this tick (`fast_`, `needs_substep`). A spill crosses at most into a face neighbour in one substep.
- **At or below 0 C only chunks that may hold a liquid that freezes or a gas that condenses are woken whole**
  (`switch_`: 1 after a load, a shift or a scan that saw such a cell; 0 after a whole-chunk scan that saw none). The
  hot wake (>= 90 C) stays: it also drives reactions such as water against flames (`test_fire`).
- **The thread pool wakes as many helpers as a pass has chunks beyond the caller's own, and the caller works too.**
- Tried and dropped: running the sim's threads at below-normal priority. Interleaved A/B runs showed no difference
  under the other sessions' load, so it stays out.

The kernel's results are unchanged: `tools/sim_equiv_probe.gd` (a hash of every cell after 400 ticks of a spill,
water bombs, lava into water with steam, water at -6 C and a fire past a puddle) is identical for the old and new DLL
with 1 and 6 threads. `test/test_sim_0137.gd`: ice loaded at 3.6 C stays ice (fails on the old DLL); a cold window
wakes nothing once its puddle froze.

## Results
Kernel (`fluid_bench`, old -> new DLL, run twice back to back): basin 1.14 -> 0.75 ms a tick, bombs 4.8 -> 3.7 ms,
reload (400 loaded chunks) 16.5 -> 7.0 ms, cold (400 chunks at -5 C) 0.63 -> 0.015 ms with 25,600 -> 0 cells scanned.

Game, flight over the developer's frozen lake (`fluid_lag=fly seed=1935341982 fluid_lag_from=1148,5955`):

| | before (median of 3) | after, melt still on (median of 3) | after, final (A/B pair, same load) |
|---|---|---|---|
| moving water when it starts | 141k | 141k | 0 (old DLL in the pair: 141k) |
| still cells woken | 9,359 | 49 | 48 (old: 1) |
| pastes in 25 s | 18,228 | 593 | 449 (old: 10,129) |
| paste queue max | 402 | 0 | 0 |
| sim tick avg / max ms | 32 / 561 | 6.0 / 28 | 20 / 623 (old: 46 / 635) |
| frame p99 / max ms | 52 / 159 | 8.6 / 190 | 35 / 213 (old: 46 / 353) |
| frames over 16.7 ms | 213 | 20 | 131 (old: 235) |

The final pair ran beside another session's game, which inflated both sides. The flight's remaining long frames have
the bubble at about 1 ms and the process phase at 0.4-3 ms: they are terrain streaming (GenerateBlock / MeshBlock
slowest, `load_ms` up to 62 ms for 5 column reads) and machine load, not fluid work.

Standing cases after (clean runs): river p99 5.2 ms, bomb 4.6, pour 4.8-5.4, none over 16.7 ms. Sim tick avg falls
(river 4.4 -> 2.9 ms, bomb 5.1 -> 3.1, pour 5.6 -> 3.6). First remesh one tick after the change (26-57 ms), paste to
remesh 5-18 ms, unchanged. Contact sheet: `fluid_lag_0137_before_after.png` in the overseer's notes folder (the frozen lake
while it loads: open water before, ice after).

## Not done
- Paste order stays oldest first (ADR 0010); a near-first order matters only with a backlog, which these fixes remove
  in the measured cases.
- The 296-512 ms single-paste frames of the report did not reproduce. A paste takes godot_voxel's spatial write lock
  on its blocks and waits for whatever task reads them (streaming's side).
- Above 4 C outside, generated ice still melts as a whole lake at once: whether still ice should melt from the outside
  alone (still water never switches) is a taste call.
- The `lake` case never rests: after ~11 s, 112-128 cells move every tick with no net change (a pour that drains as
  fast as it fills), which keeps a few chunks awake.
