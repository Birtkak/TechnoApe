extends GdUnitTestSuite

## Headless tests for /totest and /scenario (PlaytestCommands, ADR 0018).

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


func test_unticked_takes_newest_open_items_with_their_section() -> void:
	var text := "# To test\n\n## New thing (2026-11-01, ADR 1)\n\n- [ ] First check\n      goes on here.\n- [x] Done one\n- [ ] Second check\n\n## Older (2026-10-01)\n\n- [ ] Third\n"
	assert_array(PlaytestCommands.unticked(text, 8)).is_equal(["New thing: First check goes on here.", "New thing: Second check", "Older: Third"])
	assert_array(PlaytestCommands.unticked(text, 1)).is_equal(["New thing: First check goes on here."])


func test_totest_reads_the_checkout_list() -> void:
	var r := reg.execute("/totest 3")
	assert_bool(r["ok"]).is_true()
	assert_int(r["text"].count("\n- ")).is_less_equal(3)
	assert_bool(reg.execute("/totest zero")["ok"]).is_false()


func test_every_scenario_step_is_a_known_command_or_kit() -> void:
	for name in PlaytestCommands.SCENARIOS:
		for line in PlaytestCommands.lines(name, Vector3i(10, 1000, -10)):
			assert_str(line).not_contains("{")
			var t := CommandRegistry.tokenize(line.trim_prefix("/"))
			assert_bool(reg.find(t[0]).is_empty()).override_failure_message("%s: %s" % [name, line]).is_false()
			if t[0] == "kit":
				assert_bool(GameCommands.KITS.has(t[1])).override_failure_message(line).is_true()


func test_ledge_is_a_platform_under_the_teleport() -> void:
	var l := PlaytestCommands.lines("ledge", Vector3i(10, 1000, -10))
	assert_str(l[1]).is_equal("/fill -14 1024 -34 34 1024 14 stone")
	assert_str(l[2]).is_equal("/tp 10 1026 -10")


func test_scenario_fails_politely() -> void:
	assert_bool(reg.execute("/scenario nope")["ok"]).is_false()
	var r := reg.execute("/scenario river")   # no world: /goto cannot run
	assert_bool(r["ok"]).is_false()
	assert_str(r["text"]).contains("/goto river")
