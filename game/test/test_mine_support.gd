extends GdUnitTestSuite

## ADR 0142, mines need shoring: the support rule (RigidPieces.support_*), its warning and fall (MineSupport),
## on a 12 x 8 x 12 m SimWorld of solid stone (span 2.5 m: 1 m + 1 m x hardness 1.5).

const N := Vector3i(6, 4, 6)
const FLOOR := 16
const ROOF := 40          # a room dug from y 16 to 40 is 3 m tall; its roof cells are at y = 40


class FakeMain extends Node:
	var sim: SimWorld
	var fx = null
	var chat = null
	var mobs = null
	var blocks = null
	var _furniture_views := {}

	func get_world_voxel(c: Vector3i) -> int:
		return sim.get_voxel(c)

	func set_world_voxel(c: Vector3i, m: int) -> void:
		sim.set_voxel(c, m)


class FakeBubble extends RefCounted:
	var origin := Vector3i.ZERO
	var loaded := true

	func is_chunk_loaded(_lc: Vector3i) -> bool:
		return loaded


class FakeMainBubble extends FakeMain:
	var bubble := FakeBubble.new()


func _world() -> Array:
	var sim := SimWorld.new()
	sim.configure(N, 2, 1)
	sim.set_materials(GameMaterials.defs())
	sim.mark_all_loaded(true)
	sim.fill_box(Vector3i.ZERO, N * 16, GameMaterials.STONE)
	var rp := RigidPieces.new()
	rp.setup(sim, GameMaterials.defs(), GameMaterials.ATLAS_TILES)
	rp.set_origin(Vector3i.ZERO)
	rp.support_setup(MineSupport.spans(MiningTable.load_default()))
	return [sim, rp]


## A room of x0..x1, z0..z1 (exclusive), floor to roof, emptied; marked dug unless `natural`.
func _room(sim: SimWorld, rp: RigidPieces, x0: int, x1: int, z0: int, z1: int, natural := false) -> void:
	sim.fill_box(Vector3i(x0, FLOOR, z0), Vector3i(x1, ROOF, z1), GameMaterials.AIR)
	if not natural:
		rp.support_mark_box(Vector3i(x0, FLOOR, z0), Vector3i(x1 - 1, ROOF - 1, z1 - 1))


func _scan(rp: RigidPieces, x0: int, x1: int, z0: int, z1: int, props := PackedInt32Array(), check := PackedInt32Array()) -> Dictionary:
	return rp.support_scan({"lo": Vector3i(x0, FLOOR, z0), "hi": Vector3i(x1 - 1, ROOF - 1, z1 - 1), "reach": 32, "props": props, "check": check})


## A prop under the middle of the roof at (x, z): radius and capacity from its furniture kind.
func _prop(kind: String, x: int, z: int) -> PackedInt32Array:
	var pd: Dictionary = FurnitureEntity.KINDS[kind]["prop"]
	return PackedInt32Array([x, ROOF, z, int(Settings.m2v(pd["radius_m"])), int(pd["capacity_m2"] * 64.0)])


func test_spans_follow_hardness() -> void:
	var s := MineSupport.spans(MiningTable.load_default())
	assert_float(s[GameMaterials.STONE]).is_equal_approx(Settings.m2v(2.5), 0.01)
	assert_float(s[GameMaterials.AIR]).is_less(0.0)
	assert_float(s[GameMaterials.FURNITURE]).is_less(0.0)   # a prop's own voxels are no wall
	assert_float(s[GameMaterials.WOOD]).is_equal(0.0)        # wood holds and never caves in
	assert_float(MineSupport.warn_s(1.05)).is_greater(MineSupport.warn_s(1.8))   # small overloads creak longer


func test_a_3m_tunnel_stands() -> void:
	var w := _world()
	_room(w[0], w[1], 8, 88, 36, 60)   # 10 m long, 3 m wide
	var r := _scan(w[1], 8, 88, 36, 60)
	assert_int(int(r["dug_nodes"])).is_greater(1000)
	assert_array(r["clusters"]).is_empty()


func test_a_7m_room_fails_in_the_middle() -> void:
	var w := _world()
	_room(w[0], w[1], 20, 76, 20, 76)
	var r := _scan(w[1], 20, 76, 20, 76)
	assert_int(r["clusters"].size()).is_equal(1)
	var cl: Dictionary = r["clusters"][0]
	assert_float(float(cl["ratio"])).is_between(1.2, 1.6)          # 3.5 m from the walls, 2.5 m span
	assert_float(cl["center"].x).is_between(44.0, 52.0)
	assert_int(int(cl["count"])).is_between(150, 600)               # the middle 2 x 2 m, not the edges


func test_a_pillar_in_the_middle_holds_it() -> void:
	var w := _world()
	_room(w[0], w[1], 20, 76, 20, 76)
	var r := _scan(w[1], 20, 76, 20, 76, _prop("stone_pillar", 48, 48))
	assert_array(r["clusters"]).is_empty()
	assert_int(int(r["props"][0]["load"])).is_greater(100)
	assert_bool(r["props"][0]["failed"]).is_false()


func test_an_overloaded_prop_fails() -> void:
	var w := _world()
	_room(w[0], w[1], 8, 88, 8, 88)   # 10 x 10 m: a timber prop cannot carry the middle
	var r := _scan(w[1], 8, 88, 8, 88, _prop("timber_prop", 48, 48))
	assert_bool(r["props"][0]["failed"]).is_true()
	assert_int(r["clusters"].size()).is_greater(0)


func test_generated_caves_never_fall() -> void:
	var w := _world()
	_room(w[0], w[1], 8, 88, 8, 88, true)   # a cave hall as the generator made it: nothing dug
	assert_array(_scan(w[1], 8, 88, 8, 88)["clusters"]).is_empty()
	var rp: RigidPieces = w[1]
	_room(w[0], w[1], 8, 88, 8, 88)
	rp.support_clear()   # a reload forgets the marks: what stood when saved stands
	assert_array(_scan(w[1], 8, 88, 8, 88)["clusters"]).is_empty()


func test_dug_marks_survive_the_save() -> void:
	var w := _world()
	var rp: RigidPieces = w[1]
	_room(w[0], rp, 20, 76, 20, 76)
	rp.support_mark_box(Vector3i(-9, -30, 5), Vector3i(-1, -21, 13))   # negative coordinates pack too
	var chunks := rp.support_dug_chunks()
	assert_int(chunks.size() % 5).is_equal(0)
	rp.support_clear()
	assert_array(_scan(rp, 20, 76, 20, 76)["clusters"]).is_empty()
	rp.support_load_chunks(chunks)
	assert_int(_scan(rp, 20, 76, 20, 76)["clusters"].size()).is_equal(1)
	assert_int(int(rp.support_stats()["dug_blocks"])).is_equal(14 * 6 * 14 + 3 * 3 * 3)
	var w2 := _world()
	w2[1].support_load_chunks(chunks)
	var a := Array(w2[1].support_dug_chunks())
	var b := Array(rp.support_dug_chunks())
	assert_int(a.size()).is_equal(b.size())


func test_a_creaking_roof_keeps_its_clock_over_a_reload() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	_room(sim, rp, 20, 76, 20, 76)
	var fake := FakeMain.new()
	fake.sim = sim
	var sup := MineSupport.new()
	sup.main = fake
	sup.rp = rp
	sup.take([_scan(rp, 20, 76, 20, 76)])
	sup._now = 10.0
	var left: float = sup.warnings.values()[0]["t_fail"] - sup._now
	var saved := sup.to_save()
	sup.free()
	rp.support_clear()   # a fresh game: nothing in memory
	var sup2 := MineSupport.new()
	sup2.main = fake
	sup2.rp = rp
	sup2._now = 500.0
	assert_bool(sup2.load_save(saved)).is_true()
	assert_int(sup2.warnings.size()).is_equal(1)
	assert_float(sup2.warnings.values()[0]["t_fail"] - sup2._now).is_equal_approx(left, 0.01)
	assert_int(_scan(rp, 20, 76, 20, 76)["clusters"].size()).is_equal(1)   # the marks came back too
	sup2._now += left + 1.0
	for id in sup2.warnings.keys():
		sup2._warn_step(id, sup2.warnings[id])
	assert_int(sup2.stats_d["collapses"]).is_equal(1)
	sup2.free()
	fake.free()


func test_an_unloaded_roof_waits_and_falls_once_loaded() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	_room(sim, rp, 20, 76, 20, 76)
	var fake := FakeMainBubble.new()
	fake.sim = sim
	var sup := MineSupport.new()
	sup.main = fake
	sup.rp = rp
	sup.take([_scan(rp, 20, 76, 20, 76)])
	var id: int = sup.warnings.keys()[0]
	fake.bubble.loaded = false   # reloaded far away: its chunks are not in the simulation
	sup._now += 500.0
	sup._warn_step(id, sup.warnings[id])
	assert_int(sup.stats_d["collapses"]).is_equal(0)
	assert_float(sup.warnings[id]["t_fail"] - sup._now).is_equal_approx(MineSupport.REWARN_S, 0.01)
	fake.bubble.loaded = true
	sup._now += 1.0
	sup._warn_step(id, sup.warnings[id])
	assert_int(sup.stats_d["collapses"]).is_equal(0)   # it creaks again first
	sup._now += MineSupport.REWARN_S
	sup._warn_step(id, sup.warnings[id])
	assert_int(sup.stats_d["collapses"]).is_equal(1)
	sup.free()
	fake.free()


func test_the_roof_drops_as_one_rock_piece() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	_room(sim, rp, 20, 76, 20, 76)
	var cl: Dictionary = _scan(rp, 20, 76, 20, 76)["clusters"][0]
	var id := rp.support_drop(cl["cells"], 4, Vector3(48, 200, 48))
	assert_int(id).is_greater(0)
	assert_int(sim.get_voxel(Vector3i(48, ROOF, 48))).is_equal(GameMaterials.AIR)   # left the roof
	assert_int(int(rp.get_info(id)["wood"])).is_equal(int(cl["count"]) * 4)       # four voxels of rock per roof cell
	var settled := PackedInt32Array()
	for t in 30 * 10:
		settled = rp.step(1.0 / 30.0)
		if not settled.is_empty():
			break
	assert_int(settled.size()).is_equal(1)
	var landed := 0
	for x in range(40, 56):
		for z in range(40, 56):
			for y in range(FLOOR, FLOOR + 8):
				landed += int(sim.get_voxel(Vector3i(x, y, z)) == GameMaterials.STONE)
	assert_int(landed).is_greater(int(cl["count"]) * 2)   # it lies on the floor


func test_it_creaks_then_falls_unless_propped() -> void:
	var w := _world()
	var sim: SimWorld = w[0]
	var rp: RigidPieces = w[1]
	_room(sim, rp, 20, 76, 20, 76)
	var fake := FakeMain.new()
	fake.sim = sim
	var sup := MineSupport.new()
	sup.main = fake
	sup.rp = rp
	var r := _scan(rp, 20, 76, 20, 76)
	sup.take([r])
	assert_int(sup.warnings.size()).is_equal(1)
	var wait := MineSupport.warn_s(float(r["clusters"][0]["ratio"]))
	assert_float(wait).is_between(Settings.support_warn_min_s, Settings.support_warn_max_s)
	# just before the end of the warning it still hangs, creaking
	while sup._now < wait - 0.5:
		sup._now += 0.25
		for id in sup.warnings.keys():
			sup._warn_step(id, sup.warnings[id])
	assert_int(sup.warnings.size()).is_equal(1)
	assert_int(sup.stats_d["collapses"]).is_equal(0)
	# a pillar now: the next scan finds it held and the creaking stops
	sup.take([_scan(rp, 20, 76, 20, 76, PackedInt32Array(), sup.check_cells())])   # still failing: it creaks on
	assert_int(sup.warnings.size()).is_equal(1)
	sup.take([{"clusters": [], "checked": PackedInt32Array()}])   # a scan that did not reach it changes nothing
	assert_int(sup.warnings.size()).is_equal(1)
	sup.take([_scan(rp, 20, 76, 20, 76, _prop("stone_pillar", 48, 48), sup.check_cells())])
	assert_int(sup.warnings.size()).is_equal(0)
	assert_int(sup.stats_d["held"]).is_equal(1)
	# unpropped, the same roof falls when its time is up
	sup.take([r])
	sup._now += wait + 1.0
	for id in sup.warnings.keys():
		sup._warn_step(id, sup.warnings[id])
	assert_int(sup.stats_d["collapses"]).is_equal(1)
	assert_int(sup.warnings.size()).is_equal(0)
	sup.free()
	fake.free()
