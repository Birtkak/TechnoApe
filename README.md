# TechnoApe

A first-person survival and automation game on small spherical planets of 1 m blocks, built on its own
engine in Rust + wgpu with an AI-assisted workflow.

Status: **in development (fresh start 2026-11-09).** The first playable is done: Terra and Luna are baked from a
seed, with rivers, tides, weather, plants and animals, survival and an inventory. Machines, crafting and the
journey to orbit are next (`docs/plans/roadmap.md`). The story is in `docs/design/lore.md`.

## Layout

| Path | Purpose |
|------|---------|
| `CLAUDE.md` | Working agreement between the developer and Claude Code |
| `docs/design/vision.md` | What the game is: the ground truth |
| `docs/decisions/` | Architecture Decision Records, one file per decision |
| `docs/plans/roadmap.md` | What is done and what comes next |
| `crates/engine`, `crates/server`, `crates/bot`, `crates/client` | The game: world, meshing, streaming and protocol; the authoritative server; scripted bots; window, renderer and benchmark runs |
| `tools/` | Benchmark scripts |
| `prototypes/` | Throwaway spikes, each with a README of its question and measured answer |
