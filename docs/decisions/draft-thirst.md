# NNNN – Thirst and drinking (needs step 1)

- **Status:** proposed
- **Date:** 2026-11-17

## Context
`docs/design/needs.md` step 1: a third need beside health and hunger, water that knows whether it is salt, and
the use button on water to drink. It changes the save (a field in every player's vitals) and the wire (thirst in
`ToClient::Vitals`, a new `ToServer::Drink`).

## Options considered
1. Thirst carried by the water itself, a salt bit per stored column on the wire and in the save – right for
   mixed water, but a wire and save change to the water for a case nothing can make yet (no pipes, no pot).
2. Salt told from generation (`World::salt`): a cell is salt when its column's generated water is the open
   sea's (`Ground::reach == SEA`) – no new state at all; the bake's rivers and lakes and water poured on land
   are fresh.

## Decision
Option 2. The numbers, needs.md's starting points:
- Thirst 0 to 20 in `Vitals`, drained per physics step the ape takes at hunger's rate (`vitals::rate`: a walk
  twice, a sprint six times): empty in 1,200 s standing (two thirds of a 30 min day), 600 s walking.
- Empty: a point of health every 10 s, to death ("<name> died of thirst"); no healing while it is empty. An ape
  that never drinks nor eats dies at 1,400 s, as needs.md's balance line says (a unit test).
- Drinking: the use (place) button held with no block in hand on the water cell before the aimed block,
  within 6 m (`items::BENCH_REACH`), alive, in survival: empty to full in 2 s. The sea gives nothing.
- Save version 17 keeps thirst; 16 and older load their players with thirst full (`Vitals16`).
- `/thirst [n]` sets it, as `/feed`.

## Consequences
- The soak's state hash changes: vitals are hashed (`Server::hash`), and the soak now drinks at random.
- Left open: low thirst's "slower digging" (needs.md's first bite) is not built; heat's "half again faster"
  waits for warmth (step 5); a mouthful takes no water from the world yet (the ledger is untouched); sea water
  led or poured inland reads fresh (a bit per column when water can be carried far or mixes); the HUD's third
  pane is plain structure for the look pass.
