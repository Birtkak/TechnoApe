class_name RemotePlayers
extends Node3D

## The other players' avatars (ADR 0145): an ApeModel per peer that eases toward the last state the
## server relayed (20 Hz) and animates by the sent speed. Its look (ADR 0167: held item, worn clothing,
## stains, the ape colour the server gave it) comes on change.

const EASE := 12.0   # per second: ~80 ms to close most of the gap, one state interval


class Avatar extends Node3D:
	var model: ApeModel
	var pname := ""
	var target := Vector3.ZERO
	var yaw := 0.0
	var pitch := 0.0
	var speed := 0.0
	var flying := false
	var fresh := true
	var pet: Node3D          # the owner's companion as a puppet (ADR 0152): their client simulates it
	var pet_model: MobModel
	var pet_tag: Label3D
	var pet_target := Vector3.ZERO
	var pet_yaw := 0.0
	var pet_speed := 0.0
	var pet_sit := false
	var held := ""
	var colour := 0
	var held_mesh: MeshInstance3D   # in the right hand
	var worn := {}
	var stain_mat: StandardMaterial3D
	var was_stained := false   # the debug JSON: stains wear off within seconds

	## A look from the wire (ADR 0167): ids the item registry does not know are left out.
	func set_look(d: Dictionary, items: ItemRegistry, atlas: Material) -> void:
		colour = int(d.get("c", 0))
		model.set_variant(colour)
		var w := {}
		var dw: Variant = d.get("w", {})
		if dw is Dictionary:
			for slot in ["head", "body", "feet", "back"]:
				var id := str(dw.get(slot, ""))
				if id != "" and not items.get_def(id).is_empty():
					w[slot] = {"id": id}
		if w != worn:
			worn = w
			model.set_worn(w)
		var h := str(d.get("h", ""))
		if items.get_def(h).is_empty():
			h = ""
		if h != held:
			held = h
			_hold(h, items, atlas)
		var st := PlayerStains.new()
		var ds: Variant = d.get("s", {})
		if ds is Dictionary:
			for k in PlayerStains.KINDS:
				st.amount[k] = clampf(float(ds.get(k, 0.0)), 0.0, 1.0)
		var c := st.tint()
		if stain_mat == null:
			stain_mat = Player._make_stain_overlay(false)
		stain_mat.albedo_color = c
		was_stained = was_stained or c.a > 0.01
		stain_mat.emission_enabled = float(st.amount["burning"]) > 0.0
		stain_mat.emission = PlayerStains.COLORS["burning"] * 1.2
		for part in model.get_children():
			if part is GeometryInstance3D:
				part.material_overlay = stain_mat if c.a > 0.01 else null

	## The held item in the right hand, pointing forward like a tool held at the ready.
	func _hold(id: String, items: ItemRegistry, atlas: Material) -> void:
		if held_mesh == null:
			held_mesh = MeshInstance3D.new()
			model.arm_r.add_child(held_mesh)
		held_mesh.visible = id != ""
		if id == "":
			return
		var sprites := ItemSprites.get_default()
		var mat_id := ItemIcon.material_id_for(items, id)
		var b := Basis(Vector3.UP, PI / 2.0)   # the sprite in the arm's plane, its +x (the tool head) forward (-z)
		if sprites.has(id) and not sprites.is_cube(id):
			held_mesh.mesh = BlockMeshes.sprite(id)
			held_mesh.basis = b.scaled(Vector3.ONE * 0.8)
			held_mesh.position = Vector3(0, -1.3, 0) - held_mesh.basis * Viewmodel.SPRITE_GRIP
		else:
			held_mesh.mesh = BlockMeshes.sprite(id) if sprites.is_cube(id) else (BlockMeshes.cube(mat_id, atlas) if mat_id > 0 else BlockMeshes.card(id))
			held_mesh.basis = Basis.IDENTITY.scaled(Vector3.ONE * 0.3)
			held_mesh.position = Vector3(0, -1.45, -0.1)

	func set_pet(st: Array) -> void:
		if st.is_empty():
			if pet != null:
				pet.visible = false
			return
		if pet == null:
			pet = Node3D.new()
			pet.top_level = true
			pet_model = MobModel.create("wolf", false)
			pet.add_child(pet_model)
			pet_model.scale = Vector3.ONE * MobModel.UNIT
			pet_tag = Label3D.new()
			pet_tag.font_size = 48
			pet_tag.pixel_size = 0.03
			pet_tag.outline_size = 12
			pet_tag.billboard = BaseMaterial3D.BILLBOARD_ENABLED
			pet_tag.no_depth_test = true
			pet_tag.position = Vector3(0, Settings.m2v(1.3), 0)
			pet.add_child(pet_tag)
			add_child(pet)
			pet.global_position = st[0]
		pet_target = st[0]
		pet_yaw = float(st[1])
		pet_speed = float(st[2])
		pet_sit = bool(st[3])
		pet_tag.text = String(st[4])
		pet.visible = true

	func _ready() -> void:
		model = ApeModel.new()
		model.scale = Vector3.ONE * (Settings.m2v(Settings.player_height_m) / (57.0 / ApeModel.GRID))
		add_child(model)
		var tag := Label3D.new()   # the name over the head
		tag.text = pname
		tag.font_size = 64
		tag.pixel_size = 0.03
		tag.outline_size = 16
		tag.billboard = BaseMaterial3D.BILLBOARD_ENABLED
		tag.no_depth_test = true
		tag.position = Vector3(0, Settings.m2v(Settings.player_height_m + 0.35), 0)
		add_child(tag)

	func _process(delta: float) -> void:
		var k := 1.0 - exp(-EASE * delta)
		position = position.lerp(target, k)
		rotation.y = lerp_angle(rotation.y, yaw, k)
		model.animate(delta, speed, not flying, flying, pitch, 0.0)
		if held != "":
			model.arm_r.rotation.x += 0.6   # the holding arm forward
		if pet != null and pet.visible:
			pet.global_position = pet.global_position.lerp(pet_target, k)
			pet.rotation.y = lerp_angle(pet.rotation.y, pet_yaw, k)
			pet_model.rotation.x = move_toward(pet_model.rotation.x, Mob.SIT_PITCH if pet_sit else 0.0, 3.0 * delta)
			pet_model.animate(delta, 0.0 if pet_sit else pet_speed, 0.0, 0.0, 0.0, 0.0)


var _by_id := {}


func _ready() -> void:
	Net.peer_joined.connect(_add)
	Net.peer_left.connect(_remove)
	Net.peer_state.connect(_state)
	Net.peer_look.connect(_look)
	for id in Net.peers:
		_add(id, Net.peers[id])


func _add(id: int, pname: String) -> void:
	if _by_id.has(id):
		return
	var a := Avatar.new()
	a.pname = pname
	a.name = "Peer%d_%s" % [id, pname.validate_node_name()]
	a.visible = false   # until its first state arrives
	add_child(a)
	_by_id[id] = a


func _remove(id: int) -> void:
	if _by_id.has(id):
		_by_id[id].queue_free()
		_by_id.erase(id)


func _state(id: int, pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool, pet: Array = []) -> void:
	if not _by_id.has(id):
		_add(id, Net.peers.get(id, "?"))
	var a: Avatar = _by_id[id]
	if a.fresh or a.position.distance_to(pos) > Settings.m2v(8.0):   # first state or a teleport: no slide across the map
		a.position = pos
		a.rotation.y = yaw
		a.fresh = false
	a.target = pos
	a.yaw = yaw
	a.pitch = pitch
	a.speed = speed
	a.flying = flying
	a.set_pet(pet)
	a.visible = true


func _look(id: int, d: Dictionary) -> void:
	if not _by_id.has(id):
		_add(id, Net.peers.get(id, "?"))
	var main := get_parent()
	_by_id[id].set_look(d, main.items, main.player.viewmodel.atlas_material)


## What this player looks like to the others (ADR 0167), sent by Net when it changes: stains in tenths.
static func look_of(main: Node) -> Dictionary:
	var p: Node = main.player
	var w := {}
	if main.survival and main.survival.clothing:
		for slot in main.survival.clothing.worn:
			w[slot] = String(main.survival.clothing.worn[slot].get("id", ""))
	var s := {}
	for k in p.stains.amount:
		var a := snappedf(float(p.stains.amount[k]), 0.1)
		if a > 0.0:
			s[k] = a
	return {"h": String(p.inventory.held().get("id", "")) if p.inventory else "", "w": w, "s": s}


## Name -> [drawn position, yaw, ape colour] of every avatar that has shown up: the map's markers and /tp <name>.
func others() -> Dictionary:
	var out := {}
	for id in _by_id:
		var a: Avatar = _by_id[id]
		if a.visible:
			out[a.pname] = [a.position, a.rotation.y, a.colour]
	return out


## For the debug JSON: what each avatar holds, wears and is stained with.
func looks() -> Dictionary:
	var out := {}
	for id in _by_id:
		var a: Avatar = _by_id[id]
		out[a.pname] = {"held": a.held, "worn": a.worn.keys(), "stained": a.was_stained, "colour": a.colour}
	return out


## For the debug JSON: where each avatar is drawn.
func positions() -> Dictionary:
	var out := {}
	for id in _by_id:
		var p: Vector3 = _by_id[id].position
		out[str(id)] = [snappedf(p.x, 0.1), snappedf(p.y, 0.1), snappedf(p.z, 0.1)]
	return out
