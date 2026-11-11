# 0045 – Block textures from PNG files

- **Status:** accepted
- **Date:** 2026-11-11

## Context
The Studio (ADR 0044, `docs/design/studio.md` "Textures") lets the developer paint block faces. A painting
must replace a block's shader pattern (ADR 0039) as a recording replaces a code-made sound (ADR 0032), with
the exact file rule of the design doc, and no block has to be painted for the rest to run. ADR 0039 held back
from texture reads for their cost, so step 1 measured them first. Constraints: the frame budget (ADR 0004),
the perf baseline (`tests/perf/baseline.json`, ADR 0025 K), the six fixed views unchanged while no PNG exists,
no decoding on the frame thread.

## Options considered
1. A 16 x 16 texture array (Rgba8Unorm, 2048 layers, `textureLoad`, no sampler, no mips) – the design's plan;
   measured: GPU and frame thread flat, but `fly`'s VRAM rose from 561 to 817 MB in every run, also with no
   PNG present: the first sampled image took a new 256 MB memory block, past the baseline's 625 MB.
2. The same layers in a storage buffer (one `u32` a texel, 2 MB), read once per fragment – the same one read,
   no filter, no mips; buffers share the blocks already held.
3. Bake the patterns into the textures as well – one path for every block, but every block would change
   its pixels and a CPU port of the recipe would be needed (ADR 0039's option 2); not asked for.

## Decision
Option 2. Measured on the dev PC, release build, three `quick=1` runs each, median (frame thread's work ms /
GPU ms p50 / VRAM MB):

| run   | before              | built, no PNG       | every block painted (4 variants, all faces) |
|-------|---------------------|---------------------|-----------------------------------------------|
| stand | 0.562 / 1.656 / 815 | 0.347 / 1.704 / 815 | 0.398 / 1.634 / 815                          |
| fly   | 0.377 / 1.511 / 561 | 0.378 / 1.491 / 561 | 0.399 / 1.485 / 561                          |

`tools/perf.py check` passes all four against the baseline (the texture array of option 1 failed `fly` on
VRAM). One painted `fly` run of three read 817 MB: that run sits on the allocator's block edge; the median holds.

How it works:
- `crates/engine/src/textures.rs` (no GPU, so the Studio can call it): `decode` checks one file against the
  rule (8-bit RGBA, 16 high, 16/32/48/64 wide, every pixel alpha 255) and returns its variants; `load` reads
  a folder: `<block>.png` is every face, `_top`, `_side` (the four sides), `_bottom` override one; a file
  that breaks the rule, or names no block or face, is refused with its name and why, and its face falls back
  to `<block>.png` or the pattern. Decoded with the `png` crate (already in the lock file through `image`;
  now a direct dependency of the engine).
- The client's `textures` thread polls the folder's names, sizes and times once a second, loads on any
  change and sends the set over a channel; the frame thread only copies at most 2 MB of layers and the looks
  buffer when one arrives (`Gpu::take_textures`). Refusals go to stderr. `textures=dir` reads another folder
  (for tests, so temporary files never go into `data/`).
- `Look` carries per face a layer, the variant count and a painted bit (`tex`, `tex2`), and the block's one
  colour (`mean`). In `texel` a painted face of the block itself wins; else the `side`/grass redirect runs as
  before and that block's painting for the face is used. The variant is picked by the cell's hash, the same
  for every face of a block. On a side x runs along it and y down from its top edge, as the patterns do.
  A painting is drawn as painted: no grain tone on top.
- The bytes are the same numbers as `blocks.toml`'s colours (x 255), no sRGB decode, so "start from pattern"
  in the Studio round-trips.
- The one colour (the mean of the top's painting, else the side's) feeds `seen`: the bounce light and
  reflections, which is what draws the terrain far along a ray. The hotbar and "looking at" cubes take the
  top's and the side's means with plain noise. `blocks.toml` is not rewritten.
- Distance: the read is one texel per 1/16 of a cell with no filter, the same sampling as the hashed
  patterns, so a painting shimmers no more than a pattern of the same contrast; coarse far chunks show it
  stretched over their larger cells as they show the patterns. No mips.

Proof: engine tests (each kind of bad file refused, good files and their variant counts, the face fallback
and refusal by name, the mean colour), each seen failing first; `tools/views.sh` unchanged with no PNG;
a filmstrip of a temporary grass top and copper painting (`out/tex-film/sheet.png`, not shipped); a play run
whose folder gained, changed and lost files saw each within a second.

## Consequences
- Any paint program can paint blocks now; the Studio's texture page (step 3) writes the same files and can
  call the same check.
- 2048 layers in all (2 MB); a block past them keeps its pattern and is named on stderr.
- A painted face loses the per-cell grain; variants are what break up a field.
- Revisit mips or a far colour if hand-painted textures with strong contrast shimmer at distance in play.
- The palette check of ADR 0044 (a colour outside `data/palette.toml` refused) is not in this rule yet; it
  joins `textures::decode` when the palette lands.

## Review
A fresh reviewer read the diff (2026-11-11). Fixed: the two comment lines added to `data/blocks.toml` changed
`BLOCKS.hash` (a hash of the file's text, sent in `Welcome`), so this build would have refused a server of the
last one; they are gone and the rule lives here and in `textures.rs`. Faces that fall back to one
`<block>.png` took its layers once per face (2048 layers ran out after 170 four-variant blocks); they now share
them (`gpu::place`, a test seen failing at 170 first). A block painted only underneath no longer loses its
pattern in the hotbar. Left as calls: on two of the four sides a painting reads mirrored (x runs the same
world way on opposite faces, as the patterns do; the cube faces' handedness needs a filmstrip before a flip);
a file read while another program is half way through writing it is refused for up to a second, then
reloaded (the Studio now writes by rename, so it never causes this).
