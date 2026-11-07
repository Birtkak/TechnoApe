extends GdUnitTestSuite

## Underground hazards (ADR 0148): firedamp is an explosive gas the sim flashes through and tallies, the
## safety lamp finds it by sampling, spring stone pours water, the vent fan pushes gas, and the data behind
## them (items, recipes, sprites, the coal vein's pocket gas) is there.

const N := 2   # chunks per side


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	return w


## A sealed stone room [1, S-1) with its top half firedamp.
func _gas_room(w: SimWorld) -> int:
	var s := N * 16
	var n := 0
	for z in s:
		for y in s:
			for x in s:
				var p := Vector3i(x, y, z)
				var inside := x > 0 and y > 0 and z > 0 and x < s - 1 and y < s - 1 and z < s - 1
				if not inside:
					w.set_voxel(p, GameMaterials.STONE)
				elif y >= s / 2:
					w.set_voxel(p, GameMaterials.FIREDAMP)
					n += 1
	return n


func test_firedamp_is_the_explosive_gas_at_133() -> void:
	assert_int(GameMaterials.FIREDAMP).is_equal(133)
	assert_int(GameMaterials.id_of("firedamp")).is_equal(133)
	var d: Dictionary = GameMaterials.defs()[GameMaterials.FIREDAMP - 1]
	assert_str(String(d["cell_type"])).is_equal("gas")
	assert_bool(d.get("burnable", false)).is_true()
	assert_bool(d.get("explosive", false)).is_true()
	assert_int(int(d.get("lifetime", 1))).is_equal(0)   # never fades: a pocket waits for the miner
	var s: Dictionary = GameMaterials.defs()[GameMaterials.SPRING_STONE - 1]
	assert_int(int(s.get("emits", 0))).is_equal(GameMaterials.WATER)


func test_sealed_firedamp_keeps_and_a_fire_cell_flashes_it() -> void:
	var w := _world()
	var n := _gas_room(w)
	for i in 20:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.FIREDAMP)).is_equal(n)   # sealed: nothing burns, nothing fades
	assert_int(w.get_falls(Firedamp.TALLY_GAS_BURN)[0]).is_equal(0)
	var s := N * 16
	w.set_voxel(Vector3i(s / 2, s / 2 + 1, s / 2), GameMaterials.FIRE)
	var flash_max := 0
	var at := Vector3.ZERO
	for i in 30:
		w.tick(1)
		var f: PackedInt32Array = w.get_falls(Firedamp.TALLY_GAS_BURN)
		if f[0] > flash_max:
			flash_max = f[0]
			at = Vector3(f[1], f[2], f[3])
	assert_int(flash_max).is_greater_equal(Settings.firedamp_blast_cells)   # the flash is big enough for a blast ...
	assert_float(at.y).is_greater_equal(float(s / 2) - 1.0)                 # ... centred in the gas
	assert_int(w.count_material(GameMaterials.FIREDAMP)).is_less(n)        # and the gas is going
	assert_float(Firedamp.blast_share(Settings.firedamp_full_cells)).is_equal_approx(1.0, 0.001)
	assert_float(Firedamp.blast_share(1)).is_equal_approx(0.25, 0.001)


func test_the_safety_lamp_samples_the_gas_round_the_eye() -> void:
	var w := _world()
	_gas_room(w)
	var mask := PackedByteArray()
	mask.resize(256)
	mask[GameMaterials.FIREDAMP] = 1
	var s := N * 16
	var lo := w.sample_cells(Vector3i(s / 2, 3, s / 2), 2, 64, 1, mask).size() / 7     # the floor: clean air
	var hi := w.sample_cells(Vector3i(s / 2, s - 4, s / 2), 2, 64, 1, mask).size() / 7   # the roof: gas
	assert_int(lo).is_equal(0)
	assert_int(hi).is_greater(8)
	var items := ItemRegistry.load_default()
	var l := TorchLights.light_of(items, "safety_lamp")
	assert_bool(l.get("safe_light", false)).is_true()   # a light Firedamp never turns into a fire cell
	assert_bool(TorchLights.light_of(items, "torch").get("safe_light", false)).is_false()


func test_spring_stone_pours_water_into_air() -> void:
	var w := _world()
	var s := N * 16
	for z in s:
		for x in s:
			w.set_voxel(Vector3i(x, 0, z), GameMaterials.STONE)
	w.set_voxel(Vector3i(s / 2, 2, s / 2), GameMaterials.SPRING_STONE)
	for i in 40:
		w.tick(1)
	var water := w.count_material(GameMaterials.WATER)
	assert_int(water).is_greater(0)
	assert_int(w.count_material(GameMaterials.SPRING_STONE)).is_equal(1)
	for i in 40:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.WATER)).is_greater(water)   # steady, not one burst


func test_blow_pushes_gas_along_a_duct() -> void:
	var w := _world()
	var s := N * 16
	for y in range(4, 8):
		for z in range(4, 8):
			w.set_voxel(Vector3i(4, y, z), GameMaterials.FIREDAMP)
	var moved := w.blow(Vector3i(4, 4, 4), Vector3i(s - 1, 8, 8), Vector3i(1, 0, 0))
	assert_int(moved).is_equal(16)
	assert_int(w.count_in_box(Vector3i(4, 4, 4), Vector3i(5, 8, 8), GameMaterials.FIREDAMP)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(5, 4, 4), Vector3i(6, 8, 8), GameMaterials.FIREDAMP)).is_equal(16)
	assert_int(w.blow(Vector3i(4, 4, 4), Vector3i(s - 1, 8, 8), Vector3i(1, 1, 0))).is_equal(0)   # not an axis: nothing


func test_vent_fan_duct_lies_in_front_of_its_facing() -> void:
	var items := ItemRegistry.load_default()
	var f := VentFanEntity.new(items, Vector3i(100, 50, 100))
	var b := Settings.BLOCK
	var box: Array = f.blow_box()
	assert_bool(box[0] == Vector3i(100, 50, 100 + b)).is_true()
	assert_bool(box[1] == Vector3i(100 + b, 50 + b, 100 + b + int(Settings.m2v(Settings.vent_fan_reach_m)))).is_true()
	assert_bool(box[2] == Vector3i(0, 0, 1)).is_true()
	f.facing = 3
	box = f.blow_box()
	assert_bool(box[0] == Vector3i(100 - int(Settings.m2v(Settings.vent_fan_reach_m)), 50, 100)).is_true()
	assert_bool(box[1] == Vector3i(100, 50 + b, 100 + b)).is_true()
	assert_bool(f.needs_sim()).is_false()   # still: no pin
	var store := BlockEntities.new(items, RecipeRegistry.load_default(items))
	var e = store.create(Vector3i.ZERO, "vent_fan")
	assert_bool(e is VentFanEntity).is_true()
	assert_bool(e.view_parts().has("solid")).is_true()


func test_data_items_recipes_sprites_and_the_coal_pocket() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	for id in ["safety_lamp", "vent_fan"]:
		assert_bool(items.has(id)).is_true()
		assert_bool(recipes.recipes.has(id)).is_true()
	var sprites: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/item_sprites.json"))
	assert_bool(sprites["sprites"].has("safety_lamp")).is_true()
	assert_bool(sprites["sprites"].has("vent_fan")).is_true()
	var veins := OreVeinRegistry.load_default()
	assert_array(veins.errors).is_empty()
	var coal: Dictionary = veins.veins[veins.by_name["coal"]]
	assert_str(String(coal["pocket_gas"])).is_equal("firedamp")
	var text := OreVeinCodegen.generate(veins)
	assert_bool(text.contains("POCKET_GAS[COUNT] = {133,")).is_true()
	assert_bool(PlaytestCommands.SCENARIOS.has("firedamp")).is_true()
	for line in PlaytestCommands.lines("firedamp", Vector3i(0, 100, 0)):
		assert_bool(String(line).contains("{")).is_false()   # every spot filled in
