extends GdUnitTestSuite

## Sound (ADR 0044): data/sounds.json against the built assets (assets/sounds/index.json), every
## material's sound group, the bursts, animals and loops it names, and the voice pool's rules.

var audio: GameAudio


func before() -> void:
	audio = auto_free(GameAudio.new())
	audio.load_file("res://data/sounds.json")


func test_data_loads() -> void:
	assert_array(audio.errors).is_empty()
	for n in ["player/hurt", "player/eat", "player/splash", "craft/knap", "craft/anvil", "tree/crash",
			"mob/sheep", "mob/pig", "mob/chicken", "mob/bear", "ui/click", "loop/fire_big", "loop/water", "sim/blast"]:
		assert_bool(audio.has_sound(n)).override_failure_message("sound %s missing" % n).is_true()


func test_every_asset_file_loads() -> void:
	var index: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(GameAudio.ROOT + "index.json"))
	assert_int(index.size()).is_greater(80)
	for n in index:
		assert_bool(index[n].is_empty()).override_failure_message("%s has no files" % n).is_false()
		for f in index[n]:
			var st = load(GameAudio.ROOT + f)
			assert_bool(st is AudioStream).override_failure_message("%s does not load (run --import?)" % f).is_true()
			if st is AudioStream and not n.begins_with("music/") and not n.begins_with("loop/"):
				assert_float(st.get_length()).override_failure_message("%s is %.2f s" % [f, st.get_length()]).is_between(0.02, 4.0)


func test_every_material_has_a_group_and_steps() -> void:
	var defs := GameMaterials.defs()
	for i in defs.size():
		var g := audio.group_of(i + 1)
		assert_str(g).override_failure_message("%s has no group" % defs[i]["name"]).is_not_empty()
		if defs[i].get("cell_type", "") in ["solid", "powder"] and g != "none":
			assert_str(audio.material_sound("step", i + 1)).override_failure_message("%s: no step" % defs[i]["name"]).is_not_empty()
			assert_str(audio.material_sound("break", i + 1)).override_failure_message("%s: no break" % defs[i]["name"]).is_not_empty()
			assert_str(audio.material_sound("place", i + 1)).is_not_empty()
	assert_str(audio.group_of(GameMaterials.STONE)).is_equal("stone")
	assert_str(audio.group_of(GameMaterials.WOOD)).is_equal("wood")
	assert_str(audio.material_sound("step", GameMaterials.AIR)).is_empty()


func test_bursts_and_animals_exist() -> void:
	var data: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/sounds.json"))
	var fx: ParticleFx = auto_free(ParticleFx.load_default())
	for b in data["particles"]:
		assert_bool(fx.effects.has(b)).override_failure_message("burst %s is not a particle effect" % b).is_true()
	var mobs: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/mobs.json"))["mobs"]
	for sp in mobs:
		assert_bool(data["mobs"].has(sp)).override_failure_message("mob %s has no sounds" % sp).is_true()
	for pool in ["title", "day", "night"]:
		assert_array(data["music"][pool]).is_not_empty()


func test_errors_are_reported() -> void:
	var bad: GameAudio = auto_free(GameAudio.new())
	bad.load_file("res://data/sounds.json")
	bad.errors.clear()
	bad.load_dict({
		"sounds": {"x": {"asset": "no/such"}, "y": {"asset": "ui/click", "bus": "radio"}},
		"groups": {"rock": {"step": "no/step"}},
		"materials": {"stone": "lava_rock"},
		"loops": {"l": {"sound": "nope", "materials": ["unobtainium"], "where": "inside"}},
	})
	var text := "\n".join(bad.errors)
	assert_str(text).contains("no asset no/such")
	assert_str(text).contains("unknown bus radio")
	assert_str(text).contains("no asset no/step")
	assert_str(text).contains("material stone: no sound group")
	assert_str(text).contains("unknown material unobtainium")
	assert_str(text).contains("unknown where inside")


func test_play_cooldown_and_range() -> void:
	# the autoload, with its voices
	var s: GameAudio = Sound
	if not s.enabled:
		return
	var before_n: int = s.stats_data["played"]
	assert_bool(s.play("player/hurt")).is_true()        # cooldown 0.15 s
	assert_bool(s.play("player/hurt")).is_false()
	assert_bool(s.play("no/such/sound")).is_false()
	assert_int(s.stats_data["played"]).is_equal(before_n + 1)
	assert_int(int(s.stats_data["missing"].get("no/such/sound", 0))).is_greater(0)
