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
- **The death mark.** It is the cell the pile settled in, and it is kept while a pile lies in that cell or in the
  `DEATH_COLUMN` cells above it (every slot of an ape's worth of piles, plus one). It goes once the pile is
  emptied. Only the last death is marked. On the HUD it is a "Death" waypoint in its own colour (bone white, which
  no player waypoint uses) with a plus icon, seen from any body, since the pile may lie on Luna.
- **The sleeping place.** The use button (right click) with an empty hand on a bed of leaves claims it. The server
  answers "you will wake here", and the HUD shows a green "Home" waypoint. A respawn wakes the ape standing on
  the bed while the bed is still a sleeping place. Once it is not, the ape wakes at Terra's spawn and Home is
  forgotten. Waking on Luna works if the bed is there.
- **Creative loses nothing.** It cannot die: `/hurt` is refused and breath is reset in creative, so it never
  leaves a pile. This is tested.
- **Save version 18** keeps both marks per player. 17 and older load with neither (they wake at the spawn). A
  test covers 17, and the 16 path too; 15 to 4 go through the same mappings their tests already cover.

## Calls to confirm (the design leaves them open)
- **What counts as a bed of leaves beside a fire:** any `* leaves` block with a torch in one of the 26 cells
  around it. A torch is the only fire until the cooking fire (needs step 4), which joins
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
- `shortcut:` a pile that falls when dug under takes the mark away with it (the mark looks only at its own
  column). No effect or sound plays at a death's pile yet: that waits for the journal's event list.
- Left open: rooms and habitats as sleeping places (needs.md: later), the globe map's view of the game marks, and
  a waypoint shared in chat.
