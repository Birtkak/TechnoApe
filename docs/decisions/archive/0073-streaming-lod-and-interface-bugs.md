# 0073 – Streaming, LOD and interface bugs: 10 km/s flight, coarse cover, one window, still outline

- **Status:** accepted (built)
- **Date:** 2026-11-12

## Context
Five F9 reports from the developer, one round:

1. `2026-11-12_21-00-30` flying 100–1000 m/s about 125 m up, "a HUGE chunk of the world becomes like the
   lowest quality LOD" near the camera.
2. `2026-11-11_18-14-37` "glitched LOD" on Luna (1024 columns) 1 km up, on a 24576-column planet.
3. `2026-11-11_17-37-12` going from the title into a world swaps windows; asked for a loading screen instead.
4. `2026-11-11_17-38-47` the selection outline clips into the block a little; its rainbow should run like an
   RGB keyboard's.
5. `2026-11-12_20-56-50` creative flight up to 10 km/s.

## Options considered
- (1) A chunk that splits is drawn as cover while one of its children has no mesh (ADR 0038 B). Children out of
  view rank last in the queue (×3 and behind every hole), so while flying the coarse parent kept covering its
  visible children, which were already in, and stood over their finer ground. Options: rank hidden children
  higher (more work for chunks nobody sees), or let only visible children hold the cover. Chosen: the second.
- (3) Keep a process per world and hide the swap better (ADR 0038 already kept the old window up), or one
  process and one window for everything. Chosen: one event loop for the program; the title and each world are
  sessions run on it (`run_app_on_demand`), each taking over the window of the one before.
- (5) Raise `phys::FLY_MAX` alone: a step at 10 km/s is 167 m, 670 substeps of 0.25 m with three collision
  checks each, on the client, its replays and the server. Chosen: far over every body's top layer (radius +
  `AMP` + `BUILD_HEADROOM`, plus the step's length) nothing can be in the way, so the step is one move there.

## Decision
- **Flight:** `FLY_MAX` is 10 000 m/s; the wheel reaches it. Over the top layer a flying step is one move; near
  the ground the substeps stay, and a dive at 10 km/s still lands (test). No server check limits speed;
  impacts over 3 m a step were already read as teleports (creative has no health).
- **Coarse cover:** a child wholly out of view (`Streamer::hidden`) no longer holds its parent as cover in a
  drawing walk. New `Streamer::too_coarse` counts drawn, visible chunks the split rule would split; a run's JSON
  has `coarse_slow_s`, the longest that stays non-zero while the camera moves under 20 m/s, and the gate fails a
  benchmark run over 1 s. Tests: the cover case (red: 18 coarse chunks over children that were in) and a
  scripted fast flight that settles at its level.
- **Luna:** not reproduced on this build. Filmstrips of Luna at 1664 and at 1024 columns from 1 km up show the
  whole moon lit and intact; the report's build predates Luna's bake (ADR 0070). No code change.
- **One window:** `main` makes one event loop; `session` builds a world (or the title's) and runs it; the next
  session gets the window (`App::carried`), setting its fullscreen, frame and size from its settings, and makes
  its own GPU state after the old one is dropped. Until a world is in, play draws a loading screen: the chunks
  still missing against the most there were. The bake's and the map download's bars stay on the title (ADR
  0057). The new-process hand-over (`sys::wait_shown`, `shown=`) is gone.
- **Outline:** its clipping was already fixed by ADR 0041 (drawn with no depth test, on the mesh's own
  lattice); the keyboard wave is there too. New Accessibility row **Outline colours: moving | still**
  (`outline_colours`, bit 16 of `opt.x`) holds the hues where they are.
- A `fly:<m/s>` act for scripted flights (the playtest menu's step).

## Consequences
- CPU: the flight fast path saves about 2 000 collision checks per step at 10 km/s high up and costs one
  `altitude` per flying step; `too_coarse` runs only in measured frames (one distance per drawn chunk); the
  cover test adds a bounds lookup and a frustum test per incomplete child in a walk.
- A child out of view now shows nothing until its mesh comes; a camera that turns without moving (a frame that
  only culls) can see that hole for the frames until the next walk, which a landing job starts.
- Sessions share one process: statics (sizes, the audio folder, the bake caches, the contrast flag) carry from
  one world to the next. Each world still gets a fresh server, world, streamer and GPU state.
- Off-screen runs print one JSON line per session, as the separate processes did.

## Review
A fresh reviewer read the diff (principle 12). Its findings and answers:
1. *Serious:* the cover left out for a hidden child depends on the look, but a frame that only culls reuses the
   last walk when the camera did not move, so turning round showed that child's hole. Fixed: such a walk marks
   the streamer dirty, so frames walk again until the child is in (asserted in the cover test).
2. Ray-traced shadows lose the parent as a caster where a hidden child is not meshed yet. Accepted: the
   shadow pops in when the child lands. The parent's coarse surface over fine children cast wrong shadows too.
3. Between two sessions the window is not pumped while the next world is built. Measured: 0.7 s from the
   world's save to the title's GPU, 1.1 s from start to a 4 km world carried on from its save; Windows ghosts a
   window after 5 s. A new world's bake runs on the title under its bar, pumped. Accepted.
4. Flight near the ground at 10 km/s keeps 670 substeps a step: measured 7.1 ms of CPU for 60 steps (one second)
   30 m over the noise land, about 0.12 ms a step per flying player on client and server. Accepted.
5. The fast path's switch between one move and substeps is at a margin of the step's length plus 1 m over the
   top layer; client and server differ there by rounding only. Accepted.
6. `too_coarse` was timed inside the select stage: moved after the dispatch stage.
7. `sound_dir=` is a OnceLock: a later session keeps the first one's. Accepted (it is a command-line option).

## Views
After merging main the fixed views differed from their baselines by 3 to 9 % (leaves' pattern): main's own build
renders the same (seam 9.26 % against 9.27 % here; every view within 0.6 % of this branch's), so main's baselines
were stale after ADR 0084's acceptance. Accepted as main renders them.


## Follow-up: a steady `lod` view
After the merge the `lod` view differed from its baseline by 1.0 to 1.4 % from run to run on a busy PC (the line
is 0.5 %, the other views under 0.04 %). Not the LOD: every run loaded the same 8 304 chunks and quads. Two inputs
depended on how long loading took:
- **The sky's clock.** Off the server it was `time=` plus the run's seconds, so a shot's sun stood where the
  load and the shot's 400 frames had moved it (0.2 degrees a second); the view from 120 m with its long shadows
  shows it most. A shot now holds the clock at `time=`, as `run=view` did (`bench::sky_time`, test seen red).
- **The bounce rays' seed.** `Gi::frame` counted from the run's start, and it also picks which of four pixels
  the sun's trace takes (`n_lamps.y % 4`), so the 400 averaged frames got other rays when loading took more frames.
  `Gi::drop_history`, which a shot calls at load, now restarts it.
Measured (the `lod` view, 640 x 360 shots): the clock alone made quiet runs pixel-identical, but runs under twelve
busy loops (18 to 28 s loads) still differed by up to 37 a channel; with both, three quiet and two busy runs (22 s
loads) are pixel-identical. The views were accepted again: each view's sky is now exactly at its `time=`.
- **The weather's step.** After the weather (ADR 0076) the `coast` view once came out 39.65 % off, its sea a
  patchwork of light and dark rectangles: a storm's cell shadows (`cl_shadow` takes the 16 m cell where the
  sun's ray crosses the cloud slab), there because the server's grid steps every 10 s of its wall-clock time and
  a slow load reached a later step. The cover where the camera stands (`ToClient::Air`) greys the sky and the
  fill light too, so holding the client's first map was not enough (tried: three different pictures of `coast`).
  Every fixed view now runs under `weather=clear` (the server's `/weather` force from the start): the views test
  ground, light and LOD, not a fresh grid's first steps. Main's baselines had been shot under other steps
  (seam 95.9 % off, its whole ground in a cloud's shadow); accepted again.
  The force reaches the client as a step it blends in over 10 s of world time, which a shot's held clock
  stopped part-way (one shot clear, the next under the first map's cloud): a shot shows the newest map whole.
- A shot waits up to 180 s for the whole world (`views.sh` kills one at 240 s) (a run 60 s): on a swamped PC a shot of a half-loaded world was
  compared (86 % off).
- A shot fires at exactly 400 frames after loading (it waited for a second from the start as well, so a quiet
  PC averaged more frames; the reviewer's finding).
- Review (a fresh reviewer per commit): the shot fired at exactly 400 frames came from the first; from the
  second, a column whose cover was 1/255 when the force landed keeps it (the server sends a change over 1 only),
  so a faint cell's shadow can still drift with the wind's steps: under one colour step, accepted; and 200 s
  plus a bake ran past `views.sh`'s 240 s, so 180 s.
- Left: water's ripples and rises run on the process's clock (`sun.w`), so a view of a flowing river would still
  vary; no view shows one. In play the clouds' shadows are the hard-edged cells by design (ADR 0076).

## Calls for the developer to confirm
- Loading screen look: dark, the title, "Loading the world: N chunks to go" and a green bar (not filmed: the
  filmstrip only captures after loading).
- The still outline keeps the rainbow, only frozen; it could instead be plain white.
- Fly speed's wheel steps are unchanged (a factor per notch), so 1 → 10 000 m/s takes more notches.
- Report 2 is closed as "no longer reproduces"; a 24576-column planet was not baked to recheck it there.
