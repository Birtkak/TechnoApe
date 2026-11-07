class_name PlaytestCommands
extends RefCounted

## Shortcuts for the playtest list (docs/plans/to-test.md, ADR 0018), registered by GameCommands:
##   /totest [count]       the newest unticked items of the list
##   /scenario <name>      jump into a set-up test: a few existing commands run in a row
##   /count <material>     cells of a material in the sim bubble (how much steam a vent leaves, gas in a cave ...)
##   /blocks [kind]        every block entity's state on one line each (what each machine draws and has made)
##   /whatsnew             today's additions (data/whats_new.json) in chat, with a button each on top of the F4 menu
##   /tour [next|back|<n>|<id>]   walks those additions in order: says what to look for, runs the entry's lines
## A scenario or a tour stop is only command lines, so each step also works typed and reports in chat.

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
	# ADR 0148: a sealed stone room 3 m ahead, its top half firedamp; the lamp, torches, a fan and power to see it solved
	"firedamp": ["/gamemode creative", "/fill {fx0} {y0} {fz0} {fx1} {y7} {fz1} stone", "/fill {fx0i} {y1} {fz0i} {fx1i} {y6} {fz1i} air",
		"/fill {fx0i} {y4} {fz0i} {fx1i} {y6} {fz1i} firedamp", "/give safety_lamp", "/give torch 4", "/give vent_fan", "/kit electric"],
	"contraptions": ["/gamemode creative", "/kit contraptions", "/contraptions"],   # a lift, a drawbridge, a crane on cranks (ADR 0160)
	"fuse": ["/gamemode creative", "/kit tnt", "/tnt fuse 4"],   # look at open ground first: a cord burns 4 m into a charge (ADR 0150)
	"dam": ["/gamemode creative", "/stations dam cam"],   # a wheel in a race and a thin dirt dam: it seeps, creaks, breaks (ADR 0154)
	"ledge": ["/gamemode survival", "/fill {x0} {ledge} {z0} {x1} {ledge} {z1} stone", "/tp {x} {on} {z}"],   # Ctrl sprint, Shift stops at the edge
}

## What landed lately, in tour order; any lane appends to it (ADR 0130).
const WHATS_NEW := "res://data/whats_new.json"

var reg: CommandRegistry
var player: Player
var main_of := Callable()   # () -> Main or null
var tour_at := -1   # the last /tour stop shown


func register(p_reg: CommandRegistry, p_player: Player, p_main_of := Callable()) -> void:
	reg = p_reg
	player = p_player
	main_of = p_main_of
	reg.register("totest", "/totest [count]", "The newest unticked items of the playtest list (docs/plans/to-test.md)", _totest)
	reg.register("scenario", "/scenario <%s>" % "|".join(SCENARIOS.keys()), "Jump into a playtest set-up from the to-test list", _scenario, [SCENARIOS.keys()])
	reg.register("blocks", "/blocks [kind]", "Each block entity's state, one line each (watts, amps, made, output); kind filters by name", _blocks)
	reg.register("machinefx", "/machinefx [force|off|film [frames] [name]]", "Running-machine effects (ADR 0153): force runs every machine near for the look and the cost, off back to real work, film saves a burst to out/machine_fx; no argument shows the counts", _machinefx, [["force", "off", "film"]])
	reg.register("count", "/count <material>", "Cells of a material in the sim bubble (steam left by a vent, gas pooled in a cave)", _count)
	reg.register("whatsnew", "/whatsnew", "What landed lately, each one a click away (the F4 menu's top row); /tour walks them in order", _whatsnew)
	reg.register("tour", "/tour [next|back|<n>|<id>]", "The next stop of the what's-new tour: what to look for, and it takes you there or hands you the kit", _tour,
		[func() -> Array: return ["next", "back"] + whats_new().map(func(e: Dictionary) -> String: return e["id"])])


## The entries of data/whats_new.json: {id, title, what, run: [command lines], date}.
static func whats_new() -> Array:
	var d: Variant = JSON.parse_string(FileAccess.get_file_as_string(WHATS_NEW))
	return d.get("entries", []) if d is Dictionary else []


func _whatsnew(_args: PackedStringArray) -> Dictionary:
	var e := whats_new()
	if e.is_empty():
		return CommandRegistry.fail("Nothing in %s" % WHATS_NEW)
	var main: Node = main_of.call() if main_of.is_valid() else null
	if main != null and main.get("debug_menu") != null:
		main.debug_menu.open.call_deferred(true)   # after the chat has closed, as /tools does
	var lines: Array[String] = []
	for i in e.size():
		lines.append("%d. %s  (/tour %s)" % [i + 1, e[i]["title"], e[i]["id"]])
	return CommandRegistry.ok("What's new (the buttons on top of F4; /tour walks them in order):\n" + "\n".join(lines))


func _tour(args: PackedStringArray) -> Dictionary:
	var e := whats_new()
	if e.is_empty():
		return CommandRegistry.fail("Nothing in %s" % WHATS_NEW)
	var a := args[0].to_lower() if args.size() > 0 else "next"
	var i := tour_at + 1
	if a == "back":
		i = tour_at - 1
	elif a.is_valid_int():
		i = int(a) - 1
	elif a != "next":
		i = e.find_custom(func(x: Dictionary) -> bool: return x["id"] == a)
	if a == "next" and i >= e.size():
		tour_at = -1
		return CommandRegistry.ok("That was the whole tour: /tour starts it again, /whatsnew lists it")
	if i < 0 or i >= e.size():
		return CommandRegistry.fail("Usage: /tour [next|back|<1..%d>|<id>]" % e.size())
	tour_at = i
	var said: Array[String] = ["Tour %d/%d: %s. %s" % [i + 1, e.size(), e[i]["title"], e[i]["what"]]]
	for line in e[i]["run"]:
		var r := reg.execute(line)
		said.append(r["text"] if r["ok"] else "%s failed: %s" % [line, r["text"]])   # a biome too far away skips one line, not the stop
	said.append("/tour for the next: %s" % e[i + 1]["title"] if i + 1 < e.size() else "That was the last stop")
	return CommandRegistry.ok("\n".join(said.filter(func(s: String) -> bool: return s != "")))


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
	var fz := at.z + int(Settings.m2v(3.0))   # the firedamp room: 3 m ahead along +z, 6 m wide, 3 m high, one-voxel walls
	spots.merge({"fx0": at.x - half, "fx1": at.x + half, "fz0": fz, "fz1": fz + 2 * half, "fx0i": at.x - half + 1, "fx1i": at.x + half - 1,
		"fz0i": fz + 1, "fz1i": fz + 2 * half - 1, "y0": at.y, "y1": at.y + 1, "y4": at.y + half / 2, "y6": at.y + half, "y7": at.y + half + 1})
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


func _machinefx(args: PackedStringArray) -> Dictionary:
	var main: Node = main_of.call() if main_of.is_valid() else null
	if main == null or main.get("machine_fx") == null:
		return CommandRegistry.fail("No machine effects")
	if args.size() > 0 and args[0] == "film":   # a burst of consecutive frames (run with --fixed-fps 60) to watch the motion
		var path := "res://out/machine_fx/%s.png" % (args[2] if args.size() > 2 else "burst")
		FrameSheet.burst(main.get_viewport(), path, int(args[1]) if args.size() > 1 else 16, 3, 640, 360, 4)   # every 3rd frame, large
		return CommandRegistry.ok("filming " + path)
	if args.size() > 0:
		main.machine_fx.force = args[0] == "force"
	return CommandRegistry.ok("machine fx%s: %s" % [" (forced)" if main.machine_fx.force else "", JSON.stringify(main.machine_fx.stats)])


func _count(args: PackedStringArray) -> Dictionary:
	if args.size() != 1:
		return CommandRegistry.fail("Usage: /count <material>")
	var mat := GameCommands.material_id(args[0])
	if mat <= 0:
		return CommandRegistry.fail("Unknown material: %s" % args[0])
	var main: Node = main_of.call() if main_of.is_valid() else null
	if main == null or main.bubble == null or main.bubble.sim == null:
		return CommandRegistry.fail("No world")
	var text := "%s: %d cells in the sim bubble" % [args[0], main.bubble.sim.count_material(mat)]
	if GameMaterials.is_liquid(mat):   # the volume too: a spreading liquid splits into more, thinner cells
		text += " (%d eighths)" % main.bubble.sim.count_liquid_eighths(mat)
	return CommandRegistry.ok(text)


## Keys of an entity summary worth a line in chat; the rest (inventories, positions) stays in the F3 / stats JSON.
const BLOCK_KEYS := ["watts", "amps", "volts", "open", "blown", "tripped", "burnt", "made", "smelted", "progress", "rpm", "pressure", "output"]


static func block_line(e: Dictionary) -> String:
	var parts: Array[String] = [String(e.get("kind", "?"))]
	for k in BLOCK_KEYS:
		if not e.has(k):
			continue
		var v: Variant = e[k]
		if v is float:
			v = "%.1f" % v
		elif v is Dictionary:
			v = "%s x%d" % [v.get("id", "-"), int(v.get("count", 0))] if not v.is_empty() else "-"
		parts.append("%s %s" % [k, v])
	return " ".join(parts)


func _blocks(args: PackedStringArray) -> Dictionary:
	var main: Node = main_of.call() if main_of.is_valid() else null
	if main == null or main.blocks == null:
		return CommandRegistry.fail("No world")
	var want := args[0].to_lower() if args.size() > 0 else ""
	var out: Array[String] = []
	for p in main.blocks._by_pos:
		var e: Dictionary = main.blocks._by_pos[p].summary()
		if want == "" or String(e.get("kind", "")).contains(want):
			out.append(block_line(e))
	if out.is_empty():
		return CommandRegistry.ok("No block entities" + (" like %s" % want if want != "" else ""))
	return CommandRegistry.ok("%d block entities:
%s" % [out.size(), "
".join(out)])
