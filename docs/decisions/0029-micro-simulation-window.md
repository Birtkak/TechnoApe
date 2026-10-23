# 0029 – The micro simulation window (ADR 0012 step 1 in the game)

- **Superseded** by ADR 0033 (the bubble ticks the world's voxels); the code was deleted, ADR 0036.
- **Status:** accepted for the flag (`micro_sim=1`, implies `near_field=1`); the default stays off until the user plays it
- **Date:** 2026-10-23 (the user: "job 1" of `docs/plans/fable-backlog.md`)

## Context
ADR 0012 chose the brick world: the 6.25 cm cell is the world grid, the
simulation runs at that resolution near the player, the ray-cast near field
(ADR 0021) shows it, and godot_voxel meshes the 0.5 m view beyond. P10 showed
the kernel can tick 6.25 cm cells with conditions (background tick, distance
LOD, cell budget, static ground that loosens, liquid fullness); none of it ran
in the game, whose bubble ticked 0.5 m voxels and wrote them into the store as
whole bricks. ADR 0012's open conditions: a dense window 24-32 m wide and
16 m tall inside the 32 m render window (6 B/cell), uploads by brick, a
brick-sparse kernel later. The backlog asked four questions: dense window or
sparse kernel; what the far field gets and how often; whether fluid levels are
still needed at 6.25 cm; which view of an expanded brick is solid.

## Decision
`micro_sim=1` (`Settings.micro_sim`) adds `scripts/micro_bubble.gd`
(`MicroBubble`), owned by `SimBubble`:

- **A second SimWorld at 8 cells per voxel, dense, 32 x 16 x 32 m**
  (512 x 256 x 512 cells, 384 MB) — the dense window, not the sparse kernel:
  it fits the RAM budget (table), reuses the whole v6 kernel, and the
  sparse kernel stays the step for when the window must grow. The window is
  4 x 2 x 4 of the bubble's 8 m chunks and moves in whole chunks.
- **Toroidal** (`SimWorld.set_wrapped`): physical index = (local + offset) &
  mask, so a shift moves an offset instead of 400 MB (a memmove would cost
  tens of ms per 8 m step). Vacated chunks become unloaded walls with their
  state reset; `load_from_coarse` refills them. Window-local coordinates did
  not change anywhere in the kernel; `touch` and the LOD distance were the
  only places doing index arithmetic.
- **The bubble holds the chunks the micro window owns**
  (`set_chunk_held`: loaded state 2): they do not tick, are walls to the
  0.5 m neighbours, are not carved by `fill_sphere` nor lit by `ignite_box`,
  and are not written into the store by the 0.5 m path. Entering chunks are
  expanded from the bubble's cells on the main thread with the pool
  (`load_from_coarse`, about 1.5-2 ms per 8 m chunk, one or more per frame
  under `micro_load_budget_ms`): a partly full liquid becomes its bottom cell
  layers; only chunks with something that moves are woken, and the copies are
  stamped old so static ground does not loosen at a load seam.
- **Publishing, every idle frame** (`SimWorld.publish`, `micro_publish_budget_ms`):
  the micro tick's dirty boxes become a queue of bricks (physical index, so it
  survives shifts); each brick is written
  - into the near field's `BrickStore` as cells: a homogeneous brick is one
    word again, a plant's cells show its pattern (paint ids);
  - into the bubble's held cell as the brick's **majority**: a material that
    fills half the brick wins; else a plant whose pattern is at least half
    intact; else the main liquid at a level of round(volume / 512) eighths;
    else air. Burning is copied as a flag without a push (particles sample the
    bubble, which now also reads held chunks).
  The far field, collision (ADR 0011: `VoxelBoxMover` reads 0.5 m voxels),
  stains, particles and `get_voxel` all read that majority through the
  bubble's existing budgeted push queue (ADR 0010/0020). So "solid" for an
  expanded brick is the majority: half the cells.
- **Edits** (mining, placing, `/fill`, blasts, the fire test) go to both: the
  bubble at once (so `get_voxel` sees them) and the micro window through a
  queue applied when both simulations are idle (no main-thread wait on a
  running tick).
- **Ticks** run on both simulations' threads in the same frame; the main
  thread only touches either when both are idle. The micro tick uses the v6
  budget (`micro_cell_budget` 1.5 M cells) and LOD (`micro_lod_m` 12 m).
- **Where the player sits**: centred in x/z, low in y (`MicroBubble.CENTER`):
  6-14 m of the window below the feet, 2-10 m above the head, because things
  fall. The window moves once the player strays 0.65-0.75 chunk from that
  spot, so it never flaps on a border.

Kernel changes for 6.25 cm (`SimWorld`, all opt-in, the 0.5 m game unchanged):
- **Plants are their pattern** (`set_micro_masks`, from the near field's
  micro patterns): a tuft fills only its blade cells, not a solid 0.5 m box,
  so sand does not rest on invisible grass. A replaceable plant cell in the
  way of a liquid or powder is washed away or buried (Minecraft's rule for
  water and tall grass).
- **Cascade** (`set_cascade`): a cell vacated into air may be entered again
  in the same tick (still "fresh" for ADR 0013 loosening), so a column falls
  together instead of one 6.25 cm layer per tick; and a pressed liquid cell
  next to air always flows (the "calm pool" damping only kept water walls
  standing). A 1.5 m water ball 1.6 m up is down in 27 ticks (99 before), a
  sand ball in 38 (43: sand's pace is its pile's one-step slopes).

## Answers to the backlog's questions
- **Dense or sparse:** dense, 32 x 16 x 32 m, 384 MB; the process stays far
  inside 4 GB (table). Sparse when the window must be taller or wider.
- **Far field:** the majority per brick, written into the bubble as the
  brick changes, pushed to godot_voxel at the bubble's existing push rate.
- **Fluid levels at 6.25 cm:** kept. A cell is small, but fullness is what
  conserves mass (tests), lets thin films settle and gives the 0.5 m view its
  level (volume / 512 eighths); the micro window and the bubble now agree.
- **Which view is solid:** the majority; half a brick is solid.

## Measured (dev box, 1280x720, vsync off, 20 s after load, `micro_sim=1`)
Budget matrix (ADR 0014 scenarios, `near_field=1` in both; `other_godot_processes` 0 in every row;
blast scenarios fly at 8 m instead of ADR 0021's 20 m so the craters land inside the micro window):

| scenario | off: p50 / p99 / max | **micro: p50 / p99 / max** | micro > 8.3 / > 16.7 | micro tick p50 / p99 / max | scanned max | publish ms max / queue max |
|---|---|---|---|---|---|---|
| fly 6 m/s, 30 m up | 3.17 / 4.31 / 6.75 | 3.19 / 4.29 / 7.10 | 0 / 0 | 0.06 / 0.10 / 0.16 | 0 | 0.09 / 0 |
| fast flight 40 m/s | 3.70 / 5.44 / 18.94 | 3.72 / 5.57 / 16.52 | 1 / 0 | 0.07 / 0.12 / 0.13 | 0 | 0.22 / 0 |
| walk + mine + place | 3.19 / 4.16 / 6.48 | 3.13 / 4.06 / 6.85 | 0 / 0 | 0.06 / 0.37 / 0.49 | 37 k | 0.08 / 0 |
| forest fire (`fire_test=14`) | 5.68 / 6.38 / 25.53 | 5.65 / 6.21 / 28.76 | 1 / 1 | 0.86 / 1.88 / 7.33 | 1.50 M (budget) | 1.06 / 122 |
| water spill (1.5 m ball / 2 s) | 4.99 / 6.37 / 9.36 | 4.97 / 5.96 / 8.81 | 2 / 0 | 3.06 / 4.59 / 6.38 | 442 k | 1.09 / 284 |
| 8 m blast / 2 s | 4.66 / 5.77 / 8.86 | 4.58 / 5.55 / 7.90 | 0 / 0 | 0.06 / 0.13 / 0.87 | 639 k | 0.27 / 0 |
| 50 x 1 m craters / 2 s | 4.61 / 5.53 / 8.03 | 4.59 / 5.57 / 8.18 | 0 / 0 | 0.06 / 0.11 / 0.26 | 20 k | 0.10 / 0 |

RSS 1.42 GB off, 1.80 GB on (the window is 384 MB); VRAM unchanged (the near field already
held its volume). The micro tick runs on its own thread (it costs the frame nothing but its
start call); the frame pays the publish (at most 1.1 ms, the queue drains between ticks) and
the expansion of entering chunks (1.5-2.5 ms per 8 m chunk, the walk's shifts and the
first fill). Fast flight shifts the window 121 times in 20 s and stays at p99 5.6 ms.

Two rows fail the budget **with the micro simulation off as well**, on today's code: the
forest fire's one frame of 23-29 ms is a pipeline compiled on the render thread about 11 s after load
in (`new_pipelines` 1, render CPU 22-27 ms; also 1 s once, before the shader cache held it),
and fast flight's worst frame is render-thread uploads (`other_ms`). Neither is the
simulation; both are regressions since ADR 0021's table (6.42 / 10.53 ms) to chase on their
own. With the micro simulation on nothing else crosses 8.3 ms more than twice.

Update, same day: the fire's compile was the liquid/gas terrain material, never drawn in a
forest before the first smoke. `scripts/visuals/material_warmup.gd` now draws every terrain
model under the loading screen; the fire then has no frame over 8.3 ms (near field on: p99
6.39, max 7.22 ms; plain renderer max 5.56 ms). Fast flight's 19 ms frame (`other_ms` 16,
driver time outside Godot's phases) did not come back in two reruns (max 7.95 / 10.25 ms).

What the spike found and fixed before this table: applying the fire test (ignite a 15 m
cube = 12 M cells) on the main thread cost 35 ms; `ignite_box` and `fill_sphere` now run one
job per chunk on the pool, and queued box edits are applied in 1 m slabs under
`micro_load_budget_ms` (the 8 m blast's frame fell from 10.8 to 7.9 ms).

Filmstrips (session, `game/out/micro_sand8`, `micro_water8`, frames 0.33 s apart, camera
3 m up): a 3 m sand ball falls as a ball of 6.25 cm cells and settles into a cone; a water
ball falls, flattens and runs out into the grass, washing tufts away where it flows.

## Consequences
- The window's faces are walls both ways: water flowing out of the micro
  window stops there, and the 0.5 m simulation outside cannot flow in. An
  exchange across the face (or a window that never ends, the sparse kernel)
  is the next step when it shows in play.
- Flying high, the ground is below the window (6-14 m under the feet): edits
  there run at 0.5 m. The near field still shows them (as whole bricks).
- Water at 6.25 cm falls and spreads, but a large body still flattens slower
  than real water: pressure does not cross a pool (P10 §6). Compressible-cell
  water is the next liquid spike.
- Fire, smoke and gases at 6.25 cm move one cell per tick: a fire spreads
  eight times slower in metres than at 0.5 m. Retuning `fire_hp` and ignite
  chances per cell is content work for when the flag goes default.
- Loose flags and liquid momentum are not copied across the load (as ADR
  0013 already accepts for saved chunks).
- Chiseling (job 2) is now an edit into the micro cells: the publish path
  already writes cells to the store and the majority to the far field.
