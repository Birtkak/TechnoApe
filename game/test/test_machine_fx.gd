extends GdUnitTestSuite

## Running machines read as running (ADR 0153): MachineFx's data parses on every kind it names, effects
## start with work and stop with it, a starved machine blinks, needles follow their value, belts give no light.

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities
var mfx: MachineFx

const B := Settings.BLOCK


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)
	Inventory.clock_s = 1000.0
	MachineFx.errors.clear()
	mfx = auto_free(MachineFx.new())
	mfx.setup(store, null, null)


func after_test() -> void:
	Inventory.clock_s = 0.0


func _frames(n: int, eye: Vector3) -> void:
	for i in n:
		mfx.step(1.0 / 60.0, eye)


func test_every_kind_evaluates() -> void:
	var x := 0
	for kind in MachineFx.data()["kinds"]:
		var e: RefCounted = store.create(Vector3i(x, 0, 0), kind)
		x += 8 * B
		assert_object(e).override_failure_message(kind).is_not_null()
		MachineFx.working(e)
		MachineFx.starved(e)
		for p in MachineFx.spec(kind).get("parts", []):
			if p.get("motion", "") == "needle":
				assert_that(MachineFx.eval(p["value"], e)).override_failure_message(kind).is_not_null()
		for em in MachineFx.spec(kind).get("emit", []):
			assert_bool(ParticleFx.load_default().effects.has(em["effect"])).override_failure_message(em["effect"]).is_true()
	assert_array(MachineFx.errors).is_empty()


func test_boiler_puffs_while_lit_and_stops_when_out() -> void:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	var eye := Vector3(10, 10, -30)
	_frames(60, eye)
	assert_int(mfx.stats["near"]).is_equal(1)
	assert_int(mfx.stats["bursts"]).is_equal(0)   # cold: still
	assert_bool(b.ignite()).is_true()
	b.pressure = 1.0
	_frames(120, eye)
	assert_int(mfx.stats["working"]).is_equal(1)
	var made: int = mfx.stats["bursts"]
	assert_int(made).is_greater(5)   # steam, smoke and embers over 2 s
	var needle: Node3D = mfx._recs[b.pos]["parts"][0]["pivot"]
	assert_float(absf(needle.basis.get_euler().z)).is_greater(0.3)   # pressure 1.0 of 1.5: 23 degrees past upright
	b.lit_time = 0
	_frames(30, eye)   # one evaluation turns it off
	var after: int = mfx.stats["bursts"]
	_frames(120, eye)
	assert_int(mfx.stats["bursts"]).is_equal(after)
	assert_int(mfx.stats["working"]).is_equal(0)


func test_lv_machine_spins_when_powered_and_blinks_when_starved() -> void:
	var m: MachineEntity = store.create(Vector3i.ZERO, "macerator")
	m.use({"id": "iron_ore", "count": items.units("iron_ore")})
	var eye := Vector3(8, 8, -20)
	_frames(30, eye)
	var r: Dictionary = mfx._recs[m.pos]
	assert_bool(r["starved"]).is_true()   # ore in, no power
	assert_bool(r["working"]).is_false()
	var roller: Node3D = r["parts"][0]["pivot"]
	var still := roller.basis
	_frames(30, eye)
	assert_bool(roller.basis.is_equal_approx(still)).is_true()
	var blinks := 0
	for i in 60:
		mfx.step(1.0 / 60.0, eye)
		blinks += int((r["lamp"] as Node).get_meta("st", 0) == 2)
	assert_int(blinks).is_between(15, 45)   # on half the time
	m.power_ticks = 1000
	_frames(30, eye)
	assert_bool(r["working"]).is_true()
	assert_int((r["lamp"] as Node).get_meta("st", 0)).is_equal(1)   # steady run lamp
	assert_bool(roller.basis.is_equal_approx(still)).is_false()


func test_far_machines_are_not_evaluated() -> void:
	store.create(Vector3i.ZERO, "boiler")
	_frames(40, Vector3(Settings.m2v(Settings.machine_fx_m) + 100.0, 0, 0))
	assert_int(mfx.stats["near"]).is_equal(0)


func test_belts_and_lathes_give_no_firelight() -> void:
	assert_str(MachineFx.light_of("belt")).is_equal("none")
	assert_str(MachineFx.light_of("lathe")).is_equal("none")
	assert_str(MachineFx.light_of("electric_lamp")).is_equal("lamp")
	assert_str(MachineFx.light_of("firepit")).is_equal("fire")   # no entry: unchanged
