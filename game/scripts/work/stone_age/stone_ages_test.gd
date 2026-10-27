class_name StoneAgesTest
extends RefCounted

## Debug (flint_age_test=1 runs it after the ADR 0030 checks; stone_ages_test=1 runs it alone; ADR 0059):
## the four Stone Age stages through the player's own click code on a laid-out strip: bark peeled off
## a birch trunk, birch tar and tempered flint from earth ovens, a tempered head knapped and hafted
## with tar, a bone needle, a fur pelt dried and sewn into a cloak and worn, the atlatl and the bow, a
## rack smoking meat over a firepit, an axe roughout ground on sandstone, grain ground to flour, dough
## and bread, flax retted and a trap set in a pool, a loom weaving, cold-hammered copper, malachite in
## a pit kiln, an ember carried, a fishing line, a dugout burnt out and scraped, launched and paddled,
## a megalith quarried, levered and raised into a stone circle, a painting on a wall. Waiting is game
## hours on the calendar (StoneAge.now_days) plus block-entity ticks. Screenshots only in a rendered
## run (flint_age_shots=0 skips them); one JSON line "debug stone_ages {...}" says what happened.

var main: Node
var r := {}
var shots := []
var base := Vector3i.ZERO
var g0 := 0
var inv: PlayerInventory
var tree: SceneTree
const B := Settings.BLOCK


func run(p_main: Node) -> void:
	main = p_main
	tree = main.get_tree()
	inv = main.player_inv
	for i in inv.inv.size():
		inv.inv.set_slot(i, {})   # room for everything the test hands out
	await tree.create_timer(0.6).timeout
	main.player.autopilot_action = "look"
	base = BlockEntities.anchor_for(Vector3i(main.player.position.floor()))
	g0 = floori(float(main.world.surface_height(base.x, base.z)) / B) * B - 1
	for i in 200:   # the bubble takes the strip in one native fill once its chunks are loaded
		if _lay_out():
			break
		await tree.create_timer(0.1).timeout
	r["laid_out"] = _laid
	await tree.create_timer(1.5).timeout   # the strip reaches the terrain the aim ray reads
	var sections := [_tar_and_tempering, _hafting, _bone_and_fur, _throwing, _rack, _polishing, _grain, _water,
		_loom_and_copper, _ember_and_fishing, _dugout, _megaliths, _painting]
	for s in sections:
		await s.call()
	r["stone_age"] = main.stone_age.summary()
	r["quests"] = main.quests.summary()
	r["shots"] = shots
	print("debug stone_ages " + JSON.stringify(r))


# ---------------- the strip ----------------

func at(dx: int, dz: int) -> Vector3i:
	return Vector3i(base.x + dx * B, g0 + 1, base.z + dz * B)


## The ground block under `at(dx, dz)`.
func ground(dx: int, dz: int) -> Vector3i:
	return at(dx, dz) - Vector3i(0, B, 0)


var _laid := false

func _lay_out() -> bool:
	var lo := Vector3i(base.x - 11 * B, g0 - 3 * B, base.z - 17 * B)
	var hi := Vector3i(base.x + 11 * B, g0 + 1, base.z + 3 * B)
	# pieces an earlier test left on this ground go first (flint_age_test runs on the same spot)
	for e in main.blocks.all_work():
		var q: Vector3i = e.pos
		if q.x >= lo.x and q.x < hi.x and q.z >= lo.z and q.z < hi.z:
			main.work.remove_piece(e)
	if main.bubble.fill_box(lo, Vector3i(hi.x, g0, hi.z), GameMaterials.DIRT) < 0:
		var bb: SimBubble = main.bubble
		var clo := Vector3i((Vector3(lo) / Settings.CHUNK).floor())
		var chi := Vector3i((Vector3(Vector3i(hi.x, g0, hi.z) - Vector3i.ONE) / Settings.CHUNK).floor())
		r["lay_out_why"] = {"origin": bb.origin, "lo": clo, "hi": chi, "player": main.player.position}   # the bubble is not there yet
		return false
	_laid = true
	_fill(Vector3i(lo.x, g0, lo.z), Vector3i(hi.x, g0 + 1, hi.z), GameMaterials.GRASS)
	_fill(Vector3i(lo.x, g0 + 1, lo.z), Vector3i(hi.x, g0 + 1 + 8 * B, hi.z), GameMaterials.AIR)
	# a pool two blocks deep: 4 x 10 blocks
	_fill(ground(4, -14) - Vector3i(0, B, 0), ground(8, -4) + Vector3i(0, B, 0), GameMaterials.WATER_STATIC)
	_block(ground(3, -6), GameMaterials.SANDSTONE)          # a sandstone slab beside the pool
	_block(ground(1, -3), GameMaterials.SAND)               # sand for the tempering oven
	_block(ground(-3, -2), GameMaterials.STONE)             # a flat rock: the quern
	for x in [-8, -7, -6]:
		_block(ground(x, -12), GameMaterials.STONE)        # a row of rock to quarry
	_block(at(-6, 0), GameMaterials.STONE)                  # a rock to paint
	_block(at(-6, 0) + Vector3i(0, B, 0), GameMaterials.STONE)
	_block(at(-4, -8), GameMaterials.BIRCH_BARK)            # a birch trunk
	_block(at(-4, -8) + Vector3i(0, B, 0), GameMaterials.BIRCH_BARK)
	# a lying trunk for the canoe: 4 wide, 3 high, 24 long (6 blocks along z)
	_fill(at(-10, -10), at(-10, -10) + Vector3i(4, 3, 24), GameMaterials.WOOD)
	return true


func _fill(lo: Vector3i, hi: Vector3i, m: int) -> void:
	if main.bubble.fill_box(lo, hi, m) >= 0:
		return
	var d := hi - lo
	if d.x * d.y * d.z > 4096:
		push_warning("stone_ages_test: %s..%s is outside the loaded bubble" % [lo, hi])
		return
	for z in range(lo.z, hi.z):
		for y in range(lo.y, hi.y):
			for x in range(lo.x, hi.x):
				main.set_world_voxel(Vector3i(x, y, z), m)


func _block(p: Vector3i, m: int) -> void:
	_fill(p, p + Vector3i.ONE * B, m)


## The player stands a block and a half south of p (on the strip's ground), looking at nothing yet.
func _stand(p: Vector3i, dx: float = 0.5, dz: float = 3.5) -> void:
	main.player.position = Vector3(p.x + dx * B, g0 + 1.05, p.z + dz * B)
	main.player.velocity = Vector3.ZERO


func _give(id: String, n: int = 1) -> void:
	if inv.inv.count_of(id) < n:
		inv.pickup(id, n - inv.inv.count_of(id))


## Puts n of id in hotbar slot HAND (taking them from the inventory when they are there) and selects it.
const HAND := 7

func _hold(id: String, n: int = 1) -> void:
	var have := inv.inv.get_slot(HAND)
	if have.is_empty() or have["id"] != id or int(have["count"]) < n:
		inv.inv.remove(id, mini(inv.inv.count_of(id), n))
		inv.inv.set_slot(HAND, {"id": id, "count": n})
	inv.select(HAND)
	main.player._place_cooldown = 0.0


func _use(point: Vector3) -> Vector3i:
	main.player._place_cooldown = 0.0
	return main._aim_use(point)


func _click_top(t: Vector3i) -> void:
	_use(Vector3(t) + Vector3(0.5, 0.999, 0.5))


## A new or moved piece reaches the terrain the aim ray reads at the bubble's next push.
func _settle() -> void:
	await tree.create_timer(0.6).timeout


## The middle of a work piece's footprint.
func _mid(e: RefCounted) -> Vector3:
	var cells: Array[Vector3i] = BlockEntities.footprint(e)
	var lo: Vector3i = cells[0]
	var hi: Vector3i = cells[cells.size() - 1]
	return (Vector3(lo) + Vector3(hi) + Vector3.ONE) * 0.5


## Game hours on the calendar, with the block entities ticked over them.
func _hours(h: float) -> void:
	var cal = main.atmosphere.calendar
	var left := h
	while left > 0.0:
		var step := minf(left, 0.5)
		cal.advance(step / 24.0)
		StoneAge.now_days = cal.total()
		main.blocks.tick()
		left -= step
	await tree.process_frame


func _fast_forward(ticks: int) -> void:
	while ticks > 0:
		for i in mini(ticks, 100):
			main.blocks.tick()
		ticks -= 100
		await tree.process_frame


func _chip_all(a: Vector3i) -> int:
	var k = main.blocks.at(a)
	var clicks := 0
	_hold("loose_stone")
	while main.blocks.at(a) == k and k is KnapEntity and clicks < 80:
		var cell := Vector2i(-1, -1)
		for z in KnapEntity.N:
			for x in KnapEntity.N:
				if cell.x < 0 and k.is_marked(x, z):
					cell = Vector2i(x, z)
		if cell.x < 0:
			break
		var p := Vector3(a) + Vector3((cell.x + 0.5) / 8.0, 1.0 / 8.0, (cell.y + 0.5) / 8.0) * B
		var cam: Camera3D = main.player.camera
		var dir := (p - cam.global_position).normalized()
		main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
		main.work.hit(main.player._aim(), cam.global_position, -cam.global_transform.basis.z, false)
		clicks += 1
		await tree.process_frame
	return clicks


## Knaps `input` into recipe `rid` at spot (dx, dz) through the player's clicks.
func _knap(input: String, rid: String, dx: int, dz: int) -> bool:
	var a := at(dx, dz)
	_stand(a)
	_hold(input)
	_click_top(a - Vector3i.UP)
	await tree.process_frame
	if not (main.blocks.at(a) is KnapEntity):
		return false
	main.work.choose(a, rid)
	await _chip_all(a)
	await tree.create_timer(0.3).timeout   # the head pops out and lands
	return main.blocks.at(a) == null


## Picks up every item lying around (the player walks over them in play).
func _collect() -> void:
	await tree.create_timer(0.4).timeout
	for e in main._items_root.get_children():
		if e is ItemEntity and not e.is_queued_for_deletion() and e.position.distance_to(main.player.position) < Settings.m2v(12.0):
			var s: Dictionary = e.stack
			if inv.pickup(s["id"], int(s["count"]), s) == 0:
				e.queue_free()


func _shot(name: String, at_p: Vector3, dist_m: float = 1.6) -> void:
	if "flint_age_shots=0" in OS.get_cmdline_user_args() or DisplayServer.get_name() == "headless":
		return
	var keep: Vector3 = main.player.position
	var eye := at_p + Vector3(0, Settings.m2v(dist_m) * 0.8, Settings.m2v(dist_m))
	main.player.position = eye - Vector3(0, main.player.camera.position.y, 0)
	var cam: Camera3D = main.player.camera
	for i in 4:
		await tree.process_frame
		var dir := (at_p - cam.global_position).normalized()
		main.player.set_look(atan2(-dir.x, -dir.z), asin(dir.y))
	await tree.create_timer(0.3).timeout
	var path: String = DebugCapture.out_dir + "/stone_ages_%s.png" % name
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(DebugCapture.out_dir))
	main.get_viewport().get_texture().get_image().save_png(path)
	shots.append(path)
	main.player.position = keep
	await tree.process_frame


# ---------------- 1. Old Stone Age ----------------

func _tar_and_tempering() -> void:
	# bark off the birch trunk with a knife
	var trunk := at(-4, -8)
	_stand(trunk, 0.5, 3.0)
	_hold("flint_knife")
	var bark0 := inv.inv.count_of("birch_bark")
	for i in 6:
		_use(Vector3(trunk) + Vector3(0.5 * B, 0.3 * B + i * 0.9, B + 0.2))
		await tree.process_frame
	await _collect()
	r["bark_peeled"] = inv.inv.count_of("birch_bark") - bark0
	_give("birch_bark", 4)
	# the earth oven for tar: a shovel on soil, bark, dirt, sticks, fire
	for job in [["birch_bark", "dirt", 0, "tar"], ["flint", "sand", 1, "tempered"]]:
		var g := ground(job[2], -3)
		_stand(g + Vector3i(0, B, 0))
		_hold("flint_shovel")
		_click_top(g + Vector3i(0, B - 1, 0))
		await _settle()
		var o = main.blocks.at(g + Vector3i(0, B - 2, 0))
		r[job[3] + "_oven_dug"] = o is EarthOvenEntity
		if not (o is EarthOvenEntity):
			continue
		var aim := Vector3(o.pos) + Vector3(0.5 * B, 1.5, 0.5 * B)
		_hold(job[0], 4)
		for i in 4:
			_use(aim)
		_hold(job[1], 2 * main.items.units(job[1]))
		for i in 2:
			_use(aim)
		_hold("stick", 4)
		for i in 4:
			_use(aim)
		_hold("firestarter")
		_use(aim)
		r[job[3] + "_oven_lit"] = o.is_lit()
		r[job[3] + "_oven_state"] = o.summary()
		if job[3] == "tar":
			await _shot("earth_oven", aim)
		await _hours(3.2)
		inv.select(8)
		inv.inv.set_slot(8, {})   # an empty hand
		_use(aim)
		await _collect()
	r["birch_tar"] = inv.inv.count_of("birch_tar")
	r["tempered_flint"] = inv.inv.count_of("tempered_flint")


func _hafting() -> void:
	# a tempered head, then stick and tar on the ground: the axe lasts three times as long
	_give("tempered_flint", 1)
	r["tempered_knapped"] = await _knap("tempered_flint", "knap_flint_axe_head", 2, 0)
	await _collect()
	var head := {}
	for i in inv.inv.size():
		var s: Dictionary = inv.inv.get_slot(i)
		if not s.is_empty() and s["id"] == "flint_axe_head" and s.has("dur_x"):
			head = s.duplicate()
			head["count"] = 1
	r["tempered_head_dur_x"] = head.get("dur_x", 0.0)
	var a := at(3, 0)
	_stand(a)
	_hold("flint_axe_head")
	if not head.is_empty():
		inv.inv.set_slot(HAND, head.duplicate())
	_click_top(a - Vector3i.UP)
	await _settle()
	var aim := Vector3(a) + Vector3(0.5 * B, 0.5, 0.5 * B)
	var piece = main.blocks.at(a)
	r["haft_piece"] = piece.summary() if piece else "none"
	_hold("stick")
	_use(aim)
	_hold("birch_tar")
	_use(aim)
	r["haft_after"] = main.blocks.at(a).summary() if main.blocks.at(a) else "done"
	await _collect()
	var axe := {}
	for i in inv.inv.size():
		var s: Dictionary = inv.inv.get_slot(i)
		if not s.is_empty() and s["id"] == "flint_axe" and s.has("dur_x"):
			axe = s
	r["tar_axe_dur_x"] = axe.get("dur_x", 0.0)
	r["tar_axe_uses"] = Inventory.uses_left(main.items, axe) if not axe.is_empty() else 0


func _bone_and_fur() -> void:
	r["bone_needle"] = await _knap("bone", "knap_bone_needle", 4, 0)
	await _collect()
	r["has_needle"] = inv.inv.count_of("bone_needle")
	# a raw hide left unscraped dries into a pelt
	var h := at(5, 0)
	_stand(h)
	_hold("raw_hide")
	_click_top(h - Vector3i.UP)
	await _fast_forward(150 * 20 + 2)
	var hide = main.blocks.at(h)
	r["pelt_dried"] = hide.item if hide is GroundEntity else "gone"
	inv.inv.set_slot(8, {})
	inv.select(8)
	_use(Vector3(h) + Vector3(0.5 * B, 0.3, 0.5 * B))
	await _collect()
	r["pelts"] = inv.inv.count_of("fur_pelt")
	# sew a cloak on the ground: three pelts, two cordage, 24 needle strokes
	_give("fur_pelt", 3)
	var s := at(6, 0)
	_stand(s)
	_hold("fur_pelt", 3)
	_click_top(s - Vector3i.UP)
	await _settle()
	r["sew_placed"] = main.blocks.at(s) is StrokeEntity
	main.stone_age.choose(s, "sew_fur_cloak")
	var aim := Vector3(s) + Vector3(0.5 * B, 0.5, 0.5 * B)
	_hold("fur_pelt", 2)
	_use(aim)
	_use(aim)
	_hold("cordage", 2)
	_use(aim)
	_use(aim)
	await _shot("sewing", aim)
	_hold("bone_needle")
	for i in 30:
		if main.blocks.at(s) == null:
			break
		_use(aim)
		await tree.process_frame
	await _collect()
	r["cloak_sewn"] = inv.inv.count_of("fur_cloak")
	_hold("fur_cloak")
	_use(Vector3(s) + Vector3(0, 1, 0))
	r["worn"] = main.stone_age.clothing.summary()
	await tree.create_timer(0.3).timeout
	r["felt_c"] = main.survival.felt_c() if main.survival else 0.0
	r["env_c"] = main.survival.env_c if main.survival else 0.0


func _throwing() -> void:
	_stand(at(0, 1))
	main.player.set_look(0.0, 0.2)
	_give("flint_spear")
	_hold("atlatl")
	var n0: int = main.projectiles.stats["thrown"]
	main.player._use(true)
	r["atlatl_threw"] = main.projectiles.stats["thrown"] - n0
	r["atlatl_speed_m"] = snappedf(Settings.v2m(main.projectiles._flying[-1]["vel"].length()), 0.1) if main.projectiles.flying() > 0 else 0.0
	await tree.create_timer(1.0).timeout
	_hold("bow")
	_give("arrow", 4)
	main.player.set_look(PI, 0.1)
	main.player._use(true)   # draws the bow; release shoots (a full draw here)
	main.stone_age.draw_t = 1.0
	main.stone_age.loose()
	r["bow_shot"] = main.projectiles.stats["thrown"] - n0 - 1
	r["arrows_left"] = inv.inv.count_of("arrow")
	await tree.create_timer(1.0).timeout
	await _collect()


# ---------------- 2. Middle Stone Age ----------------

func _rack() -> void:
	var p := at(-1, -5)
	_stand(p)
	_hold("drying_rack")
	_click_top(p - Vector3i.UP)
	await _settle()
	var rack = main.blocks.at(p)
	r["rack_placed"] = rack is RackEntity
	if not (rack is RackEntity):
		return
	var aim := Vector3(p) + Vector3(0.5, 0.6, 0.5) * B
	_hold("raw_meat", 2)
	_use(aim)
	_use(aim)
	# a lit firepit beside it
	var f := at(0, -5)
	_stand(f)
	_hold("firepit")
	_click_top(f - Vector3i.UP)
	await _settle()
	var fp = main.blocks.at(f)
	if fp is FirepitEntity:
		fp.inv.set_slot(FirepitEntity.FUEL, {"id": "firewood", "count": 16})
		r["rack_fire_lit"] = fp.ignite()
	await tree.create_timer(1.2).timeout   # StoneAge looks for smoke once a second
	r["rack_smoky"] = rack.smoky
	await _shot("rack_smoking", aim)
	await _hours(10.5)
	r["rack_hung"] = rack.summary()["hung"]
	inv.inv.set_slot(8, {})
	inv.select(8)
	_use(aim)
	_use(aim)
	await _collect()
	r["smoked_meat"] = inv.inv.count_of("smoked_meat")


# ---------------- 3. New Stone Age ----------------

func _polishing() -> void:
	var g := ground(3, -6)
	_stand(g + Vector3i(0, B, 0), 0.5, 3.0)
	_hold("axe_roughout")
	_click_top(g + Vector3i(0, B - 1, 0))
	await _settle()
	var p := g + Vector3i(0, B, 0)
	var e = main.blocks.at(p)
	r["polish_placed"] = e is StrokeEntity and e.station == "sandstone"
	if not (e is StrokeEntity):
		return
	r["polish_water"] = e.water_ok
	var aim := Vector3(p) + Vector3(0.5 * B, 0.5, 0.5 * B)
	var sand := 0
	for i in 70:
		if main.blocks.at(p) != e:
			break
		if e.needs_grit():
			_hold("sand", main.items.units("sand"))
			_use(aim)
			sand += 1
			if i == 0:
				await _shot("polishing", aim)
		_hold("loose_stone")
		_use(aim)
		await tree.process_frame
	r["polish_grit"] = sand
	await _collect()
	r["polished_head"] = inv.inv.count_of("polished_axe_head")
	var a := at(2, -8)
	_stand(a)
	_hold("polished_axe_head")
	_click_top(a - Vector3i.UP)
	await _settle()
	var am := Vector3(a) + Vector3(0.5 * B, 0.5, 0.5 * B)
	_hold("stick")
	_use(am)
	_hold("cordage")
	_use(am)
	await _collect()
	r["polished_axe"] = inv.inv.count_of("polished_stone_axe")
	r["polished_axe_speed"] = main.mining.break_time("wood", main.items.get_def("flint_axe")) / maxf(main.mining.break_time("wood", main.items.get_def("polished_stone_axe")), 0.001)


func _grain() -> void:
	var g := ground(-3, -2)
	_stand(g + Vector3i(0, B, 0))
	_hold("einkorn", 2)
	_click_top(g + Vector3i(0, B - 1, 0))
	await _settle()
	var p := g + Vector3i(0, B, 0)
	var aim := Vector3(p) + Vector3(0.5 * B, 0.5, 0.5 * B)
	_use(aim)
	var e = main.blocks.at(p)
	r["quern"] = e.summary() if e is StrokeEntity else "none"
	_hold("loose_stone")
	for i in 14:
		_use(aim)
		await tree.process_frame
	await _collect()
	r["flour"] = inv.inv.count_of("flour")
	# wet at the pool, bake on the rack's firepit
	_stand(ground(4, -6), 0.5, 3.0)
	_hold("flour")
	_use(Vector3(ground(5, -8)) + Vector3(2, B + 0.5, 2))
	await _collect()
	r["dough"] = inv.inv.count_of("flatbread_dough")
	var fp = main.blocks.at(at(0, -5))
	if fp is FirepitEntity and r["dough"] > 0:
		fp.inv.set_slot(FirepitEntity.INPUT, {"id": "flatbread_dough", "count": 1})
		inv.inv.remove("flatbread_dough", 1)
		await _fast_forward(320)
		r["flatbread"] = fp.inv.get_slot(FirepitEntity.OUTPUT).get("id", "")


func _water() -> void:
	var w := ground(5, -10)
	_stand(w + Vector3i(0, B, 0), -1.5, 0.5)
	_hold("flax", 4)
	var aim := Vector3(w) + Vector3(0.5 * B, B - 0.5, 0.5 * B)
	_use(aim)
	await _settle()
	for i in 3:
		_use(aim)
		await tree.process_frame
	var ww: Array = main.blocks.all_of("water_work")
	r["flax_in_water"] = ww[0].summary() if not ww.is_empty() else "none"
	var t := ground(6, -12)
	_hold("basket_trap")
	_use(Vector3(t) + Vector3(0.5 * B, B - 0.5, 0.5 * B))
	await _settle()
	await _hours(36.5)
	inv.inv.set_slot(8, {})
	inv.select(8)
	_use(aim)
	await _collect()
	r["retted_flax"] = inv.inv.count_of("retted_flax")
	r["water_work"] = main.blocks.all_of("water_work").map(func(x: WaterWorkEntity) -> Dictionary: return x.summary())
	await _shot("water_work", Vector3(w) + Vector3(2, B, 2))


func _loom_and_copper() -> void:
	var l := at(-2, -1)
	_stand(l)
	_hold("loom")
	_click_top(l - Vector3i.UP)
	await _settle()
	var e = main.blocks.at(l)
	r["loom_placed"] = e is StrokeEntity and e.kind == "loom"
	var aim := Vector3(l) + Vector3(0.5, 0.5, 0.5) * B
	_hold("linen_thread", 4)
	for i in 4:
		_use(aim)
	_hold("stick")
	for i in 18:
		_use(aim)
		await tree.process_frame
	await _shot("loom", aim)
	await _collect()
	r["linen_cloth"] = inv.inv.count_of("linen_cloth")
	r["copper_awl"] = await _knap("copper_nugget", "hammer_copper_awl", -1, 1)
	await _collect()
	# malachite: a green stone fired in a pit kiln on soil
	var k := at(-5, -1)
	_stand(k)
	_hold("loose_copper_ore")
	_click_top(k - Vector3i.UP)
	await _settle()
	var g = main.blocks.at(k)
	r["malachite_on_ground"] = g is GroundEntity
	if g is GroundEntity:
		var ka := Vector3(k) + Vector3(0.5 * B, 0.5, 0.5 * B)
		_hold("dry_grass", 4)
		for i in 4:
			_use(ka)
		_hold("stick", 8)
		for i in 8:
			_use(ka)
		_hold("firestarter")
		_use(ka)
		await _fast_forward(int(Settings.pit_kiln_burn_s * 20.0) + 2)
		r["malachite_fired"] = g.item


func _ember_and_fishing() -> void:
	var fp = main.blocks.at(at(0, -5))
	_stand(at(0, -5))
	_hold("tinder_fungus")
	if fp is FirepitEntity:
		if not fp.is_lit():
			fp.inv.set_slot(FirepitEntity.FUEL, {"id": "firewood", "count": 8})
			fp.ignite()
		_use(Vector3(fp.pos) + Vector3(0.5 * B, 1.0, 0.5 * B))
	await _collect()
	r["ember"] = inv.inv.count_of("fire_ember")
	_stand(ground(4, -6), 0.5, 3.0)
	_hold("fishing_rod")
	var fish0 := inv.inv.count_of("raw_fish")
	_use(Vector3(ground(5, -7)) + Vector3(2, B + 0.5, 2))
	r["fish_cast_until"] = main.stone_age._fish_until
	await _hours(1.3)
	await tree.create_timer(0.3).timeout
	await _collect()
	r["fish_caught"] = inv.inv.count_of("raw_fish") - fish0


func _dugout() -> void:
	var trunk := at(-10, -10)
	_stand(trunk, 0.5, 7.0)
	_hold("flint_adze")
	_use(Vector3(trunk) + Vector3(2, 3, 12))
	await _settle()
	var d = main.blocks.at(trunk + Vector3i(1, 1, 12))
	r["dugout_marked"] = d.summary() if d is DugoutEntity else "none"
	if not (d is DugoutEntity):
		return
	var aim := _mid(d) + Vector3(0, 1.4, 0)
	_hold("dry_grass", 4)
	for i in 4:
		_use(aim)
	_hold("firestarter")
	_use(aim)
	r["dugout_lit"] = d.is_lit()
	await _shot("dugout_burning", aim)
	await _hours(Settings.dugout_burn_h + 0.2)
	_hold("flint_adze")
	for i in DugoutEntity.ADZE_STROKES + 4:
		if main.blocks.at(d.pos) != d:
			break
		_use(aim)
		await tree.process_frame
	await _collect()
	r["dugout_made"] = inv.inv.count_of("dugout_canoe")
	# launch it on the pool, get in, paddle north
	_give("dugout_canoe")
	_stand(ground(5, -5), 0.5, 2.0)
	main.player.set_look(0.0, -0.4)
	_hold("dugout_canoe")
	_use(Vector3(ground(5, -5)) + Vector3(2, B + 0.5, 2))
	var c: Canoes = main.stone_age.canoes
	r["canoe_launched"] = c.boats.size()
	if c.boats.is_empty():
		return
	var b: Dictionary = c.boats[-1]
	b["yaw"] = 0.0
	c.board(c.boats.size() - 1)
	b["auto_paddle"] = 1.0
	var from: Vector3 = b["pos"]
	await tree.create_timer(1.2).timeout
	b["auto_paddle"] = 0.0
	await tree.create_timer(0.3).timeout
	r["canoe_moved_m"] = snappedf(Settings.v2m((b["pos"] - from).length()), 0.01)
	r["canoe_glides_m_s"] = snappedf(Settings.v2m((b["vel"] as Vector3).length()), 0.01)
	r["canoe_on_water"] = absf(float(b["pos"].y) - float(g0 + 1)) < 1.5
	r["canoe_rider"] = main.player.seat != Vector3.INF
	await _shot("canoe", b["pos"], 2.5)
	main.player.stand_up()
	r["canoe"] = c.summary()


func _megaliths() -> void:
	var m: Megaliths = main.stone_age.megaliths
	var row := ground(-8, -12)
	main.player.position = Vector3(row.x - 2.5 * B, g0 + 1.05, row.z + 0.5 * B)
	_hold("wooden_wedge", 6)
	for i in Megaliths.WEDGES:
		_use(Vector3(row) + Vector3(0.5 * B, B - 0.001, 0.5 * B))
	await _settle()
	var e = main.blocks.at(row)
	r["megalith_quarried"] = e.summary() if e is MegalithEntity else "none"
	if not (e is MegalithEntity):
		return
	var top := func() -> Vector3: return _mid(main.blocks.at(e.pos) if main.blocks.at(e.pos) else e) + Vector3(0, 0.5 * B - 0.5, 0)
	_hold("oak_log", 2 * main.items.units("oak_log"))
	_use(top.call())
	_use(top.call())
	_hold("cordage", 2)
	_use(top.call())
	_use(top.call())
	r["megalith_ready"] = e.movable()
	_hold("lever")
	for i in Megaliths.PUSH_STROKES:
		_use(top.call())
	var moved: Array = main.blocks.all_of("megalith")
	r["megalith_pushed"] = moved.size() == 1 and moved[0].pos != row
	var s: MegalithEntity = moved[0] if moved.size() == 1 else null
	if s == null:
		return
	await _settle()
	await _shot("megalith_rollers", _mid(s), 3.0)
	Input.action_press("sprint")
	for i in Megaliths.RAISE_STROKES:
		_use(_mid(s) + Vector3(0, 0.5 * B - 0.5, 0))
	Input.action_release("sprint")
	var standing := m.standing_stones()
	r["megalith_raised"] = standing.size()
	if standing.is_empty():
		return
	# five more stones and a heel stone toward the midsummer sunrise, raised for the test
	var first: MegalithEntity = standing[0]
	var radius := 5 * B
	var centre := first.pos + Vector3i(radius, 0, 0)
	for i in range(1, 6):
		var ang := PI + TAU * i / 6.0
		var p := centre + Vector3i(roundi(cos(ang) * radius / B) * B, 0, roundi(sin(ang) * radius / B) * B)
		m.make(p, "y")
	var sun := m.sunrise_dir()
	m.make(centre + Vector3i(roundi(sun.x * radius * 2.2 / B) * B, 0, roundi(sun.y * radius * 2.2 / B) * B), "y")
	m._check_circles()
	r["circles"] = m.circles.size()
	await _settle()
	await _shot("stone_circle", Vector3(centre) + Vector3(0, B, 0), 9.0)
	if m.circles.is_empty():
		return
	# an empty hand on a stone of the circle: home
	_stand(first.pos, 0.5, 4.0)
	inv.inv.set_slot(8, {})
	inv.select(8)
	_use(Vector3(first.pos) + Vector3(0.5 * B, B, 0.5 * B + 2))
	r["home_at_circle"] = main._spawn_pos.distance_to(m.circles[0]["centre"]) < 1.0
	# the midsummer sunrise inside it
	var sol := m.solstice()
	var cal = main.atmosphere.calendar
	cal.advance(fposmod(float(sol["day_of_year"]) - cal.day_of_year(), cal.year_days()) - cal.time + float(sol["time"]))
	main.player.position = m.circles[0]["centre"] + Vector3(0, 1, 0)
	for i in 70:
		await tree.process_frame
	r["solstice"] = m.stats["solstices"]


func _painting() -> void:
	var rock := at(-6, 0)
	_stand(rock, 0.5, 3.5)
	_hold("red_paint")
	for i in 3:
		_use(Vector3(rock) + Vector3(0.5 * B, 1.5 + 3.0 * i, B + 0.01))
	r["paintings"] = main.stone_age.paintings.count()
	_use(Vector3(rock) + Vector3(0.5 * B, 1.5, B + 0.01))   # over the first: the next motif, not a fourth
	r["paintings_after_repaint"] = main.stone_age.paintings.count()
	await _shot("cave_painting", Vector3(rock) + Vector3(0.5 * B, 0.6 * B, B), 1.2)
