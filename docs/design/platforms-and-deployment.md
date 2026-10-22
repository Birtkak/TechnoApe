# Platforms, hosting and updates

- **Date:** 2026-10-21 (requirements from Brent)
- **Status:** requirements captured; implementation is Phase 4+. Everything
  built before then must not block this.

> **2026-10-22: Android dropped (ADR 0009).** Windows client + Linux server
> only; the dev PC is the reference machine. Android notes below are history.

## Requirements (Brent, 2026-10-21; Android removed 2026-10-22)
1. ~~Android client (APK)~~ dropped (ADR 0009).
2. **Dedicated server on the Unraid box**, reached over **Tailscale**, so Brent
   and friends play together on a private network.
3. **Auto-updates pushed from the server to the clients**, so nobody installs
   builds by hand after the first one.

## Target matrix

| Target | Role | Export | Notes |
|---|---|---|---|
| Windows x86_64 | Primary client, dev box | Godot Windows template | Full view distance, keyboard/mouse |
| Linux x86_64 | Dedicated server | Godot Linux template, "Export as dedicated server" | Runs `--headless` in a Docker container on Unraid |
| Linux client | Nice-to-have | Same as server template without the server flag | Not planned |

## Android consequences for the foundation (history: dropped in ADR 0009)
- **Scripting language.** Godot's C# export to Android is still marked
  *experimental* in the current docs (bionic runtime, no JNI, some APIs
  crash), and C# cannot export to web at all. GDScript and GDExtension (C++)
  export to every platform. So the foundation should be **GDScript for game
  logic + native (godot_voxel / GDExtension) for hot loops**, and C# only if
  P2 proves GDScript unworkable *and* the Android C# export is verified on a
  real device first.
- **godot_voxel** ships Android arm64 GDExtension builds, so the terrain tech
  stays the same on phone and PC.
- **Performance budget** must be a setting, not a constant: view distance,
  chunk mesh LOD, shadow quality, and tick rates scale per platform.
- **Input** abstraction from day 1 (actions, not raw keys) so touch and
  gamepad map cleanly.
- **Verification:** an Android export (even a stub scene) runs in CI early so
  the APK path never rots.

## Dedicated server on Unraid over Tailscale
- **Process:** Godot Linux "dedicated server" export (no renderer, smaller
  binary) in a Docker image: `debian-slim` + server binary + `.pck`.
  World saves on a mounted volume (`/data`). Unraid runs it as a regular
  container; a Compose file or Unraid template lives in `deploy/`.
- **Network:** Tailscale on the Unraid host (or a sidecar). The server binds
  to the tailnet address; clients connect by MagicDNS name, e.g.
  `technoape.<tailnet>.ts.net:7777`. No port-forwarding, no public exposure.
  Tailnet membership is the first layer of auth; a per-player token the second.
- **Protocol:** Godot high-level multiplayer (ENet/UDP) with the server
  authoritative. Because world generation is deterministic from the seed,
  clients generate terrain locally and the server only sends **deltas**
  (block edits, entities, inventories). This keeps bandwidth tiny for a voxel
  world. godot_voxel's own (experimental) `VoxelTerrainMultiplayerSynchronizer`
  is the alternative if we use its terrain nodes; to be evaluated.
- **Server-side terrain without rendering** must be verified: godot_voxel
  nodes in a headless build (P2 follow-up).
- **Version gate:** the server advertises its protocol/content version; a
  mismatched client is refused with an "update available" message that
  triggers the update flow below.

## Auto-update flow
1. **CI builds** (GitHub Actions): Windows zip, Linux
   server Docker image. Artifacts are versioned `vMAJOR.MINOR.PATCH`.
2. **Manifest** on the Unraid box (small HTTP server container, same tailnet):
   `manifest.json` with latest version, per-platform download URL, size,
   SHA-256, minimum compatible protocol version.
3. **Windows client:** on launch, fetch manifest; if newer, download. Content
   updates ship as a `.pck` loaded via `ProjectSettings.load_resource_pack()`
   or swapped on disk; an engine-version bump downloads the full zip and a
   tiny helper script swaps the executable after exit.
4. ~~Android client~~ (dropped, ADR 0009). Android does not allow silent installs. The game
   downloads the APK and hands it to the system installer (one tap to
   confirm; "install unknown apps" granted once). Alternative with zero code:
   point [Obtainium](https://github.com/ImranR98/Obtainium) at the manifest
   or GitHub releases.
5. **Server:** Unraid pulls the new image (Watchtower-style auto-pull or a
   manual update button). Saves are versioned so a newer server can migrate
   an older world.

## What this means right now (Phases 1–3)
- Keep the simulation layer engine-agnostic and deterministic.
- Define a **network-shaped boundary** between simulation and presentation
  even in single-player (commands in, state deltas out).
- Chunk-based, versioned save format from the first prototype that saves.
- Prefer GDScript + native over C# unless P2 forces otherwise.
- (Android export smoke test dropped with ADR 0009.)
