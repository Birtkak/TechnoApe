# ADR 0113: Machines, energy and fluid networks

Status: accepted (2026-11-02), Phase 3/4 groundwork (backlog job 7). Builds on ADR 0007 (block entities), ADR 0050
(pins), ADR 0097-0100 (the steam age: boilers, pipes, the turbine, transport).

## Context
GregTech depth means machines in the hundreds and cables and pipes in the thousands. The steam age already has the
shape of it: machines are block entities placed as work pieces (`work` key on the item, a `kind` in
`BlockEntities.create`), ticked at 20 Hz, saved as JSON; `BoilerEntity.feed` walks pipes once a second in GDScript
(fine for 8 pipes, not for a factory). The turbine spins and powers nothing. What was missing: a network layer that
scales, a rule for what a machine may ask of the material simulation, and a place for multiblocks.

## Decision

### Machines are block entities, placed as work pieces
- A machine is a `RefCounted` entity in `scripts/work/` with the work-piece interface (`kind`, `pos`, `tick`,
  `is_lit`, `use(held)`, `contents`, `look_lines`, `view_parts`, `to_dict` / `load_from`, `summary`), registered in
  `BlockEntities.create`, `WORK_KINDS`, `HEIGHT` / `SCALE` and `WorkSystem`'s solid list; its item carries
  `"work": "<kind>"`. No material ids: a machine's voxels are its footprint, its look a MicroMesher model.
- Machines tick at Minecraft's 20 Hz in GDScript as today (a machine's own logic is a few branches, not voxel work).
  The budget: `phys_cost.blocks` in the JSON stays under 1 ms with a few hundred ticking entities; past that,
  machines sleep when idle (`_ticking` only holds what moves) and the solver below carries the rest.

### Networks: one native solver, one channel at a time
- A channel (`energy`, later `steam`, `water`, `items`) is solved once a second (every 20 block ticks, after
  `BoilerEntity.feed`) by `Networks.solve(store, channel)`: every entity with a `net(channel)` method joins as a
  box (its footprint) with a role and an amount: a **conductor** (cable, pipe) with a capacity per tick, a
  **source** with what it offers, a **sink** with what it wants. The native `NetworkSolver` (technosim) hashes the
  block cells of every box, unions boxes that touch face to face, and moves per component
  `flow = min(supply, demand, weakest conductor)`: sources give in proportion to their offer, sinks get in
  proportion to their want. Each entity then hears `net_in(channel, amount)`.
- Units: energy in EU/t (GregTech's number, per block tick); a copper cable carries `Settings.cable_eu_t` (32),
  a turbine at working pressure offers `Settings.turbine_eu_t` (32), the electric furnace wants
  `Settings.electric_furnace_eu_t` (16) and runs a second of fire when it gets it. Voltage tiers, cable loss,
  batteries and machine explosions are open (GregTech's whole game); the solver's shape takes them as per-component
  rules later.
- Fluids go through the same solver with the fluid as the channel (a tank: source when it holds some, sink while
  it has room; a pipe: a conductor). The steam age's `feed` stays as it is until a fluid channel replaces it.
- Cost: `tools/network_bench.gd` solves 10 000 cables and 500 machines in one component and in 500 small ones;
  the budget for a solve is 2 ms (once a second, main thread). The GDScript gather is one pass over the entities
  that answer `net`; nothing per voxel.

### How a machine keeps a sim bubble alive
Machines that need the material simulation (a pump drinking a lake, a quarry carving, a sprinkler) say so with
`needs_sim()`. ADR 0050's pins are the mechanism: a machine pin is a `SimBubble` pin around the machine with
`keep_alive` set, which skips the quiet / age release while the machine asks for it, at most
`Settings.machine_pins_max` (2) at once, nearest to the player first; the rest run only when the main window
covers them. Everything outside a window is frozen and stored (vision: the bubble is the knob); machines do not
catch up on elapsed time when they come back (Minecraft's rule, open). Implemented with the first machine that needs
it (the pump); nothing today does.

### Multiblocks
A controller entity names a pattern in its data (`multiblock`: layers of strings, a key of materials or entity
kinds, like a shaped recipe). It forms on a right click with a wrench and re-checks when a block in its box
changes (`Main.set_world_voxel` notifies entities whose footprint covers the voxel); formed, it covers its parts'
cells as its footprint so the network sees the whole hull. Pattern checks read the bubble-first voxels
(`get_world_voxel`) over at most a few hundred cells per check: GDScript, since it is per block and on demand,
not per voxel per tick. First multiblock: the blast furnace hull when the electric age needs it (open).

### Save and sync
Entity state saves as today (`BlockEntities.to_dict`, every 30 s while dirty and on exit): energy buffers,
progress, inventories. Network state is never saved: it is recomputed on the first solve after load. Server
authority (ADR 0002, job 5): the server ticks machines and solves networks; clients get entity state deltas
(the `changed` signal already marks what to send) and never solve.

## Proven end to end
Boiler -> bronze pipe -> steam turbine -> copper cable -> electric furnace: the turbine offers energy while the
boiler has pressure, the cable carries it, the furnace smelts without fuel while it gets its 16 EU/t and stops when
the boiler goes cold (`test_networks.gd`, `/kit electric`).

## The electrical age, tier 1 (added 2026-11-02 on the developer's ask)
GregTech's shape, slow and dear on purpose so the first machines feel earned: rubber (birch tar kneaded with
sulfur, cured 20 s in a furnace) wraps everything; insulated wire, coated boards (plank in rubber), vacuum tubes
(glass, wire, rod) and resistors (coal between wires) make the **primitive circuit** (two tubes, a resistor, two
wires on a board, by hand at the table); eight iron plates round a cable make a **machine hull**; wire, rod, plate
and screws make the **electric motor**, a motor and plates the **electric piston**. Every tier-1 machine is two
circuits, a hull, motors or pistons and plates: the **lathe** (ingot to two rods, rod to four screws; one screw
can be sawn from a rod by hand, slowly, so the first motor is possible before the first lathe), the **bending press**
(ingot to plate) and the **wiremill** (copper ingot to eight wires, the anvil gives four). Machines are
`MachineEntity` (one class, `"machine": {station, eu_t}` on the item, recipes of type `smelt` with the machine as
station, so the recipe browser lists them under the machine). The book has an Electrical Age chapter ending in
"The First Machine".

## Open for the developer (taste)
- Which machines come first (the electric furnace is the proof; GregTech starts with a macerator, a compressor,
  an extractor and a generator), and whether the first generator is the steam turbine (default) or a burning one.
- Voltage tiers and cable loss, batteries, machine explosions on overload.
- Catch-up on return (none by default), how far a factory runs from the player (pins), item pipes vs belts.

## Consequences
- No new material ids; the cable and the furnace are work pieces.
- `BoilerEntity.feed` and the new solver both run once a second; the feed moves onto the solver's steam channel
  when pipes grow past a dozen.
