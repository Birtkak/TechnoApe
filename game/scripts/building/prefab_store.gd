class_name PrefabStore
extends RefCounted

## Saved builds (ADR 0038): `<base>/<category>/<name>.json` holding {"version": 1, "size": [x, y, z],
## "palette": [material names, "" first for nothing], "runs": [[palette index, count], ...]} over the
## grid x fastest, then z, then y (ShapeTool's grid order). Names, not ids: a prefab outlives id
## changes; a material that no longer exists loads as nothing. The last commit is kept as LAST.

const VERSION := 1
const LAST_CATEGORY := "recent"
const LAST_NAME := "last build"

var base: String


func _init(p_base := "user://prefabs") -> void:
	base = p_base


## {size, palette, runs} for a grid of material ids (0 = nothing); the runs are native (ShapeTool).
static func encode(size: Vector3i, grid: PackedByteArray) -> Dictionary:
	var r := ShapeTool.new().runs(grid)
	var palette := [""]
	var index := {0: 0}
	var runs := []
	for k in range(0, r.size(), 2):
		var m := r[k]
		if not index.has(m):
			index[m] = palette.size()
			palette.append(GameMaterials.name_of(m))
		runs.append([index[m], r[k + 1]])
	return {"version": VERSION, "size": [size.x, size.y, size.z], "palette": palette, "runs": runs}


## {size: Vector3i, grid: PackedByteArray} from encode()'s form, or {} if it does not add up.
static func decode(d: Dictionary) -> Dictionary:
	var s: Array = d.get("size", [])
	if s.size() != 3:
		return {}
	var size := Vector3i(int(s[0]), int(s[1]), int(s[2]))
	var ids := []
	for nm in d.get("palette", []):
		ids.append(0 if nm == "" else GameMaterials.id_of(nm))
	var r := PackedInt32Array()
	for run in d.get("runs", []):
		var i := int(run[0])
		r.append(ids[i] if i >= 0 and i < ids.size() else 0)
		r.append(int(run[1]))
	var grid := ShapeTool.new().unruns(r)
	if grid.size() != size.x * size.y * size.z:
		return {}
	return {"size": size, "grid": grid}


func save(category: String, name: String, size: Vector3i, grid: PackedByteArray) -> String:
	var dir := base.path_join(_clean(category))
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(dir))
	var path := dir.path_join(_clean(name) + ".json")
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		return ""
	f.store_string(JSON.stringify(encode(size, grid)))
	return path


func load_prefab(category: String, name: String) -> Dictionary:
	var path := base.path_join(_clean(category)).path_join(_clean(name) + ".json")
	if not FileAccess.file_exists(path):
		return {}
	var d: Variant = JSON.parse_string(FileAccess.get_file_as_string(path))
	return decode(d) if d is Dictionary else {}


## {category: [names]}, sorted.
func list() -> Dictionary:
	var out := {}
	var root := DirAccess.open(base)
	if root == null:
		return out
	for cat in root.get_directories():
		var names := []
		var d := DirAccess.open(base.path_join(cat))
		for f in d.get_files():
			if f.ends_with(".json"):
				names.append(f.get_basename())
		names.sort()
		out[cat] = names
	return out


func save_last(size: Vector3i, grid: PackedByteArray) -> void:
	save(LAST_CATEGORY, LAST_NAME, size, grid)


static func _clean(s: String) -> String:
	var t := s.strip_edges().validate_filename()
	return t if t != "" else "unnamed"
