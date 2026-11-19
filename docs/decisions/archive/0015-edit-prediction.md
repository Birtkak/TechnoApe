# 0015 – Edit prediction: show at once, put back what the server did not confirm

- **Status:** accepted
- **Date:** 2026-11-10

## Context
ADR 0008: a dig or a place shows in the frame of the click, and the server confirms or reverts it. A
round trip is at least one 50 ms tick, and longer over Tailscale. ADR 0014 gave every edit a `seq`, a shared
rule (`edit::changes`) and a `Done` sent after the tick's `Edits`.

## Options considered
- **Undo log per edit:** keep each cell's block from before the edit and restore it on refusal. This goes
  wrong when another player's edit lands on the cell in between, or when an accepted edit differs on the
  server (a player there that the client did not know of).
- **Server truth per covered cell:** keep the server's latest word on every cell a pending edit covers. A
  cell shows the newest pending edit's block. When its last pending edit is answered, refused or not, the
  cell takes the server's word. This is exact because the server's `Edits` for a tick arrive before its
  `Done`.

## Decision
The second option: `engine::edit::Predictor` with three calls.
- `edit` applies the shape at once and returns the `seq` to send.
- `server` takes a cell from `Edits`.
- `done` puts back every cell the server did not confirm and returns them for remeshing. A cell that already
  shows the server's block is left alone.

A shape that does not fit the world is sent but not predicted. A put-back cell with no server edit goes back
to "generated" (`World::put_edit(b, None)`), so a predicting client's world hashes the same as the server's.

The soak's bots predict like clients, with only their own box kept clear. The check every 100 ticks (each
bot's world equals the server's) therefore covers the predictor through thousands of refusals, overlaps and
cells other bots kept open. A predictor that never puts back fails it at tick 100.

## Consequences
- The `edit` benchmark run remeshes in the frame of each edit: 2670 -> 2480 fps, far inside ADR 0004.
- With many drags in flight, `done`'s check whether a newer edit covers a cell is a linear scan. It becomes
  a count per cell if that shows up in a profile.
- Movement prediction is unchanged (snap over 1 m, ADR 0007).
