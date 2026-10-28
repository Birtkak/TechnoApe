extends SceneTree

## Snapshot of the game's textures for the Texture Studio artifact (game/art/README.md): the block atlas,
## every item sprite as a 16x16 PNG and the material -> tile table, as one JSON.
##   godot --headless --path game --script res://tools/art_bundle.gd -- out=<file.json>


func _init() -> void:
	var out := "res://out/art_bundle.json"
	for a in OS.get_cmdline_user_args():
		if a.begins_with("out="):
			out = a.substr(4)
	var sprites := ItemSprites.get_default()
	var mats := []
	for d in GameMaterials.defs():
		mats.append({"name": d["name"], "tiles": d.get("tiles", [0, 0, 0]), "pattern": d.get("pattern", false), "model": d.get("model", d.get("micro", ""))})
	var items := []
	var data: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/items.json"))
	for it in data["items"]:
		var id: String = it["id"]
		var png = null
		if sprites.has(id):
			png = Marshalls.raw_to_base64(sprites.image(id).save_png_to_buffer())
		items.append({"id": id, "name": it.get("name", id), "material": it.get("material"), "sprite": png, "cube": sprites.has(id) and sprites.is_cube(id)})
	var atlas: Image = (load("res://assets/textures/atlas.png") as Texture2D).get_image()
	var f := FileAccess.open(out, FileAccess.WRITE)
	f.store_string(JSON.stringify({"atlas": Marshalls.raw_to_base64(atlas.save_png_to_buffer()), "cols": 8, "materials": mats, "items": items}))
	f.close()
	print("art_bundle items=%d materials=%d -> %s" % [items.size(), mats.size(), out])
	quit(0)
