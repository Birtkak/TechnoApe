# ADR 0155: Pollution pushes back

Status: accepted (2026-11-07). Fun pass row H (`docs/plans/fun-pass.md`). Builds on ADR 0141 (pollution is real
matter). No material ids, no native change, no GEN_VERSION change.

## Context
ADR 0141 made waste real cells (sludge, fouled water, waste ground, smoke), but beyond an instant crop death on fouled
water and animals hurt standing in it, nothing pushed back. The fun pass asks for real but fair consequences, each
with a warning sign and a counter, kept simple and data-driven so seasons (ADR 0156) can hook in.

## Decision
All numbers are in `settings.gd` (`smoke_*`, `crop_*`, `fish_*`). GDScript only samples: a native `count_in_box`
for smoke, a few voxel reads per random tick for crops, the mob's existing three body probes for fish.

**Fish die in fouled water.** A swimmer (`moves: swim`) in fouled water is no longer hurt every 2 s; it builds
`Mob.foul_s` instead (clean water takes it back at the same rate). Sign: from `fish_gasp_s` (6 s) it gives up
wandering and rises to hang just under the surface, slow ("gasp"), and its look line says "Gasping: fouled water".
At `fish_foul_die_s` (24 s) it dies, rolls belly-up, drifts up to the surface and floats there `fish_float_s`
(30 s) before it goes (its drop comes then; a floating fish is not saved). Healthy fish pick no swim targets in
fouled water, so a plume pushes them aside before it kills them. Counter: stop the sludge (scrubber, settling pond).
Land animals keep the ADR 0141 hurt.

**Crops sicken, then wilt.** `Farming.pollution(p)` names what fouls a crop: fouled water watering it (the farmland's
cached scan), toxic sludge or waste ground in the ring of blocks round it (soil level and crop level), or heavy smoke
over it (smoke or toxic gas in `crop_smoke_cells` of the 4 voxels above its top). Each polluted random tick adds one
to the crop's saved `sick`, each clean tick takes one off. Sign: a sickly crop grows at `crop_sick_growth` (a
quarter) and its look line says "Sickly: <cause>". At `crop_wilt_ticks` (3) it wilts: its base voxels become dead
bushes, the rest air (it used to vanish at the first fouled tick). Counter: clear the waste ground (shovel or
scrubber), move the water source, vent the smoke; cleaned ground lets a sickly crop recover.

**Smoke makes the player cough.** Twice a second Survival counts smoke and toxic gas cells in a 7x7x7 voxel box
round the head (two native counts). At `smoke_dense_cells` (60 of 343) the player is breathing dense smoke: after
`smoke_cough_after_s` (2 s) a cough sound every 3 s, a toast the first time ("Get to fresh air, vent it, or wear a
mask"), status "Coughing" and the screen edge greys; after `smoke_hurt_after_s` (8 s) one health per 4 s, which
can kill (survival only). Clean air takes the exposure back twice as fast. Counter: fresh air, a chimney or vent
shaft (ADR 0141), or a worn piece with `"smoke_mask": true`: the new **cloth mask** (head slot, plant fiber + wool
+ charcoal, by hand) or the diving mask. A mask takes the head slot from a helmet: that is the trade.

## Consequences
- `/pollute` lays a fouled pond with fish and two wheat plots (one beside waste ground) on a slab ahead;
  `/pollute smoke` shuts the player in a smoke booth. `farm_speed=40` makes the crops wilt within a minute.
- Tests: `test/test_pollution_effects.gd` (sickly then wilted, recovery, smoke threshold, masks);
  `test_pollution` still passes (its fouled crop now wilts after three ticks).
- Seasons can hook in through `Farming.pollution` (a drought cause) and the same settings.
- Not done: a sickly crop's look is unchanged (no tint without new ids); fish killed by pollution still drop raw fish.
