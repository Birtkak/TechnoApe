extends GdUnitTestSuite

## The LV network in real units (ADR 0123): the native DC solver (a voltage divider, cable drop, a collapsed run,
## a junction), and the entities on it: a dynamo over cables runs a furnace at ~48 V, too many machines on one
## dynamo settle on what it carries instead of flapping, a cable past its amps burns, a fuse blows, a switch
## cuts, a battery charges from the dynamo and carries the lamp when the boiler goes cold, saves.

const B := Settings.BLOCK

var items: ItemRegistry
var recipes: RecipeRegistry
var store: BlockEntities


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	store = BlockEntities.new(items, recipes)
	Inventory.clock_s = 1000.0


func after_test() -> void:
	Inventory.clock_s = 0.0


func _box(x: int, w: int = B) -> PackedInt32Array:
	return PackedInt32Array([x, 0, 0, w, B, w])


# ---------- the solver ----------

func test_voltage_divider_source_cable_load() -> void:
	var s := NetworkSolver.new()
	# a 52 V source behind 0.3 ohm, a 1 ohm cable, a load of 400 W at 48 V (G = 0.1736 S, R = 5.76 ohm)
	var boxes := _box(0) + _box(B) + _box(2 * B)
	var kinds := PackedInt32Array([NetworkSolver.DC_SOURCE, NetworkSolver.DC_CONDUCTOR, NetworkSolver.DC_LOAD])
	var r := s.solve_dc(boxes, kinds, PackedFloat32Array([52.0, 1.0, 400.0 / (48.0 * 48.0)]), PackedFloat32Array([0.3, 0.0, 0.0]))
	# the load's node: 52 * R / (r + Rcable + R) where the cable edges carry (r_a + r_b) / 2 each: 0.5 + 0.5 + bus
	var i := 52.0 / (0.3 + 1.0 + 0.0001 + 5.76)
	assert_float(r[2 * 4 + 1]).is_equal_approx(i * 5.76, 0.05)   # ~42.4 V at the load
	assert_float(r[1 * 4 + 2]).is_equal_approx(i, 0.01)           # the cable carries the loop current
	assert_float(r[0 * 4 + 2]).is_equal_approx(i, 0.01)
	assert_float(r[2 * 4 + 3]).is_equal_approx(i * i * 5.76, 1.0)  # watts into the load
	assert_int(s.dc_stats()["count"]).is_equal(1)


func test_a_run_of_cables_collapses_to_one_resistor_with_the_same_answer() -> void:
	var s := NetworkSolver.new()
	var n := 200
	var boxes := PackedInt32Array()
	var kinds := PackedInt32Array()
	var a := PackedFloat32Array()
	var b := PackedFloat32Array()
	for i in n + 2:
		boxes.append_array(_box(i * B))
		kinds.append(NetworkSolver.DC_SOURCE if i == 0 else (NetworkSolver.DC_LOAD if i == n + 1 else NetworkSolver.DC_CONDUCTOR))
		a.append(52.0 if i == 0 else (0.1736 if i == n + 1 else 0.0014))
		b.append(0.3 if i == 0 else 0.0)
	var r := s.solve_dc(boxes, kinds, a, b)
	assert_int(s.dc_stats()["reduced_nodes"]).is_equal(2)   # the 200 cables are one resistor between the ends
	var rc := n * 0.0014
	var i := 52.0 / (0.3 + rc + 0.0001 + 1.0 / 0.1736)
	assert_float(r[(n + 1) * 4 + 1]).is_equal_approx(i / 0.1736, 0.05)
	# every cable carries the loop current and the voltage falls along the run
	assert_float(r[50 * 4 + 2]).is_equal_approx(i, 0.01)
	assert_bool(r[10 * 4 + 1] > r[150 * 4 + 1]).is_true()


func test_two_loads_at_a_junction_share_and_an_open_branch_floats() -> void:
	var s := NetworkSolver.new()
	# source at 0, cable at B, two loads at 2B and (B, 0, B) (both touch the cable), a lone cable far away
	var boxes := _box(0) + _box(B) + _box(2 * B) + PackedInt32Array([B, 0, B, B, B, B]) + _box(20 * B)
	var kinds := PackedInt32Array([NetworkSolver.DC_SOURCE, NetworkSolver.DC_CONDUCTOR, NetworkSolver.DC_LOAD, NetworkSolver.DC_LOAD, NetworkSolver.DC_CONDUCTOR])
	var r := s.solve_dc(boxes, kinds, PackedFloat32Array([52.0, 0.0014, 0.1736, 0.1736, 0.0014]), PackedFloat32Array([0.3, 0, 0, 0, 0]))
	assert_float(r[2 * 4 + 1]).is_equal_approx(r[3 * 4 + 1], 0.01)
	assert_float(r[0 * 4 + 2]).is_equal_approx(r[2 * 4 + 2] + r[3 * 4 + 2], 0.01)
	assert_float(r[4 * 4 + 1]).is_equal(0.0)
	assert_int(s.dc_stats()["count"]).is_equal(2)


# ---------- the entities ----------

## A lit steel boiler at the origin, a pipe at 5B, the turbine at 6B (3 blocks wide), a cable at 9B: machines from 10B.
func _dynamo() -> BoilerEntity:
	var b: BoilerEntity = store.create(Vector3i.ZERO, "boiler")
	b.item = "steel_boiler"
	b.use({"id": "coal", "count": 8}, items.get_def("coal"), false, true)
	for i in 4:
		b.use({"id": "water_bucket", "count": 1}, items.get_def("water_bucket"))
	assert_bool(b.ignite()).is_true()
	store.create(Vector3i(5 * B, 0, 0), "pipe")
	store.create(Vector3i(6 * B, 0, 0), "turbine")
	store.create(Vector3i(9 * B, 0, 0), "cable")
	return b


func _ticks(n: int) -> void:
	for i in n:
		Inventory.clock_s += BlockEntities.TICK_S
		store.tick()


func test_dynamo_runs_a_furnace_at_forty_eight_volts() -> void:
	_dynamo()
	var f: ElectricFurnaceEntity = store.create(Vector3i(10 * B, 0, 0), "electric_furnace")
	f.use({"id": "raw_meat", "count": items.units("raw_meat")})
	_ticks(20 * 15)
	assert_bool(f.is_lit()).override_failure_message(f.load.status()).is_true()
	assert_float(f.load.volts).is_between(44.0, 52.0)
	assert_float(f.load.drawn).is_between(300.0, 500.0)
	assert_str(Networks.look_line(f)).contains("Power:")
	_ticks(20 * 15)
	assert_int(f.smelted).is_greater(0)


func test_too_many_machines_settle_without_flapping() -> void:
	_dynamo()
	var ms: Array = []
	for k in 8:   # eight lathes along one cable (each 2 blocks wide): 1.6 kW wanted; the dynamo sags under ~6 of them
		store.create(Vector3i((10 + 4 * k) * B, 0, 0), "cable")
		store.create(Vector3i((11 + 4 * k) * B, 0, 0), "cable")
		var m: MachineEntity = store.create(Vector3i((12 + 4 * k) * B, 0, 0), "lathe")
		m.use({"id": "iron_ingot", "count": 64})
		ms.append(m)
	_ticks(20 * 30)
	var running := 0
	for m in ms:
		running += 1 if m.load.on else 0
	assert_int(running).is_between(3, 7)
	for m in ms:
		if m.load.on:
			assert_float(m.load.volts).is_greater_equal(Settings.lv_stop_frac * Settings.lv_volts)
	# stable: the same set is running ten seconds later
	var before: Array = ms.map(func(m): return m.load.on)
	_ticks(20 * 10)
	assert_array(ms.map(func(m): return m.load.on)).is_equal(before)


func test_overcurrent_burns_a_cable_and_a_fuse_blows_first_when_fitted() -> void:
	# one cable rated 40 A feeding a 3 kW load at 48 V: 62 A
	var s := NetworkSolver.new()
	var cable: CableEntity = store.create(Vector3i(0, 0, 0), "cable")
	for i in 12:
		cable.net_in("energy", {"volts": 48.0, "amps": 62.0, "watts": 0.0})
	assert_bool(cable.burnt).is_true()
	assert_dict(cable.net("energy")).is_empty()
	assert_int(cable.use({"id": "copper_cable", "count": 2})["consume"]).is_equal(1)
	assert_bool(cable.burnt).is_false()
	var fuse: CircuitEntity = store.create(Vector3i(4 * B, 0, 0), "fuse_box")
	assert_bool(fuse.net_in("energy", {"volts": 48.0, "amps": 31.0, "watts": 0.0})).is_false()
	assert_bool(fuse.open).is_true()
	assert_dict(fuse.net("energy")).is_empty()
	assert_str(fuse.use({"id": "bucket", "count": 1})["msg"]).contains("blown")
	assert_int(fuse.use({"id": "fuse", "count": 4})["consume"]).is_equal(1)
	assert_bool(fuse.open).is_false()
	var br: CircuitEntity = store.create(Vector3i(8 * B, 0, 0), "breaker")
	br.net_in("energy", {"volts": 48.0, "amps": 40.0, "watts": 0.0})
	assert_bool(br.open).is_true()
	br.use({})
	assert_bool(br.open).is_false()


func test_a_switch_cuts_the_circuit_beyond_it() -> void:
	_dynamo()
	var sw: CircuitEntity = store.create(Vector3i(10 * B, 0, 0), "knife_switch")
	var lamp: ElectricLampEntity = store.create(Vector3i(11 * B, 0, 0), "electric_lamp")
	_ticks(20 * 10)
	assert_bool(lamp.is_lit()).override_failure_message(lamp.load.status()).is_true()
	sw.use({})
	assert_bool(sw.open).is_true()
	_ticks(40)
	assert_bool(lamp.is_lit()).is_false()
	sw.use({})
	_ticks(20 * 8)
	assert_bool(lamp.is_lit()).is_true()


func test_battery_charges_from_the_dynamo_and_carries_the_lamp_at_night() -> void:
	var b := _dynamo()
	var bat: BatteryBoxEntity = store.create(Vector3i(10 * B, 0, 0), "battery_box")
	store.create(Vector3i(12 * B, 0, 0), "cable")
	var lamp: ElectricLampEntity = store.create(Vector3i(13 * B, 0, 0), "electric_lamp")
	_ticks(20 * 30)
	assert_bool(lamp.is_lit()).is_true()
	assert_bool(bat.charge > 1.0).override_failure_message("charge %.1f Wh, %.1f A" % [bat.charge, bat.amps]).is_true()
	assert_bool(bat.amps < 0.0).is_true()   # the dynamo above its EMF pushes current in
	b.pressure = 0.0
	b.lit_time = 0
	b.water = 0.0
	_ticks(20 * 5)
	assert_bool(lamp.is_lit()).override_failure_message(lamp.load.status()).is_true()
	assert_bool(bat.amps > 0.0).is_true()   # now it gives
	assert_str(bat.look_lines()[0]).contains("giving")


func test_saves_keep_the_circuit_state() -> void:
	var sw: CircuitEntity = store.create(Vector3i(0, 0, 0), "knife_switch")
	sw.open = true
	var c: CableEntity = store.create(Vector3i(4 * B, 0, 0), "cable")
	c.burnt = true
	var bat: BatteryBoxEntity = store.create(Vector3i(8 * B, 0, 0), "battery_box")
	bat.charge = 123.0
	var store2 := BlockEntities.new(items, recipes)
	assert_bool(store2.load_dict(store.to_dict())).is_true()
	assert_bool(store2.at(Vector3i(0, 0, 0)).open).is_true()
	assert_bool(store2.at(Vector3i(4 * B, 0, 0)).burnt).is_true()
	assert_float(store2.at(Vector3i(8 * B, 0, 0)).charge).is_equal(123.0)
