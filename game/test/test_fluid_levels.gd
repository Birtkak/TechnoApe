extends GdUnitTestSuite

## Liquids settle into thin layers (SimWorld fullness) and the terrain draws
## each cell at its level as a Minecraft-style fluid (godot_voxel
## VoxelBlockyFluid), the surface sloping between neighbours.


func _fluid_materials() -> Dictionary:
	var out := {}
	for d in GameMaterials.defs():
		if d.has("fluid_slot") and not d.get("liquid_static", false):
			out[int(d["fluid_slot"])] = StandardMaterial3D.new()
	return out


func test_library_has_a_model_per_level() -> void:
	var lib := GameMaterials.build_library(StandardMaterial3D.new(), StandardMaterial3D.new(), _fluid_materials())
	var slots := _fluid_materials().size()
	assert_int(lib.models.size()).is_equal(GameMaterials.FLUID_LEVEL_BASE + slots * 8)
	var water: VoxelBlockyModel = lib.models[GameMaterials.WATER]
	assert_bool(water is VoxelBlockyModelFluid).is_true()
	assert_int((water as VoxelBlockyModelFluid).level).is_equal(8)
	var static_water: VoxelBlockyModelFluid = lib.models[GameMaterials.WATER_STATIC]
	assert_object(static_water.fluid).is_same((water as VoxelBlockyModelFluid).fluid)   # lakes connect to flowing water
	var half: VoxelBlockyModelFluid = lib.models[GameMaterials.FLUID_LEVEL_BASE + 3]
	assert_int(half.level).is_equal(4)
	assert_bool(lib.models[GameMaterials.STONE] is VoxelBlockyModelCube).is_true()
	# without fluid materials (tools, tests) liquids stay cubes
	var plain := GameMaterials.build_library(StandardMaterial3D.new())
	assert_int(plain.models.size()).is_equal(GameMaterials.defs().size() + 1)


func test_voxel_values_map_back_to_materials() -> void:
	var sludge_slot := int(GameMaterials.defs()[GameMaterials.TOXIC_SLUDGE - 1]["fluid_slot"])
	var v := GameMaterials.FLUID_LEVEL_BASE + sludge_slot * 8 + 2
	assert_int(GameMaterials.material_of_voxel(v)).is_equal(GameMaterials.TOXIC_SLUDGE)
	assert_int(GameMaterials.eighths_of_voxel(v)).is_equal(3)
	assert_int(GameMaterials.material_of_voxel(GameMaterials.WATER)).is_equal(GameMaterials.WATER)
	assert_int(GameMaterials.eighths_of_voxel(GameMaterials.STONE)).is_equal(8)


func _sim() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(4, 2, 4), 2, 9)
	w.set_materials(_no_films(GameMaterials.defs()))   # these count exact mass; films drying up (ADR 0069 B3) loses it on purpose
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	return w


func test_a_block_of_water_spreads_into_a_thin_layer() -> void:
	var w := _sim()
	w.fill_box(Vector3i(28, 1, 28), Vector3i(32, 5, 32), GameMaterials.WATER)   # 4 x 4 x 4 cells
	for i in 400:
		w.tick(1)
	assert_int(w.count_in_box(Vector3i(0, 2, 0), Vector3i(64, 32, 64), GameMaterials.WATER)).override_failure_message("water still stacked").is_equal(0)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(64, 2, 64), GameMaterials.WATER)).is_greater(64)   # wider than it was
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(64 * 8)                                  # nothing lost


func test_levels_survive_the_trip_through_the_terrain() -> void:
	var w := _sim()
	w.fill_box(Vector3i(28, 1, 28), Vector3i(32, 5, 32), GameMaterials.WATER)
	for i in 400:
		w.tick(1)
	var partial := 0
	var copy := _sim()
	for cz in 4:
		for cx in 4:
			var c := Vector3i(cx, 0, cz)
			var bytes: PackedByteArray = w.get_chunk_bytes(c, 2)
			for i in range(0, bytes.size(), 2):
				if bytes.decode_u16(i) >= GameMaterials.FLUID_LEVEL_BASE:
					partial += 1
			copy.set_chunk_bytes(c, bytes, 2)
	assert_int(partial).override_failure_message("no partly full cells reached the terrain").is_greater(0)
	assert_int(copy.count_liquid_eighths(GameMaterials.WATER)).is_equal(w.count_liquid_eighths(GameMaterials.WATER))


static func _no_films(defs: Array) -> Array:
	var out := []
	for d in defs:
		var c: Dictionary = d.duplicate()
		c.erase("film_lifetime_ticks")
		out.append(c)
	return out
