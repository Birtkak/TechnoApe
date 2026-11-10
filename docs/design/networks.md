# The network engine: shafts, pipes, wires and belts on one graph

- **Status:** design proposal, 2026-11-10. `progression.md` asks for "one network engine, three rule
  sets" (pipes, shafts, wires); `fundamentals.md` 2.5 says networks are graphs, not blocks, and that belts
  are runs. This says what the shared engine is and how each rule set solves. Each rule set still gets
  its own ADR and benchmark when built. The feel of each (how punishing a burst boiler is) is taste, set
  when the content is.

## What is shared
- **A network is a graph** built from the blocks that form it: **nodes** (a machine's port, a junction,
  a tank, a gearbox, a transformer) and **edges** (a straight or bent run of pipe, shaft, wire or belt,
  with its length and kind). A hundred blocks of pipe are one edge.
- **Built as blocks change, inside the tick.** Placing a segment extends an edge or joins up to six
  networks; removing one may split a network, found by a flood limited to that network. Ids are given in
  order of creation, so every run of the game numbers them alike.
- **Chunks do not matter.** A network lives in the server's memory whole, wherever its blocks are. The
  planet is small enough that every factory on it runs all the time (fundamentals 2.5).
- **Asleep when steady.** A network whose inputs and outputs have not changed past a dead band is not
  visited. A change at any node wakes it.
- **One tick order:** networks by id; inside a network, nodes by id. Effects on the world (a fire, a
  burst, water taken from a river) are queued and applied after all networks, in id order. That keeps
  the tick deterministic, and lets networks be solved in parallel later, because none reads another.
- **One conservation hook** (fundamentals 4.8). Each rule set states what it conserves, and the soak
  checks it every tick: energy in = out + stored + lost; fluid units; item counts.
- **One save and one wire format:** per network its nodes, edges and state. A client receives only the
  gauges of nodes in regions it subscribes to (`world-data.md`).
- **Failures are world events:** a melted cable, a burst boiler and a snapped shaft become block edits
  and, where it fits, fire or steam, through the same queue.

## Shafts and cogs: one speed per network
- Everything joined rigidly turns together, so a shaft network has **one unknown: its speed.** Gear
  ratios give each branch a fixed multiple of it.
- Each tick: sum the torque of every source at its branch speed (an engine's curve falls as it speeds
  up), subtract every load's (a millstone's rises), divide by the network's inertia: that is the change
  in speed.
- Falls out for free: an overloaded line slows everything on it; a flywheel is just inertia; a loop of
  gears whose ratios disagree cannot turn and jams (or strips a wooden cog).
- Cost: a few operations per device. Thousands of machines are microseconds.
- Conserves: work in = work out + friction + change in spin energy.

## Pipes: the water's own model, in tubes
- Nodes hold a fluid's amount, temperature and pressure; edges carry a flow with momentum. This is the
  "virtual pipes" scheme `liquid.rs` already uses between gaps (ADR 0020), with the same integer units,
  so a pipe's open end and the world's water are one ledger with no conversion.
- Liquids barely compress; steam and gases do, by a table per fluid.
- Temperature rides with the fluid. A pipe loses heat to its air cell (`air.md`), more at night, less
  when lagged. Steam that cools turns to water in the pipe and must drain.
- Over a pipe's pressure rating it bursts. A boiler with no water overheats and bursts harder.
- Open ends: a pump's intake takes from a gap, a river reach or the sea; a vent gives steam to the air.
  Both go through the ledger.
- Conserves: units of each fluid; heat in = heat out + lost to air.

## Wires: voltage, current, loss
- What the player must be able to feel (progression): too little supply sags the voltage and machines
  slow; too much voltage burns a machine out; too much current heats a cable until it melts; a long
  low-voltage line loses most of its power, which is why transformers and tiers exist.
- That needs a current per edge and a voltage per node. Two ways to get them, **to be decided by a spike**:
  1. **Trees only, solved exactly.** A network without loops has one path from each source to each load:
     currents are sums along the tree, voltages drop along it, in one pass. Loops are not allowed, or a
     loop's last cable carries nothing.
  2. **Any shape, relaxed.** Each tick a few sweeps nudge every node's voltage toward what its
     neighbours imply, starting from last tick's answer. Loops and rings work. The answer settles over a
     few ticks, which reads as the grid reacting.
  *Proposal:* the second. The HV megaproject is a ring around the planet, and a ring is a loop.
- Transformers cut the graph into voltage islands; each island is its own small problem.
- Integer millivolts and milliamps. An island re-solves only when a source or load changed.
- Conserves: power generated = power used + cable loss + charge into storage.

## Belts: runs of items
- A belt run is an edge that holds its items as a list of gaps (Factorio's transport lines): a step
  moves the whole run by shrinking the first gap. A full, blocked belt costs nothing per tick.
- Splitters, sorters and inserters are nodes. The slow shaft-driven conveyor of the starter is the same
  edge with a speed taken from its shaft network.
- Clients draw items from the gap list; only nearby runs are sent.
- Conserves: items in = items out + items on the belt.

## Budgets (*proposals*, checked by named worst-case runs)
| What | Budget |
|---|---|
| All networks on a fully built planet (10,000 machines, 200 km of runs) | 2 ms of the server tick, p99 |
| Placing or removing one segment | under 50 µs; a split of a 10,000-node network under 2 ms, amortised over ticks if needed |
| Wire, per player near a factory | within the 20 kB/s of fundamentals 3 |

## Order of building
1. The shared graph with **shafts** first: the starter needs them, and they are the simplest solver.
   Milestone test: a bot builds a crank, a shaft and a millstone, and the conservation hook holds.
2. **Pipes**, reusing the liquid's link code; the boiler and steam engine join pipes to shafts.
3. **Belts** with the shaft machines.
4. **Wires** at LV, after their spike.
