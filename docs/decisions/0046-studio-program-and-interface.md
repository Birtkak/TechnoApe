# 0046 – The Studio's program and its interface library

- **Status:** accepted
- **Date:** 2026-11-11

## Context
Step 3 of `docs/design/studio.md`: the Studio's shell and its texture page, a second program beside the game
(ADR 0044). It needs an interface far larger than the game's own menu toolkit (file list, tabs, a pixel
canvas, sliders, text fields, a modal, drag to reorder), and it will later draw the game's renderer in the same
window, so it must share the workspace's wgpu 30 and winit 0.30 and never pull a second copy. Claude must be
able to check its layout without eyes (working principle 10).

## Options considered
1. egui 0.37 (`eframe` with only its wgpu backend, `egui-wgpu` for the off-screen shot) – the 0.36 and 0.37
   releases depend on `wgpu ^30` and `winit ^0.30.13`, the workspace's own (Cargo.lock holds one wgpu,
   30.0.1, and one winit, 0.30.13); immediate mode keeps the page a plain function of the Studio's state, so a
   shot is the same code run on an off-screen texture; a wgpu paint callback lets the game's renderer draw
   into a panel later. Costs: a larger dependency tree for the Studio's binary (not the game's).
2. Grow the game's menu toolkit – no new crate, but text fields, sliders, scrolling and modals would all be
   ours to write, and the game's client would carry editor code (ADR 0044 refused that).
3. Another toolkit (iced, slint) – their wgpu and winit releases trail or differ from ours; a second wgpu is
   refused outright.

## Decision
Option 1: `eframe` 0.37 (`default-features = false`, `wgpu` and `default_fonts`), `egui-wgpu` 0.37, in
`crates/studio` only; the game never links egui. Details, as routine calls:

- **One crate, `crates/studio`, binary `technoape-studio`**, handed over as `build/TechnoApeStudio.exe` by
  `tools/playtest.sh`. Without arguments it opens its window in the repository the exe sits in, as the game
  does.
- **The palette lives in the engine** (`crates/engine/src/palette.rs`): parse, check and write
  `data/palette.toml`; it refuses a name that is empty, not lower case or used twice, and a colour listed
  twice (a pixel must name one colour). Writing it drops comments and writes its own header.
- **The editing is pure and tested apart from the window** (`crates/studio/src/art.rs`): pencil, fill,
  line, mirror (left-right across the variant's middle), variants, undo and redo as whole snapshots per
  file (a few KB each), the PNG rule and the palette check, repainting a colour, the drafts list, the tiled
  sheet. The window (`ui.rs`) only maps the pointer and keys to those.
- **The canvas is drawn as one rectangle per pixel**, no GPU textures: at most 256 per canvas and 2,304 per
  preview, so the page needs no texture bookkeeping and a shot draws exactly what the window draws.
- **Headless:** `check=1`, `drafts=1`, `render=<png> sheet=`, and `shot=<png> [open=<block>]
  [page=palette]`, which runs three egui passes at 1280 x 800 and draws the last with egui-wgpu into an
  off-screen texture read back to a PNG. The dark theme in both.
- **A face without its own file** shows the block's every-face file dimmed; the first stroke on it makes the
  face's own file from that picture (or, with nothing painted, from the palette colour nearest the block's
  `colour`). A file is dirty while it differs from what is on disk, so undoing back to the saved picture
  clears its dot.
- **A palette change** repaints every texture in memory as the slider moves (refusing a step onto another
  palette colour, which would merge the two for good) and, when the slider is let go or the hex is entered,
  writes `data/palette.toml` and repaints the files on disk that use the old colour. Saving a texture or the
  palette takes it off `data/drafts.txt`.
- **The PNG rule is the game's** (`engine::textures::decode`); `check=1` adds the palette and block-name
  checks on top. **Shortcut, marked in the code:** the 3 x 3 preview picks variants by its own hash of the
  tile until the game has its variant pick.

## Consequences
- The Studio builds with the workspace and shares its wgpu; the lit-cube preview can come as an egui-wgpu
  paint callback without a second device.
- An egui release that moves to wgpu 31 must wait for the game's move (or the other way round); both are
  pinned by the workspace.
- `check=1` is cheap enough for the gate (ADR 0044 says the gate grows by it); not added yet.
- Left for later steps: the lit cube, "start from pattern", the game's watch on `data/`. Not verified by a
  shot: the pointer's own work (painting, dragging swatches); that logic is the tested `art` module.

## Review
A fresh reviewer read the diff (2026-11-11). Fixed, each with a test seen failing first where it is `art`'s:
a slider let go where it started rewrote every texture using that colour and took the palette off the drafts
list (`repaint` now changes nothing for a colour onto itself, and `flush` stops there); the palette now leaves
`data/drafts.txt` only when a colour itself changed, not on a new order, name, add or remove; a repaint that
could not write one file stopped there with the palette already saved, leaving the rest off-palette and the
Studio showing them saved (it now repaints the others and shows the unwritten ones as not saved); a face
painted over a PNG the Studio had refused on load silently replaced that file on save (a new file now never
replaces one on disk); textures are written to a temporary file and renamed, so the game's poll never reads
half a file. The Studio's copy of the PNG rule is gone. Left as calls: the Studio does not watch `data/`, so a
file changed by another program while it is open is overwritten by the next save of it; a slider passing over
a colour that some file holds outside the palette merges those pixels in memory.
