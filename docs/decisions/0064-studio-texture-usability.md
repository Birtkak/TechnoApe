# 0064 – The Studio's Textures page made plain: the block in view, patterns first, variants that explain themselves

- **Status:** accepted (steps 1 to 3 of the usability round in `docs/design/studio.md`; the developer's call
  that the pattern panel does not write `blocks.toml`; the rest routine technical calls)
- **Date:** 2026-11-12

## Context
The developer's first hour in the Studio (2026-11-12): the lit block was never seen (it sat under Tools,
Start from and Colours, off a 1080p screen); "Pattern" stamped the block's own row with no choice and no
preview; the variants had no word beside them and "Make variants" showed sets whose first square was variant 1
unchanged, so a flat variant 1 made every set look broken; a click on a set replaced the file. The round's bar:
someone new makes a texture they would keep in five minutes, every control has a plain word and a tip, nothing
replaces work without a preview, and unsaved work survives a crash.

## Options considered
1. **Where the block lives:** a fixed box on top of the inspector (chosen), or a fourth column. A column takes
   width from the canvas at 1080p; the box costs 220 points of the inspector's height, which scrolls anyway.
2. **How the block follows a stroke:** write the renderer's textures every frame, at the stroke's end only (as
   before), or the open file every 120 ms while the button is held (chosen, after measuring: below).
3. **Where a pending change lives:** in the file's undo history (a step taken back on Cancel), or beside the
   files as one pending picture that every view draws in place of the file (chosen): nothing pending can be
   saved by accident, and Use is exactly one undo step.
4. **The pattern panel's output:** the PNG only (chosen, the developer's answer), or the PNG and the block's row.
5. **The recovery file:** in `shell.rs`, or a small module of its own (chosen, `crates/studio/src/recover.rs`):
   three sessions edit the Studio at once and `shell.rs` is shared.

## Decision
- **Step 1, the block always in view** (`ui/textures.rs` `preview_box`, one hook `Page::inspector_top` in
  `ui/mod.rs`): chips `Block | 3 x 3 | 5 x 5 | Unfolded`, Block first, 190 points of picture, above the
  inspector's scroll area. A double click puts the block's camera back; the scene's controls (ground, time,
  year) moved to a "Scene" section below. While a stroke is held the open file is written for the renderer at
  most every 120 ms. The tiled previews pick variants with the game's hash: `art::variant_at(cell, n)` is
  `paint_texel`'s line, and a test holds it to the shader's text and to the formula for 121 cells and 1 to 4
  variants (red against the old pick at cell -2,-3).
- **Step 2, the pattern panel** (`pattern::Recipe`, `Textures::pattern_panel`): "Start from: Pattern" opens it
  beside the canvas: the eleven patterns as thumbnails in the colours chosen; three wells (second and third
  "auto": one step darker and lighter on the base's ramp), a click on a well then a swatch sets it (the
  swatches show in the panel too while a well waits, so it works when the inspector's palette is scrolled
  away); size, amount and contrast only where the pattern reads them, each with its number and a reset;
  Another (a new seed: the cell the pattern is drawn at); this variant or every variant (each its own seed, a
  file of one variant gets four). The result is pending in the canvas, the previews and the block; Use is one
  undo step "Start from pattern"; the panel opens again with the file's last recipe in the session.
  `pattern::texel` drew a block that is its own side from the registry's row, so a changed row showed only on
  the top; it now draws the row it is given (no change for the game's look: there the two are the same row).
- **Step 3, variants** (`Textures::wall_panel`, `ui/wall.rs`'s words): the tabs say Variant with the design's
  tip; each tab has an x (`art::remove_variant`; asks first when the variant is not one flat colour; one undo
  step). "Generate variants" opens beside the canvas; a set is drawn as a 4 x 4 patch mixed with the game's
  hash at 2.5 px a texel, its variants at 2 px under it. The words: How different, New batch, More like this,
  Back, "as now" on the set made around the file. A click makes a set pending; Use this or Cancel. A flat
  variant 1 shows "there is nothing to vary" and offers the pattern panel set to every variant.
- **The safety net** (`recover.rs`): `Recovery::keep(now, files)` writes a page's unsaved files to
  `studio-recovery/<page>/` at most every 3 s and only when they changed (into a temporary folder, then put in
  place, so a half-written set is never offered); none clears it; a clean close clears it; `left()` is what a
  crash left and `offer()` asks Restore or Discard. Restore makes each file unsaved work with an undo step
  "Recovered"; kept files the disk already holds are not offered. Textures use it from the frame
  (`ui/mod.rs` `recover`); a shot never writes the set.

## Measured
- Writing the open texture for the renderer while a stroke is held: 1.2 to 1.8 ms a write in a debug build,
  every 120 ms (`TEXWRITE` on a scripted drag); the renderer's round trip in the same runs 10 to 50 ms. The
  block follows the stroke. The whole set at a stroke's end costs 17 ms once (the folder is removed and made
  again, as before this round).
- Shots at 1920 x 1080, read: `shot=out/x.png open=grass` shows the lit block without scrolling; the open
  pattern panel (bricks, copper / stone dark / copper light, size 3) pending in the canvas and the block; the
  wall with a set pending; the flat case on grass; the recovery offer.
- Drive scripts as tests (`crates/studio/tests/drive.rs`): pick bricks, three colours and a size, Use, save,
  `check=1` passes; generate, pick (pending, not saved), Use this, the file is 48 px wide, the x asks and
  removes (32 px); a kept set is offered, restored and saved, and Discard clears it. `cargo test -p studio`:
  94 unit and 26 drive tests green.

## Review
A fresh subagent read the diff of the four commits against the task. Its findings and what was done:
1. A set pending (pattern or wall) stayed open when the face or season changed, and Use this then wrote it over
   the new face's file. Fixed: a switch of face or season closes the panel, the wall and the pending set
   (`Textures::leave`), and `wall_use` uses a set only for the file it was made for. Test
   `a_pending_set_goes_with_a_switch_of_face`, seen red first.
2. The wall's chosen set outlived the switch (the same cause): fixed with 1.
3. The recovery set could be written again in the frame of a clean quit, after it was cleared. Fixed: no keep
   while the Studio is quitting or closing.
4. A headless shot run in the working folder offered, and could clear, the developer's kept work. Fixed: a
   shot neither offers, clears nor writes it unless `recover=1` (the drive test now says so, and checks that a
   plain shot leaves the set alone).
5. The x on a variant tab showed for a face drawn from another face's file (inherited, dimmed): now only on a
   face's own file. Answered, not changed: the pattern thumbnails keep each pattern's own size and amount
   (colours and contrast follow the choices), so the row compares patterns, not settings; the wall's Cancel
   keeps the wall open on purpose, to pick another set.
Not handled: two Studios open on one folder share `studio-recovery/textures/`; the later write wins.

## Consequences
- The Models and Sounds pages can keep their unsaved work with the same `Recovery` after the merge (a page
  hands it (name, bytes)); each page keeps its own folder.
- `look::draw` at 1 px a texel draws nothing in a shot (found on the wall's singles, now at 2 px); a preview
  squeezed to 1 px a texel would be blank. Not chased here.
- The panel and the wall stand beside the canvas: at 1280 x 800 the canvas is about 300 points wide while one
  is open.
