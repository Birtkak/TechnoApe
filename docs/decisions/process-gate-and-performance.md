# Process, the gate and performance

The reference machine and frame budget, how speed is measured and guarded, the test harness and the gate, the levers and
optimisation batches, features-first and sharing the PC. What holds today. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- **The dev PC is the reference machine.** The game should run perfectly on it; weaker PCs get lower presets of the same
  settings, not a second budget [0004].
- **Features first, speed in batches** (2026-11-13): while features are added or changed, do not optimise them; measure
  once, write the cost in `docs/perf-debt.md`, merge. An optimisation batch later works the list down until the budget
  holds again [0096]. (CLAUDE.md principle 14.)
- **Never deferred to a batch:** a stalled frame thread, a crash, a leak, a failing soak, or a frame so slow the game
  cannot be judged by playing (about 20 ms p99) [0096].
- **Benchmark series must be short:** 25-30 minute series are too long; the default series is half length, 3 reps, the
  1 km planet and the filmstrips (about 8 min; `base` and `huge` about 6 min each) [0012].
- **Merging the quad buffer at 32 M quads** (2026-11-14): `QUAD_CAP_M` is 32 by default (128 MB more buffer; `quad_cap_m=`
  on a run); the baseline took the new `gpu_alloc_mb` only [0120].
- **The GPU's idle gap between frames is closed, not fixed** (2026-11-11): about 0.45 ms a frame, a fixed cost of the
  driver; nothing to do in the game [0025 M].
- **Lattice merging of terrain quads adopted** (2026-11-11): `Body::lattice(level)` (see streaming-and-meshing.md) [0025 I2].
- **Thinner traces taste calls:** the half-resolution history and smoothing of package C was a taste call, yes [0025 C].
- **The look-changing options stay off unless the developer takes them:** `shadows=sharp` (-3.3 ms of the forest's GPU),
  `leaves=fast` (-0.8 to -1.3 ms), fluff only within about 40-64 m (turns far crowns toward cubes) [0103, 0111, 0120].
- **A frame cap at the display's refresh rate in `run=play`** (on screen only) closes "the frame loop holds a core" [0025;
  built with ADR 0028's settings pages].

## Reference machine and budget

- **Reference machine = the dev box:** Ryzen 5 5600X (6c/12t), RTX 3070 8 GB, 32 GB RAM, Windows 11 [0004].
- **Budget at default settings, vsync off, after load, in every standard run** [0004]:
  - p99 frame time <= 8.3 ms (120 fps class), no single frame > 16.7 ms;
  - process RAM <= 8 GB (working set), VRAM <= 6 GB;
  - the server's tick in `run=sim` (`stand` with 50 players at work and water running): p99 <= 25 ms, no tick over
    50 ms, and the run still holds the frame budget [0004, added by 0025 S].
- A regression past the budget is a bug. Every system ships with a benchmark that reports these numbers [0004]. Between
  optimisation batches the budget is a target the batch restores, not a gate on each feature: `budget_ok` may be false
  on main; a feature over the budget is merged with its cost accepted (`bash tools/check.sh accept-perf`) and named
  in a `docs/perf-debt.md` row [0096].
- Settings knobs exist from the start for every expensive system [0004].
- **Measuring honestly** [0004]:
  - frame time is the wall clock between frame starts, never an engine-smoothed delta;
  - "after load" starts when streaming, meshing and the simulation have had no backlog for 30 frames in a row (capped,
    and reported if the cap was hit);
  - frames that a debug capture (screenshot, filmstrip readback) stalled are excluded and counted;
  - RAM is the process working set, VRAM from the driver;
  - every frame over 8.3 ms is listed with its own phase times (main thread per system, render CPU, GPU), so a spike
    names its cause;
  - a run that shared the machine with another heavy process does not count: the run checks and repeats.
- Every headless run's JSON line carries `frame_p99_ms`, `frame_max_ms`, `ram_mb`, `vram_mb`, the `spikes` list and
  `budget_ok` [0004]; and `cpu_p50`, `cpu_p99`, `gpu_p50`, `gpu_p99` objects with every stage, and per spike its `cpu`
  stretches and `gpu` stages; `gpu_ms` keeps its meaning (G-buffer start to screen end), `gpu_p50.total` is the whole GPU
  frame. GPU stages need `TIMESTAMP_QUERY_INSIDE_ENCODERS` (the RTX 3070 on Vulkan has it) [0025 A].
- `run=sim` (`crates/client/src/sim.rs`): `stand`'s view while the in-process server carries 50 bots plus a 51st that
  pours water twice a second. The line adds `ticks`, `tick_ms_p50/p99/max`, `tick_cores`, `jobs_measured`,
  `job_wait_ms_p50/p99/max`, `worker_cores`, `frame_cores`; `tools/bench.sh` runs it in the small series [0025 S].

## The speed guard

- `tests/perf/baseline.json` holds each quick run's numbers (the medians of quiet runs); both tiers of `tools/check.sh`
  fail a run worse than its baseline by more than its tolerance (`tools/perf.py`); a run over it is run twice more and
  judged on the median of three [0025 K].
- It holds our work, not the frame (the driver's wait for a screen image moves the frame p50 12-17 % without moving our
  work). Tolerances (`TOL` in `tools/perf.py`): `work_ms` (the frame thread's stages less `acquire`, `present`,
  `between`) +30 % or +0.25 ms; `gpu_ms_p50` +15 %; `job_us_avg` +20 % (or +50 us); `gpu_alloc_mb` +10 % (or +64 MB);
  `sim`'s `tick_ms_p99` +60 % (or +1 ms). Frame p50 is printed but not held [0025 K].
- **`gpu_alloc_mb` (wgpu's own allocation) is held instead of `vram_mb`**: the driver counts a process's VRAM in 256 MB
  blocks, so the same build read 817 or 1073 MB. `vram_mb` stays in every line and in `budget_ok` [0025 K, amended
  2026-11-12].
- `accept-perf` writes a new baseline (three runs each) and refuses unless every run gave its line and every number
  (tested); a change slower on purpose runs it and its commit says why [0025 K].
- The baseline is this machine's: a run elsewhere fails against it [0025 K].
- Sessions sharing the PC hold `out/.lock` (or `out/.lock-k` around `check.sh`, which refuses to start under
  `out/.lock`) around runs whose numbers they keep [0025 K].

## Test harness and the gate

- **Soak** (`crates/server/src/soak.rs`, `technoape-server soak=1 bots=50 minutes=10 seed=`): bots in the server's
  process on in-memory links, no sleeping. Each tick each bot may change its walk, teleport (half within two columns of
  a face edge, an eighth at a corner, a tenth on the moon), dig or place within 3 m. After every tick: every player
  finite, none under the core, no walking player inside a block; every 100 ticks every bot's world copy equals the
  server's, chunk by chunk. A failure is shrunk (greedy: drop bots while it still fails; each bot draws from its own
  splitmix64 stream), recorded to `out/soak-fail.rec` and printed with a repro line [0011].
- **State hash:** `World::chunk_hashes` (one per edited chunk) and `Server::hash` (the world, every player's physics and
  input, the tick and carried-over time), FNV-1a (stable across Rust releases) [0011]. The soak's hash for a seed must not
  change in any optimisation (seed 1, 3 minutes: `90e4ed04d6628c96` at the time of 0025 and 0025 L) [0025].
- **Flight recorder:** `Server::record` writes every join, message and leave with its tick, and a hash every second, as
  length-prefixed postcard records, flushed every tick. The dedicated server records to `out/rec/` only with `record=1`
  (or `record=file`); the client's local server records by default when on screen; `record=0` turns it off.
  `technoape-server replay=file until=tick` replays headless, stops at the first checkpoint that disagrees and dumps the
  players [0011].
- **Shots:** `run=stand ... shot=file.png` renders one frame a second after load. It clears to magenta and counts magenta
  pixels whose view ray passes inside a body's lowest possible terrain (radius minus `AMP`, minus 1 m): those are
  cracks. It stores the frame at a third of its size (640 x 360); with `baseline=` it counts pixels that differ by more
  than 8 of 255 in a channel (a threshold of 32 missed a real change). JSON field `shot`: cracks, first crack pixels,
  difference in percent [0018].
- **Viewpoints:** `tests/views/views.txt` lists name, allowed cracks and pose. `tools/views.sh` runs them and fails on
  more cracks than allowed or over 0.5 % of pixels changed; `tools/views.sh accept` stores new baselines after a look at
  `out/views/*.png`. A new GPU driver can move pixels; the first gate after one may need an accept [0018].
- **Feel report:** `run=play` adds `feel`: corrections (time and size), drift, jerk (p50, p99, max, five worst; from the
  fixed-step physics path), clicks and click-to-change (frames and ms from a predicted edit until every full-detail chunk
  that samples its cells draws it, `Streamer::shows`). The gate plays a scripted 10 s run (`acts=`): every click must show
  within 16.7 ms and 0 corrections are allowed. Landing jerk is reported, not gated [0018].
- New controls get an `act` for `acts=` scripts [0018].
- **The gate** (`bash tools/check.sh`, the fast and full tiers and their times are in CLAUDE.md) runs `cargo test`, a
  soak on a fresh seed with recording, the replay of that recording (determinism and the recorder at once), the quick
  benchmark runs held to the speed guard (budget in all, no missing chunks in `fly`), the fixed viewpoints and the feel
  report, and prints one JSON verdict [0011, 0018, 0025 K]. The fast tier adds `sim` and is run after every change; the
  full gate is for handovers.
- `crates/client/src/main.rs` is split by moves only into `main.rs` (`Args`, event loop, `main()`), `app.rs`, `gpu.rs`,
  `bench.rs`, `frame.rs`, `menu.rs`, `play.rs`; `App` is the one struct every part takes by reference [0025 L].

## Sharing the PC

- **Two slots for builds, tests and pictures; the PC alone for kept speed numbers** (`tools/slot.sh`) [0096]:
  `job` takes one of two slots (`out/.job1`, `out/.job2`) and waits while `out/.lock` is held; `measure` takes
  `out/.lock` and waits for the slots to empty; `quiet` waits for both and runs a tool that takes the lock itself (the
  gate, the benchmark). A slot older than 30 minutes (the lock: 45) is taken over. The slots live in the main checkout's
  `out/`, for every worktree.
- **No freeze:** the cargo home's `config.toml` gives a cargo 6 of 12 threads (`jobs = 6`) and a test binary 6 threads
  [0096].
- **One compiler cache:** sccache 0.19.0 in `~/Tools/sccache`, cargo's `rustc-wrapper`; crates built incrementally (the
  workspace's own in the test profile) are not cached [0096].
- The Stop hook's test run takes no slot (it must not wait past its time limit); shortcut: give it a slot if tests time
  out again [0096].
- Speed numbers taken while a `job` runs are worthless: only `measure` and `quiet` runs are kept [0096].

## Speed rules that hold

- **mimalloc is the global allocator** (client, server, bot); a C dependency built by `cc` [0012].
- **One terrain lookup per column** in `phys::blocked` and **Fx hasher** for `World::edits` [0012].
- **Rayon workers run below normal priority** (`low_workers=0` turns it off); the frame thread is not raised above normal
  (no gain under load) [0012].
- **Every fast path keeps the slow path as its test's reference** and is seen failing when broken on purpose. World
  generation stays bit-identical through an optimisation: saves and bakes stay valid, `made_hash` unchanged [0102].
- **A plant shape has a build step (`shape_of`) and a per-cell step (`cell`)**; what does not depend on the cell goes in
  its `Crown`. `tree_rolls` is derived from `data/biomes.toml` at load. Far tiles ask only for trees (`tree_candidate`);
  the far canopy walks each tree's column from its top (`plant_top_with`); a tile's spots live in a dense grid [0102].
- **Height tiles:** a 34 x 34 `f64` tile per chunk footprint with min and max, keyed face, level, x, y, in a bounded cache
  in `Body` (4096 tiles, the older half evicted), built on the workers; heights to the bit [0025 E].
- **`select` keeps its last LOD walk** and walks again only when a job changed a slot, a slot was evicted, or the camera
  moved 4 m (plus 1 % of its height over a body; the moon's orbit alone does not force a walk); a left-out cover asks for
  a new walk only when the view's planes changed or the camera moved [0025 F, 0111].
- **A slot remembers `complete`'s answer for one walk (`complete_at`)**; slots live in a slab, each knowing its eight
  children, so the walk follows links not hashed keys. `dispatch` returns at once when no worker is free and sorts only
  the nearest 256 requests (the rest only if those did not fill the workers); a pending chunk is not handed over while
  the quad buffer is full [0120].
- **BLAS** is dropped the frame its mesh's range is released [0025 G1]. BLAS compaction is not used (slower to trace)
  [0025 G2].
- **TLAS:** instances are identity transforms, rays are turned into the body's frame in `ray()`/`seen()`, a persistent
  chunk table, the TLAS rebuilt only when its instances change [0025 B]. It starts at 4,096 slots (`TLAS_FIRST`) and is
  made anew at twice its need (power of two, at most 65,536) when instances and splits outgrow it; the G-buffer's and the
  bounce's bind groups are made anew with it (wgpu 30 walks every slot on a build) [0111]. Stills (a chunk's still models
  and plants) are instances of their own (mask `16 << body`, `Chunk::sto` the first still quad, custom index plus
  `STILL_INST`): only chunks first built within 128 m are split, they stay split until their mesh goes, at most 4096 and
  never past the slots the chunks leave free; shadow and lamp rays ask for them, the bounce and reflections do not
  [0104]. `frame.rs` asserts `PLANTS_FAR` stays under 120 m [0104].
- **Denoising at half resolution** (taste call: yes): history and smoothing at half resolution guided by the pixel each
  2x2 traced this frame; three `atrous` passes 1, 2, 4 half-pixels apart; the composite upsamples from the four nearest
  half-res values weighted by normal and plane [0025 C].
- **Visible draws sorted near to far** by `DrawChunk::dist` [0025 D1]. A depth pre-pass for opaque terrain does not pay
  [0103, 0111].
- **Lamps by place:** `World` keeps the light-giving edits in an index (`lights`) with a change counter; up to 1024
  lamps; every frame the client lists them in a grid of 32^3 cells of 4 m around the camera (binding 9), each cell a
  range into a list of lamps within 8 m; `light()` loops over its cell's list only [0025 H].
- **Merging inside a lattice:** `mesh` merges inside a lattice of `Body::lattice(level)` cells, 8 halved while a lattice
  cell's sag off the sphere would pass half a pixel at the nearest distance its level is drawn from at the lowest detail;
  the chunk info carries it (bits 16-17); `place()` interpolates drawn and traced vertices between exact lattice points;
  on by default (`greedy=0` draws every face) [0025 I2].
- **Eclipse terms off the pixel:** `eclipse_masks` works out once a frame which bodies can cover the sun's or the moon's
  disc from any point within 80 m (`AMP`) plus 100 m of another body's surface and writes them as bits (`n_lamps.z`,
  `.w`); `light()` calls `left_by` only for a flagged body [0025 J1].
- **Thin G-buffer, 16 bytes a pixel:** direct light `Rg11b10Ufloat`, albedo `Rgba8Unorm`, normal (octahedron, two 16-bit
  numbers) and distance (f32 bits) in one `Rg32Uint` (`gb_pack`, `gb_normal`); the feature `RG11B10UFLOAT_RENDERABLE` is
  required [0025 J2].
- **Plants' depth first** (`fs_still_depth`, `still_depth_pipeline`): still quads' depth after the cutout blocks' and
  before the terrain, then `fs_still` where equal; the ground under a plant is not lit [0103].
- **Fluff on open faces only:** the mesher puts cutout faces that open on air (bit 25) first and counts them
  (`Mesh::fluff` -> `DrawChunk::fluff`) [0103].
- **`vs_still`:** a plant past its reach returns at once; its normal is the cross of its two turned edges (`plant_turn`,
  `turned`) [0103].
- **Indexed quads:** one static index buffer (`quad_index`, 65536 quads, six indices a quad over four corners); vertex =
  quad * 4 + corner; draws are `multi_draw_indexed_indirect` over regions `IND_*` of `indirect` (20-byte args; the sea
  stays unindexed); `quad_draws` splits a run longer than the buffer; a swapped slope's corners are `quad_corner`'s
  3 4 5 0 and the slope's normal goes flat (`VOut.slope`) from each triangle's first corner; traced geometry keeps its six
  corners [0103].
- **Clouds:** a cell under `CL_OPEN` cover skips its hashes and `cl_soft` returns 0 at once; `cl_shadow` reads the wind
  from the same locate and texel as the cover [0104].
- **Looks in slots:** `hud::Looks` keeps the rects of a look drawn in a slot and lays them by moving them [0120].
- **Bots count the server's ticks, not wall time** (waits, a walk's give-up, a held dig); the tests' servers tick at
  20 Hz. A bot script's `wait s` is `s` seconds of ticks [0120].
- **Quad buffer** is 32 M quads by default (`QUAD_CAP_M`, `quad_cap_m=`) [0120].
- Run-to-run drift over an hour can exceed an effect: before/after is an **interleaved A/B** of two builds run alternately,
  median of three rounds; a 1440p run takes a copy of the developer's settings as `settings=` (2560 x 1440, detail 8,
  leaves fancy, clutter many, shadows soft). The window size of a run comes from the settings file's `width=`/`height=`
  [0111, 0120].

## Open (the batch list)

- **Sun rays once a texel, not once a pixel** (exact; the light is a texel's already): a pass that lists visible texels,
  traces them once and lets the G-buffer read them back, needs a deferred restructure. The next big GPU win (up to about
  5 ms of 5.5 in the forest at 1440p) [0111, 0120]. The G-buffer's direct light and its shadow ray per fragment is about
  half of what is left [0103].
- The vertex and raster share of the forest (3.3 ms at 640 x 360 with 2.6 M quads): fewer quads far off [0111].
- The LOD walk at detail 8: a ceiling per region so the walk makes no slots in the air; walking on a worker with the frame
  thread only culling (perf-debt, ADR 0105's row) [0120].
- After a remesh in place frees room in a full quad buffer, a still camera does not ask for pending chunks until something
  forces a walk (`Streamer::apply`: mark the walk dirty when a release clears `Alloc::full`) [0120].
- Frame thread: TLAS rebuild sets every instance each time, BLAS creation spikes (`BUILDS_MAX` 256 a frame), `table` and
  `submit`; ideas: set only changed instances, spread BLAS creation, grow by `update` [0102].
- `fly`'s burst of 9.5-11.6 ms frames 500-580 m into the flight since the thin G-buffer; the frame thread waits in
  `get_current_texture`; with `quad_cap_m` 12, 20 or 24 it does not appear; cause open, inside the budget [0025 J2].
- Rows left on the debt list: shadow rays through blades (about 1.5 ms in the meadow, 2 in the forest at 2560), fancy
  leaves' candidates (about 1 ms), the cloud march under partial cover (1.0-1.4 ms), `cl_shadow` (0.6 ms) [0104].
- `season-winter` view sits at its 0.5 % tolerance; accepting it is a look call [0120].
- Still over budget at 1440p, by the GPU (forest G-buffer about 12.6 ms) [0120].

## No longer holds

- 0004's budget as a gate on every feature: tempered by 0096 (a target a batch restores).
- 0011's gate of a 3-minute soak and four quick runs taking 1.5 minutes: the gate has fast and full tiers (CLAUDE.md).
- 0011's "dedicated server records by default": `record=1` only since 2026-11-12 (about 12 GB a day at 50 players).
- 0012's frame thread above normal priority: dropped. Profile-guided optimisation: not tried.
- 0025 A2 (submit before the acquire), G2 (BLAS compaction), M (third swapchain image, Mailbox, own pictures) and I's
  prototype: reverted or closed, none in the code.
- 0025 K's `vram_mb` guard: replaced by `gpu_alloc_mb`.
- 0025 D2 (`light()` as a compute pass): not done; the idea lives on as "sun rays once a texel".
- 0025's "open: the frame loop holds a core": closed by the frame cap in play.
- 0103's depth pre-pass for the terrain, fluff cut at a distance, local-array removal: tried, not kept.
- 0111/0120 tried and dropped: shared sun rays in a 2 x 2 quad, an alpha-only texel for ray candidates, dropping old BLAS
  a few a frame, keeping drawn bounds from the walk, a filter or a heap before dispatch's sort, a flood stop for the
  undercut, piles and painted boxes as greedy quads, `grid_items()` caching.
- Per-batch measurement tables of 0025, 0102, 0103, 0104, 0111, 0120: history only.
