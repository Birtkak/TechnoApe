# NNNN – Studio maps: the file, tiling and looping, CPU evaluation

- **Status:** proposed (build step 1 of `docs/design/studio-maps.md`; steps 2 and 3 add to it)
- **Date:** 2026-11-18

## Context
The Studio gets a Maps page: a node graph that makes a small picture (a texture, a wind, a turbulence, or a
plain PNG for any program). The developer asked for maps that tile and loop "without thinking about it". The
on-disk format is hard to undo once maps are saved, so it is decided here.

## Options considered
1. A file per map, `data/maps/<name>.toml`, `[[node]]` rows. Pros: text, git shows what changed, Claude can
   write a draft. Cons: ids by hand.
2. One `maps.toml` with every map. Cons: one bad map refuses all; long diffs.
3. Tiling by blending the picture's edges, looping by cross-fading the ends. Cons: smears, ghosts, and the
   step across the seam is still visible on noise.
4. Tiling and looping by periodic lattices. Pros: exact by construction, no blend. Cons: `scale` and `speed`
   must be whole numbers.

## Decision
Options 1 and 4, worked out on the CPU.

- **The file.** Top: `size` (16, 32, 64, 128, 256, 512), `frames` (1 to 64), `seconds` (0.1 to 60). Each
  `[[node]]`: `id`, `kind`, `at = [x, y]`, its numbers by name, its words (`mode`, `shape`, `ramp`) and its
  input pins by name. A pin holds another node's id (a wire) or a number in 0..1 (the slider it shows when
  nothing is plugged in; a direction pin has none). Colours names a palette `ramp` or `colours = [names]`.
  The Studio writes every number, so the file says what each node does. An unknown key, kind or word, a
  number out of range or not whole where it must be, a wire to no node, a circle, a pin fed a kind it cannot
  take, and no `out` or two are refused by `check=1` with the file and the node named. The kinds and their
  numbers are `SPECS` in `crates/studio/src/map.rs`.
- **Kinds of data.** A picture holds three numbers a pixel: a number (v, v, v), a colour (r, g, b), a direction
  (x, y, 0) with y down the picture, each part -1..1. A number plugs into a colour pin as grey and a colour into
  a number pin as its brightness. Only Slope and Curl make a direction.
- **Tiling.** Every generator lies on a lattice of whole `scale` cells a side whose corners wrap by `scale`
  (an octave o has `scale` x 2^o). Shapes and gradients repeat a whole number of times; a straight gradient
  rises and falls (a triangle wave), since a ramp that only rises steps at the edge. Blur, Warp and Move
  sample with wrap.
- **Which kinds a node takes.** Levels, Steps and Invert take numbers only (a colour is first made a number by
  its brightness). Blur, Move, Warp and Blend take any kind and give back the kind they were fed.
- **Unplugged pins** keep their slider's number in the file, so pulling a wire loses nothing a reader needs.
- **Looping.** Time t = frame / frames, and frame `frames` is frame 0 again. Noise has time as a third lattice
  axis of 2 x `speed` x 2^o cells a loop, so a speed of 1 still changes. Cells circle their points `speed` times
  a loop, Speckle draws `speed` new patterns a loop, and Move scrolls `speed_x`, `speed_y` whole sides a loop.
- **Evaluation.** CPU, node by node in wire order. Each node keeps its pictures by (frame, size), up to eight,
  until a number at or before it changes (`Graph::set`). `Graph::pic(node, frame, size)` takes any size, so a
  preview can ask for less than the map's. Blur's radius scales with the size asked for.
- **Without eyes.** `technoape-studio map=<name> out=x.png [frame=k]` writes the out at the map's size (an
  animated map without `frame` as a strip of frames side by side) and prints one JSON line: size, frames, kind,
  the milliseconds the work took, and an FNV-1a hash of the pixels. A direction is written in red and green,
  128 as no push.

### Step 2: the page
- **Our own graph on `egui::Scene`** (no node-graph crate): pan with the middle button, zoom to the pointer with
  the wheel (0.2 to 2), F frames all. Every edit goes through `Map::add`, `wire`, `unplug`, `remove`, `copy`,
  `set_num` and `set_word`; a wire is refused there (circle, kind) and the map is never left invalid
  (`repair` unplugs what a removal broke). The page shows the refusal's reason while the wire is dragged.
- **Pins by kind:** number grey, colour yellow, direction purple, 12 points across; a wire takes the colour of
  the pin it leaves. Groups' bands: Make green, Change blue-grey, Combine brown, Turn into plum, Out grey.
- **The Maps page colour** is ice cyan (110, 214, 240), apart from the eight pages, the accent, amber, red and
  green.
- **Speed:** node pictures are 64 px, worked out in the frame (`Graph::update` reworks only what is after a
  changed number). The inspector's full-size picture is worked out on a worker thread; the node's 64 px
  picture stands in until it comes.
- **Numbers on the node, as Blender's:** each number, and each unplugged pin's, is a row on the node (name
  left, value right, a lit rail; a sideways drag moves it relative to where it was, a seed one step every 4
  points, else the row's width is the whole range; one undo step a drag). Choices are drop-downs on the node.
  The inspector shows the same numbers with a reset at each row's end. Rows' text hides under zoom 0.45. A
  click on a number does not type into it (yet).
- **Framing:** on open and on F the nodes fill about 80 % of the graph area, zoom at most 1.5; refitted while
  the area settles or is resized, until the view is moved by hand. A dot grid moves with the graph.
- **Space adds a node** (the design also gave Space+drag to pan; one key cannot do both, the middle button pans).
- **The page bar** hides the Studio's title when the tabs would not fit in one row, so nine tabs stay one row
  at 1600 x 900 at 133 %; at 1920 x 1080 nothing changes.

### Step 3, first half: motion
- **Play** (button, K; Space is Add a node) plays the loop at frames / seconds; the frame slider follows, and
  moving it pauses. Node pictures hold their frame while it plays. The previewed node's whole loop is worked
  out once on a worker at the map's size up to 256 (`motion::Strip`, its frames and each frame's push) and
  kept until the node's stamp changes; one worker at a time, the old loop shown meanwhile.
- **Particles:** 300 dots at hashed spots, each living 2 to 4 s then starting elsewhere (so piles do not stay),
  trails of 10 steps, over the map drawn dim. The push is a direction as it is; a number, or a colour's
  brightness, is curled (central differences, wrapped) and scaled so the strongest push is 1. `speed` is
  picture sides a second at that push.
- **Sway:** 26 blades and one taller plant, side view, reading the map's middle row as it scrolls past at a
  tenth of a side a second (the row drawn under the ground). The bend is a direction's x part, or a number
  around its middle, times `strength`; the tip moves most (offset grows with the square of the height).
- Particles and Sway start playing when chosen and stand still when paused; plain 2D painting.

## Consequences
- Every map tiles and loops exactly, and the tests show it (the step across the edge is no larger than a
  step inside; frame `frames` equals frame 0).
- `scale` and `speed` cannot take fractions. Mirror in Move does not flip a direction field's sign.
- A ten-node 256 px graph takes milliseconds on the CPU (number in the commit). A worker thread comes with
  the page if a graph is too slow for a frame. Keeping only eight pictures per node means Play reworks each
  frame. That is for step 3 to measure.
