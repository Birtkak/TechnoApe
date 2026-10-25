class_name LooksTest
extends RefCounted

## Debug (looks_test=1, ADR 0053): sets down everything part C of the 2026-10-25 playtest reworked
## on a flattened patch beside the player and photographs it: wheat and carrots at every age, the
## placed flowers, mushrooms and a seashell, a standing torch and one on a wall, ten more torches in a
## ring (the budget case), the crafting table, dropped ore, mushrooms and a torch, and the held torch
## lighting a night scene. Shots go to the run's out dir /shots (looks_shots=0 skips them); one JSON
## line "debug looks {...}" lists what was placed and the light stats.

var main: Node
var r := {}
var _o: Vector3i   # the patch's corner: ground at y - 1


## Knobs after "--": looks_torches=N (the budget ring, default 10), light_m / light_e / light_att
## (Settings.torch_light_range_m / _energy / _attenuation, for before-and-after shots).
var args := {}


func run(p_main: Node) -> void:
	main = p_main
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	if args.has("light_m"):
		Settings.torch_light_range_m = float(args["light_m"])
	if args.has("light_e"):
		Settings.torch_light_energy = float(args["light_e"])
	if args.has("light_att"):
		Settings.torch_light_attenuation = float(args["light_att"])
	for c in main.torch_lights.get_children():
		if c is OmniLight3D:
			c.omni_attenuation = Settings.torch_light_attenuation
	for c in main.player.get_children():
		if c is OmniLight3D:
			c.visible = false   # the inside=1 debug torch (40 m) would hide what the torches light
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/gamemode creative")
	main.chat.send("/time set noon")
	main.player.flying = true
	if DebugCapture.looks_budget:
		await _budget(tree)
		return
	var me := Vector3i(main.player.position.floor())
	_o = me + Vector3i(-24, 0, 6)
	_o.y = _ground(me) + 1
	_flatten(Vector3i(58, 24, 58))
	main.player.position = Vector3(_o) + Vector3(-8, 12, -10)   # out of the shots
	await tree.create_timer(0.5).timeout
	# farm: wheat ages 0..7 in one row of 0.5 m plots, carrots in the next
	var f: Farming = main.farming
	var planted := 0
	for row in 2:
		var kind := "wheat" if row == 0 else "carrots"
		for age in 8:
			var plot := _o + Vector3i(2 + age * 6, -1, 2 + row * 6)
			for z in Settings.BLOCK:
				for x in Settings.BLOCK:
					f.till(plot + Vector3i(x, 0, z))
			for z in Settings.BLOCK:
				for x in Settings.BLOCK:
					if f.plant(plot + Vector3i(x, 0, z), kind):
						planted += 1
						if age > 0:
							f.grow(PlantStamps.crop_anchor(plot + Vector3i(x, 0, z)), age)
	r["crops"] = planted
	# placed clutter: the flowers, two mushrooms of each, a seashell, in a row
	var row2 := _o + Vector3i(4, 0, 20)
	var k := 0
	for name in ["flower", "flower_red", "cornflower", "oxeye_daisy", "allium", "mushroom_brown", "mushroom_red", "mushroom_brown", "mushroom_red", "seashell"]:
		main._on_placed(row2 + Vector3i(k * 4, 0, 0), GameMaterials.id_of(name))
		k += 1
	# torches: one standing, one on a stone wall, a ring of ten
	var wall_x := _o.x + 50
	for y in 10:
		for z in range(20, 30):
			main.set_world_voxel(Vector3i(wall_x, _o.y + y, _o.z + z), GameMaterials.STONE)
	main._on_placed(Vector3i(wall_x - 1, _o.y + 4, _o.z + 24), GameMaterials.id_of("torch"))
	main._on_placed(Vector3i(wall_x - 6, _o.y, _o.z + 24), GameMaterials.id_of("torch"))
	var ring := _o + Vector3i(26, 0, 46)
	for i in 10:
		var a := TAU * i / 10.0
		main._on_placed(ring + Vector3i(roundi(cos(a) * 8), 0, roundi(sin(a) * 8)), GameMaterials.id_of("torch"))
	# the crafting table and dropped things before it
	var table := Vector3i(_o.x + 4, _o.y, _o.z + 34)
	main._on_placed(table, GameMaterials.CRAFTING_TABLE)
	var drop := Vector3(_o.x + 16, _o.y + 1, _o.z + 30)
	for id in ["brown_mushroom", "red_mushroom", "iron_ore", "copper_ore", "tin_ore", "kelp", "seashell", "torch"]:
		main.spawn_item({"id": id, "count": 1}, drop)
		drop += Vector3(3, 0, 0)
	await tree.create_timer(3.0).timeout
	r["torches"] = main.blocks.all_of("torch").size()
	if DebugCapture.looks_shots:
		var o := Vector3(_o)
		await _shot(tree, "farm", o + Vector3(24, 14, -14), o + Vector3(24, 0, 6))
		await _shot(tree, "farm_close", o + Vector3(10, 7, -6), o + Vector3(14, 1, 5))
		await _shot(tree, "farm_ripe", o + Vector3(42, 6, -5), o + Vector3(42, 1, 6))
		await _shot(tree, "flowers", o + Vector3(20, 7, 10), o + Vector3(20, 2, 20))
		await _shot(tree, "torches", Vector3(wall_x - 14, _o.y + 8, _o.z + 14), Vector3(wall_x - 2, _o.y + 3, _o.z + 24))
		await _shot(tree, "torch_close", Vector3(wall_x - 12, _o.y + 6, _o.z + 19), Vector3(wall_x - 3, _o.y + 3, _o.z + 24))
		await _shot(tree, "table", o + Vector3(14, 8, 24), o + Vector3(8, 2, 37))
		await _shot(tree, "dropped", o + Vector3(26, 6, 22), o + Vector3(26, 1, 30))
		main.chat.send("/time set midnight")
		await tree.create_timer(1.0).timeout
		await _shot(tree, "torches_night", Vector3(wall_x - 14, _o.y + 8, _o.z + 14), Vector3(wall_x - 2, _o.y + 3, _o.z + 24))
		await _shot(tree, "ring_night", Vector3(ring) + Vector3(0, 22, -26), Vector3(ring))
		await _shot(tree, "dropped_night", o + Vector3(26, 6, 22), o + Vector3(26, 1, 30))
		main.chat.send("/cam off")
		main._hold("torch")
		main.player.position = Vector3(_o) + Vector3(20, 0, 34)
		main.player.flying = false
		main.player._yaw = PI   # toward the ring of torches (+z)
		main.player._pitch = -0.35
		main.player._apply_look()
		await tree.create_timer(1.0).timeout
		main.chat.send("/shot held_torch_night")
		await tree.create_timer(0.5).timeout
		main.chat.send("/time set noon")
		await tree.create_timer(1.0).timeout
		main.chat.send("/shot held_torch")
		await tree.create_timer(0.5).timeout
		main._hold("brown_mushroom")
		await tree.create_timer(0.5).timeout
		main.chat.send("/shot held_mushroom")
		await tree.create_timer(0.5).timeout
		main._hold("iron_ore")
		await tree.create_timer(0.5).timeout
		main.chat.send("/shot held_ore hud")
		await tree.create_timer(0.5).timeout
		main._hold("torch")
		main.chat.send("/time set midnight")
	r["lights"] = main.torch_lights.stats.duplicate()
	r["ok"] = r["torches"] >= 12 and r["crops"] >= 60
	print("debug looks ", JSON.stringify(r))


## looks_budget=1: the budget case of ADR 0053 without the patch's big edits: ten torches in a ring
## on the ground ahead, at midnight, a torch in the hand (the view tilts down onto the ring).
func _budget(tree: SceneTree) -> void:
	main.player.flying = false
	for i in 40:   # land first: the autopilot starts in the air, and the ring belongs on the ground, not in a crown
		if main.player.is_on_floor():
			break
		await tree.create_timer(0.1).timeout
	await tree.create_timer(3.0).timeout   # the fall brought new ground in: let it generate before the torches go down
	var me := Vector3i(main.player.position.floor())
	var fwd := Vector3(sin(main.player._yaw), 0, cos(main.player._yaw)) * -1.0   # the player's look (yaw 0 = -z)
	var centre := me + Vector3i((fwd * Settings.m2v(3.0)).round())
	var n := int(args.get("looks_torches", "10"))
	for i in n:
		var a := TAU * i / mini(n, 10) + (0.3 if i >= 10 else 0.0)
		var rad := Settings.m2v(1.5) * (1 + i / 10)   # rings of ten, 1.5 m apart
		var c := centre + Vector3i(roundi(cos(a) * rad), 0, roundi(sin(a) * rad))
		c.y = _ground(c) + 1
		main._on_placed(c, GameMaterials.id_of("torch"))
		await tree.process_frame   # one a frame, as fast as clicks could ever place them
	main._hold("torch")
	main.chat.send("/time set midnight")
	main.player.set_look(main.player._yaw, -0.6)
	await tree.create_timer(1.0).timeout
	r["centre"] = centre
	r["player"] = main.player.position
	r["torches"] = main.blocks.all_of("torch").size()
	r["lights"] = main.torch_lights.stats.duplicate()
	r["ok"] = r["torches"] >= mini(n, 10) and r["lights"]["lit"] >= mini(n, 10) and r["lights"]["held"]
	print("debug looks ", JSON.stringify(r))
	if args.get("looks_shot", "") != "":
		await tree.create_timer(2.0).timeout
		main.chat.send("/shot " + String(args["looks_shot"]))


func _shot(tree: SceneTree, name: String, at: Vector3, look: Vector3) -> void:
	main.chat.send("/cam %.1f %.1f %.1f %.1f %.1f %.1f" % [at.x, at.y, at.z, look.x, look.y, look.z])
	await tree.create_timer(0.8).timeout
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
			for y in range(-3, size.y):
				var m := GameMaterials.AIR
				if y == -1:
					m = GameMaterials.GRASS
				elif y < -1:
					m = GameMaterials.DIRT
				main.set_world_voxel(_o + Vector3i(x, y, z), m)
