class_name BlockEntities
extends RefCounted

## Block entities by voxel position (ADR 0007): the inventories and state
## that a voxel id alone cannot hold (chests, furnaces, beds and chairs, work
## pieces). Every entity covers a footprint of 12.5 cm voxels (ADR 0034): a
## BLOCK^3 cube with its anchor (pos) at the min corner, furniture its own
## cells, a torch its one voxel; every covered voxel maps to the anchor through
## _cells, so at() finds the entity from any of them. Created when such a
## block is placed (material "entity" key), lazily when one is opened that
## has none (generated or debug-placed blocks), removed with their contents
## when the block breaks. Ticked at Minecraft's 20 ticks per second by Main.
## Saved as versioned JSON next to the world's SQLite file (save_file /
## load_file; Main writes it every 30 s while dirty and on exit).

const TICK_S := 0.05
const SAVE_VERSION := 1

var items: ItemRegistry
var recipes: RecipeRegistry
var _by_pos := {}            # Vector3i -> ChestEntity / FurnaceEntity / FurnitureEntity / work pieces
var _ticking: Array = []     # entities with state over time (furnaces, firepits, pit kilns, molds)
var _cells := {}             # Vector3i -> anchor of the entity whose footprint covers it
var ticks := 0
var dirty := false           # something changed since the last save


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry) -> void:
	items = p_items
	recipes = p_recipes


func create(pos: Vector3i, kind: String) -> RefCounted:
	remove(pos)
	var e: RefCounted = null
	match kind:
		"chest", "storage_pot":
			e = ChestEntity.new(items, pos, kind)
		"torch":
			e = TorchEntity.new(pos)
		"station":
			e = StationEntity.new(pos)
		"furnace":
			e = FurnaceEntity.new(items, recipes, pos)
			_ticking.append(e)
		"furniture":
			e = FurnitureEntity.new(pos)
		# Vintage Story work pieces (ADR 0023): drawn by Main from view_parts()
		"knap":
			e = KnapEntity.new(pos)
		"clayform":
			e = ClayFormEntity.new(pos)
		"ground":
			e = GroundEntity.new(recipes, pos)
			_ticking.append(e)
		"firepit":
			e = FirepitEntity.new(items, recipes, pos)
			_ticking.append(e)
		"sieve":
			e = SieveEntity.new(recipes, pos)
		"firewood_pile":
			e = FirewoodPileEntity.new(recipes, pos)
			_ticking.append(e)
		# crafting in the world (ADR 0042)
		"assembly":
			e = AssemblyEntity.new(recipes, pos)
		"anvil":
			e = AnvilEntity.new(recipes, pos)
			_ticking.append(e)
		"grave":
			e = GraveEntity.new(items, pos)   # what a dead player carried (plan item D12)
		_:
			push_error("unknown block entity kind " + kind)
			return null
	_by_pos[pos] = e
	_register_cells(e)
	e.changed.connect(_mark_dirty)
	dirty = true
	return e


const SINGLE := ["torch"]   # entities that are one voxel
## Flat pieces are lower than a block, so they never hide the ground behind them from the aim ray:
## a knapping stone or a mold on the ground is one voxel high, a firepit two.
const HEIGHT := {"knap": 1, "ground": 1, "firepit": 2, "assembly": 1}


## The voxels an entity covers (ADR 0034).
static func footprint(e: RefCounted) -> Array[Vector3i]:
	if e is FurnitureEntity:
		return e.cells
	if e.kind in SINGLE:
		return [e.pos]
	return cube(e.pos, height_of(e.kind))


## Footprint height in voxels of a block entity kind.
static func height_of(p_kind: String) -> int:
	return int(HEIGHT.get(p_kind, Settings.BLOCK))


## BLOCK x height x BLOCK voxels from `anchor` (min corner): a station's or work piece's footprint.
static func cube(anchor: Vector3i, height: int = Settings.BLOCK) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var n := Settings.BLOCK
	for z in n:
		for y in height:
			for x in n:
				out.append(anchor + Vector3i(x, y, z))
	return out


## Where a block-sized thing goes when placed against a face (ADR 0034): x and z
## snap to the block grid, y is the voxel in front of the face, so it stands on
## the real ground.
static func anchor_for(front: Vector3i) -> Vector3i:
	var n := Settings.BLOCK
	return Vector3i(floori(float(front.x) / n) * n, front.y, floori(float(front.z) / n) * n)


func _mark_dirty() -> void:
	dirty = true


## A bed or chair anchored at `anchor`; registers every voxel it covers.
func create_furniture(anchor: Vector3i, furniture: String, facing: int) -> FurnitureEntity:
	remove(anchor)
	var e := FurnitureEntity.new(anchor, furniture, facing)
	_by_pos[anchor] = e
	_register_cells(e)
	e.changed.connect(_mark_dirty)
	dirty = true
	return e


func _register_cells(e: RefCounted) -> void:
	for c in footprint(e):
		_cells[c] = e.pos


## The furniture covering voxel p, or null.
func furniture_at(p: Vector3i) -> FurnitureEntity:
	var a = _cells.get(p)
	return _by_pos.get(a) as FurnitureEntity if a != null else null


func all_furniture() -> Array:
	var out := []
	for p in _by_pos:
		if _by_pos[p] is FurnitureEntity:
			out.append(_by_pos[p])
	return out


const WORK_KINDS := ["knap", "clayform", "ground", "firepit", "sieve", "firewood_pile", "assembly", "anvil", "grave"]


func all_of(p_kind: String) -> Array:
	var out := []
	for p in _by_pos:
		if _by_pos[p].kind == p_kind:
			out.append(_by_pos[p])
	return out


static func is_work(e: RefCounted) -> bool:
	return e != null and e.kind in WORK_KINDS


func all_work() -> Array:
	var out := []
	for p in _by_pos:
		if is_work(_by_pos[p]):
			out.append(_by_pos[p])
	return out


## The entity whose footprint covers voxel `pos`, or null.
func at(pos: Vector3i) -> RefCounted:
	return _by_pos.get(_cells.get(pos, pos))


func has(pos: Vector3i) -> bool:
	return at(pos) != null


func get_or_create(pos: Vector3i, kind: String) -> RefCounted:
	var e: RefCounted = _by_pos.get(pos)
	if e != null and e.kind == kind:
		return e
	return create(pos, kind)


## Removes the entity; returns its stacks (to drop as item entities).
func remove(pos: Vector3i) -> Array:
	if _cells.has(pos):
		pos = _cells[pos]   # any voxel of a bed or chair removes all of it
	var e: RefCounted = _by_pos.get(pos)
	if e == null:
		return []
	for c in footprint(e):
		if _cells.get(c) == pos:
			_cells.erase(c)
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
	_cells.clear()
	for ed in d.get("entities", []):
		var pa: Array = ed.get("pos", [])
		if pa.size() != 3:
			continue
		var e := create(Vector3i(int(pa[0]), int(pa[1]), int(pa[2])), String(ed.get("kind", "")))
		if e != null:
			if e is KnapEntity or e is ClayFormEntity:
				e.load_from(ed, recipes)   # they keep their recipe by id
			else:
				e.load_from(ed)
			_register_cells(e)   # furniture knows its cells only after load_from
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
