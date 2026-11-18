# TechnoApe Studio: the pages and their tools

- **Status:** design, 2026-11-11, written for the night's build (the developer: a robust program with clear
  tools for someone who is no artist; cut corners as Blender and game engines let you; a good interface).
  `studio.md` says what the Studio is and its file rules; this says what each page does.
- Each part names the wave that builds it. A part not built yet is only a plan.

## What the Studio is for
The developer is not an artist and Claude must not be one. So the Studio's tools do the craft and leave the
choices: it offers, the developer picks and adjusts. Five ideas run through every page.

1. **Pick, don't draw.** Wherever something can be generated from a few numbers (a colour ramp, a tree, a
   sound, a flame), the page shows a **wall of variations** and the developer clicks the one they like, then
   "more like this". Nothing generated is final: it lands in the editor as ordinary, hand-editable data.
2. **Sliders over files.** A thing is its source plus a short list of changes (a base sound and its chain; a
   tree's numbers and its seed), kept as data. Changing a slider never destroys the source.
3. **The palette and its ramps do the shading.** Nobody picks "a slightly darker green": tools step along a
   ramp of palette colours.
4. **See it in the game at once.** Every page previews with the game's own renderer and mixer, and a running
   game takes a saved file within a second.
5. **Nothing is lost.** Undo everywhere, per file; saves go through a temporary file; a refused file is
   never overwritten; the Studio notices files changed by another program.

## The interface (wave 1)
One look for every page, so a tool learned once works everywhere.

- **Frame:** a bar of pages on top (Textures, Palette, Models, Sounds, Effects, Structures, Flora, Fauna; a
  page not built yet is not shown). Left: the files. Middle: the work. Right: the inspector (tools on top,
  then what the chosen tool or thing can be set to, then the preview). Bottom: one status line.
- **The files column:** a search box, then filter chips (All, Made, Drafts, Unsaved, Problems), then rows
  with a small picture, the name, a dot when unsaved and "draft" when it is Claude's. Its foot says how many
  are still drafts.
- **The status line:** left, what the tool under the pointer does and its keys ("Pencil: left paints the
  first colour, right the second; Alt picks"); middle, the pointer's place and colour; right, saved or not,
  and a **problems badge**: the file check runs after every save and when a file changes on disk, the badge
  counts what it refuses, a click lists them and a click on one opens it.
- **Every control has a tooltip** with what it does in one line and its key.
- **Ctrl+P, the command box:** type to find any file, page, tool or action and run it. **F1** shows every
  key of the page. Keys are the same on every page where the tool is the same (Ctrl+Z, Ctrl+S, B, G, I, M).
- **Toasts** in the bottom right for what happened ("Saved dirt.png", "Refused: stone_top.png is 24 wide"),
  gone after four seconds, red ones stay until clicked.
- **History:** the inspector can show the file's undo steps by name; a click goes back to one.
- **Look** (ADR draft-studio-look; like Adobe's dark tools): neutral greys with no blue in them (work area
  `#1e1e1e`, panels `#2a2a2a`, hover `#353535`, chosen `#434343`, hairlines `#3d3d3d`, field borders
  `#525252`; text `#e6e6e6`, secondary `#b0b0b0`, disabled `#6e6e6e`), all named in `ui/look.rs`.
  **One blue** (`#2680eb`) for the main action and for focus: the one filled pill on a screen (Try in game,
  Save when there is work to save, Use, Import), a text field's 2 px focus ring, a chosen node's ring. Text in
  the system's clean sans (Segoe UI, regular and semibold: section titles and tabs) at 13 and 11.5 points;
  View > Interface font "Game" puts the game's pixel font back with its pixel-sharp scale steps (kept in
  `studio.txt`). Controls are quiet: a button is text that gets a fill on hover, a chosen chip `#434343`
  with white text in a segmented group, sliders a 2 px track with a round handle; corners 4 on controls,
  8 on cards, menus and windows, with a soft shadow under menus. Sections have flat headers (chevron,
  semibold title, a 2 px bar in the page's colour) with hairlines between them, not bands. Tabs are plain
  text with a dot of the page's colour, the open one white and underlined in it; the top bar is 40 points
  (it grows to two rows of tabs only when they do not fit) and carries a search-like "Find… Ctrl+P".
  Tooltips show the key in a keycap. Nothing moves when a tooltip or toast appears. **Each page has its own
  colour**, never used for what is chosen. Status keeps its three: amber for drafts and unsaved, red for
  problems, green for fine.
- **The inspector's top** is the same on every page: Undo, Redo and Save for the open file (grey when there
  is nothing to undo or redo). Below it, sections; in a section every label stands in one column with its
  control beside it (the label goes over the control when the panel is narrow). Tools stand in three
  columns with their key weak beside the name; an option that is on or off is a round chip, not a tool.
- **The layout is the developer's**, and `studio.txt` keeps it. Each page has its own two side panels: they
  drag wide (the work keeps 520 points) and hide (F2 the files, F3 the inspector, F4 both: the work area
  alone), so Models may have a wide inspector while Sounds has none. The View menu also puts the inspector
  on the left (everywhere) and makes the layout as new. A click on a section's title folds it shut; a drag
  of the title up or down moves the section among the others.
  Where a page's work is two parts one over the other (Models: view and timeline; Structures: view and
  slice; Sounds: wave and chain) the bar between them drags, and a double click puts it back.
- **The font is the game's own** (`data/font.txt`), each character as wide as it is drawn (digits a fixed
  width); a change to the file shows at the Studio's next build. Buttons have a fine edge and keep their
  size under the pointer; a slider is a thin rail lit up to a slim handle.
- **Sharp text:** the font is one size everywhere (small text is told by its colour), and the interface
  scale steps only through the sizes that draw a pixel of the font on whole screen pixels (on a plain
  screen 67, 133 and 200 %); the window starts on the one nearest its kept scale. The characters the game's
  font has not (degree, plus-minus, squared, middle dot, times, ellipsis) are the Studio's own cells.
- **Tools have icons** (`crates/studio/src/icons.txt`, 8 x 8 cells drawn at the font's pixel) before their
  names, on Models and Textures.
- **A player for scale** on Fauna as on Models (P): a 0.6 x 1.8 m outline beside the creature, in the
  circle's middle when it walks one.
- **Where the developer was:** the Studio starts on the page it was closed on, each page with the file it
  had open (`studio.txt`: `page=`, `open.<Page>=`; a file that is gone is skipped; `open=` on the command
  line wins). Ctrl+1 to Ctrl+7 go to the pages. The inspector's top names the open file in the page's colour.
- **Try in game (F5):** saves the open file and starts the game on it: a creative world that keeps no save,
  the model, effect, structure or creature standing ahead at the spawn (a texture's block is given, to
  place). The game then shows every later save within a second, so the Studio and the game stay side by
  side. Try in game again while that game still runs says the open structure's `/structure` or the open
  creature's `/spawn` there (the game was started with `lines=stdin`), in place of a second game; on the
  other pages the running game shows a save by itself. In a running game `/structure <name> [seed]` stands a structure file five blocks ahead and
  `/spawn <creature>[:speed]` shows a creature walking in place three blocks ahead (a preview for the one
  player: creatures do not live in the world yet).
- **The game view's scenes:** Grass, Cave, None (the thing alone on a plain grey) and Checker (the same on
  a checkerboard). None and Checker are not offered where the thing is blocks (Structures, Textures).
- **File rows** of one family ("dirt: dig, place, step") stand under the family's name; a row's kind
  ("grown", "quadruped") is a weak tag after its name. Panel widths and the interface scale
  (80 to 200 %) are kept in `studio.txt` beside `settings.txt`.
- **Outside changes:** the Studio watches `data/` as the game does. A file changed elsewhere reloads when it
  has no unsaved work, and asks which to keep when it has.

## Palette: ramps (wave 1)
A ramp is a run of palette colours from dark to light that belong together ("grass": 5 greens).

- `data/palette.toml`: a colour may carry `ramp = "grass"`; a ramp's order is the file's order. A colour
  with no ramp stands alone.
- The Palette page shows each ramp as a row and the loose colours last. **Make a ramp:** pick a base
  colour, set steps (3 to 9), how far the hue turns towards cool in the dark and warm in the light, and how
  the saturation falls at both ends; a wall of variations shows candidates; a click adds it under a name.
  The same sliders stay on an existing ramp, so changing them recolours it (and so every texture using it).
- **Mood sliders** over the whole palette (warmer/cooler, more/less saturated, lighter/darker) with a
  before/after of every painted block; "apply" rewrites the palette once.

## Textures: the tools (wave 1)
Besides pencil, fill, picker, line and mirror:

- **Tiled painting (T):** the canvas shows the face's neighbours ghosted around it and strokes wrap over the
  edges, so a texture tiles by construction. A **seam view** marks edge pixels that clash with the far edge.
- **Shade (U):** left click makes a pixel one step darker on its ramp, right click one lighter. Dragging
  shades each pixel once per stroke.
- **Scatter (N):** paints with the first colour and its ramp neighbours at random, by an amount slider; a
  quick way to get grain that is still only palette colours.
- **Spray (A)** in a radius; **dither fill** (a checker of the first and second colour); **gradient** between
  two colours of one ramp, dithered, along a dragged line.
- **Select (S):** a rectangle; move it, flip it, turn it a quarter, copy and paste between faces, variants
  and blocks. The arrow keys shift the whole face with wrap.
- **Replace colour** in the file; **swap ramp** (repaint a texture from one ramp to another of the same
  length: stone to sandstone in one click).
- **Start from pattern:** the block's current shader pattern drawn into the canvas, each texel snapped to
  the nearest palette colour.
- **Start from a picture:** any image cut to 16 x 16 and snapped to the palette, as a rough to paint over.
- **Make variants:** from variant 1, make 2 to 4 by moving and re-scattering a share of its pixels (a
  slider), each still editable.
- **Copy from block:** start from another block's texture.
- The preview can be 3 x 3 or 5 x 5, and shows top, side and bottom unfolded.

## Models (game: wave 1; page: wave 2)
The file, `data/models/<name>.toml`:

```toml
colours = ["stone dark", "stone", "copper"]   # game palette names; at most 62 per model
item = 0.3                 # optional: one cell's size in metres as an item, in hand and in a pile (0.05..1;
                           # absent: 0.35 in hand; in a pile 0.35, or 1 for a model of painted boxes; ADR 0114)

[[part]]
name = "body"
parent = ""                # "" for a root; else another part's name
at = [0, 0, 0]             # where the box's corner sits in the parent's voxels (a root: in the cell)
pivot = [4, 0, 4]          # the point it turns about, in its own voxels
box = [8, 6, 8]            # x, y (up), z in voxels, each 1..32
voxels = """
aab.....
"""                        # layers from the bottom up, a blank line between layers; in a layer one row
                           # per z, one letter per x: '.' empty, then a-z A-Z 0-9 in the order of `colours`

[[point]]                  # a named place for an effect, a sound or a held thing
name = "flame"
part = "body"
at = [4, 2, 1]

[[clip]]
name = "work"
length = 1.0               # seconds
loop = true
key = [
  { part = "door", t = 0.0, move = [0, 0, 0], turn = [0, 0, 0], ease = "smooth" },
  { part = "door", t = 0.5, turn = [0, 70, 0] },
]                          # move in voxels, turn in degrees about x, y, z at the pivot;
                           # ease: linear, smooth, step (how it leaves this key); a part with no key rests
```

- A voxel is 12.5 cm, the fine cell of ADR 0033: 8 to a block's edge, two texture pixels wide. A root part's `at` places it in a 1 m cell whose corner is 0,0,0.
- In the game: each part is meshed once (hidden faces dropped, flat runs merged), drawn per instance with
  one transform per part, lit by the light of the cell it stands in. A block row may name `model = "x"`:
  the model is drawn in the cell in place of the cube (the cell still collides as a whole block for now).
  The item in the hand is a model when one of the block's name exists, and so is a stack lying in a pile (a
  standing item laid on its face; ADR 0114); painted boxes are drawn there too, a quad a pixel. Clip `idle`
  loops when present.
- Files reload live, as textures do.

**The Models page** (wave 2): a 3D view drawn by the game's renderer. Left click adds a voxel on the face
under the pointer, right click removes, the picker and palette as on Textures; box, line and fill tools work
on the plane of the face first clicked; mirror in x and z; a slice slider hides everything above a height.
**Shade by light:** one click recolours every voxel along its ramp by how exposed it is (open sides lighter,
crevices and undersides darker), the corner an artist would paint by hand. **Extrude:** drag a face of a
selection to pull it out. **From a texture:** a block's painted faces become an 8-voxel cube to carve (each voxel the commoner colour of its 2 x 2 pixels). The
parts are a tree in the files column of the page with their pivots shown as crosses; a **timeline** under the
view has one row per part with its keys, a play button and a clip chooser; moving or turning a part while
the timeline stands on a time sets a key there. **Ready-made motions** fill a clip from one choice (spin
about the pivot, swing, bob, shake, open and shut), then the keys are ordinary keys. A 1 m block and a
player's outline can stand beside the model for scale.

## Sounds (wave 2)
Today a row of `data/sounds.toml` is made in code from layers, or replaced by a recording. The page keeps
both and adds a chain, so one base becomes many natural sounds:

- **Bases:** every layered row is a base, and so is every WAV under `data/sounds/base/` (the developer's
  own recordings or sounds they have the right to use). A shipped set of code-made bases covers the usual
  needs: thud, click, crack, crunch, hiss, hum, ring, whoosh, drip, pop, rumble, chirp, growl, scrape.
- **A row may be made from bases:** `from = [{ base = "thud", gain = 1.0, delay = 0.0 }, ...]` (layered),
  then `chain = [...]`, each step one change with its numbers:
  `pitch` (semitones), `speed` (times, pitch kept), `stretch` (pitch and speed together), `lowpass`,
  `highpass`, `bandpass` (Hz, q), `reverb` (room size, damping, mix), `echo` (seconds, feedback, mix),
  `drive` (distortion), `tremolo` and `vibrato` (rate, depth), `fade` (in, out), `trim` (start, end),
  `reverse`, `gain`, `crush` (bits).
- **Natural, not repeated:** `variants`, and per variant a random spread on any chain number
  (`spread = { pitch = 1.5, lowpass = 400 }`), so eight takes of one footstep differ a little in several
  ways, not only in pitch.
- Everything is rendered once when the sound card opens, as today: the chain costs nothing while playing.
- **The page:** the wave drawn large with its envelope; the bases as layers with gain and delay; the chain
  as a stack of cards to add, drag in order, switch off and set by sliders; space plays, and every change
  plays at once when "audition" is on. **Mutate** fills a wall with six variations of the chain's numbers,
  each playable; a click takes one. **Compare** plays the saved and the edited sound in turn. **In place:**
  plays it as the game would at a chosen distance and in a cave or the open. A family (dig, place, step of
  one material) is edited side by side.

## Effects (wave 3)
On top of today's rows in `data/effects.toml` (ADR 0031):

- **Colour over life:** `gradient = [[0.0, "lamp light"], [0.4, "copper"], [1.0, "stone dark"]]` (palette
  names), in place of one colour. **Size and brightness over life** as curves of a few points.
- **Motion from noise:** `noise = { strength, scale, speed }` pushes particles through a moving noise field
  (flames lick, smoke curls, dust drifts); `rise` and `spread` shape a plume.
- **Glow:** how much light an effect's particles give, from its gradient.
- **Lasting effects:** `rate` particles per second for as long as it plays (a fire burns, a chimney
  smokes), beside today's one-time `count`.
- **Where they start and how they look:** `start` gains ring, cone (an angle about a direction) and a box of
  any size; `look` gains a model (a leaf, a chip) and a small painted texture; particles may face the camera,
  lie along their flight (rain, sparks) or tumble. An effect may play another when a particle dies or lands
  (today's `then`), and several rows may be grouped into one named effect (fire = flames + embers + smoke).
- **The page:** the effect playing in the game's renderer on a small scene; the gradient as a bar with
  stops to drag and palette swatches to drop on it; curves as small graphs; the noise shown as a moving
  field; **presets** (fire, smoke, sparks, steam, dust, drip, glint, leaves) as starts; a wall of variations.

## A model's life: states (wave 3)
What makes a furnace a furnace, in its model file, edited on the Models page:

```toml
[[state]]
name = "working"
clip = "work"
effects = [{ effect = "fire", point = "flame" }, { effect = "smoke", point = "chimney" }]
sound = "furnace.burn"       # a looping row
light = { point = "flame", colour = "lamp light", strength = 2.0 }
```

The game switches a model between its states (`idle` when none is asked for); the page has a state chooser
and plays the clip, the effects and the sound together, so a machine is judged whole. Timed events in a clip
(`event = [{ t = 0.4, sound = "furnace.clank" }, { t = 0.4, effect = "sparks", point = "door" }]`) play a
sound or an effect at a moment of the motion.

## Structures and flora (wave 3)
- **A structure** is a box of whole world blocks, `data/structures/<name>.toml` (a legend of block names to
  letters, then layers as in a model, `.` leaves the world as it is, `_` clears to air). The page builds it
  with the game's renderer: place and dig as in the game, plus box, fill, mirror, copy.
- **Flora** is grown, then edited. A plant row has a kind (tree, bush, grass tuft, flower, vine, mushroom),
  its numbers (trunk height and thickness, lean, branches and their angle, crown shape and size, leaf
  density, root flare) and the blocks or models it is made of. The page shows a wall of seeds for the
  numbers; a click picks one; "freeze" turns it into an ordinary structure or model for hand work. Small
  plants (tufts, flowers) are models; trees are structures of blocks.
- World generation places neither yet; a row may say where it belongs (biome, how many per hectare) so the
  generator can take it when it grows plants.

## Fauna (wave 4)
A creature is a model with parts and clips plus a row of `data/creatures.toml`: its model, its size, which
clips are its walk, idle, eat and hurt, its sounds, how fast it walks. **Rig by choice:** pick a body plan
(two legs, four legs, six legs, wings, fish, snake); the page makes the parts and a walk, idle and turn that
fit the plan and the leg length, as ordinary parts and keys to change. A preview walks it in a circle on the
game's ground at its real speed so sliding feet are seen. The game does not spawn creatures yet; the page
makes what the game will take. Built (ADR 0053): biped, quadruped, bird, fish, hexapod and snake; the walk,
idle, turn, eat and hurt are made from the row, never keyed; wings as a plan of their own are the bird's.

## The variations wall (every wave uses it)
One shared piece of the interface (`ui/wall.rs`): six to twelve candidates made from the current thing.
Pages feed it their own kind (ramps, textures' variants, chains, effects, seeds, body plans).

- **The grid:** large rounded pictures with no text under them, sharing the width evenly (the cell size
  lies between the page's least and half as large again). The one the wall was made around carries a
  "Current" badge.
- **Picking:** a click picks one (an accent ring and a check badge) and the preview follows; a double click
  is Use. Hovering a picture lightens its edge and shows a round "more like this" button at its top right
  (a new set around that one) and, where items can be heard, a play button at its bottom left.
- **One bar above the grid**, left to right: Use (the blue pill) and Cancel on a page whose pick waits for
  Use (Textures' variant sets, Maps); "More like this" for the picked one (grey until one is picked);
  "New batch"; "Back" (the set before the last "more like this"); "How different", a slider from subtle to
  wild.
- **Keys** while a wall is up: Enter uses, Escape cancels, M is more like the picked one (M, Enter and Escape
  are the wall's then, not the page's).
