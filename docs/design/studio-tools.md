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
- **Look:** dark, calm, one accent colour for what is chosen; panels on one grid with the same gaps; text in
  two sizes only; nothing moves when a tooltip or toast appears. Panel widths and the interface scale
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
  The item in the hand is a model when one of the block's name exists. Clip `idle` loops when present.
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
One shared piece of the interface: a grid of six to twelve previews made from the current thing by a
"spread" slider, a click takes one, "more like this" refills the wall around it, "back" returns to the last
wall. Pages feed it their own kind (ramps, textures' variants, chains, effects, seeds, body plans).
