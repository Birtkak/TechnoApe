# 0039 – Block patterns with parts

- **Status:** accepted
- **Date:** 2026-11-11

## Context
A block's look was a colour and one of three patterns (plain, grass, stone), drawn by `texel` in
`crates/client/src/shader.wgsl` (ADR 0013, 0033). The developer wanted more patterns, with coloured parts
and sliders, and designed them on a web page, `tools/review/workbench.html`. The page's recipe (`PATTERNS`,
`darker`, `lighter`, `part`, `texel`) was written from whole numbers and the shader's own `hash`, so the game
could take it over as it is. Constraints: plain, grass and stone must draw exactly as before (the six
reference views must not change), no more texture reads, a low cost per fragment, content stays data.

## Options considered
1. Port the page's recipe into the shader as it is, with the new fields in the `looks` buffer – one source of
   truth (the page), no textures, the cost is a few hashes on the blocks that use a pattern.
2. Bake each block's faces into a 16 x 16 texture array on the CPU – any pattern for one texture read, but it
   adds texture reads, a CPU port of the recipe, and the vision's per-block hash variation is lost unless
   several variants are baked.
3. Hand-paint textures – the texture atlas the vision foresees, but no content pipeline exists yet.

## Decision
Option 1. Eight new patterns: pebbles, speckle, frame, weave, planks, tiles, bricks, rings (ids 3-10 in
`Look.info`'s low byte). New optional fields per block, checked in `crates/engine/src/blocks.rs` (with
`deny_unknown_fields` kept):

- `colour2`, `colour3` (0..1 each): the second and third parts; default the page's `darker` (x 0.55) and
  `lighter` (x 1.25 + 0.2, at most 1) of `colour`, rounded to two places as the page writes them.
- `size` (1..8 texels, whole) and `amount` (0..1): default per pattern as the page's `PATTERNS[..].def`.
  Plain, grass and stone refuse them.
- `contrast` (0..2, default 1): how strong the three tones of grain are; 1 is exactly the old look (the
  shader picks the old `tone` itself when contrast is 1, so no float contraction can move a pixel).
- `side` on any block: a non-grass block takes the named block's whole look on its four sides and its foot
  (the page applies it to every face but the top); grass keeps its fringe rule.

The `looks` buffer grew from 16 to 64 bytes a block (`Look`: colour, info; colour2, size; colour3, amount;
contrast). On a side the pattern's x runs along the face and y down from its top edge, as the page draws
it, so bricks and planks stand the right way on both kinds of side face. The page's `Math.round` is
`floor(v + 0.5)` in the shader (WGSL rounds halves to even). The registry hash that client and server compare
covers the file's whole text, so it covers the new fields with no change.

Not ported to the CPU: nothing on the CPU needs the texels. The hotbar and "looking at" cubes in `hud.rs`
draw their own 8 x 8 noise in the block's `colour` with its `side` block's colour on the sides, which already
follows the new side rule; a patterned block shows there as its base colour. The bounce and reflection rays
(`seen`) take the block's base `colour`, as before.

Proof: a filmstrip of a temporary row of each pattern (not shipped) compared by eye with the recipe, e.g.
bricks at size 4 with mortar every fourth row from the top and a half-brick offset on alternate rows, tiles
at size 4 with grout every fourth texel, rings at size 2 alternating every two texels from the rim, and a
rings block whose sides are planks. `tools/views.sh` reports the six views unchanged.

## Consequences
- New looks are data: a block row picks a pattern and its sliders; which blocks ship stays the developer's
  choice (this change adds none).
- Cost per fragment: every side face of a non-grass block now follows its `side` (one more dependent read of
  the small `looks` storage buffer, no texture); plain, grass and stone add a select and a branch. A patterned
  block adds a switch and, at worst, three more `hash` calls (speckle and planks; four in all with the grain).
  No texture reads were added.
- The page and the shader must change together: a new pattern goes into `part` in both, with `game: true` in
  `PATTERNS`. The page remains the place to design them.
- Revisit when a texture atlas arrives (vision: 16 x 16 textures): these patterns could be baked into it.
