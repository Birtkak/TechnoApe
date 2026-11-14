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
(filled below with the stills and numbers)

## Shortcuts
- `shortcut:` another player's held light moves at the roster's 1 Hz; per tick when players are drawn moving.
- A wall torch's wall is derived, not chosen: a corner torch leans to the first solid side in `SIDES` order.

## Review
