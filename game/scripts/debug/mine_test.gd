class_name MineTest
extends RefCounted

## Debug (mine_test=collapse|propped|pick|laser, ADR 0142): mines need shoring, filmed and timed. 14 m under the
## player a 14 x 6 x 14 m block of stone with a basalt roof is set (the same rock on any seed), then:
##   collapse: a 7 x 7 x 3 m room dug through the dig path (Main._on_dug, 0.5 m blocks), filmed from a corner: a
##             filmstrip through the creaking (support_strip.png) and a burst of the fall (support_burst.png);
##   propped:  the same room, a stone pillar set in its middle while it creaks: it must stop and hold;
##   pick:     a 3 x 3 m tunnel dug a block at a time at pick_rate blocks/s for the run (no warning expected);
##   laser:    the same tunnel cut by /laser pulses, a metre forward each.
## One JSON line "debug mine {...}": the support stats (scan ms per dig on the driver thread, the main thread's
## part), warnings, collapses; the frame budget is DebugCapture's own JSON.

const DEPTH_M := 14.0
const ROOM_M := 7.0
const HEIGHT_M := 3.0
const TUNNEL_M := 3.0

var main: Node
var r := {}
var mode := ""
var pick_rate := 4.0     # 0.5 m blocks a second (a good pick in stone is ~1)
var _c := Vector3i.ZERO  # the room's floor centre


func run(p_main: Node, p_mode: String) -> void:
	main = p_main
	mode = p_mode
	var tree: SceneTree = main.get_tree()
	r["mode"] = mode
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/time set noon")
	main.chat.send("/gamemode creative")
	main.player.flying = true
	Settings.cave_ins_all_modes = true   # the run digs in creative (flying, no damage); the rule is the survival one
	var me := Vector3i(main.player.position.floor())
	var g: int = main.world.surface_height(me.x, me.z)
	var b := Settings.BLOCK
	_c = Vector3i(me.x / b * b, (g - int(Settings.m2v(DEPTH_M))) / b * b, me.z / b * b)
	main.chat.send("/tp %d %d %d" % [_c.x, _c.y + int(Settings.m2v(1.0)), _c.z])
	await tree.create_timer(6.0).timeout
	var half := int(Settings.m2v(ROOM_M + 7.0) / 2.0)
	main._debug_fill(_c - Vector3i(half, int(Settings.m2v(1.0)), half), _c + Vector3i(half, int(Settings.m2v(HEIGHT_M + 2.0)), half), GameMaterials.STONE)
	# the roof rock in dark basalt (span 2.25 m), so a falling slab reads against the stone walls
	main._debug_fill(_c + Vector3i(-half, int(Settings.m2v(HEIGHT_M)), -half), _c + Vector3i(half, int(Settings.m2v(HEIGHT_M + 2.0)), half), GameMaterials.id_of("basalt"))
	await tree.create_timer(1.0).timeout
	r["at"] = _c
	match mode:
		"collapse", "propped":
			await _room(tree)
		"pick":
			await _tunnel_pick(tree)
		"laser":
			await _tunnel_laser(tree)
	r["support"] = main.support.stats() if main.support else {}
	print("debug mine ", JSON.stringify(r))
	DebugCapture._finish()


## The blocks of a box (world voxels, min corner inclusive, size in blocks) dug the way a pick digs them.
func _dig_blocks(lo: Vector3i, size: Vector3i, per_frame: int, tree: SceneTree) -> void:
	var b := Settings.BLOCK
	var n := 0
	for y in range(size.y - 1, -1, -1):   # from the top down, as a miner works a face
		for z in size.z:
			for x in size.x:
				_dig_one(lo + Vector3i(x, y, z) * b)
				n += 1
				if n % per_frame == 0:
					await tree.process_frame


func _dig_one(a: Vector3i) -> void:
	var cells := []
	for c in BlockEntities.cube(a):
		if main.get_world_voxel(c) > 0:
			cells.append(c)
	if not cells.is_empty():
		main._on_dug(cells, [], Vector3(a) + Vector3.ONE * Settings.BLOCK * 0.5)


func _room(tree: SceneTree) -> void:
	var nb := int(Settings.m2v(ROOM_M)) / Settings.BLOCK
	var hb := int(Settings.m2v(HEIGHT_M)) / Settings.BLOCK
	var lo := _c - Vector3i(nb / 2, 0, nb / 2) * Settings.BLOCK
	var t0 := Time.get_ticks_msec()
	await _dig_blocks(lo, Vector3i(nb, hb, nb), 24, tree)
	r["dig_s"] = (Time.get_ticks_msec() - t0) / 1000.0
	# the camera in a corner, looking at the middle of the roof
	var span := int(Settings.m2v(ROOM_M * 0.42))
	var top := _c.y + int(Settings.m2v(HEIGHT_M))
	_cam(tree, Vector3(_c.x - span, _c.y + int(Settings.m2v(1.6)), _c.z - span), Vector3(_c.x, top - 4, _c.z))
	await tree.create_timer(2.5).timeout
	var strip := FrameSheet.new(320, 180, 6)
	var t_dug := Time.get_ticks_msec()
	var sup: MineSupport = main.support
	var first_warn := -1.0
	var fell := -1.0
	var propped := false
	var burst_done := false
	while Time.get_ticks_msec() - t_dug < 90000:
		await tree.create_timer(0.25).timeout
		var s := (Time.get_ticks_msec() - t_dug) / 1000.0
		if first_warn < 0.0 and not sup.warnings.is_empty():
			first_warn = s
			var w: Dictionary = sup.warnings.values()[0]
			r["warning"] = {"cells": w["count"], "ratio": snappedf(w["ratio"], 0.01), "falls_in_s": snappedf(w["t_fail"] - sup._now, 0.1)}
		if mode == "propped" and first_warn >= 0.0 and not propped and s > first_warn + 3.0:
			propped = main._place_furniture("stone_pillar", _c, 0)
			r["pillar_placed_at_s"] = snappedf(s, 0.1)
			r["pillar_placed"] = propped
		if strip.count() < 24 and int(s * 4) % 8 == 0:
			strip.add(main.get_viewport())   # every 2 s
		var falling: bool = fell < 0.0 and sup.stats_d["collapses"] > 0
		if falling:
			fell = s
			r["fell_at_s"] = snappedf(s, 0.1)
			await _burst(tree, DebugCapture.out_dir + "/support_burst.png")
			burst_done = true
			await tree.create_timer(3.0).timeout
			strip.add(main.get_viewport())
			break
		if mode == "propped" and propped and s > first_warn + 30.0 and sup.warnings.is_empty():
			break   # creaking stopped and stayed stopped for a while
	strip.add(main.get_viewport())
	strip.save(DebugCapture.out_dir + "/support_strip.png")
	if not burst_done and mode == "propped":
		await _burst(tree, DebugCapture.out_dir + "/support_burst.png")
	r["first_warn_s"] = snappedf(first_warn, 0.1)
	r["roof_mid_after"] = GameMaterials.name_of(main.get_world_voxel(Vector3i(_c.x, _c.y + int(Settings.m2v(HEIGHT_M)), _c.z)))
	r["ok"] = first_warn >= 0.0 and (fell > first_warn if mode == "collapse" else fell < 0.0 and propped)


## 48 frames, every second one: the fall of a roof takes under a second.
func _burst(tree: SceneTree, path: String) -> void:
	var sheet := FrameSheet.new(320, 180, 8)
	for i in 96:
		await tree.process_frame
		if i % 2 == 0:
			sheet.add(main.get_viewport())
	sheet.save(path)


func _tunnel_pick(tree: SceneTree) -> void:
	var w := int(Settings.m2v(TUNNEL_M)) / Settings.BLOCK
	var lo := _c - Vector3i(int(Settings.m2v(ROOM_M * 0.5)), 0, w / 2 * Settings.BLOCK)
	var b := Settings.BLOCK
	var t0 := Time.get_ticks_msec()
	var n := 0
	var period := 1.0 / pick_rate
	for x in int(Settings.m2v(ROOM_M + 4.0)) / b:
		main.player.position = Vector3(lo.x + x * b - b, _c.y + 4, _c.z)
		for y in range(w - 1, -1, -1):
			for z in w:
				_dig_one(lo + Vector3i(x, y, z) * b)
				n += 1
				await tree.create_timer(period).timeout
				if (Time.get_ticks_msec() - t0) / 1000.0 > DebugCapture.duration - 12.0:
					r["blocks"] = n
					return
	r["blocks"] = n


func _tunnel_laser(tree: SceneTree) -> void:
	main.player.position = Vector3(_c.x - int(Settings.m2v(ROOM_M * 0.5)), _c.y + int(Settings.m2v(1.5)), _c.z)
	main.player.set_look(-PI / 2.0, 0.0)   # facing +x
	var t0 := Time.get_ticks_msec()
	var n := 0
	while (Time.get_ticks_msec() - t0) / 1000.0 < DebugCapture.duration - 14.0:
		main.chat.send("/laser 1.5 40")
		await tree.create_timer(1.7).timeout
		main.player.position.x += Settings.m2v(0.75)
		n += 1
	r["laser_shots"] = n


func _cam(tree: SceneTree, at: Vector3, look: Vector3) -> void:
	var spot := "%d %d %d %d %d %d" % [at.x, at.y, at.z, look.x, look.y, look.z]
	main.chat.send("/cam off")
	main.chat.send("/cam " + spot)
