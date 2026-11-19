# 0141 – The crucible: the first furnace

- **Status:** proposed
- **Date:** 2026-11-16

## Context
A survival player could gather copper ore and craft at a workbench (ADR 0140), but could not get from ore to a copper
tool without `/give`. The way in is to gather ore, melt it to ingots, cast the ingots into heads, and haft a head on a stick.

These were the developer's calls:
- The crucible is one placed block with a plain furnace window: in, fuel, out, and its heat.
- It reaches 1100 C, and a furnace only works the recipes its heat reaches.
- A furnace's top heat and its recipes are data. Nothing is built for the bloomery or the blast furnace yet.
- Moulds last forever. Wood is enough fuel for copper. Bronze, not brass.

## Options considered
1. **A furnace's slots in the client's inventory, like the crafting grid.** Simple, but the work would stop when the
   window closes, and two players could not share one furnace. Refused.
2. **The slots in the world, keyed by the block's cell (chosen).** The server's map `furnaces` sits beside the piles.
   It ticks while there is fire or heat above the air's, and it is dropped when the furnace is idle and empty. It is
   reached through the existing item moves as a new `Holder::Furnace(Block)`, so it gets the same whole-or-nothing
   moves, reach checks and ledger as every other holder.
3. **Heat as a curve (Newton cooling).** Nicer to watch, but harder to read off and to test. A straight line toward
   a target was chosen: the target is the burning fuel's own heat capped at the furnace's top, and the line falls to
   the air's heat without a fire.

## Decision
- **The block.** `crucible` (`data/blocks.toml`, id 219) has `window = "crucible"`, is loose, and is drawn by the
  furnace model.
  - It is made at a workbench from eight clay round an empty middle.
  - Right click opens it. Anyone within `BENCH_REACH` (6 m) may use it, and walking out closes it.
- **Its four slots: in, fuel, mould and out.**
  - They are server-authoritative, saved with the world (save version 15; a v14 save loads with no furnaces) and
    counted by the items ledger.
  - Digging the crucible lays everything it held as a pile at its cell.
  - The fuel slot takes only fuel and the mould slot only moulds. The out slot only empties.
- **Fire.**
  - A fuel is lit only when the fire is out and there is work. A lit fuel burns its full seconds.
  - Heat moves at `rise` C/s toward min(the fuel's heat, top), and falls at `fall` C/s to the air's heat.
  - A work's seconds count only while the heat is at or above the work's heat.
- **Rows in `data/recipes.toml`.**
  - `[[melt]]`: an input to an output.
  - `[[cast]]`: an input and a mould (kept) to an output; it needs a cast chosen in the window.
  - `[[fuel]]` and `[[furnace]]`.
  - Every row is checked at load. A bad row is refused with its name. The refusals: an unknown item, an amount of 0,
    heat or seconds out of range, a melt with a mould or a cast without one, a work that makes its own input, a name
    used twice, two melts of one input, a fuel twice, a furnace on a block that does not open the window, and
    rise or fall not over 0.
- **Protocol (appended).**
  - `ToServer::Choose { at, cast }`.
  - `ToClient::Furnace { at, f }` is sent to those with it open, only when it changed.
  - `ToClient::Fires` tells everyone which furnaces burn, so the model plays its `working` state: fire, smoke, sound and light.
- **The window.**
  - It shows the four slots, the fire's seconds left, the heat as a number and as a bar marked at the work's need,
    the work's arrow and its percentage, and the row of casts to click.
  - The recipe viewer shows melts and casts "in a crucible, at 1085 C" with a dim "No fill".

### Tunable numbers

| Number | Value | Where |
|---|---|---|
| Crucible top heat | 1100 C | `data/recipes.toml` `[[furnace]] top` |
| Heat rise | 100 C/s | `[[furnace]] rise` |
| Heat fall | 25 C/s | `[[furnace]] fall` |
| Air's heat | 20 C | `crates/engine/src/furnace.rs` `AIR_C` |
| Stick | 8 s, 700 C (too cool for copper) | `[[fuel]]` |
| Logs (any kind) | 40 s, 1150 C | `[[fuel]]` |
| Coal lump | 80 s, 1300 C | `[[fuel]]` |
| Melts to 1 ingot | 4 nuggets, 2 gravel, 1 malachite rubble, 1 chalcopyrite rubble; 1085 C, 8 s | `[[melt]]` |
| Casts | pick head 2 ingots, axe 2, shovel 1, knife 1, hammer 3; 1085 C, 5 s; clay mould kept | `[[cast]]` |
| Clay mould | 2 clay side by side (2 x 2) | `[[recipe]]` |
| Crucible | 8 clay round an empty middle (workbench) | `[[recipe]]` |
| Reach | 6 m | `crates/engine/src/items.rs` `BENCH_REACH` |
| Row limits | heat <= 4000 C, seconds <= 600 | `crates/engine/src/recipes.rs` `HEAT_MAX`, `SECS_MAX` |

## Consequences
- **Bloomery, blast furnace and kiln.** Each is another `[[furnace]]` row and window plus its rows. Nothing about
  them is built yet.
- **Known shortcuts.**
  - Nuggets and rubble now smelt, but the ore ledger (`ore-budget.md`) never held that metal; the gap is not solved.
  - One rubble gives one ingot. A crusher later should give more.
  - The copper hammer head has no use yet.
  - The air's heat is one constant, not the biome's or the planet's.
  - A lit fuel burns out all its seconds even after the work ends.
- **Changes from the brief.**
  - Shift-click from the pack sends fuel to Fuel, a mould to Mould and the rest to In.
  - New acts `sslot:<k>` and `cast:<name>` for scripted shots.
  - Save version 15.
  - A separate `Fires` message, so everyone sees the fire, not only those with the window open.
- **Tests.**
  - Engine: `the_crucible_needs_the_bench_and_its_rows_read`, `bad_furnace_rows_are_refused_with_their_name`, and
    `furnace.rs`'s heat, melt, no-fuel and cast/fits tests.
  - Server:
    - `a_crucible_melts_gravel_and_casts_a_head_with_the_ledger_exact`
    - `two_players_work_one_crucible`
    - `a_crucible_dug_full_drops_all_it_held`
    - `a_save_mid_melt_carries_on`
    - `a_far_or_made_up_crucible_is_refused`
    - `the_furnaces_are_kept_and_a_14_has_none`
    - the ignored `two_hundred_crucibles_cost`
  - Bot: `a_survival_bot_melts_gravel_casts_a_head_and_digs_with_the_pick`.
- **Cost.** 200 burning crucibles were measured once on the server tick (release build, 600 ticks). The tick went
  from 0.0615 to 0.0693 ms, about 0.04 µs per crucible, with no window open. That is nothing, so `docs/perf-debt.md`
  has no row. An open window costs one `ToClient::Furnace` per tick while its numbers change.
