# NNNN – The metal sea: the liquid outer core under the mantle's floor

- **Status:** proposed (step 2 of ADR 0152; the overseer's calls of 2026-11-18)
- **Date:** 2026-11-18

## Context
Since ADR 0154 a new world's planet has its layer 0 about 4 m over R/√2. Under it every cell read as
undiggable rock, nothing was meshed, and a player who dug out a floor block looked into nothing and stood on
invisible rock. Under the mantle lies the liquid outer core (`docs/design/deep-world.md`): a sea of white-hot
iron and nickel.

## Options considered
1. **Per-column fill levels**, as the stored water (`liquid.rs`), with a hidden grid of halving shells under
   layer 0 to hold them. Lets the metal flow and pool, but a connected liquid shell finds one level anyway: the
   storage, the grid and the flow would cost a great deal to show what one number says.
2. **One level for the whole body**, as the open sea's (`Body.sea`, the `SEA` account): generated, stored nowhere.
   A later step lowers that one value as metal is pumped out and counts it in a ledger.
3. **A block of molten metal** under layer 0. A grid under layer 0 again, and the metal would read as blocks,
   which ADR 0152 rules out ("liquid metal never freezes into blocks").

## Decision
Option 2.

- **One level a body.** `Body.core: Option<f64>`, a layer coordinate like `sea`: `with_mantle` sets it to 0, the
  bottom of layer 0, on a body that got mantle layers. Luna, a planet too small for a mantle, `mantle=0` and saves
  from before have none and behave as before. No save field, no wire field (it follows from `mantle`), no
  pumping, no flow, no grid under layer 0: those are core mining's and the hidden shells' steps.
- **Calm.** It does not rise into a shaft: a breached floor shows a still surface at its level. Pressure making it
  well up is a later step.
- **Things float on it.** Rock (3.3) and apes are far lighter than liquid iron (7): physics stays as it is, the
  cell under layer 0 still holds a player or rubble up, digging and placing under layer 0 stays refused. No
  swimming in metal.
- **It burns.** A survival player whose feet are in a layer-0 cell that is not solid, on a body with `core`, loses a
  point of health every `vitals::BURN` steps (6, 0.1 s), to death "<name> burned in the liquid metal" (told as
  every death is, by its string). The count lives on the server's client (`burn`), not saved. Creative is unhurt.
- **It is drawn as a glowing surface.** `metal_quads` (world/water.rs) gives a level-0 chunk at layer 0 one quad
  of kind 6 per column whose layer-0 cell is not solid (generated or edited: a re-mesh after a dig adds it), with
  the trailing water quads. `vs_sea` lays it as a flat river quad 1/8 of the cell over its bottom (from the chunk's
  base, so ADR 0154's GPU shift of bases and `r0` lands it right), and marks it (`side` bit 2); `fs_water` gives it
  `metal_sea`: opaque, unlit, emissive `METAL_GLOW` (linear 4.0, 2.6, 0.9 before the shoulder), with two octaves
  of value noise on the grid drifting slowly as darker convection cells (down to 0.3 of the glow: the shoulder rolls anything brighter to the same white). No tide, waves,
  transparency or refraction. No GPU layout change.
- **It lights the hole.** `World::light_of`: a layer-0 cell open to the metal is a light of `CORE_LIGHT`
  ([1.0, 0.66, 0.26], a lamp's strength: every open cell is one, and at [3.0, 2.0, 0.8] a 5 x 5 shaft burnt white) in `set_block` and `refresh_defs`, so a dig in this session, an
  edit sent to a client and a save loaded (`from_save` puts its edits with `set_block`) all light it. 8 lines.
- **`floor=k`** on a stand run stands on the bottom of layer k (the filmstrip needed the eye at the floor; `depth=`
  is metres under a ground whose height is not known ahead).

## Consequences
- A drained core, metal taken out, its ledger and the field (`deep-world.md` 3) change `Body.core` and need the
  hidden shells under layer 0; this step's code reads only that one value.
- Shortcuts: quads only in level-0 chunks (a hole is seen from near), one a column, unmerged (holes are small);
  a large opened floor would want them merged as the sea's are.
- Not done: pressure, heat from hot air or rock, sound, particles, swimming in metal, the 8 km planet's tuning.

## Checks
| Check | Result |
|---|---|
| Tests, red then green | `the_metal_sea_lies_at_the_mantles_floor_and_only_there` (red: `core` None on the mantle body); `the_metal_shows_only_down_a_hole_in_the_floor` (red: no quads where three layer-0 cells were opened; green: exactly those three, kind 6, level 8/64, none a chunk up nor without the mantle); `the_metal_sea_burns_a_player_who_steps_into_it` (red: lost 0 in a second, want 10; the whole floor and creative unhurt); `a_hole_to_the_metal_sea_is_a_light_and_stays_one_through_a_save` (red: no light). `cargo test --workspace` green |
| Seen | `out/metal.png`: `run=stand floor=4 pitch=-55` in a 7 x 7 room on the floor with a 3 x 3 hole through layer 0: the hole shows a flat bright yellow-white surface just under the floor's top, the room's walls lit warm yellow by the hole, the floor's top (facing away from the lights' centres) dark; between the first and last frame the surface's tint shifts slowly (a deeper yellow drifts in). No gap or flicker at the hole's rim. At `CORE_LIGHT` 3.0 a 5 x 5 shaft burnt white: it is a lamp's strength now. The shoulder rolls 4.0 and 2.6 both to near white, so the cells dip to 0.3 to show at all |
| Views | `tools/views.sh` against main's unchanged baselines: ok, all twelve, no cracks, largest diff 0.244 % (winter), orbit 0.201 % as ADR 0154 left it; the surface looks as before |
| Cost | none away from the floor: `metal_quads` returns at once unless a level-0 chunk at layer 0 with cells; one branch in `fs_water`. No speed run, no perf-debt row |
