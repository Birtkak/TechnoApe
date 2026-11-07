extends GdUnitTestSuite

## Fuse cord and flung debris (ADR 0150): the cord burns end to end at Settings.fuse_speed_mps in the real
## kernel, lights nothing beside it, and lights the TNT it reaches; blast debris lands as loose voxels.

const StubMain := preload("res://test/tnt_stub_main.gd")
const HZ := 30
const LINES := [8, 20, 32, 44]   # z of four cords side by side: the mean of four random burns
const X0 := 2
const LEN := 56                  # voxels (7 m)


## A stub whose world below y = 0 is stone (debris lands on it).
class FloorStub:
	extends "res://test/tnt_stub_main.gd"
	func get_world_voxel(p: Vector3i) -> int:
		return GameMaterials.STONE if p.y < 0 else int(voxels.get(p, 0))


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(4, 2, 4), 2, 7)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 4, 64), GameMaterials.STONE)
	return w


func test_ids_data_and_a_cold_fire() -> void:
	assert_int(GameMaterials.FUSE_CORD).is_equal(137)
	assert_int(GameMaterials.FUSE_SPARK).is_equal(138)
	assert_int(GameMaterials.id_of("fuse_cord")).is_equal(137)
	assert_int(GameMaterials.id_of("fuse_spark")).is_equal(138)
	var defs := GameMaterials.defs()
	var cord: Dictionary = defs[GameMaterials.FUSE_CORD - 1]
	# its fire is colder than anything else catches: the ground beside it never burns
	for i in defs.size():
		var d: Dictionary = defs[i]
		if d.get("burnable", false) and i + 1 != GameMaterials.FUSE_CORD:
			assert_int(int(d.get("autoignition_temperature", 100))).override_failure_message(d["name"]).is_greater(int(cord["temperature_of_fire"]))
	assert_int(int(defs[GameMaterials.FUSE_SPARK - 1]["temperature_of_fire"])).is_less(int(cord["autoignition_temperature"]))
	var items := ItemRegistry.load_default()
	assert_bool(items.has("fuse_cord")).is_true()
	assert_bool(RecipeRegistry.load_default(items).recipes.has("fuse_cord")).is_true()


## Four 7 m cords between rows of tall grass, lit at x = X0: each burns to its end at the set speed (the
## mean of the four within 20 %), the grass stays, and the cord is ash.
func test_cord_burns_end_to_end_at_the_set_speed() -> void:
	var w := _world()
	for z in LINES:
		w.fill_box(Vector3i(X0, 4, z), Vector3i(X0 + LEN, 5, z + 1), GameMaterials.FUSE_CORD)
		w.fill_box(Vector3i(X0, 4, z - 1), Vector3i(X0 + LEN, 5, z), GameMaterials.TALL_GRASS)
		w.fill_box(Vector3i(X0, 4, z + 1), Vector3i(X0 + LEN, 5, z + 2), GameMaterials.TALL_GRASS)
		w.ignite(Vector3i(X0, 4, z))
	var grass := w.count_material(GameMaterials.TALL_GRASS)
	var reached := {}   # z -> tick its last cell caught
	var last := X0 + LEN - 1
	for t in int(3.0 * LEN * Settings.VOXEL_M / Settings.fuse_speed_mps * HZ):
		w.tick(1)
		for z in LINES:
			if not reached.has(z) and (w.get_flags(Vector3i(last, 4, z)) & 1 or w.get_voxel(Vector3i(last, 4, z)) != GameMaterials.FUSE_CORD):
				reached[z] = t + 1
		if reached.size() == LINES.size():
			break
	assert_int(reached.size()).is_equal(LINES.size())   # every cord burnt to its end
	var mean := 0.0
	for z in reached:
		mean += reached[z] / float(HZ) / LINES.size()
	var expected := (LEN - 1) * Settings.VOXEL_M / Settings.fuse_speed_mps
	print("fuse_cord: %.2f m burnt in %.2f s (mean of %d), expected %.2f s" % [(LEN - 1) * Settings.VOXEL_M, mean, LINES.size(), expected])
	assert_float(mean).override_failure_message("mean %.2f s, expected %.2f s" % [mean, expected]).is_between(expected * 0.95, expected * 1.05)
	for i in 4 * HZ:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.FUSE_CORD)).is_equal(0)
	assert_int(w.count_material(GameMaterials.TALL_GRASS)).is_equal(grass)   # nothing beside it caught


## A cord laid against a charge's floor edge (not mid-face) lights it when the burn arrives, not before.
func test_cord_lights_the_tnt_it_reaches() -> void:
	var w := _world()
	var stub = auto_free(StubMain.new())
	stub.bubble.sim = w
	add_child(stub)
	var t: Tnt = auto_free(Tnt.new())
	stub.add_child(t)
	t.setup(stub)
	var a := Vector3i(48, 4, 28)
	for y in 4:
		for z in 4:
			for x in 4:
				stub.voxels[a + Vector3i(x, y, z)] = GameMaterials.TNT
	t.register_cells([a])
	var z0 := a.z + 3   # the face's corner column
	w.fill_box(Vector3i(X0, 4, z0), Vector3i(a.x, 5, z0 + 1), GameMaterials.FUSE_CORD)
	w.ignite(Vector3i(X0, 4, z0))
	var lit_at := -1.0
	for i in 40 * HZ:
		w.tick(1)
		t._fire_check(1.0 / HZ)
		if t.stats["fire_lit"] > 0:
			lit_at = (i + 1) / float(HZ)
			break
	var expected := (a.x - X0) * Settings.VOXEL_M / Settings.fuse_speed_mps
	print("fuse_cord: tnt lit at %.2f s, expected ~%.2f s" % [lit_at, expected])
	assert_float(lit_at).override_failure_message("lit at %.2f s, expected ~%.2f s" % [lit_at, expected]).is_between(expected * 0.9, expected * 1.1)
	assert_int(t.primed.size()).is_equal(1)


## Debris: capped per blast, never more voxels than the blast removed, lands as loose ground (rock as gravel).
func test_debris_is_capped_and_lands_as_loose_voxels() -> void:
	var stub = auto_free(FloorStub.new())
	add_child(stub)
	var d: BlastDebris = auto_free(BlastDebris.new())
	stub.add_child(d)
	var box := BoxMesh.new()
	box.size = Vector3.ONE * Settings.BLOCK
	d.setup(stub, StandardMaterial3D.new(), box)
	assert_int(d.throw(Vector3(0, 8, 0), 16.0, {"stone": 1000.0, "tnt": 50.0}, 100000)).is_equal(Settings.tnt_debris_max)
	assert_int(d.throw(Vector3(0, 8, 0), 16.0, {"stone": 1000.0}, BlastDebris.REMOVED_PER_CHUNK * 3)).is_equal(3)   # a small blast: few
	assert_int(d.throw(Vector3(0, 8, 0), 16.0, {"tnt": 1000.0}, 100000)).is_equal(0)   # never TNT
	for i in 600:
		d._process(1.0 / 60.0)
		if d.live_count() == 0:
			break
	assert_int(d.live_count()).is_equal(0)
	assert_int(d.stats["landed"]).is_equal(Settings.tnt_debris_max + 3)
	assert_int(d.stats["voxels"]).is_between(1, d.stats["landed"] * BlastDebris.VOXELS)
	var kinds := {}
	for p in stub.voxels:
		kinds[stub.voxels[p]] = true
	assert_array(kinds.keys()).is_equal([GameMaterials.GRAVEL])
	assert_int(BlastDebris.loose_of(GameMaterials.GRASS)).is_equal(GameMaterials.DIRT)
	assert_int(BlastDebris.loose_of(GameMaterials.WOOD)).is_equal(GameMaterials.WOOD)


## Up a staircase (each step one up and one over: the cells touch only by edges) and through a pool of water:
## the cord still burns to its end, at about the set speed.
func test_cord_climbs_steps_and_burns_under_water() -> void:
	var w := _world()
	var cells: Array[Vector3i] = []
	for i in 24:   # 24 steps up along x at z = 10
		cells.append(Vector3i(X0 + i, 4 + i, 10))
	for i in 40:   # 40 cells along x at z = 40, under water from x = 12 to 32
		cells.append(Vector3i(X0 + i, 4, 40))
	for c in cells:
		w.set_voxel(c, GameMaterials.FUSE_CORD)
	w.fill_box(Vector3i(12, 5, 39), Vector3i(32, 8, 42), GameMaterials.WATER)
	w.fill_box(Vector3i(12, 4, 39), Vector3i(32, 5, 40), GameMaterials.WATER)
	w.fill_box(Vector3i(12, 4, 41), Vector3i(32, 5, 42), GameMaterials.WATER)
	w.ignite(cells[0])
	w.ignite(cells[24])
	var ends := [cells[23], cells[63]]
	var t := 0
	while t < 30 * HZ and ends.any(func(e: Vector3i) -> bool: return w.get_voxel(e) == GameMaterials.FUSE_CORD and not w.get_flags(e) & 1):
		w.tick(1)
		t += 1
	print("fuse_cord: steps and water burnt through in %.2f s" % (t / float(HZ)))
	assert_int(t).is_less(30 * HZ)
	var expected := 39 * Settings.VOXEL_M / Settings.fuse_speed_mps   # the longer, 40-cell cord
	assert_float(t / float(HZ)).is_between(expected * 0.9, expected * 1.1)


class FakePlayer:
	extends Node3D
	signal died
	var game_mode := GameMode.SURVIVAL
	var knocked := Vector3.ZERO
	func knock(v: Vector3) -> void:
		knocked += v


class FakeInv:
	extends RefCounted
	var taken := 0
	func damage(n: int, _cause: String) -> void:
		taken += n
	func is_dead() -> bool:
		return false


## A flung chunk that flies into the player hurts and shoves them, then drops there.
func test_debris_hurts_the_player_it_hits() -> void:
	var stub = auto_free(FloorStub.new())
	add_child(stub)
	var pl: FakePlayer = auto_free(FakePlayer.new())
	stub.player = pl
	stub.player_inv = FakeInv.new()
	pl.position = Vector3(Settings.m2v(3.0), 0, 0)
	var d: BlastDebris = auto_free(BlastDebris.new())
	stub.add_child(d)
	var box := BoxMesh.new()
	box.size = Vector3.ONE * Settings.BLOCK
	d.setup(stub, StandardMaterial3D.new(), box)
	d.throw(Vector3(0, 6, 0), 0.0, {"dirt": 100.0}, BlastDebris.REMOVED_PER_CHUNK)
	d._live[0]["v"] = Vector3(Settings.m2v(10.0), 0, 0)   # straight at the player's middle
	for i in 60:
		d._process(1.0 / 60.0)
	assert_int(stub.player_inv.taken).is_equal(Settings.tnt_debris_damage)
	assert_float(pl.knocked.x).is_greater(0.0)
	assert_int(d.stats["hits"]).is_equal(1)
