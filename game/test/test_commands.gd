extends GdUnitTestSuite

## Headless tests for chat commands and game modes (ADR 0018): tokenizing,
## dispatch, aliases, Tab completion, /gamemode, /give, /clear, /kit, the
## game mode rules, pick block and the creative item picker.

var items: ItemRegistry
var recipes: RecipeRegistry
var inv: PlayerInventory
var reg: CommandRegistry
var player: Player
var cmds: GameCommands


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)
	inv = PlayerInventory.new(items, recipes)
	reg = CommandRegistry.new()
	player = auto_free(Player.new())   # not in the tree: only its state is used
	player.inventory = inv
	cmds = GameCommands.new(reg, player, inv, null)


# ---------- registry ----------

func test_tokenize_splits_and_quotes() -> void:
	assert_array(Array(CommandRegistry.tokenize("give  oak_log 64"))).is_equal(["give", "oak_log", "64"])
	assert_array(Array(CommandRegistry.tokenize("say \"hello there\" ape"))).is_equal(["say", "hello there", "ape"])
	assert_array(Array(CommandRegistry.tokenize("   "))).is_empty()


func test_unknown_and_empty_commands_fail_politely() -> void:
	var r := reg.execute("/nope 1 2")
	assert_bool(r["ok"]).is_false()
	assert_str(r["text"]).contains("Unknown command")
	assert_bool(reg.execute("/")["ok"]).is_false()


func test_help_lists_every_command() -> void:
	var r := reg.execute("/help")
	assert_bool(r["ok"]).is_true()
	for n in ["gamemode", "give", "tp", "time", "kill", "kit", "goto", "bug"]:
		assert_str(r["text"]).contains("/" + n)
	assert_str(reg.execute("/help gm")["text"]).contains("/gamemode")


func test_complete_command_names_and_arguments() -> void:
	assert_array(reg.complete("/gam")).is_equal(["/gamemode"])
	assert_array(reg.complete("/gamemode c")).is_equal(["/gamemode creative"])
	assert_array(reg.complete("/gamemode ")).is_equal(["/gamemode creative", "/gamemode survival"])
	assert_array(reg.complete("/give oak_l")).contains(["/give oak_log"])
	assert_array(reg.complete("/time set no")).is_equal(["/time set noon"])
	assert_array(reg.complete("hello")).is_empty()
	assert_array(reg.complete("/kill ")).is_empty()


# ---------- game modes ----------

func test_game_mode_parse_accepts_minecraft_forms() -> void:
	for s in ["1", "c", "creative", "Creative", " CREATIVE "]:
		assert_str(GameMode.parse(s)).is_equal(GameMode.CREATIVE)
	for s in ["0", "s", "survival"]:
		assert_str(GameMode.parse(s)).is_equal(GameMode.SURVIVAL)
	assert_str(GameMode.parse("spectator")).is_equal("")


func test_game_mode_rules() -> void:
	assert_bool(GameMode.rule(GameMode.CREATIVE, "instant_break")).is_true()
	assert_bool(GameMode.rule(GameMode.CREATIVE, "takes_damage")).is_false()
	assert_bool(GameMode.rule(GameMode.CREATIVE, "consumes_items")).is_false()
	assert_bool(GameMode.rule(GameMode.SURVIVAL, "can_fly")).is_false()
	assert_bool(GameMode.rule(GameMode.SURVIVAL, "drops")).is_true()
	assert_bool(GameMode.rule("nonsense", "takes_damage")).is_true()   # unknown = survival


func test_gamemode_command_switches_and_lands() -> void:
	player.flying = true
	var r := reg.execute("/gamemode 1")
	assert_bool(r["ok"]).is_true()
	assert_str(player.game_mode).is_equal(GameMode.CREATIVE)
	assert_bool(player.flying).is_true()   # creative keeps flight
	assert_bool(reg.execute("/gm s")["ok"]).is_true()
	assert_str(player.game_mode).is_equal(GameMode.SURVIVAL)
	assert_bool(player.flying).is_false()  # survival lands the player
	assert_bool(reg.execute("/gamemode spectator")["ok"]).is_false()
	assert_str(player.game_mode).is_equal(GameMode.SURVIVAL)


# ---------- items ----------

func test_give_and_clear() -> void:
	assert_bool(reg.execute("/give oak_log 70")["ok"]).is_true()
	assert_int(inv.inv.count_of("oak_log")).is_equal(70)
	assert_bool(reg.execute("/give flint_pickaxe")["ok"]).is_true()
	assert_int(inv.inv.count_of("flint_pickaxe")).is_equal(1)
	assert_bool(reg.execute("/give unobtainium")["ok"]).is_false()
	assert_bool(reg.execute("/give oak_log lots")["ok"]).is_false()
	assert_bool(reg.execute("/give oak_log 0")["ok"]).is_false()
	var r := reg.execute("/clear oak_log")
	assert_bool(r["ok"]).is_true()
	assert_int(inv.inv.count_of("oak_log")).is_equal(0)
	assert_int(inv.inv.count_of("flint_pickaxe")).is_equal(1)
	assert_bool(reg.execute("/clear")["ok"]).is_true()
	assert_bool(reg.execute("/clear")["ok"]).is_false()   # nothing left


func test_every_kit_item_exists_and_kits_give() -> void:
	for kit in GameCommands.KITS:
		for id in GameCommands.KITS[kit]:
			assert_bool(items.has(id)).override_failure_message("kit %s: unknown item %s" % [kit, id]).is_true()
	assert_bool(reg.execute("/kit tools")["ok"]).is_true()
	assert_int(inv.inv.count_of("iron_pickaxe")).is_equal(1)
	assert_bool(reg.execute("/kit nope")["ok"]).is_false()


func test_commands_without_a_world_fail_instead_of_crashing() -> void:
	for line in ["/tp 0 300 0", "/time set noon", "/spawn", "/goto cave", "/blast 4", "/stations", "/bug test"]:
		if line == "/tp 0 300 0":
			continue   # /tp only needs the player
		assert_bool(reg.execute(line)["ok"]).override_failure_message(line).is_false()


func test_tp_absolute_and_relative() -> void:
	player.position = Vector3(10.5, 300.0, -4.5)
	assert_bool(reg.execute("/tp 2 310 -8")["ok"]).is_true()
	assert_vector(player.position).is_equal(Vector3(2.5, 310.0, -7.5))
	assert_bool(reg.execute("/tp ~ ~10 ~-1")["ok"]).is_true()
	assert_vector(player.position).is_equal(Vector3(2.5, 320.0, -8.5))
	assert_bool(reg.execute("/tp 1 2")["ok"]).is_false()
	assert_bool(reg.execute("/tp a b c")["ok"]).is_false()


func test_kill_and_heal() -> void:
	inv.damage(5)
	assert_bool(reg.execute("/heal")["ok"]).is_true()
	assert_int(inv.health).is_equal(inv.max_health)
	assert_bool(reg.execute("/kill")["ok"]).is_true()
	assert_bool(inv.is_dead()).is_true()


# ---------- pick block / creative picker ----------

func test_item_for_material() -> void:
	assert_str(items.item_for_material("stone")).is_equal("stone")
	assert_str(items.item_for_material("dirt")).is_equal("dirt")
	assert_str(items.item_for_material("grass")).is_equal("grass_block")
	assert_str(items.item_for_material("no_such_material")).is_equal("")


func test_pick_block_creative_and_survival() -> void:
	# creative: conjures a stack into the empty selected slot
	assert_bool(inv.pick_block("dirt", true)).is_true()
	assert_dict(inv.held()).is_equal({"id": "dirt", "count": 64})
	# already in the hotbar: selects it
	inv.select(4)
	assert_bool(inv.pick_block("dirt", true)).is_true()
	assert_int(inv.selected).is_equal(0)
	# survival: nothing carried -> nothing happens
	assert_bool(inv.pick_block("sand", false)).is_false()
	# survival: a stack in the main grid swaps into the hand
	inv.inv.set_slot(20, {"id": "sand", "count": 5})
	assert_bool(inv.pick_block("sand", false)).is_true()
	assert_dict(inv.held()).is_equal({"id": "sand", "count": 5})
	assert_dict(inv.inv.get_slot(20)).is_equal({"id": "dirt", "count": 64})


func test_creative_take_cursor_inventory_and_trash() -> void:
	assert_bool(inv.creative_take("glass", false)).is_true()
	assert_dict(inv.cursor).is_equal({"id": "glass", "count": 64})
	assert_bool(inv.creative_take("stone", false)).is_true()   # a held stack is trashed first
	assert_dict(inv.cursor).is_empty()
	assert_bool(inv.creative_take("stone", true)).is_true()
	assert_int(inv.inv.count_of("stone")).is_equal(64)
	assert_bool(inv.creative_take("nope", true)).is_false()


# ---------- /setblock and /fill ----------

func test_coords_relative_absolute_and_floored() -> void:
	var cur := Vector3(10.4, 20.9, -3.2)
	var p: Variant = cmds._coords(PackedStringArray(["~", "~-1", "~2"]), 0, cur)
	assert_vector(p).is_equal(Vector3(10, 19, -2))
	p = cmds._coords(PackedStringArray(["x", "5", "6", "7"]), 1, cur)
	assert_vector(p).is_equal(Vector3(5, 6, 7))
	# /tp: whole x/z name the voxel's centre, y stays as typed
	p = cmds._coords(PackedStringArray(["5", "6", "7"]), 0, cur, true)
	assert_vector(p).is_equal(Vector3(5.5, 6, 7.5))
	assert_bool(cmds._coords(PackedStringArray(["~", "up", "1"]), 0, cur) is Dictionary).is_true()


func test_material_ids_match_definition_order() -> void:
	assert_int(GameCommands.material_id("air")).is_equal(GameMaterials.AIR)
	assert_int(GameCommands.material_id("STONE")).is_equal(GameMaterials.STONE)
	assert_int(GameCommands.material_id("no_such_thing")).is_equal(-1)
	assert_array(cmds._material_names()).contains(["air", "stone", "water"])


func test_fill_and_setblock_refuse_block_entities_and_bad_input() -> void:
	assert_bool(cmds._placeable("chest") is Dictionary).is_true()
	assert_bool(cmds._placeable("nope") is Dictionary).is_true()
	assert_int(cmds._placeable("sand")).is_equal(GameCommands.material_id("sand"))
	# no world in headless tests: both fail with a message instead of crashing
	assert_bool(reg.execute("/setblock ~ ~ ~ stone")["ok"]).is_false()
	assert_bool(reg.execute("/fill 0 0 0 1 1 1 stone")["ok"]).is_false()
