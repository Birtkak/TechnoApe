# 0133 – The Studio catches up: plants, leaves' fluff, painted figures

- **Status:** accepted (tool tier, ADR 0096 / CLAUDE.md principle 13)
- **Date:** 2026-11-15

## Context
The developer: "Quickly get studio up to date. remove the outdated tabs and placeholders for textures/models. and
make it so i can edit and create the new way we make leaves, grass and animals." Since the Studio's pages were
built (ADR 0044, 0046, 0047, 0053) the game moved on: small plants are crossed painted planes (ADR 0091), leaves
are 32 px cutout paintings read from `data/textures/hd/` with fluff cards over the crown (ADR 0090, 0099), and
animals are painted boxes (ADR 0116). What was out of date:

- The Textures page opened and saved the 16 px leaf and bark files, which the game no longer reads: an
  `hd/<name>` file is read in their place. 19 such files.
- `data/drafts.txt` named 8 voxel plant models deleted with ADR 0091 (grass tuft, fern, flowers).
- The Models page grew voxel plants ("Plant…", the "Plant cross" starter): the game no longer makes plants so.
- Every model's thumbnail was black (painted figures have empty voxels), and the views drew nothing for them:
  the Studio preferred `target/release/technoape.exe` (built 11-12, before painted figures) over the game beside it.
- Nothing edited plants, a leaf's fluff, a part's boxes or a model's painting; Fauna could not make a new row.

No tab was out of date: all seven still edit files the game reads. None removed.

## Decision
- **Textures:** `art::textures` lists a folder's PNGs and `hd/`'s; `art::tex_path` is the file the game reads.
  Load, save, the disk check, the watch and the palette's repaints use them. The 19 shadowed 16 px files are
  deleted (`tools/paint_textures.py` still writes both when run).
- **Leaves:** a cutout block's inspector has **Fluff** (size, push, droop, dense, light), written to its row's
  `fluff` when a slider is let go (`fluff_row`); like Kind it shows after a rebuild (blocks.toml is built in).
- **Plants** (new page, `ui/plants.rs`): the `kind = "plant"` rows and their paintings (base, `.1`-`.4`, looks,
  `.back`, `.far`). Left paints the painting at its pixels, right a stand of 3 x 3 drawn from the game's own
  `plant_faces`; planes 2-4, size (kept at its foot), "+ alternate", **New plant** (a tuft and a row). Save holds
  the row to `engine::clutter::parse` and each painting to `engine::models::painting` and the 64 x 80 px rule
  before writing any. A new or changed row shows in the game after a rebuild (clutter.toml is built in).
- **Animals:** the Models page keeps each model's painting with it (its own undo, a stroke a step), shows it in
  a **Painting** section with the open part's nets outlined, and lists the part's **boxes** (corner, size, turn,
  net). **Add a box** lays a new box's net under the painting in the first colour; **Voxels to painted boxes**
  turns a starter (Creature) into a figure (`voxel::add_box`, `voxel::voxels_to_boxes`). Save holds the painting
  to `check_painting` too. Thumbnails sample the painted faces. Fauna has **New creature** (the open row copied
  under a new name; its model is picked in Body). The view picks the newest game exe.
- Removed: the voxel plant generator (`voxel::plant`, `PLANTS`, its menu, starter and test).

## Evidence
Tests (red seen first except where said): `a_new_plant_saves_as_the_game_reads_it` (red "not built"),
`fluff_is_written_to_the_row_and_read_back` (red: None), `a_voxel_starter_becomes_a_painted_figure` (red "not
built"), `a_fine_painting_in_hd_is_the_one_opened_and_repainted` (written after the code; not seen red).
Shots read (`out/st/`, not committed): `s1-Models` (the ape drawn painted, after the exe fix), `s1-leaves`
(broadleaf's 32 px painting, fluff cards in the Block view), `s2-plants-new` (a clover from nothing),
`s2-bush` (berry bush, 4 planes, its looks), `s3-fluff` (spruce's numbers), `s4-figure` and `s4-painting`
(a creature from nothing in a copy of `data/`, saved; boxes and the painting's nets). The game read that saved
figure: `run=stand models=<copy> model=creature`, 19 models loaded, none refused, the figure in the filmstrip.

## Shortcuts
- The plant stand is drawn by the Studio (egui, back to front by plane), without the climate tint, wind or
  light; the game view cannot stand a chosen plant yet.
- A plant's ground, seasons and biomes stay in the files; Ctrl+Z on Models undoes the model, not paint strokes
  (the Painting section's own buttons do); the painting is not in the crash-recovery set.
- The voxel tools stay on the Models page (the engine still reads voxels; the old voxel models are test data).

## Review
A fresh reviewer (Sonnet) read the diff (over 300 lines). Fixed: a stand of a plant with no base or numbered
painting indexed an empty list (a panic); the Fluff sliders wrote nothing when a drag was let go (the numbers
are now kept while moving; a driven drag writes the row); a doubled `#[test]`; a refused `<model>.png` could be
written over (now refused); a new plant's name was checked against the built-in rows, not the file, and an
existing painting could be overwritten; a plant's row was written before its paintings (now after); a broken
clutter.toml showed an empty page (now a problem row); removing a season left its 16 px namesake to show
(both go now). Left as shortcuts above: the model's and painting's undo are apart, the painting is not in the
recovery set, a size drag is a step a frame. The drive tests also caught one of mine: an eighth tab wrapped
the bar to a second row at 100 %, pushing the pattern panel's Use off an 800 px window; tabs now fit their
words (64 px at least). `cargo test -p studio`: 120 unit and 29 drive tests pass.
