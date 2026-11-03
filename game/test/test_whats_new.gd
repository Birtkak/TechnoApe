extends GdUnitTestSuite

## /whatsnew and /tour (ADR 0130): data/whats_new.json stays runnable, the tour steps through it.

var items: ItemRegistry
var inv: PlayerInventory
var reg: CommandRegistry
var player: Player
var cmds: GameCommands   # held: the registry's callables do not keep it alive


func before_test() -> void:
	items = ItemRegistry.load_default()
	inv = PlayerInventory.new(items, RecipeRegistry.load_default(items))
	reg = CommandRegistry.new()
	player = auto_free(Player.new())   # not in the tree: only its state is used
	player.inventory = inv
	cmds = GameCommands.new(reg, player, inv, null)


func test_every_entry_is_complete_and_unique() -> void:
	var e := PlaytestCommands.whats_new()
	assert_int(e.size()).is_greater(0)
	var ids := {}
	for x in e:
		for k in ["id", "title", "what", "run"]:
			assert_bool(x.has(k)).override_failure_message("%s lacks %s" % [x, k]).is_true()
		assert_str(x["id"]).not_contains(" ")
		assert_bool(ids.has(x["id"])).override_failure_message("two entries are " + x["id"]).is_false()
		ids[x["id"]] = true


## Every line names a registered command, and its kit, station set, animal, material, clothing or place exists.
func test_every_line_runs_a_known_command_with_real_arguments() -> void:
	var mobs: Dictionary = JSON.parse_string(FileAccess.get_file_as_string("res://data/mobs.json"))["mobs"]
	var gen := TerrainGenerator.new()   # seed 1337: every place within 4 km of 0, 0, as /goto searches
	for x in PlaytestCommands.whats_new():
		for line in x["run"]:
			var t := CommandRegistry.tokenize(String(line).trim_prefix("/"))
			var msg := "%s: %s" % [x["id"], line]
			assert_bool(reg.find(t[0]).is_empty()).override_failure_message(msg).is_false()
			match t[0]:
				"kit":
					assert_bool(GameCommands.KITS.has(t[1])).override_failure_message(msg).is_true()
				"stations":
					assert_bool(t.size() == 1 or t[1] in ["electric", "factory", "lv", "power"]).override_failure_message(msg).is_true()
				"summon":
					assert_bool(mobs.has(t[1])).override_failure_message(msg).is_true()
				"blast", "count":
					assert_int(GameCommands.material_id(t[t.size() - 1])).override_failure_message(msg).is_greater(0)
				"wear":
					assert_str(Clothing.slot_of(items.get_def(t[1]))).override_failure_message(msg).is_not_empty()
				"goto", "locate":
					assert_int(gen.gen.find_nearest(t[1], 0, 0, int(Settings.m2v(4000.0))).y).override_failure_message(msg).is_greater_equal(0)


func test_tour_walks_forward_back_and_by_id() -> void:
	var n := PlaytestCommands.whats_new().size()
	var first := reg.execute("/tour")
	assert_bool(first["ok"]).is_true()
	assert_str(first["text"]).starts_with("Tour 1/%d" % n)
	assert_str(reg.execute("/tour")["text"]).starts_with("Tour 2/%d" % n)
	assert_str(reg.execute("/tour back")["text"]).starts_with("Tour 1/%d" % n)
	var last: Dictionary = PlaytestCommands.whats_new()[n - 1]
	var r := reg.execute("/tour " + last["id"])
	assert_str(r["text"]).starts_with("Tour %d/%d: %s" % [n, n, last["title"]])
	assert_str(r["text"]).contains("last stop")
	assert_str(reg.execute("/tour")["text"]).contains("whole tour")   # past the end: starts over
	assert_str(reg.execute("/tour")["text"]).starts_with("Tour 1/")
	assert_bool(reg.execute("/tour nope")["ok"]).is_false()
	assert_bool(reg.execute("/tour 0")["ok"]).is_false()


func test_tour_completes_ids_and_whatsnew_lists_every_entry() -> void:
	assert_array(reg.complete("/tour lo")).contains(["/tour lookout"])
	var r := reg.execute("/whatsnew")
	assert_bool(r["ok"]).is_true()
	for x in PlaytestCommands.whats_new():
		assert_str(r["text"]).contains("/tour " + x["id"])
