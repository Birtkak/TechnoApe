# 0115 – The bare hand: the ape's own arm in hand, and one swing for everything held

- **Status:** proposed (built; waits for the developer's look and merge)
- **Date:** 2026-11-14

## Context
The developer: "make it so when you're holding nothing it shows your hand like in Minecraft". With an empty slot
nothing was drawn at the lower right. An item with a model was drawn there by `Models::frame` (ADR 0047, 0114),
a block without one as a HUD cube (`hud::cube`, play.rs) that dipped on a dig or place (`App::dip`, a 0.18 s
pulse, ADR 0031); a model in hand never moved. The player's body is `data/models/ape.toml` (`FIGURE`), its right
arm the part `arm_r` (1 x 5 x 2 voxels, the light hand at its bottom layer).

## Options considered
1. **Where the arm comes from.** (a) A new arm model file: a second thing to paint and keep like the body.
   (b) **Chosen:** the ape's own `arm_r` part (and any part hung from it), drawn as one more piece of the ape
   model on the existing instance path, in its own colours and lit as every model is. No new pipeline or shader.
2. **The pose.** (a) A `hand` clip in `ape.toml`: the clip system turns parts about the model, but the arm in
   hand is placed in the eye's frame, not the body's, so a clip would still need code to put it there. (b)
   **Chosen, smaller:** named constants in `models.rs` (`ARM_HAND`, `ARM_WAY`: the hand's end and the way to the
   shoulder in the eye's metres; `SWING_REACH`, `SWING_AIM`, `SWING_CHOP`), with their comment.
3. **The swing.** Nothing held as a model swung; the HUD cube's dip is a swing. **Chosen:** one swing for all,
   driven by the same events: `App::swung` starts the dip (now `SWING_S` = 0.25 s, as long as a survival dig's
   hit) on a dig, a place, a knapping strike, a pick-up and every hit of a held survival dig (so it repeats).
   The cube dips by its pixels as before; a model in hand and the bare arm are thrust 45 % of the way toward a
   point on the look and chopped down 0.35 rad about the hand, by `models::swing` (eased: a quick strike, a
   slower return). The first try, turned down about the shoulder (0.9 rad, then 0.35), took the arm out of the
   view for a third of the swing: the filmstrip showed it, so it now strikes in toward the aim (the test holds
   the hand inside the view all the way). Client side only, no wire change.

## Decision
- `Models::frame` takes `hand: Option<Hand>` (`Hand { item: Option<&str>, swing }`): None shows nothing; an item
  is drawn as before (turned by the swing); no item draws the figure's arm part(s) only, at rest pose, by a
  frame from the arm's voxels to the eye's.
- `frame.rs::hand_shown`: in play, no hammer or bucket up, no F3, not the title's world, and now **not on the
  death screen** (for a held item too). The pause menu and the inventory still show the hand, as the item did.
- With an item or block in hand the arm is not drawn behind it; **holding the item in the visible hand is a
  later step.**

## Evidence
- Tests, each seen failing first: `models::tests::an_empty_hand_draws_the_figures_arm_only` (red: nothing drawn;
  then red again on its added length check, the arm 5 m long: the rest pose's voxel scale was taken twice, which
  the first still showed as a huge beige wedge), `no_hand_draws_no_arm`, `the_swing_strikes_toward_the_aim_and_back`
  (red on the turned-down swing), `a_swing_peaks_once_and_ends_at_rest`, `frame::tests::the_hand_shows_in_play_only`
  (red with the death screen's rule taken out). The in-hand tests of ADR 0114 stay green. `cargo test -p client`:
  162 passed, 1 ignored.
- Stills, off-screen `run=play planet=768 seed=1 save=0` (`out/hand/`): `arm.png` (survival, empty slot 9, noon,
  the frame beside its lower right doubled: the hand's light end and the copper forearm from the lower right
  corner, angled in), `arm_dusk.png` (the same at sunset: the arm darkened with the light), `swing.png` (8
  frames 40 ms apart of a creative dig: rest, thrust in with the forearm's side shown, back), `item.png` (16
  frames 80 ms apart: the hand axe in hand, a held survival dig of grass, the axe moving on each hit; no arm
  behind it).
- Cost: one model piece more (the arm's quads) when the hand is empty, none when an item is held; not measured.

## How it reads next to Minecraft
- Size: about the same share of the screen as Minecraft's arm (the lower right sixth), its end at the hotbar's
  right edge. Angle: in toward the middle as Minecraft's, a little flatter.
- The ape's voxels are flat colours, so the arm is two plain slabs (light hand, copper forearm) with no skin
  pattern: coarser than Minecraft's 4 x 12 pixel arm. Painting the arm's voxels, or a painted box for the
  forearm (ADR 0081), is the cheap fix if it reads too bare.

## For the developer to confirm
- The arm's place, angle and size: its hand's end 0.55 m ahead, 0.30 right, 0.30 down; the ape's own voxels
  (12.5 cm), so the arm is 0.625 m long, 12.5 x 25 cm thick.
- The swing: 0.25 s, toward the aim and chopped (not the brief's forward-down arc, which left the view); the
  cube's dip now 0.25 s too (was 0.18).
- Hidden on the death screen, held item included.

## Shortcuts
- The arm is drawn at the ape's rest pose; a clip of the ape never moves it in hand.
- No bob with the steps for the arm or a model in hand (the cube keeps its bob).
- The zoom (C) moves the arm as it moves a held model today: it is placed in metres, so it grows on screen.

## Review (the overseer read the diff; it wrote the brief, not the code)
- *Nothing to change.* The arm's piece is pushed after the model's own instances and its draw's count follows, so
  a herd of the same figure and the bare arm share a draw rightly; a part with no instance and no arm piece makes no
  draw (`count > 0`); the hand's rules are one tested function.
- *Answered, not changed:* the arm's one piece is not counted in ADR 0114's per-model reserve against `MAX_PIECES`
  (one piece at a cap of 65,536); the rest pose is built twice a frame while the hand is empty (two small
  allocations: for the optimisation batch if it shows); in the first quarter second of a run the swing's age is
  inside its length, so the hand may strike once at spawn; the death screen now hides a held item too.
