# 0005 – Windows client, Linux server on the home server, automatic updates

- **Status:** accepted
- **Date:** 2026-11-09 (targets first set 2026-10-21, Android dropped 2026-10-22, carried over unchanged)

## Context
The developer plays on Windows and hosts co-op on the household Unraid server, reached from anywhere over
Tailscale. Players should always run the version the server runs without installing anything by hand.

## Decision
- **Targets:** Windows x86_64 client; Linux x86_64 dedicated server. No mobile or console targets.
- **Server:** the same Rust code built without a window, in a Docker container on the Unraid box, reached
  over the Tailscale network. Server-authoritative; the world is deterministic from its seed, so only changes
  cross the wire (ADR 0002).
- **Updates:** CI builds the client and the server image; the server publishes a version manifest; the client
  checks it at start and updates itself before joining.

## Consequences
- Singleplayer and multiplayer share one code path (a local server), so the dedicated server is a build
  flag, not a separate program.
- Deployment and the updater come after the first playable; nothing built before then may block them.
