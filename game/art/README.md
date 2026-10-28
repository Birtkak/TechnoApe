# Hand-painted art

Your own textures, layered over the generated ones. The generators (`tools/make_atlas.py`,
`tools/make_item_sprites.py`) draw everything procedurally and overwrite their output, so never edit
`assets/textures/atlas.png` or `data/item_sprites.json` by hand: put a PNG here and it wins on every run.
Delete the PNG to go back to the generated look.

- `items/<item_id>.png`: 16x16, replaces that item's sprite. The sprite is also the item's 3D look:
  held and dropped items are the sprite extruded one pixel thick; `cube` items draw it on every face.
  Items without a sprite (most blocks) show their material's tiles: paint the tile instead.
- `tiles/<index>.png` or `tiles/<index>_<note>.png`: 16x16, replaces atlas tile `<index>`.
  A tile can be shared by several materials (top, side, bottom); changing it changes all of them.
  - Plain materials (stone, dirt, grass, sand, ...) never show the tile as a picture: every 12.5 cm
    voxel takes one texel, picked by a hash of its position (ADR 0036). The tile is a palette; only
    which colours it holds, and how often, matters.
  - `pattern` materials (planks, bricks, cobble, bark, ores, ...) lay the tile over the world grid,
    one texel per voxel, repeating every 2 m (ADR 0039). Draw them seamless.

Two more kinds live elsewhere, because the game reads them at run time (this folder only feeds the
generators):

- `data/art_models/<name>.json`: a cell model that replaces one model part, at exactly the part's size
  (its pivot and animation are set in code around it). `work_<kind>` = a machine or work piece's
  solid part, built facing 0 (the game turns it); `mob_<species>_<part>` = an animal part (every leg
  of an animal shares one); `ape_<part>` = the player's head, torso, arm_r, arm_l (also the
  first-person arm), leg_r, leg_l, tail. `layers` go bottom to top, each `size.z` rows of `size.x`
  characters: `.` empty, a character of `0-9A-Za-z` = that colour of the family's palette
  (`WorkPalette`, `MobModel` or `ApeModel` PALETTE), `*` = keep the game's own cell (a machine's fire
  or gauge stays live). Code: `scripts/work/art_models.gd`.
- `assets/ui/<skin>.png`: a nine-slice skin for a menu, HUD or inventory box (`UiStyle.SKINS` lists
  the names); corners are a third of the image, drawn 3x. Needs `--import`. Code: `UiStyle.skin`.

Any 16x16 PNG works (Aseprite, Pixelorama, Libresprite, Paint.NET). The Texture Studio artifact shows
the current look of all of these, edits them (a voxel editor for models), previews them the way the
game draws them, and exports a zip laid out from `game/`: unzip it into `game/`.

Apply:

```
python tools/make_atlas.py
python tools/make_item_sprites.py
godot --headless --path . --import
```

Refresh the Texture Studio's snapshot of the game: `godot --headless --path . res://tools/art_bundle.tscn`
(writes `out/art_bundle.json`), then ask Claude to republish the studio with it.
