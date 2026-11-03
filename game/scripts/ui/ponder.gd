class_name Ponder
extends RefCounted

## The guide scenes (ADR 0132): `data/ponder/<id>.json`, one short animated lesson each on how pieces
## connect, played by PonderScreen. A scene: {title, items: [item ids it is the guide for], steps: [...]}.
## A step keeps what the steps before it placed and adds its own: `text` (the caption), `place`
## [{kind, at, item?, facing?, set: {field: value}}], `remove` [at...], `blocks` [{material, at, size}],
## `mark` [{at, size, ok}] and `flow` [{from, to, kind}] (both only while the step shows), `seconds`.
## Positions and sizes are in 0.5 m blocks (ADR 0034) from the stage's origin, `at` the min corner.

const DIR := "res://data/ponder"
const FLOWS := ["steam", "water", "power", "items"]

static var _scenes := {}


## Every scene by id, loaded once (file name = id).
static func scenes() -> Dictionary:
	if not _scenes.is_empty():
		return _scenes
	var d := DirAccess.open(DIR)
	if d == null:
		return _scenes
	var files := d.get_files()
	files.sort()
	for f in files:
		if not f.ends_with(".json"):
			continue
		var text := FileAccess.get_file_as_string(DIR + "/" + f)
		var parsed = JSON.parse_string(text)
		if parsed is Dictionary:
			_scenes[f.get_basename()] = parsed
		else:
			push_error("ponder: %s is not a JSON object" % f)
	return _scenes


static func get_scene(id: String) -> Dictionary:
	return scenes().get(id, {})


## The scene that teaches `item_id` (its `items` list), or "".
static func for_item(item_id: String) -> String:
	for id in scenes():
		if item_id in Array(scenes()[id].get("items", [])):
			return id
	return ""


## What is wrong with a scene: kinds no item places, unknown items and materials, fields an entity
## lacks, bad flow kinds, missing captions. Empty when it is sound (test_ponder.gd runs it on every file).
static func check(scene: Dictionary, items: ItemRegistry, recipes: RecipeRegistry) -> PackedStringArray:
	var bad := PackedStringArray()
	var kinds := {}
	for id in items.all_ids():
		var w := String(items.get_def(id).get("work", ""))
		if w != "":
			kinds[w] = true
	if String(scene.get("title", "")) == "":
		bad.append("no title")
	for it in scene.get("items", []):
		if not items.has(String(it)):
			bad.append("unknown item " + String(it))
	var steps: Array = scene.get("steps", [])
	if steps.size() < 2:
		bad.append("fewer than two steps")
	var store := BlockEntities.new(items, recipes)
	for i in steps.size():
		var s: Dictionary = steps[i]
		if String(s.get("text", "")) == "":
			bad.append("step %d has no text" % i)
		for a in s.get("remove", []):
			store.remove(PonderScreen._v(a) * Settings.BLOCK)
		for p in s.get("place", []):
			var kind := String(p.get("kind", ""))
			if not kinds.has(kind):
				bad.append("step %d places unknown kind %s" % [i, kind])
				continue
			if p.has("item") and not items.has(String(p["item"])):
				bad.append("step %d names unknown item %s" % [i, String(p["item"])])
			var e: RefCounted = store.create(PonderScreen._v(p.get("at", [0, 0, 0])) * Settings.BLOCK, kind)
			for k in p.get("set", {}):
				if not (String(k) in e):
					bad.append("step %d sets %s.%s which it lacks" % [i, kind, String(k)])
		for st in s.get("set", []):
			var e: RefCounted = store.at(PonderScreen._v(st.get("at", [0, 0, 0])) * Settings.BLOCK)
			if e == null:
				bad.append("step %d sets a piece that is not at %s" % [i, str(st.get("at"))])
				continue
			for k in st.get("set", {}):
				if not (String(k) in e):
					bad.append("step %d sets %s.%s which it lacks" % [i, e.kind, String(k)])
		for b in s.get("blocks", []):
			if GameMaterials.id_of(String(b.get("material", ""))) <= 0:
				bad.append("step %d uses unknown material %s" % [i, String(b.get("material", ""))])
		for f in s.get("flow", []):
			if not String(f.get("kind", "")) in FLOWS:
				bad.append("step %d flows unknown kind %s" % [i, String(f.get("kind", ""))])
	return bad
