class_name Projectiles
extends Node3D

## Thrown items in flight (ADR 0030): a flint spear leaves the hand at its
## item's throw.speed_m along the look and falls with gravity. Each physics
## frame its path is swept against animals (MobManager.pick) and the voxel
## grid (World.raycast); an animal takes throw.damage. Then (or on hitting
## the ground, or after MAX_S) it drops as an ordinary item to pick up.

const GRAVITY_M := 9.8
const MAX_S := 6.0

var main: Node            # Main: items, mobs, world, spawn_item
var _flying: Array = []   # {stack, pos, vel, age, damage, node}
var stats := {"thrown": 0, "hits": 0, "landed": 0}


func throw(stack: Dictionary, from: Vector3, dir: Vector3) -> void:
	if stack.is_empty():
		return
	var t: Dictionary = main.items.get_def(stack["id"]).get("throw", {})
	var node := MeshInstance3D.new()
	node.mesh = BlockMeshes.sprite(stack["id"])
	node.position = from
	add_child(node)
	var f := {"stack": stack, "pos": from, "vel": dir.normalized() * Settings.m2v(float(t.get("speed_m", 12.0))),
		"age": 0.0, "damage": int(t.get("damage", 2)), "node": node}
	_orient(f)
	_flying.append(f)
	stats["thrown"] += 1


func flying() -> int:
	return _flying.size()


func _physics_process(delta: float) -> void:
	if _flying.is_empty():
		return
	var alive: Array = []
	for f in _flying:
		f["vel"].y -= Settings.m2v(GRAVITY_M) * delta
		f["age"] += delta
		var a: Vector3 = f["pos"]
		var seg: Vector3 = f["vel"] * delta
		var dist := seg.length()
		var dir := seg / maxf(dist, 1e-6)
		var mob_hit: Dictionary = main.mobs.pick(a, dir, dist) if main.mobs != null else {}
		var block_hit: VoxelRaycastResult = main.world.raycast(a, dir, dist, VoxelBody.MASK_SOLID)   # through sticks and work spots
		if not mob_hit.is_empty() and (block_hit == null or float(mob_hit["dist"]) < float(block_hit.distance)):
			main.mobs.attack(mob_hit["mob"], f["damage"], a)
			stats["hits"] += 1
			_land(f, a + dir * maxf(0.0, float(mob_hit["dist"]) - 0.3), Vector3(0, 1.5, 0))
			continue
		if block_hit != null:
			stats["landed"] += 1
			_land(f, a + dir * maxf(0.0, float(block_hit.distance) - 0.3), Vector3.ZERO)
			continue
		if f["age"] > MAX_S:
			_land(f, a, f["vel"] * 0.1)
			continue
		f["pos"] = a + seg
		f["node"].position = f["pos"]
		_orient(f)
		alive.append(f)
	_flying = alive


## The item sprite's diagonal (handle bottom left, tip top right) along the flight.
func _orient(f: Dictionary) -> void:
	var fwd: Vector3 = f["vel"].normalized()
	var side := fwd.cross(Vector3.UP)
	if side.length() < 0.01:
		side = Vector3.RIGHT
	side = side.normalized()
	var up := side.cross(fwd)
	var x := (fwd - up) / sqrt(2.0)
	var y := (fwd + up) / sqrt(2.0)
	f["node"].basis = Basis(x, y, x.cross(y)).scaled(Vector3.ONE * 1.6)


func _land(f: Dictionary, at: Vector3, vel: Vector3) -> void:
	f["node"].queue_free()
	if f["stack"].get("broken", false):
		stats["broke"] = int(stats.get("broke", 0)) + 1   # worn out on this throw (ADR 0042)
		var fx: ParticleFx = main.get("fx")
		if fx:
			fx.burst("poof", at, {"material": 0, "normal": Vector3.UP})
		return
	main.spawn_item(f["stack"], at, vel)
