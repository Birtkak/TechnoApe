class_name BlockEntities
extends RefCounted

## Block entities by voxel position (ADR 0007): the inventories and state
## that a voxel id alone cannot hold (chests, furnaces). Created when such a
## block is placed (material "entity" key), lazily when one is opened that
## has none (generated or debug-placed blocks), removed with their contents
## when the block breaks. Ticked at Minecraft's 20 ticks per second by Main.
## Not saved yet: world edits are not persisted either (see ADR 0007).

const TICK_S := 0.05

var items: ItemRegistry
var recipes: RecipeRegistry
var _by_pos := {}            # Vector3i -> ChestEntity / FurnaceEntity
var _ticking: Array = []     # entities with state over time (furnaces)
var ticks := 0


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
	return flipped


func summary() -> Dictionary:
	var list := []
	for p in _by_pos:
		list.append(_by_pos[p].summary())
	return {"count": _by_pos.size(), "ticks": ticks, "entities": list.slice(0, 8)}
