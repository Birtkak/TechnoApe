# CLAUDE.md – TechnoApe

First-person survival and automation game on small spherical planets of 0.5 m blocks, with a seamless climb
to orbit and over to the moon. Our own engine, the Pillars of Simulation Engine, in Rust + wgpu; multiplayer
from day one (a dedicated Linux server on the home server over Tailscale, singleplayer is a local server). Solo developer; Claude Code does
most implementation, the developer steers, plays and decides.

**Fresh start (2026-11-09).** An earlier flat-world game in Godot was dropped and deleted from the repo. Its
code, data and decisions do not apply; do not look for them or rebuild them from git history unless the
developer asks.

Ground truth: `docs/design/vision.md`; what the code must be able to do (feel, expandability, performance,
testing): `docs/design/fundamentals.md`; the ages: `docs/design/progression.md`. Decisions: `docs/decisions/`
(ADR 0001-0010, read them first).
Plan: `docs/plans/roadmap.md`. Prototype results: `prototypes/*/README.md`.

## Working principles

1. **Foundation over speed.** Spike alternatives in `prototypes/` and measure before committing.
2. **Every significant choice gets an ADR** (`docs/decisions/NNNN-title.md`, next free number, template
   `0000-adr-template.md`).
3. **Ask the developer only for taste, scope and direction calls.** Routine technical calls: a sensible
   default, recorded.
4. **Give yourself a check to run** before asking the developer to look: `cargo test` for logic, the headless
   run for behaviour and numbers, a filmstrip for visuals. Show the evidence. **Red, then green:** for a bug
   (F9 reports too) or new logic, write the test first and watch it fail for the right reason, then make it
   pass; a test never seen failing proves nothing. An optimisation keeps the slow path as its test's reference.
   A Stop hook (`tools/stop-check.sh`) runs `cargo test` when a session that edited `crates/` ends its turn.
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
12. **The author does not review.** Before a commit that changes engine, server or shader code, a fresh
    reviewer that did not write it reads the diff (`/code-review`, or a subagent given the diff and the task,
    not the author's summary). Fix or answer each finding; the ADR row says what the review found.

## Stack

- Rust 1.99.1 (rustup) with `CARGO_HOME=~/Tools/rust/cargo` and `RUSTUP_HOME=~/Tools/rust/rustup`; add
  `~/Tools/rust/cargo/bin` to PATH. wgpu 30, winit 0.30 (versions from the engine spike).
- Release builds for any performance number: fat LTO, one codegen unit, `target-cpu=native`.
- Libraries per system are chosen when the system is built, each in its own ADR.
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
  `run=stand|fly|orbit|edit`, `quick=1` (quarter length, a smoke check in seconds), `planet=12288|24576|1536`
  (columns per face edge: the standard 4 km planet, the huge 8 km one, a small one; ADR 0021), `seed=`, `out=file.jsonl`, `connect=host[:7777]` to join a running server
  (`technoape-server listen=0.0.0.0:7777 planet= seed=` prints a JSON line every 5 s).
- Saves (ADR 0016): `run=play` keeps `saves/seed-<seed>-planet-<n>.sav`, the server `saves/server.sav`; both
  carry on from it. `save=file` names another, `save=0` keeps none.
- Bots: `technoape-bot connect=host n=50 script="wait 1; dig 0.25 -0.5 0.25; block 0.25 -0.5 0.25; report"`
  runs windowless players on the protocol (commands: spawn, walk, wait, dig, place, block, report, quit;
  `script=-` takes them live from stdin). Output is one JSON line per report or block. Use bots, not a second
  windowed client, for multiplayer checks: two windows on one PC throttle each other (ADR 0006).
- Play: `technoape.exe run=play onscreen=1` (WASD, space; left click digs a block, right click places one;
  H the build hammer, B its shape, wheel its height; V the bucket, left scoops, right pours;
  in space the mouse pitches you freely and Q/E roll, gravity rights you as it pulls (ADR 0024); C held zooms 3x;
  1-9 the hotbar; space swims up in water, ADR 0020; T chats, / runs a command: /time, /playmode, /give,
  /goto, /timewarp (. and , step it), /help; Tab completes a command, the arrows pick, else the up arrow
  recalls the last line; in creative (/playmode creative) space twice flies, the wheel its speed up to
  1 km/s; F3 the overlay of
  numbers; F9 a bug report; Esc the menu: settings for resolution, display mode, field of view, mouse and
  detail, kept in `settings.txt`, ADR 0023). `acts="t:action,..."`
  scripts those inputs off-screen (ADR 0017; `say:/time set noon`, `f3`, `f9`, `type:text`, `tab`, `enter`, `esc`, `click:1`, `point:x:y`, `C`/`c` the zoom;
  `seconds=` ends a play run, `settings=out/x.txt` keeps the real settings out of it), and a
  filmstrip shows the result.
- **Builds for the developer** (double-click; without arguments the title menu opens on screen in the repository's
  folder, saves/ and bugs/: Singleplayer lists the saves and makes new worlds): `bash tools/playtest.sh build` refreshes `build/TechnoApe.exe` after every change
  that can be run; `bash tools/playtest.sh` refreshes `playtest/TechnoApe.exe` only when handing a version over
  to play-test. `target/` is Claude's own.
- **Bug reports:** F9 writes `bugs/<UTC time>/`: `report.md` (the developer's words, a run=stand line for the
  same view, the F3 state, the chat), `screen.png`, and `world.sav` when there was a save. When asked to look
  at bugs, read those first.
- To look at something: `run=stand at=x,y,z yaw=deg pitch=deg eye=m` stands anywhere on the planet, `time=s`
  sets the sky's clock (0 is 9 am at the spawn, a day is 1200 s; ADR 0022), `place=di:dj:dl:block,...` puts
  blocks (lamps, copper; `_` for a space) around the stand point;
  `capture_ms=2500 sheet=out/x.png` writes a filmstrip, `capture_w=1280` makes its frames large. Read the sheet.
- **The gate:** `bash tools/check.sh` (about 4.5 min, 5.5 after a release build; the soak and its replay run beside the tests and the fixed viewpoints: tests, a soak, its replay, the quick benchmark, the fixed
  viewpoints and a played run's feel report, ADR 0018) prints
  one JSON verdict; it is what "done" means (ADR 0011). **After every change: `bash tools/check.sh fast`**
  (80 s after a build: a 1 minute soak, `stand` and `fly` only; the verdict's `seconds` says where the time
  went); the full gate is for handovers. Soak alone: `technoape-server soak=1 bots=50 minutes=10
  seed=1` (a failure prints its repro line and writes `out/soak-fail.rec`). Sessions record to `out/rec/`;
  `technoape-server replay=file until=tick` replays one headless and dumps the players.
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
- GitHub: https://github.com/Birtkak/TechnoApe (private).
