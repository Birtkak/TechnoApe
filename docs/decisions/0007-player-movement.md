# 0007 – Player movement: shared fixed-step physics, server-authoritative

- **Status:** accepted
- **Date:** 2026-11-09

## Context
The first playable needs walking on a round planet: gravity toward the body's centre, walking and jumping in
the local frame, collision with 0.5 m blocks, across cube-face seams. The server owns the world (ADR 0006),
so movement must run there too, yet a player must feel their own input at once at any frame rate.

## Options considered
1. Client moves freely and sends its position – instant, but the server trusts the client; cheating and
   desync are built in.
2. Server-only movement, client shows what arrives – correct, but every key press waits a round trip.
3. One movement function in `engine`, run by the server per tick and by the client per frame for its own
   player (prediction), the server's position winning on disagreement – both feel and authority.

## Decision
Option 3, in `crates/engine/src/phys.rs`:
- **Radial gravity:** up is away from the nearest body's centre; G = 9.81 m/s². No frame per face: the look
  direction is kept in the tangent plane and carried along, so seams need no special case.
- **Snappy walking:** the horizontal velocity is the wished one (4.3 m/s), on the ground and in the air; a
  jump of 4.8 m/s (about 1.2 m). Inertia and air control are a feel call for later (ADR 0008).
- **Collision:** a 0.6 x 1.8 m box (eye at 1.6 m) moved axis by axis in substeps of at most 0.25 m, each
  move bisected to the first contact. The box is sampled every 0.5 m in height and on a 3x3 grid across, so
  no block fits between samples. One-block step-up while on the ground.
- **Fixed step:** DT = 1/60 s on both sides; each side carries the leftover time, so the same inputs give the
  same path at any frame or tick rate (deterministic, as `fundamentals.md` asks).
- **Wire:** the client sends `Input` (move, jump, look); the server steps it every tick until a `Move`
  (fly/teleport, creative, taken as sent). The client snaps to the server's position when they disagree by
  more than a metre (`snaps`, `drift_max_m` in the run's JSON); bots walk through the same path.

## Consequences
- Bots, the client and the server share one movement code path; tests drive it without a window.
- The snap is a shortcut: with latency it will rubber-band. Replaying unacknowledged inputs on correction
  (input sequence numbers) is the upgrade, due with the dedicated server over Tailscale.
- Point sampling assumes blocks of at least 0.5 m; fine cells (12.5 cm) will need a box-vs-grid sweep.
- `Move` is unchecked; a survival mode must reject or limit it.
