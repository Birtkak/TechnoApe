class_name Viewmodel
extends Node3D

## First-person arm and held item, Minecraft style: with an empty hand the
## ape's right arm (ApeModel.build_arm_mesh, hand at this node's origin) pokes
## into the view from the lower right; a held block (a small cube) or item
## (its extruded sprite) replaces the arm. The hand swings on use. The
## offhand stack (ADR 0055) sits still in the lower left. Hidden in third person.

var items: ItemRegistry
var atlas_material: Material
var _hand: Node3D           # the right hand: arm or held item; it swings
var _mesh: MeshInstance3D
var _arm: MeshInstance3D
var _off: MeshInstance3D    # the offhand stack, lower left
var _off_id := ""
var _swing := 0.0        # 0 = rest, 1 = fully swung
var _swinging := false
var _rest_pos := Vector3(0.5, -0.54, -0.82)
const OFF_POS := Vector3(-0.5, -0.56, -0.82)
var _current_id := ""

# held like a Minecraft tool: handle in the hand, head up and away from the view
const HELD_SPRITE_SCALE := 0.6    # bigger in the hand (developer, 2026-10-27)
const HELD_SPRITE_ROTATION := Vector3(0, 155, -10)   # back face toward the view: head up and to the left
const SPRITE_GRIP := Vector3(-0.3, -0.3, 0.0)         # handle end of a tool sprite (pixel 3, 13), in mesh units
const HAND := Vector3(0.0, 0.02, -0.16)
const SPRITE_HAND := Vector3(0.24, -0.1, -0.12)        # where the handle end sits: lower right, like Minecraft
# a handled tool (pickaxe, axe, knife...) is held bigger and more upright, Minecraft's first-person tool pose
const TOOL_SCALE := 1.05
const TOOL_ROTATION := Vector3(0, 125, -5)
const TOOL_HAND := Vector3(0.42, -0.14, -0.05)
const HANDLE_TURN := Basis(Vector3(0.7071068, 0.7071068, 0), PI)   # half a turn about a tool sprite's handle


func _ready() -> void:
	_hand = Node3D.new()
	add_child(_hand)
	_arm = ApeModel.build_arm_mesh()
	_arm.rotation_degrees = Vector3(62, 0, -28)   # hand at the origin, arm back towards the camera's lower right
	_arm.rotate_object_local(Vector3.UP, PI)      # back of the hand and the outer sleeve towards the camera
	_arm.scale = Vector3.ONE * 0.8
	_hand.add_child(_arm)
	_mesh = MeshInstance3D.new()
	_mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_mesh.position = HAND
	_hand.add_child(_mesh)
	_off = MeshInstance3D.new()
	_off.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_off.visible = false
	add_child(_off)
	# laid out in blocks (0.5 m) in front of the camera; the world unit is a 12.5 cm voxel (ADR 0033)
	scale = Vector3.ONE * Settings.BLOCK
	_hand.position = _rest_pos
	_hand.rotation_degrees = Vector3(0, 20, 0)
	show_item({})


func show_item(stack: Dictionary) -> void:
	var id: String = stack.get("id", "")
	if id == _current_id:
		return
	_current_id = id
	# Minecraft: the bare arm only shows with an empty hand; a held item replaces it
	_arm.visible = id == ""
	if id == "":
		_mesh.visible = false
		return
	_mesh.visible = true
	_dress(_mesh, id, HAND, 1.0)


## The offhand stack, mirrored into the lower left; it does not swing.
func show_offhand(stack: Dictionary) -> void:
	var id: String = stack.get("id", "")
	if id == _off_id:
		return
	_off_id = id
	_off.visible = id != ""
	if id != "":
		_dress(_off, id, Vector3.ZERO, -1.0)
		_off.position += OFF_POS
		_off.rotation_degrees.y -= 40.0


## Gives `mi` the look of item `id` held at `at` (a sprite has its own grip point); `side` -1
## mirrors a sprite's pose for the left hand.
func _dress(mi: MeshInstance3D, id: String, at: Vector3, side: float) -> void:
	mi.position = at
	var mat_id := ItemIcon.material_id_for(items, id)
	var sprites := ItemSprites.get_default()
	if sprites.is_cube(id):
		mi.mesh = BlockMeshes.sprite(id)
		mi.scale = Vector3.ONE * 0.32
		mi.rotation_degrees = Vector3.ZERO
	elif sprites.has(id):
		var tool := _is_tool(sprites, id)
		var sc := TOOL_SCALE if tool else HELD_SPRITE_SCALE
		var rot := (TOOL_ROTATION if tool else HELD_SPRITE_ROTATION) * Vector3(1, side, side)
		var b := Basis.from_euler(rot * PI / 180.0)
		if _edge_out(sprites, id):
			b = b * HANDLE_TURN
		mi.mesh = BlockMeshes.sprite(id)
		mi.basis = b.scaled(Vector3.ONE * sc)
		var hand := (TOOL_HAND if tool else SPRITE_HAND) * Vector3(side, 1, 1)
		mi.position = hand - b * (SPRITE_GRIP * sc)
	elif mat_id > 0:
		mi.mesh = BlockMeshes.cube(mat_id, atlas_material)
		mi.scale = Vector3.ONE * 0.32
		mi.rotation_degrees = Vector3.ZERO
	else:
		mi.mesh = BlockMeshes.card(id)
		mi.scale = Vector3.ONE * 0.4
		mi.rotation_degrees = Vector3(0, 0, -30 * side)


## An axe's honed edge is on its sprite's left, which the tool pose turns back toward the view (bug
## report 2026-10-27 15-23-17, "the axe is the wrong way in hand"): the head is turned over about the
## handle (the sprite's diagonal through its centre, so the grip stays put) to cut forward instead.
static func _edge_out(sprites: ItemSprites, id: String) -> bool:
	for l in sprites.sprites[id].get("layers", []):
		if l.get("shape", "") == "axe_head":
			return true
	return false


## A tool sprite starts with a handle layer (make_item_sprites.py: H / SH).
static func _is_tool(sprites: ItemSprites, id: String) -> bool:
	var layers: Array = sprites.sprites[id].get("layers", [])
	return not layers.is_empty() and layers[0].get("shape", "") in ["handle", "short_handle", "saw_handle"]


## Stain colour on the bare arm (Player._apply_stain_tint); null clears it.
func set_overlay(m: Material) -> void:
	_arm.material_overlay = m


func swing() -> void:
	_swinging = true


func swing_amount() -> float:
	return _swing


func _process(delta: float) -> void:
	if _swinging:
		_swing = minf(1.0, _swing + delta * 7.0)
		if _swing >= 1.0:
			_swinging = false
	else:
		_swing = maxf(0.0, _swing - delta * 5.0)
	var s := sin(_swing * PI)
	_hand.position = _rest_pos + Vector3(-0.18 * s, -0.08 * s, -0.28 * s)
	_hand.rotation_degrees = Vector3(-40 * s, 20 - 25 * s, 0)
