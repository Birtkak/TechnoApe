class_name WorldList
extends RefCounted

## The worlds the title screen lists (ADR 0032): user://worlds/index.json,
## one entry per world {name, seed, seed_text, game_mode, created,
## last_played}. A world's files are keyed by its seed and generator version
## (world_<seed>_g<gen>.sqlite, blocks_..., quests_..., player_...), so two
## worlds never share a seed. Saves made before the title screen existed are
## found by their files and listed as "World <seed>".

const DIR := "user://worlds"
const INDEX := "user://worlds/index.json"

var dir := DIR
var entries: Array = []


static func load_default() -> WorldList:
	var w := WorldList.new()
	w.load_index()
	return w


func _index_path() -> String:
	return dir + "/index.json"


func load_index() -> void:
	entries.clear()
	if FileAccess.file_exists(_index_path()):
		var d = JSON.parse_string(FileAccess.get_file_as_string(_index_path()))
		if d is Dictionary:
			for e in d.get("worlds", []):
				if e is Dictionary and e.has("seed"):
					e["seed"] = int(e["seed"])
					entries.append(e)
	_adopt_orphans()


## Older saves without an index entry.
func _adopt_orphans() -> void:
	var da := DirAccess.open(dir)
	if da == null:
		return
	var changed := false
	for f in da.get_files():
		if not (f.begins_with("world_") and f.ends_with(".sqlite")):
			continue
		var parts := f.trim_prefix("world_").trim_suffix(".sqlite").split("_g")
		if parts.size() != 2 or not parts[0].is_valid_int():
			continue
		var s := int(parts[0])
		if find_seed(s).is_empty():
			var t := FileAccess.get_modified_time(dir + "/" + f)
			entries.append({"name": "World %d" % s, "seed": s, "seed_text": str(s), "game_mode": "survival",
				"created": t, "last_played": t})
			changed = true
	if changed:
		save_index()


func save_index() -> void:
	DirAccess.make_dir_recursive_absolute(dir)
	var f := FileAccess.open(_index_path() + ".tmp", FileAccess.WRITE)
	if f == null:
		return
	f.store_string(JSON.stringify({"version": 1, "worlds": entries}, "  "))
	f.close()
	DirAccess.rename_absolute(_index_path() + ".tmp", _index_path())


## Newest first.
func sorted() -> Array:
	var out := entries.duplicate()
	out.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return int(a.get("last_played", 0)) > int(b.get("last_played", 0)))
	return out


func find_seed(s: int) -> Dictionary:
	for e in entries:
		if int(e["seed"]) == s:
			return e
	return {}


## A new world. An empty seed text picks a random unused seed. Returns the
## entry, or {"error": text} when the seed is taken or the name is empty.
func create(p_name: String, seed_text: String, game_mode: String) -> Dictionary:
	p_name = p_name.strip_edges()
	if p_name == "":
		return {"error": "Give the world a name."}
	seed_text = seed_text.strip_edges()
	var s := 0
	if seed_text == "":
		var rng := RandomNumberGenerator.new()
		rng.randomize()
		s = rng.randi() & 0x7FFFFFFF
		while not find_seed(s).is_empty():
			s = rng.randi() & 0x7FFFFFFF
		seed_text = str(s)
	else:
		s = Settings.seed_from_string(seed_text)
	var taken := find_seed(s)
	if not taken.is_empty():
		return {"error": "\"%s\" already uses this seed." % taken["name"]}
	var now := int(Time.get_unix_time_from_system())
	var e := {"name": p_name, "seed": s, "seed_text": seed_text, "game_mode": game_mode, "created": now, "last_played": now}
	entries.append(e)
	save_index()
	return e


func touch(e: Dictionary) -> void:
	e["last_played"] = int(Time.get_unix_time_from_system())
	save_index()


## The newest generator version this world has save files for, or -1 when it has none yet.
## Saves are keyed by generator version, so after a GEN_VERSION bump the world opens empty.
func saved_gen(e: Dictionary) -> int:
	var best := -1
	var da := DirAccess.open(dir)
	if da == null:
		return best
	var key := "_%d_g" % int(e["seed"])
	for f in da.get_files():
		var i := f.find(key)
		if i >= 0:
			var g := f.substr(i + key.length()).get_slice(".", 0)
			if g.is_valid_int():
				best = maxi(best, int(g))
	return best


## Deletes the world's entry and every save file of its seed (all generator versions).
func delete(e: Dictionary) -> int:
	var removed := 0
	var da := DirAccess.open(dir)
	if da != null:
		var key := "_%d_g" % int(e["seed"])
		for f in da.get_files():
			if f.contains(key) and f != "index.json":
				if da.remove(f) == OK:
					removed += 1
	entries.erase(e)
	save_index()
	return removed


## Points Settings at a world before the game scene loads.
static func select(e: Dictionary) -> void:
	Settings.set_seed_text(String(e.get("seed_text", str(e["seed"]))))
	Settings.world_seed = int(e["seed"])
	Settings.world_name = String(e["name"])
	Settings.game_mode = String(e.get("game_mode", "survival"))
	Settings.persist_world = true
