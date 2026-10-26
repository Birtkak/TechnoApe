class_name LiquidsTest
extends RefCounted

## Debug (liquids_test=1, bug reports 2026-10-25 "give all liquids the water look" and "make items burn
## in lava"): on a flattened patch beside the player, one pool per liquid (oil, water, blood, toxic
## sludge, lava, in that order so oil is far from the lava), each with a deep end and a shallow shelf
## over a sand bed, and a ledge the liquid spills over. Photographs each pool by day and the row at
## night, then drops sticks and an obsidian block into the lava and shoots the puff. Shots go to the
## run's out dir /shots; one JSON line "debug liquids {...}" says what burned and what survived.

const LIQUIDS := ["oil", "water", "blood", "toxic_sludge", "lava"]
const POOL := 16     # voxels across (2 m)
const GAP := 6       # stone between pools
const DEEP := 7      # voxels of liquid at the deep end

var main: Node
var r := {}
var _o: Vector3i   # the patch's corner: ground at y - 1


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/gamemode creative")
	main.chat.send("/time set noon")
	main.chat.send("/weather clear 600")
	main.player.flying = true
	var me := Vector3i(main.player.position.floor())
	_o = me + Vector3i(-48, 0, 10)
	_o.y = _ground(me) + 1
	var size := Vector3i(LIQUIDS.size() * (POOL + GAP) + GAP, 20, POOL + 2 * GAP)
	_flatten(size)
	main.player.position = Vector3(_o) + Vector3(-10, 14, -12)   # out of the shots
	await tree.create_timer(0.5).timeout
	for i in LIQUIDS.size():
		_pool(_pool_corner(i), GameMaterials.id_of(LIQUIDS[i]))
	await tree.create_timer(1.5).timeout   # the spills start pouring into the pools
	var shots: bool = DebugCapture.looks_shots
	if shots:
		for i in [2, 0, 1, 3, 4]:   # blood first: it dries up (its "lifetime")
			var c := Vector3(_pool_corner(i)) + Vector3(POOL * 0.5, 0, POOL * 0.5)
			await _shot(tree, "liquid_" + LIQUIDS[i], c + Vector3(-2, 10, -16), c + Vector3(0, -3, 0))
		var mid := Vector3(_o) + Vector3(size.x * 0.5, 0, GAP + POOL * 0.5)
		await _shot(tree, "liquids_row", mid + Vector3(0, 26, -34), mid + Vector3(0, -2, 0))
	# items into the lava: sticks burn, obsidian (fireproof) survives
	var lava_c := Vector3(_pool_corner(LIQUIDS.size() - 1)) + Vector3(POOL * 0.5, 0, POOL * 0.5)
	if shots:
		main.chat.send("/cam %.1f %.1f %.1f %.1f %.1f %.1f" % [lava_c.x - 3, lava_c.y + 8, lava_c.z - 12, lava_c.x, lava_c.y, lava_c.z])
		await tree.create_timer(0.8).timeout
	var burned0: int = main.item_stats["burned"]
	var sticks := []
	for k in 3:
		sticks.append(main.spawn_item({"id": "stick", "count": 4}, lava_c + Vector3(k * 3 - 3, 6, 0)))
	var obsidian: ItemEntity = main.spawn_item({"id": "obsidian", "count": 1}, lava_c + Vector3(0, 6, 4))
	for e in sticks + [obsidian]:
		e.throw()
	var shot_taken := false
	for _f in 240:
		await tree.process_frame
		if shots and not shot_taken and main.item_stats["burned"] > burned0:
			await tree.create_timer(0.12).timeout   # let the flames and smoke rise a little
			main.chat.send("/shot item_burn")
			shot_taken = true
		if main.item_stats["burned"] - burned0 >= 3 and _f > 120:
			break
	r["burned"] = main.item_stats["burned"] - burned0
	r["obsidian_alive"] = is_instance_valid(obsidian) and not obsidian.is_queued_for_deletion()
	if shots:
		main.chat.send("/time set midnight")
		await tree.create_timer(1.0).timeout
		var mid := Vector3(_o) + Vector3(size.x * 0.5, 0, GAP + POOL * 0.5)
		await _shot(tree, "liquids_row_night", mid + Vector3(0, 26, -34), mid + Vector3(0, -2, 0))
		var c := Vector3(_pool_corner(LIQUIDS.size() - 1)) + Vector3(POOL * 0.5, 0, POOL * 0.5)
		await _shot(tree, "liquid_lava_night", c + Vector3(-2, 10, -16), c + Vector3(0, -3, 0))
		main.chat.send("/time set noon")
	r["ok"] = r["burned"] >= 3 and r["obsidian_alive"]
	print("debug liquids ", JSON.stringify(r))


func _pool_corner(i: int) -> Vector3i:
	return _o + Vector3i(GAP + i * (POOL + GAP), 0, GAP)


## A pool dug into the patch at `c` (ground at c.y - 1): stone walls, a sand bed with a few stones,
## the back half a shelf 2 voxels under the brim, the front half DEEP, and a stone ledge at the back
## 4 voxels above the brim with a tub of liquid on it that pours down into the pool.
func _pool(c: Vector3i, m: int) -> void:
	var stone := GameMaterials.STONE
	var sand := GameMaterials.id_of("sand_static")
	for z in range(-1, POOL + 1):
		for x in range(-1, POOL + 1):
			var wall := x < 0 or z < 0 or x >= POOL or z >= POOL
			for y in range(-DEEP - 2, 0):
				var p := c + Vector3i(x, y, z)
				if wall:
					main.set_world_voxel(p, stone)
					continue
				var bed := -DEEP - 1 if z < POOL / 2 else -3
				if y <= bed:
					main.set_world_voxel(p, sand if (x * 7 + z * 3) % 11 != 0 or y < bed else stone)
				elif y < -1:
					main.set_world_voxel(p, m)
				else:
					main.set_world_voxel(p, GameMaterials.AIR)   # one voxel of bank above the surface
	# the ledge: a stone tub at the back, its front lip lower so the liquid spills toward the pool
	for z in range(POOL - 4, POOL + 1):
		for x in range(4, POOL - 4):
			for y in range(-1, 5):
				var p := c + Vector3i(x, y, z)
				var rim := x == 4 or x == POOL - 5 or z == POOL
				if y == -1 or (rim and y < 4):
					main.set_world_voxel(p, stone)
				elif y >= 0 and y < 3 and z > POOL - 4:
					main.set_world_voxel(p, m)


func _shot(tree: SceneTree, name: String, at: Vector3, look: Vector3) -> void:
	main.chat.send("/cam %.1f %.1f %.1f %.1f %.1f %.1f" % [at.x, at.y, at.z, look.x, look.y, look.z])
	await tree.create_timer(1.0).timeout
	main.chat.send("/shot " + name)
	await tree.create_timer(0.4).timeout


func _ground(p: Vector3i) -> int:
	for dy in range(16, -64, -1):
		if MobManager.is_solid(main.get_world_voxel(p + Vector3i(0, dy, 0))):
			return p.y + dy
	return p.y - 1


## Clears `size` above the patch's corner and lays grass under it, dirt below that.
func _flatten(size: Vector3i) -> void:
	for z in size.z:
		for x in size.x:
			for y in range(-12, size.y):
				var m := GameMaterials.AIR
				if y == -1:
					m = GameMaterials.GRASS
				elif y < -1:
					m = GameMaterials.DIRT
				main.set_world_voxel(_o + Vector3i(x, y, z), m)
