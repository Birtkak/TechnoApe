# ADR 0096: Three eras: material, steam, electrical

Status: accepted (2026-10-27)

## Context
The bronze and iron ages are in the game (ADR 0070). `progression.md` stopped at "steel, then machines,
not designed yet". The developer asked what comes after iron, wanting each step to feel rewarding by
making earlier mechanics easier. A mechanical power stage (water wheels, windmills) was proposed and
dropped.

## Decision
1. **Three eras.** Material age = stone, copper, bronze, iron (by hand, Vintage Story). Steam age = a slowly
   improving steampunk era. Electrical age = GregTech.
2. **No mechanical power.** Iron leads straight into steam.
3. **Steam age in three stages**, each built from the parts of the one before, each automating a chore
   the player knows by hand: low pressure (small coal boiler, steam forge hammer, steam sieve), high
   pressure (blast furnace and steel, steel boiler, ore crusher, pipes and tanks), steam works (hoppers,
   belts, steam drill, rails, steam turbine as the door to electricity). Steel moves into the steam age.
4. **Boilers explode** when they run dry while hot.
5. **The ore crusher doubles metal per ore block** (stage 2).
6. **GregTech mindset from late steam onward:** many intermediate products, needed in quantity; every ore
   is used sooner or later, so players plan expeditions and bring back all they can carry.

## Consequences
- Machines need plates, rods and other intermediates as items and recipes; recipe data grows a lot.
- Steam as a working fluid and boiler explosions touch the material simulation; how much is simulated
  (real steam gas vs. a boiler-internal number) is still open and gets its own ADR when built.
- Storage and hauling (minecart, rails, bigger chests) become core, not comfort.
- Recipe design rule: no ore is a dead end.
