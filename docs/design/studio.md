# TechnoApe Studio: our own tool for the game's looks

- **Status:** design, 2026-11-11 (the developer: "our own little voxel blender", so the game's looks are
  made by hand and not by AI; the name is the developer's). Steps 1 to 3 are being built. The coding
  agents spike each step, measure it and write the ADRs that follow ADR 0044.
- The developer's calls (2026-11-11): block textures and voxel models first, particles and trees and
  structures later; 12.5 cm model voxels (first 1/16 m, changed the same evening); models have simple moving parts from the first version; Claude may make
  drafts, the developer redoes them.

## The rule it serves
Everything a player sees up close is made by the developer's hand. Claude builds the tool and may fill a gap
with a draft; a draft is listed as one until the developer has saved their own over it.

## What it is
A second program in the workspace (`technoape-studio`, handed over as `build/TechnoApeStudio.exe`) that
reads and writes the game's own data files. There is no export step and no second format: the file the
Studio saves is the file the game loads.

- **It draws with the game's renderer**, sun, block light and post-processing included, so a texture or
  model looks in the Studio as it will in the world. How the code is shared (a library crate out of
  `crates/client`, or a second binary in it) is the coding session's call.
- **A running game picks up a saved file** within a second (it watches `data/`), so the developer paints with
  the game open beside it.
- **Any paint program still works** for textures: they are plain PNG files, held to the palette below.
- **Claude can use it without eyes** (working principle 10): `technoape-studio render=<file> sheet=out/x.png`
  writes a turntable filmstrip, `check=1` validates every art file and prints one JSON line, `drafts=1` lists
  what is still Claude's.

## Textures (first)
Today a block's face is a recipe in the shader (ADR 0039: a colour and a pattern). A painted texture replaces
it the way a recording replaces a code-made sound (ADR 0032):

- `data/textures/<block>.png`, 16 x 16, is the block's every face. `<block>_top.png`, `_side.png` and
  `_bottom.png` override single faces. A block with no file keeps its pattern, so nothing has to be painted
  before the rest works.
- **Variants:** a file 32, 48 or 64 px wide holds 2 to 4 variants side by side; each block in the world picks
  one by its position's hash, so a field of grass does not repeat.
- The exact rule: 8-bit RGBA, 16 px high, 16, 32, 48 or 64 px wide, every pixel opaque. A face's file wins
  over `<block>.png`; `_side` is the four sides. A file that breaks the rule is refused by name and the
  block keeps its pattern.
- Far terrain, the bounce light and the hotbar need one colour per block: for a painted block the game
  takes the mean of its texture when it loads it (top face for the top, else the side), so `blocks.toml` is
  not rewritten and `colour` stays the look of the unpainted block.
- In the game the textures are layers in one small storage buffer, read once per fragment (ADR 0045: a
  texture array cost a 256 MB memory block on `fly`, the buffer is 2 MB). A painted face is drawn exactly as
  painted, with none of the patterns' grain.
- Open, found by the review: on two of the four sides a painting shows mirrored, as the patterns always
  have. It must be righted before anything that is not symmetric is painted.

The texture page: a 16 x 16 canvas with pencil, fill, picker, line, mirror and undo; the block shown
tiled 3 x 3 flat and as a lit cube beside its neighbours; the faces and variants as tabs; "start from
pattern" copies the block's current shader pattern into the canvas so the first stroke is not on an empty
square. There is no eraser: a texture has no empty pixels.

### The window's layout
One window, three columns under a bar of pages (Textures, Palette; later Models, Effects, Structures).

- **Left, narrow: the files.** Every block of `blocks.toml` by name with a small picture of its face, a
  search box on top. A painted block shows its texture, an unpainted one its `colour` greyed; a draft carries
  a mark. A click opens the block.
- **Middle, the largest: the canvas.** The 16 x 16 face as large as fits in whole multiples, with a pixel
  grid from 8x up. Above it two rows of tabs: the face (All, Top, Side, Bottom; a face with no file of its
  own is dimmed and shows what it falls back to) and the variant (1 to 4, and "+"). Under it the pixel under
  the pointer and its colour's name.
- **Right: tools on top, palette below, preview at the bottom.** Tools are a row of buttons with their key
  shown. The palette is a grid of swatches, the chosen one outlined, its name under the grid. The preview is
  the face tiled 3 x 3 (variants mixed as the game will mix them); the lit cube joins it when the Studio
  draws with the game's renderer.
- **Keys:** B pencil, G fill, I picker (also Alt held), L line, M mirror on and off, 1 to 4 the variant,
  Ctrl+Z and Ctrl+Y undo and redo, Ctrl+S save, the wheel zooms, the middle button drags the canvas. Right
  click paints the second colour (chosen by right click on a swatch).
- **Saving** is explicit (Ctrl+S); an unsaved file shows a dot by its name, and closing with unsaved work
  asks first. Undo is per file and survives a save.
- **The Palette page:** the swatches large, in rows the developer orders by dragging; a click selects, and
  the right column shows the name, the colour as hex and three sliders (hue, saturation, value), and how many
  files use it. "Add" makes a new colour; "Remove" is greyed while the colour is in use.

## Models (second)
A model is for everything that is not a whole 1 m cube: the tool in the hand, a dropped item, a plant, a
torch, a machine, later creatures and players. Its voxels are 12.5 cm, the size of the fine cells (ADR 0033), two texture pixels wide,
so a model is made of the same pieces the world will break into and its edges fall on the pixel grid.

- `data/models/<name>.toml`: the game palette's colours it uses, by name, and **parts**. A part has a name, a parent, a
  pivot, and a box of voxels up to 32 on a side (4 m), written as layers of palette letters. It is text so
  git shows what changed and Claude can write a draft.
- **Clips:** a model has named clips (`idle`, `use`, `walk`); a clip has keyframes that move and turn whole
  parts about their pivots, a length, an easing and whether it loops. Parts are rigid: no bending, no bones
  inside a part. The curves are the effect system's (ADR 0031) where they fit.
- In the game a model is meshed once into the block's vertex format and drawn per instance with one
  transform per part. It takes the same light as the blocks around it. The game has no model drawing yet;
  that is the larger half of this work, and its budget is measured against ADR 0004 with a benchmark of many
  models on screen.
- A row in `blocks.toml` or the items file names its model; the first use is the item in the hand.

The model page: place, remove and paint voxels, a box tool, mirror, undo; a slice view for the inside; the
parts as a tree with their pivots; a timeline with keyframes and play; a 1 m block and a player's outline
beside the model for scale.

## One fixed palette
The developer's call (2026-11-11): everything is painted from one set of game colours.

- `data/palette.toml` holds them, each with a name: rows `[[colour]]` with `name = "moss"` and
  `rgb = "#4d9433"` (sRGB, the bytes a PNG holds, so a pixel matches exactly or not at all). Names are
  unique, lower case; the order in the file is the order on the page. The canvases offer only these; there
  is no colour mixer on a texture or a model.
- The palette has its own page, where the developer adds, names and changes colours. Changing a colour
  repaints every texture that uses it (models follow by themselves: they store the colour's place in the
  palette, not the colour). A colour still in use cannot be removed.
- `check=1` refuses a texture with a pixel that is not in the palette and names the file and the pixel, so a
  PNG from another paint program is held to the same rule.
- How many colours is the developer's to grow; a model file's letters allow 255.

## Drafts
`data/drafts.txt` lists the art files Claude made. The Studio marks them in its file list and takes a file
off the list when the developer saves it. A release check can refuse to ship while the list is not empty.

## Later pages
- **Effects:** edit the rows of `data/effects.toml` with a live preview (ADR 0031).
- **Trees and structures:** build from whole world blocks; world generation places them.
  Built (ADR 0052) as one Structures page for hand-made and grown files; world generation does not place them yet.

Both wait until textures and models are in use. The Studio's shell (file list, preview, undo, save, the
watch on `data/`) is built once in steps 3 and 5 and these pages reuse it.

## Usability round (2026-11-12)
The developer's first hour in the Studio, in their words: "it's not clear how the variants work if at all";
"have a visible block with the texture you're painting"; "moving the camera around, placing voxels on the
right place and clearness what you are doing and where is difficult inside the models screen"; sounds must
"work with outside files"; and "make using and generating patterns very clear and easy since then you can
mostly use patterns, tweak some values and colours and get a nice texture".

One rule for the whole round: **nothing replaces the developer's work without showing it first.** A pattern,
a generated set or an imported sound appears as a pending change in the canvas and every preview, with
**Use** and **Cancel**; Use is one undo step. Each step below is built and checked alone, in this order.

**The bar** (the developer, the same day: "so robust and easy to use every person can make nice models and
textures in minutes"). Two tests decide whether a step is done, beside its own check:

- **Five minutes, no manual.** Someone who has never opened the Studio makes a texture they would keep
  (pattern, tune, touch up, save) and a small model (a starter shape, recolour, add and remove, save) in five
  minutes each. Every page therefore says in its empty state what the first click is, every control has a
  plain word and a tip, and no step needs a key that is not also a button.
- **It never loses work and never dies on a file.** A bad or half-written file is named and skipped, never a
  crash at start (the 2026-11-12 build did not open because `blocks.toml` had a row it did not know: an
  unknown field must be a warning in the Problems list). Unsaved work is kept in a recovery file every few
  seconds and offered back after a crash. Everything is undoable, including Use, import and remove.

For models the pattern panel's twin is **starter shapes**: a new model begins from a list (block, slab, post,
tool handle with a head, plant cross, creature of body, head and four legs with its parts and pivots set)
that is recoloured and reshaped, never from an empty box.

### 1. The block is always in view (Textures)
Today the lit block exists ("In the game", `ui/textures.rs` `lit_cube`) but sits at the bottom of the right
column, under Tools, Start from, Colours and Preview: at 1080p it is off the screen, so the developer never
saw it.

- The preview is the **top of the right column and does not scroll**: one box of about 220 px with the chips
  `Block | 3 x 3 | 5 x 5 | Unfolded`, Block chosen at the start. Tools, Start from and Colours scroll under it.
- Block is the game's renderer as now (the block on a floor of itself, drag turns, wheel zooms); a double
  click puts the camera back.
- It follows the stroke while the button is down if that stays inside the frame budget, else at the stroke's
  end as now: measure and say which.
- A pending pattern or variant set shows here too.
- The tiled preview mixes variants with the game's own hash (`shader.wgsl` `paint_texel`), not the Studio's
  (`art.rs`, marked `shortcut:`), so the field in the Studio is the field in the world. A test holds the two
  to the same pick for a list of cells.

Done when `shot=out/x.png open=grass` at 1920 x 1080 shows the lit block without scrolling.

**Built** (ADR 0064): the box on top of the inspector; while a stroke is held the block follows it (the open
file written every 120 ms, about 1.5 ms a write); the variant pick is the shader's, held by a test.

### 2. Patterns are the main way to start a texture (Textures)
Today "Pattern" stamps the block's row from `blocks.toml` into the canvas: no choice, no values, no preview.
The eleven recipes and their values exist (`engine/src/blocks.rs` `Pattern`, `studio/src/pattern.rs`).

"Start from, Pattern" opens a **pattern panel** in the middle column, where the variants wall opens now:

- **A row of the eleven patterns as thumbnails**, each drawn with the colours and values chosen so far, its
  name under it. A click picks one. The block's own row is the starting choice.
- **Three colour wells** (base, second, third). A click on a well, then on a palette swatch, sets it. Second
  and third start as "auto": one step darker and lighter on the base colour's ramp, and say so.
- **Sliders for size, amount and contrast**, only those the chosen pattern reads (plain, grass and stone take
  neither size nor amount), each with its number and a reset to the pattern's default.
- **Another**: a new seed for the same values, where the recipe has chance in it.
- **Live:** the canvas, the tiled preview and the block redraw while a slider moves.
- **Fills:** this variant (the default) or every variant, each from its own seed. That is the short road to a
  varied field: one pattern, four seeds.
- **Use** and **Cancel**. The panel opens again with the file's last values within a session.
- One line on top says what it is: "A pattern draws the face from three colours and a few numbers. Pick one,
  tune it, then paint over it."

The result is snapped to the palette as now and saved as the PNG; `blocks.toml` is not written (decided,
see the answers at the end). Done when a `drive` script picks bricks, sets three colours and a size, uses it, and the saved PNG passes
`check=1`; a `shot=` of the open panel is read.

**Built** (ADR 0064): the panel stands left of the canvas; while a well waits for a colour the swatches show
in the panel too. The drive script is the test `the_pattern_panel_picks_bricks_three_colours_and_a_size`.

### 3. Variants explain themselves (Textures)
Today: tabs `1 2 3 +` with no word beside them; "Make variants" opens nine sets, each three small squares, the
first of which is variant 1 unchanged (flat green in the developer's screenshot, so every set looked broken);
"spread", "Others", "Back", "first" and "more like this" are not explained; a click on a set replaces the file.

- The tabs get the label **Variant** and a tip: "The game gives each block one of these by its place, so a
  field does not repeat."
- **Generate variants** (the new name). Each set on the wall is drawn **as ground: a 4 x 4 patch with its
  variants mixed as the game mixes them**, the single variants small under it. That is the thing being chosen.
- Words: "How different" (was spread, from slight to strong), "New batch" (was Others), "More like this" on
  the hovered set, "Back" stays. The line under the title: "Each set keeps your variant 1 and changes a share
  of its pixels for the others."
- A click on a set makes it pending (canvas, previews, block); **Use this** or **Cancel**.
- When variant 1 is one flat colour there is nothing to vary: the wall says so and offers "Start from a
  pattern" (step 2, every variant) instead of nine near-empty sets.
- A variant's tab has a small x to remove it (asks when it holds paint), if there is no way to today.

Done when a `drive` script generates, picks and uses a set and the file is 48 px wide; the flat case is a
`shot=` that is read.

**Built** (ADR 0064): the wall stands left of the canvas; the set made around the file is marked "as now". The
drive script is the test `generate_variants_picks_a_set_and_a_variant_is_removed`. The bar's recovery file is
built for textures (`recover.rs`, kept every 3 s in `studio-recovery/`), ready for the other pages.

### 4. The model view: where am I, where does the click land (Models)
Today: right drag turns, middle drag pans, the wheel zooms; left adds on the hovered face and right removes; a
white hover cube, a grid of 1 m lines, the bounds box. No axes, no view buttons, no word in the view about
what a click does.

**Camera**
- A **view cube** in the view's top right corner with the axes in colour (x red, y green and up, z blue). A
  click on a face looks from there: Front, Back, Left, Right, Top, Bottom; numpad 1, 3, 7 do the same and 5
  switches between perspective and a flat (orthographic) view.
- **F frames** the selection, else the part. Turning goes about the selection, else the model's middle, never
  about a point off in space. The wheel zooms toward the pointer.
- Alt + left drag also turns, for a hand without a middle button.
- The floor names its edges ("front" where the model faces in the hand and in the world).

**Placing**
- A **ghost voxel in the chosen colour**, half see-through, where a left click will add; the voxel a right
  click will remove is outlined red; the hovered face is lit. The white cube goes.
- The floor grid shows the **12.5 cm voxel lines** under the part, the 1 m lines heavier. A click on an empty
  floor cell places the first voxel there.
- **Layer mode** (the "slice view" this document promised): a plane at one height, PgUp and PgDn move it;
  clicks land on the plane, voxels above it are dimmed. For building floor by floor and for the inside.
- **Edges** (a toggle, on at the start): thin dark lines between voxels, so a face of one colour still reads
  as voxels.
- Mirror shows its plane while it is on.

**Clearness**
- A small bar in the view's top left, always: the tool and what the buttons do, such as "Add: left adds,
  right removes. Right drag turns."
- The status line reads the cell: "x 3 y 5 z 2 of 16 x 16 x 16, part head".

Done when a `drive` script sets each of the six views and frames a part, and a filmstrip of hover, add and
remove in layer mode is read; picking on the layer plane has a test.

**Built** (2026-11-12, ADR 0065), with the starter shapes of "The bar": Ctrl+N and New model show the shapes
(the page with no model shows them too, with what the first click does). The far side's views are Shift+1, 3,
7 (Ctrl+number is the page keys). Page Up/Down move the layer; the selection moves up and down with Shift. The
flat view is the eye 8 times as far and the renderer's field of view 8 times narrower (its `zoom` command),
not a true orthographic camera.

### 5. Sounds from outside files (Sounds)
**Built 2026-11-12** (ADR 0066: symphonia reads the four formats in the Studio only; the written file is 48 kHz
mono 16-bit WAV; `crates/studio/src/import.rs`, the panel in `ui/sounds.rs`).

Today a recording `data/sounds/<name>.wav` replaces a code-made sound and `data/sounds/base/*.wav` are bases
for chains (ADR 0032, 0048), but only as WAV, only by copying files by hand, and nothing trims or levels them.
Making a sound from layers and chains alone asks for audio skill the developer does not have; a good recording
from the net is the better start. This lifts "import of other formats" from the left-out list for sounds.

- **Import:** drop a file on the Sounds page or give its path. WAV, OGG, MP3 and FLAC are read. The decoder is
  a crate in the Studio only; the Studio writes 48 kHz mono 16-bit WAV, so the game still reads WAV alone.
  The crate and the written format get their ADR from the coding session.
- **The import panel:** the waveform, play, two handles to cut the start and the end, "Trim silence" (on),
  a short fade at both ends (5 ms, against clicks), "Level" to the row's peak (on), and the length in ms.
- **Then one of two:** "Use as this sound" writes `data/sounds/<row>.wav`, or the next take `<row>.N.wav`
  (up to 8) when the row has one; "Keep as a base" writes `data/sounds/base/<name>.wav`, and the row's `from`
  and `chain` then make many sounds out of one download (pitch, spread, trim). A running game picks either up
  within a second, as now.
- **Credits:** the panel asks where the file came from (a URL) and its licence (own, CC0, CC BY, other), kept
  in `data/sounds/credits.toml`. `check=1` refuses a recording with no row there. A CC BY row is what a
  credits screen must list before any public release (see the answers at the end).
- A warning, not a refusal, for a one-shot over 10 s.

Done when: a test decodes one small file of each format and gets the same length within a frame; a `drive`
script imports, trims and uses one and `sounds=1` shows the row playing a recording; `check=1` fails on a
recording without credits.

## Second round (2026-11-12, after the first play-test of the usability round)
The developer, with the merged Studio open: "the blue box indicating parts is not aligned with the model";
"for making small models like saplings, leaves, pebbles it's too limiting with the bigger voxels or not being
able to put a pattern on a voxel"; "colours for textures feel limited". Two calls made the same evening:
**fine voxels per model** (not pattern materials on voxels) and **the one palette stays and grows in place**
(not a free colour picker). Not built yet; steps 7 and 8 continue the usability round's numbering.

### 6. The part box and the edge lines sit on the model (bug)
On `snake` (a chain of four parts, tail selected) and on `leaf` (one part with `at` and a pivot, a Spin clip)
the selected part's blue box and the Edges lines stand a voxel or two beside the drawn voxels. The overlay and
the mesh must take one transform (parent chain, `at`, pivot, the clip's pose), and a click must land on the
voxel under the pointer. Sent to the models session as a red-then-green fix; ADR 0065 gets the finding.

### 7. Fine voxels per model
A sapling, a leaf or a pebble is a handful of 12.5 cm voxels and cannot have a shape. A model may therefore
choose a finer voxel.

- **The file:** `voxel = 16` in `data/models/<name>.toml` (voxels per metre; 8 when absent, so every model
  made so far is unchanged). 16 is 6.25 cm: exactly one texture pixel, so a fine model beside a painted block
  has the same grain. One size per model; parts do not mix. A part's box stays at most 32 on a side, which is
  2 m for a fine model.
- **Units:** `at`, pivots, points and a clip's moves are counted in the model's own voxels, as now. The
  mesher scales by the model's size.
- **The Studio:** New model asks the size beside the starter shape, in plain words: "Normal (12.5 cm): tools,
  machines, creatures" and "Fine (6.25 cm): small plants, pebbles, items". The floor grid and the status line
  follow. "Make fine" turns a normal model into a fine one (every voxel becomes 2 x 2 x 2, nothing is lost);
  the way back is offered only as a preview with Use and Cancel, since it merges voxels.
- **Patterns for models come from this:** at one voxel per pixel a pattern is painted voxels. The texture
  page's Scatter, Shade, Spray and Dither become model tools on the faces under the pointer, and **Pattern
  fill** paints a selection's surface from the pattern panel's recipe (three colours, size, amount, contrast,
  Another). So "a pattern on a voxel" needs no second kind of material.
- **Cost, to be measured before it is kept** (CPU first): a fine model has 8 times the voxels of the same
  volume. `run=models` with a realistic count of fine models (a meadow of saplings as a block row's `model =`)
  must hold ADR 0004; the result and the choice go in an ADR.
- Accepted: a fine model is finer than the 12.5 cm cells the world breaks into (ADR 0033). It is for small
  things that are whole or gone.

Done when: a model file with `voxel = 16` has its tests (parse, mesh size, the old files unchanged); "Make
fine" round-trips in a test; a `drive` script makes a fine sapling with Pattern fill and its `shot=` is read;
the models benchmark's numbers are in the ADR.

### 8. The palette grows where you paint
The palette has 109 colours and its own page, but while painting there is no way to get a colour that is
missing, so the palette feels like a wall. The rule stays (every pixel and voxel is a palette colour, so
changing a colour repaints everything that uses it); adding becomes one click.

- **"+" at the end of the swatches** on the Textures and Models pages and in the pattern panel's wells: a
  small picker (hue, saturation, value, hex) and a name with a suggestion; Add puts the colour in
  `data/palette.toml` and selects it. Undoable; a colour nothing uses can be removed in the same place.
- **Add shades:** from the chosen colour, a ramp of five (two darker leaning cooler, two lighter leaning
  warmer), shown before Use, named after the colour. The new shades join the colour's ramp, so Shade and the
  patterns' "auto" second and third colours use them at once.
- **Finding a colour as the palette grows:** swatches stay grouped by ramp, a search box filters by name, and
  the count is shown ("112 of 255": a model file's letters allow 255 colours; the Studio warns near the end).

Done when a `drive` script adds a colour and a ramp from the Textures page, paints with them, saves, and
`check=1` passes; removing an unused colour and refusing a used one have tests.

## Left out on purpose
Import of outside sound files is in since 2026-11-12 (above). Import and export of other tools' formats, sculpting, bones and bending meshes, textures on models (a
model's voxels carry colour only), sizes other than 16 px and 12.5 cm (6.25 cm models are in since the second round, step 7), an editor inside the game. Each can be
added when its lack is felt.

## Build order
Each step is small, checked alone, and useful if the next one never comes.

1. **Spike, measured:** blocks drawn from a 16 x 16 texture array against today's patterns, on `stand` and
   `fly` (GPU ms per frame, VRAM). The answer decides the textures' ADR.
2. **Textures in the game:** the PNG rule above, a test that a bad file is refused with its name, the fixed
   views unchanged while no PNG exists. Already usable with any paint program.
3. **The Studio's shell and the texture page**, and the game's watch on `data/`.
4. **Models in the game:** the file format with its tests, the mesher, one static model drawn in the hand,
   the many-models benchmark.
5. **The model page**, static models.
6. **Parts and clips** in the game and the timeline in the Studio.
7. Later: the effects page, then trees and structures.

## Open questions for the developer
- **Light from a model:** may a model glow per voxel (a lit screen on a machine), or only as a whole like a
  lamp block? The default is as a whole until a machine needs more.

Answered 2026-11-12 (usability round):
- **The pattern panel does not write the block's row** (left to Claude). The PNG is the look; the game
  already takes a painted block's far colour from the texture's mean, so `blocks.toml` has nothing to gain
  and one file changes per Use.
- **Sound licences: own, CC0 and CC BY** (the developer: the game is not planned for public release).
  Anything else is refused by `check=1`. **Before any public release:** every CC BY row of
  `data/sounds/credits.toml` must be shown on a credits screen (author, title, source, licence), and a
  release check must refuse to ship without it. Do not drop the credits file's source and licence fields.
