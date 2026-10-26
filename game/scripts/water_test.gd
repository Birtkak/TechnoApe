class_name WaterTest
extends RefCounted

## Debug (water_test=1, ADR 0068): finite still water in the running game. From the river the autopilot stands
## in (target=river), it finds the bank, digs a trench into it from 0.5 m under the surface up into the
## open air, and after a while measures how much of the trench (up to the river's level) took water, that
## nothing stands above that level, that the river beside it lost water (it is finite: no reservoir refills
## it), that nothing flowed past the trench and that the simulation went quiet again; then the same for a
## crater blasted where the river meets its bank. Screenshots of before and after go to the run's out dir
## (water_shots=0 skips them); one JSON line "debug water {...}" says what happened.

const DIRS := [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]

var main: Node
var r := {}


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(1.0).timeout
	main.player.autopilot_action = "look"   # stand still: the bubble follows the player
	main.player.autopilot_speed_m = 0.0
	main.chat.send("/time set noon")
	main.chat.send("/gamemode creative")   # hover over the river (under water the screen is tinted)
	main.player.flying = true
	# the nearest water column (rings around the player, every other voxel) and its surface
	var me := Vector3i(main.player.position.floor())
	var near: Vector3i = main.world.generator.gen.find_nearest("river", me.x, me.z, int(Settings.m2v(400.0)))
	if near.y > 0:
		me = near
		main.chat.send("/tp %d %d %d" % [near.x, near.y + 24, near.z])   # stand over it, so the bubble holds it
		await tree.create_timer(8.0).timeout
	var p := me
	var top := -1
	for ring in range(0, 160, 2):
		for i in maxi(ring * 4, 1):
			var a := TAU * i / maxi(ring * 4, 1)
			var c := me + Vector3i(roundi(cos(a) * ring), 0, roundi(sin(a) * ring))
			for dy in range(24, -48, -1):
				if _water(c + Vector3i(0, dy, 0)):
					top = c.y + dy
					p = Vector3i(c.x, top, c.z)
					break
			if top >= 0:
				break
		if top >= 0:
			break
	r["surface_y"] = top
	r["water_at"] = p
	if top < 0:
		_done("no water near the player")
		return
	# from there, go into the water a few cells so the bank search starts inside the river
	for d in DIRS:
		var inner: Vector3i = p + d * 3
		if _water(inner) and _water(inner - d * 2):
			p = inner
			break
	# the nearest bank: the first column along an axis whose cell at the surface level is solid ground
	var dir := Vector3i.ZERO
	var k_bank := 0
	for k in range(1, 200):
		for d in DIRS:
			var c: Vector3i = Vector3i(p.x, top, p.z) + d * k
			if dir == Vector3i.ZERO and not _water(c) and GameMaterials.blocks_bodies(main.world.get_voxel(c)):
				dir = d
				k_bank = k
		if dir != Vector3i.ZERO:
			break
	if dir == Vector3i.ZERO:
		_done("no bank found")
		return
	var side := Vector3i(dir.z, 0, dir.x)   # across the trench
	var start: Vector3i = Vector3i(p.x, top, p.z) + dir * k_bank
	var length := 12
	var cells: Array[Vector3i] = []
	for a in length:
		for b in range(-3, 4):
			for y in range(top - 3, top + 9):
				cells.append(start + dir * a + side * b + Vector3i(0, y - top, 0))
	var river_probe: Vector3i = Vector3i(p.x, top, p.z) + dir * maxi(k_bank / 2, 0)
	r["bank"] = start
	r["dir"] = dir
	r["river_before"] = _count_water_column(river_probe)
	var cam_at: Vector3 = Vector3(start - dir * 4 + side * 10 + Vector3i(0, 14, 0))
	var cam_look: Vector3 = Vector3(start + dir * 5)
	if DebugCapture.water_shots:
		await _shot(tree, "water_before", cam_at, cam_look)
	var lo: Vector3i = cells[0]
	var hi: Vector3i = cells[0]
	for c in cells:
		lo = lo.min(c)
		hi = hi.max(c)
	main._debug_fill(lo, hi + Vector3i.ONE, GameMaterials.AIR)
	var t0 := Time.get_ticks_msec()
	# the trench takes water from the river beside it (checked every second, 20 s at most); above the level, nothing
	var below := 0
	var below_water := 0
	var above_water := 0
	for sec in 20:
		await tree.create_timer(1.0).timeout
		below = 0
		below_water = 0
		above_water = 0
		for c in cells:
			if c.y <= top:
				below += 1
				below_water += 1 if _water(c) else 0
			elif _water(c):
				above_water += 1
		if float(below_water) / maxf(below, 1.0) >= 0.98 or (sec >= 8 and main.bubble.active_chunks == 0):
			break
	r["trench_cells_to_level"] = below
	r["trench_filled"] = below_water
	r["trench_full_share"] = snappedf(float(below_water) / maxf(below, 1.0), 0.01)
	r["water_above_level"] = above_water
	r["river_after"] = _count_water_column(river_probe)
	# the reach: nothing beyond the trench end plus the reach (it ends in the bank, so nothing at all)
	var beyond := 0
	for a in range(length + 1, length + 32):
		for b in range(-3, 4):
			if _water(start + dir * a + side * b):
				beyond += 1
	r["water_past_trench"] = beyond
	r["sim_active_chunks"] = main.bubble.active_chunks
	r["waited_ms"] = Time.get_ticks_msec() - t0
	if DebugCapture.water_shots:
		await _shot(tree, "water_after", cam_at, cam_look)
	# a 2.5 m crater where the river meets its bank (bug report 02-19-55) takes water from the river; finite water
	# (ADR 0068): the river beside it drops, nothing stands above its level
	var cc: Vector3 = Vector3(start - dir * 2 + side * 14 + Vector3i(0, -4, 0)) + Vector3(0.5, 0.5, 0.5)
	var cr := int(Settings.m2v(2.5))
	var probe2: Vector3i = Vector3i(cc.floor()) - dir * (int(Settings.m2v(2.5)) + 10)   # in the river, outside the crater
	probe2.y = top
	r["crater_river_before"] = _count_water_column(probe2)
	if DebugCapture.water_shots:   # watch it fill (the filmstrip, capture_every=...)
		var at: Vector3 = cc - Vector3(dir) * 28.0 + Vector3(side) * 18.0 + Vector3(0, 30, 0)
		var spot := "%d %d %d %d %d %d" % [at.x, at.y, at.z, cc.x, cc.y, cc.z]
		main.chat.send("/cam off")
		main.chat.send("/cam " + spot)
		await tree.create_timer(1.0).timeout
	main.bubble.blast(cc, float(cr))
	var t1 := Time.get_ticks_msec()
	var filled_at := -1
	var shares: Array = []
	var c_below := 0
	var c_water := 0
	var c_above := 0
	for s in 30:
		await tree.create_timer(1.0).timeout
		c_below = 0
		c_water = 0
		c_above = 0
		for z in range(-cr, cr + 1):
			for y in range(-cr, cr + 1):
				for x in range(-cr, cr + 1):
					var c := Vector3i((cc + Vector3(x, y, z)).floor())
					if Vector3(x, y, z).length() > cr - 1.0:
						continue
					if c.y <= top:
						c_below += 1
						c_water += 1 if _water(c) else 0
					elif _water(c):
						c_above += 1
		shares.append(snappedf(float(c_water) / maxf(c_below, 1.0), 0.01))
		if filled_at < 0 and float(c_water) / maxf(c_below, 1.0) >= 0.95:
			filled_at = s + 1
			break
		if s >= 8 and main.bubble.active_chunks == 0:
			break
	if DebugCapture.water_shots:
		main.chat.send("/shot water_crater")
		await tree.create_timer(0.5).timeout
		main.chat.send("/cam off")
	r["crater_cells_to_level"] = c_below
	r["crater_full_share"] = snappedf(float(c_water) / maxf(c_below, 1.0), 0.01)
	r["crater_filled_s"] = filled_at
	r["crater_share_by_s"] = shares
	r["crater_water_above_level"] = c_above
	r["crater_river_after"] = _count_water_column(probe2)
	r["crater_waited_ms"] = Time.get_ticks_msec() - t1
	r["bubble"] = {"still_woken": main.bubble.stats().get("still_woken", 0)}
	r["sim_active_chunks_after"] = main.bubble.active_chunks
	# finite: both holes took water, the river lost some (never gained), nothing above its level, nothing past the trench
	r["crater_water"] = c_water
	r["ok"] = r["trench_filled"] >= 30 and above_water == 0 and r["river_after"] <= r["river_before"] and beyond == 0 \
		and c_water >= 100 and c_above == 0 and r["crater_river_after"] <= r["crater_river_before"] \
		and int(r["bubble"]["still_woken"]) > 0
	_done("")


func _water(c: Vector3i) -> bool:
	var m: int = main.world.get_voxel(c)
	return m == GameMaterials.WATER or m == GameMaterials.WATER_STATIC


func _count_water_column(c: Vector3i) -> int:
	var n := 0
	for dy in range(-32, 8):
		n += 1 if _water(c + Vector3i(0, dy, 0)) else 0
	return n


## A frame from a free camera at `at` looking at `look` (the command only takes when it is off first).
func _shot(tree: SceneTree, name: String, at: Vector3, look: Vector3) -> void:
	var spot := "%d %d %d %d %d %d" % [at.x, at.y, at.z, look.x, look.y, look.z]
	main.chat.send("/cam off")
	main.chat.send("/cam " + spot)
	await tree.create_timer(0.5).timeout
	main.chat.send("/cam off")
	main.chat.send("/cam " + spot)
	await tree.create_timer(1.5).timeout
	main.chat.send("/shot " + name)
	await tree.create_timer(0.5).timeout
	main.chat.send("/cam off")


func _done(why: String) -> void:
	if why != "":
		r["error"] = why
		r["ok"] = false
	print("debug water ", JSON.stringify(r))
