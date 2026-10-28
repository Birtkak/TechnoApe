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

Any 16x16 PNG works (Aseprite, Pixelorama, Libresprite, Paint.NET). The Texture Studio artifact shows
the current look, paints, previews in 3D the way the game draws it, and exports a zip laid out as this
folder: unzip it into `game/`.

Apply:

```
python tools/make_atlas.py
python tools/make_item_sprites.py
godot --headless --path . --import
```

Refresh the Texture Studio's snapshot of the game: `godot --headless --path . --script res://tools/art_bundle.gd`
(writes `out/art_bundle.json`), then ask Claude to republish the studio with it.
