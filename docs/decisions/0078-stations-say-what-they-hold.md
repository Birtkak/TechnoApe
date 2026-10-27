# ADR 0078: Stations say what they hold, load on a click, and burn

Date: 2026-10-27. Status: accepted.

## Context

Five bug reports from one smithing playtest (2026-10-27_15-34-22, 15-50-30, 15-58-30, 15-59-22, 16-00-17):
the look-at panel named every work piece "Work Spot" and never showed water or lava; nothing told the player
how much coal was in a forge or what lay on a fire; loading a firepit, forge or bloomery needed a one-second
hold (ADR 0043); smithing gave no cue what to do or that the metal had gone cold; and a lit campfire did not
hurt to stand in, while fire voxels do.

## Decision

1. **Look-at panel.** `Main._describe_block` returns `{title?, line?}` and the player merges it over its own
   guess: a work piece is titled by its item (Firepit, Forge, Stone Anvil, Sieve; Knapping, Clay forming,
   ground storage by the item lying there). `FirepitEntity.look_lines()` (fire and fuel left, fuel count in
   items, vessel, what is on the fire with its heat or cook progress, what is ready) and
   `AnvilEntity.look_lines()` (metal and shape, heat, cells short, extra cells, how the hammer works) fill
   the panel; the HUD draws one line per `\n`. The aim ray passes through liquids, so the player marches
   the ray up to its hit and names the first liquid voxel; under a liquid the panel keeps naming blocks.
2. **Plain click.** `WorkSystem.hold_key` no longer claims firepits: a right click lays one item on (holding
   repeats at the placement pace), an empty hand takes one off. Holding stays for ground pickups.
3. **Smithing cues.** A blow on cold or blocked metal plays `ui/error`; a blow that brings a column to its
   shape plays `ui/click`; a finished piece puffs and plays `ui/confirm` with a chat line; picking a shape
   says how to use the hammer (`AnvilEntity.HOW_TO`); the metal going cold on the anvil emits `cooled`, which
   hisses (`craft/quench`) and steams.
4. **Fire pieces burn.** `Player.fire_at` (wired by Main): a lit firepit, forge or bloomery at the feet or
   just under them counts as touching fire for the stains, so it sets the player alight like a fire voxel.

## Consequences

No new sounds, materials or ids. The scripted checks (`craft_test`, `metal_age_test`) load fires with
clicks (`CraftTest._put_on`). The liquid march costs up to ~2 voxel reads per voxel of reach per frame.
