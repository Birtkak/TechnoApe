extends GdUnitTestSuite

## Fire rules of the native kernel (bubble polish, ADR 0010), with explicit
## material values so they test the kernel, not the current data tuning:
## thin fuels spread (ignite chance = clamp(10 / fire_hp, 1/8, 1)), water
## stops fire, a wooden wall burns down, and fire never crosses stone.
## Pacing numbers live in tools/fire_bench.gd.

const HZ := 30


func _world(overrides: Dictionary = {}, seed_n := 11) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(4, 2, 4), 2, seed_n)
	var defs := GameMaterials.defs()
	for d in defs:
		if overrides.has(d["name"]):
			d.merge(overrides[d["name"]], true)
	w.set_materials(defs)
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 4, 64), GameMaterials.STONE)
	return w


func _run(w: SimWorld, seconds: float) -> void:
	for i in int(seconds * HZ):
		w.tick(1)


func test_grass_field_burns_across() -> void:
	var w := _world({"tall_grass": {"fire_hp": 40}})
	w.fill_box(Vector3i(0, 4, 0), Vector3i(64, 5, 64), GameMaterials.TALL_GRASS)
	w.ignite(Vector3i(32, 4, 32))
	_run(w, 40.0)
	assert_int(w.count_material(GameMaterials.TALL_GRASS)).is_less(64 * 64 / 50)   # > 98 % burnt


func test_short_lived_grass_dies_out() -> void:
	# documents why fire_hp matters: 10 ticks is too short to pass the fire on reliably
	var w := _world({"tall_grass": {"fire_hp": 10, "ignite_chance": 0.125}})
	w.fill_box(Vector3i(0, 4, 0), Vector3i(64, 5, 64), GameMaterials.TALL_GRASS)
	w.ignite(Vector3i(32, 4, 32))
	_run(w, 10.0)
	assert_int(w.count_material(GameMaterials.TALL_GRASS)).is_greater(64 * 64 - 50)


func test_plank_wall_burns_down_and_leaves_ash() -> void:
	var w := _world()
	w.fill_box(Vector3i(8, 4, 30), Vector3i(24, 12, 31), GameMaterials.PLANKS)
	w.ignite(Vector3i(8, 4, 30))
	_run(w, 60.0)
	assert_int(w.count_material(GameMaterials.PLANKS)).is_equal(0)
	assert_int(w.count_burning()).is_equal(0)


func test_water_slows_the_fire() -> void:
	# flames drift through air since 2026-10-27 and can hop a one-cell puddle: the water slows the fire, it no longer
	# always stops it, so one seed proves nothing either way; beams burnt past it on every seed would mean water stopped mattering
	var left := 0
	for s in range(1, 7):
		var w := _world({}, s)
		w.fill_box(Vector3i(8, 4, 30), Vector3i(40, 5, 31), GameMaterials.WOOD)
		w.fill_box(Vector3i(20, 4, 31), Vector3i(21, 5, 32), GameMaterials.WATER_STATIC)
		w.ignite(Vector3i(8, 4, 30))
		_run(w, 60.0)
		left += w.count_in_box(Vector3i(22, 4, 30), Vector3i(40, 5, 31), GameMaterials.WOOD)
	assert_int(left).is_greater(0)


func test_fire_does_not_cross_stone() -> void:
	var w := _world({"tall_grass": {"fire_hp": 40}})
	w.fill_box(Vector3i(0, 4, 0), Vector3i(64, 5, 64), GameMaterials.TALL_GRASS)
	w.fill_box(Vector3i(30, 4, 0), Vector3i(32, 12, 64), GameMaterials.STONE)   # 1 m stone wall, 4 m high
	w.ignite(Vector3i(10, 4, 32))
	_run(w, 40.0)
	assert_int(w.count_in_box(Vector3i(32, 4, 0), Vector3i(64, 5, 64), GameMaterials.TALL_GRASS)).is_equal(32 * 64)


func test_burning_flag_is_visible() -> void:
	var w := _world()
	w.fill_box(Vector3i(8, 4, 30), Vector3i(10, 5, 31), GameMaterials.WOOD)
	w.ignite(Vector3i(8, 4, 30))
	assert_int(w.get_flags(Vector3i(8, 4, 30)) & 1).is_equal(1)
	assert_int(w.get_flags(Vector3i(9, 4, 30)) & 1).is_equal(0)


## ADR 0144: flowing lava lights what it touches and does not put it out; the sim counts burning and boiling cells for the sounds.
func test_lava_lights_wood_and_keeps_it_burning() -> void:
	var w := _world()
	w.fill_box(Vector3i(20, 4, 20), Vector3i(24, 12, 21), GameMaterials.WOOD)
	w.fill_box(Vector3i(20, 4, 21), Vector3i(24, 8, 22), GameMaterials.LAVA)
	_run(w, 5.0)
	assert_int(w.count_burning()).is_greater(0)
	assert_int(w.get_falls(4)[0]).is_greater(0)


func test_still_lava_lights_nothing() -> void:
	var w := _world()
	w.fill_box(Vector3i(20, 4, 20), Vector3i(24, 8, 21), GameMaterials.WOOD)
	w.fill_box(Vector3i(20, 4, 21), Vector3i(24, 8, 22), GameMaterials.LAVA_STATIC)
	_run(w, 5.0)
	assert_int(w.count_burning()).is_equal(0)


func test_lava_on_water_hisses() -> void:
	var w := _world()
	w.fill_box(Vector3i(20, 4, 20), Vector3i(30, 8, 30), GameMaterials.WATER)
	var seen := 0
	w.fill_box(Vector3i(24, 8, 24), Vector3i(26, 10, 26), GameMaterials.LAVA)
	for i in 90:
		w.tick(1)
		seen += w.get_falls(5)[0]
	assert_int(seen).is_greater(0)
