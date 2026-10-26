class_name MetalAgeTest
extends CraftTest

## Debug (metal_age_test=1, ADR 0070): the bronze and iron ages through the player's own click code,
## on CraftTest's cleared strip of grass and with its helpers. Bronze: a molten copper crucible goes
## on a firepit, tin nuggets are laid on with held right clicks, the fire is lit and the molten bronze
## is cast in an ingot mold. Iron: a bloomery takes nuggets and charcoal and leaves a hot bloom; a
## forge heats it again; a stone anvil refuses it, a bronze anvil takes it and a bronze hammer beats
## it into an ingot. Waiting is fast-forwarded by ticking the block entities. Screenshots go to the
## run's out dir (craft_shots=0 skips them); one JSON line "debug metal {...}" says what happened.


var aim_log := []   # blows whose ray met another column than the one aimed at


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(0.6).timeout
	main.player.autopilot_action = "look"
	var B := Settings.BLOCK
	base = BlockEntities.anchor_for(Vector3i(main.player.position.floor()))
	g0 = floori(float(main.world.surface_height(base.x, base.z)) / B) * B - 1
	var lo := Vector3i(base.x - 8 * B, g0 - 2 * B, base.z - 8 * B)
	var hi := Vector3i(base.x + 8 * B, g0 + 1, base.z + B)
	_fill(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT)
	_fill(Vector3i(lo.x, g0, lo.z), Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.GRASS)
	_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 7 * B, hi.z), GameMaterials.AIR)
	await tree.create_timer(1.5).timeout
	var inv: PlayerInventory = main.player_inv
	var at := func(dx: int, dz: int) -> Vector3i: return Vector3i(base.x + dx * B, g0 + 1, base.z + dz * B)
	var mid := Vector3(0.5, 0.2, 0.5)

	# ---- bronze: tin into molten copper on a firepit, cast in an ingot mold ----
	var f_at: Vector3i = at.call(-3, -3)
	inv.pickup("firepit", 1)
	main._hold("firepit")
	_click_top(f_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var fp = main.blocks.at(f_at)
	r["firepit_placed"] = fp is FirepitEntity
	inv.pickup("oak_log", 2 * main.items.units("oak_log"))
	main._hold("oak_log")
	await _hold_on(f_at, mid, 2)
	inv.pickup("crucible_copper", 1)
	main._hold("crucible_copper")
	await _hold_on(f_at, mid)
	r["copper_on_fire"] = fp is FirepitEntity and fp.inv.get_slot(FirepitEntity.VESSEL).get("id", "") == "crucible_copper"
	inv.pickup("tin_nugget", 2)
	main._hold("tin_nugget")
	await _hold_on(f_at, mid, 2)
	r["tin_on"] = int(fp.inv.get_slot(FirepitEntity.INPUT).get("count", 0)) if fp is FirepitEntity else -1
	inv.pickup("firestarter", 1)
	main._hold("firestarter")
	_use_on(f_at, mid)
	r["firepit_lit"] = fp is FirepitEntity and fp.is_lit()
	await _fast_forward(305)
	await _shot(tree, "bronze_melt", Vector3(f_at) + mid * B, 1.1)
	_empty_hand()
	await _hold_on(f_at, mid)
	await _pick_up_near(tree, f_at)
	r["crucible_bronze"] = inv.inv.count_of("crucible_bronze")
	if r["crucible_bronze"] == 0:
		r["shots"] = shots
		print("debug metal " + JSON.stringify(r))   # no bronze: stop before _hold hands one out
		return
	var m_at: Vector3i = at.call(-1, -3)
	inv.pickup("ingot_mold", 1)
	main._hold("ingot_mold")
	_click_top(m_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	main._hold("crucible_bronze")
	_use_on(m_at, Vector3(0.5, 0.05, 0.5))
	r["crucible_back"] = inv.inv.count_of("crucible") == 1
	await _fast_forward(int(Settings.cast_cool_s * 20.0) + 4)
	_empty_hand()
	_use_on(m_at, Vector3(0.5, 0.05, 0.5))
	await _pick_up_near(tree, m_at)
	r["bronze_ingot"] = inv.inv.count_of("bronze_ingot")

	# ---- iron: the bloomery ----
	var b_at: Vector3i = at.call(1, -3)
	inv.pickup("bloomery", 1)
	main._hold("bloomery")
	_click_top(b_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var bl = main.blocks.at(b_at)
	r["bloomery_placed"] = bl is BloomeryEntity
	r["bloomery_solid"] = main.get_world_voxel(b_at) == GameMaterials.WORK_BLOCK
	inv.pickup("iron_nugget", 8)
	main._hold("iron_nugget")
	await _hold_on(b_at, Vector3(0.5, 0.5, 0.98), 8)
	r["nuggets_in"] = int(bl.inv.get_slot(FirepitEntity.INPUT).get("count", 0)) if bl is BloomeryEntity else -1
	inv.pickup("charcoal", 2)
	main._hold("charcoal")
	await _hold_on(b_at, Vector3(0.5, 0.5, 0.98), 2)
	r["charcoal_in"] = int(bl.inv.get_slot(FirepitEntity.FUEL).get("count", 0)) if bl is BloomeryEntity else -1
	main._hold("firestarter")
	_use_on(b_at, Vector3(0.5, 0.5, 0.98))
	r["bloomery_lit"] = bl is BloomeryEntity and bl.is_lit()
	await _fast_forward(1200)
	await _shot(tree, "bloomery_lit", Vector3(b_at) + Vector3(0.5, 0.4, 0.5) * B, 1.4)
	await _fast_forward(1205)
	r["bloom_made"] = bl is BloomeryEntity and bl.inv.get_slot(FirepitEntity.OUTPUT).get("id", "") == "iron_bloom"
	_empty_hand()
	await _hold_on(b_at, Vector3(0.5, 0.5, 0.98))
	await _pick_up_near(tree, b_at)
	r["iron_bloom"] = inv.inv.count_of("iron_bloom")
	if r["iron_bloom"] == 0:
		r["shots"] = shots
		print("debug metal " + JSON.stringify(r))
		return

	# ---- the forge heats the bloom again ----
	var g_at: Vector3i = at.call(3, -3)
	inv.pickup("forge", 1)
	main._hold("forge")
	_click_top(g_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var fg = main.blocks.at(g_at)
	r["forge_placed"] = fg is ForgeEntity
	inv.pickup("charcoal", 2)
	main._hold("charcoal")
	await _hold_on(g_at, mid, 2)
	main._hold("iron_bloom")
	await _hold_on(g_at, mid)
	r["bloom_in_forge"] = fg is ForgeEntity and fg.inv.get_slot(FirepitEntity.INPUT).get("id", "") == "iron_bloom"
	main._hold("firestarter")
	_use_on(g_at, mid)
	await _fast_forward(int(Settings.metal_heat_s * 20.0) + 4)
	await _shot(tree, "forge", Vector3(g_at) + mid * B, 1.1)
	_empty_hand()
	await _hold_on(g_at, mid)
	await _pick_up_near(tree, g_at)
	var slot := _slot_of(inv, "iron_bloom")
	r["bloom_heat"] = snappedf(Inventory.heat_of(inv.inv.get_slot(slot)), 0.01) if slot >= 0 else -1.0

	# ---- a stone anvil refuses iron; the bronze anvil takes it and the hammer makes an ingot ----
	var s_at: Vector3i = at.call(-3, -6)
	inv.pickup("stone_anvil", 1)
	main._hold("stone_anvil")
	_click_top(s_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var stone = main.blocks.at(s_at)
	if slot >= 0 and stone is AnvilEntity:
		inv.select(slot)
		_use_on(s_at, Vector3(0.5, 0.6, 0.5))
		r["stone_anvil_refused"] = not stone.has_piece() and inv.inv.count_of("iron_bloom") == 1
	var n_at: Vector3i = at.call(0, -6)
	inv.pickup("bronze_anvil", 1)
	main._hold("bronze_anvil")
	_click_top(n_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var anvil = main.blocks.at(n_at)
	r["bronze_anvil_placed"] = anvil is AnvilEntity and anvil.kind == "bronze_anvil"
	slot = _slot_of(inv, "iron_bloom")
	if anvil is AnvilEntity and slot >= 0:
		inv.select(slot)
		_use_on(n_at, Vector3(0.5, 0.6, 0.5))
		r["bloom_on_anvil"] = anvil.has_piece()
		main.work.choose(n_at, "smith_iron_ingot")
		inv.pickup("bronze_hammer", 1)
		main._hold("bronze_hammer")
		var blows := []
		for x in range(2, 6):
			blows += [[x, 2, Vector2i(0, 1)], [x, 5, Vector2i(0, -1)], [x, 2, Vector2i(0, 1)], [x, 5, Vector2i(0, -1)]]
		for z in [3, 4]:
			for x in [2, 2, 2, 1, 1, 3, 3, 2]:
				blows.append([x, z, Vector2i(-1, 0)])
			for x in [5, 5, 5, 6, 6, 4, 4, 5]:
				blows.append([x, z, Vector2i(1, 0)])
		var landed := 0
		var missed := []
		for i in blows.size():
			if i == 16:
				await _shot(tree, "bronze_anvil_half", Vector3(n_at) + Vector3(0.5, 0.6, 0.5) * B, 1.1)
			var b: Array = blows[i]
			var before: int = anvil.hits
			await _blow(tree, anvil, b[0], b[1], b[2])
			landed += anvil.hits - before
			if anvil.hits == before:
				missed.append(b)
		r["blows_landed"] = landed
		r["blows_missed"] = missed
		r["blows_off_column"] = aim_log
		r["blows"] = blows.size()
		r["anvil_left"] = anvil.summary()
		var rows := []
		for z in AnvilEntity.N:
			var row := ""
			for x in AnvilEntity.N:
				row += str(anvil.height(x, z))
			rows.append(row)
		r["anvil_heights"] = rows
		await _pick_up_near(tree, n_at)
		r["iron_ingot"] = inv.inv.count_of("iron_ingot")

	main.atmosphere.set_time_of_day(0.45)
	await _shot(tree, "metal_all", Vector3(at.call(0, -4)) + Vector3(0.5, 0.0, 0.5) * B, 3.0)
	r["work"] = main.work.summary()
	r["work"].erase("pieces")
	r["shots"] = shots
	print("debug metal " + JSON.stringify(r))


## CraftTest's blow, from higher up, aimed at points across the column's top until the work system's
## own ray meets this column (as a player shifts their aim), so a column nearer the player as tall as
## this one does not take the blow (the bar's ends are low next to a tall middle).
func _blow(tree: SceneTree, anvil: AnvilEntity, x: int, z: int, push: Vector2i) -> void:
	var k := float(Settings.BLOCK) / AnvilEntity.N
	var cam: Camera3D = main.player.camera
	for f in [0.3, 0.45, 0.1, -0.2, -0.4]:
		var top := Vector3(anvil.pos) + Vector3((x + 0.5 + f * push.x) * k, (AnvilEntity.BASE_H + anvil.height(x, z)) * k - 0.02, (z + 0.5 + f * push.y) * k)
		var back := Vector3(-push.x, 0, -push.y) * Settings.m2v(0.3)
		main.player.position = top + back + Vector3(0, Settings.m2v(0.35), 0)   # feet just over the anvil: looking almost straight down
		await tree.process_frame
		var dir := (top - cam.global_position).normalized()
		main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
		if main.work._anvil_column(anvil, cam.global_position, -cam.global_transform.basis.z) == Vector2i(x, z):
			break
	var col: Vector2i = main.work._anvil_column(anvil, cam.global_position, -cam.global_transform.basis.z)
	if col != Vector2i(x, z):
		aim_log.append([x, z, col.x, col.y])
	main.work.hit(main.player._aim(), cam.global_position, -cam.global_transform.basis.z, false)
