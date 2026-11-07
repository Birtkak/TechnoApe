class_name GameCommands
extends RefCounted

## The commands a player can type in chat (ADR 0018). Holds the game objects
## the handlers touch; any of them may be null (headless tests), and a
## command that needs a missing one fails with a message instead of crashing.
## Coordinates are voxels, as F3 shows them ("voxel ..."); one voxel = 12.5 cm (ADR 0033).

## Minecraft's named times on our clock (0 midnight, 0.25 sunrise, 0.5 noon).
const TIMES := {"sunrise": 0.25, "day": 0.29, "noon": 0.5, "sunset": 0.75, "night": 0.79, "midnight": 0.0}

## Playtest kits (/kit): item id -> count, in items or, for voxel items, blocks (BLOCK^3 voxels; until
## 2026-10-25 they counted voxels, so "stone": 64 gave one block). test_commands.gd checks every id exists.
const KITS := {
	"tools": {"iron_pickaxe": 1, "copper_pickaxe": 1, "copper_axe": 1, "copper_shovel": 1, "copper_hoe": 1, "copper_knife": 1,
		"copper_saw": 1, "copper_hammer": 1, "shears": 1, "flint_and_steel": 1, "pan": 1, "torch": 32, "coal": 32, "oak_log": 16},
	"builder": {"deconstruction_hammer": 1, "stone": 64, "cobblestone": 64, "bricks": 64, "dirt": 64, "grass_block": 64, "sand": 64, "gravel": 64,
		"oak_planks": 64, "oak_log": 64, "birch_log": 64, "spruce_log": 64, "glass": 64, "sandstone": 64, "terracotta": 64,
		"basalt": 64, "clay": 64, "thatch": 64, "wattle": 64, "daub": 64, "torch": 64,
		"blueprint": 4, "builder": 1, "hand_crank": 1, "chest": 1},   # ADR 0161
	"house": {"door": 4, "trapdoor": 4, "fence": 16, "fence_gate": 4, "ladder": 16, "window": 8, "table": 2, "shelf": 2,
		"lantern": 8, "flower_pot": 4, "painting": 4, "chair": 4, "bed": 1, "carpet": 16},   # ADR 0094
	"stations": {"crafting_table": 4, "furnace": 4, "chest": 4, "storage_pot": 2, "firepit": 2, "sieve": 2, "stone_anvil": 1,
		"cooking_pot": 1, "crucible": 1, "ingot_mold": 1, "bed": 1, "iron_ore": 16, "copper_ore": 16, "tin_ore": 16, "coal": 32},
	"farm": {"iron_hoe": 1, "wheat_seeds": 32, "carrot": 16, "wheat": 32, "pumpkin": 4, "blueberry": 16, "egg": 8, "bread": 8,
		"water_bucket": 1, "shears": 1, "bed": 1, "chair": 4, "wool": 16, "carpet": 16},
	"stone": {"flint": 8, "stick": 16, "clay_ball": 16, "dry_grass": 12, "plant_fiber": 8, "loose_stone": 4, "loose_copper_ore": 8,
		"firestarter": 1, "oak_log": 8, "cordage": 6},   # Vintage Story early game (ADR 0023)
	"flint": {"flint_knife": 1, "flint_shovel": 1, "flint_spear": 1, "atlatl": 1, "bow": 1, "arrow": 16, "firestarter": 1, "firepit": 1,
		"birch_tar": 8, "tempered_flint": 8, "bone_needle": 1, "fur_pelt": 6, "cordage": 16, "drying_rack": 1, "raw_meat": 4,
		"axe_roughout": 1, "sand": 1, "dirt": 2, "loose_stone": 8, "stick": 32, "dry_grass": 24, "einkorn": 8, "flax": 8,
		"linen_thread": 8, "loom": 1, "flint_adze": 1, "dugout_canoe": 1, "lever": 1, "wooden_wedge": 12, "oak_log": 2,
		"red_paint": 1, "fat_lamp": 1, "basket_trap": 1, "tinder_fungus": 4},   # the four Stone Age stages (ADR 0059)
	"smith": {"stone_anvil": 1, "firepit": 1, "copper_hammer": 1, "copper_ingot": 6, "oak_log": 8, "firestarter": 1, "crucible": 1,
		"ingot_mold": 1, "loose_copper_ore": 16, "flint_axe_head": 1, "copper_saw_blade": 1, "stick": 8, "cordage": 4},   # crafting in the world (ADR 0042)
	"bronze": {"firepit": 1, "crucible_copper": 1, "tin_nugget": 8, "loose_tin_ore": 4, "axe_mold": 1, "pickaxe_mold": 1, "ingot_mold": 1,
		"bronze_ingot": 8, "stone_anvil": 1, "copper_hammer": 1, "firestarter": 1, "firewood": 16, "stick": 8},   # the bronze age (ADR 0070)
	"iron": {"bloomery": 1, "forge": 1, "bronze_anvil": 1, "bronze_hammer": 1, "charcoal": 32, "iron_nugget": 16, "loose_iron_ore": 8,
		"iron_ingot": 6, "iron_bloom": 1, "firestarter": 1, "stick": 8},   # the iron age (ADR 0070)
	"steam": {"small_boiler": 1, "bronze_pipe": 8, "copper_pipe": 8, "steam_hammer": 1, "steam_sieve": 1, "forge": 1, "coal": 32, "water_bucket": 1,
		"iron_ingot": 8, "iron_plate": 4, "gravel": 16, "firestarter": 1, "wrench": 1, "steel_boiler": 1, "blast_furnace": 1, "ore_crusher": 1,
		"water_tank": 1, "steam_turbine": 1, "iron_ore": 8, "fire_brick": 4, "steam_drill": 1, "surveyors_stake": 4, "compressor": 1,
		"hopper": 4, "chute": 4, "belt": 16, "chest": 2},   # the steam age (ADR 0097, 0099)
	"gear": {"goggles": 1, "leather_apron": 1, "miners_helmet": 1, "travelers_backpack": 1, "boiler_suit": 1, "jetpack": 1,
		"rail": 32, "minecart": 1, "chest_cart": 1},   # steam age gear and the ore train (ADR 0099)
	"magic": {"wand_basic": 1, "spell_spark_bolt": 1, "spell_fire_bolt": 1, "spell_speed_up": 1, "mana_dust": 64, "fire_essence": 16},
	"electric": {"small_boiler": 1, "bronze_pipe": 4, "steam_turbine": 1, "water_tank": 1, "water_bucket": 1, "coal": 32, "firestarter": 1,
		"copper_cable": 16, "thin_cable": 8, "heavy_copper_cable": 8, "insulated_wire": 16, "battery_box": 1, "lead_acid_cell": 4,
		"water_wheel": 1, "wind_turbine": 1, "hand_crank": 1, "electric_lamp": 4, "daylight_switch": 1, "meter_panel": 1, "multimeter": 1,
		"knife_switch": 1, "fuse_box": 1, "breaker": 1, "fuse": 4, "wrench": 1, "electric_furnace": 1, "pump": 1, "raw_meat": 8, "iron_ore": 8, "vent_fan": 2, "safety_lamp": 1},   # the energy network (ADR 0113, 0117, 0123)
	"contraptions": {"lift_winch": 1, "drawbridge_hinge": 1, "crane": 1, "hand_crank": 3, "knife_switch": 2, "copper_cable": 16, "oak_planks": 64},   # ADR 0160
	"lv": {"lathe": 1, "bending_press": 1, "wiremill": 1, "macerator": 1, "alloy_smelter": 1, "assembler": 1, "extractor": 1, "cutter": 1,
		"extruder": 1, "canning_machine": 1, "electric_compressor": 1, "electrolyzer": 1, "coke_oven": 1, "bricks": 26, "coal_coke": 8,
		"hopper": 2, "belt": 8, "iron_ingot": 8, "copper_ingot": 8, "tin_ingot": 8, "steel_ingot": 4, "tin_nugget": 12, "copper_ore": 4,
		"crushed_copper_ore": 8, "primitive_circuit": 4, "machine_hull": 2, "electric_motor": 2, "rubber": 16, "vacuum_tube": 4,
		"resistor": 2, "coated_board": 2, "glass": 4, "oak_log": 8},   # the LV machines and what they work (ADR 0117, 0131); power from /kit electric
	"workshop": {"spyglass": 1, "diving_mask": 1, "rubber_boots": 1, "hand_drill": 1, "canned_meat": 4, "canned_fish": 4, "canned_vegetables": 4,
		"sticky_resin": 16, "tin_can": 4, "stick": 8},   # the Workshop chapter's LV round 2 goods (ADR 0131)
	"social": {"sign": 4, "paper": 8, "charcoal": 4, "note": 1, "trading_post": 1, "iron_ingot": 16, "oak_planks": 32, "cobblestone": 32, "chest": 1},   # signs, notes, a trading post (ADR 0164)
	"gadgets": {"laser_cutter": 1, "crater_rod": 1, "instant_tree": 1, "tape_measure": 1, "world_vacuum": 1, "sounding_rod": 1,
		"water_balloon": 16, "sand_bomb": 16, "lava_bomb": 16},   # PlaytestTools
	"toys": {"glider": 1, "climbing_rope": 8, "grappling_hook": 1},   # movement toys (ADR 0151)
	"water": {"bucket": 1, "water_bucket": 1, "crater_rod": 1, "laser_cutter": 1, "lava_bomb": 8, "water_balloon": 8, "cobblestone": 16},   # river tests (ADR 0104)
	"tnt": {"tnt": 64, "fuse_cord": 16, "igniter": 1, "flint_and_steel": 1, "sulfur": 8, "charcoal": 4, "sand": 2},   # TNT (ADR 0065)
	"fire": {"flint_and_steel": 1, "firestarter": 1, "firepit": 1, "firewood": 32, "coal": 16, "oak_log": 16, "dry_grass": 16,
		"torch": 16, "water_bucket": 1, "water_balloon": 16},   # forest fires and rain (ADR 0056)
	"clutter": {"seashell": 16, "kelp": 16, "brown_mushroom": 16, "red_mushroom": 16, "dandelion": 16, "poppy": 16, "cornflower": 16,
		"oxeye_daisy": 16, "allium": 16, "stick": 16, "flint": 16, "loose_stone": 16},   # ground pieces (ADR 0047)
}

## What WorldGen.find_nearest can look for (biome names work too).
const FEATURES := ["cave", "deep_cave", "ruin", "ruin_house", "ruin_watchtower", "ruin_stone_circle", "ruin_crypt", "crypt_room", "ruin_mud_house", "ruin_shrine", "ruin_wreck", "ruin_mine", "ruin_camp", "ruin_lookout", "ruin_igloo", "ruin_jungle_temple", "ruin_tomb", "bridge", "cairn", "volcano", "rock_arch", "hoodoo", "waterfall", "lake", "river", "spring", "marsh", "alpine", "snow", "mountain", "forest", "old_growth_forest", "taiga", "jungle", "plains", "desert",
	"coal_vein", "copper_vein", "cassiterite_vein", "iron_vein", "copper_iron_vein",
	"sulfur_cave", "lush_cave", "magma_cave", "crystal_cave", "fungal_cave", "aquifer", "magma_conduit"]

## /goto and /locate search 4 km, or as far as this when asked (/goto ruin_igloo 16).
const FIND_MAX_KM := 16.0

## /fill's limit: 64^3, about one bubble chunk column's worth of remeshing.
const FILL_MAX := 262144

var reg: CommandRegistry
var player: Player
var player_inv: PlayerInventory
var main: Node   # Main: atmosphere, spawn_item, spawn position
var _cam: DebugCamera   # /cam, made on first use
var sky: SkyCommands   # /date, /moon, /eclipse, /constellations (ADR 0025)
var playtest := PlaytestCommands.new()   # /totest, /scenario


func _init(p_reg: CommandRegistry, p_player: Player, p_inv: PlayerInventory, p_main: Node = null) -> void:
	reg = p_reg
	player = p_player
	player_inv = p_inv
	main = p_main
	reg.register("gamemode", "/gamemode <survival|creative>", "Switch game mode (0, 1, s and c work too)",
		_gamemode, [GameMode.names()], ["gm"])
	reg.register("noclip", "/noclip [on|off]", "Fly through everything (creative or spectator); toggles without an argument", _noclip, [["on", "off"]])
	reg.register("give", "/give <item> [count]", "Give yourself items", _give, [_item_ids])
	reg.register("clear", "/clear [item]", "Empty your inventory, or remove one item", _clear, [_item_ids])
	reg.register("drop", "/drop <item> [count]", "Drop a stack at your feet, from nothing (playtests: a shared item in multiplayer)", _drop, [_item_ids])
	reg.register("tp", "/tp <x> <y> <z>", "Teleport to a voxel position; ~ is relative (~ ~10 ~)",
		_tp, [["~"], ["~"], ["~"]], ["teleport"])
	reg.register("time", "/time set <day|noon|sunset|night|midnight|sunrise|0..1>", "Set or show (/time query) the time of day",
		_time, [["set", "query"], TIMES.keys()])
	reg.register("fullbright", "/fullbright [on|off]", "Testing light: flat white ambient and no fog, so caves show their shape", _fullbright, [["on", "off"]])
	reg.register("kill", "/kill", "Die and respawn at the world spawn", _kill)
	reg.register("heal", "/heal", "Restore full health", _heal)
	reg.register("stain", "/stain <wet|bloody|oily|toxic|burning|clear>", "Coat the ape like touching that liquid (Noita stains)", _stain, [PlayerStains.KINDS + ["clear"]])
	reg.register("seed", "/seed", "Show the world seed", _seed)
	reg.register("wear", "/wear <item>", "Put on clothing or armour (as a right click with it would); what was in that slot is gone", _wear, [_clothing_ids])
	reg.register("kit", "/kit <%s>" % "|".join(KITS.keys()), "Give a playtest kit", _kit, [KITS.keys()])
	reg.register("spawn", "/spawn", "Teleport to the world spawn", _spawn)
	reg.register("sethome", "/sethome", "Mark where you stand as home (a green waypoint; /home goes there)", _sethome)
	reg.register("home", "/home", "Teleport to your home (/sethome marks it)", _home)
	reg.register("lightmap", "/lightmap [on|off]", "The light overlay (F7): red marks always dark, yellow dark at night", _lightmap, [["on", "off"]])
	reg.register("locate", "/locate <feature> [km]", "Find the nearest cave, ruin, lake, biome ... (within 4 km, or km up to %d)" % int(FIND_MAX_KM), _locate, [FEATURES])
	reg.register("goto", "/goto <feature> [km]", "Teleport to the nearest cave, ruin, lake, biome ... (within 4 km, or km up to %d)" % int(FIND_MAX_KM), _goto, [FEATURES])
	reg.register("daycycle", "/daycycle <on|off|seconds>", "Run the sun (on = 20 min days) or freeze it", _daycycle, [["on", "off"]])
	reg.register("blast", "/blast <radius_m> [material]", "Carve a crater where you look, or drop a ball of a material (water, sand, lava ...)",
		_blast, [["1", "2", "4", "8"], _material_names])
	reg.register("brush", "/brush <shape> [size] | hold <seconds>", "Set the dig/build brush (ADR 0065): cube, sphere, cylinder, flat, column; size in voxels (1-8, creative 16, 32); hold = keep the break button down (scripted checks)",
		_brush, [Brush.SHAPES + ["hold"], ["1", "2", "4", "8", "16", "32"]])
	reg.register("book", "/book [chapter]", "Open the progression book, at a chapter if given", _book_cmd, [_chapter_ids])
	reg.register("browse", "/browse <item> [recipes|uses]", "Open the recipe browser on an item (as R or U over it)", _browse, [_item_ids, ["recipes", "uses"]])
	reg.register("digonly", "/digonly <material> | off", "Dig only this material, as Shift + middle click on it (ADR 0144); again to take it off the list", _digonly)
	reg.register("tools", "/tools", "Open the build ring (the V key): build modes, prefabs and, under Brush, the brush's shapes and size", _tools)
	reg.register("tnt", "/tnt <grid [n] [spacing_m]|fuse [length_m]|light|stats>", "TNT (ADR 0065): grid lays n blocks on the ground where you look and lights the nearest (a chain); fuse lays a charge there with a lit fuse cord toward you (ADR 0150); light lights the aimed charge",
		_tnt, [["grid", "light", "stats"], ["1", "5", "20"], ["1", "1.5", "2"]])
	reg.register("meteor", "/meteor [here|near|stats]", "A meteorite falls (ADR 0157): here 6 m ahead, near 120 m ahead, none a few hundred metres off as at night",
		_meteor, [["here", "near", "stats"]])
	reg.register("support", "/support [stats|scan|probe|modes <all|survival>|on|off]", "Mine shoring (ADR 0142): what creaks and when it falls, scan the roof here, the aimed roof's load, which game modes cave in",
		_support, [["stats", "scan", "probe", "modes", "on", "off"], ["all", "survival"]])
	reg.register("setblock", "/setblock <x> <y> <z> <material>", "Set one voxel; ~ is relative to you (~ ~-1 ~ is under your feet)",
		_setblock, [["~"], ["~"], ["~"], _material_names])
	reg.register("fill", "/fill <x1> <y1> <z1> <x2> <y2> <z2> <material>", "Fill a box (corners included, up to %d voxels) inside the sim bubble" % FILL_MAX,
		_fill, [["~"], ["~"], ["~"], ["~"], ["~"], ["~"], _material_names])
	reg.register("ponder", "/ponder [scene]", "Open the guide screen (ADR 0132): how boilers, pipes, cables and machines connect; no scene lists them", _ponder, [Ponder.scenes().keys()])
	reg.register("stations", "/stations [electric|factory|lv|power|dam [cam]|pollution|smoke|fx [n]]","Place a crafting table, chest and furnace ahead; electric: a boiler, turbine, cables and an electric furnace; factory: that plus hopper-fed machines, a battery, a lamp; lv: the electric line and the ADR 0131 machines; power: the small sources, meters and switches (ADR 0113, 0117, 0123, 0131); dam: a pool, a water wheel and a weak dirt dam that breaks (ADR 0154); pollution: a fouled stream beside a settling pond, smoke: a fire in a dug room (ADR 0141); fx: 30 running machines of every animated kind (ADR 0153)", _stations, [["electric", "factory", "lv", "power", "dam", "pollution", "smoke", "fx"]])
	reg.register("fish", "/fish <pond|foul|traps|cast|state>", "ADR 0163: a fish pond (or a fouled one) 2 m ahead with a rod in hand; traps: a snare, a pit trap, a rabbit, a deer and a tamed wolf; cast: an automatic cast, bite and reel down ahead; state: the rod's state", _fish, [["pond", "foul", "traps", "cast", "state"]])
	reg.register("contraptions", "/contraptions [toggle|ride] [cam]", "ADR 0160: a lift, a drawbridge and a crane ahead, each on a turning hand crank; toggle flips every drive's lever, ride stands you on the lift; cam films them", _contraptions, [["toggle", "ride", "cam"]])
	reg.register("pollute", "/pollute [smoke]", "ADR 0155: a fouled pond with fish and wheat beside waste ground ahead (farm_speed=40 to watch crops wilt); smoke: a smoke booth round you", _pollute, [["smoke"]])
	reg.register("summon", "/summon <sheep|pig|chicken|bear> [count] [baby|hungry]", "Spawn animals where you look (ADR 0022); hungry predators hunt at once (ADR 0027)",
		_summon, [["sheep", "pig", "chicken", "bear"], ["1", "2", "4", "8"], ["baby", "hungry"]])
	reg.register("name", "/name <text>", "Name the companion you look at (ADR 0152); the tag over it shows the name", _name)
	reg.register("tame", "/tame", "Tame the wolf you look at (or the nearest within 12 m) as your companion (ADR 0152); empty hand toggles sit/stay", _tame)
	reg.register("quests", "/quests [list|complete <id|all>|reset]", "Show or change progression book progress (ADR 0028)",
		_quests, [["list", "complete", "reset"], _quest_ids])
	reg.register("grade", "/grade [%s]" % "|".join(Atmosphere.GRADES.keys()), "Switch the lighting grade (ADR 0041), or show the current one", _grade, [Atmosphere.GRADES.keys()])
	reg.register("shot", "/shot <name> [hud]", "Save the view as out/shots/<name>.png, without the HUD unless hud is given", _shot)
	reg.register("showcase", "/showcase <name> [<name> ...] [on=<material>]", "Set materials or items side by side on a pad above you and film them into out/showcase/showcase.png", _showcase)
	reg.register("cam", "/cam [off | <x> <y> <z> [<look_x> <look_y> <look_z>]]", "Free camera: flies anywhere while you, the terrain detail and the simulation stay put", _cam_cmd)
	reg.register("survival", "/survival [hunger <0..20> | body <C> | spoil <0..1>]", "Hunger, body temperature and the air around you (ADR 0040); spoil sets the held food's freshness", _survival)
	reg.register("shape", "/shape <box|wall|cylinder|cone|sphere|stairs|slope> [w h d] [hollow n] [at x y z] | commit | cancel | turn [tip] | mirror", "Shape building (V in game): set a shape down at the crosshair, then commit", _shape)
	reg.register("box", "/box <build|dig|select> <x1> <y1> <z1> <x2> <y2> <z2> | go", "Box mode by coordinates (V in game); go digs a marked box at once", _box)
	reg.register("blueprint", "/blueprint copy x1 y1 z1 x2 y2 z2 | place [at x y z] [turn 0-3] [mirror] | list | clear",
		"Copy a box onto a written blueprint, place the held one (a ghost, or at once with at), list or clear the sites (ADR 0161)", _blueprint)
	reg.register("prefab", "/prefab save <category> <name> | place <category> <name> [at x y z] | list", "Save the selected box as a prefab, place one, or list them", _prefab)
	reg.register("chop", "/chop [cam]", "Chop through the nearest big trunk 8-20 m away like a player would: 1 m brush digs at 1.5 m up, one every 0.3 s", _chop)
	reg.register("weather", "/weather [clear|rain|storm] [seconds]", "Show or set the weather (rain turns to snow below 0 C; a storm has lightning); a forest fire brings rain on its own", _weather, [["clear", "rain", "storm"]])
	reg.register("lightning", "/lightning [here]", "A lightning strike on the highest exposed point around you (or a rod), or at your feet with 'here'", _lightning, [["here"]])
	reg.register("erupt", "/erupt", "The nearest volcano erupts: rumble and ash, then lava over the rim (stand within 40 m: /goto volcano)", _erupt)
	reg.register("waypoint", "/waypoint [add <name>|remove <name>|list]", "Waypoints (also on the map, M): add one where you stand", _waypoint, [["add", "remove", "list"]], ["wp"])
	reg.register("map", "/map", "Open the full map", _map)
	reg.register("ignite", "/ignite", "Set what you look at on fire, as flint and steel would (up to the laser's range)", _ignite)
	reg.register("gadget", "/gadget <item> [sneak]", "Use a playtest gadget where you look, as a right click would (/kit gadgets gives them all)", _gadget, [_gadget_ids, ["sneak"]])
	reg.register("use", "/use [aim|close]", "Right click with the held item where you look; aim shows the placement ghost in scripted runs, close shuts an open screen", _use_cmd, [["aim", "close"]])
	reg.register("laser", "/laser [seconds] [sweep_deg]", "Fire the laser cutter where you look for a while (default 1.5 s), turning across sweep_deg (default 20)", _laser)
	reg.register("fell", "/fell [direction_deg|pillar] [cam]", "Fell the thickest tree 8-20 m away, across your view", _fell)
	reg.register("bug", "/bug <what went wrong>", "Save a bug report (screenshot, state, log) for Claude to read", _bug)
	reg.register("sound", "/sound <name> [x y z] | list | stats", "Play a sound (ADR 0044), list them, or show the sound stats", _sound, [Sound.sound_names() + ["list", "stats"]])
	reg.register("music", "/music [next|stop|<track>]", "Show, skip to the next, stop or pick a music track", _music, [["next", "stop"]])
	playtest.register(reg, player, get.bind("main"))   # main is set after construction; a method callable keeps no cycle
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo != null:
		sky = SkyCommands.new(atmo)
		sky.register(reg)


func _item_ids() -> Array:
	if player_inv == null:
		return []
	var ids := player_inv.items.all_ids()
	ids.sort()
	return ids


func _gamemode(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if args.is_empty():
		return CommandRegistry.ok("Game mode: %s" % player.game_mode)
	var mode := GameMode.parse(args[0])
	if mode == "":
		return CommandRegistry.fail("Unknown game mode: %s (survival, creative, spectator)" % args[0])
	player.set_game_mode(mode)
	return CommandRegistry.ok("Set own game mode to %s Mode" % mode.capitalize())


## /noclip: flight that passes through voxels, for looking around inside the world (debugging).
func _noclip(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if not GameMode.rule(player.game_mode, "can_fly"):
		return CommandRegistry.fail("Noclip needs creative or spectator mode")
	var on: bool = not player.noclip if args.is_empty() else args[0] in ["on", "1", "true"]
	player.noclip = on
	if on:
		player.flying = true
	elif GameMode.rule(player.game_mode, "noclip"):
		player.noclip = true
		return CommandRegistry.fail("Spectators always pass through")
	return CommandRegistry.ok("Noclip %s" % ("on" if on else "off"))


func _drop(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not player_inv.items.has(args[0].to_lower()):
		return CommandRegistry.fail("Usage: /drop <item> [count]")
	var count := int(args[1]) if args.size() > 1 and args[1].is_valid_int() else 1
	Net.record(func() -> void:
		main.spawn_item({"id": args[0].to_lower(), "count": maxi(1, count)}, player.position + Vector3(0, Settings.m2v(0.5), 0)))
	return CommandRegistry.ok("Dropped %d %s" % [maxi(1, count), args[0].to_lower()])


func _give(args: PackedStringArray) -> Dictionary:
	if player_inv == null:
		return CommandRegistry.fail("No inventory")
	if args.is_empty():
		return CommandRegistry.fail("Usage: /give <item> [count]")
	var id := args[0].to_lower()
	if not player_inv.items.has(id):
		return CommandRegistry.fail("Unknown item: %s" % args[0])
	var count := 1
	if args.size() > 1:
		if not args[1].is_valid_int() or int(args[1]) < 1:
			return CommandRegistry.fail("Count must be a whole number of 1 or more: %s" % args[1])
		count = mini(int(args[1]), PlayerInventory.SIZE * player_inv.items.max_stack(id))
	var left := player_inv.pickup(id, count)
	if left > 0 and main != null and player != null:
		# inventory full: the rest lands at the feet, as vanilla /give does
		main.spawn_item({"id": id, "count": left}, player.position + Vector3(0, 1, 0))
	return CommandRegistry.ok("Gave %d [%s]" % [count, player_inv.items.display_name(id)])


func _clear(args: PackedStringArray) -> Dictionary:
	if player_inv == null:
		return CommandRegistry.fail("No inventory")
	var only := args[0].to_lower() if args.size() > 0 else ""
	if only != "" and not player_inv.items.has(only):
		return CommandRegistry.fail("Unknown item: %s" % args[0])
	var removed := 0
	for i in PlayerInventory.SIZE:
		var s: Dictionary = player_inv.inv.get_slot(i)
		if s.is_empty() or (only != "" and s["id"] != only):
			continue
		removed += int(s["count"])
		player_inv.inv.set_slot(i, {})
	if removed == 0:
		return CommandRegistry.fail("No items were found")
	return CommandRegistry.ok("Removed %d item(s)" % removed)


func _tp(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if args.size() != 3:
		return CommandRegistry.fail("Usage: /tp <x> <y> <z>")
	var p: Variant = _coords(args, 0, player.position, true)
	if p is Dictionary:
		return p
	var out: Vector3 = p
	out.y = clampf(out.y, 0.0, float(Settings.WORLD_HEIGHT_VOXELS))
	player.teleport(out)
	return CommandRegistry.ok("Teleported to %.1f, %.1f, %.1f" % [out.x, out.y, out.z])


## Three coordinates from args[start..start+2] as a Vector3, or a fail() result.
## `~` / `~n` are relative to `cur`. centre: whole x/z name the voxel's centre (/tp);
## without it the result is floored to the voxel that holds it (/setblock, /fill).
func _coords(args: PackedStringArray, start: int, cur: Vector3, centre := false) -> Variant:
	var out := Vector3.ZERO
	for i in 3:
		var a := args[start + i]
		var rel := a.begins_with("~")
		var num := a.substr(1) if rel else a
		if num != "" and not num.is_valid_float():
			return CommandRegistry.fail("Not a coordinate: %s" % a)
		var v := float(num) if num != "" else 0.0
		if rel:
			out[i] = cur[i] + v
		else:
			out[i] = v + (0.5 if centre and i != 1 and num.is_valid_int() else 0.0)
	return out if centre else out.floor()


## Material id for a GameMaterials name (ids are 1 + definition index, 0 = air), or -1.
static func material_id(name: String) -> int:
	if name.to_lower() == "air":
		return GameMaterials.AIR
	var defs := GameMaterials.defs()
	for i in defs.size():
		if defs[i]["name"] == name.to_lower():
			return i + 1
	return -1


## Material id for /setblock and /fill, or a fail() result. Block-entity voxels
## (chest, furnace ...) are refused: the voxel alone would have no entity behind it.
func _placeable(name: String) -> Variant:
	var m := material_id(name)
	if m < 0:
		return CommandRegistry.fail("Unknown material: %s" % name)
	if m != GameMaterials.AIR and GameMaterials.defs()[m - 1].has("entity"):
		return CommandRegistry.fail("%s is a block entity: place its item or use /stations" % name)
	return m


## World edits typed before the bubble has loaded (start commands, the F4 menu's
## "extra commands") wait for it, then report in chat; ~ resolves at that point.
func _queue_until_loaded(cb: Callable, args: PackedStringArray) -> Dictionary:
	_run_when_loaded(cb, args)
	return CommandRegistry.ok("Waiting for the world to load ...")


func _run_when_loaded(cb: Callable, args: PackedStringArray) -> void:
	var tree: SceneTree = main.get_tree()
	while not main.bubble.is_loaded():
		await tree.process_frame
	var r: Dictionary = cb.call(args)
	var chat: Node = main.get("chat")
	if chat != null:
		chat.add_message(r["text"], ChatBox.FEEDBACK if r["ok"] else ChatBox.ERROR)


func _setblock(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.size() != 4:
		return CommandRegistry.fail("Usage: /setblock <x> <y> <z> <material>")
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_setblock, args)
	var p: Variant = _coords(args, 0, player.position)
	if p is Dictionary:
		return p
	var m: Variant = _placeable(args[3])
	if m is Dictionary:
		return m
	var wp := Vector3i(p)
	Net.recording = true
	var set_ok: bool = main.bubble.set_voxel(wp, m)
	Net.recording = false
	if not set_ok:
		if not main.world.is_editable(wp):
			return CommandRegistry.fail("%d, %d, %d is not loaded" % [wp.x, wp.y, wp.z])
		main.world.set_voxel(wp, m)   # outside the bubble: straight to the frozen terrain
	return CommandRegistry.ok("Set %d, %d, %d to %s" % [wp.x, wp.y, wp.z, args[3].to_lower()])


func _fill(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.size() != 7:
		return CommandRegistry.fail("Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <material>")
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_fill, args)
	var a: Variant = _coords(args, 0, player.position)
	if a is Dictionary:
		return a
	var b: Variant = _coords(args, 3, player.position)
	if b is Dictionary:
		return b
	var m: Variant = _placeable(args[6])
	if m is Dictionary:
		return m
	var lo := Vector3i((a as Vector3).min(b))
	var hi := Vector3i((a as Vector3).max(b)) + Vector3i.ONE   # corners included, as in Minecraft
	var d := hi - lo
	if d.x * d.y * d.z > FILL_MAX:
		return CommandRegistry.fail("Too many voxels: %d (limit %d)" % [d.x * d.y * d.z, FILL_MAX])
	Net.recording = true   # ADR 0145: a /fill reaches the other players
	var n: int = main.bubble.fill_box(lo, hi, m)
	Net.recording = false
	if n < 0:
		return CommandRegistry.fail("The box must lie inside the loaded sim bubble around you")
	return CommandRegistry.ok("Filled %d voxels with %s" % [n, args[6].to_lower()])


func _fullbright(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky to change")
	atmo.fullbright = args[0] == "on" if not args.is_empty() else not atmo.fullbright
	return CommandRegistry.ok("Fullbright %s" % ("on" if atmo.fullbright else "off"))


func _time(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky to change")
	if args.is_empty() or args[0] == "query":
		var now: float = atmo.time_of_day
		var hours := fposmod(now * 24.0, 24.0)
		return CommandRegistry.ok("The time is %02d:%02d (%.3f), %s" % [int(hours), int(fmod(hours, 1.0) * 60.0), now, atmo.calendar.date_text()])
	if args[0] != "set" or args.size() < 2:
		return CommandRegistry.fail("Usage: /time set <day|noon|sunset|night|midnight|sunrise|0..1>")
	var key := args[1].to_lower()
	var t := 0.0
	if TIMES.has(key):
		t = TIMES[key]
		# sunrise and sunset move with the seasons (ADR 0025); day / night are just after them
		var cal: Calendar = atmo.calendar
		match key:
			"sunrise": t = cal.sunrise()
			"day": t = cal.sunrise() + 0.04
			"sunset": t = cal.sunset()
			"night": t = cal.sunset() + 0.04
	elif key.is_valid_float():
		t = fposmod(float(key), 1.0)
	else:
		return CommandRegistry.fail("Unknown time: %s" % args[1])
	atmo.set_time_of_day(t)
	return CommandRegistry.ok("Set the time to %s" % key)


func _name(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or main.mobs == null:
		return CommandRegistry.fail("No animals here")
	var w: Mob = player.aimed_mob()
	if w == null or w.owner_id != "player":
		return CommandRegistry.fail("Look at your companion first")
	if args.is_empty():
		return CommandRegistry.fail("Usage: /name <text>")
	w.set_pet_name(" ".join(args))
	return CommandRegistry.ok("Named %s" % w.display_name())


func _tame(_args: PackedStringArray) -> Dictionary:
	if main == null or player == null or main.mobs == null:
		return CommandRegistry.fail("No animals here")
	var w: Mob = player.aimed_mob()
	if w == null or w.species != "wolf":
		w = null
		var best := Settings.m2v(12.0)
		for m in main.mobs.active():
			if m.species == "wolf" and not m.is_dead() and m.position.distance_to(player.position) < best:
				best = m.position.distance_to(player.position)
				w = m
	if w == null:
		return CommandRegistry.fail("No wolf within 12 m (/summon wolf)")
	w.set_companion(player)
	return CommandRegistry.ok("The wolf is your companion now")


func _summon(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or not ("mobs" in main) or main.mobs == null:
		return CommandRegistry.fail("No animals here")
	var sp := args[0].to_lower() if not args.is_empty() else ""
	if not main.mobs.defs.has(sp):
		return CommandRegistry.fail("Usage: /summon <%s> [count] [baby]" % "|".join(main.mobs.defs.keys()))
	var n := clampi(int(args[1]) if args.size() > 1 and args[1].is_valid_int() else 1, 1, 32)
	var baby := args.size() > 1 and args[args.size() - 1].to_lower() == "baby"
	var hungry := args.size() > 1 and args[args.size() - 1].to_lower() == "hungry"
	var fwd := -player.camera.global_transform.basis.z
	fwd.y = 0
	var at := player.position + fwd.normalized() * Settings.m2v(3.0)
	for i in n:
		var q := at + Vector3(randf_range(-1, 1), 0, randf_range(-1, 1)) * Settings.m2v(2.0)
		if WildMoves.habitat(main.mobs.defs[sp]) == "water":   # swimmers go into the water below, not onto the bank
			var w := _water_below(Vector3i(q.floor()))
			if w == Vector3.INF:
				continue
			q = w
		var m: Mob = main.mobs.spawn(sp, Vector3(q.x, q.y + 0.5, q.z), baby, randf() * TAU)
		if hungry and m != null:
			m.make_hungry()
	return CommandRegistry.ok("Summoned %d %s%s" % [n, "baby " if baby else "", sp])


## A spot in the water within 4 m below `p` (two voxels under the surface where it is deep enough), or INF.
func _water_below(p: Vector3i) -> Vector3:
	for d in int(Settings.m2v(4.0)):
		var c := p + Vector3i(0, -d, 0)
		if main.mobs.is_liquid(c):
			var deep: bool = main.mobs.is_liquid(c + Vector3i(0, -2, 0))
			return Vector3(c.x + 0.5, c.y - (2 if deep else 0.4), c.z + 0.5)
	return Vector3.INF


func _book() -> QuestBook:
	return main.get("quests") if main != null else null


func _quest_ids() -> Array:
	var b := _book()
	return (b.quests.keys() + ["all"]) if b else []


func _quests(args: PackedStringArray) -> Dictionary:
	var b := _book()
	if b == null:
		return CommandRegistry.fail("No progression book")
	var sub := args[0] if not args.is_empty() else "list"
	match sub:
		"list":
			var lines := []
			for c in b.chapters:
				var p := b.chapter_progress(c["id"])
				lines.append("%s %d/%d" % [c["title"], p.x, p.y])
				for q in c["quests"]:
					if not b.is_done(q["id"]) and b.is_unlocked(q["id"]):
						lines.append("  open: %s (%s)" % [q["title"], q["id"]])
			return CommandRegistry.ok("
".join(lines))
		"complete":
			if args.size() < 2:
				return CommandRegistry.fail("Usage: /quests complete <id|all>")
			if args[1] == "all":
				for id in b.quests:
					b.complete(id)
				return CommandRegistry.ok("Completed every quest")
			if not b.quests.has(args[1]):
				return CommandRegistry.fail("No quest %s" % args[1])
			b.complete(args[1])
			return CommandRegistry.ok("Completed %s" % b.quests[args[1]]["title"])
		"reset":
			b.reset()
			return CommandRegistry.ok("Progression book reset")
	return CommandRegistry.fail("Usage: /quests [list|complete <id|all>|reset]")


func _kill(_args: PackedStringArray) -> Dictionary:
	if player == null or player_inv == null:
		return CommandRegistry.fail("No player")
	player_inv.damage(player_inv.health, "termination order (/kill)")   # bypasses creative: /kill works in every mode
	player.died.emit()
	return CommandRegistry.ok("Killed the ape")


func _heal(_args: PackedStringArray) -> Dictionary:
	if player_inv == null:
		return CommandRegistry.fail("No player")
	player_inv.heal(player_inv.max_health)
	return CommandRegistry.ok("Healed")


func _stain(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	var k := args[0].to_lower() if not args.is_empty() else ""
	if k == "clear":
		player.stains.clear()
		return CommandRegistry.ok("Stains cleared")
	if not PlayerStains.KINDS.has(k):
		return CommandRegistry.fail("Usage: /stain <%s|clear>" % "|".join(PlayerStains.KINDS))
	player.stains.apply(k)
	return CommandRegistry.ok("Stained: %s" % ", ".join(player.stains.active()))


func _kit(args: PackedStringArray) -> Dictionary:
	if args.is_empty() or not KITS.has(args[0].to_lower()):
		return CommandRegistry.fail("Usage: /kit <%s>" % "|".join(KITS.keys()))
	var kit: Dictionary = KITS[args[0].to_lower()]
	for id in kit:
		var r := _give(PackedStringArray([id, str(int(kit[id]) * player_inv.items.units(id))]))   # blocks of voxel items
		if not r["ok"]:
			return r
	return CommandRegistry.ok("Gave the %s kit" % args[0].to_lower())


func _clothing_ids() -> Array:
	return player_inv.items.all_ids().filter(func(id: String) -> bool: return Clothing.slot_of(player_inv.items.get_def(id)) != "") if player_inv else []


func _wear(args: PackedStringArray) -> Dictionary:
	var sa: Variant = main.get("stone_age") if main != null else null
	if sa == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not args[0] in _clothing_ids():
		return CommandRegistry.fail("Usage: /wear <item>: one of %s" % ", ".join(_clothing_ids()))
	sa.clothing.wear(Inventory.make_stack(player_inv.items, args[0], 1))
	return CommandRegistry.ok("Wearing %s (armor %d)" % [args[0], sa.clothing.armor()])


func _gadget_ids() -> Array:
	return player_inv.items.all_ids().filter(func(id: String) -> bool: return player_inv.items.get_def(id).has("playtest")) if player_inv else []


func _gadget(args: PackedStringArray) -> Dictionary:
	if main == null or main.gadgets == null or player_inv == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not player_inv.items.has(args[0]) or not player_inv.items.get_def(args[0]).has("playtest"):
		return CommandRegistry.fail("Gadgets: %s" % ", ".join(_gadget_ids()))
	var cam: Camera3D = player.camera
	var dir := -cam.global_transform.basis.z
	var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, dir, Settings.m2v(Settings.laser_range_m))
	if not main.gadgets.use(player_inv.items.get_def(args[0]), hit, cam.global_position, dir, args.size() > 1 and args[1] == "sneak"):
		return CommandRegistry.fail("%s did nothing there" % player_inv.items.display_name(args[0]))
	return CommandRegistry.ok("Used [%s]" % player_inv.items.display_name(args[0]))


func _use_cmd(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if args.size() > 0 and args[0] == "aim":
		player.script_aim = not player.script_aim
		return CommandRegistry.ok("Aim ghost in scripted runs: %s" % ("on" if player.script_aim else "off"))
	if args.size() > 0 and args[0] == "close":
		player.set_ui_open(false)
		return CommandRegistry.ok("Closed")
	player._use(true)
	return CommandRegistry.ok("Used")


func _weather(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("weather") == null:
		return CommandRegistry.fail("No weather (weather=0 or no world)")
	var w: Weather = main.weather
	if args.is_empty():
		return CommandRegistry.ok("Weather: %s" % JSON.stringify(w.summary()))
	var secs := float(args[1]) if args.size() > 1 and args[1].is_valid_float() else 0.0
	if not w.set_weather(args[0].to_lower(), secs):
		return CommandRegistry.fail("Usage: /weather [clear|rain|storm] [seconds]")
	return CommandRegistry.ok("Weather: %s" % args[0].to_lower())


func _lightning(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("weather") == null:
		return CommandRegistry.fail("No weather (weather=0 or no world)")
	var at := Vector3i(0, -1, 0)
	if not args.is_empty() and args[0].to_lower() == "here":
		at = Vector3i(player.position.floor()) + Vector3i.DOWN
	var p: Vector3i = main.weather.strike(at)
	if p.y < 0:
		return CommandRegistry.fail("Nothing to strike (the sim is busy or nothing exposed was sampled)")
	return CommandRegistry.ok("Lightning at %s%s" % [p, " (grounded by a rod)" if main.weather.stats["last_strike"][3] else ""])


func _erupt(_args: PackedStringArray) -> Dictionary:
	if main == null or main.get("volcano") == null:
		return CommandRegistry.fail("No volcanoes (weather=0 or no world)")
	return CommandRegistry.ok(main.volcano.erupt())


func _waypoint(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("waypoints") == null or player == null:
		return CommandRegistry.fail("No world")
	var wps: Waypoints = main.waypoints
	var sub := args[0].to_lower() if args.size() > 0 else "list"
	var wp_name := " ".join(args.slice(1)) if args.size() > 1 else ""
	match sub:
		"add":
			var p := Vector3i(player.position.floor())
			wps.add(wp_name, p)
			return CommandRegistry.ok("Waypoint %s at %d %d %d" % [wps.list[-1]["name"], p.x, p.y, p.z])
		"remove":
			for w in wps.list:
				if String(w["name"]).to_lower() == wp_name.to_lower():
					wps.remove(int(w["id"]))
					return CommandRegistry.ok("Removed %s" % wp_name)
			return CommandRegistry.fail("No waypoint %s" % wp_name)
		"list":
			var lines := []
			for w in wps.list:
				var p: Vector3i = w["pos"]
				lines.append("%s (%d %d %d)" % [w["name"], p.x, p.y, p.z])
			return CommandRegistry.ok("Waypoints: " + (", ".join(lines) if not lines.is_empty() else "none"))
	return CommandRegistry.fail("Usage: /waypoint [add <name>|remove <name>|list]")


func _map(_args: PackedStringArray) -> Dictionary:
	if main == null or main.get("map_screen") == null:
		return CommandRegistry.fail("No world")
	main.call_deferred("_open_map")   # after the chat has closed and given the mouse back
	return CommandRegistry.ok("Map")


func _ignite(_args: PackedStringArray) -> Dictionary:
	if main == null or main.get("weather") == null:
		return CommandRegistry.fail("No weather (weather=0 or no world)")
	var cam: Camera3D = player.camera
	var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, -cam.global_transform.basis.z, Settings.m2v(Settings.laser_range_m))
	if not main.weather.ignite(hit):
		return CommandRegistry.fail("Nothing there catches fire (aim at a face with air in front, inside the simulated area)")
	return CommandRegistry.ok("Fire at %s" % hit.previous_position)


func _laser(args: PackedStringArray) -> Dictionary:
	if main == null or main.gadgets == null:
		return CommandRegistry.fail("No world")
	var secs := float(args[0]) if args.size() > 0 and args[0].is_valid_float() else 1.5
	var sweep := float(args[1]) if args.size() > 1 and args[1].is_valid_float() else 20.0
	main.gadgets.fire_for(clampf(secs, 0.1, 30.0), sweep)
	return CommandRegistry.ok("Laser for %.1f s across %.0f deg" % [secs, sweep])


func _spawn(_args: PackedStringArray) -> Dictionary:
	if player == null or main == null:
		return CommandRegistry.fail("No world")
	player.teleport(main.spawn_position())
	return CommandRegistry.ok("Teleported to spawn")


func _sethome(_args: PackedStringArray) -> Dictionary:
	if player == null or main == null:
		return CommandRegistry.fail("No world")
	var at := Vector3i(player.position.floor())
	main.waypoints.set_unique("home", "Home", at)
	return CommandRegistry.ok("Home set at %d %d %d" % [at.x, at.y, at.z])


func _lightmap(args: PackedStringArray) -> Dictionary:
	if main == null or main.light_overlay == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or (args[0] == "on") != main.light_overlay.visible:
		main.light_overlay.toggle()
	return CommandRegistry.ok("Light overlay " + ("on" if main.light_overlay.visible else "off"))


func _home(_args: PackedStringArray) -> Dictionary:
	if player == null or main == null:
		return CommandRegistry.fail("No world")
	var w: Dictionary = main.waypoints.of_kind("home")
	if w.is_empty():
		return CommandRegistry.fail("No home yet: /sethome marks one")
	player.teleport(Vector3(w["pos"]))
	return CommandRegistry.ok("Teleported home")


## Nearest feature as a voxel position, or a fail() result.
func _find(args: PackedStringArray) -> Variant:
	if args.is_empty():
		return CommandRegistry.fail("Usage: /locate <feature>")
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	var kind := args[0].to_lower()
	var p := player.position
	# continents are ~8 km across (ADR 0035): a biome can be a few km away, a rare one's structure further
	var km := clampf(float(args[1]), 0.1, FIND_MAX_KM) if args.size() > 1 and args[1].is_valid_float() else 4.0
	var t: Vector3i = main.world.generator.gen.find_nearest(kind, int(p.x), int(p.z), int(Settings.m2v(km * 1000.0)))
	if t.y < 0:
		return CommandRegistry.fail("No %s within %s km" % [kind, str(km)])
	return t


func _locate(args: PackedStringArray) -> Dictionary:
	var t: Variant = _find(args)
	if t is Dictionary:
		return t
	var d := Settings.v2m(Vector2(t.x - player.position.x, t.z - player.position.z).length())
	return CommandRegistry.ok("The nearest %s is at %d, %d, %d (%d m away)" % [args[0], t.x, t.y, t.z, int(d)])


func _goto(args: PackedStringArray) -> Dictionary:
	var t: Variant = _find(args)
	if t is Dictionary:
		return t
	var kind := args[0].to_lower()
	# caves: stand in the room itself; surface features: drop in from a little above
	var up := 0.1 if kind.ends_with("cave") else 3.0
	if kind in ["aquifer", "magma_conduit"]:   # buried in rock or lava: land on the ground above, find it by ear
		t.y = main.world.surface_height(t.x, t.z)
		up = 3.0
	if kind == "river":   # the nearest river cell can be a dry bed (the spawn monolith's): go to running water
		var wet := wet_river_near(main.world.generator, t)
		if wet.y < 0:
			return CommandRegistry.fail("No running river within %d m of %d, %d" % [int(RIVER_SEARCH_M), t.x, t.z])
		t = wet
	player.teleport(Vector3(t.x + 0.5, t.y + up, t.z + 0.5))
	return CommandRegistry.ok("Teleported to the nearest %s at %d, %d, %d" % [kind, t.x, t.y, t.z])


const RIVER_SEARCH_M := 400.0
const RIVER_STEP_M := 4.0


## The river column with water nearest `from` (rings of RIVER_STEP_M out to RIVER_SEARCH_M), standing on the
## water surface; y < 0 when none. column_info's water_level is -1 on a dry river bed.
static func wet_river_near(generator: TerrainGenerator, from: Vector3i) -> Vector3i:
	var step := int(Settings.m2v(RIVER_STEP_M))
	for ring in int(RIVER_SEARCH_M / RIVER_STEP_M) + 1:
		for i in maxi(1, ring * 8):
			var s := i % maxi(1, ring * 2) - ring   # walk the ring's four sides
			var side := i / maxi(1, ring * 2)
			var o: Vector2i = [Vector2i(s, -ring), Vector2i(ring, s), Vector2i(-s, ring), Vector2i(-ring, -s)][side]
			var x := from.x + o.x * step
			var z := from.z + o.y * step
			var c := generator.column_info(x, z)
			if float(c["river"]) > 0.5 and int(c["water_level"]) >= 0:
				return Vector3i(x, int(c["water_level"]), z)
	return Vector3i(0, -1, 0)


func _daycycle(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky")
	if args.is_empty():
		return CommandRegistry.ok("Day length: %s" % ("frozen" if atmo.day_length_s <= 0.0 else "%d s" % int(atmo.day_length_s)))
	match args[0].to_lower():
		"on": atmo.day_length_s = 1200.0
		"off": atmo.day_length_s = 0.0
		_:
			if not args[0].is_valid_float():
				return CommandRegistry.fail("Usage: /daycycle <on|off|seconds>")
			atmo.day_length_s = maxf(float(args[0]), 0.0)
	return CommandRegistry.ok("Day cycle %s" % ("frozen" if atmo.day_length_s <= 0.0 else "%d s per day" % int(atmo.day_length_s)))


func _material_names() -> Array:
	var out := ["air"]
	for d in GameMaterials.defs():
		out.append(d["name"])
	return out


func _blast(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not args[0].is_valid_float() or float(args[0]) <= 0.0 or float(args[0]) > 16.0:
		return CommandRegistry.fail("Usage: /blast <radius_m 0..16> [material]")
	var mat := material_id(args[1]) if args.size() > 1 else GameMaterials.AIR
	if mat < 0:
		return CommandRegistry.fail("Unknown material: %s" % args[1])
	var cam: Camera3D = player.camera
	var r := Settings.m2v(float(args[0]))
	var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, -cam.global_transform.basis.z, 64.0)
	var at: Vector3 = Vector3(hit.position) + Vector3(0.5, 0.5, 0.5) if hit else cam.global_position - cam.global_transform.basis.z * 16.0
	if mat != GameMaterials.AIR:
		at.y += r + 2.0   # a ball above the spot, so it falls and spreads
	var n: int = main.bubble.blast(at, r, mat)
	if n > 0 and mat == GameMaterials.AIR:
		Sound.play("sim/blast", at, {"db": clampf(float(args[0]) - 3.0, -6.0, 4.0)})
	return CommandRegistry.ok("Blast of %s m: %d voxels" % [args[0], n])


func _brush(args: PackedStringArray) -> Dictionary:
	if player == null:
		return CommandRegistry.fail("No player")
	if args.size() == 2 and args[0] == "hold":
		player.script_break_s = clampf(float(args[1]), 0.0, 60.0)
		return CommandRegistry.ok("Breaking for %.1f s" % player.script_break_s)
	if args.is_empty() or not Brush.SHAPES.has(args[0]):
		return CommandRegistry.fail("Usage: /brush <%s> [size]" % "|".join(Brush.SHAPES))
	var size := int(args[1]) if args.size() > 1 else player.brush_size
	if not Brush.sizes_for(true).has(size):
		return CommandRegistry.fail("Sizes: %s" % str(Brush.sizes_for(true)))
	player.set_brush(size, args[0])
	return CommandRegistry.ok("Brush: %s (digs %s)" % [Brush.describe(size, args[0]), Brush.describe(player.dig_size(), args[0])])


func _digonly(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("player") == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or args[0] == "off":
		main.player.toggle_dig_only(0)
		return CommandRegistry.ok("Digging everything")
	var m := GameMaterials.id_of(args[0])
	if m <= 0:
		return CommandRegistry.fail("Unknown material: %s" % args[0])
	main.player.toggle_dig_only(m)
	return CommandRegistry.ok("Digging only: %s" % main.player.dig_only_names())


func _tools(_args: PackedStringArray) -> Dictionary:
	if main == null or main.get("builder") == null:
		return CommandRegistry.fail("No world")
	main.builder._open_menu.call_deferred()   # after the chat has closed: it would otherwise keep the mouse free
	return CommandRegistry.ok("Build ring open (Esc closes it)")


func _chapter_ids() -> Array:
	var b := _book()
	return b.chapters.map(func(c: Dictionary) -> String: return c["id"]) if b else []


## /book [chapter] (ADR 0130): the inventory opens on the progression book, at that chapter.
func _book_cmd(args: PackedStringArray) -> Dictionary:
	if _book() == null or main.get("inv_screen") == null:
		return CommandRegistry.fail("No world")
	var id := args[0].to_lower() if args.size() > 0 else ""
	if id != "" and not _chapter_ids().has(id):
		return CommandRegistry.fail("Chapters: %s" % ", ".join(_chapter_ids()))
	_open_inventory.call_deferred(func() -> void:
		main.inv_screen.open_book()
		if id != "":
			main.inv_screen.quest_view.open_chapter(id))
	return CommandRegistry.ok("Progression book open" + (" at " + id if id != "" else ""))


## /browse <item> [recipes|uses] (ADR 0130): the recipe browser on an item, as R or U over it.
func _browse(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("inv_screen") == null:
		return CommandRegistry.fail("No world")
	if args.is_empty() or not _item_ids().has(args[0].to_lower()):
		return CommandRegistry.fail("Usage: /browse <item> [recipes|uses]")
	var mode := "uses" if args.size() > 1 and args[1].to_lower() == "uses" else "recipes"
	var id := args[0].to_lower()
	_open_inventory.call_deferred(func() -> void: main.inv_screen.browser_view.show_item(id, mode))
	return CommandRegistry.ok("Showing the %s of %s" % [mode, id])


## After the chat has closed (it frees the mouse on its way out), as /tools does.
func _open_inventory(then: Callable) -> void:
	if not player.ui_open:
		player.set_ui_open(true)
	then.call()


## /tnt grid [n] [spacing_m]: n TNT blocks in a square grid on the ground where you look (block
## grid, `spacing_m` apart, default 1.5 m: each in the next one's blast), the nearest lit.
func _support(args: PackedStringArray) -> Dictionary:
	if main == null or main.support == null:
		return CommandRegistry.fail("Cave-ins are off (cave_ins=0 or rigid_pieces=0)")
	var sup: MineSupport = main.support
	match args[0] if args.size() > 0 else "stats":
		"stats":
			return CommandRegistry.ok(JSON.stringify(sup.stats()))
		"scan":
			var p := Vector3i(player.position.floor())
			sup.prop_changed(p - Vector3i(Settings.BLOCK, 0, Settings.BLOCK))
			return CommandRegistry.ok("Roof around you queued for a check")
		"probe":
			var cam: Camera3D = player.camera
			var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, -cam.global_transform.basis.z, Settings.m2v(Settings.reach_m))
			if hit == null:
				return CommandRegistry.fail("Aim at a roof")
			return CommandRegistry.ok(sup.hint(hit.position) + " " + JSON.stringify(sup.probe))
		"modes":
			Settings.cave_ins_all_modes = args.size() > 1 and args[1] == "all"
			return CommandRegistry.ok("Cave-ins in %s" % ("every mode" if Settings.cave_ins_all_modes else "survival only"))
		"on", "off":
			Settings.cave_ins = args[0] == "on"
			return CommandRegistry.ok("Cave-ins %s" % args[0])
	return CommandRegistry.fail("/support [stats|scan|probe|modes <all|survival>|on|off]")


func _meteor(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("meteors") == null:
		return CommandRegistry.fail("No world")
	return main.meteors.command(args)


func _tnt(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or main.tnt == null:
		return CommandRegistry.fail("No world")
	var tnt: Tnt = main.tnt
	var cam: Camera3D = player.camera
	var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, -cam.global_transform.basis.z, Settings.m2v(48.0))
	match args[0] if args.size() > 0 else "":
		"stats":
			return CommandRegistry.ok(JSON.stringify(tnt.snapshot()))
		"light":
			return CommandRegistry.ok("Lit") if tnt.light_at(hit) else CommandRegistry.fail("Aim at TNT")
		"grid":
			if hit == null:
				return CommandRegistry.fail("Aim at the ground")
			var n := clampi(int(args[1]) if args.size() > 1 else 20, 1, 400)
			var step := maxi(Settings.BLOCK, roundi(Settings.m2v(float(args[2]) if args.size() > 2 else 1.5) / Settings.BLOCK) * Settings.BLOCK)
			var side := ceili(sqrt(n))
			var origin := Tnt.anchor_of(hit.position) - Vector3i(side / 2 * step, 0, side / 2 * step)
			var placed: Array[Vector3i] = []
			for i in n:
				var a := origin + Vector3i((i % side) * step, 0, (i / side) * step)
				a.y += Settings.BLOCK   # all at the aimed block's top: level, so the chain is the same on any ground
				if main.bubble.fill_box(a, a + Vector3i.ONE * Settings.BLOCK, GameMaterials.TNT) < 0:
					continue
				tnt.register_cells([a])
				placed.append(a)
			if placed.is_empty():
				return CommandRegistry.fail("Place it inside the simulated area around you")
			var near := placed[0]
			for a in placed:
				if Vector3(a).distance_to(player.position) < Vector3(near).distance_to(player.position):
					near = a
			tnt.light(near)
			return CommandRegistry.ok("TNT: %d blocks, %.2f m apart, the nearest lit" % [placed.size(), Settings.v2m(step)])
		"fuse":
			if hit == null:
				return CommandRegistry.fail("Aim at the ground")
			var a := Tnt.anchor_of(hit.position) + Vector3i(0, Settings.BLOCK, 0)
			if main.bubble.fill_box(a, a + Vector3i.ONE * Settings.BLOCK, GameMaterials.TNT) < 0:
				return CommandRegistry.fail("Place it inside the simulated area around you")
			tnt.register_cells([a])
			var to := player.position - Vector3(a)
			var dir := Vector3i(signi(int(to.x)), 0, 0) if absf(to.x) >= absf(to.z) else Vector3i(0, 0, signi(int(to.z)))
			var length := float(args[1]) if args.size() > 1 else 4.0
			var end := tnt.lay_fuse(a, dir if dir != Vector3i.ZERO else Vector3i(1, 0, 0), maxi(2, roundi(Settings.m2v(length))))
			if end == Vector3i.MAX:
				return CommandRegistry.fail("Place it inside the simulated area around you")
			return CommandRegistry.ok("TNT with a %.1f m fuse cord, lit at %s (%.0f m away): about %.1f s" % [length, end, Settings.v2m(Vector3(end).distance_to(player.position)), length / Settings.fuse_speed_mps])
	return CommandRegistry.fail("Usage: /tnt <grid [n] [spacing_m]|fuse [length_m]|light|stats>")


## /sound <name> [x y z] plays a sound (at the player, or a voxel position); list and stats report.
func _sound(args: PackedStringArray) -> Dictionary:
	if args.is_empty() or args[0] == "stats":
		return CommandRegistry.ok(JSON.stringify(Sound.stats()))
	if args[0] == "list":
		return CommandRegistry.ok(", ".join(Sound.sound_names()))
	if not Sound.has_sound(args[0]):
		return CommandRegistry.fail("Unknown sound: %s (/sound list)" % args[0])
	var at := player.position + Vector3(0, Settings.m2v(1.0), 0) if player else Vector3.INF
	if args.size() >= 4:
		at = Vector3(float(args[1]), float(args[2]), float(args[3]))
	return CommandRegistry.ok("Played %s" % args[0]) if Sound.play(args[0], at) else CommandRegistry.fail("Not played (cooling down, out of range or sound=0)")


func _music(args: PackedStringArray) -> Dictionary:
	match args[0] if args.size() > 0 else "":
		"next":
			return CommandRegistry.ok("Playing %s" % Sound.next_music())
		"stop":
			Sound.stop_music()
			return CommandRegistry.ok("Music stopped; the next track comes after the usual silence")
		"":
			var now: String = Sound.stats()["music"]
			return CommandRegistry.ok("Playing %s" % now if now != "" else "No music playing")
	if not args[0].begins_with("music/"):
		args[0] = "music/" + args[0]
	Sound.play_music(args[0])
	return CommandRegistry.ok("Playing %s" % Sound.stats()["music"])


func _ponder(args: PackedStringArray) -> Dictionary:
	if args.is_empty():
		return CommandRegistry.ok("Guides: " + ", ".join(Ponder.scenes().keys()))
	var id := args[0]
	if not Ponder.scenes().has(id):
		id = Ponder.for_item(id)   # an item id opens its guide
	if main == null or main.get("ponder") == null or not main.ponder.open(id):
		return CommandRegistry.fail("No guide '%s'. Guides: %s" % [args[0], ", ".join(Ponder.scenes().keys())])
	return CommandRegistry.ok("Guide: " + String(Ponder.get_scene(id).get("title", id)))


func _fish(args: PackedStringArray) -> Dictionary:
	if main == null or main.fishing == null:
		return CommandRegistry.fail("No world")
	var what := args[0] if args.size() > 0 else "pond"
	if what == "state":
		return CommandRegistry.ok(JSON.stringify(main.fishing.snapshot()))
	if what == "cast":   # scripted: the bite and the reel play themselves, the cast goes down ahead
		main.fishing.auto = true
		var b: Basis = player.camera.global_transform.basis
		var d: Vector3 = (-b.z * Vector3(1, 0, 1)).normalized() * 3.0 + Vector3(0, -1.6, 0)
		main.fishing.use_rod(player.camera.global_position, d.normalized())
		return CommandRegistry.ok("Cast: " + main.fishing.snapshot()["state"])
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_fish, args)
	var laid: String = main.fishing.debug_scene(what)
	if what == "traps" and args.size() > 1 and args[1] == "cam":   # film it from above (/cam off to leave)
		var c: Vector3 = main.fishing.scene_at
		var back: Vector3 = (player.position - c) * Vector3(1, 0, 1)
		_debug_cam().enable(c + back.normalized() * Settings.m2v(2.5) + Vector3(0, Settings.m2v(2.2), 0), c + Vector3(-2, -2, 0))
	return CommandRegistry.ok("Laid " + laid)


var _lift_spot := Vector3.INF   # /contraptions: the lift platform's top centre


func _contraptions(args: PackedStringArray) -> Dictionary:
	if main == null or main.contraptions == null:
		return CommandRegistry.fail("No contraptions (rigid_pieces=0?)")
	if "toggle" in args:
		var n := 0
		for k in ContraptionEntity.KINDS:
			for e: ContraptionEntity in main.blocks.all_of(k):
				e.use({})
				n += 1
		return CommandRegistry.ok("Flipped %d levers" % n)
	if "ride" in args:
		if _lift_spot == Vector3.INF:
			return CommandRegistry.fail("Build them first: /contraptions")
		player.position = _lift_spot
		player.velocity = Vector3.ZERO
		return CommandRegistry.ok("On the lift")
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_contraptions, args)
	var facing := Transport.facing_from_yaw(player.rotation_degrees.y)
	var base := Vector3i((player.position + Vector3(Transport.dir_of(facing)) / Settings.BLOCK * Settings.m2v(3.0)).floor())
	var d: Dictionary = main.contraptions.debug_build(base, facing)
	_lift_spot = d["lift"]
	if "cam" in args:
		_debug_cam().enable(d["cam"], d["look"])
	return CommandRegistry.ok("Laid " + d["laid"])


func _pollute(args: PackedStringArray) -> Dictionary:
	if main == null:
		return CommandRegistry.fail("No world")
	if not main.bubble.is_loaded():
		return _queue_until_loaded(_pollute, args)
	return CommandRegistry.ok("Laid " + preload("res://scripts/debug/pollute_scene.gd").lay(main, args[0] if args.size() > 0 else ""))


func _stations(args: PackedStringArray) -> Dictionary:
	if main == null:
		return CommandRegistry.fail("No world")
	if args.size() > 0 and args[0] == "pollution":   # ADR 0141: a stream fouled beside a settling pond
		if not main.bubble.is_loaded():
			return _queue_until_loaded(_stations, args)   # its fills need the sim window loaded
		# a 13 m stone slab 4 m up toward +z, centred on the player's x (is_loaded() vouches for 8 m around): no terrain
		# in the way of the water or the view
		var anchor := BlockEntities.anchor_for(Vector3i((player.position + Vector3(-12 * Settings.BLOCK, Settings.m2v(4.0), Settings.m2v(4.0))).floor()))
		return CommandRegistry.ok("Laid " + main.work.debug_pollution(anchor))
	if args.size() > 0 and args[0] == "smoke":   # ADR 0141: smoke pools under a roof, no wind down there
		if not main.bubble.is_loaded():
			return _queue_until_loaded(_stations, args)
		var at := Vector3i((player.position + Vector3(0, 0, Settings.m2v(2.0))).floor())
		at.y = main.world.surface_height(at.x, at.z)
		var d: Dictionary = main.work.debug_cave_smoke(at)
		_debug_cam().enable(d["cam"], d["look"])
		return CommandRegistry.ok("Laid %s (/cam off to leave the view)" % d["laid"])
	if args.size() > 0 and args[0] == "fx":   # ADR 0153: 30 running machines of every animated kind, 4 m ahead
		var f3: Vector3 = -player.camera.global_transform.basis.z
		f3.y = 0
		var fw := Vector3i(f3.normalized().round())
		if fw == Vector3i.ZERO or (fw.x != 0 and fw.z != 0):
			fw = Vector3i(0, 0, -1)
		var rt := Vector3i(Vector3(fw).cross(Vector3.UP).round())
		var a0 := BlockEntities.anchor_for(Vector3i((player.position + Vector3(fw) * Settings.m2v(4.0) - Vector3(rt) * Settings.m2v(7.5)).floor()))
		a0.y = int(floor(player.position.y))
		var only: Array = []   # /stations fx boiler,cutter,...: those kinds in one row, filmed close
		var n := 30
		if args.size() > 1:
			if args[1].is_valid_int():
				n = int(args[1])
			else:
				only = Array(args[1].split(","))
				n = only.size()
		var laid: String = main.work.debug_fx_grid(a0, rt, fw, n, only)
		var cols := mini(n, 6)
		var rows := ceili(n / 6.0)
		var mid := Vector3(a0) + Vector3(rt) * Settings.m2v(1.5 * (cols - 1) + 0.6) + Vector3(fw) * Settings.m2v(1.5 * (rows - 1) + 0.6)
		mid.y = a0.y + Settings.m2v(1.0)
		var back := 1.0 + 0.9 * cols + 1.0 * rows   # metres: the whole factory in view, a short row close up
		_debug_cam().enable(mid - Vector3(fw) * Settings.m2v(back) + Vector3(0, Settings.m2v(back * 0.6), 0), mid)
		return CommandRegistry.ok("Laid %s (/cam off to leave the view)" % laid)
	if args.size() > 0 and args[0] in ["electric", "factory", "lv", "power", "dam"]:   # ADR 0113, 0117, 0123, 0131: the energy line, the factory, the LV machines or the small sources, 4 m ahead, along the player's right
		var fwd: Vector3 = -player.camera.global_transform.basis.z
		fwd.y = 0
		var right := Vector3i(fwd.normalized().cross(Vector3.UP).round())
		if right == Vector3i.ZERO:
			right = Vector3i.RIGHT
		var q := player.position + fwd.normalized() * Settings.m2v(4.0) - Vector3(right) * Settings.m2v(2.5)
		var anchor := BlockEntities.anchor_for(Vector3i(q.floor()))
		anchor.y = main.world.surface_height(anchor.x, anchor.z) + 1
		var laid: String
		match args[0]:
			"factory":
				laid = main.work.debug_factory_line(anchor, right)
			"lv":
				laid = main.work.debug_lv_line(anchor, right)
			"power":
				laid = main.work.debug_power_line(anchor, right)
			"dam":
				laid = main.work.debug_dam(anchor, right)
			_:
				laid = main.work.debug_electric_line(anchor, right)
		if args.size() > 1 and args[1] == "cam":   # a free camera over the line's first 10 m (filming its machines, ADR 0153)
			var look := Vector3(anchor) + Vector3(right) * Settings.m2v(5.0) + Vector3(0, Settings.m2v(0.8), 0)
			_debug_cam().enable(look - fwd.normalized() * Settings.m2v(5.5) + Vector3(0, Settings.m2v(3.5), 0), look)
			laid += " (/cam off to leave the view)"
		return CommandRegistry.ok("Laid " + laid)
	main.place_stations()
	return CommandRegistry.ok("Placing a crafting table, chest and furnace ahead")


func _debug_cam() -> DebugCamera:
	if _cam == null:
		_cam = DebugCamera.new(player)
		main.add_child(_cam)
	return _cam


func _grade(args: PackedStringArray) -> Dictionary:
	var atmo: Node = main.get("atmosphere") if main != null else null
	if atmo == null:
		return CommandRegistry.fail("No sky")
	if args.is_empty():
		return CommandRegistry.ok("Lighting grade: %s" % atmo.grade)
	if not atmo.apply_grade(args[0].to_lower()):
		return CommandRegistry.fail("Grades: %s" % ", ".join(Atmosphere.GRADES.keys()))
	return CommandRegistry.ok("Lighting grade: %s" % atmo.grade)


func _shot(args: PackedStringArray) -> Dictionary:
	if main == null or args.is_empty() or not args[0].is_valid_filename():
		return CommandRegistry.fail("Usage: /shot <name> [hud]")
	var dir := DebugCapture.out_dir + "/shots" if DebugCapture.autopilot else "res://out/shots"
	_save_shot(dir, args[0], args.size() > 1 and args[1] == "hud")
	return CommandRegistry.ok("Saving %s/%s.png" % [dir, args[0]])


## Hides the HUD and chat for two frames, saves the frame, shows them again (a coroutine).
func _save_shot(dir: String, file: String, with_hud: bool) -> void:
	var layers: Array = [] if with_hud else [main.get("game_hud"), main.get("chat"), main.get("inv_screen")]
	var was: Array = []
	for l in layers:
		was.append(l != null and l.visible)
		if l != null:
			l.visible = false
	await RenderingServer.frame_post_draw
	await RenderingServer.frame_post_draw
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(dir))
	main.get_viewport().get_texture().get_image().save_png("%s/%s.png" % [dir, file])
	for i in layers.size():
		if layers[i] != null:
			layers[i].visible = was[i]


func _showcase(args: PackedStringArray) -> Dictionary:
	if main == null or args.is_empty():
		return CommandRegistry.fail("Usage: /showcase <name> [<name> ...] [on=<material>]")
	var names := PackedStringArray()
	var on := "grass"
	for a in args:
		if a.begins_with("on="):
			on = a.substr(3)
		else:
			names.append(a)
	Showcase.new().run(main, names, on)
	return CommandRegistry.ok("Filming %d on a pad above you" % names.size())


func _cam_cmd(args: PackedStringArray) -> Dictionary:
	if main == null or player == null:
		return CommandRegistry.fail("No player")
	var cam := _debug_cam()
	if args.size() == 1 and args[0] == "off":
		cam.disable()
		return CommandRegistry.ok("Back to the player's view")
	if args.is_empty():
		if cam.is_on():
			cam.disable()
			return CommandRegistry.ok("Back to the player's view")
		cam.enable()
		return CommandRegistry.ok("Free camera on (/cam again to leave)")
	if args.size() != 3 and args.size() != 6:
		return CommandRegistry.fail("Usage: /cam [off | <x> <y> <z> [<look_x> <look_y> <look_z>]]")
	var at: Variant = _coords(args, 0, cam.global_position if cam.is_on() else player.camera.global_position, true)
	if at is Dictionary:
		return at
	var target: Variant = null
	if args.size() == 6:
		target = _coords(args, 3, at, true)
		if target is Dictionary:
			return target
	cam.enable(at, target)
	return CommandRegistry.ok("Free camera at %.0f, %.0f, %.0f" % [at.x, at.y, at.z])


## A camera spot that sees a tree at `c` (world voxels, the trunk's foot) fall: around `away` (+-75 deg),
## 12-30 m off, 2-24 m up, scored by clear lines to 4, 10 and 16 m up the trunk (a line may end in the
## tree itself). Returns {at, clear} or {}.
func _vantage(c: Vector3, away: Vector3) -> Dictionary:
	var best := {}
	for dist_m in [14.0, 18.0, 22.0, 26.0, 30.0]:
		for up_m in [2.0, 6.0, 10.0, 16.0, 24.0]:
			for k in range(-5, 6):
				var dir := away.rotated(Vector3.UP, deg_to_rad(15.0 * k))
				var at := c + dir * Settings.m2v(dist_m)
				at.y = main.world.surface_height(int(at.x), int(at.z)) + Settings.m2v(up_m)
				var clear := 0
				for aim_m in [4.0, 10.0, 16.0]:
					var to := c + Vector3(0, Settings.m2v(aim_m), 0) - at
					var hit: VoxelRaycastResult = main.world.raycast(at, to.normalized(), to.length())
					if hit == null or Vector2(hit.position.x - c.x, hit.position.z - c.z).length() < Settings.m2v(6.0):
						clear += 1
				if clear > int(best.get("clear", -1)):
					best = {"at": at, "clear": clear}
				if clear == 3:
					return best
	return best


func _survival(args: PackedStringArray) -> Dictionary:
	if main == null or main.survival == null:
		return CommandRegistry.fail("No survival")
	var sv: Survival = main.survival
	if args.size() == 2 and args[1].is_valid_float():
		var v := float(args[1])
		match args[0]:
			"hunger":
				sv.hunger = clampf(v, 0.0, Settings.hunger_max)
				sv.saturation = minf(sv.saturation, sv.hunger)
			"body":
				sv.body_c = clampf(v, 25.0, 45.0)
			"spoil":
				var h: Dictionary = player_inv.held()
				if not h.has("spoil"):
					return CommandRegistry.fail("The held item does not spoil")
				h["spoil"] = clampf(v, 0.0, 1.0)
				player_inv.changed.emit()
			_:
				return CommandRegistry.fail("Usage: /survival [hunger <0..20> | body <C> | spoil <0..1>]")
	sv._measure()
	var s := sv.snapshot()
	return CommandRegistry.ok("Hunger %.1f (+%.1f), body %.1f C %s, air %.1f C%s%s" % [s["hunger"], s["saturation"], s["body_c"], s["status"],
		s["env_c"], ", in a room" if s["in_room"] else "", ", heat %.1f" % s["heat"] if s["heat"] > 0.05 else ""])


func _shape(args: PackedStringArray) -> Dictionary:
	if main == null or main.builder == null:
		return CommandRegistry.fail("No builder")
	var b: ShapeBuilder = main.builder
	if args.is_empty():
		return CommandRegistry.fail("Usage: /shape <shape> [w h d] [hollow n] | commit | cancel | turn [tip] | mirror")
	match args[0]:
		"commit":
			var n := b.commit()
			return CommandRegistry.ok("Built %d voxels" % n) if n > 0 else CommandRegistry.fail("Not built: %s" % b.refusal)
		"cancel":
			b._set_state(ShapeBuilder.State.OFF)
			return CommandRegistry.ok("Building off")
		"turn":
			b.turn(2 if args.size() > 1 and args[1] == "tip" else 1)
			return CommandRegistry.ok("Turned: %s" % str(b.wsize))
		"mirror":
			b.mirror = not b.mirror
			b._changed()
			return CommandRegistry.ok("Mirrored")
		"menu":
			if args.size() > 1 and args[1] == "pick":
				b._close_menu()
				return CommandRegistry.ok("Menu closed: %s, %s" % [b.snapshot()["state"], b.snapshot()["shape"]])
			b._open_menu()
			var want := " ".join(args.slice(1)).to_lower()
			for i in b.menu.items.size():
				if String(b.menu.items[i]["label"]).to_lower() == want:
					b.menu.feed(Vector2.from_angle(b.menu._angle_of(i)) * 80.0)   # what mouse motion does
			return CommandRegistry.ok("Menu open on %s" % (b.menu.items[b.menu.selected]["label"] if b.menu.selected >= 0 else "nothing"))
		"drag":
			if args.size() < 3:
				return CommandRegistry.fail("Usage: /shape drag <top|+x|-x|+z|-z|hollow|move> <voxels>")
			var seen := b.drag_by(args[1], float(args[2]))
			return CommandRegistry.ok("Dragged %s (aimed at %s): %s at %s, hollow %d" % [args[1], seen, str(b.wsize), str(b.lo), b.hollow])
	if not ShapeBuilder.SHAPES.has(args[0]):
		return CommandRegistry.fail("Unknown shape: %s" % args[0])
	if b._held_material() <= 0:
		return CommandRegistry.fail("Hold a voxel material to build with")
	var size := Vector3i.ZERO
	if args.size() >= 4 and args[1].is_valid_int() and args[2].is_valid_int() and args[3].is_valid_int():
		size = Vector3i(int(args[1]), int(args[2]), int(args[3]))
	b.start_shape(args[0], size)
	var hi := args.find("hollow")
	if hi >= 0 and hi + 1 < args.size():
		b.hollow = int(args[hi + 1])
	b.place_at_aim(_at_arg(args))
	return CommandRegistry.ok("%s %s at %s" % [args[0], str(b.wsize), str(b.lo)])


## "at <x> <y> <z>" in a command's args (~ relative to the player) as a Vector3i, else null.
func _blueprint(args: PackedStringArray) -> Dictionary:
	if main == null or main.get("blueprints") == null:
		return CommandRegistry.fail("No blueprints here")
	var bp: Blueprints = main.blueprints
	var usage := "Usage: /blueprint copy x1 y1 z1 x2 y2 z2 | place [at x y z] [turn 0-3] [mirror] | list | clear"
	match args[0] if args.size() > 0 else "":
		"list":
			return CommandRegistry.ok(JSON.stringify(bp.summary()))
		"clear":
			bp.clear_sites()
			return CommandRegistry.ok("Sites cleared")
		"copy":
			if args.size() < 7:
				return CommandRegistry.fail(usage)
			var a: Variant = _coords(args, 1, player.position)
			var b: Variant = _coords(args, 4, player.position)
			if not (a is Vector3 and b is Vector3):
				return CommandRegistry.fail(usage)
			var lo := Vector3i((a as Vector3).min(b))
			var d := Blueprint.capture(main, lo, Vector3i((a as Vector3).max(b)) - lo + Vector3i.ONE)
			if d.is_empty() or d.has("error"):
				return CommandRegistry.fail(d.get("error", "Nothing there to copy"))
			main.player_inv.pickup("written_blueprint", 1, {"blueprint": d})
			return CommandRegistry.ok("Copied: " + Blueprint.describe(d, main.items))
		"place":
			var held: Dictionary = main.player_inv.held()
			if String(held.get("id", "")) != "written_blueprint":
				return CommandRegistry.fail("Hold a written blueprint")
			var at: Variant = _at_arg(args)
			if at == null:
				bp.start_placing(held["blueprint"])
				return CommandRegistry.ok("Placing: click to mark the site")
			var rot := 0
			var t := args.find("turn")
			for _q in (int(args[t + 1]) if t >= 0 and t + 1 < args.size() else 0):
				rot = ShapeTool.new().turn(rot, 1)
			var i := bp.mark(held["blueprint"], at, rot, args.has("mirror"))
			return CommandRegistry.ok("Site %d marked at %s" % [i, at]) if i >= 0 else CommandRegistry.fail("This blueprint is blank")
	return CommandRegistry.fail(usage)


func _at_arg(args: PackedStringArray) -> Variant:
	var i := args.find("at")
	if i < 0 or i + 3 >= args.size():
		return null
	var p: Variant = _coords(args, i + 1, player.position)
	return Vector3i(p) if p is Vector3 else null


func _box(args: PackedStringArray) -> Dictionary:
	if main == null or main.builder == null:
		return CommandRegistry.fail("No builder")
	var b: ShapeBuilder = main.builder
	if args.size() == 1 and args[0] == "go":
		return CommandRegistry.ok("Dug %d voxels" % b.stats["dug"]) if b.dig_now() else CommandRegistry.fail("No box marked to dig")
	if args.size() != 7 or not args[0] in ["build", "dig", "select"]:
		return CommandRegistry.fail("Usage: /box <build|dig|select> <x1> <y1> <z1> <x2> <y2> <z2> | go")
	var a: Variant = _coords(args, 1, player.position)
	if a is Dictionary:
		return a
	var c: Variant = _coords(args, 4, player.position)
	if c is Dictionary:
		return c
	b.start_box(args[0])
	b.mark_box(Vector3i(a), Vector3i(c))
	return CommandRegistry.ok("Box %s from %s, size %s (%s)" % [args[0], str(b.lo), str(b.wsize), b.snapshot()["state"]])


func _prefab(args: PackedStringArray) -> Dictionary:
	if main == null or main.builder == null:
		return CommandRegistry.fail("No builder")
	var b: ShapeBuilder = main.builder
	if args.size() == 1 and args[0] == "list":
		return CommandRegistry.ok(str(b.prefabs.list()))
	if args.size() < 3:
		return CommandRegistry.fail("Usage: /prefab save <category> <name> | place <category> <name> | list")
	var nm := " ".join(args.slice(2))
	if args[0] == "save":
		var path := b.save_selection(args[1], nm)
		return CommandRegistry.ok("Saved %s" % path) if path != "" else CommandRegistry.fail("Select a box first (V, Select), with something built in it")
	if args[0] == "place":
		var at: Variant = _at_arg(args)
		if at != null:
			nm = " ".join(args.slice(2, args.find("at")))
		if not b.start_prefab(args[1], nm):
			return CommandRegistry.fail("No prefab %s / %s" % [args[1], nm])
		b.place_at_aim(at)
		return CommandRegistry.ok("Placing %s, %s" % [nm, str(b.wsize)])
	return CommandRegistry.fail("Usage: /prefab save <category> <name> | place <category> <name> | list")


func _chop(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or main.pieces == null:
		return CommandRegistry.fail("Rigid pieces are off (started with rigid_pieces=0)")
	var tr: Dictionary = main.pieces.find_trunk(player.position, 20.0, main.world)
	if tr.is_empty():
		return CommandRegistry.fail("No standing trunk 8-20 m away")
	var plan: Dictionary = main.pieces.chop_plan(tr, player.position)
	var c: Vector3 = plan["centre"]
	var eye: Vector3 = player.camera.global_position
	var look: Vector3 = c + Vector3(0, Settings.m2v(5.0), 0) - eye
	player._yaw = atan2(-look.x, -look.z)
	player._pitch = atan2(look.y, Vector2(look.x, look.z).length())
	player._apply_look()
	if args.has("cam"):
		var v: Dictionary = _vantage(c, Vector3(eye.x - c.x, 0.0, eye.z - c.z).normalized())
		if not v.is_empty():
			_debug_cam().enable(v["at"], c + Vector3(0, Settings.m2v(8.0), 0))
	_chop_run(plan)
	return CommandRegistry.ok("Chopping a trunk of %d columns: %d digs" % [tr["columns"], plan["hits"].size()])


func _chop_run(plan: Dictionary) -> void:
	for hit: Vector3i in plan["hits"]:
		await main.get_tree().create_timer(0.3).timeout
		var cells := Brush.dig_cells(hit, plan["size"], "cube")
		main._on_dug(cells, [], Vector3(hit) + Vector3(0.5, 0.5, 0.5))


func _fell(args: PackedStringArray) -> Dictionary:
	if main == null or player == null or main.pieces == null:
		return CommandRegistry.fail("Rigid pieces are off (started with rigid_pieces=0)")
	var deg := float(args[0]) if args.size() > 0 and args[0].is_valid_float() else 0.0
	var with_cam := args.has("cam")
	var t: Dictionary
	if args.has("pillar"):
		t = main.pieces.fell_pillar(player.camera.global_position, -player.camera.global_transform.basis.z, main.world)
	else:
		t = main.pieces.fell_nearest(player.position, 20.0, deg, main.world)
	if t.is_empty():
		return CommandRegistry.fail("No standing trunk 8-20 m away")
	# face it, looking at 5 m up the trunk (from above the canopy: at the crown): it falls across the view
	var eye: Vector3 = player.camera.global_position
	var aim_m := 5.0 if eye.y - t["centre"].y < Settings.m2v(20.0) else 16.0
	var look: Vector3 = t["centre"] + Vector3(0, Settings.m2v(aim_m), 0) - eye
	player._yaw = atan2(-look.x, -look.z)
	player._pitch = atan2(look.y, Vector2(look.x, look.z).length())
	player._apply_look()
	if with_cam:
		# a free camera where the fall can be seen: the first spot near the player's side of the fall
		# line with a clear line to the trunk and the crown (a forest hides it from most places)
		var c: Vector3 = t["centre"]
		var away := Vector3(eye.x - c.x, 0.0, eye.z - c.z).normalized()
		var v: Dictionary = _vantage(c, away)
		if not v.is_empty():
			_debug_cam().enable(v["at"], c + Vector3(0, Settings.m2v(8.0), 0))
			t["clear_rays"] = v["clear"]
	return CommandRegistry.ok("Felled a trunk of %d columns (%d voxels cut) at %s m, view from %s m (%d/3 clear)" % [t["columns"], t["cut"],
		str((t["centre"] / Settings.m2v(1.0)).snapped(Vector3.ONE * 0.1)), str((player.get_viewport().get_camera_3d().global_position / Settings.m2v(1.0)).snapped(Vector3.ONE * 0.1)),
		t.get("clear_rays", -1)])


func _bug(args: PackedStringArray) -> Dictionary:
	if main == null:
		return CommandRegistry.fail("No game to report on")
	var path := BugReport.write(main, " ".join(args), null)
	if path == "":
		return CommandRegistry.fail("Could not save the bug report")
	return CommandRegistry.ok("Bug report saved: %s" % path)


func _seed(_args: PackedStringArray) -> Dictionary:
	var text := " (\"%s\")" % Settings.world_seed_text if Settings.world_seed_text != "" else ""
	return CommandRegistry.ok("Seed: [%d]%s" % [Settings.world_seed, text])
