# ADR 0117: The electrical age (LV): machines as process recipes, storage, lamps

Status: accepted (2026-11-02), the overnight content run on top of ADR 0113 (networks) and ADR 0115 (process
recipes). Progression ends at the first electric tier (LV) for now.

## Decision
- **Every LV machine is `MachineEntity`** (`scripts/work/machine_entity.gd`): the item carries
  `"machine": {"eu_t", "inputs"}` and the machine runs the `process` recipes (ADR 0115) whose station is its
  id. A recipe's ingredient list is a spec per unit (`["copper_ingot", "copper_ingot", "copper_ingot", "tin_ingot"]`;
  a voxel item spec is one block of it, as the crusher's ore), `seconds` the work at full power, `info` the EU/t
  for the browser. Hand-worked like every work piece (right click loads a block's worth of anything a recipe
  takes, an empty hand takes the output); hoppers and belts load and unload them through `Transport`. The
  electric furnace stays a `FurnaceEntity` subclass (`ElectricFurnaceEntity`) so it runs every furnace smelt as is.
  A new machine is an item, its recipes and a line in `BlockEntities.create`'s machine list.
- **The machines:** lathe (ingot to 2 rods, rod to 4 screws), bending press (ingot to plate), wiremill (copper
  ingot to 8 wire), **macerator** (a block of ore to 3 crushed, the steam crusher gives 2: the doubling step of
  this tier), **alloy smelter** (2 inputs: 3 copper + tin to 4 bronze; iron + 2 coal to steel without the blast
  furnace), **assembler** (4 inputs: a board's parts to 2 primitive circuits, a motor from fewer parts). All cost
  two circuits and a hull. Numbers are defaults, developer to confirm.
- **Battery box** (`BatteryBoxEntity`): the solver gets a fourth role, `SINK_LOW`, fed only from what the sinks
  leave (`low = min(supply - flow, low demand, cap - flow)`), so a charging battery never starves a machine. It is a
  low sink while charging and a source while giving; it flips by itself: charging and given nothing while it holds
  charge, it offers next solve; giving and asked for nothing, it charges again (a second of lag each way: default,
  developer to confirm). Holds `battery_eu` (16 000), moves `battery_eu_t` (32).
- **Electric lamp** (`ElectricLampEntity`): a sink of 1 EU/t, `is_lit()` while fed, so `TorchLights` lights the
  room from it (the fire colour for now; a cooler lamp colour is a one-line follow-up in TorchLights).
- **Sinks are filled in order, not shared** (changed during the factory run): a machine either gets its whole
  EU/t or waits; a proportional share browned every machine out together the moment demand passed supply
  (33 > 32 EU/t). Sources and low sinks still share proportionally. The order is the entity store's (placement
  order): the first machines built keep running when power is short. Default, developer to confirm.
- **Machine pins** (`MachinePins`, `SimBubble.pin_at` + `keep_alive`): once a second WorkSystem keeps a pin
  around every machine with `needs_sim()` that the main window does not cover, nearest first, at most
  `machine_pins_max` (2). The **pump** is the first such machine: 4 EU/t, a bucket a second from the water under
  it through `Bucket.scoop_cells` (the bucket's own edit path over the bubble, a pin or the terrain), into a
  water tank beside it.
- **The coke oven is the first multiblock**: a controller in front of a hollow 3 x 3 x 3 of bricks; WorkSystem
  checks the pattern once a second (27 bubble-first reads) and the oven covers the hull's cells while formed, so a
  hopper anywhere on the hull feeds it. It burns no power: coal to coal coke (twice coal's burn, one coke makes a
  steel in the alloy smelter), logs to three charcoal. `enabled` on `MachineEntity` is the general hook: an
  unpowered machine runs while enabled.
- Units stay EU per block tick; one solve a second; the weakest cable caps a component (ADR 0113). The debug
  layouts use the steel boiler: a small boiler left alone bursts after a minute (ADR 0097), which killed the
  first unattended factory run.

## Not built (and why)
- Voltage tiers, cable loss, overload explosions: MV does not exist; they would only punish.
- Fluid channels on the solver: the steam age's `feed` carries the only fluid and works.

## Evidence
`test_electrical_age_2.gd` (pump over a fake world, tank hand-off, pins' needs_sim, the coke oven forming, coking
and breaking, saves); `/stations factory` in the game (hopper-fed macerator and furnace, battery, lamp, assembler);
`test_electrical_age.gd` (data, costs, lathe, macerator with hopper pull, two-input alloy smelter, assembler,
battery charging then feeding a lamp with the boiler cold, SINK_LOW solver, saves); `/kit electric` and
`/stations electric` in the game.
