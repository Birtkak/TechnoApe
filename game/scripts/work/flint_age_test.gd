class_name FlintAgeTest
extends RefCounted

## Debug (flint_age_test=1, ADR 0030): the deeper Flint Age through the
## player's own click code on a cleared strip of grass. A hide is laid down,
## scraped and dried; a charcoal pit is dug, stacked with firewood, lit,
## sealed with dirt and burnt down (and a second, open one burns away); a
## pan washes gravel in a pool; a spear is thrown at a sheep; torches go on
## the floor (and not on a wall) and get lights; thatch, wattle and daub,
## a straw bed and a storage pot are placed and used; a bag goes on; a pot
## of stew cooks in a firepit. Waiting is fast-forwarded by ticking the
## block entities. Screenshots go to the run's out dir (flint_age_shots=0
## skips them); one JSON line "debug flint_age {...}" says what happened.
## Everything stands on the 0.5 m block grid (ADR 0034): the strip's surface is
## laid so pieces and brush cubes of BLOCK voxels line up with it, spots step a
## block, and aim points are fractions of a block from a block's anchor.

var main: Node
var r := {}
var shots := []
var base := Vector3i.ZERO
var g0 := 0


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(0.6).timeout
	main.player.autopilot_action = "look"
	var B := Settings.BLOCK
	base = BlockEntities.anchor_for(Vector3i(main.player.position.floor()))
	# the grass layer's top voxel just under a multiple of BLOCK, so the blocks above it are on the grid
	g0 = floori(float(main.world.surface_height(base.x, base.z)) / B) * B - 1
	# a flat 19 x 9 block grass strip ahead
	var lo := Vector3i(base.x - 9 * B, g0 - 2 * B, base.z - 8 * B)
	var hi := Vector3i(base.x + 10 * B, g0 + 1, base.z + B)
	_fill(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT)
	_fill(Vector3i(lo.x, g0, lo.z), Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.GRASS)
	_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 7 * B, hi.z), GameMaterials.AIR)
	await tree.create_timer(1.5).timeout   # the cleared strip reaches the terrain the aim ray reads
	var inv: PlayerInventory = main.player_inv
	var at := func(dx: int, dz: int) -> Vector3i: return Vector3i(base.x + dx * B, g0 + 1, base.z + dz * B)

	# ---- a hide: lay it down, scrape it, let it dry ----
	inv.pickup("raw_hide", 1)
	inv.pickup("flint_knife", 1)
	var h: Vector3i = at.call(-6, -3)
	main._hold("raw_hide")
	_click_top(h - Vector3i.UP)
	await tree.process_frame
	var hide = main.blocks.at(h)
	r["hide_placed"] = hide is GroundEntity and hide.item == "raw_hide"
	main._hold("flint_knife")
	for i in 8:
		main._aim_use(Vector3(h) + Vector3(0.5, 0.06, 0.5) * Settings.BLOCK)
		await tree.process_frame
	r["hide_scraped"] = hide.item if hide is GroundEntity else "gone"
	await _shot(tree, "hide", Vector3(h) + Vector3(0.5, 0.0, 0.5) * Settings.BLOCK)
	await _fast_forward(90 * 20 + 1)
	r["hide_dried"] = hide.item if hide is GroundEntity else "gone"

	# ---- the charcoal pit: a trench two cells wide and two deep (front to back),
	# 16 + 16 firewood in the back row, lit, then sealed: dirt on top and in the front row ----
	var pa: Vector3i = Vector3i(base.x - 2 * B, g0 + 1 - B, base.z - 4 * B)
	var pb: Vector3i = pa + Vector3i(B, 0, 0)
	var front := [pa + Vector3i(0, 0, B), pb + Vector3i(0, 0, B)]
	for p in [pa, pb] + front:
		_block(p, GameMaterials.AIR)
	await tree.create_timer(1.5).timeout   # the hole reaches the terrain the aim ray reads
	inv.pickup("firewood", 40)
	main._hold("firewood")
	for p in [pa, pb]:
		_click_top(p - Vector3i.UP)
		await tree.process_frame
		for i in Settings.firewood_per_pile + 4:   # a few extra: a full pile takes no more
			if main.blocks.at(p) is FirewoodPileEntity and main.blocks.at(p).is_full():
				break
			main._aim_use(Vector3(p) + Vector3(0.5, 0.999, 0.5) * Settings.BLOCK)
			await tree.process_frame
	await tree.create_timer(0.6).timeout   # the piles reach the terrain the aim ray reads
	var a = main.blocks.at(pa)
	var b = main.blocks.at(pb)
	r["piles"] = [a.count if a is FirewoodPileEntity else -1, b.count if b is FirewoodPileEntity else -1]
	await _shot(tree, "firewood_piles", Vector3(pa) + Vector3(1.0, 0.5, 0.5) * Settings.BLOCK)
	inv.pickup("firestarter", 1)
	main._hold("firestarter")
	r["light_aim"] = _aim_info(Vector3(pa) + Vector3(0.5, 0.999, 0.5) * Settings.BLOCK)
	main._aim_use(Vector3(pa) + Vector3(0.5, 0.999, 0.5) * Settings.BLOCK)
	r["pit_lit"] = a is FirewoodPileEntity and a.is_lit() and b.is_lit()
	r["pile_states"] = [a.state, b.state] if a is FirewoodPileEntity and b is FirewoodPileEntity else []
	main.work._check_pits()
	r["open_faces_lit"] = main.work.open_faces(main.work.pit_of(pa))
	inv.pickup("dirt", 4 * main.items.units("dirt"))   # four blocks of dirt (brush cubes)
	main._hold("dirt")
	r["dirt_aims"] = []
	for p in [pa, pb]:
		# dirt against the pile's front face (into the open trench row), then on its top
		for face in [Vector3(0.5, 0.5, 0.999), Vector3(0.5, 0.999, 0.5)]:
			r["dirt_aims"].append(_aim_info(Vector3(p) + face * Settings.BLOCK))
			main._aim_use(Vector3(p) + face * Settings.BLOCK)
			await tree.process_frame
	r["open_faces_sealed"] = main.work.open_faces(main.work.pit_of(pa))
	main.work._check_pits()
	r["pit_sealed"] = a is FirewoodPileEntity and a.sealed
	if not (a is FirewoodPileEntity and b is FirewoodPileEntity):
		r["shots"] = shots
		print("debug flint_age " + JSON.stringify(r))   # nothing to burn: stop here
		return
	await _shot(tree, "charcoal_pit_smoke", Vector3(pa) + Vector3(1.0, 1.5, 0.5) * Settings.BLOCK, 2.2)
	# a second pit left open next to it bursts into flames once its grace time is up
	var pc: Vector3i = at.call(4, -6)
	main._hold("firewood")
	r["open_pit_place_aim"] = _aim_info(Vector3(pc - Vector3i.UP) + Vector3(0.5, 0.999, 0.5))
	_click_top(pc - Vector3i.UP)
	await tree.create_timer(0.6).timeout
	r["open_pit_pile"] = main.blocks.at(pc) is FirewoodPileEntity
	main._hold("firestarter")
	r["open_pit_light_aim"] = _aim_info(Vector3(pc) + Vector3(0.5, 0.999, 0.5) * Settings.BLOCK)
	main._aim_use(Vector3(pc) + Vector3(0.5, 0.999, 0.5) * Settings.BLOCK)
	r["open_pit_lit"] = main.blocks.at(pc) is FirewoodPileEntity and main.blocks.at(pc).is_lit()
	await _fast_forward(int(Settings.charcoal_pit_grace_s * 20.0) + 2)
	main.work._check_pits()
	r["open_pit_burnt"] = main.blocks.at(pc) == null and main.work.stats["pits_burnt"] == 1
	r["open_pit_fire"] = main.get_world_voxel(pc) == GameMaterials.FIRE
	await _fast_forward(int(Settings.charcoal_pit_burn_s * 20.0))
	r["pit_charred"] = [a.state, b.state]
	r["charcoal"] = a.charcoal + b.charcoal
	# dig it out: the dirt cover, then the piles, through the mining path's drops
	for p in [pa, pb]:
		_block(p + Vector3i(0, B, 0), GameMaterials.AIR)
		_block(p + Vector3i(0, 0, B), GameMaterials.AIR)
	await tree.create_timer(0.3).timeout
	await _shot(tree, "charcoal", Vector3(pa) + Vector3(1.0, 0.5, 0.5) * Settings.BLOCK)
	var before: int = inv.inv.count_of("charcoal")
	for p in [pa, pb]:
		var pile = main.blocks.at(p)
		for s in pile.contents() if pile else []:
			inv.pickup(s["id"], s["count"])
		if pile:
			main.work.remove_piece(pile)
	r["charcoal_dug"] = inv.inv.count_of("charcoal") - before

	# ---- the pan: a pool of water over gravel ----
	var pool: Vector3i = Vector3i(base.x + 2 * B, g0 + 1 - B, base.z - 3 * B)
	for dx in 2:
		for dz in 2:
			_block(pool + Vector3i(dx * B, -B, dz * B), GameMaterials.GRAVEL_STATIC)
			_block(pool + Vector3i(dx * B, 0, dz * B), GameMaterials.WATER_STATIC)
	await tree.create_timer(1.5).timeout
	inv.pickup("pan", 1)
	main._hold("pan")
	var loot_ids := ["copper_nugget", "tin_nugget", "iron_nugget", "flint"]
	var loot_before := {}
	for id in loot_ids:
		loot_before[id] = inv.inv.count_of(id)
	r["pan_aims"] = []
	for n in 4:
		r["pan_aims"].append(_aim_info(Vector3(pool) + Vector3(0.5 + (n % 2), 0.95, 0.5 + (n / 2)) * Settings.BLOCK))
		for i in Settings.pan_shakes:
			main._aim_use(Vector3(pool) + Vector3(0.5 + (n % 2), 0.95, 0.5 + (n / 2)) * Settings.BLOCK)
			await tree.process_frame
	r["pans"] = main.work.stats["pans"]
	var washed := 0   # bed voxels (the gravel's top layer under the pool) the pan turned to water
	for x in 2 * B:
		for z in 2 * B:
			if main.get_world_voxel(pool + Vector3i(x, -1, z)) == GameMaterials.WATER_STATIC:
				washed += 1
	r["pool_bed_washed"] = washed > 0
	var loot_after := {}
	for id in loot_ids:
		loot_after[id] = inv.inv.count_of(id)
	r["pan_loot"] = _gained(loot_before, loot_after)

	# ---- the spear: throw it at a sheep ----
	var sheep: Mob = main.mobs.spawn("sheep", Vector3(base.x, g0 + 1.0, base.z - 6 * B) + Vector3(0.5, 0, 0.5) * B)
	await tree.create_timer(0.3).timeout
	var hp: int = sheep.health if sheep else -1
	inv.pickup("flint_spear", 1)
	main._hold("flint_spear")
	var dir: Vector3 = (sheep.aabb().get_center() - main.player.camera.global_position).normalized() if sheep else Vector3.FORWARD
	main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y) + 0.06)
	main.player._use(true)
	r["spear_left_hand"] = inv.inv.count_of("flint_spear") == 0
	for i in 60:
		await tree.physics_frame
		if main.projectiles.flying() == 0:
			break
	r["spear_hits"] = main.projectiles.stats["hits"]
	r["sheep_hurt"] = sheep != null and (not is_instance_valid(sheep) or sheep.health < hp)

	# ---- torches: on the floor, not on a wall ----
	inv.pickup("torch", 4)
	main._hold("torch")
	var t1: Vector3i = at.call(-7, -1)
	var t2: Vector3i = at.call(-5, -1)
	_click_top(t1 - Vector3i.UP)
	main.player._place_cooldown = 0.0
	r["torch2_aim"] = _aim_info(Vector3(t2 - Vector3i.UP) + Vector3(0.5, 0.999, 0.5))
	_click_top(t2 - Vector3i.UP)
	var wall: Vector3i = at.call(-3, -1)
	_block(wall, GameMaterials.STONE)
	await tree.create_timer(0.3).timeout
	main._aim_use(Vector3(wall) + Vector3(0.001, 0.5, 0.5) * Settings.BLOCK)   # the stone's west face
	await tree.create_timer(0.4).timeout
	r["torches_placed"] = [main.get_world_voxel(t1) == GameMaterials.TORCH, main.get_world_voxel(t2) == GameMaterials.TORCH]
	r["torch_on_wall"] = main.get_world_voxel(wall + Vector3i(-1, B / 2, B / 2)) == GameMaterials.TORCH
	r["torch_lights"] = main.torch_lights.stats.duplicate()

	# ---- shelter: thatch, wattle, wattle and daub ----
	var blocks := {"thatch": at.call(6, -3), "wattle": at.call(7, -3), "daub_wall": at.call(8, -3)}
	for id in blocks:
		inv.pickup(id, main.items.units(id))   # a block's worth of a voxel item
		main._hold(id)
		main.player._place_cooldown = 0.0
		_click_top(blocks[id] - Vector3i.UP)
	await tree.create_timer(0.3).timeout
	var placed := {}
	for id in blocks:
		placed[id] = GameMaterials.name_of(main.get_world_voxel(blocks[id]))
	r["shelter"] = placed

	# ---- straw bed and storage pot ----
	inv.pickup("straw_bed", 1)
	main._hold("straw_bed")
	main.player._place_cooldown = 0.0
	var bed_at: Vector3i = at.call(6, -1)   # looking along +x from the start: the bed runs into the strip
	_click_top(bed_at - Vector3i.UP)
	await tree.create_timer(0.3).timeout
	var bed: FurnitureEntity = main.blocks.furniture_at(bed_at)
	r["straw_bed"] = bed.furniture if bed else "not placed"
	main.atmosphere.set_time_of_day(0.0)
	await tree.create_timer(0.6).timeout
	await _shot(tree, "torches_night", Vector3(at.call(-5, -1)) + Vector3(0.5, 0.3, 0.5) * Settings.BLOCK, 2.4)
	if bed:
		main._use_furniture(bed_at)
	r["slept_to"] = snappedf(main.atmosphere.time_of_day, 0.01)
	var pot_at: Vector3i = at.call(-6, -6)
	inv.pickup("storage_pot", 1)
	main._hold("storage_pot")
	main.player._place_cooldown = 0.0
	_click_top(pot_at - Vector3i.UP)
	await tree.create_timer(0.3).timeout
	r["storage_pot_block"] = main.get_world_voxel(pot_at) == GameMaterials.STORAGE_POT
	main._on_block_used(pot_at, GameMaterials.STORAGE_POT)
	r["storage_pot_screen"] = main.inv_screen.is_open and inv.container != null and inv.container.inv.size() == 9
	main.inv_screen.close()
	main.player.set_ui_open(false)

	# ---- the bag ----
	inv.cursor = {"id": "leather_bag", "count": 1}
	inv.click_bag_item()
	r["bag_worn"] = inv.has_bag()

	# ---- stew in a firepit ----
	var f_at: Vector3i = at.call(-2, -1)
	inv.pickup("firepit", 1)
	main._hold("firepit")
	main.player._place_cooldown = 0.0
	_click_top(f_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var fp = main.blocks.at(f_at)
	if fp is FirepitEntity:
		fp.inv.set_slot(FirepitEntity.VESSEL, {"id": "cooking_pot", "count": 1})
		fp.inv.set_slot(FirepitEntity.INPUT, {"id": "raw_meat", "count": 2})
		fp.inv.set_slot(FirepitEntity.FUEL, {"id": "firewood", "count": 8})
		r["firepit_lit"] = fp.ignite()
		await _fast_forward(320)
		r["stew_pot"] = fp.inv.get_slot(FirepitEntity.VESSEL).get("id", "")
	main.atmosphere.set_time_of_day(0.45)
	await _shot(tree, "all", Vector3(at.call(0, -4)) + Vector3(0.5, 0.0, 0.5) * Settings.BLOCK, 5.0)
	r["work"] = main.work.summary()
	r["work"].erase("pieces")
	r["quests"] = main.quests.summary()
	r["shots"] = shots
	print("debug flint_age " + JSON.stringify(r))


## What the aim ray hits when the player looks at `at`: [hit voxel, its material, the voxel before it].
func _aim_info(at: Vector3) -> Array:
	var dir: Vector3 = (at - main.player.camera.global_position).normalized()
	main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	var hit: VoxelRaycastResult = main.player._aim()
	if hit == null:
		return ["none"]
	return [hit.position, GameMaterials.name_of(main.get_world_voxel(hit.position)), hit.previous_position]


func _gained(before: Dictionary, after: Dictionary) -> Dictionary:
	var out := {}
	for id in after:
		var d: int = int(after[id]) - int(before.get(id, 0))
		if d > 0:
			out[id] = d
	return out


## Fills the world box [lo, hi) through the bubble in one native call, voxel by voxel when it cannot.
func _fill(lo: Vector3i, hi: Vector3i, m: int) -> void:
	if main.bubble.fill_box(lo, hi, m) >= 0:
		return
	for z in range(lo.z, hi.z):
		for y in range(lo.y, hi.y):
			for x in range(lo.x, hi.x):
				main.set_world_voxel(Vector3i(x, y, z), m)


## One 0.5 m block of BLOCK^3 voxels from its anchor.
func _block(p: Vector3i, m: int) -> void:
	_fill(p, p + Vector3i.ONE * Settings.BLOCK, m)


func _click_top(t: Vector3i) -> void:
	main._aim_use(Vector3(t) + Vector3(0.5, 0.999, 0.5))


## Waiting, sped up: 100 block-entity ticks (5 s of game) per frame.
func _fast_forward(ticks: int) -> void:
	var tree := main.get_tree()
	while ticks > 0:
		for i in mini(ticks, 100):
			main.blocks.tick()
		ticks -= 100
		await tree.process_frame


## Looks at `at` from `dist_m` metres up and back, waits for the view to draw, saves a PNG.
func _shot(tree: SceneTree, name: String, at: Vector3, dist_m: float = 1.4) -> void:
	if "flint_age_shots=0" in OS.get_cmdline_user_args():
		return
	var keep: Vector3 = main.player.position
	var eye := at + Vector3(0, Settings.m2v(dist_m) * 0.8, Settings.m2v(dist_m))
	main.player.position = eye - Vector3(0, main.player.camera.position.y, 0)
	var cam: Camera3D = main.player.camera
	for i in 4:
		await tree.process_frame
		var dir := (at - cam.global_position).normalized()
		main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	await tree.create_timer(0.3).timeout
	var path: String = DebugCapture.out_dir + "/flint_age_%s.png" % name
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DebugCapture.out_dir))
	main.get_viewport().get_texture().get_image().save_png(path)
	shots.append(path)
	main.player.position = keep
	await tree.process_frame
