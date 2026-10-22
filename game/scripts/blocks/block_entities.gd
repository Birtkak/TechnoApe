class_name BlockEntities
extends RefCounted

## Block entities by voxel position (ADR 0007): the inventories and state
## that a voxel id alone cannot hold (chests, furnaces). Created when such a
## block is placed (material "entity" key), lazily when one is opened that
## has none (generated or debug-placed blocks), removed with their contents
## when the block breaks. Ticked at Minecraft's 20 ticks per second by Main.
## Saved as versioned JSON next to the world's SQLite file (save_file /
## load_file; Main writes it every 30 s while dirty and on exit).

const TICK_S := 0.05
const SAVE_VERSION := 1

var items: ItemRegistry
var recipes: RecipeRegistry
var _by_pos := {}            # Vector3i -> ChestEntity / FurnaceEntity
var _ticking: Array = []     # entities with state over time (furnaces)
var ticks := 0
var dirty := false           # something changed since the last save


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry) -> void:
	items = p_items
	recipes = p_recipes


func create(pos: Vector3i, kind: String) -> RefCounted:
	remove(pos)
	var e: RefCounted = null
	match kind:
		"chest":
			e = ChestEntity.new(items, pos)
		"furnace":
			e = FurnaceEntity.new(items, recipes, pos)
			_ticking.append(e)
		_:
			push_error("unknown block entity kind " + kind)
			return null
	_by_pos[pos] = e
	e.changed.connect(func() -> void: dirty = true)
	dirty = true
	return e


func at(pos: Vector3i) -> RefCounted:
	return _by_pos.get(pos)


func has(pos: Vector3i) -> bool:
	return _by_pos.has(pos)


func get_or_create(pos: Vector3i, kind: String) -> RefCounted:
	var e: RefCounted = _by_pos.get(pos)
	if e != null and e.kind == kind:
		return e
	return create(pos, kind)


## Removes the entity; returns its stacks (to drop as item entities).
func remove(pos: Vector3i) -> Array:
	var e: RefCounted = _by_pos.get(pos)
	if e == null:
		return []
	_by_pos.erase(pos)
	_ticking.erase(e)
	dirty = true
	return e.contents()


func size() -> int:
	return _by_pos.size()


## Advances every ticking entity by one tick. Returns the positions whose
## lit state flipped.
func tick() -> Array[Vector3i]:
	ticks += 1
	var flipped: Array[Vector3i] = []
	for e in _ticking:
		if e.tick():
			flipped.append(e.pos)
		if e.is_lit():
			dirty = true     # burn and cook progress move
	return flipped


# ---------------- persistence ----------------

func to_dict() -> Dictionary:
	var list := []
	for p in _by_pos:
		list.append(_by_pos[p].to_dict())
	return {"version": SAVE_VERSION, "entities": list}


## Replaces everything with the saved entities. Returns false on bad data.
func load_dict(d: Dictionary) -> bool:
	if int(d.get("version", 0)) != SAVE_VERSION:
		push_error("block entities: unsupported save version %s" % d.get("version"))
		return false
	_by_pos.clear()
	_ticking.clear()
	for ed in d.get("entities", []):
		var pa: Array = ed.get("pos", [])
		if pa.size() != 3:
			continue
		var e := create(Vector3i(int(pa[0]), int(pa[1]), int(pa[2])), String(ed.get("kind", "")))
		if e != null:
			e.load_from(ed)
	dirty = false
	return true


## Writes atomically (temp file + rename) so a crash never leaves half a save.
func save_file(path: String) -> bool:
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var tmp := path + ".tmp"
	var f := FileAccess.open(tmp, FileAccess.WRITE)
	if f == null:
		push_error("block entities: cannot write %s" % tmp)
		return false
	f.store_string(JSON.stringify(to_dict()))
	f.close()
	if DirAccess.rename_absolute(tmp, path) != OK:
		push_error("block entities: cannot rename %s" % tmp)
		return false
	dirty = false
	return true


func load_file(path: String) -> bool:
	if not FileAccess.file_exists(path):
		return false
	var data = JSON.parse_string(FileAccess.get_file_as_string(path))
	if not (data is Dictionary):
		push_error("block entities: %s is not valid JSON" % path)
		return false
	return load_dict(data)


func summary() -> Dictionary:
	var list := []
	for p in _by_pos:
		list.append(_by_pos[p].summary())
	return {"count": _by_pos.size(), "ticks": ticks, "entities": list.slice(0, 8)}
