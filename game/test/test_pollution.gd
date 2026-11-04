extends GdUnitTestSuite

## ADR 0141: pollution is real matter. The sim rules (sludge fouls water, a still body dilutes it back, a dug pond
## stays fouled, sludge eats soil one layer deep and settles on gravel, resting sludge on stone sleeps, smoke drifts
## with the wind), machine waste out of a port (stall when blocked, chimneys, pipes into the scrubber), and what fouled
## water does to crops and to the player.

const N := 3
const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


func _sim() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(true)
	w.set_cascade(true)
	w.set_gravity(0.17)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(14, 1, 14), Vector3i(26, 8, 26), GameMaterials.STONE)   # a basin so nothing runs off
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 8, 25), 0)
	return w


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


func _count(w: SimWorld, m: int) -> int:
	return w.count_in_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), m)


# ---------- the sim rules ----------

func test_sludge_fouls_a_still_body_that_clears_once_it_stops() -> void:
	var w := _sim()
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 5, 25), GameMaterials.WATER_STATIC)   # a generated pool
	w.fill_box(Vector3i(19, 5, 19), Vector3i(21, 6, 21), GameMaterials.TOXIC_SLUDGE)   # 4 cells poured on it
	_run(w, 300)
	var peak := _count(w, GameMaterials.FOULED_WATER)
	assert_int(peak).override_failure_message("no fouled water").is_greater(3)
	assert_int(_count(w, GameMaterials.TOXIC_SLUDGE)).is_equal(0)
	_run(w, 12000)
	var left := _count(w, GameMaterials.FOULED_WATER)
	assert_int(left).override_failure_message("%d of %d fouled left" % [left, peak]).is_less(maxi(1, peak / 2))


func test_a_dug_pond_stays_fouled() -> void:
	var w := _sim()
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 3, 25), GameMaterials.WATER)   # moving water: a pond the player made
	_run(w, 30)
	w.fill_box(Vector3i(19, 2, 19), Vector3i(21, 3, 21), GameMaterials.FOULED_WATER)
	var before := _count(w, GameMaterials.FOULED_WATER)
	_run(w, 3000)
	assert_int(_count(w, GameMaterials.FOULED_WATER)).is_greater(before)
	assert_int(_count(w, GameMaterials.WATER_STATIC)).is_equal(0)


func test_sludge_eats_soil_one_layer_deep() -> void:
	var w := _sim()
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 4, 25), GameMaterials.DIRT)   # three layers of dirt
	w.fill_box(Vector3i(15, 4, 15), Vector3i(25, 5, 25), GameMaterials.TOXIC_SLUDGE)
	_run(w, 4000)
	var top := w.count_in_box(Vector3i(15, 3, 15), Vector3i(25, 4, 25), GameMaterials.WASTE_GROUND)
	assert_int(top).override_failure_message("%d of 100 top dirt cells eaten" % top).is_greater(30)
	assert_int(w.count_in_box(Vector3i(15, 1, 15), Vector3i(25, 3, 25), GameMaterials.DIRT)).is_equal(200)   # the layers under it are untouched


func test_sludge_settles_on_gravel_and_resting_sludge_on_stone_sleeps() -> void:
	var w := _sim()
	w.fill_box(Vector3i(15, 1, 15), Vector3i(25, 2, 25), GameMaterials.GRAVEL_STATIC)   # the tank's gravel bed
	w.fill_box(Vector3i(15, 2, 15), Vector3i(25, 4, 25), GameMaterials.TOXIC_SLUDGE)   # two layers poured in
	_run(w, 2000)
	var settled := w.count_in_box(Vector3i(15, 2, 15), Vector3i(25, 4, 25), GameMaterials.WASTE_GROUND)
	assert_int(settled).override_failure_message("%d of 200 settled" % settled).is_greater(100)
	assert_int(w.count_in_box(Vector3i(15, 1, 15), Vector3i(25, 2, 25), GameMaterials.GRAVEL_STATIC)).is_equal(100)   # the bed stays
	# poured thin it spreads to films: a film settles in proportion to its eighths, it does not vanish at the first touch
	var thin := _sim()
	thin.fill_box(Vector3i(15, 1, 15), Vector3i(25, 2, 25), GameMaterials.GRAVEL_STATIC)
	thin.fill_box(Vector3i(15, 2, 15), Vector3i(20, 3, 25), GameMaterials.TOXIC_SLUDGE)   # 50 cells over a 100-cell bed
	_run(thin, 200)
	var kept := _count(thin, GameMaterials.TOXIC_SLUDGE) + _count(thin, GameMaterials.WASTE_GROUND)
	assert_int(kept).override_failure_message("%d of 50 left after 200 ticks" % kept).is_greater(35)
	_run(thin, 4000)
	assert_int(_count(thin, GameMaterials.WASTE_GROUND)).is_greater(15)
	# sludge alone on stone has nothing to react with: it rests and its chunks sleep
	var s := _sim()
	s.fill_box(Vector3i(15, 1, 15), Vector3i(25, 2, 25), GameMaterials.TOXIC_SLUDGE)
	_run(s, 400)
	assert_int(s.get_active_chunk_count()).is_equal(0)


func test_smoke_drifts_with_the_wind() -> void:
	var still := _sim()
	var windy := _sim()
	windy.set_wind(Vector2(0.8, 0.0))
	for w in [still, windy]:
		w.fill_box(Vector3i(22, 10, 22), Vector3i(26, 12, 26), GameMaterials.SMOKE)
		_run(w, 40)
	var east_windy := windy.count_in_box(Vector3i(30, 0, 0), Vector3i(48, 48, 48), GameMaterials.SMOKE)
	var east_still := still.count_in_box(Vector3i(30, 0, 0), Vector3i(48, 48, 48), GameMaterials.SMOKE)
	assert_int(east_windy).override_failure_message("windy %d, still %d" % [east_windy, east_still]).is_greater(east_still + 5)
	assert_int(windy.count_in_box(Vector3i(0, 0, 0), Vector3i(18, 48, 48), GameMaterials.SMOKE)).is_less_equal(still.count_in_box(Vector3i(0, 0, 0), Vector3i(18, 48, 48), GameMaterials.SMOKE))


func test_ids_and_rules_are_data() -> void:
	assert_str(GameMaterials.name_of(218)).is_equal("fouled_water")
	assert_str(GameMaterials.name_of(219)).is_equal("waste_ground")
	assert_str(GameMaterials.name_of(220)).is_equal("slag")
	assert_str(GameMaterials.name_of(221)).is_equal("chimney")
	assert_str(GameMaterials.name_of(222)).is_equal("reserved_222")
	assert_str(GameMaterials.name_of(GameMaterials.CAVE_IDS)).is_equal("dripstone")
	for r in GameMaterials.waste_reactions():
		if r.get("passive", false):
			assert_bool(int(r["input_cell1"]) in [GameMaterials.TOXIC_SLUDGE, GameMaterials.FOULED_WATER]).is_true()   # only the waste rolls; ground and still water stay inert
	var mining := MiningTable.load_default()
	for n in ["waste_ground", "slag", "chimney"]:
		assert_bool(mining.entry(n).is_empty()).override_failure_message("%s not in mining.json" % n).is_false()
		assert_bool(items.has(n)).is_true()


# ---------- machine waste ----------

func _vox_world(vox: Dictionary) -> Array:   # [get_voxel, set_voxel] over a dictionary (air where absent)
	return [func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR)),
		func(p: Vector3i, m: int) -> void: vox[p] = m]


func _electrolyzer() -> MachineEntity:
	var m := MachineEntity.new(items, recipes, Vector3i.ZERO, "electrolyzer")
	m.use({"id": "crushed_copper_ore", "count": 64})
	m.use({"id": "carbon_electrode", "count": 16})
	return m


func test_a_machine_holds_its_waste_and_stalls_when_the_port_is_blocked() -> void:
	var m := _electrolyzer()
	for i in 2000:
		m.power_ticks = 2
		m.tick()
	assert_int(Waste.total(m.waste)).is_equal(Settings.waste_port_cells)   # 4 runs of 64 cells, then it stops
	assert_int(m.made).is_equal(8)
	assert_bool(m.current().is_empty()).is_true()
	assert_str(" ".join(m.look_lines())).contains("Waste port blocked: toxic sludge")
	# the port at the back (-z for facing 0), at floor level: air takes it, a wall keeps it
	var vox := {}
	var io := _vox_world(vox)
	for c in Waste.port_cells(m.pos, 2 * B, 2 * B, Waste.back_dir(m), false):
		assert_int(c.z).is_less(0)
		assert_int(c.y).is_less(2)
	assert_int(Waste.emit(m, io[0], io[1], {})).is_equal(8)
	assert_int(Waste.emit(m, io[0], io[1], {})).is_equal(0)   # its own sludge sits there now
	assert_bool(m.current().is_empty()).is_false()   # 8 cells out: it runs again
	var n := 0
	for c in vox:
		assert_int(int(vox[c])).is_equal(GameMaterials.TOXIC_SLUDGE)
		n += 1
	assert_int(n).is_equal(8)
	var saved := MachineEntity.new(items, recipes, Vector3i.ZERO, "electrolyzer")
	saved.load_from(JSON.parse_string(JSON.stringify(m.to_dict())))
	assert_int(Waste.total(saved.waste)).is_equal(Waste.total(m.waste))


func test_smoke_leaves_from_the_top_of_a_chimney_stack() -> void:
	var m := MachineEntity.new(items, recipes, Vector3i.ZERO, "alloy_smelter")
	Waste.add(m.waste, GameMaterials.SMOKE, 6)
	var vox := {}
	for y in range(2 * B, 4 * B):   # two chimney blocks on the smelter's top, over its (0, 0) quarter
		for x in B:
			for z in B:
				vox[Vector3i(x, y, z)] = GameMaterials.CHIMNEY
	var io := _vox_world(vox)
	assert_int(Waste.emit(m, io[0], io[1], {})).is_equal(6)
	for c in vox:
		if vox[c] == GameMaterials.SMOKE:
			assert_int(c.y).is_greater_equal(4 * B)
			assert_bool(c.x < B and c.z < B).is_true()
	# without the chimney it leaves just over the machine's top
	var bare := {}
	var io2 := _vox_world(bare)
	Waste.add(m.waste, GameMaterials.SMOKE, 4)
	assert_int(Waste.emit(m, io2[0], io2[1], {})).is_equal(4)
	for c in bare:
		assert_int(c.y).is_between(2 * B, 2 * B + 1)


func test_piped_sludge_fills_the_scrubber_which_makes_sulfur_and_clean_water() -> void:
	var store := BlockEntities.new(items, recipes)
	var e: MachineEntity = store.create(Vector3i.ZERO, "electrolyzer")
	for z in [-B, -2 * B]:   # behind it, a run to the scrubber
		store.create(Vector3i(0, 0, z), "pipe").axis = 1
	var s: MachineEntity = store.create(Vector3i(0, 0, -4 * B), "scrubber")
	assert_int(s.intake_material).is_equal(GameMaterials.TOXIC_SLUDGE)
	Waste.add(e.waste, GameMaterials.TOXIC_SLUDGE, 64)
	var runs := Waste.pipe_runs(store, [e])
	assert_int(runs[e]["pipes"].size()).is_equal(2)
	assert_bool(s in runs[e]["intakes"]).is_true()
	var vox := {}
	for piece in [e, s]:
		for c in BlockEntities.footprint(piece):
			vox[c] = GameMaterials.WORK_BLOCK
	var solid := vox.size()
	var io := _vox_world(vox)
	for i in 4:
		Waste.emit(e, io[0], io[1], runs)
	assert_int(s.intake).is_equal(64)
	assert_int(vox.size()).is_equal(solid)   # nothing spilled
	# a bucket by hand pours into its tank and the bucket comes back; a bucket of it makes one sulfur
	var r := s.use({"id": "toxic_sludge_bucket", "count": 1})
	assert_int(int(r["consume"])).is_equal(1)
	assert_str(String(r["give"][0]["id"])).is_equal("bucket")
	assert_str(String(s.current().get("id", ""))).is_equal("scrub_sludge")
	for i in 200:
		s.power_ticks = 2
		s.tick()
	assert_str(String(s.output.get("id", ""))).is_equal("sulfur")
	assert_int(int(s.waste.get(GameMaterials.WATER, 0))).is_greater(0)
	assert_int(s.intake).is_equal(64)
	# a full scrubber backs the line up: the run ends in it, so the sludge waits in the electrolyzer
	s.intake = Settings.waste_intake_buckets * Bucket.capacity()
	Waste.add(e.waste, GameMaterials.TOXIC_SLUDGE, 16)
	assert_int(Waste.emit(e, io[0], io[1], Waste.pipe_runs(store, [e]))).is_equal(0)
	assert_int(Waste.total(e.waste)).is_equal(16)


func test_a_firebox_leaves_ash() -> void:
	var store := BlockEntities.new(items, recipes)
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	assert_bool(b.ignite()).is_true()
	assert_int(int(b.waste.get(GameMaterials.ASH, 0))).is_equal(8)
	assert_int(int(Waste.firebox(items, "small_boiler").get("smoke_s", 0))).is_greater(0)
	Waste.add(b.waste, GameMaterials.ASH, Settings.waste_port_cells)
	b.lit_time = 1
	b.tick()   # the next coal would light: the choked port lets the fire die instead
	assert_bool(b.is_lit()).is_false()


# ---------- fouled water's harm ----------

func test_fouled_water_wilts_the_crop_it_waters() -> void:
	var farm := Farming.load_default()
	var vox := {}
	farm.get_voxel = func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR))
	farm.set_voxel = func(p: Vector3i, m: int) -> void:
		if m == GameMaterials.AIR:
			vox.erase(p)
		else:
			vox[p] = m
	farm.rng.seed = 1234
	farm.block = 1
	for x in range(-8, 9):
		for z in range(-8, 9):
			vox[Vector3i(x, 0, z)] = GameMaterials.GRASS
	vox[Vector3i(3, 0, 0)] = GameMaterials.FOULED_WATER
	farm.till(Vector3i(0, 0, 0))
	assert_bool(farm.plant(Vector3i(0, 0, 0), "wheat")).is_true()
	assert_bool(farm.near_water(Vector3i(0, 0, 0))).is_true()   # it waters all the same
	assert_bool(farm.fouled_water(Vector3i(0, 0, 0))).is_true()
	for i in 600:
		farm.tick(20)
	assert_int(int(farm.stats["wilted"])).is_equal(1)
	assert_bool(farm.crops.is_empty()).is_true()


func test_drinking_fouled_water_makes_you_sick() -> void:
	var s: Survival = auto_free(Survival.new())
	assert_bool(items.get_def("bucket")["fills"].has("fouled_water")).is_true()
	s.eat(items.get_def("water_bucket"), 0.0)
	assert_float(s.poison_s).is_equal(0.0)
	s.eat(items.get_def("fouled_water_bucket"), 0.0)
	assert_float(s.poison_s).is_greater(10.0)
