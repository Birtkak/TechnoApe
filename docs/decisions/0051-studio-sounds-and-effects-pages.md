# 0051 – The Studio's Sounds and Effects pages

- **Status:** accepted
- **Date:** 2026-11-12

## Context
`docs/design/studio-tools.md`, "Sounds" and "Effects": two pages on the game's own rows. The game sides are
ADR 0048 (bases and chains) and ADR 0050 (the effects' new fields). Both files hold many rows, so a save may
touch only its own row; the page must never wait on a render; a shot never opens the sound card or writes
into `data/`.

## Decision
- **One row's text** (`crates/studio/src/rows.rs`): a row runs from its `[[header]]` line to its last line of
  its own (blank lines and comments after it belong to what follows). A save puts the new text there in the
  file's own line endings, every other byte unchanged, checks the whole file with the game's reader
  (`audio::parse`, `fx::parse_set`), and writes through a temporary file and a rename. A new row goes at the
  end after one blank line. A row is kept as its keys as written, so keys the page does not edit (a sound's
  `layer`) stay exactly as they were; the row's own inner comments are kept too.
- **Sounds** (`sound.rs`, `ui/sounds.rs`): the page edits `from`, `chain`, `spread` and `variants`. A card
  switched off is heard without (one off: `render_row`'s `skip`, so the others draw the same numbers; several:
  left out) and is **not saved** (the file has no "off"). Rendering runs on one worker thread, newest job of
  each kind wins; it holds a bank of only the rows the rendered rows are made from (not `music.menu`), built
  on demand. Mutate moves each number by up to a share of its range (frequencies by octaves), deterministic for
  the wall's seed. Starts are ten fixed chains (`STARTS`). In place's cave is a reverb step plus a little
  dullness, until the game's cave reverb. Loop rows are re-sent each lap (the mixer cannot stop a clip), so
  an edit is heard from the next lap.
- **Effects** (`effect.rs`, `ui/effects.rs`): the page writes its copy of the whole file (disk text with its
  edited rows put in) to `out/studio-view/effects.toml` and the renderer plays from it. The view protocol
  (`crates/client/src/view.rs`) gains `effects <file>` (read and parsed on the reading thread, swapped in by
  `Fx::swap`: what plays keeps playing with the new numbers) and `effect <name>` / `effect -` (`Fx::show`: that
  row or group lasting on the stand cell's floor, from its start). The Studio starts the renderer with `fx=1`.
  A page that does not set `Viewport::effect` before drawing shows none. A group's row switched off is not
  saved in it. The wall's previews are small closed-form sketches, not the game's renderer.

## Consequences
- Not built: States on the Models page (part 3); a sound row's own scalar keys (`gain`, `reach`, `bus`) as
  controls.
- Added 2026-11-12: `colour` (three sliders, for a square or texture without a gradient; "one colour" turns a
  gradient into its first stop's colour), a cone's `dir` (three sliders that never reach a length of 0), `then`
  (a pick of the file's saved rows) and `look = "model"`/`"texture"` with a pick of `data/models/*.toml` or the
  game's own blocks. Choosing a look drops what the game takes only with another (`effect::choose_look`: a
  gradient and glow off squares and textures, a face off debris and blocks, white for a square with no colour),
  so the page never holds a row the game refuses. In an inspector under 220 points wide a label stands over its
  slider. Drive test `effects_colour_dir_then_texture_and_model_save_as_the_game_reads_them`.
- Review: not yet (a fresh reviewer reads the diff before the commit).
