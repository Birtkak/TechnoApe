class_name ItemEntity
extends RigidBody3D

## A dropped stack lying in the world (ADR 0005): a tiny textured cube (or a
## coloured card for non-block items) that falls, bobs, spins, merges with
## equal stacks nearby, is picked up by walking over it and despawns after
## Settings.item_despawn_s. Pure Minecraft behaviour; the matter inside is
## inert until placed back (then it obeys the material simulation).

var stack := {}
var age := 0.0
var items: ItemRegistry
var _visual: MeshInstance3D
var _phase := 0.0
var _initial_velocity := Vector3.ZERO
var world: World          # to detect being inside a solid voxel (mesh/collider lag) and pop out upwards
var _unfreeze_at := 0.15


func set_initial_velocity(v: Vector3) -> void:
	_initial_velocity = v


static func create(p_items: ItemRegistry, p_stack: Dictionary, atlas_material: Material) -> ItemEntity:
	var e := ItemEntity.new()
	e.items = p_items
	e.stack = {"id": p_stack["id"], "count": int(p_stack["count"])}
	e.name = "Item_" + p_stack["id"]
	e._build(atlas_material)
	return e


func _build(atlas_material: Material) -> void:
	mass = 0.2
	lock_rotation = true
	collision_layer = 4          # items: collide with the terrain (layer 1) only, never with the player or each other
	collision_mask = 1
	freeze = true                # let the mined voxel's collider update before physics touches the item
	gravity_scale = Settings.m2v(9.8) / 9.8   # realistic 9.8 m/s^2 in voxel units
	continuous_cd = true
	linear_damp = 0.5
	physics_material_override = PhysicsMaterial.new()
	physics_material_override.friction = 0.9
	physics_material_override.bounce = 0.1
	var shape := CollisionShape3D.new()
	var box := BoxShape3D.new()
	box.size = Vector3.ONE * 0.5
	shape.shape = box
	add_child(shape)
	_visual = MeshInstance3D.new()
	var mat_id := ItemIcon.material_id_for(items, stack["id"])
	if mat_id > 0:
		_visual.mesh = BlockMeshes.cube(mat_id, atlas_material)
		_visual.scale = Vector3.ONE * 0.5
	else:
		_visual.mesh = BlockMeshes.card(stack["id"])
		_visual.scale = Vector3.ONE * 0.5
	_visual.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(_visual)
	_phase = randf() * TAU


func _process(delta: float) -> void:
	age += delta
	if freeze and age >= _unfreeze_at:
		if world and _solid_at(position):
			_unfreeze_at = age + 0.1   # the broken voxel has not been pasted/remeshed yet: wait
		else:
			freeze = false
			linear_velocity = _initial_velocity
	_phase += delta
	_visual.position.y = 0.08 + sin(_phase * 2.0) * 0.06
	_visual.rotation.y = _phase * 1.2


## Minecraft pops items out of blocks they end up inside (our colliders lag
## the voxel data by a remesh); move up to the first free voxel.
func _physics_process(_delta: float) -> void:
	if freeze or world == null or not _solid_at(position):
		return
	var p := position
	for _i in 8:
		p.y = floor(p.y) + 1.25
		if not _solid_at(p):
			position = p
			linear_velocity = Vector3.ZERO
			return


func _solid_at(p: Vector3) -> bool:
	var m := world.get_voxel(Vector3i(p.floor()))
	if m <= 0:
		return false
	var d: Dictionary = GameMaterials.defs()[m - 1]
	# cross-quad vegetation has no collision; liquids and gases do not hold items either
	return not d.get("cross", false) and d.get("cell_type", "solid") != "gas" and (d.get("cell_type", "solid") != "liquid" or d.get("liquid_sand", false))


func can_pickup() -> bool:
	return age >= Settings.item_pickup_delay_s


func expired() -> bool:
	return age >= Settings.item_despawn_s


func set_count(n: int) -> void:
	stack["count"] = n
