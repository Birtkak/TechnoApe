class_name RemotePlayers
extends Node3D

## The other players' avatars (ADR 0145): an ApeModel per peer that eases toward the last state the
## server relayed (20 Hz) and animates by the sent speed.

const EASE := 12.0   # per second: ~80 ms to close most of the gap, one state interval


class Avatar extends Node3D:
	var model: ApeModel
	var target := Vector3.ZERO
	var yaw := 0.0
	var pitch := 0.0
	var speed := 0.0
	var flying := false
	var fresh := true

	func _ready() -> void:
		model = ApeModel.new()
		model.scale = Vector3.ONE * (Settings.m2v(Settings.player_height_m) / (57.0 / ApeModel.GRID))
		add_child(model)

	func _process(delta: float) -> void:
		var k := 1.0 - exp(-EASE * delta)
		position = position.lerp(target, k)
		rotation.y = lerp_angle(rotation.y, yaw, k)
		model.animate(delta, speed, not flying, flying, pitch, 0.0)


var _by_id := {}


func _ready() -> void:
	Net.peer_joined.connect(_add)
	Net.peer_left.connect(_remove)
	Net.peer_state.connect(_state)
	for id in Net.peers:
		_add(id, Net.peers[id])


func _add(id: int, pname: String) -> void:
	if _by_id.has(id):
		return
	var a := Avatar.new()
	a.name = "Peer%d_%s" % [id, pname.validate_node_name()]
	a.visible = false   # until its first state arrives
	add_child(a)
	_by_id[id] = a


func _remove(id: int) -> void:
	if _by_id.has(id):
		_by_id[id].queue_free()
		_by_id.erase(id)


func _state(id: int, pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool) -> void:
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
	a.visible = true


## For the debug JSON: where each avatar is drawn.
func positions() -> Dictionary:
	var out := {}
	for id in _by_id:
		var p: Vector3 = _by_id[id].position
		out[str(id)] = [snappedf(p.x, 0.1), snappedf(p.y, 0.1), snappedf(p.z, 0.1)]
	return out
