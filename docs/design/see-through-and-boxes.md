# See-through blocks and textured box models

- **Status:** design, 2026-11-12. Steps 1 (the spike, ADR 0068) and 2 (cutout blocks, ADR 0069) are built. Each step starts with a measured spike and gets its ADR
  from the coding session.
- **The developer's calls (2026-11-12):** "I want see-through leaves anyways and I want detailed forest floor
  clutter later and see-through glass, so I'd suggest integrating textured boxes and make it a big nice
  upgrade." Models are painted at **32 pixels per metre**, blocks stay at 16. Textured boxes **replace** the
  fine 6.25 cm voxels of `studio.md`'s second round (its step 7 is withdrawn; steps 6 and 8 stand).
- **The model:** Vintage Story's small things (flint, saplings, loose stones) are a few free-size, tilted
  boxes wearing a painted texture, plants are thin planes with empty pixels, and a field of them varies by a
  turn and an alternate shape per place. We take that, held to our palette and our budget.

## What it gives
1. **Leaves you see through** and plants with real outlines: a texture may have empty pixels.
2. **Glass:** blocks and panes that show what is behind them, tinted.
3. **Small detailed models:** a flint, a sapling, a mushroom, a twig, made of a few painted boxes.
4. **Forest floor clutter** (later): thousands of those on the ground at no cost to the frame thread.

## What the code is today (surveyed 2026-11-12)
These facts shape every choice below; a step that finds one changed says so in its ADR.

- Painted textures are one `u32` per texel in a storage buffer, 16 x 16 per layer, 2048 layers, 2 MB
  (ADR 0045). The alpha byte is stored and thrown away; `engine/src/textures.rs` `decode` refuses any pixel
  that is not opaque, and the Studio's `check=1` calls the same function.
- The mesher culls a face against **any** non-air neighbour (`engine/src/mesh.rs`); no block property is read.
- The frame is deferred: g-buffer (sky, terrain, models), effects, the light passes, then a "screen" pass that
  composites and draws water alpha-blended, unsorted, without depth write. Nothing in terrain or models tests
  alpha.
- Sun, lamps and bounce are hardware ray queries against one BLAS per chunk built from the mesher's quads,
  flagged opaque (ADR 0022, 0025). Whatever is meshed blocks light; models cast no shadow.
- A model is voxel parts of flat palette colour, 16 bytes a quad, drawn as one instance per piece with a
  transform posed on the CPU each frame, within 128 m, about 1000 pieces recommended (ADR 0047). A part has
  no rest rotation. No block row uses `model =` yet.
- Leaves, shrubs and reeds are whole cube blocks placed by the bake's still plants (ADR 0043). There is no
  scatter of small models and nothing lies on the ground to be picked up.
- Far terrain is coarser chunks drawn by the same shader, the textures stretched over larger cells.

## Part A: see-through blocks

### The data
A block row gains `see`, one of:

- absent: **opaque**, as every block today.
- `see = "cutout"`: every pixel of its paintings is fully there or fully empty (alpha 255 or 0). Leaves,
  shrubs, reeds, grates.
- `see = "glass"`: pixels may be partly clear (any alpha); the colour is still a palette colour. Glass, ice
  later.

`decode` takes the row's kind: an empty pixel in an opaque block's file is refused by name as now, so no
existing texture changes meaning. A cutout or glass block with no painting keeps its pattern and is opaque,
so nothing has to be painted before the rest works.

### The mesher
A face is culled only against an **opaque** neighbour, or against the **same** glass block (a glass wall has
no inner faces). Between two cutout blocks the faces stay by default; the Graphics setting **Leaves**
(fancy, fast) drops them on fast and draws cutout blocks opaque with their mean colour. Culling reads one bit
per block id from a table, not the row, so the inner loop stays as it is: measure the mesh time before and
after (the slow path is the test's reference).

### Drawing
- **Cutout** is drawn in the g-buffer pass with the terrain: the fragment shader reads the texel it already
  reads and discards an empty one. It writes depth and takes light like any block. No sorting, no blending.
- **Glass** cannot go through the g-buffer (one surface per pixel). It is drawn after the composite, beside
  water: blended, depth-tested, lit by the forward path water uses. Order between glass and water is by chunk
  distance, glass faces inside a chunk unsorted; the spike shows whether that is seen and what sorting costs.
- **Far:** from the first coarse level a cutout block is opaque with its mean colour and glass is left out
  (or opaque, the spike decides by looks), so the stretched textures never show stretched holes.

### Light
The sun must fall through a crown and through a window, and the rays are the frame's largest GPU cost.
Three ways for the spike to measure on a forest view, cheapest first:

1. Cutout and glass stay opaque to rays: block-shaped shadows under trees, a dark room behind glass. Glass
   then needs at least to be left out of the BLAS.
2. On a hit, the shader looks up the texel at the hit point; an empty pixel (or glass, with its tint)
   continues the ray from there, a few steps at most. Dappled light, coloured light through glass.
3. Non-opaque geometry with a candidate loop in the ray query.

The taste default is 2 if it fits ADR 0004, else 1 for leaves with glass left out of the BLAS. The Studio's
lit block uses the game's renderer and so shows the same.

### The Studio
- An **Eraser** on the Textures page for a cutout or glass block (the page had none because textures had no
  empty pixels); empty pixels show as a checkerboard on the canvas and as holes on the lit block.
- For glass, the second colour well becomes **how clear** (four steps), painted like a colour.
- A row's kind is chosen on the block (opaque, cutout, glass) with a line saying what each is for.
- The pattern panel gains two recipes that have holes: leaves and grate.

## Part B: textured box models

### The file
`data/models/<name>.toml` keeps its voxel parts as they are (12.5 cm, flat colour; every model made so far
loads unchanged). A part may instead hold **boxes**:

- a box has a corner and a size in **1/32 m** (one model pixel), a turn about its own middle, and for each of
  its six faces a rectangle of the model's painting;
- a box with no thickness on one axis is a **plane**, drawn from both sides (a plant's cross, a leaf);
- the painting is `data/models/<name>.png`, palette colours only, empty pixels allowed, 32 pixels per metre;
- a part gains a **rest turn** (`turn = [x, y, z]`), so a flint lies tilted without a clip;
- alternates are `<name>.1.toml` to `.4.toml`, picked by place as textures pick variants.

Limits (boxes per model, the painting's size) come from the spike; a first guess is 64 boxes and 128 x 128.
A model may mix voxel parts and box parts. Clips, points and states work on both.

### In the game: two ways to draw one model
- **Moving** (in the hand, a creature, a machine in a state): instances as today, with a second quad format
  that carries a place in the model's painting. Where the paintings are kept (the texel buffer's layers, or a
  second buffer) is the spike's call, measured in VRAM and GPU time as ADR 0045 did.
- **Still** (a block row's `model =`, clutter): the model's quads are **added to the chunk's mesh by the
  mesher's worker**, turned and chosen by the cell's hash. The frame thread does nothing per model and there
  is no 1000-piece ceiling. This is what makes clutter affordable on a CPU-bound game, and it is the larger
  half of the work. Whether still models enter the BLAS (cast and take ray shadows) is per row: off for
  clutter, on for a machine.

### The Studio: boxes without ever seeing a UV
The bar of the usability round holds: someone new makes a flint in five minutes.

- **Box tools** on the Models page: add a box, drag a face to size it (it snaps to pixels), move, turn (in
  steps of 22.5 degrees), copy, mirror. The ghost, the red outline and the view bar of the model view apply.
- **Paint straight on the model:** pencil, fill, picker, scatter, shade, spray, the eraser and Pattern fill
  work on a box's faces in the 3D view. The Studio lays the faces out in the PNG by itself and re-lays them
  when a box changes size, keeping the paint.
- A flat **face view** beside the 3D view for the face under the pointer, for exact pixels.
- **Starter shapes:** pebble, flint, twig, mushroom, fern, sapling (crossed planes), grass tuft, glass pane.
- **Preview as ground:** a patch of 8 x 8 blocks with the model scattered as the game will scatter it, its
  alternates and turns mixed.
- `check=1` holds a model's painting to the palette and the alpha rule, and names the file and pixel.

## Part C: clutter and pick-ups (designed in `clutter-and-pickups.md`, 2026-11-12)
That document is the design; the lines below are the first sketch. Step 7 is two steps there: 7a clutter,
and 7b pick-ups, which also need items (`items.md`).

- **Clutter** is still box models placed by the seed: a biome row lists models with a density, a cell's hash
  picks one, its alternate and its turn. Nothing is stored; a dug-up or trampled one is a delta like any edit.
  Density and range are Graphics settings.
- **Pick-ups** (flint, sticks, loose stones) are clutter the server knows: taken is a delta, the item goes to
  the inventory. That needs things lying on the ground as a server concept, which does not exist yet.
- Leaves on trees are Part A, not clutter: crowns stay blocks, now cutout.

## Budget and checks
- ADR 0004 holds at default settings. New measured views: a **forest** viewpoint and fly-through (cutout
  overdraw, rays through crowns), a **glass house**, a **meadow of still models** at clutter density.
- CPU first: every step reports mesh time per chunk and the frame thread's ms before and after, then GPU ms
  and VRAM.
- The fixed viewpoints do not change while no row has `see` and no model has boxes.

## Build order
Each step is useful if the next never comes.

1. **Spike, measured** (`prototypes/`): cutout terrain on a forest view with the three light options; still
   box models in the chunk mesh at clutter density. The answers decide steps 2 and 5.
2. **Cutout blocks in the game** and the Studio's eraser: leaves, shrubs, reeds. **Built** (ADR 0069).
3. **Glass.**
4. **Box models in the game**, moving (the hand, `model=` on a run): the file, the mesher, the paintings'
   store, the rest turn.
5. **The Studio's box tools** and painting on the model.
6. **Still models in the chunk mesh** and `model =` on block rows, with alternates and turns.
7. Later: clutter by biome, then pick-ups.

## Open questions for the developer
- **Tinted shadows:** should light through coloured glass be coloured? The default is yes if option 2 holds.
- **Clutter and footsteps:** does walking flatten or break clutter? The default is no; it is decoration.
