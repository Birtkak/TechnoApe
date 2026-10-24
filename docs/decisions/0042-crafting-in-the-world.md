# ADR 0042: Crafting in the world

Status: accepted (2026-10-24)

## Context
ADR 0037 decision 3: tools and gear are made in the world (knapping, assembling on the ground or a
workbench, anvil, moulds, firepit, pottery, hides); the grid stays for bulk and simple recipes.
Knapping, clay forming, the pit kiln, casting, hides and the charcoal pit were already in the world
(ADR 0023, 0030). Still in the grid or a screen: every tool (head + stick + binding, shapeless), the
firepit (a furnace screen with a vessel slot), and nothing at all for smithing. Item definitions
listed a `durability` that nothing read.

The developer's calls (2026-10-24):
- assembly: **build it up on the ground**: the head first, then each part with a right click, every
  step drawn, over a free LotL layout or fixed workbench slots;
- this pass holds all four parts: **tool assembly, the firepit in the world, tool wear, anvil smithing**.

## Options considered
- Assembly: build up on the ground (chosen); LotL free laying (parts dropped near each other, a hint,
  a hit joins them: freer, harder to read); a workbench with fixed spots (clearest, tied to one block).
- Smithing rules: Vintage Story's full set (heavy hit, upset, split on a 3D voxel piece) or a
  smaller one. Chosen: column heights on an 8 x 8 grid, two moves (drive, split), volume kept.
- Heat: per-stack heat ticked in every inventory, an item per state ("hot copper ingot", like the
  molten crucible), or **a timestamp** on the stack. Chosen: the timestamp; nothing ticks.
- The first anvil: a cast copper anvil (Vintage Story, many pours) or **a stone anvil** (TerraFirmaCraft)
  from cobblestone. Chosen: stone; it needs a copper pickaxe for the stone, so it follows the first copper.

## Decision
1. **Per-stack data.** A stack is `{id, count}` plus data kept through every inventory move, the
   cursor, pickups, drops, chests and saves (`Inventory.make_stack` / `data_of`): `spoil` (ADR 0040),
   `wear`, `hot_until`, `work`. Items whose data tells them apart (wear, work) stack to 1, so merges
   never mix them; spoil and heat average by count.
2. **Tool wear.** `durability` is uses. One use per block's worth of voxels dug (a one-voxel break is
   one use), per till, per knife stroke on a hide, per firestarter lighting, per hammer blow, per
   throw, per craft that keeps the tool (the axe splitting firewood); an attack costs a weapon one
   and any other tool two (Minecraft). At its durability the tool breaks: the hand empties, a puff,
   "Your X broke." in the chat. A spear that wears out on a throw shatters where it lands. Creative
   tools never wear. The icon gets Minecraft's green-to-red bar, the tooltip "Durability n / N".
   The firestarter now has 24 uses.
3. **Assembly on the ground.** Recipe type `assemble`: `parts` in order (ids or #tags, 2..6). Right
   click a top face with a first part (a tool head): it lies there (`AssemblyEntity`, a flat work
   spot); each right click with the next part adds it, the last one finishes the tool, which pops
   out like a knapped head. A wrong part says what comes next; an empty hand takes the last part
   back; breaking the piece gives every part back. The parts are drawn as their own item sprites
   laid flat, head and handle on one centre and size, so the tool's icon grows on the ground. Every
   flint and copper tool moved from the grid to `assemble`; the older iron tools stay in the grid
   until the iron age.
4. **The firepit by hand.** No screen: right click lays on one fuel item (one kind, up to
   `Settings.firepit_fuel_max`), food (it cooks on the fire), ore one stone at a time (with a vessel
   on it), a vessel (crucible, cooking pot); a firestarter lights it; an empty hand takes things off
   (cooked food, the vessel, the input, the fuel while unlit). Holding right click keeps laying on.
   What lies on it is drawn as item sprites; the fuel shows as crossed sticks, one per item. The
   slots stay underneath (survival heat, saves and tests read them).
5. **Heat.** `Inventory.clock_s` is play time (advanced by Main, saved with the player). Hot metal
   carries `hot_until`; heat is `(hot_until - clock_s) / Settings.metal_cool_s` (40 s from full).
   A "heatable" item on a lit firepit's input climbs to full over `Settings.metal_heat_s` (8 s).
   Hot stacks glow on their icon; the tooltip counts the seconds left. Copper ingots are heatable.
6. **Smithing.** A stone anvil (7 cobblestone on a crafting table, `AnvilEntity`, a solid work block)
   takes a hot ingot: a lump of 32 cells (4 x 4, two high) in the middle of an 8 x 8 grid of
   6.25 cm cells; a picker offers the `smith` recipes for that metal (`input`, `layers` bottom first,
   at most 4, every cell resting on the one below, so a recipe is a height per column). With a
   hammer, **left click drives** the top cell of the aimed column one cell away from the player (the
   camera's facing snapped to an axis: walk round the anvil to push the other way); **right click
   splits** it off (lost). Metal above the shape is drawn red, missing metal as a green ghost. The
   piece is done when every column has its height. Below `Settings.smith_min_heat` the hammer does
   nothing: an empty hand takes the piece off as a work piece item that keeps its cells, recipe and
   heat; it heats on the firepit like an ingot and goes back on the anvil. Another hot ingot of the
   same metal adds a lump on top. Recipes: the five copper tool heads (casting stays the other
   route), the hammer head (also cast, in a new hammer mold) and the saw blade (only smithed). A
   copper saw (blade + stick) keeps in the grid and makes four planks from a log, twice the hand.
7. **Book and browser.** The browser shows assembly as the parts in order and smithing as the shape's
   height map with its ingots. Quest texts say how things are made now; a Copper Age chapter (ingot,
   hammer, anvil, red hot, smithing, saw) follows the Flint Age, with new events `assembled`,
   `heated`, `smithed`. `/kit smith` hands out an anvil, a firepit, a hammer, ingots and fuel.

## Consequences
- `craft_test=1` drives assembly, the firepit, smithing a hammer head (28 blows from the right sides,
  the piece taken off and put back half way) and a tool breaking through the player's own click
  code, and prints `debug craft {...}`; it runs headless with `craft_shots=0`.
- The grid's tool recipes are gone; old saves keep their tools (without wear, so as new).
- Per-stack data means a new key must be added to `make_stack`'s rules if it should merge or drop.
- Smithing is small on purpose (two moves, height maps). Upsetting, welding, quality and repair at the
  anvil can come with bronze and iron, as can metal anvils and tongs (hot metal does not burn yet).
- Numbers (durabilities, heat and cool times, 32 cells per ingot) are first guesses to tune in play.
