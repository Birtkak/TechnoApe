# draft – What death costs (needs step 3)

- **Status:** proposed
- **Date:** 2026-11-17

## Context
`docs/design/needs.md` "What death costs" (decided in ADR 0047) and step 3 of its build order: piles, the death
waypoint, the sleeping place. Piles exist (ADR 0074): a respawn already laid what a dead ape carried at its feet.
Still missing were the death waypoint, the sleeping place and a test that nothing is lost or made wherever the
ape dies.

## Options considered
1. Marks kept by the client, saved with its waypoint file. That is wrong for a server-authoritative world: the pile
   is the server's, and only the server knows when it empties.
2. The server keeps two marks per player (`save::Marks`: the death pile's cell, the sleeping place) and sends them
   in a new `ToClient::Marks`. The client draws them beside its own waypoints.

## Decision
Option 2, as needs.md says:
- **The pile.** On `Respawn`, everything the ape carried is laid where its body lies (`Server::lay`, unchanged,
  ADR 0074). Out of a block it was buried in, it goes upwards. In water it falls to the bed. In the air it falls
  at most `FALL` (64) layers, so a death high in the air or in space leaves its pile floating. The items' ledger
  holds: nothing is lost or made. Anyone may take from it.
- **The death mark.** It is the list of cells the death's things went into, lowest first (`lay` returns them; a
  death with many stacks fills a column of piles). A cell leaves the list once its pile is gone, and the mark goes
  when the list is empty. Because a cell leaves at once, a pile laid there later by anyone does not bring the mark
  back. When a pile falls or is lifted (`resettle`, after a dig under it or a block placed in it), its cells in
  every player's list, online or absent, move with it. Only the last death that laid anything is marked. The client
  is sent the lowest cell. On the HUD it is a "Death" waypoint in its own colour (bone white, which no player
  waypoint uses) with a plus icon, seen from any body, since the pile may lie on Luna.
- **The sleeping place.** A bed of leaves beside a fire, where the ape fits. The two cells over the bed hold no
  block (so no torch on top), and the eye of an ape standing on it is out of the water. The use button (right click)
  with an empty hand claims it, last of the button's uses: a window first, then a pick-up (ADR 0144), then a drink
  (ADR 0145), then the bed (`play::claims`). The server claims it only within reach and in sight: nothing solid on
  the straight line from the eye to where the ape would stand on it, so not through a wall into another's room.
  - Why a line and not the server's aim: the server holds the look a tick behind, from the client's inputs, as
    ADR 0144 found for pick-ups. The line needs no look and gives the same answer on a replay.
  - The server answers "you will wake here", or "no sleeping place: ..." for leaves that do not qualify. The HUD
    shows a green "Home" waypoint.
  - A respawn wakes the ape with its feet on the bed (`vitals::wake_at`) while the bed still qualifies. A block
    placed on it, water over it, or the fire gone sends the ape to Terra's spawn, and Home is forgotten. Waking on
    Luna works if the bed is there. Two players may share one bed.
- **Creative loses nothing.** It cannot die: `/hurt` is refused and breath is reset in creative, so it never
  leaves a pile. This is tested.
- **What else the ape held:** a workbench's grid goes into the ape's pack at death (`uncraft`) and so into the pile.
  A crucible's slots are the crucible's, so death only closes its window. There is no item on a cursor on the
  server (a move is one message from slot to slot). An ape that leaves while dead keeps its things in the save, with
  its name, and lays them when it comes back and respawns. Tests cover the grid and the leave with the ledger exact.
- **Save version 18** keeps both marks per player. 17 and older load with neither (they wake at the spawn). A
  test covers 17, and the 16 path too; 15 to 4 go through the same mappings their tests already cover.

## Calls to confirm (the design leaves them open)
- **What counts as a bed of leaves beside a fire:** any `* leaves` block with a torch in one of the 26 cells
  around it and room to stand on it. A torch is the only fire until the cooking fire (needs step 4), which joins
  `vitals::sleeping_place` when it comes. Only the bed's own cube face is looked at.
- **How it is claimed:** the use button with an empty hand on the bed, no sleep and no night skipped (needs.md: a
  shared world has one clock).
- **When the pile is laid:** at the respawn, as ADR 0074 already did, not at the moment of death. The body lies
  where it fell in between.
- **The death mark lasts** until the pile is empty and is not named for the death's cause.

## Consequences
- **Old clients cannot join a new server**, nor new clients an old one: `ToClient::Marks` is a new message
  (appended last, tag 21, pinned by a test), and the protocol has no version. The friends' client needs the
  deploy.
- **Recordings made before this change do not replay** if they hold a `Loaded` save of version 17 inline
  (`Ev::Loaded`): this build reads a 17 from disk but replays decode the current layout. Recordings that start from
  a fresh world replay; the marks are not in the state hash.
- Saves of 17 load, every player without marks.
- The soak: bots that die respawn as before, and their piles lie where they died. Piles are bounded by the items'
  ledger: a death lays only what was carried and makes nothing, which the soak's ledger check asserts every
  check. No pile is ever removed by time (items.md: nothing despawns).
- No effect or sound plays at a death or its pile yet: a `shortcut:` where deaths are announced names the
  journal events ("died", "pile laid") that should raise them.
- A pile left floating after its 64-layer fall is simply a pile in the air. It does not block building (a pile is
  never solid; a block placed in its cell lifts it). Reaching it to pick it up takes flight or a pillar built up
  to it.
- Left open: rooms and habitats as sleeping places (needs.md: later), the globe map's view of the game marks, and
  a waypoint shared in chat.
