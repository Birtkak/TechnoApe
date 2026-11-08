class_name ItemRegistry
extends RefCounted

## Items loaded from data/items.json. Engine-agnostic (no nodes), testable headless.

var items := {}          # id -> Dictionary (the raw definition, validated)
var _by_tag := {}        # tag -> Array[String] of ids
var errors: Array[String] = []
var _units := {}         # id -> voxels per recipe unit, for items that place a plain terrain voxel


static func load_default() -> ItemRegistry:
	var r := ItemRegistry.new()
	r.load_file("res://data/items.json")
	r.load_file("res://data/stone_age_items.json")   # the four Stone Age stages (ADR 0059)
	r.load_file("res://data/lv_items.json")   # more of the electrical age (ADR 0131)
	r.load_file("res://data/fishing.json")   # fish species, snares and pit traps (ADR 0163)
	return r


func load_file(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(f.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("items"):
		errors.append("items.json: expected {items: [...]}")
		return false
	for d in data["items"]:
		if not _validate(d):
			continue
		items[d["id"]] = d
		# the dedicated server runs without the voxel extension, so GameMaterials does not compile there (its ledger needs no units)
		if d.has("material") and not d.has("empties_to") and ClassDB.class_exists("VoxelBlockyLibrary") and GameMaterials.is_plain_voxel(GameMaterials.id_of(String(d["material"]))):
			_units[d["id"]] = Settings.BLOCK * Settings.BLOCK * Settings.BLOCK
		for t in d.get("tags", []):
			if not _by_tag.has(t):
				_by_tag[t] = []
			_by_tag[t].append(d["id"])
	return errors.is_empty()


func _validate(d) -> bool:
	if typeof(d) != TYPE_DICTIONARY or not d.has("id") or not d.has("name"):
		errors.append("item without id/name: %s" % str(d))
		return false
	if items.has(d["id"]):
		if d.has("stub"):
			return false   # another part's item, already defined by that part (ADR 0059)
		errors.append("duplicate item id %s" % d["id"])
		return false
	if d.has("stack") and (int(d["stack"]) < 1 or int(d["stack"]) > 64):
		errors.append("item %s: stack must be 1..64" % d["id"])
		return false
	return true


## The item that places a material (pick block): the one named like the
## material first, else any placing it that is not a filled container.
func item_for_material(material_name: String) -> String:
	if items.has(material_name) and items[material_name].get("material", "") == material_name:
		return material_name
	var ids := items.keys()
	ids.sort()
	for id in ids:
		var d: Dictionary = items[id]
		if d.get("material", "") == material_name and not d.has("empties_to"):
			return id
	return ""


func has(id: String) -> bool:
	return items.has(id)


func get_def(id: String) -> Dictionary:
	return items.get(id, {})


func display_name(id: String) -> String:
	return items.get(id, {}).get("name", id)


## Stack size in counted units: a voxel item's stack holds `stack` blocks of voxels.
func max_stack(id: String) -> int:
	return int(items.get(id, {}).get("stack", 64)) * units(id)


## Voxels in one recipe unit (ADR 0034): an item that places a plain terrain
## voxel (dirt, cobblestone, planks, logs, ores) is counted in 12.5 cm voxels,
## and recipes, smelting and fuel use it in blocks of BLOCK^3 voxels (0.5 m);
## every other item counts in pieces (1).
func units(id: String) -> int:
	return int(_units.get(id, 1))


func is_voxel_item(id: String) -> bool:
	return _units.has(id)


func has_tag(id: String, tag: String) -> bool:
	return items.has(id) and tag in items[id].get("tags", [])


func items_with_tag(tag: String) -> Array:
	return _by_tag.get(tag, []).duplicate()


## True when `id` satisfies an ingredient spec: an item id, or "#tag".
func matches(id: String, spec: String) -> bool:
	if spec.begins_with("#"):
		return has_tag(id, spec.substr(1))
	return id == spec


func all_ids() -> Array:
	return items.keys()


func search(text: String) -> Array:
	var q := text.to_lower()
	var out := []
	for id in items:
		var d: Dictionary = items[id]
		if q in id or q in String(d["name"]).to_lower() or q in " ".join(d.get("tags", [])):
			out.append(id)
	out.sort()
	return out
