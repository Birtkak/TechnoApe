extends GdUnitTestSuite

## The progression walk (ADR 0139): from empty hands to the last LV quest over the game data.
## Fails on dead ends only (a quest or a step on the road needs something nothing makes or gives);
## the balance numbers go to docs/plans/progression-walk.md (tools/progression_report.tscn).

const Walk := preload("res://scripts/crafting/progression_walk.gd")

var w
var res: Dictionary


func before() -> void:
	w = Walk.load_default()
	res = w.walk()


func test_no_dead_ends_from_spawn_to_lv() -> void:
	var lines := []
	for d in res["dead"]:
		lines.append("%s needs %s %s" % [d["quest"], d["need"], " > ".join(d["path"])])
	assert_array(lines).override_failure_message("dead ends: " + "; ".join(lines)).is_empty()


## The gates come from the data: tool tiers from mining.json, heat from item tags, power from watts.
func test_steps_are_gated_like_the_game() -> void:
	var gates := []
	for s in w.sources["iron_ore"]:
		if s["what"] == "iron_ore":
			gates = s["gates"]
	assert_array(gates).is_equal(["~pickaxe:2"])
	var smith_iron: Array = w._capital(w.recipes.recipes["smith_iron_pickaxe_head"])
	assert_array(smith_iron).contains(["#hammer", "bronze_anvil", "forge", "@fire"])
	assert_array(w._capital(w.recipes.recipes["extract_resin"])).contains(["extractor", "@dynamo:200", "#cable"])
	assert_str(w._fuel_spec(w.recipes.recipes["bloom_iron_ore"])).is_equal("@coal_fuel")
	assert_bool(w.usable(w.recipes.recipes["melt_copper_ore"], {"copper_ore": true, "crucible": true, "firepit": true})).is_false()   # no fuel, no fire


## The walk plays the chapters in order from empty hands and ends at the canned food.
func test_walk_starts_empty_handed_and_reaches_lv() -> void:
	var chs: Array = res["chapters"]
	assert_str(chs[0]["tools"]["pickaxe"]).is_equal("")
	assert_str(chs[-1]["road"][-1]).is_equal("canned_food")
	for ch in chs:
		assert_float(ch["total_s"]).is_greater(0.0)
	assert_bool(w.owned.has("copper_pickaxe") or w.owned.has("bronze_pickaxe")).is_true()
