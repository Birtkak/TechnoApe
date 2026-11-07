extends GdUnitTestSuite

## Meteorites (ADR 0157): the materials and the iron route, the deterministic night roll and spot, and
## the crater made once, only when loaded, kept across a save, cooled after its hot hours.

const GROUND := 100


class StubBubble:
	extends RefCounted
	var loaded := false
	var blasts := []
	func blast(center: Vector3, radius: float, material := 0, only := PackedInt32Array()) -> int:
		blasts.append([center, radius, material, only])
		return 1
	func get_voxel(p: Vector3i) -> int:
		if not loaded:
			return -1
		return GameMaterials.STONE if p.y < GROUND else GameMaterials.AIR


class StubWorld:
	extends RefCounted
	func surface_height(_x: int, _z: int) -> int:
		return GROUND


class StubMain:
	extends Node
	var bubble := StubBubble.new()
	var world := StubWorld.new()


func test_materials_and_the_iron_route() -> void:
	assert_int(GameMaterials.METEORIC_IRON).is_equal(147)
	assert_int(GameMaterials.SCORCHED_EARTH).is_equal(148)
	assert_int(GameMaterials.id_of("meteoric_iron")).is_equal(147)
	assert_int(GameMaterials.id_of("scorched_earth")).is_equal(148)
	assert_array(GameMaterials.defs()[146]["tiles"]).is_equal([186, 186, 186])
	var items := ItemRegistry.load_default()
	assert_bool(items.has("meteoric_iron")).is_true()
	var recipes := RecipeRegistry.load_default(items)
	var r: Dictionary = recipes.smelting_for("meteoric_iron", "furnace")
	assert_str(String(r.get("output", {}).get("id", ""))).is_equal("iron_ingot")   # no bloomery, no bloom
	var mining := MiningTable.load_default()
	assert_str(str(mining.entry("meteoric_iron").get("drops", ""))).is_equal("meteoric_iron")
	assert_int(int(mining.entry("meteoric_iron").get("min_tier", 9))).is_equal(1)   # a copper pick


func test_night_roll_and_spot_are_deterministic() -> void:
	assert_float(Meteors.fall_time(7, 3, 0.0)).is_equal(-1.0)
	var t := Meteors.fall_time(7, 3, 1.0)
	assert_float(t).is_between(3.85, 4.15)
	assert_float(Meteors.fall_time(7, 3, 1.0)).is_equal(t)
	var falls := 0
	for d in 400:
		if Meteors.fall_time(7, d, 0.12) >= 0.0:
			falls += 1
	assert_int(falls).is_between(25, 75)   # about 12 % of nights
	var from := Vector3(1000, 900, -400)
	var a := Meteors.spot_for(7, t, from, 250.0, 600.0)
	assert_vector(a[0]).is_equal(Meteors.spot_for(7, t, from, 250.0, 600.0)[0])
	assert_vector(a[0]).is_not_equal(Meteors.spot_for(7, t + 0.01, from, 250.0, 600.0)[0])
	assert_vector(a[0]).is_not_equal(Meteors.spot_for(8, t, from, 250.0, 600.0)[0])
	var d_m: float = (a[0] as Vector2).distance_to(Vector2(from.x, from.z)) * Settings.VOXEL_M
	assert_float(d_m).is_between(249.0, 601.0)
	assert_str(Meteors.compass(0, -1)).is_equal("north")
	assert_str(Meteors.compass(1, 0)).is_equal("east")


func test_crater_applied_once_when_loaded_and_kept_across_a_save() -> void:
	var main := auto_free(StubMain.new()) as StubMain
	var m := auto_free(Meteors.new()) as Meteors
	m.main = main
	m.events.append({"t": 10.0, "x": 40, "z": -20, "seed": 5, "fire": true, "applied": false, "cooled": false})
	m.check(10.0)
	assert_int(main.bubble.blasts.size()).is_equal(0)   # not loaded yet: waits
	main.bubble.loaded = true
	m.check(10.01)
	var made: int = main.bubble.blasts.size()
	assert_int(made).is_greater(4)
	assert_bool(m.events[0]["applied"]).is_true()
	var mats := main.bubble.blasts.map(func(b: Array) -> int: return b[2])
	assert_bool(mats.has(GameMaterials.METEORIC_IRON)).is_true()
	assert_bool(mats.has(GameMaterials.SCORCHED_EARTH)).is_true()
	assert_bool(mats.has(GameMaterials.LAVA)).is_true()   # hot: a molten core
	assert_bool(mats.has(GameMaterials.FIRE)).is_true()
	var bowl: Vector3 = main.bubble.blasts[0][0]
	assert_float(bowl.y).is_between(GROUND - 1.0, GROUND + Settings.m2v(Settings.meteor_crater_m))
	m.check(10.02)
	assert_int(main.bubble.blasts.size()).is_equal(made)   # once only
	# saved and loaded: still made, never again
	var m2 := auto_free(Meteors.new()) as Meteors
	m2.main = main
	m2.load_dict(JSON.parse_string(JSON.stringify(m.to_dict())))
	m2.check(10.03)
	assert_int(main.bubble.blasts.size()).is_equal(made)
	# after its hot hours the lava turns to basalt, once
	m2.check(10.0 + Settings.meteor_hot_h / 24.0 + 0.01)
	assert_int(main.bubble.blasts.size()).is_equal(made + 1)
	assert_int(main.bubble.blasts[-1][2]).is_equal(GameMaterials.BASALT)
	m2.check(12.0)
	assert_int(main.bubble.blasts.size()).is_equal(made + 1)


func test_a_cold_arrival_finds_no_lava() -> void:
	var main := auto_free(StubMain.new()) as StubMain
	main.bubble.loaded = true
	var m := auto_free(Meteors.new()) as Meteors
	m.main = main
	m.events.append({"t": 1.0, "x": 0, "z": 0, "seed": 9, "fire": true, "applied": false, "cooled": false})
	m.check(5.0)
	var mats := main.bubble.blasts.map(func(b: Array) -> int: return b[2])
	assert_bool(mats.has(GameMaterials.METEORIC_IRON)).is_true()
	assert_bool(mats.has(GameMaterials.LAVA)).is_false()
	assert_bool(mats.has(GameMaterials.FIRE)).is_false()
	assert_bool(m.events[0]["cooled"]).is_true()


func test_the_walk_finds_meteoric_iron() -> void:
	var w = load("res://scripts/crafting/progression_walk.gd").load_default()
	w.walk()
	assert_bool(w.have.has("meteoric_iron")).is_true()
	assert_bool(w.cost.get("meteoric_iron", 0.0) > 300.0).is_true()   # rare: never a cheap road
