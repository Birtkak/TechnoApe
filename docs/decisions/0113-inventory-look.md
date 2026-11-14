# 0113 – The inventory's look: one window, the ape, pictures of things

- **Status:** accepted (built; waits for the developer's look and merge)
- **Date:** 2026-11-14

## Context
The developer: "make the inventory look better: nicer UI and a character preview and a better creative inv menu",
and then "grass etc. in the inventory should also keep their real look". The screens of ADR 0075 were a one-row
panel floating mid-screen; creative's tab names and the hint line were clipped; every item was the grey cube of its
`look` block with a cyan corner mark (items had no pictures, ADR 0110). Behaviour (clicks, moves, the wire) stays
as it is: this is presentation in `crates/client` only. No engine, server or shader change.

## Options considered
1. **Pictures of items.** (a) Paint a 16 px icon for every item: a file per item, the Studio's work, never in step
   with the model. (b) Render the model into a texture on the GPU: a new pass and target for a 40 px square.
   (c) **Chosen:** a pure CPU helper (`sprite.rs`) turns a loaded model into a small flat picture once, on the
   models' watcher thread, and the HUD draws it as rects of one colour run each (`hud::Paint`, no new widget).
2. **Plants and leaves in a slot.** Cutout blocks (leaves, shrub, reed; ADR 0069) are drawn flat and upright as
   their painting with the empty pixels left out, tinted to the season the world shows; a block with a `model`
   uses its model's picture; only true cube blocks keep `hud::cube`. Grass blocks are cubes in the world and stay
   cubes. Grass blades and flowers are clutter, not items, so there is nothing more to draw.
3. **The ape's picture.** A live 3D preview (a second camera into the G-buffer) against the same sprite helper at
   176 px: the sprite, since it costs nothing a frame and the ape stands still on this screen anyway.

## Decision
- **One window for both modes** (`inventory::layout`, still pure, drives draw and click as before), in the Esc
  menu's style: the HUD's glass with the accent's corner brackets, a title row (the accent's bar and the name, a
  hairline under it), sections under small cyan headings, slots with a 1 px edge, a brighter fill and cyan edge
  under the pointer, the slot in hand framed in cyan and bracketed, hotbar slots numbered 1-9. The world is a
  little darker under it, as under the menu. The hint line is shortened to fit inside the window.
- **Survival:** the ape | the pack's rows over the hotbar | "On the ground". The ape stands on a dark inset with a
  soft shadow under its feet; under it the worn slot headed "Pack" with health and hunger as numbers beside it.
  The rows' heading names the pack and its slots ("FRAME PACK - 27 SLOTS"; the font has no middle dot). With no
  pack: "No pack worn" and a dim line of what to do. The window keeps its size (three rows' room, the ground's
  column) so nothing jumps when a pack is worn or a pile comes in reach ("Nothing near" when none).
- **Creative:** the title row holds the search as an always-visible field (a click on it opens search as the old
  search tab did: it is still `tabs[SEARCH]`, so `inv_click` is unchanged; typing works only then, as before);
  six tabs wide enough for their names, the open one solid cyan with dark ink as the menu's lit row; a 9 x 6 grid
  (`GRID_ROWS` 5 -> 6) with a scrollbar thumb at its right; "81 items" under it; a divider; the hotbar.
  An empty search says "Nothing named '<q>'".
- **Pictures** (`sprite::sprite`): orthographic, every visible voxel face and painted box face of the model at
  rest sampled at half a pixel or closer into a depth buffer, each face shaded by its turn to a light from the
  upper left; fitted into n x n with a pixel's margin. Slots: 40 px, from 35 degrees round and 30 above (painted
  clutter from 55 above, so its painted top shows). The ape: 176 px from 22 round and 8 above. Made once per load
  of a model (`Loaded::pictures`), kept until the next. `icon` uses: the model of the item's name, else the
  block's `model`, else a cutout block's painting flat (`sprite::flat`, 16 x 16), else the cube. The cyan corner
  mark is left only on items drawn as a cube.
- **A worn pack on the ape:** a block of the pack's look colour behind its shoulders, larger the more rows it
  opens, and two straps over its chest. The item in hand is drawn at its right hand.
- **Tooltip:** the name, and a dim second line when there is something to say: a tool's "34 of 40 uses", a pack's
  "worn: opens 27 slots", a stack's "12 of 64" (creative's grid: "stacks to 64"). Kept on the screen.
- **Fine faces** (ADR 0090, 32 x 32 paintings): `gpu::painting` now gives their first variant halved to 16 x 16
  (`gpu::halve`, a pixel there where two of its four are) where it gave none; the hotbar's and the hand's cubes of
  those blocks show their painting too.

## Tests (each seen failing first)
- `the_screens_put_every_slot_apart_and_inside_the_panel`, extended: both modes at 1280 x 720 and 1920 x 1080;
  the panel on the screen; every slot inside it, apart, off the ape; tabs and the search field inside, apart, off
  the slots, each tab's name fitting; the hint inside and off the slots; the ape's inset at least 150 px. Red on
  the old layout: the tab names were cut.
- `a_voxel_seen_from_the_front_fills_the_middle_in_its_colour_and_nothing_is_empty_model` (a stub returned none).
- `a_painting_seen_flat_leaves_its_empty_pixels_out`, `a_slot_shows_a_block_as_the_world_does` (a cutout block
  flat, an unpainted one and granite as cubes, an item by its model's name), `a_tooltip_says_a_tools_uses_and_what_
  a_pack_opens`: seen red by mutating the code they check (the cutout test turned to glass, the alpha test
  dropped, the pack's words changed), then green.
- `a_fine_face_halved_keeps_its_quarters_in_place_and_its_holes`: written with the code, not seen red.

## Cost
`run=play` 12 s at 1920 x 1080, `slot.sh measure`, one run each, the screen open from 1 s against closed:
frame p50 5.18 -> 6.16 ms, p99 5.75 -> 6.81. The frame thread's own stages are level (pose, table, encode within
0.03 ms); the GPU's `screen` stage +0.22 ms (the darkening and the window's rects), G-buffer +0.54 (one run each:
likely noise, the screen draws nothing there). Row in `docs/perf-debt.md`.

## Evidence
Stills in `out/inv/` (worktree), each read: `after-survival.png` (frame pack worn, straps on the ape, hand axe,
flint, granite, grass, broadleaf leaves and shrub carried, the hand axe's tooltip "40 of 40 uses"),
`after-survival-nopack.png` ("No pack worn", stick, berries and flint as pictures, the stick's tooltip),
`after-survival-1080.png` (1920 x 1080, a basket worn: "BASKET - 9 SLOTS"), `after-creative.png` (the nature tab:
leaves flat as their paintings, reed with its holes, the scrollbar, "81 items", a tooltip),
`after-creative-search.png` ("gr" typed, 17 items), `after-creative-nothing.png` (nothing named 'zzq').

## Consequences
- Any new item with a model of its name, or block with a `model`, gets its picture with no other change; a model
  edited in the Studio shows in the slots at its next reload.
- The hotbar under the world (`hud::layer`) still draws cubes; it could take the same pictures.

## Calls for the developer to confirm
- The window's size fixed per mode (room for three rows and the pile's column even when empty) rather than
  growing with the pack.
- The views: items from three-quarters above (35 / 30 degrees, clutter 55), the ape nearly level and turned 22
  degrees.
- Cutout blocks (leaves, shrub, reed) as their flat painting rather than a cube with holes.
- The pack shown as a block behind the shoulders with straps, the item in hand at the ape's hand.
- Creative without the ape (it would crowd the grid); health and hunger as numbers under the ape, not bars.
- The dark wash over the world while the screen is open.

## Shortcuts
- A season's own painting is not drawn in a slot: summer's painting tinted by the season's colour over summer's
  (`gpu::painting` holds summer's only).
- The pack on the ape is placed by the ape model's proportions, not from its parts; a model per pack when packs get
  their own.
- A cutout block's flat picture is made each frame from its 256 pixels (cheap; cache it if the grid grows).
- No drag-spread, number keys, pack rows drawn as that pack (items.md "The screen"), unchanged from ADR 0075.

## Review
