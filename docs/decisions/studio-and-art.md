# The Studio and the art it makes

What holds today for the Studio (`crates/studio`, binary `technoape-studio`), the texture, model, painted-figure and
map files it writes, and the painted looks of blocks, plants and figures. Numbers in brackets are the ADRs
(`archive/NNNN-*.md`) a decision came from; INDEX.md lists every number.

## The developer's calls

- The game's art gets the developer's own hand: a stand-alone program of our own, drawing with the game's renderer and
  writing the game's own data files (option 3 of 0044). Not Aseprite, MagicaVoxel or Blockbench, not an editor mode
  in the game [0044].
- Block textures and voxel models first; particles, then trees and structures, later [0044].
- Model voxels are 12.5 cm, the fine cell of ADR 0033 (first 1/16 m, changed the same evening so a model matches what
  the world breaks into) [0044].
- Models have rigid moving parts with keyframed clips from the first version [0044].
- Claude may make drafts; they are listed in `data/drafts.txt` until the developer has redone them [0044].
- One fixed palette (`data/palette.toml`): every texture and model is painted from it; the check refuses a colour
  outside it. It grows in place where you paint, it is not replaced by a second one [0044, 0080].
- The pattern panel writes the PNG only, never `blocks.toml` [0064].
- Painted block textures are **calm and readable**: three or four colours from one ramp, low contrast between
  neighbours, a material shape instead of noise, each rock and ore told apart at a glance, readable from 100 m [0084].
- Calmer leaves (two close tones in soft clumps), bare winter branches in the bark colours, and lamps that join into
  one glowing surface (ADR 0085) [0084, 2026-11-13 calls].
- Winter twigs must grow out of the wood, "not just a block with random sticks flying" [0084].
- Flora at 32 px a metre with rich colour ("still too low quality of details and colours"); deeper greens, painted for
  the plants' shader; calm bark; every mass (leaves, grasses, ferns, reeds, bushes, shrub, cactus, bamboo, palm fronds)
  gets outline, depth and a far version, single flowers, mushrooms, sticks and stones do not [0084].
- Reeds must differ (four kinds); the shrub stands in the trees' greens [0084].
- The ape, animals, furnace and leaf are painted boxes ("paint the character like the new way but also all the animals
  and other entities") [0116].
- "Quickly get studio up to date": remove outdated tabs and placeholders and make leaves, grass and animals editable the
  new way. No tab was outdated; none removed [0133].
- Opaque error messages are reworded; the Models page works on painted boxes only (no voxel tools); sliders such as a
  reed's fluff show live in the view [0135].
- The Studio must look "more like modern Adobe tools", not "the same and bland" [0155].
- Maps tile and loop "without thinking about it" [0153].
- The layout is the developer's: make big what is needed at the moment, as in Blender [0046].

## Program, libraries and headless runs

- **One crate** `crates/studio`, binary `technoape-studio`, handed over as `build/TechnoApeStudio.exe` by
  `tools/playtest.sh`. Without arguments it opens its window in the repository the exe sits in [0046].
- **egui** via `eframe` 0.37 (`default-features = false`, `wgpu` and `default_fonts`) and `egui-wgpu` 0.37, in
  `crates/studio` only; the game never links egui. It shares the workspace's wgpu 30 and winit 0.30; a second copy is
  refused. An egui move to wgpu 31 waits for the game's move. The `experimental` feature of eframe's own egui is on for
  widget themes (`look::Theme`) [0046, 0155].
- **The palette lives in the engine** (`crates/engine/src/palette.rs`): parse, check, write `data/palette.toml`. It
  refuses a name that is empty, not lower case or used twice, and a colour listed twice. Writing drops comments and
  writes its own header [0046].
- **Editing is pure and tested apart from the window** (`crates/studio/src/art.rs`): pencil, fill, line, mirror,
  variants, undo and redo as whole snapshots per file, the PNG rule and palette check, repainting a colour, the drafts
  list, the tiled sheet. `ui.rs` only maps the pointer and keys [0046].
- **The canvas is one rectangle per pixel**, no GPU textures (at most 256 per canvas, 2,304 per preview) [0046].
- **Headless** (one JSON line, exit 1 on a refusal that names file and pixel) [0044, 0046, 0153]:
  - `check=1` (palette and every texture), `drafts=1`, `render=<png> sheet=out/x.png` (3 x 3),
  - `shot=<png> [open=<block>] [page=palette]` (three egui passes at 1280 x 800, the last drawn off-screen and read back;
    dark theme; a shot never offers, clears or writes the recovery set unless `recover=1`),
  - `map=<name> out=x.png [frame=k]`, `input=` drive scripts.
- **Saving**: textures are written to a temporary file and renamed, so the game's poll never reads half a file. A file is
  dirty while it differs from the disk, so undoing back to the saved picture clears its dot. A new file never replaces
  one on disk (a PNG the Studio refused on load is not overwritten). Saving a texture or the palette takes it off
  `data/drafts.txt`; the palette leaves it only when a colour itself changed, not on a new order, name, add or remove
  [0046].
- **The Studio does not watch `data/`**: a file changed by another program while open is overwritten by the next save of it
  [0046]. (Models: the view watches model files changed outside, 0049.)
- **A face without its own file** shows the block's every-face file dimmed; the first stroke makes the face's own file from
  that picture (or from the palette colour nearest the block's `colour`) [0046].
- **A palette change** repaints every texture in memory as the slider moves (refusing a step onto another palette colour,
  which would merge two) and, when let go or the hex is entered, writes `data/palette.toml` and repaints the files on disk
  using the old colour. A colour onto itself changes nothing. A repaint that cannot write a file repaints the others and
  shows the unwritten ones as not saved [0046].
- **Try in game** (F5, `shell::try_args`): saves the open file and starts the game exe the view uses with
  `run=play onscreen=1 save=0`, creative by `acts=`, and `model=`, `fxplay=`, `structure=` or `creature=` for the open
  thing. The Studio keeps the game it started and its stdin (`shell::game_line`); it does not wait for it or close it
  [0046].

## Pages and layout

- **Nine pages** (`look::PAGES`): Textures, Palette, Models, Sounds, Effects, Structures, Fauna, Plants, Maps. Ctrl+1..9
  open them; the command box (Ctrl+P) lists them with their keys [0046, 0133, 0153].
- **A colour per page** says where you are (tab dot and underline, section bar); the accent means "chosen". Maps is ice
  cyan (110, 214, 240) [0046, 0153, 0155].
- **Undo, Redo, Save in the frame** (`btn:undo|redo|save` for `input=`), not per page [0046].
- **`look::row`** lines labels up in one column and falls back to label over control under 250 points wide, so the
  inspector fits at any interface scale. Every slider's number box is one width (`look::VALUE`); value boxes are typed
  into for exact numbers [0046].
- **The layout is the developer's**, built on the frame, no docking library: `studio.txt` keeps `page=`, `open.<Page>=`,
  `fold=Page/Section`, `split.Page/name=share`, `hide=Page/files|inspector`, `width.Page/files|inspector=` (`files=` and
  `inspector=` stay as the width until a page's own is dragged), `order.Page=A|B|C` (sections in the dragged order;
  `look::section` returns a `Sec`, placed by last frame's heights, one frame late when a height changes). Side panels
  are at most what leaves the work 520 points; on a small window the 20 and 30 % shares hold. Checked by shots from
  800 x 600 to 2560 x 1440 at 200 % [0046].
- **The inspector's top** names the open file; a section's title shows a grip; a model with no clip chosen has a one-line
  timeline [0046].
- **Font** (0155 replaces the 0046 game-font default): Segoe UI Regular and Semibold read from the system font folder at
  start (no font file in the repo, no new crate), egui's own font where missing. Body 13, small 11.5, titles and tabs
  semibold. The game's pixel font stays as View > Interface font: Clean | Game (`font=game` in `studio.txt`, Clean when
  absent). Game font: `crates/studio/src/font.rs` writes a TrueType file in memory from `client::font_cells`, one text size,
  scale steps `shell::sharp_scale` (67, 133, 200 % on a plain screen), label column 124, extra characters in `font::MORE`.
  The clean font's scale menu offers 80 to 200 %; a scale kept from the game font snaps to the nearest at the first frame
  [0046, 0155].
- **Icons**: `icons.txt` beside the code, read by `look::icons` (every tool of both pages has one; they read, they are not
  art) [0046].
- **The look** (ADR 0155's tokens in `ui/look.rs`): work area `#1e1e1e`, panels `#2a2a2a`, raised and hover `#353535`,
  pressed and chosen `#434343`, hairlines `#3d3d3d`, field borders `#525252`; text `#e6e6e6`, secondary `#b0b0b0`,
  disabled `#6e6e6e`; accent `#2680eb` (hover `#378ef0`); positive `#33ab84`, notice `#e68619`, negative `#f0626a` (a step lighter than the brief's `#2d9d78` and `#e34850`, so small status text keeps 4.5:1 on the
  panels).
  One accent for the main action (one filled pill: Try in game, Save when dirty, Use, Import; `look::primary`, one per
  screen) and for focus (2 px ring, the chosen node). Quiet buttons (fill on hover), corners 4 on controls and 8 on
  cards, menus and windows, soft shadows, 1 px field borders, sliders with a 2 px track and round handle, chips as one
  segmented group, tooltips with the key in a keycap. Frame: 40 point top bar of plain-text tabs, "Find... Ctrl+P",
  View, Keys, the scale, Try in game (the open tab white, underlined in its page colour); flat section headers; the open
  file's row chosen grey with an accent bar; "draft" as a small amber pill; a slim status line [0155].
- **The variations wall** (`wall.rs`, `ui/wall.rs`): one action bar above the grid (Use and Cancel where the page's pick
  waits, More like this, New batch, Back, How different from subtle to wild); large even wordless pictures, a click picks,
  a double click uses, a round "more like this" and play button on hover. `wall::Act` has Use and Cancel, so Maps and
  Textures let the wall draw them [0155].
- **The Studio's text** about 45 refusals say what did not happen, why, and what to do [0135].

## Textures: PNG files and the Textures page

- **The rule** (`engine::textures::decode`, also what `check=1` calls): 8-bit RGBA, 16 high, 16/32/48/64 wide, every pixel
  alpha 255; the width is 1 to 4 variants side by side. `<block>.png` is every face; `_top`, `_side` (the four sides) and
  `_bottom` override one. A file that breaks the rule, or names no block or face, is refused with its name and why and
  its face falls back to `<block>.png` or the pattern. `check=1` adds the palette and block-name checks [0045, 0046, 0044].
- **No block must be painted**: the shader patterns (ADR 0039) stay the look of a block without a file and the start of a
  painting [0044, 0045, 0084].
- **Storage** (option 2 of 0045): the layers in a storage buffer, one `u32` a texel, read once per fragment, no filter,
  no mips (a texture array took a new 256 MB memory block and failed `fly` on VRAM). 2048 layers (2 MB); a block past them
  keeps its pattern and is named on stderr. Faces that fall back to one `<block>.png` share its layers
  (`gpu::place`) [0045].
- The client's `textures` thread polls names, sizes and times once a second, loads on a change and sends the set; the frame
  thread copies at most 2 MB of layers and the looks buffer (`Gpu::take_textures`). `textures=dir` reads another folder.
  Decoding with the `png` crate, never on the frame thread [0045].
- `Look` carries per face a layer, the variant count and a painted bit (`tex`, `tex2`), and the block's one colour
  (`mean`). A painted face of the block itself wins; else the `side`/grass redirect runs. The variant is picked by the
  cell's hash (`art::variant_at(cell, n)` is `paint_texel`'s line, held to the shader's text by a test), the same for every
  face of a block. On a side x runs along it and y down from its top edge; `texel` turns x on the -x and +y sides so a
  texture reads left to right from outside on all four sides [0045, 0047, 0064].
- A painting is drawn as painted, no grain tone on top. The bytes are the same numbers as `blocks.toml`'s colours (x 255),
  no sRGB decode. `mean` (top's painting, else side's) feeds `seen` (bounce light, reflections, far terrain); the hotbar
  and "looking at" cubes take the top's and side's means. `blocks.toml` is not rewritten [0045].
- Distance: one texel per 1/16 of a cell, no filter, no mips; coarse far chunks show the painting stretched over their
  larger cells [0045].
- **The block always in view** (`ui/textures.rs` `preview_box`, hook `Page::inspector_top`): chips Block, 3 x 3, 5 x 5,
  Unfolded, 190 points of picture above the inspector's scroll area; a double click resets its camera; ground, time and
  year are in a "Scene" section. While a stroke is held the open file is written for the renderer at most every 120 ms
  [0064].
- **The pattern panel** (`pattern::Recipe`, `Textures::pattern_panel`): "Start from: Pattern" shows the eleven patterns as
  thumbnails in the chosen colours; three wells (second and third "auto": a step darker and lighter on the base's ramp);
  size, amount, contrast where the pattern reads them; Another (new seed); this variant or every variant. The result is
  pending in canvas, previews and block; Use is one undo step "Start from pattern". `pattern::texel` draws the row it is
  given [0064].
- **Variants**: tabs say Variant, each with an x (`art::remove_variant`, asks first when not one flat colour, one undo
  step). "Generate variants" (`Textures::wall_panel`) draws a set as a 4 x 4 patch mixed with the game's hash; a click makes
  it pending, Use this or Cancel; a flat variant 1 says there is nothing to vary and offers the pattern panel [0064].
- **Pending changes** live beside the files as one pending picture every view draws in place of the file, so nothing
  pending is saved by accident. A switch of face or season closes the panel, the wall and the pending set
  (`Textures::leave`); `wall_use` uses a set only for the file it was made for [0064].
- **Leaves and bark**: `art::textures` lists a folder's PNGs and `hd/`'s; `art::tex_path` is the file the game reads (an
  `hd/<name>` file shadows the 16 px one). A cutout block's inspector has **Fluff** (size, push, droop, long, dense,
  light) written to its row's `fluff` when a slider is let go (`fluff_row`) [0133, 0135].
- **Fluff live**: while a slider is dragged the view takes `fluff <id> size push droop long dense light` (six finite
  numbers, clamped to the sliders' ranges; `Gpu::set_fluff` writes the block's fluff and light words in its four season
  rows); nothing keeps them, the next `looks_of` writes the row's own back. Letting go writes the row, and a running game
  and the view take it within a second (ADR 0134). The Studio sends the row's numbers to the view when the slider stops.
  A renderer older than this prints "unknown command" and draws the saved numbers [0135].

## Palette

- The palette has no size limit (textures hold RGB, models name colours); one model holds at most 62 colours, a letter
  each (`engine::models::LETTERS`). Swatches show "N colours" (k of n under a search); the Models page shows "This model:
  k of 62 colours", amber from 56 [0080].
- **Under every palette's swatches** (one function, `textures::swatches`: Textures' Colours, Models' Colours, the pattern
  panel's wells; `ui/grow.rs`): a search over colour and ramp names, and "+ New colour", "Add shades", "Remove", and
  "Undo: <what>" after a change made there. Each page has its own search and Undo [0080].
- **The swatches send commands to the Palette page** (`add:<name>=#rrggbb`, `shades:<name>`, `remove:<name>`,
  `undo:<step>`), which the frame runs after the page is drawn; checks, file, drafts list and undo stay that page's.
  Undo sends the step's own name and the Palette page undoes only when its last step is that one. The Palette page
  writes at once, a step holds the file's old text, undo writes it back and repaints the textures [0080, 0049].
- **+ New colour**: hue, saturation and value sliders, a hex box, a name suggested from hue and lightness (`ramp::suggest`,
  free in the palette; follows the colour until typed). Add is greyed with the reason in red while the name or colour is
  taken. It goes at the end of the palette and becomes the first colour (or fills the armed well) [0080].
- **Add shades**: `ramp::shades` makes `ramp::make`'s ramp of five around the first colour and adds four: "<name> darker",
  "dark", "light", "lighter" (numbered when taken), each before the first colour lighter than it; a loose colour gets a new
  ramp named after it. Shade and the patterns' auto colours walk ramps [0080].
- **Remove** says "N file(s) use it (...)": a texture in memory or on disk, or a saved model file whose `colours` names it
  (`palette::models_using`, read at most once a second). An unsaved model's new colour is not seen [0080].

## Recovery

- `crates/studio/src/recover.rs`: `Recovery::keep(now, files)` writes a page's unsaved files to `studio-recovery/<page>/` at
  most every 3 s and only when they changed (into a temporary folder, then put in place); none clears it; a clean close
  clears it (no keep while quitting); `left()` is what a crash left, `offer()` asks Restore or Discard. Restore makes each
  file unsaved work with an undo step "Recovered"; kept files the disk already holds are not offered [0064].
- Textures use it from the frame (`ui/mod.rs` `recover`). Models keeps `<name>.toml` (the text a save writes) and Sounds
  `<row>.toml`, through `Page` methods `kept`, `unsaved_of`, `restore`; a new model or row stays new. A kept file the
  page's reader refuses is set aside in `studio-recovery/<page>-refused/` with a red toast. A shot leaves the sets alone
  [0064, 0080]. Two Studios on one folder share the set; the later write wins. A model's painting is not in the set
  [0064, 0133].

## Voxel models and the model file

- **The format** (`crates/engine/src/models.rs`, `data/models/<name>.toml`): parts of coloured voxels in a tree, named
  points, clips of keys. The voxel is `engine::models::VOXEL` = 1/8 m (8 to a block's edge), a part's box is 1..32 a side
  [0047].
- `Model::parse(file, text, &Palette)` (serde, `deny_unknown_fields`) refuses with the file's name and why: a colour not in
  the palette, over 62 colours, a box outside 1..32, a layer or row of the wrong size, a letter past the colours, a part
  named twice, an unknown parent or a loop, a point or key naming no part, t outside the clip. Also `write`,
  `decode`/`encode` of the voxel rows, `mesh(&Part)` (faces against the part's own voxels dropped, same-colour runs merged
  per slice) [0047].
- `pose(clip, t)`: per part a `Mat4` from its voxels to the cell's metres; keys per part, ease linear/smooth/step from the
  key it leaves, held before the first and after the last; turn x then y then z about the pivot; children follow; a
  looping clip wraps [0047].
- `models::write_keeping(old, model, life)`: a save keeps the developer's text: leading comments on top, other comment
  lines above the section they stood above (found by header and `name`, else by place; a comment inside a section goes
  above the next one), CRLF when the old file had it. Key order and blank lines are the writer's [0047].
- **Game side**: a block row may name a `model` (the registry hash is of the file's text, so it moves only when a row uses
  the field); `World::models` keeps the cells holding such a block; `stream::hide_models` turns them to air for the mesher
  (cube neither drawn nor traced); collision is untouched. The client (`crates/client/src/models.rs`) reads and meshes the
  folder on its own thread (`gpu::watch`, palette from `data/palette.toml`), copies the packed quads once per change (1 MB
  buffer, 16 B a quad), poses every instance per frame on the CPU in the camera's frame (f64, no precision loss far from
  the body's centre) and writes one 3 x 4 transform per part (64 B a piece); one draw per part, one instance per piece
  (`vs_model`/`fs_model`), in the terrain's G-buffer pass, lit by `light()` where it stands. A cell's frame maps the
  model's x, y (up), z to the grid's j, layer, i. Model cells are drawn within 128 m. A block in hand is drawn as its
  model when a file of its name exists. Clip `idle` loops when present. The main bind group has 10 storage buffers; the
  device asks the adapter's own limit [0047].
- Run keys: `models=dir`; `model=name[:clip]` stands one 3 m ahead of the stand point; `run=models n=` benchmarks n models
  in a fan (clip from `model=`, else `idle`, else the first) [0047].
- **Cap: 1000 animated models in a frame** recommended (0.3 ms frame thread, about 0.3 ms GPU; 5000 is 1.6 ms). Not
  enforced beyond the buffer (65,536 pieces). Posing is per instance on the frame thread (`// shortcut` at MAX_PIECES);
  move to rayon or the GPU when a factory holds thousands of machines [0047].
- Models cast no shadow and are not in the traced scene; a model cell lets the sun through [0047].
- A watcher whose `data/palette.toml` fails to read or parse keeps the last good model of each file still there (a refused
  model file keeps its own). The palette's own changes reload only with the next change in the models folder [0047].
- The game still reads voxel models (they are test data in the Studio: `furnace_voxels.toml`, `snake_voxels.toml` were
  removed with the voxel tools) [0135].

## The game's renderer in the Studio

- **A child process** (option 3 of 0049): the game runs as `run=view`, an off-screen stand run that never ends on its own,
  reads one command per line on stdin and after `frame id` writes a line `frame id w h fov` then w*h*4 bytes of RGBA to
  stdout; the Studio copies it into an egui texture. The client's module is `crates/client/src/view.rs`; hooks in
  `frame.rs`, `app.rs`, `gpu.rs`, `lib.rs` apply only when `run == "view"`. The frame loop sleeps to at most 60 frames a
  second. The interface never waits on the renderer; a crash leaves the Studio on its 2D previews; with no built game the
  view stays 2D [0049].
- **Commands**: `cam ex ey ez tx ty tz`, `time s`, `model name [clip] t` / `model -`, `models dir`, `textures dir`,
  `place di:dj:dl:block,...` (`_` air; an offset beyond 64 cells from the stand cell is refused), `reset`, `size w h` (held
  to the device's largest texture; the Studio clamps 640-1920 x 360-1200), `bare 1|0` (no sky, land or sea: the thing on a
  plain grey, the light unchanged), `zoom k` (the flat view's long lens: the eye 8 times as far, the field of view 8 times
  narrower), `fluff` (above), `frame id` [0049, 0065, 0135].
- **Finding the exe**: the first that exists of `renderer=`, `target/release/technoape.exe`, the Studio's sibling
  `technoape.exe`, `build/TechnoApe.exe`; since 0133 the view picks the newest game exe (an old release build drew
  painted figures black). Started on a thread with `CREATE_NO_WINDOW`, `save=0 record=0 sound=0`, its own `settings.txt`
  under `out/studio-view/` [0049, 0133].
- **`ui::viewport::Viewport`** (shared; a page holds one): `ui(ui, height, fallback)` draws and orbits (drag turns, wheel
  zooms, middle drag pans), `controls(ui)` (scene, time slider, renderer state), `show_model`, `models(gen)`,
  `textures(dir, gen)`, `place`, `ray(pos)`, `to_screen(p)`, `settle(ctx, max)`, `report()`. Scenes: Grass, Cave (dark, beside
  a lamp), None, Checker (the bare frame with ground colour made a checkerboard: `viewport::checker`; a thing pixel in
  exactly the ground's grey turns checkered, a shortcut). Structures and Textures are land-only (`Viewport::land_only`),
  they offer Grass; Fauna's circle pad is land too [0049].
- **The view frame is square metres**: `models::view_axes` is the cell's axes made square (1 m, right angles, up the cell's
  up, floor middle kept); the view's camera (`view::cam_pose`), the shown model and its states' effect points use it, so the
  overlay's box and edges and the click ray agree with the drawn model. Placed block models keep their cell's axes [0065].
- Start to first frame 2.9-3.3 s; a frame's delay 15.6-21.7 ms. The overlay (grid, pivots, selection) can lag the 3D image
  by a frame. Frames go through a pipe (about 1.8 MB at 774 x 588). Shortcut: render only when asked if the Studio's GPU
  use shows beside the game [0049].
- Palette changes show in a model view only once the model is next written [0049].

### The view runs a small planet [0169, 2026-11-19]
The developer: the Studio takes too long to load the game's renderer. Measured: the view (`technoape run=view`, a
child process; the window is never frozen) ran the 4 km planet to show one thing at the spawn: two 20 MB bakes and
the world (2.4 s), then 20,376 chunks to the horizon before its first frame (7.8 s), and a 46.6 s bake when none was
kept. The Studio now starts it with `planet=768 seed=1` (`Viewport::start`): first frame 11.0 s to 2.75 s warm,
about 60 s to 5.0 s cold. Seed 1 because seed 1337's small planet spawns inside a forest.
- The view's clutter models are copied from the game's `data/models` (15 errors a start before, none after).
- Structures keep unsaved work in `studio-recovery/` as Textures, Models, Sounds and Maps do.
- `data/palette.toml` and `data/drafts.txt` are written temp-then-rename (`import::put_file`); the game reloads the
  palette live.
- Open: the background is now a small planet's meadow (the developer's taste; two arguments to revert). Effects,
  Fauna and Plants still lose unsaved work on a crash. A dead renderer leaves the view on its 2D preview until the
  Studio restarts. `Viewport::delays` grows a value a frame. In the game: frames wait for every chunk to the
  horizon (`frame.rs`), and the view's setup runs on the low-priority workers. Full list: the session's report.

## The Models page

- **Tools** (0135 replaces the voxel tools of 0065): Paint (default), Picker, Pivot, Point. Paint works on the model in the
  view (`voxel::pick_paint`: the ray against `Model::box_faces`, the nearest face turned to the ray with a painted pixel
  there, else the nearest face) and on the painting, half the work area beside the view, the hovered pixel lit on both.
  Left the first colour, drag a stroke, right the second, E the eraser. Point adds on the face clicked. One undo holds
  model, life and painting (`MDoc.hist`): Ctrl+Z takes a stroke back. One pixel at a time, no brush size [0135].
- **Inspector**: Paint (tools, eraser, the palette in a box), States, Part, Point, Ready-made motion, Model. New models
  from the starter shapes are painted boxes at once (`voxels_to_boxes`) [0135].
- **Camera** (0065, still in force): a view cube top right (x red, y green and up, z blue; a click on a face looks from
  there) with Flat and Frame; numpad or row 1/3/7 Front/Right/Top, Shift Back/Left/Bottom (Ctrl+number is the page keys), 5
  flat; the six views are inspector buttons; F frames the selection, else the part. Turning goes about the selection, else
  the model's middle (`Orbit::turn_about`); the wheel zooms toward the pointer (`Orbit::zoom_about`); Alt + left drag
  turns. The floor names its edges (front is +z). All behind `Viewport::model`; other pages keep the plain orbit [0065].
- **Flat view** is a long lens, not a true orthographic camera (a true one later if the slight perspective matters for a
  big model) [0065].
- **Clearness**: a bar in the view's top left says the tool, what each button does, how the view turns and what is on; the
  status line reads "x 1 y 1 z 2 of 4 x 3 x 6, part body: dirt" [0065].
- **Starters** (Ctrl+N, New model): block, slab, post, tool, plant cross (removed by 0133), creature (body, head, four legs,
  pivots at neck and hips, facing +z); the page with no model shows the list. Every starter reads back (a test). Making a
  new model is not an undo step [0065, 0133].
- While a state plays, the renderer poses its clip on its own clock and the overlay the page's clip at its time: stop the
  state to edit [0065].
- **Painting a figure**: a model's painting (`<model>.png`) is kept with it (its own undo, a stroke a step; now in the
  model's history, 0135), shown in a Painting section with the open part's nets outlined, and the part's **boxes** are
  listed (corner, size, turn, net). **Add a box** lays a new box's net under the painting in the first colour
  (`voxel::add_box`). Save holds the painting to `check_painting` and refuses to write over a refused `<model>.png`.
  Thumbnails sample the painted faces. Fauna has **New creature** (the open row copied under a new name; its model is
  picked in Body). Shortcut: a model's painting is not in the crash-recovery set; a size drag is a step a frame [0133,
  0135].
- The Fauna page shows the player in the circle (`Viewport::player`, also Models' outline) [0046].

## Plants and Maps pages

- **Plants** (`ui/plants.rs`, new in 0133): the `kind = "plant"` rows and their paintings (base, `.1`-`.4`, looks, `.back`,
  `.far`). Left paints the painting at its pixels, right a stand of 3 x 3 drawn from the game's own `plant_faces`; planes
  2-4, size (kept at its foot), "+ alternate", **New plant** (a tuft and a row). Save holds the row to
  `engine::clutter::parse` and each painting to `engine::models::painting` and the 64 x 80 px rule before writing any; the
  row is written after the paintings. A new or changed row shows in the game after a rebuild (`clutter.toml` is built in).
  The stand is drawn by the Studio (back to front by plane) without climate tint, wind or light [0133].
- **Maps file** (`data/maps/<name>.toml`, one file a map): top `size` (16, 32, 64, 128, 256, 512), `frames` (1 to 64),
  `seconds` (0.1 to 60). Each `[[node]]`: `id`, `kind`, `at = [x, y]`, its numbers by name, its words (`mode`, `shape`,
  `ramp`) and its input pins by name. A pin holds another node's id (a wire) or a number in 0..1 (the slider it shows
  unplugged; a direction pin has none; the number stays in the file). Colours names a palette `ramp` or
  `colours = [names]`. The Studio writes every number. `check=1` refuses with file and node named: an unknown key, kind or
  word, a number out of range or not whole where it must be, a wire to no node, a circle, a pin fed a kind it cannot
  take, no `out` or two. Kinds and numbers are `SPECS` in `crates/studio/src/map.rs` [0153].
- **Kinds of data**: a picture holds three numbers a pixel: a number (v, v, v), a colour (r, g, b), a direction (x, y, 0,
  y down the picture), each part -1..1. A number plugs into a colour pin as grey, a colour into a number pin as its
  brightness. Only Slope and Curl make a direction. Levels, Steps, Invert take numbers only; Blur, Move, Warp, Blend take
  any kind and give back the kind they were fed [0153].
- **Tiling and looping are exact by construction** (periodic lattices, no blending): every generator lies on a lattice of
  whole `scale` cells a side whose corners wrap by `scale` (octave o has `scale` x 2^o); a straight gradient is a triangle
  wave; Blur, Warp, Move sample with wrap. `scale` and `speed` are whole numbers. Time t = frame / frames, frame `frames`
  is frame 0 again; noise has time as a third lattice axis of 2 x `speed` x 2^o cells a loop; Cells circle their points
  `speed` times a loop, Speckle draws `speed` new patterns a loop, Move scrolls `speed_x`, `speed_y` whole sides a loop.
  Mirror in Move does not flip a direction field's sign [0153].
- **Evaluation** is CPU, node by node in wire order; each node keeps its pictures by (frame, size), up to eight, until a
  number at or before it changes (`Graph::set`). `Graph::pic(node, frame, size)` takes any size. Blur's radius scales with
  the size. A direction is scaled as a whole so its strongest push is 1, never clamped by part [0153].
- `technoape-studio map=<name> out=x.png [frame=k]` writes the out at the map's size (an animated map without `frame` as a
  strip) and prints one JSON line: size, frames, kind, milliseconds, an FNV-1a hash of the pixels. A direction is written
  in red and green, 128 as no push [0153].
- **The page**: our own graph on `egui::Scene` (no node-graph crate): pan middle button, zoom to the pointer 0.2 to 2, F
  frames all (nodes fill about 80 %, zoom at most 1.5, refitted until moved by hand). Every edit goes through `Map::add`,
  `wire`, `unplug`, `remove`, `copy`, `set_num`, `set_word`; a refused wire says why while dragged; the map is never left
  invalid (`repair`). Pins by kind: number grey, colour yellow, direction purple, 12 points; a wire takes the colour of the
  pin it leaves. Group bands: Make green, Change blue-grey, Combine brown, Turn into plum, Out grey. Numbers are rows on
  the node (a drag is relative; a seed one step every 4 points; one undo step a drag; double click types; rows' text hides
  under zoom 0.45); choices are drop-downs. Space adds a node [0153].
- **Speed**: node pictures 64 px worked out in the frame (`Graph::update`), a pass starts no new node picture after 4 ms;
  the full-size preview and the loop are worked out on one worker at a time, stale results dropped (`motion::Strip`, the
  previewed node's loop at the map's size up to 256) [0153].
- **Motion**: Play (button, K) plays at frames / seconds. **Particles**: 300 dots, each living 2 to 4 s, trails of 10 steps,
  push is a direction, or a number/brightness curled (central differences, wrapped) and scaled so the strongest push is 1;
  `speed` is picture sides a second. **Sway**: 26 blades and one taller plant, reading the map's middle row scrolling at a
  tenth of a side a second; bend is a direction's x part, or a number around its middle, times `strength`, offset growing
  with height squared [0153].
- **Use as a block's texture** (inspector): block, face (all, top, side, bottom), 1 to 4 variants (each with every seed moved
  on by 7919 x the variant), worked out at the block face's side (else another face's, else 16). A colour snaps to the
  nearest palette colour; a number runs along a ramp; a direction is refused with the reason. Use puts the picture into the
  Textures page's document as one undo step "Map <name>" and saves through its save. Nothing is written before Use [0153].
- **Export**: a path box, `exports/<name>.png` to start (`/exports/` is git-ignored); number grey, colour as is, direction red
  and green with 128 as no push, alpha 255; an animated map is one strip at its size. The button says Replace when the file
  is there [0153].
- **New map** (File, Ctrl+N): seven starters in code (`ui/maps/make.rs`: clouds, stone cells, wood grain, wind gusts, smoke
  swirl, ripples, flicker); a map is a file, and a draft, only after Save. **Variations** (M): the shared wall with the
  open map and nine others, each with every seed moved on by a random amount [0153].
- **The page bar** hides the Studio's title when the tabs would not fit in one row; tabs fit their words (64 px at least)
  [0153, 0133]. (Game reads maps: ADR 0157, not this topic.)

## Painted block textures (the generator)

- **`tools/paint_textures.py`** (`python -I tools/paint_textures.py [block ...] [--preview out/x.png]`) paints from palette
  names, deterministic from the block's name; a style change is one edit and a run; the result is plain PNGs the
  developer can repaint. Every written file goes on `data/drafts.txt` (and `data/palette.toml` when colours are added);
  `gravel.png` and water are never written. A block with no recipe has none and is named on stderr. The script does not
  check that a file has since been hand-painted: take it off `data/drafts.txt` and out of the script's reach first
  [0084].
- **Ramps of five steps by key**: `d` dark, `s` shade, `m` mid, `l` lit, `L` light. A palette ramp with `X dark`, `X`,
  `X light` gains `X shade` (55 % of the way from dark to mid) and `X lit` (a third from mid to light) when a texture asks.
  A block with no ramp gets one from its `blocks.toml` colour (darker steps cool, lighter warm); a season look from that
  season's row. Only colours a texture uses are added; colours already in the palette before a run are kept
  (`Palette.before`); the script asserts every ramp it grows runs dark to light and uses the Studio's `luma` weights.
  Grass's new steps sit in a ramp of their own (`grass steps`) because the engine test `the_data_file_loads` holds the
  `grass` ramp to three colours [0084].
- **Most texels `s`, `m`, `l`; `d` draws the shapes** (cracks, joints, laminae, rims), `L` the glints; `levels` sets exact
  shares [0084].
- **Shapes per material** (16 px, `<block>.png`, sides in `_side`): grass of every biome and season is fine two-tone ground
  with short dark blades and a few lit tips, its side dirt with tufts of 2 to 5 rows hanging over the top edge (top row
  mid with a few lit tips); each rock has its own recipe (stone soft beds and a crack, granite and gabbro two-texel
  speckle, gneiss broad bands, schist fine bands with mica, sandstone fine beds, shale laminae, limestone thick beds with
  a shell on top, slate cleavage partings, marble a grey vein, quartzite healed cracks, tuff clasts, andesite phenocrysts,
  basalt columns); each ore is visible on its host (chalcopyrite on andesite, cassiterite on granite, galena on limestone,
  pitchblende on granite, malachite on stone, gold in white quartz, coal, hematite banded, bauxite nodules, bog iron in mud,
  gossan); the three gravels are the developer's gravel mix with copper, tin or gold grains; log has rings with pith and
  bark plates; reed blades; lamps a two-texel frame round an even glow, four variants (joined by ADR 0085); copper a
  bevelled plate; sand fine grains with ripples; snow and ice near flat; clay and mud smooth; Luna's dusts with small rocks,
  anorthosite feldspar laths, iron-nickel Widmanstatten bands [0084].
- **Tiling**: all noise periodic on 16 texels; strata, bark plates and grass overhang use the block's seed, not the
  variant's, so they run on across neighbours; a vein or crack crossing an edge runs corner to corner; features no larger
  than four texels [0084].
- **Leaves** (cutouts, four variants each): two close tones in soft clumps (cell 4), the lit one on 15 %, the texel under a
  hole in shade, no dark rims; a cube of leaves is at most 15 % open (the script asserts it); autumn broadleaf is the
  orange with a shade and a lit step of its own. Species by shape first, colour second: broadleaf soft round clumps, oak
  big lobed clumps with notches, birch small light open leaves, willow a hanging curtain of strands, spruce drooping tiers,
  pine needle rosettes over a dark ground, jungle broad ovals with a midrib, acacia flat layers of leaflets, palm a slanted
  rib with leaflets, bamboo lance leaves. Colours a step of hue apart around the mid green the climate tint multiplies
  [0084].
- **Bark** (`<log>_side`, three variants, ends in `<log>`): birch white with dark lenticel dashes, oak grey ridges broken
  across, pine big plates (spruce small plates), willow diamonds, jungle pale with moss, acacia grey with orange
  under-bark, palm rings of leaf scars, bamboo a jointed culm with a hollow-ring end. Every bark is squeezed into a narrow
  band of its ramp (`calm`: ridged and plates 1 to 4 or 1 to 5; willow, smooth and palm 2 to 5); birch lenticels dark
  grey and few; bamboo nodes a single step [0084].
- **Seasons**: deciduous species (broadleaf, oak, birch, willow) take spring, autumn and bare winter looks; blossom is the
  spring look (broadleaf pink-white, birch and willow catkins); fruit is the late-summer look `@fruit` (broadleaf apples,
  oak acorns, jungle fruit, palm coconuts, ripe berries), which the script adds once the engine's season list names it.
  Spruce, jungle and acacia have no winter look and stay green. Winter looks are four roles in file order, each with its
  wood at the painting's **bottom edge**: `root` (one stem from the middle that fans out), `root off-centre`, `through` (a
  limb bottom to top at x 8, 16 at 32 px, with side twigs), `tips` (a few fine twigs). Limbs taper from three texels to
  one, bark-mid with a dark side, twig tips lit; the script asserts every texel reaches the bottom edge through its
  neighbours and a limb stops where it leaves the painting. ADR 0090 picks the role by distance from the wood and turns the
  face so its bottom meets the wood; `reed@winter` has three variants so the four-variant role pick leaves it alone. Winter
  twigs share the species' log-side band (tones 1 to 4) [0084, 0090].
- **32 px a metre flora** (`data/textures/hd/<block>.png`, variants side by side, 32 x 32 a face, same names and season
  tags; the 16 px files stay; the Studio's `check=1` reads only the top folder): colour from **rich ramps** (`<name> rich` in
  the palette, `r1`..`r8`, eight tones, lights toward warm yellow-green, shadows toward cool blue-green; a rich ramp the
  script changes is written over in the palette). Leaves are single leaf shapes stamped in clumps back to front, each lit as
  a ball from the top left over a dark accent in the gaps, drawn until about 92 % of the mass is covered; only three leaves
  in ten get a random step of tone. Clusters must read from 10 to 20 m: fewer, larger clumps for birch, acacia, bamboo;
  fewer, longer rosettes for pine. Non-flora blocks stay calm at 16 px [0084].
- **Far block paintings**: `data/textures/hd/far/<block>[@season].png`, 8 px a face, for every leaves block, the shrub, reed
  and cactus (each 4 x 4 cell takes its most common colour where a quarter is filled). New 32 px paintings: `reed` and its
  seasons (a stand a block high, never under half the block), `shrub@spring`, `shrub@autumn`, `cactus` (eight ribs meeting
  at a crown) and `cactus_side` (four ribs a face, lit left) [0084].
- **Reeds**: four kinds (`REED_KINDS`: tall with cattails, tall without, short leaning, sparse with a broken stalk); the reed
  block's four variants are the four kinds, `reeds.back.png` is the tall kind two tones darker (`darker`) for a bed's back
  row. Ferns each have their own fronds, spans and lean; short and tall grass get a fourth outline (`.3`) [0084].
- **Shrub** in the broadleaf family: summer hue 92 to 102, spring 84 to 94; means of opaque pixels (70,136,21) and
  (89,148,25) against broadleaf (69,132,19) and (98,145,39). A painted block is drawn as painted with no tint [0084].
- **Small plants as planes** (ADR 0091's format: one upright plane a file in `data/plants/<name>.png`), one pixel is 1/16 m
  at 16 px, 32 px a metre now; width x height in `PLANT_SIZES`: short grass 32 x 13, tall grass 32 x 32, wild wheat
  32 x 40, fern 48 x 22, reeds 32 x 64, flowers 32 x 16, berry bush 40 x 40. Each plant has `.1` and `.2` besides itself;
  the berry bush adds `.bloom` and `.fruit`. A tuft fills its width edge to edge (wrapping at the sides). The game tints
  only green pixels (g above r and b), so straw, petals, cattails and fruit keep their colour [0084].
- **Painted for the plants' shader**: the meadow's greens are a fixed ramp `MEADOW` in `tools/paint_textures.py`, a deep
  saturated green, yellow only in the two lightest tones; blades stay mid green and are lit only at the tip (tone follows
  height^1.7), variants turn the hue by -7 and +6 degrees. The shader multiplies green by about (0.75, 0.86, 0.5) in a
  middling climate and darkens every plant to x0.45 at the root, so a blade's root is only two tones down (tone = 2 + t^1.7
  x 5) and the fern's rib and stems start at tone 2. No dark outlines (the darkest tones lifted to mid where mass meets
  air); dark at the root, bright at the tips [0084].
- **Outline, depth, distance for every mass**: a tuft's blade heights follow its own outline (`envelope`: two or three bulges
  with dents, falling to the sides, each variant its own); roots in the middle 80 % with outer blades leaning out; a back
  layer darker, a lighter front layer over it; bushes two or three lobes, the lit upper left of each clump, up to two tones
  lost in the lower third. `data/plants/<name>.far.png` at 8 px a metre per mass plant kind (not read by the game when 0084
  was written; far plants used the near painting); `data/plants/meadow ground.png` three 32 px tiles of mottled grass top.
  Single flowers, mushrooms, sticks, stones: rich but simple, no far version [0084].
- **Clutter models** (ADR 0082): painted by `tools/paint_clutter.py`, which imports `MEADOW`, `rich_rgb` and the palette from
  `paint_textures.py` and adds its twelve ramps; stones, sticks and flint have the top lit and the sides shaded from the
  ground up with soft noise, glints and pits; the mushroom a red cap with pale spots and gills. Model boxes unchanged,
  only paintings [0084].
- **Mining paintings** at 16 px in the calm style, made by `paint_textures.py` always with a want list of block names (a full
  run would repaint the flora): the six halo blocks (iron-stained rock, copper-stained rock, quartz-veined rock, greisen,
  bleached rock, dolomite; one block is the outer grade for one ore and the inner for another, every inner block more
  coloured or veined than its outer one, ADR 0095); **rubble** one painting for every row with `rubble =`
  (26 rocks and hard ores, `<rock> rubble.png`: six to eight broken pieces each a shifted cut of the rock, one in three a
  step darker, near-black gaps, a rim lit up-left and shaded down-right stepping only along the rock's common colours, a
  rock with mean luma under 51 taking its lightest colour for every rim); **flint** is the clutter model's painting (top
  dark glassy blue-black flaked, a pale chalky cortex rind on one edge, sides cortex above glassy core; ramp `flint
  cortex`). The item icon of flint cannot be painted yet (an item's `look` is a block cube) [0084, 0093, 0095, 0097].
- **Open calls** the developer confirms: the style's shares (mostly mid, a quarter shade, a quarter lit) and shape strength;
  the palette's size (it grew from 109 to 349 for the calm set and carries about 1,150 colours now, from the rich ramps
  of flora and figures); whether biome grasses share ramps; hosts for ores with none in `minerals.toml` (chalcopyrite on
  andesite, malachite on stone, gold quartz as a white quartz vein, bog iron in mud); bare crowns under fast leaves being
  dark cubes (engine work: see ADR 0088 for what was done) [0084].

## Painted figures

- `tools/paint_figures.py`, beside `tools/paint_clutter.py` (its `Model`, ramps, palette), paints the ape (23 boxes), boar
  (13), deer (11), hare (9), snake (5), furnace (7), leaf (1 plane) and, with ADR 0117, the torch (2). A recipe per figure
  names boxes on the parts and a paint function per face pixel. The output is a draft (`data/drafts.txt`), made again by one
  command [0116].
- **Rig kept**: every part keeps its name, parent, pivot and `at`; its voxels are emptied to a `.` grid of its own size and
  its boxes follow, so `creatures.rs`'s walk (by part name), clips, points and states are unchanged [0116].
- **A painted box** is a box in a part's voxels whose faces are pixels of the model's `.png`, 4 px a voxel, drawn a quad a
  pixel on the instance path (ADR 0081, 0114). A box side over 63 px is not drawn (the quad's 6-bit size): the script
  refuses one and recipes split (the furnace's body in two) [0116].
- Budget: 24,328 quads against `MAX_MQUADS` 65,536, with 30 % kept free (a test). The ape's skin ramp is
  `rich_rgb(26, 0.6, 0.12)` [0116].
- Client: a painted figure's inventory picture is its painting; the bare arm draws `arm_r`'s boxes and those of parts hung
  from it; `painted_item()` (one part with boxes) decides the hold and the lying pose [0116].
- Cost: 200 deer or apes GPU p50 +0.5-0.6 ms against voxel models (row in `docs/perf-debt.md`) [0116].
- Shortcut: a pixel a quad (the furnace is 6,912 quads). The look of each figure is the developer's to confirm [0116].

## No longer holds

- 0044's "still to decide" list (texture array, shared renderer code, interface library, file layout, model drawing): decided
  by 0045, 0046, 0047, 0049.
- 0045's texture array option and its "palette check not in this rule yet": the storage buffer was chosen; `check=1` has
  the palette check (0046).
- 0045's mirrored sides left as a call: fixed by 0047 (`texel` turns x on -x and +y sides).
- 0046's game font as the interface font and its one-size sharp-text rules as the default: replaced by 0155 (Clean font;
  Game is a setting). Its "Added" mention of two global hide flags (never released): replaced by per-page `hide=`/`width.`.
- 0046's "Left out: docking, saved layout per page": layout per page was built the same day (0046 third addition).
- 0064's "textures use recovery; Models and Sounds can later": built by 0080.
- 0065's voxel tools (Add, boxes, lines, layer mode, mirror, edges, selection, Shade by light, Carve, "Voxels to painted
  boxes", the colour count) and the plant-cross starter: removed by 0135 and 0133. The voxel pick edges, ghost voxel and
  layer-plane placement no longer exist as tools.
- 0080's "Textures' and Models' search/Undo shared one": each page has its own (stated in 0080 itself).
- 0084's 16 px leaf and bark files and its "far paintings not read yet": 16 px `hd` shadowed files deleted by 0133; the
  `.far` files may since have been read (checked outside this topic: not looked at). Its "Not here: the crack looks of ADR
  0094" belongs to mining.
- 0084's 349 colours (then 430, 526, 565): the palette grew (about 1,150 `[[colour]]` entries in `data/palette.toml`).
- 0116's "proposed, waits for developer": built and in use (0133, 0135 rely on it).
