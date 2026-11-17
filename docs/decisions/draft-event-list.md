# Draft – The event list

- **Status:** proposed (the journal's step 1, `docs/design/journal.md`)
- **Date:** 2026-11-17

## Context
The journal learns from what happens in the world. Its design asks for "events out of the tick": small records
that the systems already deciding things push, in a fixed order, so a replay makes the same ones. The effects
and the sounds should listen to the same list. Until now each client worked out what had happened from the
state it was sent:
- a dig or place from the cells of `Edits`;
- a loose block's fall from two edits in a row that looked like a move;
- a worn-out tool from a slot that emptied in `Inventory`.

Step 1 adds the list and moves the effects and sounds onto it. It has no new behaviour: the views and the soak's
state hash say so. The list's shape on the wire is hard to undo.

## Options considered
1. **A typed enum, one message a tick, the events a client perceives** (*chosen*). `engine::events::Event`
   has a kind per thing that happened, each with its own fields. `ToClient::Events(Vec<Event>)` is appended
   at the end of `ToClient`, and each tick's events go before that tick's `Edits`.
   - For: exact, cheap (postcard: 2 to 15 bytes an event), checked by the compiler. A kind is added at the end
     without touching the others.
   - Against: a field once shipped never changes, and a new field is a new kind.
2. **A generic record** (`kind: u16, at, by, numbers: Vec<f32>`).
   - For: new kinds without a wire change.
   - Against: every reader decodes by convention; a wrong index is a silent bug, not a compile error.
3. **Events inside `Edits`** (a cause per cell).
   - For: one message.
   - Against: most events are no edit (a take, a craft, a death, a fire), and `Edits` is also the joining
     client's state, where a cause means nothing.
4. **Keep inferring on the client and add the list only for the journal.**
   - For: no client change.
   - Against: two sources of what happened, which can disagree (the fall heuristic already mistook moves across a
     face's edge for a dig and a place).

## Decision
Option 1.

**The kinds** (the wire's order, pinned in `events::tests`):

| # | Kind | Fields |
|---|---|---|
| 0 | `Edit` | `at, was, now, by: Option<player>` (None: the world, a torch with nothing to hang on or put out by water) |
| 1 | `Fall` | `from, to, block` |
| 2 | `Worn` | `item, by` |
| 3 | `Taken` | `at, row, by` (a pick-up or a plant cut) |
| 4 | `Craft` | `recipe, n, by` |
| 5 | `Fire` | `at, on` |
| 6 | `Died` | `by` |

**Where they are raised.** The server's systems push the events into one list on the server, in the tick's
order, and the list is sent and cleared once a tick.

| Kind | Raised by |
|---|---|
| `Edit` | `edit` (every changed cell, crack drops and rubble included), `/place`, `/structure`, the unheld torch, the torch water put out |
| `Fall` | the tick: one per move that `Falls::step` made; a move across a face's edge is two `Edit`s, as the client drew it before |
| `Worn` | `edit` and `prospect` |
| `Taken` | `take` |
| `Craft` | `craft`, when something was made |
| `Fire` | `furnaces`, beside `Fires` |
| `Died` | where a death is already announced (`live`'s lines in the tick and `rock_hits`), by the player whose cause it is; that code's logic is unchanged |

**Who is sent what.** A client gets the events within `PERCEIVE` (80 m, the animals' `SEE`) of it, and every one
of its own. An event that happened to a player (`Worn`, `Craft`, `Died`) counts as where that player is.

**The client** keeps what the tick's events said of cells (`fx::Pending`).
- Its `Edits` are applied through `fx::server_cell`: a cell an `Edit` names plays its break or place and is heard
  (`server_edit` as before, the same three sounds per message and 32 effects per frame).
- The cell a `Fall` came to glides in from the one it left, with its grains and the slump's hiss.
- Every other cell changes silently. The pair heuristic is gone.
- `Worn` for this player plays the tool's snap and chips.
- The client's own dig and place are still played at the click, and its server edits show nothing twice
  (`Predictor`).

**Replay and hash.** The events are a pure result of the tick, and the recording logs only the inputs, so a
replay makes the same events. Neither the recording nor the state hash holds them. The 1 minute soak's hash is
the same as main's.

## Consequences
- **Old clients.** postcard knows no variant 21, so an older client's link ends at its first `Events` message
  (that is the first edit near it). Clients from before this change cannot play on a server with it. The
  friends' zip updates itself (ADR 0130).
- **What plays differently, all of it edge cases.**
  - A joining client's batch of past edits plays no breaks; it did play up to 32 a frame before, at random
    places.
  - Edits farther than 80 m play nothing; no sound carries that far, and a break's specks are below a pixel there.
  - A tool worn while an inventory move was pending now snaps; the old guess skipped it.
- **What did not move, and why.**
  - The client's own predicted effects (the dig's chips, the place's grow, the knap, steps, landing) play at the
    input, before the server answers.
  - The prospecting knock answers the striker only (`Prospect`).
  - An animal's voice is part of the herd state it is sent (`Animals`, its call count).
  - A furnace's working state lasts and a joiner needs it (`Fires` stays; `Fire` is beside it for the journal).
  - Hurt plays from this player's own `Vitals`; health is moved in `vitals.rs`, which step 2's `need:` triggers
    will read.
  - Thunder is drawn from the weather cells every client has.
  - UI sounds are the client's own.
- Step 2 reads the same list on the server for `look:` and `have:`; `see:` will use `PERCEIVE` and the facing.
- **Cost:** the sim tick before and after, in the commit.
