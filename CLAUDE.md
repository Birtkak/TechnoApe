# CLAUDE.md – TechnoApe

First-person survival and automation game on small spherical planets of 0.5 m blocks, with a seamless climb
to orbit and over to the moon. Our own engine in Rust + wgpu; multiplayer from day one (a dedicated Linux
server on the home server over Tailscale, singleplayer is a local server). Solo developer; Claude Code does
most implementation, the developer steers, plays and decides.

**Fresh start (2026-11-09).** An earlier flat-world game in Godot was dropped and deleted from the repo. Its
code, data and decisions do not apply; do not look for them or rebuild them from git history unless the
developer asks.

Ground truth: `docs/design/vision.md`. Decisions: `docs/decisions/` (ADR 0001-0005, read them first).
Plan: `docs/plans/roadmap.md`. Prototype results: `prototypes/*/README.md`.

## Working principles

1. **Foundation over speed.** Spike alternatives in `prototypes/` and measure before committing.
2. **Every significant choice gets an ADR** (`docs/decisions/NNNN-title.md`, next free number, template
   `0000-adr-template.md`).
3. **Ask the developer only for taste, scope and direction calls.** Routine technical calls: a sensible
   default, recorded.
4. **Give yourself a check to run** before asking the developer to look: `cargo test` for logic, the headless
   run for behaviour and numbers, a filmstrip for visuals. Show the evidence.
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

## Stack

- Rust 1.99.1 (rustup) with `CARGO_HOME=~/Tools/rust/cargo` and `RUSTUP_HOME=~/Tools/rust/rustup`; add
  `~/Tools/rust/cargo/bin` to PATH. wgpu 30, winit 0.30 (versions from the engine spike).
- Release builds for any performance number: fat LTO, one codegen unit, `target-cpu=native`.
- Libraries per system are chosen when the system is built, each in its own ADR.
- The engine starts from the p15 spike's renderer and streamer, branch `claude/p15-engine-spike-251147`
  (roadmap step 1); until then there is no game code in the repo.

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
