# ADR 0168: Machines are furnace sized, open a screen and have their own guide

Status: accepted (2026-11-08). No new materials, no save change (a machine still saves `slots` and `output`).

## Context
Bug reports 2026-11-08_16-09-24 and 16-11-01: the newer machines (the ADR 0160 drives, the vent fan) were 0.5 m pieces
beside the 1 m furnace, and the one shared electricity guide did not say what a given machine takes, makes or needs. The
developer: machines at least 1 m with the furnace and crafting table as the base, impactful ones bigger, small parts like
the hand crank stay small; LV machines get screens like the furnace and crafting table, "from electric we will be
gregtech style"; every feature clear in the UI.

## Decision
**Size.** `BlockEntities.scale_of` defaults to 2 blocks (1 m) for every item tagged `machine` (`ItemRegistry.machine_works`
lists their work kinds), so a new machine cannot come out small; `SCALE` still names the exceptions: the crane and the
drawbridge hinge are 1.5 m, the coke oven's controller 0.5 m (its brick shell is the multiblock). Sources and parts (hand
crank, switches) keep their own sizes. `test_machine_size.gd` holds every machine to at least the furnace's cube.
Contraptions use the drive's width and height; the drawbridge pivot stays at deck height.

**Screen.** A plain right click on an LV machine (`MachineEntity`, not the builder) opens its screen in the inventory
screen's container layout, GregTech style: input slots left (rows of three), a progress arrow, the output slot right, a
power bar (the share of its watts it got last solve) and a status line (what it makes, or why not). `MachineScreen` is a
view like `TradeView`: it mirrors the machine's inputs and output into one Inventory and writes every change back, so the
machine's data, hoppers, saves and tests are unchanged. Sneak + right click keeps the old quick load / take; a bucket of
the scrubber's intake still pours in on a plain click.

**Guide.** A machine with process recipes gets a generated ponder scene `machine:<id>` (power, IN, OUT with labels) and
a strip of every recipe as item icons; the look panel names the clicks and the P guide.

## Consequences
The electric furnace (its own entity) and the builder keep their right-click behaviour; giving them screens is the same
view with their own slots. In multiplayer the screen edits the local entity like the furnace's screen does.
