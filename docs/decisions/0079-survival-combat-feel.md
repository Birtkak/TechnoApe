# ADR 0079: A drawn bow, held eating, powder suffocation, soft water landings, merged TNT, ripe berries

Status: accepted (2026-10-27)

## Context
Six F9 bug reports from the 2026-10-27 playtest: 15-56-57 "bows charge up, aim, shoot on release",
15-36-07 "keep eating while right click is held", 15-55-49 "powders falling on you should suffocate
you", 15-18-51 "falling in water does not prevent damage", 15-48-13 "TNT should ignite other TNT
faster and their combined explosion should grow", 15-26-49 "blueberry bushes still have no drops"
(and, asked in chat, "put this in what am I looking at so players know").

## Decision
- **Bow** (`StoneAge`): a `shoots` spec with `draw_s` is drawn while right click is held and shot on
  release with Minecraft's power `(f^2 + 2f) / 3`, f = held / draw_s; speed and damage scale with it,
  under 0.1 shoots nothing, switching items lets it down. The bow: draw_s 1.0, cooldown 0.3 s. The
  atlatl stays a click.
- **Eating**: holding right click starts the next bite when one finishes (no new click needed).
- **Suffocation**: the eye voxel in a powder cell type (sand, gravel, dirt, snow ...) uses up breath
  in `Settings.suffocate_s` (2 s), then the drowning damage; read from the world in GDScript, no sim change.
- **Fall damage**: none when the landing is in liquid, including a shallow layer at the feet. The old
  code broke the fall but still dealt its damage on entering the water.
- **TNT**: chain fuse 0.1-0.3 s (was 0.5-1.5). A charge whose fuse runs out takes along every primed
  charge within its reach (radius + one block) with at most `Settings.tnt_merge_s` (0.3 s) left: one
  blast of the summed share at the share-weighted centre, so radius grows by the cube root (capped at
  8 blocks = 2x). Entity damage scales with radius / base radius.
- **Blueberry bushes**: brush digging (the per-voxel expected-value path) now honours `"seasons"`
  (it ignored them); bushes yield 6-10 per block in summer and autumn, 1-3 out of season (the bush
  always shows berries). The look-at panel shows "<Item>: ripe now" / "ripe in summer and autumn" for
  any material whose first drop entry has seasons. Plain green bushes are `leaves` (sticks only).

## Consequences
- Partial charges now hit entities less (damage follows the cube-root radius).
- A merged blast's chain search covers a bigger box: the `/tnt grid 20 1.5` run went from 23 blasts
  to 6 with 17 merged; p99 29 ms vs 27 ms before, both over budget as that stress run already was.
- No new materials, ids or tiles; no sim, generator or HUD changes.
