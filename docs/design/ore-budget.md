# The ore budget: what Terra and Luna must hold, and where

- **Status:** design proposal, 2026-11-10. It answers `progression.md`'s open question "which ores sit
  where and how much". **Every number is a first estimate** made before any recipe exists; the method and
  the checks are the lasting part. The numbers move into `data/budget.toml` and are tuned there.
- Used by: `worldgen.md` stage 10 (ore), `worldgen-plan.md` W4.

## The method
1. **Demand per ape, per age:** ingots of each metal a single player needs to finish the age, megaproject
   included. Estimated below; later computed from the recipes by a test (`crafting.md`).
2. **Stock = demand x players x slack.** A world is for up to 4 apes, and people waste, choose and build
   for beauty. Early metals get 12 times one ape's need; late ones 6 times, so recycling and Luna matter
   by the end (ADR 0008: late game needs recycling).
3. **The generator places to the budget.** Stage 10 does not roll dice per rock; it puts down bodies in
   the right geology until each mineral's stock and each distance rule below is met. A seed that cannot
   meet them is rejected when the world is made. So every world can be finished, and none is flooded.
4. **A report per seed** lists, per mineral: total stock, number of bodies, nearest body to the spawn,
   depth range. A test asserts it against the budget file. The developer reads it for the default seed.

## Units (*proposal*)
- A block is 0.125 m³. **A block of metal is 8 ingots.**
- **Grade** is ingots per ore block smelted by hand: poor 1, normal 2, rich 3. Grade falls off toward a
  body's edge, so the middle is worth finding.
- **Processing multiplies it** (progression: "two to three times the metal from the same finite ore"):
  by hand x1, crushed first (starter) x1.5, washed and spun (MV ore line) x2.5, chemistry (EV) x3 and the
  by-products. The same body lasts three times as long for a factory as for an ape with a fire.

## Demand per ape (cumulative ingots; a guess to be replaced by the recipes)

| | Starter | LV | MV | HV | EV | planet stock (x slack) |
|---|---|---|---|---|---|---|
| copper | 400 | 2,900 | 11,000 | 41,000 | 81,000 | 1,000,000 |
| tin | 60 | 360 | 1,400 | 4,400 | 4,400 | 53,000 |
| iron (as iron, then steel) | 300 | 3,300 | 43,000 | 103,000 | 203,000 | 2,400,000 |
| lead | – | 500 | 1,500 | 4,000 | 8,000 | 96,000 |
| aluminium | – | – | 3,000 | 33,000 | 93,000 | 560,000 |
| gold, silver | – | – | 100 | 600 | 2,000 | 12,000 |
| uranium | – | – | – | – | 500 | 3,000 |
| coal (blocks) | – | 5,000 | 35,000 | 95,000 | 95,000 | 1,100,000 |

Where the big numbers come from: the ring railway is 26.7 km of track at about one steel ingot a metre;
the planet grid is the same distance in cable, three lines of it; HV's solar fields and EV's launcher are
aluminium and steel by the thousand.

## What that is in the ground (at 3 ingots a block: normal ore, crushed first)

| Mineral | Ore blocks | As bodies (*proposal*) |
|---|---|---|
| copper | 330,000 | 40 stream gravels of 500; 12 green-stained caps of 4,000; 6 deep bodies of 45,000 |
| tin | 18,000 | 12 veins of 1,500, each with a gravel below it |
| iron | 800,000 | 30 bog beds of 500; 4 banded layers of 190,000 (3 m thick, 90 m square) |
| coal | 1,100,000 | 10 seams, 1.5 m thick, 100 m square |
| aluminium (bauxite) | 190,000 | 4 surface blankets, 1 m thick, 75 m square |
| lead, zinc, silver | 35,000 | 20 pockets in limestone |
| gold | 4,000 | 8 quartz veins and their gravels |
| uranium | 1,000 | 3 bodies |
| diamond | 480 | 8 small bodies of 60 in the mantle's ringwoodite, 560 to 900 m down, only where the pressure is 36 to 56 MPa and the rock under 110 C: under the cold old shields and basins, never under rifts and arcs; mantle worlds only (draft-pressure-and-formation, a draft: no recipe or use yet) |

All of it together covers well under 1 % of Terra's 102 km² of land. Ore is rare by area and generous by
amount: finding a body is an event, and one body feeds a factory for hours. That is what makes the signs
(gravels, stains, plants) and later the survey satellite worth having.

## Where, from the spawn
The spawn is scored for the first three rows (`worldgen.md` stage 11). A face is 6.7 km across.

| Need | Distance | Depth | Why there |
|---|---|---|---|
| clay, sand, flint, wood, fresh water | under 100 m | surface | the first ten minutes |
| copper gravel | under 150 m | stream bed | copper in the first hour |
| bog iron | under 500 m | wetland surface | iron by the end of the starter |
| tin vein and gravel | 800 to 1,500 m | surface to 6 m | "a walk away, not at your door": the first journey |
| limestone, coal | under 1.5 km | surface to 20 m | LV: flux, steel, the first steam plant |
| a deep copper body, lead | 1 to 3 km | 10 to 60 m | LV and MV: needs a mine, pays for a cart track |
| banded iron | another plate, over 3 km | 20 to 80 m | MV: the reason for the railway |
| bauxite | hot wet lowland, usually another face | surface | MV and HV: follows climate, not rock |
| gold, uranium, graphite | far, in the folded belts and old shields | 40 to 150 m | HV and EV |

Depth follows the tools: hands and stone to a few metres, a shaft mine to 20 m, the MV drill to 60 m, HV
and later to the bottom of the crust (which is why `worldgen.md` proposes 192 m of it).

## Running out
- A stream gravel lasts one ape about an hour; a green cap, an evening; a deep body, a tier.
- The first body near the spawn is sized to run out in the steam age. Moving on is part of the game
  (pillar 3), and the railway megaproject is the answer to it.
- The ledger (`world-data.md`) holds what is left per body; the survey satellite (EV) shows it.

## Luna
- **Luna grows to about 1 km in radius** (ADR 0027; it was 177 m): 14 km² of surface and 40 billion
  blocks, room for maria, crater fields and a moon base's mining.
- Its stock follows the same method from IV's and the exotic age's demand: titanium
  (ilmenite), aluminium (anorthite), platinum metals, thorium and rare earths, helium-3 as a thin grade
  through the dust (so it is mined by area, not by vein), ice.
- The moon base's rule ("sends a set tonnage of ore to the planet") is set as a share of Luna's stock.

## Checks
- The budget file's schema test; the per-seed report against it (W4).
- A soak where a bot digs one small body out: the ledger for that mineral reaches the report's number
  minus the body, exactly.
- Once recipes exist: a test sums every machine's and megaproject's cost per age from the data and fails
  when it drifts more than a set share from this file's demand row.
