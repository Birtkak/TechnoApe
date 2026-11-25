# CLAUDE.md – TechnoApe

First-person survival and automation game on small spherical planets of 1 m blocks (ADR 0033), with a seamless climb
to orbit and over to Luna. Our own engine, the Pillars of Simulation Engine, in Rust + wgpu; multiplayer
from day one (a dedicated Linux server on the home server over Tailscale, singleplayer is a local server). Solo developer; Claude Code does
most implementation, the developer steers, plays and decides.

**Fresh start (2026-11-09).** An earlier flat-world game was dropped and deleted from the repo. Its
code, data and decisions do not apply; do not look for them or rebuild them from git history unless the
developer asks.

Ground truth: `docs/design/vision.md`; what the code must be able to do (feel, expandability, performance,
testing): `docs/design/fundamentals.md`; the ages: `docs/design/progression.md`. Decisions: the topic documents in
`docs/decisions/` state what holds today (start with `vision-and-scope.md`); `docs/decisions/INDEX.md` resolves
any "ADR NNNN" in code or docs to its topic and state; the numbered records are in `docs/decisions/archive/`.
Plan: `docs/plans/roadmap.md`. Prototype results: `prototypes/*/README.md`.

## Working principles

1. **Foundation over speed.** Spike alternatives in `prototypes/` and measure before committing.
2. **Decisions live in the topic documents** (`docs/decisions/*.md`, what holds today). A choice that is hard to
   undo or was the developer's call (taste, scope, a format on disk or on the wire, a library, a rule of the
   world) is recorded there as a dated entry under its topic, in the form of `0000-adr-template.md`, and
   given the next number so code can cite it ("ADR 0168"); the index gets its line, and what the entry
   replaces moves to the topic's "No longer holds". Every other choice is recorded in its commit body. No
   new file per decision: a branch writes its entry in the topic document and takes its number at merge.
3. **Ask the developer only for taste, scope and direction calls.** Routine technical calls: a sensible
   default, recorded.
4. **Give yourself a check to run** before asking the developer to look: `cargo test` for logic, the headless
   run for behaviour and numbers, a filmstrip for visuals. Show the evidence. **Red, then green:** for a bug
   (F9 reports too) or new logic, write the test first and watch it fail for the right reason, then make it
   pass; a test never seen failing proves nothing. An optimisation keeps the slow path as its test's reference.
   A Stop hook (`tools/stop-check.sh`) runs the tests of the crates a session edited, and of those that depend
   on them, when it ends its turn.
5. **The design doc wins over code** when they disagree; fix one of them explicitly.
6. **Data-driven content:** items, recipes, machines, materials and biomes are data files with a schema and
   tests.
7. **Server-authoritative from the first version:** simulation separate from presentation, deterministic
   from the seed, deltas over the wire.
8. **Never stall the frame thread.** Streaming, meshing, simulation and saving run on worker threads; the
   frame thread swaps finished work in under a per-frame budget. Every system ships with a benchmark.
9. **Budget (ADR 0004):** on the dev PC at default settings, p99 frame <= 8.3 ms, no frame > 16.7 ms after
   load, RAM <= 8 GB, VRAM <= 6 GB. Always pick the discrete GPU and log it.
10. **Debug tools are for Claude too.** Everything about the running game is reachable without eyes: JSON
    stats on stdout, logs, state dumps, filmstrips. Bots are ordinary clients that tests drive by script or
    live. Never judge motion from one screenshot.
11. **Token budget.** Offload with the `free-offload` skill before reading big files, searching widely or
    writing boilerplate; keep design, engine work and final verification.
12. **The author does not review.** Before a branch that changes engine, server or shader code is merged, a
    fresh reviewer that did not write it reads its diff once (`/code-review`, or a subagent given the diff and
    the task, not the author's summary). Fix or answer each finding; the merge commit says what the review found.
13. **The tool tier is lighter** (the developer, 2026-11-12: too much waiting on checks that did not matter).
    A change that touches only `crates/studio`, `tools/` and `docs/` needs its tests and, for something seen,
    a shot that was read; red then green still holds. It gets **one decision entry per round**, not per step, and a
    fresh reviewer only when the diff is over about 300 lines. Such a branch is **merged into main without
    asking** once its tests pass, `build/` is refreshed, and the developer is told after. Anything in
    `crates/engine`, `crates/server`, `crates/client` or a shader keeps principles 2 and 12 in full;
    principle 15 says when it is merged.
14. **Features first, speed in batches** (the developer, 2026-11-13; ADR 0096). While features are added or
    changed, do not optimise them: build it, get the look and behaviour right, measure its cost **once** and
    write the row in `docs/perf-debt.md` (what it costs on which run, CPU and GPU apart, and the ideas to make
    it cheaper). A feature over the budget of principle 9 is still merged; its commit accepts the cost
    (`bash tools/check.sh accept-perf`) and names its row. Speed is then won back in an **optimisation batch**:
    measure, optimise, measure, down the debt list, until principle 9 holds again. Never left for the batch:
    a stalled frame thread (principle 8), a crash, a leak, a soak that fails, or a frame so slow the game cannot
    be judged by playing (about 20 ms p99): those are bugs.
15. **Merge early, one writer an area** (the developer, 2026-11-16, after a week of 144 merges, 31 of them
    with conflicts). An engine, server, client or shader branch is merged into main once its tests and the fast
    gate pass and its review is answered. It does not wait for the developer, who plays `build/` after and has
    what they dislike reverted; a taste call still open goes behind a setting or an argument (as `warp=` did).
    A branch lives hours, not days, and takes main in before it is merged. **One session at a time writes in an
    area** (the renderer: `crates/client`'s frame, gpu and shaders; the server; the world: `crates/engine`'s
    world, stream and mesh); sessions run side by side only across areas (Studio, data, tools, docs). The view
    baselines and `tests/perf/baseline.json` are accepted on main after the merge (`.gitattributes` keeps
    main's in a merge; `git config merge.ours.driver true` once a clone). A feature's keys and commands go in
    `docs/playing.md`, not in this file.

## Stack

- Rust 1.99.1 (rustup) with `CARGO_HOME=~/Tools/rust/cargo` and `RUSTUP_HOME=~/Tools/rust/rustup`; add
  `~/Tools/rust/cargo/bin` to PATH. wgpu 30, winit 0.30 (versions from the engine spike).
- Release builds for any performance number: fat LTO, one codegen unit, `target-cpu=native`. The one exception:
  the gate's speed guard runs the `play` build (no LTO) against its own baseline, so the gate needs no fat-LTO link.
- Libraries per system are chosen when the system is built, each with its own decision entry.
- Workspace: `crates/engine` (the Pillars of Simulation Engine: world on the cube-sphere grid, meshing, streaming, the player protocol; no
  window or GPU), `crates/server` (the authoritative world; binary `technoape-server`), `crates/bot` (windowless scripted
  players; binary `technoape-bot`) and `crates/client`
  (binary `technoape`: window, renderer, benchmark runs; runs the server on a thread unless `connect=` names
  a remote one). ADR 0006. Techniques and why:
  `docs/research/voxel-rendering.md`.

## Build, test, run

- `cargo test` (engine logic), `cargo build --release` (the binary lands in `target/release/technoape.exe`).
- A run is `technoape.exe key=value ...`, off-screen unless `onscreen=1`. It prints one JSON line (ADR 0004:
  frame times, RAM, VRAM, `budget_ok`, a `spikes` list) and exits:
  `run=stand|fly|orbit|edit|sim|fx` (`fx` keeps `fx_n=` particles alive, ADR 0031), `quick=1` (quarter length, a smoke check in seconds), `planet=6144|12288|768`
  (columns per face edge: the standard 4 km planet, the huge 8 km one, a small one; ADR 0021), `seed=`, `out=file.jsonl`, `connect=host[:7777]` to join a running server
  (`technoape-server listen=0.0.0.0:7777 planet= seed=` prints a JSON line every 5 s).
- A new world's planet is baked (ADR 0043, 0060: about 64 s for the 4 km planet on the `play` build, ADR 0204, once per seed and planet; kept in
  `saves/bakes/`); `World::new` is the old noise land (tests, saves from before the bake), `World::baked` the map.
- Saves (ADR 0016): `run=play` keeps `saves/seed-<seed>-planet-<n>.sav`, the server `saves/server.sav`; both
  carry on from it. `save=file` names another, `save=0` keeps none.
- Bots: `technoape-bot connect=host n=50 script="wait 1; dig 0.25 -0.5 0.25; block 0.25 -0.5 0.25; report"`
  runs windowless players on the protocol (commands: spawn, walk, wait, dig, place, block, report, quit; items: slots, move a b n, drop a n, take dx dy dz k a n, put a dx dy dz k n, grab <item> a n; pick dx dy dz takes a pick-up, or cuts a plant there, ADR 0082; hold k the hotbar slot in hand, strike dx dy dz strikes it on a block (knapping), ADR 0110;
  `script=-` takes them live from stdin). Output is one JSON line per report or block. Use bots, not a second
  windowed client, for multiplayer checks: two windows on one PC throttle each other (ADR 0006).
- Play: `technoape.exe run=play onscreen=1`. Every key, chat command and the `acts=` script that plays them
  off-screen (ADR 0017): `docs/playing.md`. A feature adds its keys and commands there.
- **Builds for the developer** (double-click; without arguments the title menu opens on screen in the repository's
  folder, saves/ and bugs/: Singleplayer lists the saves and makes new worlds): `bash tools/playtest.sh build` refreshes `build/TechnoApe.exe` after every change
  that can be run; `bash tools/playtest.sh` refreshes `playtest/TechnoApe.exe` only when handing a version over
  to play-test. Both are built with the `play` profile (fast to link, for trying things); `--release` with
  its fat LTO is only for performance numbers. `target/` is Claude's own.
- **The Studio** (`technoape-studio`, `crates/studio`, ADR 0044 and 0046; handed over as `build/TechnoApeStudio.exe`):
  paints `data/textures/*.png` from `data/palette.toml`. Headless, one JSON line: `check=1` (palette and every
  texture; exit 1 names file and pixel), `drafts=1` (`data/drafts.txt`), `render=<png> sheet=out/x.png` (3 x 3),
  `shot=out/x.png [open=<block>] [page=palette]` (one frame of the window off-screen; read it, never open the window),
  `map=<name> out=out/x.png [frame=k]` (a map of `data/maps/` drawn, an animated one as a strip; ADR 0153).
  **Tuning** (ADR 0171): `tune=-` is a session on stdin, one JSON line a command, no window: `creature boar`,
  `set swing 28`, `gait run`, `sweep swing 10 50 9`, `film out/x.png`, `save` (the walk by numbers: foot slide, drift,
  sink, hover); `hand` chooses the swing of `data/hand.toml` instead (ADR 0172: strike time, travel, speed); use it,
  not shots, to tune a gait or the swing.
- **Bug reports:** F9 writes `bugs/<UTC time>/`: `report.md` (the developer's words, a run=stand line for the
  same view, the F3 state, the chat), `screen.png`, and `world.sav` when there was a save. When asked to look
  at bugs, read those first.
- To look at something: `run=stand at=x,y,z yaw=deg pitch=deg eye=m` stands anywhere on the planet, `time=s`
  sets the sky's clock (0 is 9 am at the spawn, a day is 1800 s; ADR 0022, 0041), `place=di:dj:dl:block,...` puts
  blocks (lamps, copper; `_` for a space) around the stand point; effects play only in `run=play` unless
  `fx=1` (ADR 0031; `fx_cap=` the pool, the Particles setting few/normal/many), and
  `fxedit=t:di:dj:dl:block,...` (`air` to dig, `dig` a survival dig by the crack rule, ADR 0094) edits cells at t seconds to film a break or a place;
  `fxplay=name[:n]` starts a lasting effect or group (`fire`, `smoke`, ...) n times on the ground ahead and
  `model=name@state` plays a model's state (clip, effects, sound, light; ADR 0050); `data/effects.toml` reloads live (`effects=file` another; `bash tools/reload-check.sh` times all four kinds' reloads end to end); `data/blocks.toml`, `items.toml`, `creatures.toml` and `herds.toml` reload live too (ADR 0134: a row changed or added, never removed, renamed or moved; `python -I tools/live-check.py` times it);
  `capture_ms=2500 sheet=out/x.png` writes a filmstrip, `capture_w=1280` makes its frames large. Read the sheet.
- **The gate:** `bash tools/check.sh` (about 4.5 min, 5.5 after a release build; the soak and its replay run beside the tests and the fixed viewpoints: tests, a soak, its replay, the quick benchmark, the fixed
  viewpoints and a played run's feel report, ADR 0018) prints
  one JSON verdict; it is what "done" means (ADR 0011). **After every change: `bash tools/check.sh fast`**
  (95 s after a build: a 1 minute soak, `stand`, `fly` and `sim` only; the verdict's `seconds` says where the
  time went); the full gate is for handovers. Both tiers hold every benchmark run to `tests/perf/baseline.json`
  (`tools/perf.py`, ADR 0025 K: the frame thread's own work, the GPU stages, job time, VRAM, sim's tick); a
  change slower on purpose runs `bash tools/check.sh accept-perf` and its commit says why. **Sharing the PC**
  (ADR 0096): `bash tools/slot.sh job <command>` for a build, a test run, a shot or a film (two at a time);
  `bash tools/slot.sh measure <command>` for a speed run or soak whose numbers are kept (alone, holds `out/.lock`);
  the gate is such a run (`bash tools/slot.sh measure bash tools/check.sh fast`); `bash tools/slot.sh quiet bash tools/bench.sh` for the benchmark, which takes the lock itself.
  One compiler cache serves every worktree (sccache, in the cargo home's `config.toml`, with 6 jobs a cargo). Soak alone: `technoape-server soak=1 bots=50 minutes=10
  seed=1` (a failure prints its repro line and writes `out/soak-fail.rec`). A dedicated server records to `out/rec/` with `record=1` (or `record=file`), else not;
  `technoape-server replay=file until=tick` replays one headless and dumps the players.
  `technoape-server report=1 seed= planet=` prints what the ground holds (ADR 0063): provinces, caves, heat, every
  mineral's bodies, blocks, ingots against `ore-budget.md`, nearest distance and depth, and `made_hash`; Luna's the same under `luna` (her provinces, baked tubes, caves, minerals).
- **The home server** (ADR 0130): `bash tools/deploy.sh [all|server|client]` builds the committed source in Docker
  on the box and restarts the container (`docker stop` saves first: `stop` on the server's stdin), and builds the friends'
  client (`target/friends`, any AVX2 CPU) into a zip whose `Join Tower.bat` updates itself from the page; the page
  `http://100.124.84.125:7780` (Tailscale) shows the server's output and the download. `python -I deploy/web_test.py`.
- Viewpoints: `bash tools/views.sh` renders `tests/views/views.txt` and compares by number (cracks, pixels);
  after a deliberate visual change, look at `out/views/*.png`, then `bash tools/views.sh accept`.
- Full benchmark: `bash tools/bench.sh` (about 8 min for the 4 km planet and filmstrips; add `base` or `huge` for
  the other planets, about 6 min each; repeats reps a compiler disturbed), then
  `python -I tools/summarize.py`. Results: `docs/benchmarks.md`. Do not compile while it runs.

## Conventions

- Dev box: Windows 11, Ryzen 5 5600X, RTX 3070 8 GB, 32 GB RAM. Shell: PowerShell or Git Bash. From Git Bash,
  set `MSYS_NO_PATHCONV=1` before passing arguments that start with `/`.
- Runs of the game or benchmarks go off-screen or headless; kill runs that hang. One benchmark series at a
  time: two GPU-heavy processes spoil each other's numbers.
- Commits: imperative subject, the body explains why. When another session shares this checkout, stage files
  explicitly, never `git add -A`.
- Prototypes are throwaway: one folder each, its README states the question and the measured answer.
- Tools live under `~\Tools\` (the user's home), each in its own folder; the repo never names a person or a
  machine, only this home-relative layout.
- GitHub: https://github.com/Birtkak/TechnoApe.
