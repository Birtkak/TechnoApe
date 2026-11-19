# 0034 – Survival dig time, as in Minecraft

- **Status:** accepted
- **Date:** 2026-11-11

## Context
Blocks broke at one click in every mode. The effects system (ADR 0031) had nothing to show while a block
was being dug, and survival had no cost for digging. The developer's call (2026-11-11, effects.md): "dig time
in survival should be the same like minecraft". That means a held dig that takes time by the block's hardness
and the tool, cracks that spread while it is dug, and creative that stays instant. Three details were asked
and answered:
- **Stone by hand:** slow, and it drops nothing (as Minecraft without a pickaxe).
- **The hammer's shapes:** a shape takes its slowest cell's time, not the sum of its cells.
- **Looking away:** a half-dug block keeps its progress for a few seconds (5 s), then starts over.

The rules the multiplayer design holds: the server is the authority (a client must not dig faster by
lying), replay is deterministic from the ticks, and other players see the cracks.

## Options considered
1. **Client times it, server trusts the Edit.** Small, but anyone could dig stone at a click.
2. **The server times it from the client's word that it holds (chosen).** The client sends
   `ToServer::Dig(Option<Shape>)` when what it holds changes. The server counts the seconds held on its own
   tick clock and refuses a survival dig `Edit` that comes early, with a 0.15 s allowance for the network.
   It tells the others `ToClient::Digging { id, at: (shape, share done, share a second) }`, and they run the
   cracks on from that without a message a frame.
3. **The server drives the dig from held-input packets every tick.** This is exact, but it adds traffic every
   tick and it does not suit the edit path, which already predicts and is answered by `Done`.

## Decision
Option 2.
- **Data.** `data/blocks.toml` gives each solid block a `hardness`: the seconds to dig it by hand, checked to be
  in 0..60. It also gives a `loose` flag: bare hands take the block into the inventory. The values follow
  Minecraft's by-hand times:

  | block | hardness (s) | loose |
  |---|---|---|
  | grass | 0.9 | yes |
  | dirt, regolith | 0.75 | yes |
  | lamps | 0.45 | yes |
  | stone | 7.5 | no |
  | copper | 15 | no |

  A shape's time (`edit::dig_time`) is the largest hardness of its solid cells.
- **One counter for both sides.** `edit::Hold` counts the hold on the server and on the client in the same
  way, each on its own clock: the shape held now, when the hold began, and the seconds it had before. A shape
  left keeps its seconds for `DIG_KEEP_S` = 5 s (at most 8 shapes are kept), and coming back resumes them.
- **Client.** In survival, left click held gathers time on the aimed shape. A dig goes once the shape has its
  time, not when the button is let go. Held on, the dig goes on to the next block under the aim. Every 0.25 s
  held there is a hit: a few `chip` particles off the nearest face and the block's dig sound, softer and
  higher. The cracks are a new fx look (`crack`, 3), drawn each frame over the real cell's faces: up to 64
  cells, reserved past the pool so they never grow the buffer. They are texel-aligned rays in the block's own
  texels, darkened, and grow in ten stages. They show for the dig held now, for the shapes kept, and for other
  players' digs.
- **Server.** A survival air `Edit` is refused while `held + 0.15 s + 5 % of dig_time < dig_time`, so the
  client's prediction rolls back. The share is there because the server's tick clock drops a late tick's time
  and the client's wall clock does not; over 15 s of copper that could reach 0.2 s. A hold counts only on a
  shape in reach with something solid in it, so a player cannot gather time on a far block while walking to it,
  or on an empty cell before a block lands there. A player who leaves ends its dig for the others. Creative is
  unchanged.
- **Shortcut.** There are no tools yet, so "by hand" is the only speed, and stone and copper drop nothing.
  When tools come (the roadmap's mining tools), the tool divides the time and decides the drop.

## Consequences
- Survival digging costs time, and stone needs a tool once tools exist. Bots and the soak hold half their
  digs for their time; a fifth of those let go early, which tests the refusal.
- Players are told of every change in what someone digs. That is one small message per change, not per tick.
- The crack look adds a loop of at most seven rays per covered fragment, only on cells being dug, and only
  during a dig (a few cells, a few thousand pixels). The CPU cost is a few pushes a frame.
- Cracks go through the effects (`Fx`), so a run with effects off (benchmarks) shows no dig's progress; play
  always has them on.
- Trusted for now, not by this ADR: `ToServer::Creative` is taken from any client, so a cheater can skip the
  dig time. It is closed when accounts and permissions come.
- To revisit: the tool and drop rules, Minecraft's slower digging in water or in the air, and whether
  `DIG_KEEP_S` feels right in play.

| Review | Found |
|---|---|
| fresh reviewer | No outright breakage. Fixed: the server clock losing late ticks' time refused long digs (now a 5 % share); a player who left kept its cracks and hits sounding for 60 s at the others; looking away did not pause the dig; time gathered on far or empty shapes; kept shapes lost their order; two comments sat over the wrong functions. Noted: unchecked `Creative`, no cracks with effects off. |
