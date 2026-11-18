# TechnoApe Studio: the Maps page

- **Status:** design, 2026-11-18 (the developer: take the Studio from a prototype to a tool good enough to
  sell; "researched layouts and colour-coded UI with pins and sliders", "noise maps for textures to simulate
  model animation and particles", "good outside only the game use case"). **Built** (steps 1 to 3, ADR 0153).
  Where the build differs from the plan below:
  - Space adds a node; the middle button pans (one key cannot do both).
  - The starters are kept in code (`ui/maps/make.rs`), not as files; a starter becomes a file only on Save.
  - Variations: the wall holds the map as it is and nine others.
  - Use writes through the Textures page's own document and save, so it is undone there.
  - The inspector's numbers type on a single click (egui's own boxes); node rows on a double click.
  - Node pictures are worked out on the frame thread at 64 px; the full-size preview and the loop on a worker.
- `studio.md` says what the Studio is, `studio-tools.md` the look every page shares. This page follows both.

## What it is
A **map** is a small picture made by a graph of nodes instead of by hand: noise, shapes and filters wired
together, each with a few sliders. It tiles without a seam and may loop over time. One page makes maps for
three uses:

1. **A texture:** the map's colours, held to the palette, become a block's face (the pattern panel's bigger
   sibling).
2. **Motion:** a map of directions that moves over time is a wind for plants and a turbulence for particles.
   The page plays both, so the motion is judged before any game code reads it.
3. **Any other program:** a map is written as a plain PNG (one picture, or a strip of frames) at any size
   from 16 to 512 px, with free colours, so the page is useful outside the game.

The palette rule is not broken: only what goes to `data/textures/` is held to the palette. An export is not
a game file.

## What research said (2026-11-18)
Substance Designer, Material Maker, Blender and Unreal's material editor agree on these, so the page copies
them and nothing else:

- The graph is the largest area; the properties of what is chosen are on the right; a 2D preview is always in
  view; the list of nodes is a search box that pops up, not a panel that takes room.
- **A pin's colour is its kind of data** and a wire takes the colour of the pin it leaves. Blender's colours
  are the most known: grey for a number, yellow for a colour, purple for a direction.
- **An input pin with nothing plugged in shows its slider on the node.** Plugging in hides the slider.
- Every node shows a small picture of what leaves it.
- A wire dragged from a pin and let go on empty space opens the search box with only the nodes that fit.
- What users complain about: pins too small to hit, wires like spaghetti, slow previews, loops that hang.

No node-graph crate is out for egui 0.37 (egui-snarl 0.12 and egui_dock 0.21 stop at 0.36), so the graph is
our own widget on `egui::Scene`; about twenty kinds of node need no more.

## The file
`data/maps/<name>.toml`, text, so git shows what changed and Claude can write a draft (listed in
`data/drafts.txt` as every draft is):

- The map's own numbers: `size` (16, 32, 64, 128, 256 or 512 px a side), `frames` (1 is a still picture; up
  to 64) and `seconds` (how long the loop takes).
- `[[node]]` rows: an `id`, a `kind`, where it stands in the graph (`at`), its numbers by name, and what is
  plugged into each input pin (the other node's id). Exactly one node is the `out`.
- A file with an unknown kind, a wire that makes a circle, a pin fed the wrong kind or a number out of range
  is refused by `check=1` with the file and the node named; the Studio lists it under Problems and does not
  crash.

## Kinds of data and their colours
| kind | what it is | pin and wire |
|---|---|---|
| number | one value per pixel, 0 to 1 | grey |
| colour | red, green and blue per pixel | yellow |
| direction | a 2D push per pixel, each part -1 to 1 | purple |

A number plugs into a colour pin (as grey). A colour plugs into a number pin (its brightness). A number does
not become a direction by itself: the Slope and Curl nodes do that, on purpose. A wire that cannot be made is
refused while it is dragged, and the status line says why.

## The nodes
Each has a seed where it has chance in it. Every generator repeats exactly at the picture's edge (its `scale`
is a whole number of cells per side) and, when its `speed` is not 0, moves a whole number of times per loop,
so **every map tiles and every animated map loops**, without the developer thinking about it.

- **Make:** Value noise, Smooth noise (gradient noise), Cells (distance, edges or a number per cell), Speckle
  (white noise), Gradient (straight or round), Shape (disc, square, bricks, checker, stripes), Constant. The
  noises have `scale`, `octaves`, `roughness`, `speed`.
- **Change:** Levels (low, high, contrast), Steps (so many flat bands), Invert, Blur (wraps at the edge),
  Move (shift, repeat, mirror; a scroll speed), Warp (pushes one input about by another).
- **Combine:** Blend (mix, add, multiply, screen, lighter, darker, difference; an amount, and a mask pin).
- **Turn into:** Colours (a number becomes a colour along a palette ramp or a few chosen palette colours,
  with or without dither), Slope (a number's downhill direction), Curl (a direction field with no sinks, so
  particles swirl instead of piling up), Length (a direction's strength as a number).
- **Out:** one per map; it takes any kind. What it is fed decides what the map can be used for.

## The page
The frame is every page's: files left, work in the middle, inspector right.

- **Files:** the maps, each with its picture; a draft carries its mark.
- **Work: the graph.** Middle drag pans, the wheel zooms to the pointer, F frames everything (Space is Add a
  node, so it does not also pan). A faint dot grid moves with it.
  A node is a box with a band in its group's colour (Make, Change, Combine, Turn into, Out) holding its name
  and, weak, its kind, its picture, its input pins on the left and its output on the right. Pins are 12 points
  across at least. Every number of a node stands on it as a slider row (name left, value right, a lit rail; a
  drag changes it), as on Blender's nodes, and every choice as a small drop-down; a pin's row loses its slider
  when something is plugged in. A click chooses a
  node, a drag moves it, a drag on empty graph draws a box around several. Delete removes, Ctrl+D copies.
  Right click or Space opens **Add a node**, a search box; a wire let go on empty space opens the same box
  with what fits.
- **Inspector, top, never scrolling: the preview.** Chips `Map | Tiled | Particles | Sway`; under it Play
  and a frame slider when the map has more than one frame. It shows the chosen node, or the node whose
  preview is **pinned** (a pin button on every node), so a filter is tuned while the end result stays in view.
  - *Tiled* is 3 x 3, so a seam would show.
  - *Particles* lets a few hundred dots drift in the map's directions (a number map is curled first).
  - *Sway* bends a row of grass blades by the map as it passes over them.
- **Inspector, below:** the chosen node's numbers as sliders, each with its number and a reset; the map's
  size, frames and seconds; **Use**.
- **Use** (nothing replaces work without showing it first):
  - *As a block's texture:* choose block, face and how many variants (each its own seed); the result shows
    beside what is there now; Use writes the PNG, one undo step, checked as every texture is.
  - *Export:* a PNG at the map's size (a number as grey, a colour as it is, a direction in red and green
    with the middle value as "no push"); an animated map as one strip of frames. The path is the developer's;
    `exports/<name>.png` to start with.
- **New map** never starts empty: Clouds, Stone cells, Wood grain, Wind gusts, Smoke swirl, Ripples, Flicker.
  **Variations** (M) shows a wall of the same graph with other seeds; a click takes one, "More like this".
- Undo and redo per file, the recovery file, the command box, the key sheet and tips work as on every page.
- **Without eyes:** `technoape-studio map=<name> out=out/x.png [frame=k]` draws a map (an animated one as a
  strip) and prints one JSON line with its size, frames, the milliseconds it took and a hash of the pixels.
  Nodes and pins are marked for `drive` scripts as other pages' controls are.

## Speed
The graph is worked out on the CPU, node by node, each node's picture kept until something before it changes.
The live preview may use a smaller size while a slider is dragged. A graph too slow for a frame is worked out
on a worker thread; the window never waits for it. The cost of a 256 px graph of ten nodes is measured once
and written in the commit.

## Left out on purpose
The game reading a map (wind and particle rows that name one) is the next round and is game code with its
own ADR. Also out: maps drawn on the GPU, 3D maps, heights turned into lighting (normal maps: the game has no
use for them), groups of nodes as one node, notes and frames in the graph. Each can come when its lack is felt.

## Build order
Each step is checked alone and reported before the next.

1. **The core:** the file, the nodes and their working out (`crates/studio/src/map.rs`), `map=` and
   `check=1`, three starter files. Tests: a map tiles (the step across the edge is no larger than a step
   inside), an animated map loops (the frame after the last is the first), a seed gives the same pixels
   every time, a file round-trips, every refusal names its node.
2. **The page:** the graph, the pins and sliders, the Map and Tiled previews, the pinned preview, Add a node,
   undo, save, recovery. The page bar holds nine tabs in one row at 1600 x 900 at 133 %. A `drive` script
   builds a small graph and saves it; its shot is read.
3. **Motion and use:** Play, Particles and Sway, Use as a texture, Export, New map's list, Variations.
