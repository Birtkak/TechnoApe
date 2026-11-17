# Crafting and the first machines

- **Status:** design proposal, 2026-11-10, designed fresh for this game (roadmap); reworked 2026-11-16 for
  ADR 0137 (the Steam Age is the first age, three furnaces, a process need not start in the world). It builds
  on the developer's calls in ADR 0009 (the ape, GregTech-style depth) and ADR 0026 (real rocks and minerals),
  and on `progression.md`'s two rules. How the ape's hands craft and how it learns are taste calls
  (`open-questions.md`); the data model below holds either way.
- Pillar 2: **crafting is decisions, not lists.** Depth over recipe count.

## Three ideas carry it

### 1. A process is a condition, and a machine holds it
A recipe never names a machine. It names a **process** and its **conditions**, and any machine that holds
them runs it. A machine is a block you place and open: a plain window with what goes in, the fuel or power,
what comes out, and the condition it holds now (ADR 0137).

| Process | The machine | The condition it needs |
|---|---|---|
| knap | none: struck by hand (ADR 0110) | a hard stone on flint |
| wash | a pan in the hand, later a washer | water flowing past |
| dry | drying rack, later a drying kiln | warm, dry air for a time |
| fire | the fire, later a kiln | a temperature held for a time |
| melt | crucible, bloomery, blast furnace | a temperature above what the metal needs |
| cast | a mould in the furnace's window, later a casting table | molten metal and a mould |
| crush | millstone, trip hammer, crusher | impact or grinding force |
| mill, saw, press | shaft machines | torque at a speed |
| cook, ferment, rot | the fire, oven, vat, cold store (stops it) | temperature and time |

- **A machine's condition is its own:** a furnace's heat follows its fuel and its draught (bellows by hand,
  then by shaft, then an electric blower), not the weather.
- **The world may still meet a condition, where that is cheap and reads well** (optional, not a rule, and
  never a step that must come first): a pan washes only in a running stream (`rivers-and-tides.md`), a rack
  dries faster on a warm dry day (`air.md`), rain puts a fire out. Each of these is content to add or leave
  out; nothing below depends on them.

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
- **Temperature is the early ladder** (ADR 0137). A wood fire 600°; the crucible 1,100° (copper melts at
  1,085°); the bloomery 1,400° (iron as a spongy bloom to hammer, it melts only at 1,538°); the blast
  furnace 1,600° (molten iron, steel), its later multiblock 2,200°. Each step is a furnace that holds more
  heat; a furnace works only what its heat reaches.

### 3. Elements are conserved
- A mineral is what it is made of: chalcopyrite is copper, iron and sulfur; cassiterite is tin and
  oxygen. Every recipe must balance: what goes in comes out as product, by-product, slag or gas.
- That gives the depth for free. Smelt chalcopyrite in a crucible and the copper comes out while the iron
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

## The Steam Age, step by step (*proposal*; the first content to build)

| When | The ape | Needs from the world | Process |
|---|---|---|---|
| 0-10 min | drinks, eats what grows, picks up sticks, flint, loose stones, cuts plant fibre; knaps a hand axe and a knife | fresh water, plants, a stream bed | knap |
| 10-20 min | lights a fire before night; digs clay and builds the crucible | tinder, dry wood, a floodplain's clay | fire |
| 20-30 min | pans copper nuggets, melts them in the crucible, casts a pick and an axe | the stream's gravel | wash, melt, cast |
| to hour 2 | walks to tin, burns charcoal, makes bronze; a hand crank lifts water and turns a millstone | the tin vein a walk away, wood | melt (two metals), mill |
| hours 2-6 | boiler, steam engine, wooden then bronze cogs; crusher, saw, bellows on one shaft; farm and oven | wood, water, bronze | the shaft machines |
| hours 6-10 | the bloomery, iron and lodestone; the steam generator | bog iron | the age ends at LV |

**Plant fibre** (the developer, 2026-11-14; ADR 0147): cut by hand from the small plants (grass, ferns,
reeds; how many from which is `gives` in `data/clutter.toml`). It is for **cord** and for **hafting tools**: a
head bound to its handle. Neither recipe exists yet; when cord and hafting are designed, they take fibre.

The megaproject at its end is a measurement (`progression.md`): **POWER!** is ingots out of a line over a
day with no ape's input counted.

## Machines as data
- A machine is a block entity (`world-data.md`) with a window, slots, ports onto networks (`networks.md`)
  and one or more processes it can hold conditions for, each with a range (the crucible reaches 1,100°) and
  a rate.
- A new machine that runs an existing process is a data row and a model. Only a new **process kind**
  needs code.
- The three furnaces are single blocks. The blast furnace later gets a multi-block version (a built stack,
  judged by a rule over its blocks, like a megaproject in small) that is hotter and makes more at a time.

## What this needs from the engine, in order
1. Items and inventories beyond blocks: designed in `items.md`. What the ape has learned: `journal.md`.
2. **A block that holds items and opens a window**: slots, a fuel slot, a heat readout. The crucible is the
   first, and every later machine is this block with other data.
3. The process runner: each machine's contents checked against its conditions on a slow tick, only where
   machines are.
4. Shafts (`networks.md` step 1), then the rest of the Steam Age's machines.
5. Optional, later: conditions met by the world (a rack in dry air, a pan in a stream), which need the
   temperature field near fires and the air cell.
