# ADR 0123: Real electrical units: volts, amps, ohms and watts on the LV network

Status: accepted (2026-11-03). Supersedes the EU parts of ADR 0113 (the energy channel as a flow of EU/t) and
ADR 0117 (EU/t machine draws, the battery's flip, sinks filled in placement order). ADR 0122's steam and water
channels keep the flow model: they are hydraulics, not circuits.

## Context
The developer: "don't use EU power units. Let's have realistic ohm, wattage and voltage and circuitry." EU/t was a
fiction (a cable "carries 32", a battery "flips"); with real units the physics decides what runs, and cables,
fuses, switches and breakers become real things to place.

## Decision

### The tier: 48 V DC
One voltage tier, **48 V DC**, and nothing past it. Why 48 V: it is extra-low voltage (under the 60 V DC touch
limit), so bare terminals in a workshop are a story about sparks, not death; it is what telecom plants and
workshops really run on; and DC keeps the model to Ohm's law, which a player can reason about. AC and higher
tiers are a later ADR. Default, developer to confirm.

### The circuit model
- **Sources** are an EMF behind an internal resistance. The turbine's dynamo: `dynamo_volts` (52 V) at working
  speed, less as it slows (EMF scales with rpm), behind `dynamo_ohms` (0.3 ohm): about 2.2 kW at most, so five
  400 W machines before the voltage sags out. The battery box: lead-acid cells, EMF 46 V empty to 50 V full
  behind 0.1 ohm, 2000 Wh. **No flip logic**: with the dynamo above its EMF the current runs in and it charges;
  with the dynamo gone it supplies. Drained it leaves the circuit; full it takes nothing.
- **Conductors** have a resistance from their material and cross-section per block: a copper cable (6 mm^2,
  0.5 m) is 1.4 milliohm and rated 40 A; a switch, fuse or breaker 1 milliohm in line. Past its amps a cable heats,
  and after `cable_burn_s` (10 s) of that it burns out and conducts nothing until a cable in hand replaces it.
- **Loads** are their rated watts at 48 V, modelled as a conductance (P / V^2): over-voltage draws more, under-
  voltage less, exactly as a heater or motor would. Machine watts are data on the item (`"machine": {"watts"}`,
  `"load": {"watts"}` for the lamp and pump): lathe, press, wiremill, macerator, assembler 200 W; alloy smelter and
  electric furnace 400 W; pump 200 W; lamp 40 W. (`eu_t * 25`, so the old balance carries over.)
- **The solve** (`NetworkSolver.solve_dc`): boxes that touch are wired; runs of plain conductors collapse into one
  resistor between their ends (a 10 000-cable main becomes one edge); the reduced network is solved by nodal
  analysis with a Jacobi-preconditioned conjugate gradient (the conductance matrix is symmetric positive
  definite once a source or load ties a component down); every node gets its voltage, the current through it and
  its power; collapsed cables get their voltage interpolated along the run and the run's current.

### How a machine behaves (replaces "sinks fill in placement order")
A load is not switched on by the solver; it decides from the voltage at its own node (`PowerLoad`): it joins only
when that voltage holds `lv_start_frac` (0.95) of 48 V, **one load per circuit per second** (`Networks` admits the
first that asks), it stops under `lv_stop_frac` (0.85, 40.8 V) and then waits `lv_retry_s` (5 s) before trying
again. So a weak dynamo settles on the machines it can carry, one at a time, instead of all of them flapping on and
off together; the first ones that asked keep running. A waiting load sits on the circuit as a 10 mW sliver so it can
read its voltage, and a machine with no work is a plain conductor: its terminals pass the line through to whatever is
cabled beyond it (the first factory run died beyond an idle macerator before this rule). A drained battery is the
same sliver until the line rises above its EMF, then it charges. When a circuit sags, **the newest load sheds
first** (at 85 %); older loads hold on down to `lv_brownout_frac` (70 %), and a shed load's retry wait doubles on
every stop up to a minute (resetting after a steady minute), so one machine too many drops that one machine, not the
whole floor, and a source that cannot carry it stops being asked every five seconds. Default, developer to confirm.

### Circuitry
- **Knife switch** (60 A): a right click opens or closes the circuit beyond it.
- **Fuse box** (30 A): the fuse wire melts above its amps; fuse wire in hand refits it.
- **Circuit breaker** (30 A): trips above its amps; a right click resets it.
- Open, each is no conductor: the circuit beyond it is dead (the solver never sees it).
- **Components**: the primitive circuit now takes a paper capacitor (tin foil and rubber) besides its vacuum tubes
  and resistor; fuse wire is tin. Resistors, capacitors, tubes and coils are the LV-era parts list.
- **Multimeter** (was the EU meter): a right click on a piece says its circuit: volts here and across the circuit,
  the highest current, watts drawn of watts made; the look-at panel shows the same line.

### Data conversion
`game/tools/eu_to_watts.py` converts items, recipes and quest text from EU to watts and is re-runnable (a file
already converted is left alone), because the content pass adds LV data in the old fields in parallel: whichever
lane lands second runs it at merge.

## Measured
`tools/network_bench.gd` on a machine shared with three other Godot runs (the flow solves that took 1.1 ms alone took
2.8 ms here, so halve these for a quiet box): `dc_10k`, a dynamo, 10 000 cables and 500 machines, collapses to 501
nodes and solves in 366 CG iterations, 5.3 ms; `dc_islands_500`, 500 separate circuits of 20 cables, 1000 nodes,
2 iterations, 4.2 ms. Once a second, so a factory of that size costs a few ms a second on the main thread; the
in-game factory solve is 0.7 ms. `/stations factory` on real units: furnace 381 W lit, cables 17 A, battery charging
at 8 A, lamp lit, assembler made 4 circuits (`game/out/dc1/strip.png`).

### Round 2: more sources, gauges, a photocell and meters (2026-11-03)
- **Water wheel** (50 V / 1 ohm at full drive): WorkSystem counts the water in the two-block box under it once
  a second (a native `count_in_box`); half wet is full drive. About 100-300 W: lamps and a pump, not a furnace; set
  several in parallel. **Wind turbine** (54 V / 2 ohm, ~360 W peak): the wind is a slow sum of two sines of the
  clock plus a rain bonus, so lulls drop it out and rain brings it back. **Hand crank** (50 V / 10 ohm, ~60 W,
  what a person keeps up): a right click turns it `crank_s` (20 s). A still source is a plain conductor. All one
  `PowerSourceEntity` from the item's `"dynamo": {volts, ohms}`.
- **Lead-acid cell**: tin plates in sulfuric acid behind glass (the game's tin stands in for lead); four make the
  battery box.
- **Cable gauges**: thin (1.5 mm^2, 5.7 milliohm a block, 15 A, cheap) for lamps and switches, copper (6 mm^2, 40 A)
  for machines, thick (16 mm^2, 0.54 milliohm, 80 A) for the trunk. A cable keeps its gauge when laid.
- **Daylight switch**: a selenium photocell (sulfur stands in) that WorkSystem opens by day (time_of_day
  0.25-0.75) and closes at night: street lamps. **Meter panel**: a conductor whose face draws a volt bar and an
  amp bar from the last solve.
- `/stations power` lays them all working: the wheel over a trough of still water (one native fill), the turbine,
  the crank pre-turned, a meter, a fuse, a battery, the daylight switch and two lamps over thin cable.

## Open (defaults, developer to confirm)
- The sources' numbers above; whether a wheel should need flowing (river) water rather than any water under it;
  wind as a clock function rather than a world wind; the crank's 20 s per click.
- 48 V DC as the tier; the dynamo's 52 V / 0.3 ohm (~2.2 kW); machine watts; the battery's 2000 Wh.
- Start/stop thresholds (95 % / 85 %, 5 s retry) and one start a second per circuit.
- Cables never melt the player; nothing shocks. Overloaded cables could set fire to wood beside them (the sim's
  ignite API exists) when the developer wants danger.
- Cable loss is real (1.4 milliohm a block): a 100-block run at 40 A drops 5.6 V. Whether that is fun or tedious.

## Consequences
- `test_dc_circuit.gd`: the solver (a divider, a collapsed run, a junction and a floating branch), a furnace at ~48 V,
  eight lathes on one dynamo settling without flapping, a burning cable, a blowing fuse and a tripping breaker, a
  switch, a battery charging then carrying a lamp, saves. The EU-era tests were rewritten to the new units.
- `Networks.stats.flow` is now delivered watts; `networks["energy"]` holds `volts_min/max`, `amps_max`, `supply`,
  `demand` per circuit.
