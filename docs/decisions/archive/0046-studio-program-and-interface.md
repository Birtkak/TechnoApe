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

**Added 2026-11-12 (the look, second pass).** One accent alone left every page the same grey: a colour per
page (`look::PAGES`: tab, section bands) now says where you are, and the accent still means "chosen". Undo,
Redo and Save moved from each page's inspector into the frame (one place, `btn:undo|redo|save` for `input=`);
`look::row` lines labels up in one column and falls back to label over control under 250 points wide, which
keeps the earlier rule that the inspector fits at any interface scale; `look::tool` and `look::toggle` tell
tools from options; the files column splits a label into heading, name and tag (`look::split_label`, tested).
Lines between sections are gone: the band separates. "New model" and "Plant…" moved from the parts tree's head to the inspector's Model section. A slider in a
row is about 88 points at the default inspector width; the value boxes are typed into for exact numbers.

**Added 2026-11-12 (the layout is the developer's).** Asked for: make big what is needed at the moment, as
in Blender. Built on what the frame has, no docking library: `Settings` gains `hide_files`, `hide_inspector`,
`swap`, `fold=Page/Section` lines and `split.Page/name=share` lines (round trip tested); `look::section`
returns whether it is open and every inspector section is wrapped in it; `look::split` and `look::split_bar`
give a page's work area a dragged divider. The side panels' widest is now what leaves the work 520 points
(on a small window the old shares of 20 and 30 % still hold, so a large scale still fits). Checked by shots
at 800x600, 1024x600, 1280x720 at 150 %, 1400x850, 1920x1080 and 2560x1440 at 200 %. Left out: areas that
split and join freely with any editor in any area (a docking system: a library and its own ADR), dragging
sections into another order, a saved layout per page or per screen, touch.

**Added 2026-11-12 (a layout per page, sections in the developer's order, Try in game).**
- `studio.txt`: `hide=Page/files|inspector` and `width.Page/files|inspector=` replace the two global hide
  flags of the same day (never released); `order.Page=A|B|C` keeps a page's sections in the dragged order.
  `files=` and `inspector=` stay as the width a page has until its own is dragged.
- **Sections in another order.** A page draws its sections in code order into the panel, so a different
  order needs each section to be drawn somewhere else than where the panel's cursor is: `look::section` now
  returns a `Sec`, the room the section is drawn into. With no order for the page it is the panel itself and
  nothing changes. With one, the first section of a frame takes the room of them all and each gets a child
  area at its turn, placed by the heights measured last frame; a section whose height changes is one frame
  late and asks for a repaint. `display_order` and `reorder` are tested; the placing is checked by a shot
  (Fauna: Gait dragged above Parts). Widgets a page draws between sections come after the placed block.
- **Try in game** (F5, `shell::try_args`, tested): the Studio saves the open file and starts the game exe
  the view uses with `run=play onscreen=1 save=0`, creative by `acts=`, and `model=`, `fxplay=`,
  `structure=` or `creature=` for the open thing. Each argument set was run off-screen for three seconds
  without a refusal. The Studio does not wait for the game or close it.
- Left out: Blender's free areas (a docking library: its own ADR, not wanted now); dragging a section to
  another page; "Layout as new" does not make egui forget a dragged width until the Studio starts again.

**Added 2026-11-12 (a second look pass).** Small things, each seen in a shot of its page: every slider's
number box in an inspector row is one width (`look::VALUE`), so the right edges are a line; the inspector's top
names the open file; a section's title shows a grip under the pointer (it can be dragged); a palette colour's
cell is wide enough for its name; a model with no clip chosen has a one-line timeline and the view takes the
rest. Logic: `studio.txt` keeps `page=` and `open.<Page>=` and the window starts there (`Studio::resume`; the
round trip is tested, the start itself was not run on screen); Ctrl+1..7 are the pages (driven: Ctrl+4 shows
Sounds), and the command box lists them once, with their keys.

**Added 2026-11-12 (the game's font, sleeker controls).** The interface is set in the game's font. egui reads
outlines and the game has 8 x 8 cells, so `crates/studio/src/font.rs` writes a TrueType file in memory from
`client::font_cells` (a rectangle a lit run; seven tables, checksums left 0: egui's reader does not check
them): the cells stay the one source, and no font file or dependency is added. Chosen against the game's
fixed step of 6: each character steps its lit columns and one (digits keep 6, so a dragged number does not
shake), or the label column could not hold its labels. At size 14 a cell's pixel is 1.5 screen pixels, so
the edges are a little soft at 100 %: whole pixels would be 1 (too small to read) or 2 (nothing fits); at an
interface scale of 133 % a pixel is 2. Hinting and sub-pixel places are off for it. Text runs about a
quarter wider than before: the label column is 108, a slider's number box 80, the panels start at 230 and
320. Characters the font does not draw (the degree sign, the ellipsis) fall back to egui's own. Tested: egui
lays a text out as wide as the cells say. Controls: a fine edge on buttons, no growth under the pointer, the
accent when pressed; sliders with a 4 point rail, lit up to a slim handle. Seen in shots of Fauna and Sounds.

**Added 2026-11-12 (sharp text, icons, a player on Fauna, talking to the running game).**
- **Sharp text.** A pixel font is sharp only where its pixel is whole screen pixels, so: one text size (small
  text was 12 against 14, never both sharp at one scale; it is now told by its colour, and the label column is
  124); the scale steps are `shell::sharp_scale` (tested: on a plain screen 67, 133, 200 %; the screen's own
  pixels a point are counted in); the window's first frame puts the kept scale on the nearest (a shot keeps
  the scale it is given, so shots at 100 % stay soft). The range starts at 60 % for the 67 step. The six
  characters the texts use beyond the game's font are cells in `font::MORE`; the file's character map became
  segments. Seen sharp in a shot at 133 %.
- **Icons**: `icons.txt` beside the code, read by `look::icons` (tested: every tool of both pages has one that
  fits), painted as squares at the left of the button's text. Drawn blind as text and seen once in a shot at
  133 %: they read, they are not art.
- **The player on Fauna**: `Viewport::player`, which Models' outline now also uses. Seen in a shot walking in
  place; in the circle's middle not looked at.
- **Try in game, again**: the Studio keeps the game it started and its stdin; `shell::game_line` (tested) is
  the line. The game's side is ADR 0052's addition of this day.
