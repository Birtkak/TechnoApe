extends GdUnitTestSuite

## Metal tiers (ADR 0121): the steel tool set, bronze/iron/steel armour and metal spears.

var items: ItemRegistry
var recipes: RecipeRegistry


func before() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


func _made(id: String) -> bool:
	for r in recipes.recipes.values():
		if String(r.get("output", {}).get("id", "")) == id:
			return true
	return false


func test_steel_tools_beat_iron() -> void:
	for t in ["knife", "axe", "shovel", "hoe", "hammer", "saw"]:
		var s := items.get_def("steel_" + t)
		var i := items.get_def("iron_" + t)
		assert_int(int(s.get("tier", 0))).override_failure_message("steel_" + t).is_equal(4)
		assert_int(int(s["durability"])).is_greater(int(i["durability"]))
		assert_array(s["tags"]).contains([t])   # the tool class every recipe and mining rule asks for
		assert_bool(_made("steel_" + t)).is_true()
	for h in ["knife_head", "axe_head", "shovel_head", "hoe_head", "hammer_head", "saw_blade"]:
		assert_bool(_made("steel_" + h)).override_failure_message("steel_" + h).is_true()


func test_armour_rises_with_the_metal() -> void:
	for piece in ["helmet", "chestplate", "boots"]:
		var last := 0
		for m in ["copper", "bronze", "iron", "steel"]:
			var d := items.get_def("%s_%s" % [m, piece])
			var a := int(d["clothing"]["armor"])
			assert_int(a).override_failure_message("%s_%s" % [m, piece]).is_greater_equal(last)
			last = a
			assert_bool(_made("%s_%s" % [m, piece])).is_true()


func test_metal_spears() -> void:
	var last := int(items.get_def("flint_spear")["attack_damage"])
	for m in ["bronze", "iron", "steel"]:
		var d := items.get_def(m + "_spear")
		assert_int(int(d["attack_damage"])).is_greater(last)
		last = int(d["attack_damage"])
		assert_bool(d.has("throw")).is_true()
		assert_bool(_made(m + "_spear")).is_true()
