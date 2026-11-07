# ADR 0148: Underground hazards the machines solve: firedamp, the vent fan, flooding springs

Status: accepted (2026-11-07)

## Context
The fun pass (2026-11-07) wants the simulation to make trouble that the GregTech side answers, with real
damage and a warning sign and a counter for every hazard. Mines had none: coal came out clean, an opened
aquifer (ADR 0110) spilled its one chamber of still water and was done. Real coal mines had firedamp
(methane) and water, and answered with the Davy lamp, ventilation and pumps.

## Decision
1. **Firedamp is a material (id 133), generated as pockets in coal.** A gas lighter than air that creeps along
   ceilings and never fades (`lifetime` 0). The native vein generator (`FineGen::vein_voxel`) turns a whole
   ore lump into the vein's `pocket_gas` with `pocket_gas_chance` (`data/ore_veins.json`, coal: firedamp, 0.1;
   `ore_veins.gen.h` carries it). Sealed in rock the gas sleeps like any blocked cell; a pick opening the lump
   wakes it and it pours out into the gallery.
2. **The flash is the sim's own fire, the blast is TNT's.** Firedamp is `burnable` (fire_hp 4, every attempt
   lights, autoignition 20) so any fire cell, lava or burning thing runs through it in a few ticks. It is also
   `explosive`: the kernel tallies the burning cells of such a material per tick (`SimWorld::TALLY_GAS_BURN`,
   count and mean position, like the waterfall tally of ADR 0103) and the moving ones (`TALLY_GAS_MOVE`, the
   hiss loop in sounds.json). `Firedamp` (scripts/firedamp.gd) reads the burn tally every frame: a tick with
   `Settings.firedamp_blast_cells` (24) or more burning is a blast at their centre: the gas and fire within
   `firedamp_vacuum_m` go first (one pocket, one blast), then `Tnt.explode` with a share of `count /
   firedamp_full_cells` (a lump of ~150 cells is a whole block's blast): the crater, the rigid piece check,
   damage, drops and the sound TNT already has (ADR 0065). Rule 8 holds: GDScript reads one tally, never cells.
3. **Torches light it.** Torches are not fire cells, so four times a second `Firedamp` samples the cells around
   every light `TorchLights.sources` returns (placed torches, lit fires, dropped and held lights) with a
   firedamp mask and puts one fire cell where a sample hits. Lights whose definition carries `safe_light`
   (the electric lamp, the safety lamp) are skipped.
4. **The safety lamp is the warning sign.** An item with a `light` (so TorchLights carries it) marked
   `safe_light`. Held, `Firedamp` samples the gas within `safety_lamp_reach_m` of the eye and the lamp's light
   turns blue and brighter with the hits (`TorchLights.tint_held`), with a toast the first time. It never
   becomes a fire cell, so it never lights the gas. Recipe: glass over a torch over an iron plate.
5. **The vent fan is the counter.** A 0.5 m LV machine (`VentFanEntity`, 100 W like the lamp's pattern, a
   wrench turns it) that, while powered, pushes every gas cell in the duct in front of it (its own cross-section,
   `vent_fan_reach_m` = 4 m) one cell along its facing per frame: `SimWorld.blow` (native, far end first so a
   column moves as one), through the main window or a machine pin (`MachinePins.blow`; `needs_sim` while on).
   Gas pipes stay out of scope: a fan at a gallery mouth blows the gas out, a chain of fans carries it further.
6. **Spring stone pours.** A new material attribute: a solid with `emits` places that material into a random
   air neighbour with `emit_chance` per tick (`SimWorld::update_emitter`, the material is no longer inert).
   Spring stone (ADR 0110) emits water at 0.25. An aquifer stays a still chamber until breached; then its
   core keeps pouring, and stops by itself once water surrounds it again: the flood rises to the spring's
   level and no higher, so the sim budget is bounded by the mine's volume below it. The pump (ADR 0117)
   drains it; a dam of blocks holds it.
7. `GEN_VERSION` 39.

## Consequences
- Coal mining with a torch is a gamble now; the safety lamp (glass, a torch, an iron plate) is cheap on purpose.
- Digging into an aquifer below your workshop floods it for good unless you pump or wall it off.
- Tunables in `Settings` (`firedamp_*`, `safety_lamp_reach_m`, `vent_fan_reach_m`); `/scenario firedamp` builds
  a gas room ahead of the player with the lamp, torches, a fan and the electric kit.
- Not done: gas pipes, lit furnaces as igniters (they are not fire cells either; the torch rule covers the lit
  block entities TorchLights lists), a placed safety lamp, firedamp in the F3 screen.
