class_name ItemSprites
extends RefCounted

## 16x16 pixel-art item sprites from data/item_sprites.json: shared shapes
## (rows of colour keys), palettes per material, and per item a stack of
## layers (a copper pickaxe = handle + pickaxe head in the copper palette).
## Images and textures are built on first use and cached. Engine-agnostic,
## testable headless. Items without a sprite fall back to their block
## material's atlas tiles (ItemIcon, BlockMeshes).

const PATH := "res://data/item_sprites.json"
const SIZE := 16

static var _default: ItemSprites

var palettes := {}
var shapes := {}
var sprites := {}
var errors: Array[String] = []
var _images := {}
var _textures := {}


static func get_default() -> ItemSprites:
	if _default == null:
		_default = ItemSprites.new()
		var f := FileAccess.open(PATH, FileAccess.READ)
		if f == null:
			_default.errors.append("cannot open %s" % PATH)
		else:
			_default.load_json(f.get_as_text())
	return _default


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY:
		errors.append("item_sprites.json: expected an object")
		return false
	palettes = data.get("palettes", {})
	shapes = data.get("shapes", {})
	for sname in shapes:
		var rows: Array = shapes[sname].get("rows", [])
		if rows.size() != SIZE or rows.any(func(r): return String(r).length() != SIZE):
			errors.append("shape %s: needs %d rows of %d characters" % [sname, SIZE, SIZE])
	var raw: Dictionary = data.get("sprites", {})
	for id in raw:
		if _validate(id, raw[id]):
			sprites[id] = raw[id]
	return errors.is_empty()


func _validate(id: String, s) -> bool:
	if typeof(s) != TYPE_DICTIONARY or typeof(s.get("layers")) != TYPE_ARRAY or s["layers"].is_empty():
		errors.append("sprite %s: needs a non-empty layers list" % id)
		return false
	for layer in s["layers"]:
		var sname := String(layer.get("shape", ""))
		if not shapes.has(sname):
			errors.append("sprite %s: unknown shape '%s'" % [id, sname])
			return false
		if layer.has("palette") and not palettes.has(layer["palette"]):
			errors.append("sprite %s: unknown palette '%s'" % [id, layer["palette"]])
			return false
		var cols := _colors(layer)
		for row in shapes[sname]["rows"]:
			for ch in String(row):
				if ch != "." and not cols.has(ch):
					errors.append("sprite %s: colour '%s' of shape %s is not defined" % [id, ch, sname])
					return false
	return true


## Colour keys of one layer: palette, then the shape's own colours, then the layer's.
func _colors(layer: Dictionary) -> Dictionary:
	var cols := {}
	if layer.has("palette"):
		cols.merge(palettes[layer["palette"]], true)
	cols.merge(shapes[layer["shape"]].get("colors", {}), true)
	cols.merge(layer.get("colors", {}), true)
	return cols


func has(id: String) -> bool:
	return sprites.has(id)


## The sprite is a block face: drawn as a cube, not a flat item.
func is_cube(id: String) -> bool:
	return sprites.has(id) and bool(sprites[id].get("cube", false))


func image(id: String) -> Image:
	if _images.has(id):
		return _images[id]
	if not sprites.has(id):
		return null
	var img := Image.create(SIZE, SIZE, false, Image.FORMAT_RGBA8)
	img.fill(Color(0, 0, 0, 0))
	for layer in sprites[id]["layers"]:
		var cols := _colors(layer)
		var parsed := {}
		for k in cols:
			parsed[k] = Color.html(String(cols[k]))
		var rows: Array = shapes[layer["shape"]]["rows"]
		for y in SIZE:
			var row := String(rows[y])
			for x in SIZE:
				var ch := row[x]
				if ch != ".":
					img.set_pixel(x, y, parsed[ch])
	_images[id] = img
	return img


func texture(id: String) -> Texture2D:
	if _textures.has(id):
		return _textures[id]
	var img := image(id)
	if img == null:
		return null
	var t := ImageTexture.create_from_image(img)
	_textures[id] = t
	return t
