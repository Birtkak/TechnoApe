extends Node

## Snapshot of the game's textures for the Texture Studio artifact (game/art/README.md): the block atlas,
## every item sprite as a 16x16 PNG and the material -> tile table, as one JSON.
##   godot --headless --path game res://tools/art_bundle.tscn -- out=<file.json>
## (a scene, not --script: the model code needs the Settings autoload)


func _ready() -> void:
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
	var models := _models()
	f.store_string(JSON.stringify({"atlas": Marshalls.raw_to_base64(atlas.save_png_to_buffer()), "cols": 8, "materials": mats, "items": items,
		"models": models, "palettes": {"work": WorkPalette.PALETTE, "mob": MobModel.PALETTE, "ape": ApeModel.PALETTE}, "paints": ArtModels.PAINTS}))
	f.close()
	print("art_bundle items=%d materials=%d models=%d -> %s" % [items.size(), mats.size(), models.size(), out])
	get_tree().quit(0)



## Every model part the ArtModels hook covers, as {group, palette, size, layers}: work pieces in their
## fresh state facing 0, every animal part, the ape's body. A name with a file in data/art_models keeps
## that file's layers (its "*" cells included).
func _models() -> Dictionary:
	var out := {}
	var items := ItemRegistry.load_default()
	var store := BlockEntities.new(items, RecipeRegistry.load_default(items))
	var at := 0
	for kind in BlockEntities.WORK_KINDS + StoneAge.KINDS:
		at += 64
		var e: RefCounted = store.create(Vector3i(at, 0, 0), kind)
		if e != null and e.has_method("view_parts"):
			var solid: Dictionary = e.view_parts().get("solid", {})
			if not solid.is_empty():
				_add(out, "work_" + kind, "Machines and work pieces", "work", solid["cells"], solid["size"])
	var species := {"sheep": ["body", "body_shorn", "head", "leg", "leg_shorn"], "pig": ["body", "head", "leg"],
		"chicken": ["body", "head", "leg", "wing"], "bear": ["body", "head", "leg"]}
	for sp in WildModels.LAYOUT:
		species[sp] = WildModels.parts(sp)
	for sp in species:
		for part in species[sp]:
			var b: Array = MobModel._build(sp, part)
			_add(out, "mob_%s_%s" % [sp, part], "Animals", "mob", b[0], b[1])
	for p in [["head", ApeModel._head_cells(), Vector3i(20, 16, 17)], ["torso", ApeModel._torso_cells(), Vector3i(14, 24, 9)],
			["arm_r", ApeModel._arm_cells(true), Vector3i(6, 22, 6)], ["arm_l", ApeModel._arm_cells(false), Vector3i(6, 22, 6)],
			["leg_r", ApeModel._leg_cells(), Vector3i(6, 24, 7)], ["leg_l", ApeModel._leg_cells(), Vector3i(6, 24, 7)],
			["tail", ApeModel._tail_cells(), Vector3i(4, 20, 14)]]:
		_add(out, "ape_" + p[0], "Player (ape)", "ape", p[1], p[2])
	return out


func _add(out: Dictionary, name: String, group: String, palette: String, c: PackedInt32Array, size: Vector3i) -> void:
	if c.count(0) == c.size():
		return   # an empty piece (a knapping stone before its stone): nothing to rebuild
	var path: String = ArtModels.DIR + name + ".json"
	if FileAccess.file_exists(path):
		var d: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(path))
		out[name] = {"group": group, "palette": palette, "size": d["size"], "layers": d["layers"], "edited": true}
		return
	var layers := []
	for y in size.y:
		var rows := []
		for z in size.z:
			var r := ""
			for x in size.x:
				var v := c[x + size.x * (y + size.y * z)]
				r += "." if v <= 0 else ArtModels.PAINTS[v - 1]
			rows.append(r)
		layers.append(rows)
	out[name] = {"group": group, "palette": palette, "size": [size.x, size.y, size.z], "layers": layers}
