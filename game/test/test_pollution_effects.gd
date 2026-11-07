extends GdUnitTestSuite

## ADR 0155: pollution pushes back. Crops beside sludge or waste ground or under heavy smoke turn sickly (the look line
## says why), then wilt to dead bushes; clean ground brings them back. Masks filter smoke. (Fish and the cough run in
## the game: `/pollute` and the evidence sheet.)

const SOIL := Vector3i(0, 0, 0)
const CROP := Vector3i(0, 1, 0)

var vox := {}


func _farm() -> Farming:
	var farm := Farming.load_default()
	vox = {}
	farm.get_voxel = func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR))
	farm.set_voxel = func(p: Vector3i, m: int) -> void:
		if m == GameMaterials.AIR:
			vox.erase(p)
		else:
			vox[p] = m
	farm.rng.seed = 4321
	farm.block = 1
	for x in range(-8, 9):
		for z in range(-8, 9):
			vox[Vector3i(x, 0, z)] = GameMaterials.GRASS
	vox[Vector3i(-3, 0, 0)] = GameMaterials.WATER
	farm.till(SOIL)
	assert_bool(farm.plant(SOIL, "wheat")).is_true()
	return farm


## Ticks until `done` holds (or a cap): random ticks reach a crop about once per 70 calls.
func _tick_until(farm: Farming, done: Callable) -> void:
	for i in 4000:
		farm.tick(20)
		if done.call():
			return


func test_waste_ground_beside_a_crop_makes_it_sickly_then_wilts_it() -> void:
	var farm := _farm()
	vox[Vector3i(1, 0, 0)] = GameMaterials.WASTE_GROUND
	farm.grow(CROP, 7)   # ripe
	var ripe := int(farm.crops[CROP]["age"])
	assert_str(farm.pollution(CROP)).is_equal("waste ground")
	_tick_until(farm, func() -> bool: return int(farm.crops.get(CROP, {}).get("sick", 0)) > 0)
	assert_str(farm.sick_line(CROP)).is_equal("Sickly: waste ground")
	assert_int(int(farm.crops[CROP]["age"])).override_failure_message("the sign: it dies back an age").is_equal(ripe - 1)
	assert_int(int(vox[CROP])).is_equal(farm.look_of("wheat", ripe - 1))
	_tick_until(farm, func() -> bool: return farm.crops.is_empty())
	assert_bool(farm.crops.is_empty()).is_true()
	assert_int(int(farm.stats["wilted"])).is_equal(1)
	assert_int(int(vox.get(CROP, 0))).is_equal(GameMaterials.id_of("dead_bush"))


func test_a_sickly_crop_recovers_once_the_ground_is_clean() -> void:
	var farm := _farm()
	vox[Vector3i(0, 1, 1)] = GameMaterials.TOXIC_SLUDGE   # a puddle beside the crop
	_tick_until(farm, func() -> bool: return int(farm.crops.get(CROP, {}).get("sick", 0)) > 0)
	assert_bool(farm.crops.has(CROP)).is_true()
	vox.erase(Vector3i(0, 1, 1))   # cleared up
	_tick_until(farm, func() -> bool: return int(farm.crops.get(CROP, {}).get("sick", 0)) == 0)
	assert_bool(farm.crops.has(CROP)).is_true()
	assert_str(farm.sick_line(CROP)).is_equal("")
	var saved := farm.to_dict()
	var back := Farming.load_default()
	assert_bool(back.load_dict(saved)).is_true()
	assert_int(int(back.crops[CROP].get("sick", 0))).is_equal(0)


func test_heavy_smoke_over_a_crop_fouls_it_and_a_wisp_does_not() -> void:
	var farm := _farm()
	vox[CROP + Vector3i(0, 1, 0)] = GameMaterials.SMOKE
	assert_str(farm.pollution(CROP)).is_equal("")
	vox[CROP + Vector3i(0, 2, 0)] = GameMaterials.SMOKE
	assert_str(farm.pollution(CROP)).is_equal("heavy smoke")


func test_masks_filter_smoke() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	for id in ["cloth_mask", "diving_mask"]:
		assert_bool(bool(items.get_def(id)["clothing"].get("smoke_mask", false))).override_failure_message(id).is_true()
	assert_str(Clothing.slot_of(items.get_def("cloth_mask"))).is_equal("face")
	var made := false
	for r in recipes.recipes.values():
		made = made or String(r.get("output", {}).get("id", "")) == "cloth_mask"
	assert_bool(made).is_true()


func test_a_fish_dead_of_fouled_water_is_saved_floating() -> void:
	var mm: MobManager = auto_free(MobManager.new())
	mm.configure(MobManager.load_defs())
	var m: Mob = auto_free(Mob.create(mm, "fish", mm.defs["fish"], false))
	mm._active.append(m)
	m.foul_s = Settings.fish_foul_die_s
	m._die_fouled()
	m.dying = 0.4
	var saved: Array = mm.to_dict()["mobs"]
	assert_int(saved.size()).override_failure_message("the floating fish is kept").is_equal(1)
	var back: Mob = auto_free(Mob.create(mm, "fish", mm.defs["fish"], false))
	back.load_from(saved[0])
	assert_bool(back.is_dead() and back.has_meta("fouled")).is_true()
	assert_float(back.dying).is_equal_approx(0.4, 0.001)
