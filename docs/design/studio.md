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

## Left out on purpose
Import and export of other tools' formats, sculpting, bones and bending meshes, textures on models (a
model's voxels carry colour only), sizes other than 16 px and 12.5 cm, an editor inside the game. Each can be
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
