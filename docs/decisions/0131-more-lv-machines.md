# ADR 0131: More of the electrical age (LV): six machines and the chains they open

Status: accepted (2026-11-03), content on top of ADR 0117 (MachineEntity, process recipes). Nothing past LV.

## Context
The developer asked for more LV recipes. ADR 0117's machines turn, press, draw, grind, alloy and assemble; the
age still made rubber only from tar and sulfur, smelted crushed ore only in the crucible or bloomery, and had
no use for tin past bronze nor for steel past plates.

## Decision
- **Six machines**, each an item with `machine` {eu_t, inputs}, its `process` recipes and a `MachineEntity.DOOR`
  entry; all built from two primitive circuits and a machine hull. Numbers are defaults, developer to confirm.
  - **Extractor** (8 EU/t): any log to sticky resin, resin to three rubber, raw rubber to two: rubber from a forest.
  - **Cutter** (8 EU/t, built round a steel saw blade): a log to six planks of its wood (a saw by hand gives four),
    glass to three lenses, an iron plate to a gear.
  - **Extruder** (16 EU/t, built of steel plates): bronze or copper ingot to two pipes, iron ingot to three gears or
    three rods, steel ingot to two steel rods, tin ingot to four tin cans.
  - **Canning machine** (4 EU/t, two inputs): a tin can and two cooked meat or fish, or three carrots, potatoes or
    beans, to a can of food that never spoils (no `spoil_days`).
  - **Electric compressor** (16 EU/t): two coal coke to two carbon electrodes (four coal make one), three plant
    fibre to two cordage.
  - **Electrolyzer** (24 EU/t, two inputs): two crushed copper, tin or iron ore and a carbon electrode to two
    ingots: crushed ore to metal with no crucible or bloomery. 24, not 32: a whole turbine's 32 EU/t is never
    quite reached (rpm), and a machine takes all its EU/t or waits (ADR 0117), so 32 never ran.
- **Assembler** gains insulated wire (three wires and a rubber to three, the table sleeves one per rubber) and
  electric pistons (a steel rod, two iron plates and a motor to two).
- The chains: logs -> resin -> rubber -> insulated wire -> circuits; macerator -> crushed ore -> electrolyzer (coke
  -> electrode) -> ingots; tin -> cans -> canned food; steel -> rods -> pistons -> machines.
- **Data in its own files** (`data/lv_items.json`, `data/lv_recipes.json`, loaded after the main files like the
  Stone Age's) to keep merges clean. Power stays in `machine.eu_t` and recipe `info` (the unit change converts them).
- **One list of machine kinds:** `BlockEntities.create`, its footprint scale (1 m, like the lathe), `is_work` and
  the work placement read `MachineEntity.DOOR`, so a machine no longer needs a line in four lists.

## Second pass (2026-11-03): a use for every output, the Workshop chapter
- **Uses:** a can of food empties to its tin can when eaten (`empties_to`, like the stew's bowl); sticky resin
  is a binding and a tar (hafts, raw rubber) and lights four torches on a stick; rubber makes rubber boots
  (feet, armour 1, warm), a diving mask with two lenses (head, `clothing.air_x` 3: breath under a liquid lasts
  three times as long, `Player.air_mult`), a rubber belt recipe (six belts, not four) and the heavy copper
  cable (16 mm^2: `cable` {0.00053 ohm a block, 75 A} against the hide cable's {0.0014, 40}); two lenses in a
  copper pipe make the spyglass (held, the zoom key zooms by its `zoom` 8 instead of Settings.zoom_factor); a
  steel pick head on two gears and steel rods makes the hand drill (tier 4, speed 1.8, no steam).
- **Cables keep their item:** laying a cable records which cable item it was (like a pipe), so a gauge is data.
- **No hydrogen or oxygen yet:** a process recipe has one output and no way to hand a bucket back, so
  electrolysing water would eat the bucket; it waits for a machine with a byproduct slot.
- **The Workshop chapter** in quests.json (after the Electrical Age): extruder -> canning machine -> food that
  keeps is its road (final: any canned food); extractor, rubber boots, heavy cable, cutter, spyglass, diving
  mask, hand drill, electric compressor and electrolyzer are side roads. Written by `game/tools/content_lv2.py`.

## Evidence
`test_lv_machines.gd`: every machine is data (work, machine, DOOR, 1 m, circuits and hull, runs recipes), canned
food keeps; one recipe per machine (and the assembler's insulated wire) loaded by hand and run on a boiler,
turbine and cable line gives its output; every machine output is an ingredient somewhere, cans come back,
the heavy cable loses less, carries more and is saved with its gauge, the Workshop chapter tasks every machine. `/kit electric` carries the six machines and their inputs.
