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
- **Collision in grid space** (revised 2026-11-10, below): the box is 0.6 x 0.6 x 1.8 m in nominal blocks
  (1.2 x 1.2 x 3.6 cells; eye at 1.6 m), aligned with the columns and layers of the face under the feet, and
  tested exactly against every cell it overlaps; cells past a face edge are found through space, which also
  holds across a cube corner. Moved axis by axis in substeps of at most 0.25 m, each move bisected to the first
  contact. Step-up climbs exactly one layer, whatever its thickness.
- **Fixed step:** DT = 1/60 s on both sides; each side carries the leftover time, so the same inputs give the
  same path at any frame or tick rate (deterministic, as `fundamentals.md` asks).
- **Wire:** the client sends `Input` (move, jump, look); the server steps it every tick until a `Move`
  (fly/teleport, creative, taken as sent). The client snaps to the server's position when they disagree by
  more than a metre (`snaps`, `drift_max_m` in the run's JSON); bots walk through the same path.

## Consequences
- Bots, the client and the server share one movement code path; tests drive it without a window.
- The snap is a shortcut: with latency it will rubber-band. Replaying unacknowledged inputs on correction
  (input sequence numbers) is the upgrade, due with the dedicated server over Tailscale.
- The player's size is in cells, so its size in metres follows the grid: about 6% smaller 64 m down on the
  1 km planet, and up to the cube-sphere's cell variation near face corners. In exchange a two-cell-high,
  one-cell-wide passage always fits, everywhere on every body.
- Fine cells (12.5 cm) fit the same test on a finer grid.

## Revision 2026-11-10: from sampled points to grid space
The first version tested 45 points of a box oriented by the look direction. The soak (ADR 0011) found three
ways in within seconds: turning swung a corner into a wall; `up` tilts the box a little after every move on a
round body, which pushed it into a wall it touched; and at seams and corners a block corner fits between
sample points, so no margin made the test sound. In grid space the answer depends on the position alone, so a
position the physics accepted stays free, and 50 bots for 10 simulated minutes on three seeds stay out of
every block. The same soak found that a block could be placed inside a player; the server now refuses a solid
block in any cell a player's box overlaps.
- `Move` is unchecked; a survival mode must reject or limit it.
