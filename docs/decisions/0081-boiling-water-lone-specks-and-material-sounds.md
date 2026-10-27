# ADR 0081: Lava boils water, lone still specks wake, every moving voxel murmurs

Status: accepted (2026-10-27)

## Context
Five bug reports on 2026-10-27 about the native simulation: 15-54-32 "water should update even a bit
quicker"; 15-52-39 "lava hitting water should give off a bunch of gas, water turns to gas when too hot,
the amount of gas from the amount of water, and a sizzle sound"; 15-43-26 "water artifacts in caves,
also they bunch up weird, maybe simulate all water in caves"; 15-55-24 "only 5 sand bombs and it lags";
15-53-31 "voxels in motion should always have a subtle sound depending on their material".

What the sim did: one reaction per material pair (`react_lut_` holds one index), so lava + water was
only `obsidian + steam` at 0.5: every puff of steam cost a lava cell, and a lava ball in a pool made as
much steam as it had lava, not as the pool had water. Still water (ADR 0068) only wakes at a fresh
opening, so the single still cells the generator leaves on cave walls and ledges (the vertical "glass"
faces in the 15-43-26 screenshot) never move. Sound loops (ADR 0044) sampled cells by material and
by "air above / below", so only sand and gravel over air sounded; falling dirt, snow or a loose gravel
slope were silent. Water ran three substeps a tick (ADR 0069 B).

## Decision
1. **Reaction chains.** `SimWorld::Reaction::next` links the reactions of one input pair in the order
   the data lists them; `try_react` rolls them in turn and the first hit fires. Lava + water (and the
   still kinds) is now `lava + steam` at 0.6 then `obsidian + steam` at 0.25: the water cell boils
   away and the lava stays hot about six times before it crusts, so the steam cloud grows with the water
   the lava touches. The pair's data lives in `GameMaterials.reactions()`.
2. **Lone still specks wake.** In `reservoir`, a still liquid cell with none of its kind on any of its
   six sides wakes at any air beside or below it, fresh or not. It then drains or dries like all moving
   water (film life, ADR 0069 B3). A body's rim, a river's thread and a pool's bottom cell all have a
   neighbour of their kind, so bodies keep sleeping (`test_sim_0081.gd`). The generator stays as it is
   (the gen session owns it); "simulate all cave water" is not needed for the artifacts and would wake
   every cave pool for nothing.
3. **Sample bits for sound.** `sample_cells` returns two more flag bits: `SAMPLE_MOVED` (8, the cell
   moved this or the last tick) and `SAMPLE_FRESH` (16, a gas or fire born within eight ticks). Loops in
   `data/sounds.json` take `"where": "moving"` and `"fresh"`, plus `pitch` and `db` per loop, so one
   asset serves sand, gravel, earth and snow at their own pitch, and a `sizzle` loop (steam at pitch 1.8)
   plays where steam is being born: at the lava. Loops key on motion, not on air: a loose slope, a dirt
   collapse or placed sand falling all sound.
4. **Water substeps 3 -> 4** (`liquid_speed`): a third faster spread for the price measured below. The
   film rule (ADR 0069 B3) no longer counts a two-eighth neighbour as deeper: a two-eighth cell walled in
   by one-eighth films can neither split nor even out, and with four substeps a spill regularly ends in
   such clumps that would never dry (`test_liquid_films.gd`).
5. **Sand bombs:** measured, not changed in the sim (see Consequences).

## Consequences
- Material ids: none added (steam exists, id 10).
- Steam born inside still water stays there until its lifetime ends (gas cannot displace still liquid);
  the air it leaves is a fresh opening that wakes the pool around it: a lava ball in a lake makes the
  lake churn and drop a little, which is the finite-water rule doing its job. Letting gas rise through
  still liquid is a possible follow-up if that looks wrong in play.
- The big vertical "glass" triangles of the 15-43-26 screenshot are still there after this change
  (`seed=714593343`, `/tp 566 896 -1428`): they are rows of still water cells along cave ledges, each
  with a neighbour of its kind, drawn as vertical faces by the fluid height field. That is the
  generator's cave water and the fluid mesher's look, not the sim: it goes to the gen and rendering
  owners. "Water bunches up weird" in caves is the same look to judge again then.
- Sand bombs: five 2 m balls (70k voxels) in the autopilot cost nothing measurable (p99 under 5 ms, sim
  tick under 1 ms); the lag in the report (frames alternating 15 and 18 ms at 2560x1440, bubble cost 0)
  is on the render side, not reproducible here.
- Benchmarks (960x540, vsync off, 30 s, back to back; siblings' Godot runs made some noisy): five sand
  balls before p99 3.8 / max 6.7 ms, after p99 4.4-7.7 / max 8.6 ms; a 1.5 m lava ball into a 3 m
  water ball before p99 7.7 / max 16.7 ms (two other Godots running), after p99 7.3 / max 9.3 ms and
  p99 6.9 / max 9.9 ms; sim tick 3.9-5.3 ms before, 4.8 ms after. Budget held.
