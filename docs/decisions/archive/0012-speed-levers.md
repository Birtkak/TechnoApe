# 0012 – Cheap speed levers: a column cache, Fx for edits, mimalloc, low-priority workers

- **Status:** accepted
- **Date:** 2026-11-10

## Context
`docs/design/fundamentals.md` section 3 lists levers not pulled yet, each to be measured before it is
adopted; the roadmap adds a `huge` benchmark row for the 4 km planet (N = 12288) and the soak's physics, the
slowest part of the gate. The frame is already far inside ADR 0004 (p99 about 0.6 ms of 8.3), so what counts
here is worker throughput, the server's work per tick and frame spikes under load.

## Options considered
Each measured on its own build, in this order and on top of the one before: meshbench three times, a 3-minute
soak of 50 bots (seed 1, so the work is identical and its final hash must not change), and two reps of the
four quick runs. Priority as a run-time switch, A/B interleaved, calm and under 12 busy processes.

| Lever | Measured | Verdict |
|---|---|---|
| One terrain lookup per column in `phys::blocked` (it was one per cell) | soak 11.9 s -> 6.8 s, same hash | keep |
| Fx hasher for `World::edits` (SipHash before; the streamer already had Fx) | soak 6.8 s -> 4.2 s, same hash; client runs unchanged | keep |
| mimalloc as the global allocator (client, server, bot) | mesh time 51 -> 42 us per chunk, `fly` +2%, `orbit` +4% fps, RAM +1-5 MB | keep |
| Rayon workers below normal priority (`low_workers=0` turns it off) | calm: no change; under load, spikes per run 7, 9 -> 5, 6 (`orbit`) and 4, 4 -> 3, 3 (`fly`) | keep: no cost, a little gain |
| The frame thread above normal priority | under load no better (spikes 3, 6 and 5, 4) | drop |
| Profile-guided optimisation | not tried: a build script and a workload to keep in step | later |

## Decision
Keep the first four. The soak runs 2.8x faster (50 bots x 10 minutes: 44 s -> 15 s, the same final hash), the
gate 1 min 37 s -> 1 min 20 s; the client's numbers move a few percent, which the budget did not need but
costs nothing. The full series after (`docs/benchmarks.md`, 2026-11-10): all 24 reps on the 1 km and 4 km
planets hold ADR 0004 with no frame over 8.3 ms, worst frame 2.6 ms.

The developer found 25-30 minute benchmark series too long, so the series now runs at half length with 3
reps, the 1 km planet and the filmstrips by default (about 8 min; `base` and `huge` about 6 min each).

## Consequences
- mimalloc is a C dependency (built by `cc`); it builds on Windows with MSVC and will on Linux for the server.
- Under load, the frames over 8.3 ms all spend their time in `between_ms`, inside winit's loop between frames,
  and no thread priority changes that: how winit waits on Windows is the next place to look for the stall
  class of `docs/benchmarks.md`.
- The 4 km planet holds ADR 0004 (`huge` rows in `docs/benchmarks.md`), so the planet-size decision after
  the first playtest has headroom to 12288 columns per face.
