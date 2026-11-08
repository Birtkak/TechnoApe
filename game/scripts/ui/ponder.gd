class_name Ponder
extends RefCounted

## The guide scenes (ADR 0132): `data/ponder/<id>.json`, one short animated lesson each on how pieces
## connect, played by PonderScreen. A scene: {title, items: [item ids it is the guide for], steps: [...]}.
## A step keeps what the steps before it placed and adds its own: `text` (the caption), `place`
## [{kind, at, item?, facing?, set: {field: value}}], `remove` [at...], `blocks` [{material, at, size}],
## `mark` [{at, size, ok}] and `flow` [{from, to, kind}] (both only while the step shows), `seconds`.
## Positions and sizes are in 0.5 m blocks (ADR 0034) from the stage's origin, `at` the min corner.
## A flow may carry a `label` (IN, OUT, POWER) drawn over it; a scene's `recipes` (ids) show as an icon strip.
## A machine with process recipes gets its own scene, built from its item and recipes ("machine:<id>").

const DIR := "res://data/ponder"
const FLOWS := ["steam", "water", "power", "items"]

const MACHINE := "machine:"

static var _scenes := {}
static var items: ItemRegistry       # set by PonderScreen.setup (and tests): the machine scenes are built from them
static var recipes: RecipeRegistry


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
	if id.begins_with(MACHINE):
		return machine_scene(id.trim_prefix(MACHINE))
	return scenes().get(id, {})


## The scene that teaches `item_id` (its `items` list), or "".
static func for_item(item_id: String) -> String:
	if scenes().has(item_id):
		return item_id   # a scene named after the item wins (the coke oven's multiblock build)
	if not machine_scene(item_id).is_empty():
		return MACHINE + item_id
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
			var bb := AABB(Vector3(PonderScreen._v(b.get("at", [0, 0, 0])) * Settings.BLOCK), Vector3(PonderScreen._v(b.get("size", [1, 1, 1])) * Settings.BLOCK))
			for e in store.all_entities():   # a piece that grew (ADR 0168) must not sink into the scene's blocks
				var w := BlockEntities.width_of(e.kind)
				if bb.intersects(AABB(Vector3(e.pos), Vector3(w, BlockEntities.height_of(e.kind), w))):
					bad.append("step %d puts %s blocks inside the %s" % [i, String(b.get("material", "")), e.kind])
		for f in s.get("flow", []):
			if not String(f.get("kind", "")) in FLOWS:
				bad.append("step %d flows unknown kind %s" % [i, String(f.get("kind", ""))])
	return bad


## A machine's process recipes (ADR 0117): the ids MachineEntity runs.
static func process_recipes(kind: String) -> Array:
	var out := []
	if recipes == null or kind == "":
		return out
	for rid in recipes.by_station(kind):
		if recipes.recipes[rid].get("type", "") == "process":
			out.append(rid)
	return out


## The guide of a machine item with process recipes: the machine and what it does, its power by cable from a
## hand crank, then what goes in and what comes out (a hopper in from the side, the output out the front), every recipe in
## the strip. {} for anything else.
static func machine_scene(item_id: String) -> Dictionary:
	if items == null or not items.has(item_id):
		return {}
	var d := items.get_def(item_id)
	var kind := String(d.get("work", ""))
	var rs := process_recipes(kind)
	if not d.has("machine") or rs.is_empty():
		return {}
	var name := String(d.get("name", item_id))
	var w := BlockEntities.scale_of(kind)
	var h := BlockEntities.height_of(kind) / Settings.BLOCK
	var note := RegEx.create_from_string("ADR \\d+(, \\d+)*: ").sub(String(d.get("_note", "")), "", true)
	var watts := int(d["machine"].get("watts", 0))
	var inputs := int(d["machine"].get("inputs", 1))
	var at := [2, 0, 0]
	var size := [w, h, w]
	var steps := [{"text": ("The %s: %s." % [name, note]) if note != "" else "The %s." % name,
		"place": [{"kind": kind, "at": at, "facing": 0}], "mark": [{"at": at, "size": size}], "seconds": 5}]
	if watts > 0:
		steps.append({"text": "Power: it draws %d W at 48 V through the copper terminal on its back. Run a cable to it from a hand crank, a battery or a generator; a powered piece touching it needs no cable. No power, no work." % watts,
			"place": [{"kind": "hand_crank", "at": [2, 0, -3], "set": {"drive": 1.0}}, {"kind": "cable", "at": [2, 0, -2]}, {"kind": "cable", "at": [2, 0, -1]}],
			"flow": [{"from": [2, 0, -3], "to": [2, 0, 0], "kind": "power", "label": "POWER %d W" % watts}], "seconds": 6})
	var slots := (" (%d input slots: one per ingredient)" % inputs) if inputs > 1 else ""
	steps.append({"text": "Right click opens its screen: inputs left, output right, power on the side. IN: put the input in%s, or face a hopper into it. OUT: take it from the screen, or set a hopper under it. The strip below lists what it makes." % slots,
		"place": [{"kind": "hopper", "at": [1, 0, 0], "facing": 1}],   # on the side the stage camera faces
		"flow": [{"from": [0, 0, 0], "to": [2, 0, 0], "kind": "items", "label": "IN"}, {"from": [2, 0, w - 1], "to": [2, 0, w + 2], "kind": "items", "label": "OUT"}], "seconds": 9})
	return {"title": name + ": in, out, power", "items": [item_id], "recipes": rs, "steps": steps}


## "2 Iron Ingot + Coal  ->  2 Iron Rod (15 s)": a process recipe in words.
static func recipe_line(r: Dictionary) -> String:
	var counts := {}
	for spec in r.get("ingredients", []):
		counts[spec] = int(counts.get(spec, 0)) + 1
	var ins := PackedStringArray()
	for spec in counts:
		ins.append(("%d " % counts[spec] if counts[spec] > 1 else "") + _spec_name(String(spec)))
	var o: Dictionary = r.get("output", {})
	var n := int(o.get("count", 1))
	return "%s  ->  %s%s (%s s)" % [" + ".join(ins), "%d " % n if n > 1 else "", _spec_name(String(o.get("id", ""))), _num(float(r.get("seconds", 0.0)))]


static func _spec_name(spec: String) -> String:
	if spec.begins_with("#"):
		return "any " + spec.trim_prefix("#").replace("_", " ")
	return items.display_name(spec) if items != null else spec


static func _num(x: float) -> String:
	return str(int(x)) if x == floorf(x) else str(x)
