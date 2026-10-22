# 0002 – Platform targets, hosting and update distribution

- **Status:** accepted (targets and hosting); language consequence provisional
  until P2 numbers are in. **Amended by ADR 0009 (2026-10-22): Android
  dropped; Windows client + Linux server only.**
- **Date:** 2026-10-21

## Context
The project wanted an Android APK client alongside Windows, a dedicated server on
the household Unraid box reachable over Tailscale for co-op, and updates
pushed to clients automatically. Details: `docs/design/platforms-and-deployment.md`.

## Options considered
1. **Windows-only client, LAN co-op** – simplest; rejected, Android is wanted.
2. **Clients + dedicated Linux server in Docker on Unraid over Tailscale** –
   fits the hardware at hand, no public exposure, no NAT traversal.
3. **Peer-hosted co-op (one player hosts)** – no server box needed, but the
   host must stay online and Android as host is impractical. Rejected as the
   primary model; could be a fallback mode later.

For the scripting language under an Android requirement:
- **C#** – strong tooling, but Android export is experimental in Godot 4.7
  docs and web export is impossible.
- **GDScript + native (godot_voxel / GDExtension C++)** – exports everywhere;
  GDScript speed must be proven adequate for glue (P2).

## Decision
- Targets: Windows x86_64 client, Android arm64 client, Linux x86_64 dedicated
  server (Docker on Unraid), all from one Godot project.
- Networking: Tailscale tailnet, server-authoritative, Godot high-level
  multiplayer; deterministic worldgen so only deltas cross the wire.
- Updates: CI-built artifacts, a version manifest served from the Unraid
  box, in-client update check; APK via system installer prompt.
- Language: **prefer GDScript for logic + godot_voxel/GDExtension for hot
  loops**; C# only if P2 shows GDScript cannot carry the glue layer *and*
  C# Android export is validated on a device.

## Consequences
- Simulation/presentation split and a command/delta boundary from Phase 1.
- Settings-driven performance budget (view distance, LOD) per platform.
- Input via actions only.
- Need Android SDK + JDK 17 + export templates on the dev box, and a CI
  pipeline, before Phase 4; an APK smoke export earlier so it never rots.
- Must verify godot_voxel works in a headless dedicated-server build.
