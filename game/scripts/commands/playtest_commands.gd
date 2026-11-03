class_name PlaytestCommands
extends RefCounted

## Shortcuts for the playtest list (docs/plans/to-test.md, ADR 0018), registered by GameCommands:
##   /totest [count]       the newest unticked items of the list
##   /scenario <name>      jump into a set-up test: a few existing commands run in a row
##   /count <material>     cells of a material in the sim bubble (how much steam a vent leaves, gas in a cave ...)
## A scenario is only command lines, so each step also works typed and reports in chat.

## The list sits beside the game project in a checkout; an exported build has no copy.
const TO_TEST := "res://../docs/plans/to-test.md"
const SHOWN := 8   # /totest without a count
const ITEM_CHARS := 160   # one item's line in chat

## Command lines per scenario; {x} {z} {ledge} ... are voxel coordinates fixed when it starts (lines()), since a
## fill queued until the world loads would read ~ after the teleport.
const SCENARIOS := {
	"river": ["/goto river", "/gamemode creative", "/kit water"],   # scoop, crater rod, laser a trench (ADR 0104)
	"cave_stream": ["/goto river", "/gamemode creative", "/kit water", "/locate cave"],   # tunnel from the bank to that cave
	"lit_station": ["/time set night", "/daycycle off", "/stations"],   # /stations lights its furnace (cobblestone + coal)
	"ledge": ["/gamemode survival", "/fill {x0} {ledge} {z0} {x1} {ledge} {z1} stone", "/tp {x} {on} {z}"],   # Ctrl sprint, Shift stops at the edge
}

var reg: CommandRegistry
var player: Player
var main_of := Callable()   # () -> Main or null


func register(p_reg: CommandRegistry, p_player: Player, p_main_of := Callable()) -> void:
	reg = p_reg
	player = p_player
	main_of = p_main_of
	reg.register("totest", "/totest [count]", "The newest unticked items of the playtest list (docs/plans/to-test.md)", _totest)
	reg.register("scenario", "/scenario <%s>" % "|".join(SCENARIOS.keys()), "Jump into a playtest set-up from the to-test list", _scenario, [SCENARIOS.keys()])
	reg.register("count", "/count <material>", "Cells of a material in the sim bubble (steam left by a vent, gas pooled in a cave)", _count)


## The unticked "- [ ]" items of a to-test list, newest (topmost) first, each as "Section: text" on one line.
static func unticked(text: String, count: int) -> Array[String]:
	var out: Array[String] = []
	var section := ""
	var cur := ""
	for line in text.split("\n"):
		var l := line.strip_edges()
		var cont := cur != "" and l != "" and line.begins_with(" ")
		if cont:
			cur += " " + l
			continue
		if cur != "":
			out.append(cur.left(ITEM_CHARS))
			cur = ""
			if out.size() >= count:
				return out
		if l.begins_with("## "):
			section = l.substr(3).get_slice(" (", 0)
		elif l.begins_with("- [ ] "):
			cur = "%s: %s" % [section, l.substr(6)]
	if cur != "" and out.size() < count:
		out.append(cur.left(ITEM_CHARS))
	return out


func _totest(args: PackedStringArray) -> Dictionary:
	var n := SHOWN
	if args.size() > 0:
		if not args[0].is_valid_int() or int(args[0]) < 1:
			return CommandRegistry.fail("Usage: /totest [count]")
		n = int(args[0])
	var text := FileAccess.get_file_as_string(TO_TEST)
	if text == "":
		return CommandRegistry.fail("No playtest list here (%s): run from a checkout" % TO_TEST)
	var items := unticked(text, n)
	if items.is_empty():
		return CommandRegistry.ok("Everything on the playtest list is ticked")
	return CommandRegistry.ok("Newest unticked:\n- " + "\n- ".join(items))


## A scenario's command lines for a player standing at voxel `at`: the ledge is a 6 m platform 3 m up.
static func lines(name: String, at: Vector3i) -> Array:
	var half := int(Settings.m2v(3.0))
	var ledge := at.y + int(Settings.m2v(3.0))
	var spots := {"x": at.x, "z": at.z, "x0": at.x - half, "x1": at.x + half, "z0": at.z - half, "z1": at.z + half, "ledge": ledge, "on": ledge + 2}
	return SCENARIOS[name].map(func(s: String) -> String: return s.format(spots))


func _scenario(args: PackedStringArray) -> Dictionary:
	if args.is_empty() or not SCENARIOS.has(args[0].to_lower()):
		return CommandRegistry.fail("Usage: /scenario <%s>" % "|".join(SCENARIOS.keys()))
	var said: Array[String] = []
	var at := Vector3i(player.position.floor()) if player != null else Vector3i.ZERO
	for line in lines(args[0].to_lower(), at):
		var r := reg.execute(line)
		if not r["ok"]:
			return CommandRegistry.fail("%s failed: %s" % [line, r["text"]])
		said.append(r["text"])
	return CommandRegistry.ok("\n".join(said))


func _count(args: PackedStringArray) -> Dictionary:
	if args.size() != 1:
		return CommandRegistry.fail("Usage: /count <material>")
	var mat := GameCommands.material_id(args[0])
	if mat <= 0:
		return CommandRegistry.fail("Unknown material: %s" % args[0])
	var main: Node = main_of.call() if main_of.is_valid() else null
	if main == null or main.bubble == null or main.bubble.sim == null:
		return CommandRegistry.fail("No world")
	return CommandRegistry.ok("%s: %d cells in the sim bubble" % [args[0], main.bubble.sim.count_material(mat)])
