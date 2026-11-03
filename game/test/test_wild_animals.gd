extends GdUnitTestSuite

## The wild animals of ADR 0058: the data, the models, and the behaviours (flee, hop, swim, fly, pack
## hunts, boars that turn on whoever hurt them).

const NEW := ["deer", "boar", "rabbit", "fox", "wolf", "duck", "frog", "crab", "fish", "bat", "cave_spider", "poison_frog", "gloam_hound", "crow", "salmon", "jellyfish", "cave_salamander", "songbird", "goat"]


func _manager() -> MobManager:
	var mm: MobManager = auto_free(MobManager.new())
	mm.configure(MobManager.load_defs())
	return mm


func _mob(mm: MobManager, species: String, pos: Vector3) -> Mob:
	var m: Mob = auto_free(Mob.create(mm, species, mm.defs[species], false))
	m.position = pos
	mm._active.append(m)
	return m


func test_data() -> void:
	var mm := _manager()
	assert_array(mm.errors).is_empty()
	var items := ItemRegistry.load_default()
	for sp in NEW:
		assert_bool(mm.defs.has(sp)).override_failure_message(sp).is_true()
		var d: Dictionary = mm.defs[sp]
		assert_str(String(d.get("name", ""))).override_failure_message("%s: a display name (the journal reads it)" % sp).is_not_empty()
		assert_array(["land", "shore", "water", "cave", "sky"]).contains([WildMoves.habitat(d)])
		assert_array(["walk", "hop", "swim", "fly", "paddle"]).contains([WildMoves.moves_of(d)])
		for e in d.get("drops", []):
			assert_bool(items.has(String(e["id"]))).override_failure_message("%s drops unknown %s" % [sp, e["id"]]).is_true()
		for f in d.get("food", []):
			assert_bool(items.has(String(f))).override_failure_message("%s eats unknown %s" % [sp, f]).is_true()
		for p in d.get("predator", {}).get("prey", []):
			assert_bool(mm.defs.has(p)).override_failure_message("%s hunts unknown %s" % [sp, p]).is_true()
	# spawn biomes are real ones; cave groups use the cave biome names of their pocket
	var known := {}
	for b in JSON.parse_string(FileAccess.get_file_as_string("res://data/biomes.json"))["biomes"]:
		known[b["name"]] = b.get("band", "") == "cave"
	for sp in mm.defs:
		for b in mm.defs[sp].get("biomes", []):
			assert_bool(known.has(b)).override_failure_message("%s spawns in unknown biome %s" % [sp, b]).is_true()
			assert_bool(known.get(b, false) == (WildMoves.habitat(mm.defs[sp]) == "cave")).override_failure_message("%s: cave biomes for cave species only (%s)" % [sp, b]).is_true()
	# who bites: wolves (some), boars (when hurt), cave spiders (always); nothing else new
	assert_float(float(mm.defs["cave_spider"]["predator"]["aggressive_chance"])).is_equal(1.0)
	assert_float(float(mm.defs["fox"]["predator"]["aggressive_chance"])).is_equal(0.0)
	assert_bool(mm.defs["boar"].get("defends", false)).is_true()


func test_every_part_has_cells() -> void:
	for sp in NEW:
		assert_bool(WildModels.has(sp)).override_failure_message(sp).is_true()
		for part in WildModels.parts(sp):
			var spec: Array = MobModel._build(sp, part)
			var filled := 0
			for c in spec[0]:
				if c != 0:
					filled += 1
					assert_int(c - 1).is_less(MobModel.PALETTE.size())
			assert_int(filled).override_failure_message("%s/%s empty" % [sp, part]).is_greater(3)


func test_models_build_their_nodes() -> void:
	for sp in NEW:
		var m: MobModel = auto_free(MobModel.create(sp, false))
		m._ready()
		assert_object(m.body).override_failure_message(sp).is_not_null()
		assert_object(m.head).override_failure_message(sp).is_not_null()
		var legs := int(MobManager.load_defs()["mobs"][sp].get("legs", 4))
		assert_int(m.legs.size()).override_failure_message(sp).is_equal(legs)
		m.animate(0.1, 8.0, 0.0, 0.0, 0.0, 0.0)   # walks without errors, any number of legs


func test_skittish_animals_flee_the_player() -> void:
	var mm := _manager()
	var deer := _mob(mm, "deer", Vector3(0, 0, 0))
	# no player here: player_feet is ZERO, which the flee check ignores
	assert_bool(WildMoves.think(deer)).is_false()


func test_wolves_join_a_pack_mates_hunt() -> void:
	var mm := _manager()
	var a := _mob(mm, "wolf", Vector3(0, 0, 0))
	var b := _mob(mm, "wolf", Vector3(20, 0, 0))
	var deer := _mob(mm, "deer", Vector3(60, 0, 0))
	var rabbit := _mob(mm, "rabbit", Vector3(-8, 0, 0))
	a._prey = deer
	assert_object(WildMoves.pack_prey(b, rabbit)).is_same(deer)
	var fox := _mob(mm, "fox", Vector3(0, 0, 0))   # foxes hunt alone
	assert_object(WildMoves.pack_prey(fox, rabbit)).is_same(rabbit)


func test_boars_turn_on_the_attacker_and_deer_run() -> void:
	var mm := _manager()
	var boar := _mob(mm, "boar", Vector3(0, 0, 0))
	var deer := _mob(mm, "deer", Vector3(40, 0, 0))
	boar.hurt(1, Vector3(2, 0, 0))
	assert_float(boar._panic_t).is_equal(0.0)   # no running: it fights
	deer.hurt(1, Vector3(42, 0, 0))
	assert_float(deer._panic_t).is_greater(0.0)


func test_land_herds_spawn_on_their_ground() -> void:
	var mm := _manager()
	assert_bool(WildMoves.ground_ok(mm.defs["deer"], GameMaterials.GRASS)).is_true()
	assert_bool(WildMoves.ground_ok(mm.defs["crab"], GameMaterials.GRASS)).is_false()
	assert_bool(WildMoves.ground_ok(mm.defs["crab"], GameMaterials.id_of("sand"))).is_true()
	assert_bool(WildMoves.water_ok(_mob(mm, "duck", Vector3.ZERO))).is_true()
	assert_bool(WildMoves.water_ok(_mob(mm, "deer", Vector3.ZERO))).is_false()


## Night creatures (ADR 0120): gloam hounds never herd by day, hunt the player, and fade in daylight.
func test_gloam_hounds_belong_to_the_night() -> void:
	var mm := _manager()
	var d: Dictionary = mm.defs["gloam_hound"]
	assert_bool(d.get("night", false)).is_true()
	assert_float(float(d["predator"]["aggressive_chance"])).is_equal(1.0)
	assert_bool(mm.is_night()).is_false()   # no sky: day
	var hound := _mob(mm, "gloam_hound", Vector3(0, 100, 0))
	var sheep := _mob(mm, "sheep", Vector3(8, 100, 0))
	mm._night_tick()
	assert_bool(hound in mm.active()).is_false()
	assert_bool(sheep in mm.active()).is_true()
	assert_int(mm.stats["faded"]).is_equal(1)


## A hurt sheep alarms its herd (ADR 0120): mates nearby bolt, a far one and another species do not.
func test_a_hurt_animal_alarms_its_herd() -> void:
	var mm := _manager()
	var hit := _mob(mm, "sheep", Vector3(0, 100, 0))
	var mate := _mob(mm, "sheep", Vector3(Settings.m2v(5.0), 100, 0))
	var far := _mob(mm, "sheep", Vector3(Settings.m2v(30.0), 100, 0))
	var pig := _mob(mm, "pig", Vector3(Settings.m2v(3.0), 100, 0))
	assert_bool(hit.hurt(1, Vector3(-4, 100, 0))).is_true()
	assert_float(mate._panic_t).is_greater(0.0)
	assert_float(far._panic_t).is_equal(0.0)
	assert_float(pig._panic_t).is_equal(0.0)
	assert_int(mm.stats["alarmed"]).is_equal(1)


## Herds sleep at night (ADR 0120): a sheep stands still with its head down, a rabbit too (it hops).
func test_grazers_sleep_at_night() -> void:
	var mm := _manager()
	mm.night_now = true
	var sheep := _mob(mm, "sheep", Vector3(0, 100, 0))
	var rabbit := _mob(mm, "rabbit", Vector3(30, 100, 0))
	for m in [sheep, rabbit]:
		m._think_goal()
		assert_str(m.goal).is_equal("sleep")
		assert_vector(m._target).is_equal(Vector3.INF)


func test_songbirds_land_sit_and_take_off() -> void:
	var mm := _manager()
	var bird := _mob(mm, "songbird", Vector3(0, 100, 0))
	bird.set_meta("landing", true)   # a landing flight that just arrived
	assert_bool(WildMoves._perch(bird)).is_true()
	assert_str(bird.goal).is_equal("perch")
	assert_bool(WildMoves._perch(bird)).is_true()   # it keeps sitting
	bird._panic_t = 1.0                              # a scare sends it up
	assert_bool(WildMoves._perch(bird)).is_false()
	assert_str(bird.goal).is_equal("fly")
	assert_float(bird.velocity.y).is_greater(0.0)


func test_goats_give_milk_that_refills_when_they_graze() -> void:
	var mm := _manager()
	var gs := GDScript.new()
	gs.source_code = "extends Node\nvar got := []\nfunc spawn_item(s, _p, _v = Vector3.ZERO, _d = 0.0):\n\tgot.append(s)\n"
	gs.reload()
	var stub: Node = auto_free(gs.new())
	mm.main = stub
	var goat := _mob(mm, "goat", Vector3(0, 100, 0))
	var bowl := {"id": "clay_bowl", "tags": ["bowl"]}
	assert_str(mm.interact(goat, bowl)).is_equal("milked")
	assert_str(String(stub.got[0]["id"])).is_equal("bowl_of_milk")
	assert_str(mm.interact(goat, bowl)).is_equal("")   # empty until it grazes
	goat.set_sheared(false)
	assert_str(mm.interact(goat, bowl)).is_equal("milked")
	var items := ItemRegistry.load_default()
	assert_bool(items.has("milk_porridge")).is_true()


func test_night_creatures_keep_away_from_light() -> void:
	var torch := Vector3(0, 100, 0)
	assert_bool(MobManager.near_light([torch], torch + Vector3(Settings.m2v(5.0), 0, 0), MobManager.LIGHT_FADE_M)).is_true()
	assert_bool(MobManager.near_light([torch], torch + Vector3(Settings.m2v(10.0), 0, 0), MobManager.LIGHT_FADE_M)).is_false()
	assert_bool(MobManager.near_light([], torch, MobManager.LIGHT_SAFE_M)).is_false()
