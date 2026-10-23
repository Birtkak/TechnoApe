# 0027 – Wildlife: chickens, bears that hunt, carcasses, kelp and fish

- **Status:** accepted (built 2026-10-23)
- **Date:** 2026-10-23

## Context
Requested 2026-10-23:
- "small immersion things": half-eaten dead animals that got hunted, kelp
  and fish in the ocean, chickens and so on;
- bears that sometimes hunt you or other animals.

The animal system (ADR 0022) had passive sheep and pigs only. The world
has no way to hold a plant and water in the same voxel (no waterlogging),
and GDScript must not iterate voxels.

## Decision
- **Chickens** (data only, plus their model):
  - They lay an egg every 5-10 minutes (`lays`), and an egg fries into a
    fried egg (food).
  - They flap down from heights without fall damage (`slow_fall`), with
    their wings beating in the air.
  - They drop raw meat and feathers, and wheat seeds tempt them.
  - They spawn in plains, forest and savanna. The model has two legs and
    two wings.
- **Bears** are predators (`predator` in mobs.json). Their goals come
  before the animal goals:
  - **eat:** sit at a fresh kill for 10 s, head down, then regain a little
    health.
  - **attack:** a quarter of bears are aggressive (rolled once per bear and
    saved), and any bear hurt by the player is provoked for 30 s. They
    charge a player in survival within 14 m, bite for 3 hearts every
    1.2 s and shove the player back. Creative players are never attacked.
  - **hunt:** when hungry (every 4-10 minutes) a bear picks the nearest
    sheep, pig or chicken within 24 m and runs it down at 4.4 m/s. The prey
    bolts once the bear is within 8 m. A killing bite marks the prey, which
    leaves a carcass instead of its drops, and the bear eats.
  - Predators do not panic when hurt.
  - Bears spawn alone in forest, taiga, old growth and snowy taiga, with
    `rarity` 0.35 (most cells that pick a bear stay empty). Blueberries
    tempt and breed them. They drop meat and a bear hide.
- **Carcasses** (material 102, a micro model): a ribcage with scraps of
  meat over a dark stain. Bear kills leave one. The generator also places
  rare old kills in plains, forest, taiga, savanna and shrubland. Breaking
  one gives bones (and sometimes meat).
- **Ocean life** (`OceanLife`, presentation only, no voxels):
  - A native `WorldGen.water_floor_map` gives the floor and water level of
    unfrozen sea, lake and river columns around the player. It is rebuilt
    on a worker every 12 m moved and turned into instances 8 map rows per
    frame.
  - **Kelp:** dark swaying strands (one MultiMesh, the sway is in the
    vertex shader) in patchy forests 2-30 m deep, at most 10 m tall.
  - **Seagrass** in the shallows.
  - **Fish:** up to 6 schools of 8-12 fish swim between mid-water points,
    and only through water voxels.

  Kelp is not waterlogged voxels: a block in water would leave a box of
  water surface around it.
- `/summon chicken|bear [n] [hungry]`: hungry predators hunt at once.

## Consequences
- `test_wildlife.gd` covers:
  - the data and the new bodies;
  - a bear choosing the nearest prey (not other bears, not out of sight);
  - two bites killing a pig, marking the kill, counting it and making the
    bear eat;
  - prey panicking while a hurt bear does not;
  - about 25 % of bears being aggressive;
  - the carcass model and its drops;
  - old kills in plains;
  - the water map seeing the sea and not the ice.
- Autopilot runs:
  - a summoned hungry bear chases sheep and chickens, kills one and
    leaves a carcass;
  - kelp and fish show underwater;
  - spawn, forest and ocean are within the ADR 0009 budget.
- Kelp follows the generator's floor, not edits, and is not collectable.
  Fish are not catchable.
- Seen from above the water, thin kelp strands read as blue lines in the
  water's tint.
