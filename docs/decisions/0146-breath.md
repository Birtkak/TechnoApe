# draft – Breath (needs step 2)

- **Status:** accepted
- **Date:** 2026-11-17

## Context
`docs/design/needs.md` step 2: the ape cannot breathe with its head under water, above where the air ends, or
on a body without air. ADR 0079 already holds the breath of an ape buried to the eyes (10 s, then a point of
health a second, a reset the moment the head is free). The brief: one "cannot breathe" condition with three
causes, not a second mechanism; a bar shown only while the breath is short; the wire and the save as small as
they can be.

## Options considered
1. A second counter for water and space beside the buried one – two rules that could disagree, and two
   bars.
2. One `vitals::Breath` (0 to 20, a fraction kept), fed one `Option<Choke>` a step: `Buried`, `Water` or
   `Space`, each with its own length and death message – the buried rule becomes one of three causes.

## Decision
Option 2. The numbers:
- The server judges the eye (`pos + up * EYE`) each physics step, in this order: a solid block there is
  `Buried`, else water over it (`World::water_surface`) is `Water`, else more than `AIR_TOP_M` over the
  nearest body that has air (`Frames::over_air`) is `Space`.
- `AIR_TOP_M` = 1,500 m: needs.md's number, the height where the client's sky already thins the air to
  nothing (`frame.rs` now reads the same constant). Luna has no air, so on and near it the eye is always over
  the air: a survival ape on Luna runs out of air, as needs.md wants until suits exist.
- Lengths: buried 10 s (ADR 0079 unchanged), water and space 30 s (needs.md). Then a point of health a second
  (`CHOKE`), to death: "<name> was buried", "<name> drowned", "<name> ran out of air".
- Air brings it back from none to full in 2 s (needs.md), and the second's count starts again. This replaces
  ADR 0079's instant reset: an ape that bobs up for one breath gets a tenth of it back a tenth of a second.
- Creative and a respawn reset it to full.
- **Not saved:** the breath lives in the server's `Client`, as the buried count did; a load starts it anew
  full. No save version change.
- The wire: `ToClient::Vitals` gains `breath: u8` (its whole points, rounded up), sent when it changes.
- The HUD: a pane over health's, on the left, only while the breath is under 20; plain structure for the
  look pass. F3's Player section shows it; `run=play`'s vitals JSON and bots' `report` carry it as a fourth
  number.
- The soak's range check covers the breath; a bot that walks into the sea and drowns respawns as before (the
  soak never asserts its bots are alive).

## Consequences
- **Old clients cannot join a new server**, nor new clients an old one: `ToClient::Vitals` gained a field and
  the protocol has no version. The friends' client must be updated with the deploy.
- The breath is not in the state hash (the buried count was not either). A recording made before this change
  in which a survival player stayed under water now drowns on replay and differs from there; new recordings
  replay as before.
- A player who drowns or runs out of air loses health as the buried one always did: death's cost (step 3) is
  not built.
- Open taste calls: buried at 10 s against 30 s under water (kept from ADR 0079: sand presses, water is held);
  the 2 s recovery for the buried case; the bar's look and sign.
- Left open: bad air in a room (needs.md, with habitats), thin air on mountains (needs.md: not modelled),
  suits.
