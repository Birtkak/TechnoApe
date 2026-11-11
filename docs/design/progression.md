# TechnoApe – Progression: from ape to orbit

- **Status:** living document, written 2026-11-09 and reworked the same day after the developer revisited
  power. The developer's calls are in ADR 0009 and 0010; the tiers, milestones, megaprojects and rules below
  are a *proposal* built on those calls, to be cut and changed as content is made. Nothing here is in the
  first playable (roadmap step 1).

## The spine
You are an ape with bare hands on a small, finite planet. A **starter** of 7-10 hours takes you from stone
and fire (copper after about an hour) to steam and turning cogs. From then on the game runs on **electricity
in tiers** (LV, MV, HV, ...), as in GregTech: the same system all the way up, with better cables, machines
and sources. Each tier tames a survival need and ends in **a megaproject you design yourself**, which the
planet judges by its result, not its shape. Orbit and Luna are visited about halfway; living there is the
second half. After the moon base an **exotic resource** opens the space technology that leaves the planet,
and by then a whole moon or planet can be **mined out and drained**. Power stays the main thing to the end.

Three rules keep the tiers from becoming a list:
1. **Every need starts as a chore and ends as a factory** (ADR 0008).
2. **Every process starts in the world and ends in a machine** (*proposal*): ore is first washed in a real
   stream, dried in the real sun, melted in a real fire. The machine that comes later is that process in a
   box, and an ape learns by seeing it happen.
3. **The starter teaches the whole game.** Boil water, turn a shaft: the starter's fire, boiler and cogs are
   the inside of every later power plant. A reactor is a hotter fire under a bigger boiler.

## The ages at a glance

| Age | Hours | Need it tames | Megaproject and its rule |
|---|---|---|---|
| **Starter: stone** | 0-1 | warmth, water | **Home:** a closed room that stays above freezing through a whole night, with water and food inside. Ends the nomad start. |
| **Starter: steam and cogs** | 1 to 7-10 | hunger (a farm and a mill) | **The Works:** one shaft drives a line that turns raw ore into ingots for a full day with no ape touching it. |
| **LV** | 10-15 more | food (cold storage), light | **Steady grid:** a set power held for a full day and night. A rain-fed dam, steam on grown charcoal, wind and batteries: your design. |
| **MV** | 15-20 | distance: ore from other faces | **Ring railway:** a closed track that goes once around the planet. |
| **HV** | 20-25 | the night, everywhere; clean air | **Power ring:** a set power, without burning fuel, for three full days. A ring of panels, big storage, or a tower at a pole that always sees the sun. |
| **EV** | 25-30 | air (tanks, a suit) | **First orbit:** a satellite in a stable orbit that passes over every face. It maps the deposits that are left. First visit to Luna. |
| **IV** | 30-40 | every need, in sealed rooms | **Moon base:** keeps an ape alive on Luna for a set time with nothing delivered, and sends a set tonnage of ore to the planet. Unlocks the exotic resource. |
| **Exotic** | open | closed loops: nothing is carried in | **The Ark:** a ship with a living room that sustains itself and a drive fed by the exotic liquid. Leads to the next planet. |

- Starter to HV is one planet and the three networks: the game up to the first visit to space, and the
  content target after the first playable.
- IV and the exotic age need Luna as a place to live and, for the Ark, a destination.
- The hours are the developer's scale (2026-11-09): about 110-150 hours to the moon base.
- The metals, confirmed by the developer: copper in the first hour, bronze through the steam age, iron at
  its end, steel as the gate to MV.

## Milestones per age
What each big step is and what it gives you. The last milestone of an age opens the next.

### Starter, part one: stone (the first hour)

| Milestone | What it is | What it gives |
|---|---|---|
| **Fire** | sticks, tinder and a spark from two stones | warmth, light, cooked food, fired clay; the night stops being deadly |
| **Stone tools** | a knapped hand axe, a digging stick, a hammer stone | wood and rock at brush speed; bare hands only take loose things |
| **Clay pot** | shaped by hand, fired in the fire | carried water: you can leave the pond |
| **Pit kiln and charcoal** | a hole, wood, a cover of earth | heat enough to melt metal |
| **Copper** | nuggets washed out of a stream bed, melted, cast in a clay mould | metal tools, and the stuff of boilers, pipes and wires. About one hour in. |

### Starter, part two: steam and cogs (until 7-10 hours)

| Milestone | What it is | What it gives |
|---|---|---|
| **Tin and bronze** | tin is a walk away, not at your door; melted with copper | the first journey; bronze for boilers, pipes and gears |
| **Hand crank and pump** | your muscle turns a shaft; the shaft lifts water | water uphill into a tank; the same crank turns a millstone (flour, and crushed ore that gives more metal) |
| **Boiler and steam engine** | pumped water over a firebox; steam pushes a piston | a shaft that turns without you: the first thing that works while you are away |
| **Cogs, shafts, gearboxes** | wood first, bronze later | one engine drives many machines; trade speed for force |
| **Shaft machines** | crusher, saw, bellows, trip hammer, fan, a slow conveyor | planks, plates, more metal per ore, items that move without hands |
| **Farm, mill and oven** | crops on rain or a dug channel | food goes from a chore to a small line |
| **Water wheel and windmill** | rotation from the stream and the wind | power without fuel, at the mercy of the weather |
| **Iron** | a bloomery blown by shaft bellows | iron tools, a hand cart on rails, and with lodestone the magnets a dynamo needs |
| **Dynamo** | magnets and copper windings on a shaft | LV. The starter is over. |

### LV

| Milestone | What it is | What it gives |
|---|---|---|
| **Wire and the first grid** | copper wire from the dynamo to the machines | power goes round corners: machines stand where you want them, not where the shaft runs |
| **Electric motor and machines** | electric furnace, crusher, press | smaller, faster machines; shafts retire into the power plant |
| **Battery** | lead and acid cells | power through the night; the first storage |
| **Electric light** | a bulb | work and farming at night and underground |
| **Cold store** | a closed room cooled by a compressor | food stops spoiling: the food need becomes a factory |
| **Belts and sorters** | powered conveyors that split by item | a line that feeds itself |
| **Steel** | a blast furnace with an electric blower | the gate to MV |

### MV

| Milestone | What it is | What it gives |
|---|---|---|
| **Transformer** | steps voltage up and down | a line across a face that does not lose everything on the way |
| **Mining drill and ore line** | a powered drill; washer and centrifuge | bulk digging; two to three times the metal from the same finite ore |
| **Electrolyser** | splits water and ores with current | aluminium; hydrogen and oxygen. The first way water leaves the cycle. |
| **Railway and locomotive** | steel track, a steam or electric engine | bulk ore from other faces |
| **Water treatment** | settle, filter, boil | clean water out of dirty |
| **Chimney and first filter** | height, then a filter bed | the first answer to smog |

### HV

| Milestone | What it is | What it gives |
|---|---|---|
| **Pylons and the planet grid** | high-voltage lines around the planet | power made on the day side, used on the night side |
| **Turbine** | a big boiler's steam through a turbine | several times the power per fuel of a piston engine |
| **Storage at scale** | water pumped uphill, flywheels, battery banks | the night survived on stored day |
| **Solar field** | panels that want sun and no cloud | clean power that follows the day |
| **Circuits and logic** | sensors, switches, arms | a factory that decides for itself |
| **Scrubbers** | take pollution out of air and water | a planet that gets cleaner again |
| **Plastics and rubber** | chemistry on grown crops and wood | seals and insulation: what higher voltage and suits are made of |

### EV

| Milestone | What it is | What it gives |
|---|---|---|
| **Chemical plant** | reactors and distilling columns | fuels, plastics in bulk, explosives for mining |
| **Fuel and engines** | hydrogen and oxygen, or methane | the jetpack, vehicles, rocket engines |
| **Pressure suit** | sealed, with air tanks | high altitude and vacuum: the air need in a bottle |
| **Computer** | guidance and the planet map | satellites; control over distance |
| **Rocket or launcher** | a chemical rocket, or an electric rail up a mountain (orbit is 102 m/s) | orbit |
| **First moon landing** | you, a suit, a flag | samples of the ores the planet lacks |

### IV

| Milestone | What it is | What it gives |
|---|---|---|
| **Sealed habitat and airlock** | a room that holds air and heat in vacuum | a place to take the suit off |
| **Life support loop** | plants, a water recycler | no deliveries needed |
| **Radiators** | panels that shed heat to space | the limit on every machine and power plant off the planet |
| **Reactor** | fission: radiation, shielding, waste | steady power without sun |
| **Mass driver** | an electric rail on Luna (orbit there is 17 m/s) | ore to the planet for almost nothing |

### Exotic

| Milestone | What it is | What it gives |
|---|---|---|
| **The raw exotic** | a material found only off the planet | the feed for the refinery |
| **The refinery** | makes the quantum liquid; draws more power than everything before it | warp fuel |
| **Fusion** | fuel from Luna | the power the refinery asks for |
| **Planet eater** | a miner that takes whole chunks; pumps that empty seas | a moon, then a planet, mined out and drained: the mass the Ark needs |
| **Orbital yard** | a dock in free fall | things too big to launch |
| **Warp drive** | fed by the quantum liquid | the next planet |

## Power, quite real
Three things flow, on one network engine (`fundamentals.md` 2.5):

| Network | Carries | Rules worth playing with |
|---|---|---|
| **Pipes** | water, steam, later fuels and the exotic liquid | pressure and temperature; steam loses heat to cold air (more at night), a boiler without water bursts |
| **Shafts and cogs** | speed x torque | gear ratios trade one for the other; no storage; an overloaded shaft slows everything on it |
| **Wires** | volts x amps | see below |

**Electricity** (*proposal*, the detail is its own ADR and benchmark):
- **Every generator is a shaft turning a dynamo.** Water wheel, windmill, steam engine, turbine, reactor: all
  make rotation first. Solar panels are the one exception. Rotation never goes obsolete; it moves inside the
  power plant.
- **Power = volts x amps.** A cable has a current limit and loses power with the square of the current, so
  low voltage only reaches across a base. To cross the planet you step the voltage up with a transformer and
  down again at the other end. That is why tiers exist, not just bigger numbers.
- **Tiers as in GregTech:** LV, MV, HV, EV, IV, then the exotic tiers. A tier is a voltage, the cables and
  transformers that survive it, and the machines built for it.
- **Three ways to get it wrong:** too little supply and the voltage sags, machines slow down; too much
  voltage and a machine burns out; too much current and the cable heats, melts and can start a fire.
- **In space,** waste heat only leaves through radiators: a power plant on Luna is limited by how fast it
  sheds heat, not by how much sun it gets.

## The planet's air: water, weather, pollution, radiation
The developer's call (ADR 0010): the planet runs these four by itself. *Proposal:* they are one system, a
coarse grid of air cells around the whole planet. Each cell holds heat, water, dirt and wind and passes them
to its neighbours. It is not a real weather model; the rules are picked for play.

- **Water goes round.** Steam that is let out goes into the air and comes back as rain, so ponds and lakes
  refill by themselves. Open water also evaporates in the sun. Rain falls where wet air cools: at dusk (which
  circles the planet every 30 minutes) and on high ground. It lands on the surface and runs downhill with the
  ordinary water simulation. The water is **counted, not fixed:** none appears or vanishes by itself, but you
  can take it out of the cycle (see "Eating a planet").
- **Weather** is what the air cells show: clouds where the air is wet, rain and snow where it cools, wind
  driven by the warm day side and the cold night side, a storm where both are strong. It matters to play:
  rain waters crops, fills ponds and puts out fires; clouds dim solar panels; wind turns windmills and
  carries smoke; lightning hits the tallest thing.
- **Pollution:** smoke and waste from burning and from chemistry go into the air cell, drift with the wind
  and come down with the rain into water and soil. The planet is finite, so it has nowhere else to go. Enough
  of it and crops fail, water needs treating before you drink it, the ape coughs and the sun dims. The
  answers are builds: a tall chimney moves it, filters and scrubbers take it out (and leave a waste to store
  or process), clean power stops making it, plants clean it slowly.
- **Radiation** has two sources. *Rock and machines:* radioactive ore, reactors and their waste radiate;
  blocks between you and the source stop it, thick and dense ones best. Leaked material is pollution that
  fades slowly and travels with the same wind and rain. *Space:* the planet's air shields the surface. In
  orbit and on Luna the sun's radiation is unfiltered and solar storms are Luna's weather, so living
  there means living under rock. Dose builds up in the body and fades slowly.

Where each one bites:

| System | First felt | Becomes a problem | Tamed by |
|---|---|---|---|
| Water cycle | Starter: the boiler's pond refills in the rain | LV: a dam is only as good as its rain | MV: reservoirs, pumps, cooling towers |
| Weather | Starter: rain kills the fire, the wind is cold | LV: wind and cloud make power unsteady (**Steady grid**) | HV: storage and lines around the planet |
| Pollution | Starter: smoke in a closed room | MV: coal and steel, smog, bad rain on the crops | HV: filters and clean power (**Power ring**) |
| Radiation | EV: first orbit, the suit | IV: reactors, a moon without air | IV: shielding, bases under rock, waste stores |

Cost and checks:
- The grid is small. On the 1 km planet, cells 32 m wide are 48 x 48 per face: about 14,000 columns, a few
  layers high. It updates about once a second, spread over ticks, for the whole planet whether chunks are
  awake or not. The 4 km planet has 16 times the area, so the cell size scales with the planet. Target: under
  0.5 ms of the server tick; a spike measures it before anything is built on it.
- Rain becomes blocks of water only where chunks are awake; elsewhere it goes straight into the pond's total.
- Each has a conservation law for the soak test (`fundamentals.md` 4.8): water at the start = on the planet
  + in the air + in pipes and tanks + used up + shipped away; pollution made = in the air + in water and
  soil + removed + faded.
- *Proposal:* nothing is ruined by accident for good. A poisoned lake or a fallout zone recovers slowly by
  itself and fast with a cleanup factory. Mining a planet away is a choice, not an accident.

## Eating a planet
The developer's call (ADR 0009): late in the game it is physically possible to mine out and drain a whole
planet.

- **Water and rock are counted, not fixed.** Water leaves the cycle when you tank it, split it in an
  electrolyser, ship it away or feed it to the refinery. Rain is only what the air and the seas still hold: a
  drained planet is a dry planet, and its weather dies with it.
- **The size of the job:** Luna is 23 million m³ (190 million blocks), the 1 km planet 5 billion m³ (40
  billion blocks). A machine taking 1,000 blocks a second needs two days for Luna and 460 days for the
  planet. So planet eaters take **whole chunks**, and Luna is the first body you can finish.
- **Why you would:** the exotic refinery and the Ark ask for mass on a scale no deposit holds.
- Engine needs, each its own ADR and spike when the time comes:
  - a save stores "this volume is gone", not 40 billion edits;
  - the grid stops 64 m under the surface today (ADR 0003); digging to the centre needs deeper shells with
    fewer columns. Until then, nothing may assume the core can never be reached;
  - *proposal:* gravity follows the mass that is left.

## The exotic resource
- Appears only after the moon base milestone. Working name: **quantum liquid** (warp fuel); the name and
  look are the developer's.
- *Proposal:* it is **made, not found**: a raw material that exists only off the planet, turned into the
  liquid by a process that eats power and mass on a scale nothing before it did. So the last age is still a
  power problem, and the best power plant wins.
- It flows in pipes like any liquid, with its own hard rule (to design: for example it decays unless kept
  cold or powered), and feeds the exotic technology: the warp drive first.

## What the calls need from the engine
- **One network engine, three rule sets** (pipes, shafts, wires). Each gets its own ADR and benchmark.
- **Milestones are measurements.** A megaproject's rule is a quantity the simulation already tracks (power
  delivered, a room's temperature, an orbit's shape, tonnes arrived), a threshold and a duration, written as
  data. A bot can build a small one on a test planet and the test asserts the milestone fires.
- **Temperature is a field:** time of day, latitude (the poles sit in permanent twilight), height, depth
  (deep is warm, so a cave is the first shelter and later a heat source) and nearby fire, lava and machines.
- **Rooms** (`fundamentals.md` section 1) carry air and heat from Home to the Ark, and cold in the cold
  store.
- **The planet's air** above: one coarse grid for the whole planet, always running, deterministic, in the
  tick. The temperature field is part of it.
- **Bulk removal** ("Eating a planet"): coarse volumes in the save, a grid that can go below the crust.
- **The small planet makes space cheap:** a surface orbit is 102 m/s (204 m/s at 4 km). Throwing something
  into orbit with a steam catapult or an electric rail is physically possible before rockets. Do not design
  against it; players who find it have earned it.

## The opening (proposal)
Dawn at a face centre, near water. Thirst comes first, then food. Night arrives after ten minutes: dark and
cold. Two honest answers: build a fire and a shelter (costs wood and time), or **follow the day**: the sunset
moves at 5.6 m/s on the 1 km planet, between walking and sprinting, and sprinting costs food and water. Caves
are warm. Home, the first megaproject, is the moment the ape stops running from the night.

## Open (taste calls for later)
- **How an ape learns:** unlocks by doing the thing once in the world (rule 2), against a research tree.
- **The troop:** AI players as other apes of your troop (gather, build, settle), friends as more apes.
- How fast needs drain and what death costs.
- Which ores sit where (faces, depth, corners, moon) and how much: the finite budget.
- The exotic resource: its name, its raw material, its hard rule.
- How many tiers sit above IV and what the next planet changes.
- How hard pollution and radiation punish.
