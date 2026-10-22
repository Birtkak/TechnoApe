# 0009 – Drop Android; the dev PC is the reference machine

- **Status:** accepted
- **Date:** 2026-10-22
- **Amends:** ADR 0002 (targets, updates), 0003 and 0004 (Android budgets)

## Context
Request, 2026-10-22: "Let's remove the android version from this since it will
limit us. Drop android completely, but instead I want performance to still be
good on older computers, meaning it should run perfect on mine."

Android shaped several earlier calls: small simulation and render budgets
for phones, an APK update path, touch input, an Android export smoke test,
and keeping C# out of the shipped game because its Android export is
experimental. The dev box is a 2020-era mid-range gaming PC, so "older
computers" and "the dev PC" describe the same bar.

## Decision
- **Targets:** Windows x86_64 client; Linux x86_64 dedicated server (Docker
  on Unraid over Tailscale) unchanged. No Android client, no APK, no touch
  input, no Android SDK on the dev box.
- **Reference machine = the dev box:** Ryzen 5 5600X (6c/12t), RTX 3070 8 GB,
  32 GB RAM at 2666 MHz (XMP off), Windows 11. "Runs perfect" means, at
  default settings, with vsync off, in every standard benchmark run
  (autopilot fly, walk, mine, stations, browser):
  - p99 frame time ≤ 8.3 ms (120 fps class), and no single frame > 16.7 ms
    once the start-up load has finished (no visible hitch at 60 Hz);
  - process RAM ≤ 4 GB, VRAM ≤ 6 GB;
  - every system's benchmark reports its numbers on this machine, and a
    regression past the budget is a bug.
- **Weaker PCs** are served by the same settings knobs (render, view and
  simulation distance, LOD count, tick rate, shadows) with lower presets;
  there is no second platform budget to design around.
- **Language rule unchanged:** GDScript orchestrates, C++ GDExtension does the
  per-voxel work (ADR 0003). The Android reason for avoiding C# is gone, but
  the rule also rests on the measured two-orders-of-magnitude gap for voxel
  loops and on one toolchain; C# stays tooling-only unless a later ADR
  reopens it.

## Consequences
- Budgets can be set for desktop: the simulation bubble, render distance and
  LOD horizon are tuned against the reference machine, not a phone.
- Input stays action-based (good practice, gamepad later), but no touch layer.
- Auto-update ships a Windows build and the server image only.
- CLAUDE.md, vision.md and the platforms doc no longer list Android; ADR 0002
  keeps its history with a pointer here.
