# ADR 0141: Pollution is real matter

Status: accepted (2026-11-04). Builds on ADR 0113/0117 (machines on networks), 0119 (sim heat and states), 0122
(pipes), 0137 (idle sim scans cost nothing). Material ids 218..221, atlas tiles 190, 191, 215.

## Context
Factories made things out of nothing: a smelter or an electrolyzer left no trace. The world already has the matter
waste is made of (toxic sludge 53/54 from the sulfur caves, toxic gas, smoke, ash, wind), so waste should be cells in
the simulation, not a number on a machine: it piles up, flows downhill, fouls a river and has to be handled. It must
cost nothing where there is no waste, and stay inside the tick budget while a factory runs.

## Decision
**Waste outputs (data).** A recipe may carry `"waste": [{material, cells}]` (lv_recipes.json electrolysis: 64 sludge;
recipes.json alloys: slag + smoke, coke: smoke, steel and blooms: slag); an item's `"firebox": {ash, smoke_s}` makes a
fire-burning machine (boilers, bloomery, blast furnace) leave ash per fuel and smoke per second lit. A machine keeps
its waste in a buffer (saved) and `Waste.emit_all` (once a second, WorkSystem) puts up to `waste_cells_per_s` cells a
second into air at its **port**: liquids and powders 2 cells out of the back face at floor level, gases over the top.
A port that cannot take them fills the buffer to `waste_port_cells`; then the machine stalls and its look line says
"Waste port blocked: ...". A pipe touching the machine takes liquid waste: into an intake machine on the same run
(the scrubber), else out of the run's far end. A **chimney** column standing on the machine (`chimney`, 221, a
crafted block) vents gas over its top (up to `chimney_max_m`): higher stacks put smoke into the wind further away.

**Wind moves gases.** `SimWorld::set_wind` (WorkSystem sets it from the weather's wind once a second): a gas with
`wind_drift` (smoke) steps one cell downwind with that share of its moves. One small function in `update_gas`.
**No wind under a roof** (developer, 2026-11-04): a drifting gas whose column has anything but gas or fire within
`ROOF_SCAN` (96 cells, 12 m) straight up is sheltered, so smoke in a cave, mine or room rises and pools under the
roof and a furnace down there needs a vent shaft. The sim keeps no per-column sky flag, so this is a bounded scan,
only after the drift roll passes; a cavern roof higher than 12 m counts as sky.

**Fouled water** (`fouled_water`, 218, fluid slot 5, a murky brown-green, nearly opaque, glossy with an oil-like
sheen, density 0.95: it floats on clean water as a slick, so a fouled river shows from the bank). The old dilution (sludge + water ->
water) became: sludge touching water makes two fouled water (0.15); fouled water spreads into moving water (0.002);
against **still** (generated) water it spreads at 0.001 and is diluted back at 0.0015, so a plume in a lake or river
stays bounded and clears once the source stops, while a dug pond of moving water stays fouled until drained. Crops
whose water scan finds fouled water first wilt (`Farming`, stat `wilted`); animals do not step into it and are hurt
standing in it; a bucket of it drinks as `poison_s` 20 (water buckets drink now, aimed at the sky). The aimed-cell
line names fouled water, sludge, waste ground and slag; the first fouled cell the player looks at writes the
journal's `phenomenon:fouled_water` entry.

**Erosion and settling.** Sludge on soil and soft rock (dirt, grass, clay, sandstone, farmland, limestone, chalk,
red earth, savanna grass) turns it into `waste_ground` (219, weak, shovel) at 0.003; on gravel at 0.02, and on its own
waste ground at 0.004 sludge settles into it: a **settling pond** is a gravel-bed basin the port drains into. These
reactions are `"passive"`: the ground is not made reactive, so dirt and stone stay inert and sleep. Only sludge and
fouled water carry `"react_awake"`: a resting cell of them that touches a partner keeps its chunk awake until the
partner is used up. Erosion goes one layer per contact and stops when the sludge is spent; sludge on stone sleeps.

**Handling.** `scrubber` (LV, 300 W, data in lv_items.json) takes sludge by pipe or bucket into a 4-bucket tank and
makes sulfur and clean water (`scrub_sludge`), and turns waste ground back into dirt (`scrub_waste_ground`). `slag`
(220, powder) is smelting waste, shovel to clear.

All the waste reactions, the fouling ones included, are passive. A passive reaction also rolls its probability
before a partly full liquid cell is spent (`spend_partial`): with the ADR 0119 order a thin film of sludge vanished at
its first touch of water or gravel, so most of a machine's sludge disappeared instead of fouling or settling (a first
filmstrip: 256 sludge cells into a stream left 56 fouled cells; after: 732).

**Kernel changes** (small, self-contained): `react_awake` / `wind_drift` material fields, `passive` reactions,
`set_wind`, `wind_drift()`, `react_keep_awake()`, `spend_partial()`. No GEN_VERSION, streaming or generator change.

## Consequences
- `/stations pollution` lays a stone slab 4 m up with two streams and two loaded electrolyzers on a charged battery:
  one fouling its stream, one draining into a settling pond beside a clean one. `/stations smoke` lights a log pile
  in a room dug 4 m down with a vent shaft. `/stations lv` now leaves sludge behind its
  electrolyzer.
- A boiler indoors without room behind it chokes on its ash and stops: dig it out, pipe it, or give it air (kept,
  developer 2026-11-04). The gravel pond is the settling tank; no tank block.
- The new looks need no extra warmup: fouled water, waste ground, slag and chimney are library models, which
  `MaterialWarmup` draws whole (fluid slot 5 included), and no particle was added. Factory runs after: no frame over
  100 ms (max 62 and 88 ms).
- Tests: `test/test_pollution.gd`; `test_sulfur_caves` now expects fouled water.
