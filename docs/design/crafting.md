# Crafting and the first machines

- **Status:** design proposal, 2026-11-10, designed fresh for this game (roadmap). It builds on the
  developer's calls in ADR 0009 (the ape, the starter, GregTech-style depth) and ADR 0026 (real rocks and
  minerals), and on `progression.md`'s three rules. How the ape's hands craft and how it learns are taste
  calls (`open-questions.md`); the data model below holds either way.
- Pillar 2: **crafting is decisions, not lists.** Depth over recipe count.

## Three ideas carry it

### 1. A process is a condition the world can meet, or a machine can
`progression.md` rule 2: every process starts in the world and ends in a machine. So a recipe never names
a machine. It names a **process** and its **conditions**, and anything that provides them runs it:

| Process | In the world (starter) | In a machine (later) | The condition it needs |
|---|---|---|---|
| wash | a pan of gravel held in a running stream | washer | water flowing past, at some speed |
| dry | on a rack in the sun and wind | drying kiln | warm, dry air for a time |
| fire | clay in a fire, then a pit kiln | kiln, electric kiln | a temperature held for a time |
| melt | a clay crucible in charcoal, blown by bellows | furnace | a temperature above the metal's melting point |
| cast | poured into a clay or sand mould | casting table | molten metal and a mould |
| crush | a hammer stone on a flat rock | millstone, trip hammer, crusher | impact or grinding force |
| mill, saw, press | by hand, slowly | shaft machines | torque at a speed |
| cook, ferment, rot | by the fire; in a pot; by itself | oven, vat, cold store (stops it) | temperature and time |

The conditions are **measured, not declared**: the stream's speed is the river reach's
(`rivers-and-tides.md`), the air's warmth and dryness are the air cell's (`air.md`), a fire's temperature
follows its fuel and its draught. Clay dries slower on a wet day. A machine is a box that holds the
condition steady whatever the weather, which is exactly why you build one.

### 2. Materials times forms
- A **material** is a row of properties: melting point, hardness, strength, conductivity, density, what
  it is made of. A **form** is a shape: nugget, crushed ore, dust, ingot, plate, rod, wire, gear, pipe,
  block.
- Items are material x form where the pair makes sense, generated from the two tables. Adding a metal is
  one row, and it arrives with its wire, its gear and its pipe.
- Properties decide use, so substitution is real (pillar 2: "tag-based ingredients so substitutions
  emerge"): a recipe asks for "a gear with strength at least S" or "a wire". Bronze, iron and steel gears
  all fit a slot and differ in what load they survive; copper and aluminium wires differ in loss and
  weight. A tin shortage is answered by design, not by a missing recipe.
- **Temperature is the early ladder.** Wood fire 600°, charcoal with bellows 1,100° (copper melts at
  1,085°), a bloomery 1,250° (iron as a spongy bloom to hammer), a blast furnace 1,500° (steel). Each
  step is a build that holds more heat, not an unlock.

### 3. Elements are conserved
- A mineral is what it is made of: chalcopyrite is copper, iron and sulfur; cassiterite is tin and
  oxygen. Every recipe must balance: what goes in comes out as product, by-product, slag or gas.
- That gives the depth for free. Smelt chalcopyrite in a fire and the copper comes out while the iron
  goes to slag and the sulfur to smoke (pollution, `air.md`). Process it properly later and all three are
  products. One ore, several routes, real trade-offs (ADR 0026).
- It makes the **material ledger** (roadmap) honest: a body's ledger is in elements, so "how much copper
  does Terra still hold" is one number however the copper is locked up.
- It is a machine-checkable rule: a data test balances every recipe and fails on any that makes matter.

## The data
- `data/materials.toml`, `data/forms.toml`, `data/items.toml` (things that are not material x form: food,
  tools, parts), `data/recipes.toml`, `data/processes.toml`. All with a schema and tests, built into the
  binary like the blocks (ADR 0013).
- A recipe: the process; inputs by item, by tag, or by form with a property bound; outputs with amounts;
  conditions (temperature range, time, fluid, power); the tier it belongs to.
- Tests on the data alone, no game running:
  - every recipe balances by element;
  - **reachability:** starting from what the world gives (the bake's blocks and plants), every item of
    tier N is reachable using only processes of tier N or lower;
  - no loop of recipes returns more than it took;
  - the cost of every machine and megaproject per tier, summed, stays near `ore-budget.md`'s demand row.

## The starter, step by step (*proposal*; the first content to build)

| Minutes | The ape | Needs from the world | Process |
|---|---|---|---|
| 0-10 | drinks, eats what grows, picks up sticks, flint, loose stones | fresh water, plants, a stream bed | none: hands take loose things |
| 10-20 | knaps a hand axe, lights a fire before night | flint, tinder, dry wood | crush (knapping), fire |
| 20-35 | digs clay, shapes a pot and a crucible, dries and fires them | a floodplain's clay, sun or the fire | dry, fire |
| 35-50 | burns wood to charcoal in a covered pit | wood, earth | fire without air |
| 50-70 | pans copper nuggets from the stream, melts and casts an axe head | the stream's gravel and its flow | wash, melt, cast |
| hours 1-3 | walks to tin, makes bronze; a hand crank lifts water and turns a millstone | the tin vein a walk away | melt (two metals), mill |
| hours 3-6 | boiler, steam engine, wooden then bronze cogs; crusher, saw, bellows on one shaft | wood, water, bronze | the shaft machines |
| hours 6-10 | farm and oven; water wheel or windmill; bloomery iron; lodestone; the dynamo | bog iron, a river or a windy ridge | the starter ends at LV |

The megaprojects at its two ends are measurements (progression): **Home** is a room's temperature over
a night; **The Works** is ingots out of a line over a day with no ape's input counted.

## Machines as data
- A machine is a block entity (`world-data.md`) with ports onto networks (`networks.md`) and one or more
  processes it can hold conditions for, each with a range (this furnace reaches 1,200°) and a rate.
- A new machine that runs an existing process is a data row and a model. Only a new **process kind**
  needs code.
- Multi-block machines (a blast furnace is a built stack) are judged by a rule over their blocks, like a
  megaproject in small.

## What this needs from the engine, in order
1. Items and inventories beyond blocks (today the inventory counts block kinds).
2. The temperature field near fires (the air cell plus local sources) and rooms.
3. The process runner: things lying in the world (a pot by a fire, a pan in a stream) checked against
   conditions on a slow tick, only where they are.
4. Shafts (`networks.md` step 1), then the rest of the starter's machines.
