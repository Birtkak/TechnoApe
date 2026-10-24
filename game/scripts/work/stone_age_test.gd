class_name StoneAgeTest
extends RefCounted

## Debug (stone_age_test=1, ADR 0023): the road from flint to a copper
## pickaxe through the player's own click code: knap an axe head, form a
## crucible and a pickaxe mold, fire both in pit kilns, melt copper-bearing
## stones in a firepit, cast the head, then sift gravel. Waiting is fast-
## forwarded by ticking the block entities (the same tick Main runs at 20/s).
## Screenshots of each stage go to the run's out dir (stone_age_shots=0 skips
## them for a budget run); one JSON line
## "debug stone_age {...}" says what happened (every check should be true).
## Pieces are 0.5 m blocks of BLOCK^3 voxels (ADR 0034): spots step a block, and
## aim points inside a piece are fractions of a block from its anchor.

var main: Node
var r := {}
var shots := []


func run(p_main: Node) -> void:
	main = p_main
	var tree: SceneTree = main.get_tree()
	await tree.create_timer(0.6).timeout
	main.player.autopilot_action = "look"
	var B := Settings.BLOCK
	var base := BlockEntities.anchor_for(Vector3i(main.player.position.floor()))
	var gy := func(x: int, z: int) -> int: return main.world.surface_height(x, z)
	# a flat 15 x 7 block grass strip ahead at the player's ground height, so every aim ray hits the ground
	var g0: int = gy.call(base.x, base.z)
	var lo := Vector3i(base.x - 7 * B, g0 - 1, base.z - 6 * B)
	var hi := Vector3i(base.x + 8 * B, g0 + 1, base.z + B)
	_fill(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT)
	_fill(Vector3i(lo.x, g0, lo.z), Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.GRASS)
	_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 6 * B, hi.z), GameMaterials.AIR)
	await tree.create_timer(1.5).timeout   # the cleared strip reaches the terrain the aim ray reads
	var spot := func(dx: int) -> Vector3i: return Vector3i(base.x + dx * B, g0 + 1, base.z - 3 * B)
	var inv: PlayerInventory = main.player_inv
	r["survival"] = GameMode.rule(main.player.game_mode, "consumes_items")

	# ---- knapping: a flint axe head ----
	_stage("knapping")
	var a: Vector3i = spot.call(-4)
	main._hold("flint")
	var flints := inv.inv.count_of("flint")
	_click_top(a - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var k = main.blocks.at(a)
	r["knap_placed"] = k is KnapEntity
	r["flint_used"] = flints - inv.inv.count_of("flint")
	if k is KnapEntity:
		main.work.choose(a, "knap_flint_axe_head")
		main._hold("loose_stone")   # chipped with a stone in hand, never bare hands
		var todo: int = k.remaining()
		var clicks := 0
		while main.blocks.at(a) == k and clicks < 200:
			var cell := _first_marked(k)
			if clicks == todo / 2:
				await _shot(tree, "knapping", Vector3(a) + Vector3(0.5, 0.1, 0.5) * Settings.BLOCK)
			_chip(a, cell)
			clicks += 1
			await tree.process_frame   # one click per frame at most, like a player
		r["knap_clicks"] = clicks
		r["knap_cells"] = todo
		r["knap_done"] = main.blocks.at(a) == null and "flint_axe_head" in main.work.stats["finished"]

	# ---- clay forming: a crucible and a pickaxe mold ----
	_stage("clay forming")
	inv.pickup("clay_ball", 16)
	for job in [["form_crucible", -2], ["form_pickaxe_mold", 0]]:
		var b: Vector3i = spot.call(job[1])
		main._hold("clay_ball")
		var balls := inv.inv.count_of("clay_ball")
		_click_top(b - Vector3i.UP)
		await tree.create_timer(0.2).timeout
		main.work.choose(b, job[0])
		var c = main.blocks.at(b)
		var clicks := 0
		var shot := false
		while main.blocks.at(b) == c and c != null and clicks < 300:
			var y: int = c.active_layer()
			if not shot and y >= 2 or (not shot and job[0] == "form_pickaxe_mold" and y >= 1):
				shot = true
				await _shot(tree, job[0], Vector3(b) + Vector3(0.5, 0.1, 0.5) * Settings.BLOCK)
			var cell := _first_missing(c, y)
			main._aim_use(Vector3(b) + Vector3((cell.x + 0.75) / 8.0, y / 8.0, (cell.y + 0.75) / 8.0) * Settings.BLOCK)
			clicks += 1
			await tree.process_frame
		r[job[0] + "_clicks"] = clicks
		r[job[0] + "_balls"] = balls - inv.inv.count_of("clay_ball")
		r[job[0] + "_done"] = main.blocks.at(b) == null
	var raw := ["raw_crucible", "raw_pickaxe_mold"]
	for id in raw:
		r["made_" + id] = id in main.work.stats["finished"]
		inv.pickup(id, 1)   # as if walked over

	# ---- pit kilns ----
	_stage("pit kilns")
	inv.pickup("dry_grass", 8)
	inv.pickup("stick", 16)
	inv.pickup("firestarter", 1)
	var kilns := {"raw_crucible": spot.call(2), "raw_pickaxe_mold": spot.call(4)}
	for id in kilns:
		var p: Vector3i = kilns[id]
		main._hold(id)
		_click_top(p - Vector3i.UP)
		await tree.create_timer(0.2).timeout
		for i in GroundEntity.KILN_GRASS:
			main._hold("dry_grass")
			main._aim_use(Vector3(p) + Vector3(0.5, 0.15, 0.5) * Settings.BLOCK)
			await tree.process_frame
		for i in GroundEntity.KILN_STICKS:
			main._hold("stick")
			main._aim_use(Vector3(p) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
			await tree.process_frame
		main._hold("firestarter")
		main._aim_use(Vector3(p) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
		var g = main.blocks.at(p)
		r["kiln_lit_" + id] = g is GroundEntity and g.is_lit()
	await _shot(tree, "pit_kilns", Vector3(spot.call(3)) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
	await _fast_forward(int(Settings.pit_kiln_burn_s * 20.0) + 1)
	for id in kilns:
		var g = main.blocks.at(kilns[id])
		r["fired_" + id] = g.item if g is GroundEntity else "gone"

	# ---- firepit: melt copper-bearing stones in the crucible ----
	_stage("firepit")
	var f_at: Vector3i = spot.call(6)
	inv.pickup("firepit", 1)
	main._hold("firepit")
	_click_top(f_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var fp = main.blocks.at(f_at)
	r["firepit_placed"] = fp is FirepitEntity
	# pick the fired crucible up with an empty hand
	_empty_hand()
	main._aim_use(Vector3(kilns["raw_crucible"]) + Vector3(0.5, 0.3, 0.5) * Settings.BLOCK)
	r["crucible_picked_up"] = inv.inv.count_of("crucible") == 1
	if fp is FirepitEntity:
		inv.inv.remove("crucible", 1)
		fp.inv.set_slot(FirepitEntity.VESSEL, {"id": "crucible", "count": 1})
		fp.inv.set_slot(FirepitEntity.INPUT, {"id": "loose_copper_ore", "count": 4})
		fp.inv.set_slot(FirepitEntity.FUEL, {"id": "oak_log", "count": 4 * main.items.units("oak_log")})
		main._hold("firestarter")
		main._aim_use(Vector3(f_at) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
		r["firepit_lit"] = fp.is_lit()
		await _fast_forward(200)
		await _shot(tree, "firepit", Vector3(f_at) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
		await _fast_forward(220)
		r["molten"] = fp.inv.get_slot(FirepitEntity.VESSEL).get("id", "")
		if r["molten"] == "crucible_copper":
			fp.inv.set_slot(FirepitEntity.VESSEL, {})
			inv.pickup("crucible_copper", 1)

	# ---- casting ----
	_stage("casting")
	var m_at: Vector3i = kilns["raw_pickaxe_mold"]
	main._hold("crucible_copper")
	main._aim_use(Vector3(m_at) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
	var mold = main.blocks.at(m_at)
	r["cast"] = mold.cast_output if mold is GroundEntity else "no mold"
	r["crucible_back"] = inv.inv.count_of("crucible") == 1
	await _shot(tree, "casting", Vector3(m_at) + Vector3(0.5, 0.1, 0.5) * Settings.BLOCK)
	await _fast_forward(int(Settings.cast_cool_s * 20.0) + 1)
	_empty_hand()
	main._aim_use(Vector3(m_at) + Vector3(0.5, 0.2, 0.5) * Settings.BLOCK)
	r["head_in_hand"] = inv.inv.count_of("copper_pickaxe_head") == 1
	var pa := AssemblyEntity.new(main.recipes, Vector3i.ZERO)   # put together on the ground (ADR 0042)
	pa.parts.append("copper_pickaxe_head")
	r["pickaxe_recipe"] = pa.use(main.items.get_def("stick"))["done"].get("id", "")
	var pick: Dictionary = main.items.get_def("copper_pickaxe")
	r["stone_drops_with_copper"] = main.mining.drops("stone", pick)
	r["stone_drops_with_flint"] = main.mining.drops("stone", main.items.get_def("flint_pickaxe"))

	# ---- sieve ----
	_stage("sieve")
	var s_at: Vector3i = spot.call(-6)
	inv.pickup("sieve", 1)
	inv.pickup("gravel", 12 * main.items.units("gravel"))
	main._hold("sieve")
	_click_top(s_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var sv = main.blocks.at(s_at)
	r["sieve_placed"] = sv is SieveEntity
	var before: int = main.item_stats["spawned"]
	for n in 12:
		main._hold("gravel")
		main._aim_use(Vector3(s_at) + Vector3(0.5, 0.9, 0.5) * Settings.BLOCK)
		_empty_hand()
		for i in SieveEntity.SHAKES:
			await tree.process_frame
			main._aim_use(Vector3(s_at) + Vector3(0.5, 0.9, 0.5) * Settings.BLOCK)
	r["sifts"] = sv.sifted if sv is SieveEntity else 0
	r["sift_items_spawned"] = main.item_stats["spawned"] - before
	await _shot(tree, "all", Vector3(spot.call(0)) + Vector3(0.5, 0.0, 0.5) * Settings.BLOCK, 3.2)
	_stage("end")
	r["work"] = main.work.summary()
	r["quests"] = main.quests.summary() if main.quests else {}
	r["shots"] = shots
	print("debug stone_age " + JSON.stringify(r))


## Fills the world box [lo, hi) through the bubble in one native call, voxel by voxel when it cannot.
func _fill(lo: Vector3i, hi: Vector3i, m: int) -> void:
	if main.bubble.fill_box(lo, hi, m) >= 0:
		return
	for z in range(lo.z, hi.z):
		for y in range(lo.y, hi.y):
			for x in range(lo.x, hi.x):
				main.set_world_voxel(Vector3i(x, y, z), m)


## Frame number where a stage starts (lines spikes up with stages).
func _stage(name: String) -> void:
	if not r.has("stage_frames"):
		r["stage_frames"] = {}
	r["stage_frames"][name] = Engine.get_process_frames()


## Right click the top face of voxel `t`.
func _click_top(t: Vector3i) -> void:
	var hit: Vector3i = main._aim_use(Vector3(t) + Vector3(0.5, 0.999, 0.5))
	if not r.has("clicks"):
		r["clicks"] = []
	if r["clicks"].size() < 8:   # what the ray hit, for when a placement fails
		r["clicks"].append({"want": t, "hit": hit, "material": GameMaterials.name_of(main.get_world_voxel(hit)), "eye": Vector3i(main.player.camera.global_position.floor())})


func _empty_hand() -> void:
	for i in PlayerInventory.HOTBAR:
		if main.player_inv.inv.is_empty_slot(i):
			main.player_inv.select(i)
			return


func _first_marked(k: KnapEntity) -> Vector2i:
	for z in 8:
		for x in 8:
			if k.is_marked(x, z):
				return Vector2i(x, z)
	return Vector2i(-1, -1)


func _first_missing(c: ClayFormEntity, y: int) -> Vector2i:
	for z in 8:
		for x in 8:
			if c.is_target(x, y, z) and not c.is_filled(x, y, z):
				# the brush's low corner: step back so the 2x2 covers this cell
				return Vector2i(x, z)
	return Vector2i(0, 0)


## Left click on a knapping cell, aimed like a player would.
func _chip(a: Vector3i, cell: Vector2i) -> void:
	var at := Vector3(a) + Vector3((cell.x + 0.5) / 8.0, 1.0 / 8.0, (cell.y + 0.5) / 8.0) * Settings.BLOCK
	var cam: Camera3D = main.player.camera
	var dir := (at - cam.global_position).normalized()
	main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	main.work.hit(main.player._aim(), cam.global_position, -cam.global_transform.basis.z, false)


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
	if "stone_age_shots=0" in OS.get_cmdline_user_args():
		return   # reading the image back stalls the render thread ~100 ms: off for budget runs
	var keep: Vector3 = main.player.position
	var eye := at + Vector3(0, Settings.m2v(dist_m) * 0.8, Settings.m2v(dist_m))
	main.player.position = eye - Vector3(0, main.player.camera.position.y, 0)
	var cam: Camera3D = main.player.camera
	for i in 4:
		await tree.process_frame
		var dir := (at - cam.global_position).normalized()
		main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	await tree.create_timer(0.25).timeout
	var path: String = DebugCapture.out_dir + "/stone_age_%s.png" % name
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DebugCapture.out_dir))
	main.get_viewport().get_texture().get_image().save_png(path)
	shots.append(path)
	main.player.position = keep
	await tree.process_frame
