---
name: overseer
description: Run a batch of TechnoApe bugs/features as parallel agents in their own worktrees, with this session as overseer that briefs, reviews and merges. Use when the developer hands over a list of bug reports or features and asks to split them up, run them in parallel, or "oversee" the work.
---

# Overseer workflow

The developer gives a list (usually F9 bug reports). This session reads only the report
headers, groups the work, starts one agent per group, reviews each branch when it reports and
merges it into local `main`. It never does the fixes itself. Lessons below come from the
2026-10-26 run (9 tasks, ~2.5 h wall clock, 180k-380k tokens per agent).

## 1. Read and group (5 minutes, not more)

- List reports newest first and read only the "What went wrong" line of each:
  `for d in $(ls -t "$APPDATA/Godot/app_userdata/TechnoApe/bug_reports" | head -40); do ...; head -c 400 report.md; done`.
  Agents open the full report themselves.
- Group by **files touched**, not by theme. Two agents on the same file cost a merge
  conflict and a re-test. Generator work (`fine_gen.cpp`, `world_gen.cpp`) is one hot spot:
  terrain shape, ruins and ore/biome looks all landed there.
- Leave out what the developer reserved (e.g. liquid simulation for Fable) and say so in every brief.

## 2. Assign shared numbers up front

Every brief names what that agent may use, so merges need no renumbering:
- ADR numbers: `ls docs/decisions | tail -3`, then hand out the next ones, one per agent.
- `GEN_VERSION` (`game/scripts/generator.gd`): only one generator agent bumps it, or the
  overseer bumps it once at merge time. Tell the others not to touch it.
- Material ids / atlas tiles: give each agent that adds materials its own range from the reserve.

## 3. Pick the runner

- **Desktop app, typed message:** `start_session` per group (own worktree, visible to the developer).
- **Remote Control:** sessions cannot be started; use background `Agent` calls with
  `isolation: "worktree"`. Start them **one call per message**: parallel worktree creation
  raced and failed once (`commondir` error).
- Mechanical or small UI tasks: pass `model: "sonnet"` to the Agent call. Keep Opus for native,
  generator, rendering and perf work.

## 4. Order the waves by what they measure

At most 4 agents at once (each runs Godot and builds). Visual and logic tasks go first.
Frame-time work (performance, streaming, teleport) goes last, or alone, because parallel Godot
runs make its numbers noise. If the developer is waiting, start it anyway and require
back-to-back before/after medians.

## 5. Brief template

Each brief is short and holds: the reports (folder names), the task, the files/area, the
reserved numbers, scope fences, and this fixed block:

```
SETUP: run `bash tools/agent_worktree_setup.sh` (add `--native` if you build C++) first. It
moves the branch onto local main, copies the gitignored DLLs and gdUnit4 bin, and imports.
Build C++ with the GODOT_CPP_PATH it prints. Keep scratch files in your worktree, not the
shared scratchpad. `/shot` may need `--render-thread safe`.
CHECKS: run only the test files you touched or that cover your area; the overseer runs the
full suite after merging. Known failures on main: <list them>. Evidence: one before/after
contact sheet, not a folder of shots. Do not chase failures outside your area.
OUTPUT: commit on your branch (imperative subject, why in the body, Co-Authored-By line);
do not merge or push. Final report under 25 lines: branch, commits, changes, evidence path,
numbers/ids used, open issues.
```

## 6. Review and merge each branch as it lands

- `git diff --stat main...<branch>`, open the evidence sheet, then
  `git merge --no-edit refs/heads/<branch>` (full ref: a tag of the same name would win).
- After merging anything native: rebuild in `prototypes/p5-sim-spike`, copy the DLL to
  `game/addons/technosim/bin/`, run the full gdUnit suite once, and compare against the known
  failures.
- When a later branch overlaps a running agent's files, `SendMessage` that agent the new
  main commit and what changed so it merges main itself; it knows its own change best.
- Send the developer the evidence sheet (`SendUserFile`) when they are remote.

## Time sinks seen and their fix

| Lost time | Fix |
|---|---|
| Worktrees cut from `origin/main`, 59-69 commits behind local main; every agent reset by hand | setup script; or push main before a batch |
| Each agent copied DLLs and gdUnit bin, re-imported, hit missing `class_name` | setup script |
| Parallel builds raced on shared `godot-cpp/gen` | `--native` private copy |
| Every agent ran the full 470-case suite several times | targeted tests; overseer runs the full suite once |
| Agents investigated the same 4 pre-existing failures | list known failures in the brief |
| Stale Godot runs from finished agents held DLLs and skewed timings | pass `duration=` to every autopilot run; the overseer cannot kill them under auto mode, so tell the developer |
| Shared scratchpad: one agent overwrote another's script | worktree-local scratch |
| ADR / GEN_VERSION / id clashes at merge | assign them in the briefs |
