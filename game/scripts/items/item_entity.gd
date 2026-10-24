class_name ItemEntity
extends Node3D

## A dropped stack lying in the world (ADR 0005): a small textured cube or the
## item's extruded sprite (a coloured card for items with neither) that falls, bobs, spins, merges with
## equal stacks nearby, is picked up by walking over it and despawns after
## Settings.item_despawn_s. Pure Minecraft behaviour; the matter inside is
## inert until placed back (then it obeys the material simulation).
##
## Moves against the voxel grid with a VoxelBody (ADR 0011), not a rigid
## body: gravity, Minecraft drag (air 0.98 / ground 0.6 x 0.98 per 20 Hz
## tick), a small bounce, and sleep once at rest (re-checked 4 times a second
## so mining the block underneath drops it).

const SIZE := 0.6 * Settings.BLOCK  # voxels (30 cm box; vanilla's is 0.25 m)
const VISUAL := 0.8 * Settings.BLOCK   # drawn 40 cm across: at 25 cm a flint on sand was hard to find (bug report 2026-10-24)
const BOUNCE := 0.15
const SLEEP_SPEED := 0.05 * Settings.BLOCK   # voxels/s

var stack := {}
var age := 0.0
var pickup_delay := Settings.item_pickup_delay_s
var items: ItemRegistry
var world: World          # voxel data for movement and the pop-out check
var velocity := Vector3.ZERO
var frozen := true        # held still right after spawning until the mined voxel reads as air
var sleeping := false
var _visual: MeshInstance3D
var _phase := 0.0
var _initial_velocity := Vector3.ZERO
var _unfreeze_at := 0.15
var _sleep_check := 0.0
var _body: VoxelBody


## `v` in blocks (0.5 m) per second, the unit every caller throws items in.
func set_initial_velocity(v: Vector3) -> void:
	_initial_velocity = v * Settings.BLOCK


static func create(p_items: ItemRegistry, p_stack: Dictionary, atlas_material: Material) -> ItemEntity:
	var e := ItemEntity.new()
	e.items = p_items
	e.stack = Inventory.make_stack(p_items, p_stack["id"], int(p_stack["count"]), p_stack)   # keeps freshness, wear, heat
	e.name = "Item_" + p_stack["id"]
	e._build(atlas_material)
	return e


func _build(atlas_material: Material) -> void:
	_body = VoxelBody.new(AABB(Vector3(-SIZE / 2.0, 0.0, -SIZE / 2.0), Vector3.ONE * SIZE))
	_visual = MeshInstance3D.new()
	var mat_id := ItemIcon.material_id_for(items, stack["id"])
	if ItemSprites.get_default().has(stack["id"]):
		_visual.mesh = BlockMeshes.sprite(stack["id"])
	elif mat_id > 0:
		_visual.mesh = BlockMeshes.cube(mat_id, atlas_material)
	else:
		_visual.mesh = BlockMeshes.card(stack["id"])
	_visual.scale = Vector3.ONE * VISUAL
	_visual.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(_visual)
	_phase = randf() * TAU


func _process(delta: float) -> void:
	age += delta
	if frozen and age >= _unfreeze_at:
		if world and _solid_at(position):
			_unfreeze_at = age + 0.1   # the broken voxel has not reached the terrain yet: wait
		else:
			frozen = false
			velocity = _initial_velocity
	_phase += delta
	_visual.position.y = VISUAL / 2.0 + (0.08 + sin(_phase * 2.0) * 0.06) * Settings.BLOCK   # mesh is centred, position is the box bottom
	_visual.rotation.y = _phase * 1.2


func _physics_process(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	_physics_step(delta)
	var m := get_parent().get_parent() if get_parent() != null else null
	if m != null and "phys_cost" in m:
		m.phys_cost["item_bodies"] = maxf(float(m.phys_cost.get("item_bodies", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)


func _physics_step(delta: float) -> void:
	if frozen or world == null:
		return
	# Minecraft pops items out of blocks they end up inside (a voxel placed or
	# simulated into them): move up to the first free voxel.
	if _solid_at(position):
		var p := position
		for _i in 8:
			p.y = floor(p.y) + 1.0
			if not _solid_at(p):
				position = p
				velocity = Vector3.ZERO
				sleeping = false
				return
		return
	if not world.is_area_loaded(Vector3i((position / Settings.CHUNK).floor())):
		return   # never fall into terrain that has not streamed in
	if sleeping:
		_sleep_check -= delta
		if _sleep_check > 0.0:
			return
		_sleep_check = 0.25
		_body.move(position, Vector3(0, -1.0, 0), 0.05, world.terrain)
		if _body.on_floor:
			return
		sleeping = false
	velocity.y -= Settings.m2v(Settings.gravity_m) * delta
	var got := _body.move(position, velocity, delta, world.terrain)
	position += got
	if _body.blocked.x > 0.0:
		velocity.x = 0.0
	if _body.blocked.z > 0.0:
		velocity.z = 0.0
	if _body.on_floor:
		velocity.y = -velocity.y * BOUNCE if velocity.y < -2.0 * Settings.BLOCK else 0.0
		var ground := pow(0.6 * 0.98, delta * 20.0)
		velocity.x *= ground
		velocity.z *= ground
		if velocity.length() < SLEEP_SPEED:
			velocity = Vector3.ZERO
			sleeping = true
			_sleep_check = 0.25
	else:
		if _body.hit_ceiling:
			velocity.y = 0.0
		var air := pow(0.98, delta * 20.0)
		velocity.x *= air
		velocity.z *= air


func _solid_at(p: Vector3) -> bool:
	var m := world.get_voxel(Vector3i(p.floor()))
	if m <= 0:
		return false
	var d: Dictionary = GameMaterials.defs()[m - 1]
	if d.has("micro") or d.get("cross", false):
		return false   # micro-voxel plants and work spots (ADR 0023) have no collision
	var ct: String = d.get("cell_type", "solid")
	return ct == "solid" or ct == "powder"


## Thrown from the player's hand: flies at once (the frozen wait is for a voxel just mined into air).
func throw() -> void:
	frozen = false
	velocity = _initial_velocity


func can_pickup() -> bool:
	return age >= pickup_delay


func expired() -> bool:
	return age >= Settings.item_despawn_s


func set_count(n: int) -> void:
	stack["count"] = n
