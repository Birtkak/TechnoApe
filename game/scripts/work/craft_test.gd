class_name CraftTest
extends RefCounted

## Debug (craft_test=1, ADR 0042): crafting in the world through the player's own click code on a
## cleared strip of grass. A flint axe is put together on the ground (head, stick, cordage); a
## firepit is worked by hand (logs, meat, firestarter; the cooked meat taken off, then an ingot
## heated on it); a stone anvil takes the hot ingot and the hammer drives it into a hammer head,
## the player walking round the anvil so each blow pushes the metal the right way; the piece comes
## off half done and goes back on; a nearly worn axe breaks. ADR 0043: the firepit is worked with
## held right clicks (a second each, things taken off pop out beside it), a stick on the ground is
## picked up whole by a held right click, and digging one of its voxels takes all of it. Waiting is fast-forwarded by ticking
## the block entities. Screenshots go to the run's out dir (craft_shots=0 skips them); one JSON line
## "debug craft {...}" says what happened.

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
	g0 = floori(float(main.world.surface_height(base.x, base.z)) / B) * B - 1
	var lo := Vector3i(base.x - 8 * B, g0 - 2 * B, base.z - 8 * B)
	var hi := Vector3i(base.x + 8 * B, g0 + 1, base.z + B)
	_fill(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT)
	_fill(Vector3i(lo.x, g0, lo.z), Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.GRASS)
	_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 7 * B, hi.z), GameMaterials.AIR)
	await tree.create_timer(1.5).timeout
	var inv: PlayerInventory = main.player_inv
	var at := func(dx: int, dz: int) -> Vector3i: return Vector3i(base.x + dx * B, g0 + 1, base.z + dz * B)

	# ---- a flint axe, part by part on the ground ----
	inv.pickup("flint_axe_head", 1)
	inv.pickup("stick", 4)
	inv.pickup("cordage", 1)
	var a_at: Vector3i = at.call(-3, -3)
	main._hold("flint_axe_head")
	_click_top(a_at - Vector3i.UP)
	await tree.process_frame
	var asm = main.blocks.at(a_at)
	r["assembly_placed"] = asm is AssemblyEntity
	main._hold("cordage")
	_use_on(a_at, Vector3(0.5, 0.05, 0.5))
	r["binding_first_refused"] = asm is AssemblyEntity and asm.parts.size() == 1
	main._hold("stick")
	_use_on(a_at, Vector3(0.5, 0.05, 0.5))
	r["parts_after_stick"] = asm.parts.duplicate() if asm is AssemblyEntity else []
	await _shot(tree, "assembly", Vector3(a_at) + Vector3(0.5, 0.0, 0.5) * B, 1.1)
	_empty_hand()
	_use_on(a_at, Vector3(0.5, 0.05, 0.5))
	r["undo_gave_stick"] = inv.inv.count_of("stick") == 4
	main._hold("stick")
	_use_on(a_at, Vector3(0.5, 0.05, 0.5))
	main._hold("cordage")
	_use_on(a_at, Vector3(0.5, 0.05, 0.5))
	r["assembly_gone"] = main.blocks.at(a_at) == null
	await _pick_up_near(tree, a_at)
	r["flint_axe"] = inv.inv.count_of("flint_axe")

	# ---- a firepit by hand: logs, meat, light it, take the meat off, then heat an ingot ----
	var f_at: Vector3i = at.call(0, -3)
	inv.pickup("firepit", 1)
	main._hold("firepit")
	_click_top(f_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var fp = main.blocks.at(f_at)
	r["firepit_placed"] = fp is FirepitEntity
	inv.pickup("oak_log", 4 * main.items.units("oak_log"))
	main._hold("oak_log")
	await _hold_on(f_at, Vector3(0.5, 0.2, 0.5), 4)   # held: a log a second (ADR 0043)
	r["logs_on"] = fp.inv.get_slot(FirepitEntity.FUEL).get("count", 0) / main.items.units("oak_log") if fp is FirepitEntity else 0
	inv.pickup("raw_meat", 2)
	main._hold("raw_meat")
	_use_on(f_at, Vector3(0.5, 0.2, 0.5))
	r["meat_needs_a_hold"] = fp is FirepitEntity and fp.inv.is_empty_slot(FirepitEntity.INPUT)
	await _hold_on(f_at, Vector3(0.5, 0.2, 0.5))
	inv.pickup("firestarter", 1)
	main._hold("firestarter")
	_use_on(f_at, Vector3(0.5, 0.2, 0.5))
	r["firepit_lit"] = fp is FirepitEntity and fp.is_lit()
	r["firestarter_wear"] = int(inv.held().get("wear", 0))
	await tree.create_timer(0.3).timeout
	await _shot(tree, "firepit_cooking", Vector3(f_at) + Vector3(0.5, 0.2, 0.5) * B, 1.1)
	await _fast_forward(301)
	_empty_hand()
	await _hold_on(f_at, Vector3(0.5, 0.2, 0.5))
	r["meat_popped_out"] = inv.inv.count_of("cooked_meat") == 0   # it lies beside the fire, not in the hand
	await _pick_up_near(tree, f_at)
	r["cooked_meat"] = inv.inv.count_of("cooked_meat")
	inv.pickup("copper_ingot", 1)
	main._hold("copper_ingot")
	await _hold_on(f_at, Vector3(0.5, 0.2, 0.5))
	await _fast_forward(int(Settings.metal_heat_s * 20.0) + 4)
	await _shot(tree, "firepit_ingot", Vector3(f_at) + Vector3(0.5, 0.2, 0.5) * B, 1.1)
	_empty_hand()
	await _hold_on(f_at, Vector3(0.5, 0.2, 0.5))
	await _pick_up_near(tree, f_at)
	var slot := _slot_of(inv, "copper_ingot")
	r["ingot_heat"] = snappedf(Inventory.heat_of(inv.inv.get_slot(slot)), 0.01) if slot >= 0 else -1.0

	# ---- the anvil: lay the hot ingot on, pick the hammer head, drive the metal into shape ----
	var n_at: Vector3i = at.call(3, -3)
	inv.pickup("stone_anvil", 1)
	main._hold("stone_anvil")
	_click_top(n_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var anvil = main.blocks.at(n_at)
	r["anvil_placed"] = anvil is AnvilEntity
	if anvil is AnvilEntity and slot >= 0:
		inv.select(slot)
		_use_on(n_at, Vector3(0.5, 0.6, 0.5))
		r["piece_started"] = anvil.has_piece()
		main.work.choose(n_at, "smith_copper_hammer_head")
		inv.pickup("copper_hammer", 1)
		main._hold("copper_hammer")
		# the lump is columns 2..5 x rows 2..5, two high; the head is rows 3..4 from column 1 to 6
		var blows := []
		for x in range(2, 6):
			blows += [[x, 2, Vector2i(0, 1)], [x, 2, Vector2i(0, 1)], [x, 5, Vector2i(0, -1)], [x, 5, Vector2i(0, -1)]]
		var half := blows.size()
		for z in [3, 4]:
			blows += [[2, z, Vector2i(-1, 0)], [3, z, Vector2i(-1, 0)], [2, z, Vector2i(-1, 0)]]
			blows += [[5, z, Vector2i(1, 0)], [4, z, Vector2i(1, 0)], [5, z, Vector2i(1, 0)]]
		var landed := 0
		for i in blows.size():
			if i == half:
				await _shot(tree, "anvil_half", Vector3(n_at) + Vector3(0.5, 0.6, 0.5) * B, 1.1)
				# off and back on, as if to reheat it
				_empty_hand()
				_use_on(n_at, Vector3(0.5, 0.6, 0.5))
				r["piece_taken"] = inv.inv.count_of("copper_work_piece") == 1 and not anvil.has_piece()
				main._hold("copper_work_piece")
				_use_on(n_at, Vector3(0.5, 0.6, 0.5))
				r["piece_back"] = anvil.has_piece() and anvil.has_recipe()
				main._hold("copper_hammer")
			var b: Array = blows[i]
			var before: int = anvil.hits
			await _blow(tree, anvil, b[0], b[1], b[2])
			landed += anvil.hits - before
		r["blows_landed"] = landed
		r["blows"] = blows.size()
		r["anvil_left"] = anvil.summary()
		r["hammer_wear"] = int(inv.held().get("wear", 0))
		await _pick_up_near(tree, n_at)
		r["hammer_head"] = inv.inv.count_of("copper_hammer_head")
		await _shot(tree, "anvil_done", Vector3(n_at) + Vector3(0.5, 0.6, 0.5) * B, 1.1)

	# ---- sticks on the ground (ADR 0043): a held right click picks one up whole; digging one voxel takes all of it ----
	var t1: Vector3i = at.call(-3, -5)
	var t2: Vector3i = at.call(1, -5)
	for x in 10:
		main.set_world_voxel(t1 + Vector3i(x, 0, 0), GameMaterials.TWIGS)
		main.set_world_voxel(t2 + Vector3i(x, 0, 0), GameMaterials.TWIGS)
	await tree.create_timer(1.0).timeout
	_empty_hand()
	var sticks_before := inv.inv.count_of("stick")
	_use_on(t1 + Vector3i(3, 0, 0), Vector3(0.1, 0.12, 0.1))
	r["twig_needs_a_hold"] = main.get_world_voxel(t1) == GameMaterials.TWIGS
	var aim: VoxelRaycastResult = main.player._aim()
	r["twig_aim"] = [aim.position, GameMaterials.name_of(main.get_world_voxel(aim.position)), main.work.hold_key(aim)] if aim else ["none"]
	await _hold_on(t1 + Vector3i(3, 0, 0), Vector3(0.1, 0.12, 0.1))
	var left := 0
	for x in 10:
		left += 1 if main.get_world_voxel(t1 + Vector3i(x, 0, 0)) == GameMaterials.TWIGS else 0
	r["twig_voxels_left_after_hold"] = left
	await _pick_up_near(tree, t1 + Vector3i(4, 0, 0))
	r["sticks_from_hold"] = inv.inv.count_of("stick") - sticks_before
	r["dig_takes_whole_twig"] = main.player._dig_selection(t2 + Vector3i(7, 0, 0), 1).size()
	r["pickups_popped"] = main.pickups_popped

	# ---- wear: an axe one use from breaking ----
	var broke_before: int = main.tools_broken
	inv.pickup("copper_axe", 1, {"wear": int(main.items.get_def("copper_axe")["durability"]) - 1})
	main._hold("copper_axe")
	main.player._wear(1)
	r["axe_broke"] = main.tools_broken - broke_before == 1 and inv.inv.count_of("copper_axe") == 0

	await _part_d(tree, inv, at)

	main.atmosphere.set_time_of_day(0.45)
	await _shot(tree, "all", Vector3(at.call(0, -3)) + Vector3(0.5, 0.0, 0.5) * B, 3.5)
	r["work"] = main.work.summary()
	r["work"].erase("pieces")
	r["shots"] = shots
	print("debug craft " + JSON.stringify(r))


## Part D (bug reports 2026-10-25, ADR 0055): coal on a firepit, a firepit broken through a voxel that is
## not its anchor (no ghosts), a clay block formed with Shift + right click (a plain click places it),
## a block built onto a seashell, and the offhand hammer.
func _part_d(tree: SceneTree, inv: PlayerInventory, at: Callable) -> void:
	var c_at: Vector3i = at.call(-3, -6)
	inv.pickup("firepit", 1)
	main._hold("firepit")
	_click_top(c_at - Vector3i.UP)
	await tree.create_timer(0.2).timeout
	var cp = main.blocks.at(c_at)
	inv.pickup("coal", 4)
	main._hold("coal")
	_use_on(c_at, Vector3(0.5, 0.2, 0.5))
	r["coal_needs_a_hold"] = cp is FirepitEntity and cp.inv.is_empty_slot(FirepitEntity.FUEL)
	await _hold_on(c_at, Vector3(0.5, 0.2, 0.5), 2)
	r["coal_on"] = int(cp.inv.get_slot(FirepitEntity.FUEL).get("count", 0)) if cp is FirepitEntity else -1
	var cells: Array[Vector3i] = BlockEntities.footprint(cp) if cp != null else []
	main.player.voxel_broken.emit(c_at + Vector3i(3, 1, 2), [])   # a corner voxel, not the anchor
	await tree.process_frame
	await tree.process_frame
	var ghosts := 0
	for c in cells:
		if main.get_world_voxel(c) == GameMaterials.WORK_SPOT:
			ghosts += 1
	r["firepit_break_ghost_voxels"] = ghosts
	r["firepit_break_view_gone"] = main.blocks.at(c_at) == null and not main.work._views.has(c_at)

	# a clay block: Shift + right click starts clay forming and the form takes the block's voxels as balls
	var k_at: Vector3i = at.call(-1, -6)
	var u: int = main.items.units("clay")
	inv.pickup("clay", 2 * u)
	main._hold("clay")
	main.player.autopilot_sneak = true
	_click_top(k_at - Vector3i.UP)
	main.player.autopilot_sneak = false
	await tree.process_frame
	r["clay_block_forms"] = main.blocks.at(k_at) is ClayFormEntity
	if main.blocks.at(k_at) is ClayFormEntity:
		main.work.choose(k_at, "form_ingot_mold")
		var before := inv.inv.count_of("clay")
		_use_on(k_at, Vector3(0.3, 0.02, 0.3))
		r["clay_block_cells_added"] = main.work.stats["clay_cells"]
		r["clay_block_voxels_used"] = before - inv.inv.count_of("clay")
	var p_at: Vector3i = at.call(1, -6)
	_click_top(p_at - Vector3i.UP)
	await tree.process_frame
	r["clay_block_places"] = main.blocks.at(p_at) == null and main.get_world_voxel(p_at) == GameMaterials.CLAY

	# a seashell on the ground: a seashell block in hand builds onto it instead of picking it up
	var shell := GameMaterials.id_of("seashell")
	var s_at: Vector3i = at.call(3, -6)
	main.set_world_voxel(s_at, shell)
	await tree.process_frame
	inv.pickup("seashell", 4)
	main._hold("seashell")
	var aimed: Vector3i = main._aim_use(Vector3(s_at) + Vector3(0.5, 0.999, 0.5))
	await tree.process_frame
	r["shell_aimed"] = aimed == s_at
	r["shell_no_hold"] = main.player._hold_at == ""
	var beside := 0   # new shell voxels lie against the aimed face (top or side, as the ray met it)
	for d in [Vector3i.UP, Vector3i.LEFT, Vector3i.RIGHT, Vector3i.FORWARD, Vector3i.BACK]:
		if main.get_world_voxel(s_at + d) == shell:
			beside += 1
	r["shell_built_on"] = beside >= 1   # a voxel item: the brush lays a patch of it

	# the offhand: a hammer there counts for smithing, and wears there
	inv.pickup("copper_hammer", 1)
	main._hold("copper_hammer")
	inv.swap_hands()
	r["offhand_hammer_hand"] = main.work._tool_hand("hammer")
	main.work._wear_hand("off")
	r["offhand_hammer_wear"] = int(inv.offhand().get("wear", 0))
	inv.swap_hands()


## A hammer blow from the side the push goes away from: the player stands 0.8 m off the anvil
## opposite `push`, looks down at the top of column (x, z) and the left click goes through the work system.
func _blow(tree: SceneTree, anvil: AnvilEntity, x: int, z: int, push: Vector2i) -> void:
	var k := float(Settings.BLOCK) / AnvilEntity.N
	var top := Vector3(anvil.pos) + Vector3((x + 0.5) * k, (AnvilEntity.BASE_H + anvil.height(x, z)) * k - 0.02, (z + 0.5) * k)
	var back := Vector3(-push.x, 0, -push.y) * Settings.m2v(0.8)
	main.player.position = top + back + Vector3(0, Settings.m2v(0.7), 0) - Vector3(0, main.player.camera.position.y, 0)
	await tree.process_frame
	var cam: Camera3D = main.player.camera
	var dir := (top - cam.global_position).normalized()
	main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	var hit: VoxelRaycastResult = main.player._aim()
	main.work.hit(hit, cam.global_position, -cam.global_transform.basis.z, false)


func _slot_of(inv: PlayerInventory, id: String) -> int:
	for i in PlayerInventory.HOTBAR:
		if inv.inv.get_slot(i).get("id", "") == id:
			return i
	return -1


func _empty_hand() -> void:
	for i in PlayerInventory.HOTBAR:
		if main.player_inv.inv.is_empty_slot(i):
			main.player_inv.select(i)
			return


## Walks onto the spot so what popped out of a finished piece is picked up.
func _pick_up_near(tree: SceneTree, p: Vector3i) -> void:
	var keep: Vector3 = main.player.position
	main.player.position = Vector3(p) + Vector3(0.5, 0.0, 1.6) * Settings.BLOCK   # beside it: an anvil is solid
	await tree.create_timer(Settings.item_pickup_delay_s + 0.5).timeout
	main.player.position = keep
	await tree.process_frame


## Right click held at a point of the piece anchored at p (block fractions) for `times` holds (ADR 0043).
func _hold_on(p: Vector3i, frac: Vector3, times: int = 1) -> void:
	var tree := main.get_tree()
	var at := Vector3(p) + frac * Settings.BLOCK
	var cam: Camera3D = main.player.camera
	var dir := (at - cam.global_position).normalized()
	main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	main.player.autopilot_hold = true
	main.player._use(true)
	await tree.create_timer(Settings.hold_use_s * times + 0.15).timeout
	main.player.autopilot_hold = false
	await tree.process_frame


## Right click at a point of the piece anchored at p (block fractions).
func _use_on(p: Vector3i, frac: Vector3) -> void:
	main._aim_use(Vector3(p) + frac * Settings.BLOCK)


func _fill(lo: Vector3i, hi: Vector3i, m: int) -> void:
	if main.bubble.fill_box(lo, hi, m) >= 0:
		return
	for z in range(lo.z, hi.z):
		for y in range(lo.y, hi.y):
			for x in range(lo.x, hi.x):
				main.set_world_voxel(Vector3i(x, y, z), m)


func _click_top(t: Vector3i) -> void:
	main._aim_use(Vector3(t) + Vector3(0.5, 0.999, 0.5))


func _fast_forward(ticks: int) -> void:
	var tree := main.get_tree()
	while ticks > 0:
		for i in mini(ticks, 100):
			main.blocks.tick()
		ticks -= 100
		await tree.process_frame


func _shot(tree: SceneTree, name: String, at: Vector3, dist_m: float = 1.4) -> void:
	if "craft_shots=0" in OS.get_cmdline_user_args():
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
	var path: String = DebugCapture.out_dir + "/craft_%s.png" % name
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DebugCapture.out_dir))
	main.get_viewport().get_texture().get_image().save_png(path)
	shots.append(path)
	main.player.position = keep
	await tree.process_frame
