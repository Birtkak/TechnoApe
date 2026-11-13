# 0093 – Mining (a): rock digs into rubble, small stacks

- **Status:** accepted (slice (a) of `docs/design/mining.md`, ADR 0087)
- **Date:** 2026-11-13

## Context
ADR 0087: broken rock is loose rubble that blocks the way (ADR 0079's falling kind), not a pile to walk over,
and rock, rubble and ore stack to 16. Before this a survival dig of rock gave nothing (ADR 0034) and a dug
loose block fell as a pile (ADR 0074).

## Options considered
- **What a rubble block is in data:** a written `<rock> rubble` row per rock (26 rows of looks to keep in
  step with their rocks), a state bit on the block id (every reader of an id would have to know it; the
  shader reads 8 bits), or **one field on the rock's row** naming the twin's id, the twin made from the row.
- **Where dug rubble goes:** a pile at the cell (ADR 0074's rule for loose blocks), or straight into the
  slots.
- **The ore ledger (ADR 0063):** count ore in rubble as the body's until it is carried off (a second account
  that follows rubble as it falls and is picked up), or count ore in place only.

## Decision
- **`rubble = <id>` on a block row** (`data/blocks.toml`): rock (stone, the 13 rocks of `rocks.toml`, Luna's
  four) and hard ore (malachite, chalcopyrite, cassiterite, hematite, coal, galena, gold quartz,
  pitchblende). The registry makes `<name> rubble` with that id: the rock's colours and sound in pebbles,
  solid, `loose`, `falls`, 0.5 s by hand (`RUBBLE_S`), and `rubble_of` naming its rock. Ids 160..199 are kept
  for them. Soil, sand, gravel, clay, wood, leaves, ice, made blocks and loose ore (the gravels, bog iron,
  bauxite, gossan) name none: they are taken as themselves, as before.
- **A survival dig of rock or ore leaves its rubble in the cell** (`edit::dug`, one rule for the server and
  the client's and bots' prediction). Nothing goes to the slots and no pile falls. The rubble falls and slides
  as ADR 0079's sand does, and buries as it does.
- **Digging rubble puts it into the slots;** with no room for all of a dig's rubble the dig is refused and the
  rubble stays a block (nothing lost, no pile; the lead's check). The item is the
  rubble (placed, it falls again: a spoil heap). Creative digs anything away, as before.
- **Stacks:** `stack` on a block row (1..64, 64 by default); rock, rubble, ore and gossan 16.
- **The ledger counts ore in place.** A dig of an ore block takes it from its body (as ADR 0063 always did:
  the cell no longer holds the ore block); rubble is mined ore, wherever it lies or falls. The ledger stays
  exact (`ore_recount` from the edits equals the tally) because rubble is just another block that is not the
  ore. What rubble of ore is carried off and smelted is the items' ledger's.

## Evidence
- Tests seen red then green: `blocks::rock_and_ore_dig_into_rubble_and_stack_by_16` (every rock and hard ore
  has a falling, loose twin that looks and sounds like it; soil and the rest none; a taken id or a loose rock
  refused), the server's `a_survival_dig_leaves_rock_as_rubble_and_digging_that_takes_it` (granite leaves
  granite rubble, told to the digger, falls into the open cell under it, digs in under a second into the
  slots, no pile, the items' ledger even; a full pack's dig refused and the rubble left; creative unchanged), and the older dig tests (stone by hand now
  leaves rubble).
- **Spike 2, a mine full of rubble** (`fall::rubble_mine_cost`, play build): 5,766 granite rubble blocks
  resting on a 31 x 31 floor cost **9 ns a tick** all told (nothing awake: resting rubble is ordinary edits).
  A 9 x 9 hole dug under them brought down 25,473 moves in 627 ticks: **306 ns a visit, 0.66 ms in the worst
  tick**, at most **2 chunks** changed a tick. On a client each changed chunk is one mesh job on a worker
  (45 us to 1 ms, `docs/benchmarks.md`): at most some 40 jobs a second during a drop, the frame thread only
  swaps them. Resting rubble costs a frame what any block does (it is in the chunk's mesh). CPU: the server's
  tick carries the drop in under 1.5 % of its 50 ms; a quiet mine costs nothing.
- SOAK

## Review (a fresh reviewer read the diff)
- *A survival dig of rock sounded like a place and showed no break* (rock to its rubble is solid to solid):
  `edit::breaks` counts a change into `was`'s own rubble as a dig, for the effect and the sound; test
  `fx::rock_dug_into_its_rubble_plays_a_break`, seen red.
- *The soak's heap bot predicted rubble in its creative hole* (it read its mode before the server took its
  `Creative(true)`): it predicts creative there. A client's prediction can lag a `/playmode` by one message the
  same way; `Predictor::done` puts the server's block back.
- *`fxedit` with `falls=1` played its effect for air where the world shows rubble:* it reads the world's block.
- *Answered, not changed:* a hammer dig in survival over rubble beside rock (rubble to air, the rock beside it
  to the same rubble, in one batch) passes the client's test for a fall, so those two cells play no effect or
  sound; the mesh is right. shortcut: pair moves by the server's own mark when anyone notices.

## Calls for the developer to confirm (tuning, by play)
- Which blocks make rubble: every rock and hard ore; loose ores stay themselves.
- Rubble's look: its rock's colours in pebbles (a draft for the textures session); 0.5 s by hand.
- Stacks of 16 for rock, rubble, ore and gossan.
- Dug rubble goes straight into the slots (no pile); a full pack cannot dig rubble at all.
- Stone and ore placed back come only from creative or `/give` until tools take whole rock.

## Consequences
- A mine fills with rubble that must be cleared; the pack fills after 16 of a kind a slot.
- Cracks and the drop along them (slice (b), ADR 0094) turn more cells to rubble at once through the same
  `dug` and fall.
- Smelting will take rubble of ore as its input.
