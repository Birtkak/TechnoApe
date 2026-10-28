# Fable backlog

- **Written:** 2026-10-23
- **Purpose:** the jobs that need the strongest model (Claude Fable 5.1): a
  wrong call costs weeks, threads and budgets interact, or the job ends in an
  ADR. Everything that ends in JSON, a GDScript UI, or a script following an
  existing pattern goes to a cheaper model (list at the bottom).
- **How to start one:** "Read `docs/plans/fable-backlog.md` and do job N."
  Each job lists what to read first, what is already known, and what done
  looks like. Tick a job off here (and in CLAUDE.md's phase plan) when it lands.
- **2026-10-23:** jobs 1-4 (micro simulation, chisel, far field and near field of the brick world) are
  gone with the brick world (ADR 0033, 0036): one 12.5 cm grid, chiseling is the brush at size 1. Job
  numbers 5-8 are kept so older references still point at the right job.
- Next free ADR number: **0105** (0104 = world water is endless; the full list is `docs/decisions/`).

Recommended order: 5-7 as Phase 3/4 start. 8 is ongoing.

---

## 5. P4 server headless check and the delta protocol (Phase 4 groundwork)

ADR 0002 (Linux dedicated server, Docker on Unraid, Tailscale). Design first:
server-authoritative sim of bricks, dirty bricks as deltas (4 B uniform, 512 B
expanded, compressed), what clients predict. Spike: a headless server ticking
the bubble, one client receiving deltas, bandwidth and CPU measured. The
Unraid container and auto-update plumbing afterwards is cheaper-model work.

## 6. Wand and spell system design (Phase 3)

Vision: Noita wandcrafting, wands found never crafted, spells crafted, spells
act on the material simulation. Fable part: the ADR (a wand as a container of
modifiers and projectiles, cast order, mana and recharge, how a spell edits the
sim under the mass-edit rule), the native evaluator, and the data schema with
tests. Individual spells afterwards are data for a cheaper model.

## 7. Machine, energy and fluid network architecture (Phase 3/4)

GregTech depth: machines as block entities (ADR 0007 layer), power networks,
pipes, multiblocks. Fable part: the ADR and the network solver (native if it
iterates per block), tick budget, how machines keep a sim bubble alive, save
and sync. Each machine afterwards is data.

## 8. Budget regression hunts (ongoing)

Whenever an ADR 0014 scenario leaves the ADR 0009 budget. Mind the benchmark
pitfalls: wall clock not delta, no console prints, one Godot on the machine
(`other_godot_processes == 0`).

---

## Not Fable: hand to a cheaper model

- Recipe browser depth (machine recipes, bookmarks).
- ADR 0019's remaining progression systems once its rules are written.
- More content passes (ADR 0024 did tree kinds, strata, sea ice, ice spikes,
  basalt, ferns; atlas 8x10 with tiles 73..79 free): per-wood planks, saplings,
  birch, item sprites for new tools.
- New spells and machines once jobs 6 and 7 exist.
- Playtest tools, chat commands, bug-report triage.
- Server deployment on Unraid, auto-update, Linux CI.
- Dungeons generator (parked), following the world-gen patterns.
