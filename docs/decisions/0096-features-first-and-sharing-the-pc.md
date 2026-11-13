# 0096 – Features first, speed in batches; sharing the PC

- **Status:** accepted (the developer, 2026-11-13)
- **Date:** 2026-11-13

## Context
Five sessions shared one PC behind one lock (`out/.lock`) that every build, test and measurement took, and
each feature was tuned against the frame budget before it could merge: change, test, a millisecond too heavy,
optimise, test again. Most of an afternoon's waiting was the queue for the lock, and most of the lock's time
was measuring. The developer: "when adding and changing features we should leave optimizations be so we can
work through the problems and upgrades more rapidly and when everything is said and done we do an
optimization batch"; and: install a compiler cache, let the PC run several jobs when it is not testing for
speed, without freezing or timing out.

## Options considered
1. Keep one lock and the budget per feature. Safe numbers, slow rounds.
2. No lock at all. Ten sessions at once killed each other's test runs on 2026-11-12.
3. **Two slots for builds, tests and pictures; the PC alone only for kept speed numbers; a feature measured
   once and its cost written down, optimised later in a batch.**

## Decision
Option 3.
- **Features first** (CLAUDE.md principle 14): a feature is built and made right, measured once, its cost
  written in `docs/perf-debt.md`, and merged even over ADR 0004's budget with the cost accepted in the gate's
  baseline. An optimisation batch then works the list down until the budget holds. Never deferred: a stalled
  frame thread, a crash, a leak, a failing soak, or a frame too slow to judge the game by playing (about
  20 ms p99).
- **Sharing the PC** (`tools/slot.sh`): `job` takes one of two slots (`out/.job1`, `out/.job2`) and waits
  while `out/.lock` is held; `measure` takes `out/.lock` and waits for the slots to empty; `quiet` waits for
  both and runs a tool that takes the lock itself (the gate, the benchmark). A slot older than 30 minutes (the
  lock: 45) is taken over. The slots live in the main checkout's `out/`, for every worktree.
- **No freeze:** the cargo home's `config.toml` gives a cargo 6 of the PC's 12 threads (`jobs = 6`) and a test
  binary 6 threads, so two jobs fill the PC and no more.
- **One compiler cache:** sccache 0.19.0 in `~/Tools/sccache`, as cargo's `rustc-wrapper`. It reuses
  dependencies and unchanged crates between worktrees; a crate built incrementally (the workspace's own in the
  test profile) is not cached, and the wrapper made each worktree rebuild its own crates once.

## Consequences
- ADR 0004's budget is a target the batch restores, not a gate on each feature; `budget_ok` may be false on
  main between batches. The gate's speed guard (ADR 0025 K) still catches a cost nobody meant.
- Sessions stop iterating on cost: flora's crowns and plants (ADR 0090, 0091) merge on their look, with a row.
- The Stop hook's test run takes no slot (it must not wait past its time limit); with 6 jobs a cargo a third
  job slows the PC but does not stop it. shortcut: give the hook a slot if tests start timing out again.
- Speed numbers taken while a `job` runs are worthless: only `measure` and `quiet` runs are kept.
