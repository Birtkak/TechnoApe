# 0016 – Saves: only what differs from the seed, written whole, renamed into place

- **Status:** accepted
- **Date:** 2026-11-10

## Context
The roadmap's digging step ends with saves of only what changed. Terrain comes from the seed (ADR 0003),
so a save needs only the edits and the players. Principle 8: saving must not stall the tick or the frame.

## Options considered
- **Format:** postcard (already the wire and recorder format) in one file; a file per region; a database.
  One file per world is the smallest thing that works. At about 9 bytes per edited cell, a world with a
  million edits is around 9 MB. Regions come when a world outgrows that.
- **Players:** by connection id (new each session) or by name. By name, as `Hello` gives it; accounts come
  with public servers.

## Decision
`server::save::Save` holds:
- a version
- seed and planet sizes
- every registered block as (id, name)
- every edited cell, sorted
- every named player's position, look, mode and inventory, sorted by name

Loading checks the version and that every saved id still has its name in `data/blocks.toml`. New blocks
leave old saves readable; a renamed or removed block refuses them. A save that does not load stops the
start, so it is never overwritten.

How saving runs:
- The tick thread takes a snapshot every 30 s. A thread serialises it, writes `<file>.tmp` and renames it
  over the save, so a crash keeps the last good save.
- The singleplayer client saves for `run=play` to `saves/seed-<seed>-planet-<n>.sav` (`save=` names another
  file, `save=0` none) and saves again when it quits.
- The dedicated server saves to `saves/server.sav`. Ctrl-C loses up to 30 s for now; a shutdown command
  comes with server admin.

Players who leave stay in the server by name, and a `Hello` with that name brings them back with what they
held. A server that began from a save records the save after `Start`, so replays begin from it too.

## Consequences
- The soak ends with a save and a load that must give back the same world hash and the same save; it runs
  in the gate.
- The snapshot copies every edit on the tick thread. At a million edits that is tens of milliseconds every
  30 s: fine for the first playable, and the place to look if long-lived servers stutter.
- Benchmark runs keep no saves.
