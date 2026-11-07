class_name RackEntity
extends RefCounted

## A drying rack (ADR 0059): a frame of sticks and cordage with RACK_SLOTS hooks. A right click
## with meat or fish ("rack" recipes) hangs one; it dries over its dry recipe's game hours, or smokes
## over the smoke recipe's (shorter) hours while a lit firepit stands right beside or under the
## rack. Progress is a fraction that moves with whichever mode applies each tick, so a fire going
## out half way just slows it down. An empty hand takes the first finished piece (or, with none
## finished, the last one hung).

signal changed
signal cured(item: String, mode: String)   # a piece finished: the progression book counts it

const RACK_SLOTS := 4
## The three pieces this class runs (ADR 0162): hooks, speed (x the recipe's hours), and the mode its
## "active" flag turns on. The smokehouse holds three times as many and smokes in two thirds of the time;
## the salt pan evaporates brine, only while the sun is on it.
const SPECS := {
	"rack": {"slots": 4, "speed": 1.0, "modes": ["dry", "smoke"], "hint": "Hang raw meat or fish on the rack. Over a lit firepit it smokes."},
	"smokehouse": {"slots": 12, "speed": 1.5, "modes": ["dry", "smoke"], "hint": "Hang raw meat or fish inside. With a lit firepit beside it, it fills with smoke."},
	"salt_pan": {"slots": 2, "speed": 1.0, "modes": ["evaporate"], "hint": "Pour a pot of brine into the pan. The sun leaves salt."},
}

var kind := "rack"
var pos: Vector3i
var recipes: RecipeRegistry
var hung: Array = []          # {id, p: 0..1, done: output id or ""}
var smoky := false            # set by StoneAge once a second: a lit firepit next to or under it
var _told := false            # the hang hint was shown
var _last := -1.0             # StoneAge.now_days at the last tick


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i, p_kind: String = "rack") -> void:
	recipes = p_recipes
	pos = p_pos
	kind = p_kind


func is_lit() -> bool:
	return false


func _recipe(id: String, mode: String) -> Dictionary:
	for r in recipes.of_type("rack"):
		if r.get("mode", "") == mode and recipes.items.matches(id, String(r["input"])):
			return r
	return {}


func hangs(id: String) -> bool:
	for m in SPECS[kind]["modes"]:
		if not _recipe(id, m).is_empty():
			return true
	return false


## Right click with `held`. Returns {consume, give, msg}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "give": [], "msg": ""}
	if id == "":
		if hung.is_empty():
			res["msg"] = SPECS[kind]["hint"]
			return res
		var k := hung.size() - 1
		for i in hung.size():
			if hung[i]["done"] != "":
				k = i
				break
		var h: Dictionary = hung[k]
		hung.remove_at(k)
		res["give"] = [{"id": h["done"] if h["done"] != "" else h["id"], "count": 1}]
		changed.emit()
		return res
	if not hangs(id):
		res["msg"] = "That does not go here." if kind == "salt_pan" else "Only raw meat and fish hang on a rack."
		return res
	if hung.size() >= int(SPECS[kind]["slots"]):
		res["msg"] = "It is full."
		return res
	hung.append({"id": id, "p": 0.0, "done": ""})
	res["consume"] = 1
	if not _told:
		_told = true   # once per piece: five hooks do not say it five times
		res["msg"] = SPECS[kind]["hint"] if kind == "salt_pan" else ("Hung up to smoke." if smoky else "Hung up to dry. Over a lit firepit it would smoke, faster and keeping longer.")
	changed.emit()
	return res


func tick() -> bool:
	var now := StoneAge.now_days
	if _last < 0.0 or now < _last or now - _last > 3.0:
		_last = now   # first tick, or the date jumped (/time): nothing moves
		return false
	var dh := (now - _last) * 24.0
	if dh < 0.01:
		return false   # a hundredth of an hour at a time (3 s at the 20-minute day)
	_last = now
	var mode := "smoke" if smoky else "dry"
	if kind == "salt_pan":
		mode = "evaporate" if smoky else ""   # smoky = the sun is on it
		dh = 0.0 if mode == "" else dh
	dh *= float(SPECS[kind]["speed"])
	var any := false
	for h in hung:
		if h["done"] != "":
			continue
		var r := _recipe(h["id"], mode)
		if r.is_empty() and kind != "salt_pan":
			r = _recipe(h["id"], "dry")
		if r.is_empty():
			continue
		h["p"] = float(h["p"]) + dh / float(r["hours"])
		if float(h["p"]) >= 0.9999:   # (hours summed in float steps)
			h["done"] = r["output"]["id"]
			any = true
			cured.emit(h["done"], String(r["mode"]))
	if any:
		changed.emit()
	return false


func contents() -> Array:
	var out := []
	for h in hung:
		out.append({"id": h["done"] if h["done"] != "" else h["id"], "count": 1})
	return out


## Two posts and a crossbar of sticks, the pieces hanging from it as upright sprites.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	if kind != "rack":
		return _house_parts(c, size)
	WorkPalette.box(c, size, Vector3i(0, 0, 3), Vector3i(1, 8, 4), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(7, 0, 3), Vector3i(8, 8, 4), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(0, 7, 3), Vector3i(8, 8, 4), WorkPalette.WOOD_DARK)
	var laid := []
	for i in hung.size():
		var h: Dictionary = hung[i]
		laid.append({"id": h["done"] if h["done"] != "" else h["id"], "at": Vector3(0.2 + 0.2 * i, 0.62, 0.5), "upright": true, "scale": 0.3})
	return {"solid": {"cells": c, "size": size}, "items": laid}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "hung": hung.duplicate(true)}


func load_from(d: Dictionary) -> void:
	hung.clear()
	for h in d.get("hung", []):
		if h is Dictionary and recipes.items.has(String(h.get("id", ""))):
			hung.append({"id": String(h["id"]), "p": float(h.get("p", 0.0)), "done": String(h.get("done", ""))})


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "smoky": smoky, "hung": hung.duplicate(true)}


## The smokehouse: a log box with a sooty roof, the pieces hanging inside; the salt pan: a shallow clay tray,
## white when salt is left.
func _house_parts(c: PackedInt32Array, size: Vector3i) -> Dictionary:
	if kind == "salt_pan":
		WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 1, 8), WorkPalette.CLAY)
		WorkPalette.box(c, size, Vector3i(0, 1, 0), Vector3i(8, 2, 1), WorkPalette.CLAY)
		WorkPalette.box(c, size, Vector3i(0, 1, 7), Vector3i(8, 2, 8), WorkPalette.CLAY)
		WorkPalette.box(c, size, Vector3i(0, 1, 1), Vector3i(1, 2, 7), WorkPalette.CLAY)
		WorkPalette.box(c, size, Vector3i(7, 1, 1), Vector3i(8, 2, 7), WorkPalette.CLAY)
		var done := false
		for h in hung:
			done = done or h["done"] != ""
		var laid := []
		if not hung.is_empty():
			laid.append({"id": "salt_pot" if done else "brine_pot", "at": Vector3(0.5, 0.2, 0.5), "scale": 0.5})
		return {"solid": {"cells": c, "size": size}, "items": laid}
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(1, 7, 8), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(7, 0, 0), Vector3i(8, 7, 8), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(1, 0, 7), Vector3i(7, 7, 8), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(0, 7, 0), Vector3i(8, 8, 8), WorkPalette.WOOD_DARK)
	var laid := []
	for i in hung.size():
		var h: Dictionary = hung[i]
		laid.append({"id": h["done"] if h["done"] != "" else h["id"], "at": Vector3(0.2 + 0.2 * (i % 3), 0.55, 0.25 + 0.25 * (i / 3 % 3)), "upright": true, "scale": 0.25})
	return {"solid": {"cells": c, "size": size}, "items": laid}
