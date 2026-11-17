# 0142 – Tin and bronze: a furnace's second input

- **Status:** accepted
- **Date:** 2026-11-17

## Context
The Steam Age's next milestone after copper (docs/design/progression.md, "Tin and bronze"): tin is found a walk away
and melted with copper in the crucible into bronze. The crucible of ADR 0141 had one input slot, so it could not
alloy. Bronze also brings five more casts, and the cast row of the window must not grow to ten choices.

## Options considered
1. **A second input slot on every furnace (chosen).** Slot 4, appended after in, fuel, mould and out, so the first
   four keep their numbers on the wire (`Holder::Furnace(b)` slots), in recordings and in bot scripts.
2. **Alloy recipes as a grid-like pattern of several input slots.** More general, but nothing needs more than two
   inputs yet.
3. **Casts by shape: a new `shape` field on `[[cast]]`, the choice still a row name (chosen).** `Choose` and
   `Furnace::choice` are unchanged: they name a cast row, and any row of a shape stands for the shape. An old
   `Choose { cast: "cast pick head" }` (copper's row) keeps meaning "the pick head", now for whatever metal is in.
   Choosing by a shape name instead would have changed what the wire and saves carry.

## Decision
- **Slots:** `furnace::IN2 = 4`, `SLOTS = 5`. Either input slot takes any item; the window shows the second beside the
  first (the mould moved under the out slot). Shift-click from the pack: to the first In that holds the item or
  is empty, else the second.
- **`with`:** a `[[melt]]` may carry `with = [item, n]`: both stacks are needed, one in each input slot, either
  order; it takes exactly both amounts.
- **The rule that keeps what runs unambiguous:** a work with `with` runs only with both input slots full; a work of one
  input (a melt or a cast) runs only while the other input slot is empty. At load: no two melts of one input or of
  one pair of inputs (either order), `with` only on a melt and not its own first input, a cast names a `shape`
  and a melt none, no two casts of one shape and input.
- **Casts:** the window offers `Recipes::shapes(top)`, the first row of each shape; the metal in the input slot picks
  the row of that shape that runs.
- **Save version 16:** a furnace has five slots. A 15 is read as a 16 is and its furnaces get an empty fifth slot.
  The state hash of a world without furnaces is unchanged (the furnaces are hashed only when there are some).

### Tunable numbers (first guesses)

| Number | Value | Where |
|---|---|---|
| Tin melts | 4 tin pebbles, 2 tin gravel, 1 cassiterite rubble to 1 tin ingot; 232 C, 4 s | `data/recipes.toml` `[[melt]]` |
| Bronze | 3 copper ingots with 1 tin ingot to 4 bronze ingots; 1085 C, 12 s | `[[melt]] with` |
| Bronze casts | copper's ingot counts, 950 C, 5 s, clay mould | `[[cast]]` |
| Bronze tools | speed 5.0, grade 2, 400 uses, prospect 32 m | `data/items.toml` `[[material]] bronze` |

## Consequences
- **Wire:** `ToClient::Furnace` carries five slots. `ToServer::Choose` is unchanged, so old recordings still play.
- **Shortcuts:** bronze's items are drawn with `look = "copper"` until their models are painted. The tin ingot is
  drawn as cassiterite. The bronze hammer, like copper's, has no use yet.
- **Tests:**
  - Engine `furnace.rs`: `bronze_needs_both_inputs_either_way_round_and_takes_exactly_three_and_one`,
    `one_input_of_an_alloy_alone_does_nothing_and_one_input_melts_never_share`,
    `tin_melts_from_pebbles_gravel_and_rubble_on_a_stick_fire`, and
    `a_bronze_cast_is_chosen_by_its_shape_and_the_metal_picks_the_row`.
  - Engine `recipes.rs`: the bad-row cases for `with` and `shape`, and the bronze tools in the head-above-stick test.
  - Server: `a_crucible_alloys_bronze_from_both_inputs_and_casts_it_by_shape`,
    `a_15s_furnaces_load_with_their_second_input_empty`, and the five-slot `a_crucible_dug_full_drops_all_it_held`.
  - Client: `the_viewer_shows_both_of_an_alloys_inputs`, and the window's layout test with five slots.
  - Bot: `a_survival_bot_alloys_bronze_through_the_second_input`.
