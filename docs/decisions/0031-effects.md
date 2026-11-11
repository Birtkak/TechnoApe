# 0031 – One effects system: particles and motion

- **Status:** accepted
- **Date:** 2026-11-11 (built from `docs/design/effects.md`; 0029 is left for the world generation)

## Context
`docs/design/effects.md` asks for one system for every short-lived thing the game shows: debris, puffs,
droplets, and small motions (the block in hand, a block growing into place). It must be data-driven, lit by
the traced light like the world, hold a hard budget, never allocate per frame, and show other players'
edits as well as one's own. Until now the water had its own droplets in its draw and the HUD drew 2-D sparks.

## Options considered
- **Where to draw:** a forward pass after the composite (cheap, but unlit or lit differently from the world);
  into the G-buffer (lit, shaded and bounced like the world).
- **How to light** (spike, `prototypes/effects/`): the full per-fragment `light()`, or one sun visibility per
  particle. Per fragment costs 0.01 ms more at 4,096 and keeps lamps, the moon and the soft edge.
- **What starts a break:** the click (only one's own digs) or the cell's solid-to-air delta from the server
  (every player's, once each).
- **Ground collision:** a floor found once at the spawn (debris thrown out of a hole sinks into the ground
  beside it); a floor per column, looked up again when a particle crosses into another column; full voxel
  collision (too dear for the frame thread).

## Decision
- **`crates/client/src/fx.rs`:** rows from `data/effects.toml` (`break`, `place`, `grow`, `spray`, `splash`;
  each field documented there, checked by `fx::parse` and a test); a fixed pool of `CAP` = 1,024 / 4,096 /
  16,384 by the new Particles setting (few / normal / many), followed live in `run=play`; a full pool
  replaces the particle nearest its end among the next 32 from a moving cursor (not the whole pool: a long
  water front fills it); a row of one plays one at every scale; at most 32 cell events a frame play, past
  them a break still drops what lies on its cell.
- **Motion in the body's frame,** relative to its centre, under its own pull towards the centre, with drag.
  The floor is the top of the first solid block in the 8 under the particle's column; with none there, the
  lookup goes on from where it stopped once the particle gets that far. On leaving its column a particle
  looks up the new one (one terrain height, once a crossing), or bounces off it if that cell is solid; a
  cell broken under resting debris drops it to the floor below (`fall_through`).
- **The draw:** its own pass after the G-buffer's, one instance of 36 vertices per particle (debris a turning
  cube of the block's texels, a square a billboard, `block` the whole cell from the grid's corners), lit per
  fragment, never in the TLAS: particles cast no shadow. GPU stage `fx` (8 timestamps now) and frame-thread
  stage `fx`.
- **Breaks come from deltas:** `fx::changed` plays one `break` per solid-to-air change, one `place` per
  air-to-solid, whether one's own prediction, the server's answer or another player's edit.
- **The placed block grows:** a `grow` particle draws the block from 0.05 of its size up; the real mesh's
  update waits `GROW_S` = 0.1 s and the grow ends once the mesh shows it, the cell changes again, or when its
  2 s particle does. The click counts as shown at once, since the grow draws the block that frame: the feel
  report's `click_to_change_ms_max` no longer measures a place's remesh (a dig's it still does).
- **Tweens and tracks** (`fx::Tween`, `fx::Track`; linear, in, out, in-out, pulse, spring, shake): the block
  in hand dips on use (0.18 s) and bobs with the steps (one pulse per 0.9 m walked, scaled by pace).
- **The water's droplets** are now `spray` particles that splash where they land; `water.rs` keeps only
  where they start. `hud::sparks` and `App::acted` are gone.
- **When effects play:** `run=play`, `run=fx`, or any run with `fx=1`. Off, the pool is empty and nothing is
  drawn, so other runs and the reference views are unchanged; `run=sim` no longer draws the water's droplets.
- **`run=fx`** is the benchmark: bursts of `break` 2-14 m ahead until `fx_n` (default 4,096) are alive.
  `fx_cap=` sets the pool; `fxedit=t:di:dj:dl:block,...` scripts edits at times for filmstrips. It is in
  `tests/perf/baseline.json`, the full gate and the benchmark's small series.

## Measured (release, dev PC, `run=fx quick=1`, 4,096 live, three runs; 0.5 m blocks, see below for 1 m)
- **Frame thread `fx`:** p50 0.060-0.067 ms, p99 0.12-0.13 ms. **Misses the proposal of 0.05 ms.** `step`
  alone is 0.052 ms after the review's fixes (one terrain height a crossing instead of two); it was 0.030
  before the per-column floor.
- **GPU `fx`:** p50 0.060-0.061 ms, p99 0.069-0.073: inside 0.15 ms.
- Frame p50 2.10-2.19 ms, `budget_ok` in every run; VRAM 813 MB; no allocation per frame.
- At 16,384 ("many"): frame thread 0.21 ms p50 / 0.36 p99, GPU 0.16 / 0.19.

## After one-metre blocks (ADR 0033) and sound (ADR 0032)
Main was merged in before this branch landed.
- **What meant blocks:** a debris cube shows 2 x 2 of the block's texels, and the look keeps 16 texels to a
  block, so the break's debris and the place's specks doubled in size (`break` 0.1-0.18 m, `place`
  0.04-0.07 m); the dozen a block stays. The grow is a share of its cell and the water's drops have their
  own size: unchanged. Speeds, the floor lookup (8 cells, now 8 m) and `run=fx`'s distances are in metres or
  cells and stay. The tests' planet is the small one (n 768) and find the surface by walking down to it.
- **The hand's bob** follows the footsteps' own count (`walked`, reset at each footstep; `BOB` is `STEP_M`
  long), not a second one.
- **Sounds start from the cell events,** as ADR 0032 asked: the player's own dig or place is heard from the
  first cell it changes (`edit::changes`, now worked out always), another player's from the server's delta
  through `fx::server_edit`, which reads a cell's old block only with effects on or for the three cells of a
  batch that are heard (ADR 0032's review kept that read off the rest). A hammer's sound is now the first
  changed cell's block rather than the aimed one. Steps, landings and the menu keep their own call sites:
  they are not cell events.
- **Measured** (`run=fx quick=1`, three runs, while another session's world bake ran beside it):
  frame thread `fx` p50 0.058-0.069 ms, p99 0.09-0.13; **GPU `fx` p50 0.147 ms, p99 0.166-0.167**: the cubes
  have four times the pixels, each lit per fragment, so the GPU sits at the 0.15 budget at p50 and over it at
  p99. Frame p50 about 2.05 ms, `budget_ok` in every run, VRAM 811 MB. `tests/perf/baseline.json`'s `fx` row
  is still the 0.5 m one; main re-measures the baseline on a quiet machine.

## Checks
Tests in `fx.rs`: the rows read and check; the same burst for the same seed, it lands and is gone, nothing
rests inside a block; a spray drop dies on its floor and splashes there; a full pool drops the oldest and never
grows; exactly one break per solid-to-air delta, another player's too; debris falls on when the block under it
breaks; the curves. Filmstrips of a break and a place, close and from 20 m, with 1 m blocks (`prototypes/effects/close.png`,
`far.png`):
`technoape.exe run=stand fx=1 time=150 length=0.05 warmup=0.5 pitch=-40 capture_ms=100 capture_w=1280 sheet=out/fx-close.png "place=3:-1:1:stone,3:-2:1:stone" "fxedit=0.2:3:-1:1:air,0.2:3:-2:1:air,1.0:2:-1:1:stone"`;
from 20 m: two stone pillars `place=20:-4:-3..2:stone,20:-2:-3..1:stone` (each cell named), `pitch=-3`,
`capture_w=1920`, `fxedit=0.2:20:-4:2:air,0.2:20:-4:1:air,1.0:20:-2:2:stone`.
`tools/check.sh fast`.

A reviewer who did not write it read the diff and found no blocking bug; of its ten findings, fixed:
the Particles setting did nothing until a restart; a full pool scanned all of itself for each new particle
(about 1 ms a frame with a long water front at "many"); debris over a drop deeper than 4 m came to rest in
the air where the floor lookup stopped (test first); past the 32 events a frame a break also skipped
`fall_through`, leaving debris in the air (test first); a row of one played two at "many"; the grow's
timeout came 0.1 s after its particle's end; a crossing evaluated the terrain twice; `fx_cap=0` panicked;
`fxedit=` needed its entries in time order. Answered, not changed: the water's droplets are now off when
effects are (a moved effect, not a change of the views, which `tools/views.sh` confirms); another touch of
the same chunk during the 0.1 s shows the real block early around the grow, and the timed touch then
remeshes once more (rare, harmless); the place's click-to-change no longer gates the remesh (above); the
`fx` baseline's job time and VRAM match `sim`'s because both stand at the spawn (measured, three runs). A second fresh
reader of the fixes found one more: a column that found no floor compared unequal to the same column found
from higher up, so debris on a block over a void of 9 or more did not fall when it broke; fixed (test first).

## Consequences
- shortcut: the frame thread's `fx` is 0.06 ms against the 0.05 proposed; when it matters, step on a worker
  with a double buffer, or move the motion to the GPU.
- With 1 m blocks the GPU's `fx` is at its 0.15 ms budget (4,096 live). Levers, none taken: one sun
  visibility per particle (the spike's 20 %, losing lamps, the moon and the soft edge), fewer debris a block,
  or a lower cap at "normal".
- shortcut: debris collides only with the floor of its column and the side of the next; it does not hit
  overhangs or the player.
- shortcut: a hammer's batch from the server is applied cell by cell, so a burst early in it can take a
  floor from a cell a later one removes; `fall_through` then drops it.
- **Waiting for the developer:** whether blocks take time to break in survival. Until answered the break is
  the burst alone, at one click. The debris has one look (cubes of the block's texels); no second candidate.
