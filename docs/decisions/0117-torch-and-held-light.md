# 0117 – The torch, and the light a held lit item gives

- **Status:** proposed (built; waits for the developer's look and merge)
- **Date:** 2026-11-14

## Context
The overseer, for the developer: "add torches", then "a held torch gives light". There was no torch: the only
light blocks were the lamp rows, cubes. A torch is the first block that is a model standing in its cell, the
first that needs a neighbour to hold it, and the first light that moves with a player.

## Options considered
1. **Which way a wall torch leans.** (a) An orientation per placed block (a state byte in the save and on the
   wire). (b) **Chosen:** derived from the blocks around it, `fall::held_from`: the floor first, then the first
   solid side in the fixed `SIDES` order. Nothing new is saved or sent; the server's placing rule and the client's
   drawing read the same function, so they cannot disagree. Cost: a torch on a floor beside a wall stands upright
   (the floor wins), and digging its wall can turn a wall torch to its next wall instead of dropping it.
2. **A torch placed in water.** Refused at placing (**chosen**: one clause in the server's `edit`), or popped
   off by the tick. Water that later floods a torch's cell does not put it out as a block; its flame and light
   are out while under water (client side, `flame`).
3. **Its flame.** A new "torch flame" row of `data/effects.toml` (the fire's gradient, a tenth of its size, glow
   1) played through the model's state `placed` at its point `flame`, so the flame flickers its light as the
   fire does; no new effect code. No smoke (it would be a second row per torch; left).
4. **Held light on the wire.** (a) A new message "player N holds X" on each slot change. (b) **Chosen:** one
   field, `net::Player::hand: u16` (the held item, 0 none), in the roster `ToClient::Players` the server already
   sends once a second and on a roster change. Two bytes a player a second; no new message. `net::Player` is not
   in recordings (`Ev` and `ToServer` are), so old recordings replay as before; an old save holds no players.
   Cost: another player's light follows them at the roster's 1 Hz (a shortcut, `gpu::others_lamps`).

## Decision
- `data/blocks.toml`: `torch` (id 57): not solid, `see = "cutout"`, `loose` (a bare hand takes it at once as
  itself), stack 64, `model = "torch"`, `light = [0.62, 0.36, 0.14]` (warm, each channel under the lamp's), and
  two new keys: `hangs` (needs a floor or a wall: placed only there, dropped as itself when it goes) and `flame`
  (out under water, held or placed). `hangs` on a solid or falling row is refused by name.
- `data/models/torch.toml` + `.png` (drafts, painted by `tools/paint_figures.py`): a 6 cm stick, 47 cm, with a
  charred head bound in cord, 19 cm; `item = 0.6`; a point `flame`, a state `placed` playing "torch flame".
- Engine `fall::held_from` and `fall::unheld`; server `edit` refuses a hanging block with no hold or in water;
  the tick drops each unheld one as a pile of one, counted in `item_made` so the ledger stays exact.
- Client: `models::hang` leans a wall torch 30 degrees out from its wall with its foot at it; a placed model
  with a `placed` state plays it near the camera unless a flame is under water. In hand, in a slot and lying, the
  torch is its model (ADR 0114's paths, unchanged).
- Held light: any held item whose block row has `light` is a lamp at the hand (`gpu::held_lamp`, which existed
  for the local player); now only while the hand is drawn (not with the inventory, a menu, the hammer or the
  bucket), out for a `flame` under water. Other players' held lit items are lamps at their hands from the roster
  (`gpu::others_lamps`), into the same `cap_lamps` list. A held lamp casts no shadow of its holder (there is no
  body to cast it; ADR 0118 adds the proxies and keeps this).
- No recipe: `/give torch` and creative's lamps tab. Roadmap: the torch's recipe with crafting.

## Evidence
Stills (play build, 768 planet, seed 1; `out/torch/`, not committed):
- `play_flame.png`: a torch placed by a survival right click at midnight, 8 frames of 300 ms. The flame plays
  in `run=play` and flickers (`play_flame_zoom.png`: a pale core with orange tongues that change frame to frame);
  the ground around is lit warm; the aim outline is the torch's cell.
- `wall_side.png` (`wall_side_zoom.png`): a torch with no floor, held by the block beside it, leans from it with
  its foot at the block; seen partly end-on, the lean reads as about 15-20 degrees.
- `held_walk.png`: walking at midnight with a torch in hand, 6 frames of 400 ms: the warm pool moves ahead over
  the ground with the player.
- `../figures/inventory.png`: five torches in the first slot (the model as the icon) and one in hand.
- Not shot: a torch lying dropped after a click in play (at the shot's spot the aim stops at the meadow grass in
  front of the torch; the server test takes it by a survival click and counts one torch).

Costs (`docs/perf-debt.md`): 200 torches in view take the frame from 5.6 to 24 ms p99 without effects and 44
with them (two lamps a torch with effects on); one held light +0.8 ms GPU.

## Shortcuts
- `shortcut:` another player's held light moves at the roster's 1 Hz; per tick when players are drawn moving.
- A wall torch's wall is derived, not chosen: a corner torch leans to the first solid side in `SIDES` order.

## Review
A fresh reviewer read `1bbeb76f..claude/painted-figures` (the crates). Found right: the figures, the bare arm's
box mapping, `painted_item()`, `net::Player::hand` (in no save, recording or hash), the loosened block tests.
Each finding below was fixed red first (the new test seen failing with the fix disabled) or is answered.

1. **Fixed (a ledger bug).** `fall::step_as` moves a falling or sliding block into any cell that is not solid,
   so sand over or beside a torch took its cell and the torch was gone with no item. A block entering a hanging
   block's cell now notes it in `Falls::knocked` (the cell it came from, the torch); the server drops it there
   as one torch (`knock_off`, counted in `item_made`). Other movers checked: water fills gaps and never wrote
   blocks (see 5); crack and undercut drops (0094, 0112) loosen rock only and the rubble falls through `Falls`;
   a placed block and the hammer's shapes go into air cells only (`edit::changes`), so a torch's cell is left
   as it is; there is no collapse yet. `/place` and `/structure` write over any block, a torch as any other,
   as before (commands, no items). Tests: `a_falling_or_sliding_block_knocks_a_torch_off` (engine: a fall onto
   a floor torch, a pile's top sliding into a wall torch's cell, nothing noted onto stone);
   `a_torch_is_knocked_off_by_sand_and_water_and_taken_by_hand` (server: one torch lies, the ledger exact).
2. **Fixed.** `edit::aim` stopped only at solid blocks, so a torch could not be aimed at. It now stops at a block
   drawn as a model too (solid or not), by its cell: the outline is the cell's, a left click breaks it, middle
   click picks it, the cell in front takes a place. A survival dig of a block with no dig time (nothing solid in
   the shape) went nowhere: the client only sent a held dig whose time was over 0; it now goes at the click, as
   itself (the server already took a dig whose time is 0). Tests: `a_torch_is_aimed_at_not_through` (engine),
   the server test's survival click. Not done: the outline by the model's bounds (it is the cell's).
3. **Fixed, and answered.** A change of the item in hand (a slot change, a pick-up or a give into the slot in
   hand) now sends the roster in that tick (`Client::sent_hand`); test `a_change_of_hand_is_sent_at_once`. Other
   players are not drawn in the world, so the client has no smoother position than the roster's: the light
   stays at the roster's position, once a second (the shortcut stands). Creative: the grid's take is a `Move`
   into the server's slots (ADR 0074), so a creative player's held torch is in them and lights the others.
4. **Answered.** The lean is read from the client's own cells; until an edit or a chunk arrives it can differ
   from the server's choice. Cosmetic only: the server's rule decides whether the torch stays.
5. **Fixed.** A placed flame that water covers pops off as one torch (`knock_off`), checked when water moved and
   once a second (a shortcut: every placed model is looked at); test: the server test's sea torch.
