extends GdUnitTestSuite

## ADR 0119 A: column flow. Moving liquid levels as columns by head, whole eighths, deterministic neighbour order:
## a pit fills flat and the sim goes quiet, a U-tube levels through its bottom, mass is exact, lava stays slow.

const N := 3


func _world(column := true) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(_no_films(GameMaterials.defs()))
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(column)
	w.set_cascade(true)
	w.set_gravity(0.17)
	return w


## exact mass: films drying up (ADR 0069 B3) loses mass on purpose
static func _no_films(defs: Array) -> Array:
	var out := []
	for d in defs:
		var c: Dictionary = d.duplicate()
		c.erase("film_lifetime_ticks")
		out.append(c)
	return out


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


## Highest and lowest surface (in eighths) over a floor box.
func _surface_range(w: SimWorld, x0: int, x1: int, z0: int, z1: int, y0: int, y1: int, mat: int) -> Vector2i:
	var lo := 1 << 30
	var hi := -1
	for h in surface_heights(w, x0, x1, z0, z1, y0, y1, mat).values():
		lo = mini(lo, h)
		hi = maxi(hi, h)
	return Vector2i(lo, hi)


func _quiet_after(w: SimWorld, max_ticks: int) -> int:
	var still := 0
	for t in max_ticks:
		var s: Dictionary = w.tick(1)
		if int(s.get("moved_cells_per_tick", 1)) < 8:   # the odd film drying is not motion
			still += 1
			if still >= 10:
				return t - 9
		else:
			still = 0
	return -1


func test_a_block_of_water_in_a_basin_ends_flat_and_quiet() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 8, 8), Vector3i(40, 20, 40), GameMaterials.STONE)
	w.fill_box(Vector3i(9, 8, 9), Vector3i(39, 20, 39), 0)   # a 30 x 30 basin, 12 deep
	w.fill_box(Vector3i(20, 8, 20), Vector3i(28, 16, 28), GameMaterials.WATER)   # an 8 x 8 x 8 block in its middle
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	var quiet := _quiet_after(w, 240)
	assert_int(quiet).override_failure_message("still moving after 240 ticks").is_between(1, 120)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).override_failure_message("mass changed").is_equal(mass)
	var r := _surface_range(w, 9, 39, 9, 39, 8, 20, GameMaterials.WATER)
	# 512 cells over 900 columns: 4 eighths and a half each
	assert_int(r.y - r.x).override_failure_message("surface from %d to %d eighths" % [r.x, r.y]).is_less_equal(4)   # half a cell: neighbours rest at a two-eighth step
	assert_int(w.count_in_box(Vector3i(0, 16, 0), Vector3i(48, 48, 48), GameMaterials.WATER)).is_equal(0)


func test_the_same_spill_lands_the_same_twice() -> void:
	var a := _world()
	var b := _world()
	for w in [a, b]:
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(12, 8, 12), Vector3i(20, 16, 20), GameMaterials.STONE)   # a block to run off
		w.fill_box(Vector3i(13, 16, 13), Vector3i(19, 22, 19), GameMaterials.WATER)
		_run(w, 90)
	for z in range(0, 48, 3):
		for x in range(0, 48, 3):
			for y in range(8, 24, 2):
				var p := Vector3i(x, y, z)
				assert_int(a.get_voxel(p)).override_failure_message("differs at %s" % p).is_equal(b.get_voxel(p))


func test_lava_levels_slower_than_water() -> void:
	var spread := {}
	for mat in [GameMaterials.WATER, GameMaterials.LAVA]:
		var w := _world()
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(20, 8, 20), Vector3i(24, 12, 24), mat)
		_run(w, 10)
		var n := 0
		for z in 48:
			for x in 48:
				var v := w.get_voxel(Vector3i(x, 8, z))
				if v == mat or (v >= GameMaterials.FLUID_LEVEL_BASE and GameMaterials.material_of_voxel(v) == mat):
					n += 1
		spread[mat] = n
	assert_int(spread[GameMaterials.LAVA]).override_failure_message("lava %d vs water %d columns" % [spread[GameMaterials.LAVA], spread[GameMaterials.WATER]]) \
		.is_less(spread[GameMaterials.WATER])


## The surface height in eighths per column over a box (0 where the column holds none): the top cell's eighths,
## read from the chunk bytes (get_voxel reports the material alone).
static func surface_heights(w: SimWorld, x0: int, x1: int, z0: int, z1: int, y0: int, y1: int, mat: int) -> Dictionary:
	var out := {}
	var chunks := {}
	for z in range(z0, z1):
		for x in range(x0, x1):
			var h := 0
			for y in range(y0, y1):
				var ck := Vector3i(x / 16, y / 16, z / 16)
				if not chunks.has(ck):
					chunks[ck] = w.get_chunk_bytes(ck, 2)
				var b: PackedByteArray = chunks[ck]
				var o := ((y % 16) + 16 * ((x % 16) + 16 * (z % 16))) * 2
				var v := b[o] + (b[o + 1] << 8)
				if v == mat:
					h = 8 * (y + 1)
				elif v >= GameMaterials.FLUID_LEVEL_BASE and GameMaterials.material_of_voxel(v) == mat:
					h = 8 * y + (v - GameMaterials.FLUID_LEVEL_BASE) % 8 + 1
			out[Vector2i(x, z)] = h
	return out


# ---------------- ADR 0119 C: heat, pressure and state ----------------

func _box_world(ambient: float) -> SimWorld:
	var w := _world()
	w.set_ambient_c(ambient, true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), GameMaterials.STONE)
	return w


func test_water_over_lava_boils_to_steam() -> void:
	var w := _box_world(18.0)
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 24, 32), 0)   # a 16^3 room
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 10, 32), GameMaterials.LAVA_STATIC)   # a lava floor
	w.fill_box(Vector3i(20, 12, 20), Vector3i(28, 14, 28), GameMaterials.WATER)   # a slab of water over it, not touching
	w.fill_box(Vector3i(20, 11, 20), Vector3i(28, 12, 28), GameMaterials.STONE)   # on a stone shelf
	_run(w, 90)
	var steam := w.count_material(GameMaterials.STEAM)
	assert_int(steam).override_failure_message("no steam after 3 s over lava (%d)" % steam).is_greater(0)
	assert_int(int(w.cell_info(Vector3i(24, 13, 24)).get("temp_c", 0))).override_failure_message("the room stayed cold").is_greater(100)


func test_steam_condenses_where_it_is_cold() -> void:
	var w := _box_world(10.0)
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 24, 32), 0)
	w.fill_box(Vector3i(20, 10, 20), Vector3i(28, 12, 28), GameMaterials.STEAM)
	_run(w, 30)
	assert_int(w.count_material(GameMaterials.STEAM)).override_failure_message("steam survives at 10 C").is_equal(0)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).override_failure_message("no drops condensed").is_greater(0)


func test_a_sealed_hot_box_stops_boiling_as_its_pressure_rises_and_venting_resumes_it() -> void:
	var w := _box_world(110.0)   # over water's boiling point at no pressure; a sixth of pressure lifts it past 110
	w.fill_box(Vector3i(16, 16, 16), Vector3i(32, 32, 32), 0)   # a sealed 16^3 chunk-aligned room
	w.fill_box(Vector3i(16, 16, 16), Vector3i(32, 20, 32), GameMaterials.WATER)   # a quarter full of water
	var peak := 0.0
	var left_at_peak := 0
	for k in 40:   # steam has a lifetime, so the pressure peaks and falls: read it as it goes
		_run(w, 10)
		var p := w.chunk_pressure(Vector3i(24, 24, 24))
		if p > peak:
			peak = p
			left_at_peak = w.count_liquid_eighths(GameMaterials.WATER)
	assert_float(peak).override_failure_message("no pressure built (%.2f)" % peak).is_greater(0.12)
	assert_int(left_at_peak).override_failure_message("every cell boiled although the pressure rose").is_greater(0)
	w.fill_box(Vector3i(32, 16, 20), Vector3i(48, 32, 28), 0)   # vent: the east wall opens into a corridor (as hot: it is one field)
	_run(w, 120)
	var escaped := w.count_in_box(Vector3i(32, 16, 20), Vector3i(48, 32, 28), GameMaterials.STEAM)
	assert_int(escaped).override_failure_message("no steam left through the vent").is_greater(0)
	assert_float(w.chunk_pressure(Vector3i(24, 24, 24))).override_failure_message("the vented room's pressure rose past its sealed peak").is_less_equal(peak + 0.02)


func test_ice_melts_warm_and_water_freezes_cold() -> void:
	var w := _box_world(12.0)
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 24, 32), 0)
	w.fill_box(Vector3i(20, 8, 20), Vector3i(28, 10, 28), GameMaterials.ICE)
	_run(w, 60)
	assert_int(w.count_material(GameMaterials.ICE)).override_failure_message("ice at 12 C").is_equal(0)
	var cold := _box_world(-15.0)
	cold.fill_box(Vector3i(16, 8, 16), Vector3i(32, 24, 32), 0)
	cold.fill_box(Vector3i(20, 8, 20), Vector3i(28, 10, 28), GameMaterials.WATER)
	_run(cold, 60)
	assert_int(cold.count_liquid_eighths(GameMaterials.WATER)).override_failure_message("water at -15 C").is_equal(0)
	assert_int(cold.count_material(GameMaterials.ICE)).is_greater(0)


func test_sludge_over_lava_boils_to_toxic_gas_that_sinks() -> void:
	var w := _box_world(18.0)
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 30, 32), 0)   # a tall room
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 10, 32), GameMaterials.LAVA_STATIC)
	w.fill_box(Vector3i(20, 12, 20), Vector3i(28, 13, 28), GameMaterials.STONE)   # a shelf
	w.fill_box(Vector3i(20, 13, 20), Vector3i(28, 15, 28), GameMaterials.TOXIC_SLUDGE)
	_run(w, 120)
	var gas := w.count_material(GameMaterials.TOXIC_GAS)
	assert_int(gas).override_failure_message("no toxic gas after 4 s over lava").is_greater(0)
	# heavy: more of it in the room's lower half than its upper half
	var low := w.count_in_box(Vector3i(16, 8, 16), Vector3i(32, 19, 32), GameMaterials.TOXIC_GAS)
	assert_int(low * 2).override_failure_message("toxic gas rose: %d low of %d" % [low, gas]).is_greater_equal(gas)
