extends GdUnitTestSuite

## data/liquid_looks.json (bug report 2026-10-25, every liquid gets water's treatment): every moving
## liquid has a look, and every key in it is a uniform the fluid shaders know (a typo would be dropped
## silently by set_shader_parameter).


func test_every_liquid_has_a_look() -> void:
	var looks := World.liquid_looks()
	for d in GameMaterials.defs():
		if d.has("fluid_slot") and not d.get("liquid_static", false):
			assert_bool(looks.has(d["name"])).override_failure_message("no look for liquid " + d["name"]).is_true()


func test_look_keys_are_shader_uniforms() -> void:
	var names := {}
	for path in ["res://assets/shaders/terrain_fluid.gdshader", "res://assets/shaders/terrain_fluid_opaque.gdshader"]:
		var sh: Shader = load(path)
		for u in sh.get_shader_uniform_list():
			names[u["name"]] = true
	assert_bool(names.has("liquid_shallow")).is_true()
	var looks := World.liquid_looks()
	for liquid in looks:
		for k in looks[liquid]:
			assert_bool(names.has(k)).override_failure_message("%s: %s is no fluid shader uniform" % [liquid, k]).is_true()


## ADR 0119 C4: every gas has a look in data/gas_looks.json and every key is a gas shader uniform.
func test_every_gas_has_a_look_and_its_keys_are_uniforms() -> void:
	var looks := World.gas_looks()
	var names := {}
	var sh: Shader = load("res://assets/shaders/terrain_gas.gdshader")
	for u in sh.get_shader_uniform_list():
		names[u["name"]] = true
	assert_bool(names.has("gas_tint")).is_true()
	for d in GameMaterials.defs():
		if d.get("cell_type", "") == "gas":
			assert_bool(looks.has(d["name"])).override_failure_message("no look for gas " + d["name"]).is_true()
	for gas in looks:
		for k in looks[gas]:
			assert_bool(names.has(k)).override_failure_message("%s: %s is no gas shader uniform" % [gas, k]).is_true()


func test_colours_become_colours() -> void:
	var fm := ShaderMaterial.new()
	fm.shader = load("res://assets/shaders/terrain_fluid.gdshader")
	World.apply_liquid_look(fm, {"liquid_deep": [0.25, 0.5, 1.0], "glow": 1})
	assert_object(fm.get_shader_parameter("liquid_deep")).is_equal(Color(0.25, 0.5, 1.0))
	assert_float(fm.get_shader_parameter("glow")).is_equal(1.0)
