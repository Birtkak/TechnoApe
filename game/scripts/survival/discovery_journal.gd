class_name DiscoveryJournal
extends Node

## Watches the player for first finds (ADR 0061) and notes them in `Discoveries`:
##   biome / landform: the column under the player (FineGen.column_info, "landform" once part B adds it);
##   landmark: a ruin within 20 m, a waterfall within 20 m, standing in a cave (6 m and more under the
##             surface, not in water), on a river, lake, spring or marsh;
##   animal: a living animal of a species within 16 m;
##   ore / stone: an item of that entry in the inventory (picked up, mined or crafted);
##   vein: a prospecting pick's exact reading (Prospecting.use).
## Each find shows a toast and a chat line. Cheap: one column read a second, a ring search every 5 s.

const ANIMAL_M := 16.0
const LANDMARK_M := 20.0
const CAVE_M := 6.0

var main: Node
var book: Discoveries
var _t := 0.0
var _slow_t := 0.0
var stats := {"checks": 0, "noted": 0}


func setup(p_main: Node, p_book: Discoveries) -> void:
	main = p_main
	book = p_book
	book.discovered.connect(_on_discovered)
	main.player_inv.changed.connect(check_inventory)


func _process(delta: float) -> void:
	if main == null or main.player == null or main.world == null:
		return
	_t -= delta
	if _t > 0.0:
		return
	_t = Settings.discovery_check_s
	_slow_t -= Settings.discovery_check_s
	check_here(_slow_t <= 0.0)
	if _slow_t <= 0.0:
		_slow_t = 5.0


## Looks around the player; `slow` adds the ring searches (ruins, waterfalls).
func check_here(slow := true) -> void:
	stats["checks"] += 1
	var p: Vector3 = main.player.position
	var x := floori(p.x)
	var z := floori(p.z)
	var gen: FineGen = main.world.generator.gen
	var info: Dictionary = gen.column_info(x, z)
	var below := 0.0
	if main.survival != null:
		below = float(main.survival.underground_m)
	if below < CAVE_M:
		book.note("biome:" + String(info.get("biome_name", "")))
		var lf := String(info.get("landform", ""))
		if lf != "":
			book.note("landform:" + lf)
		if float(info.get("river", 0.0)) > 0.3:
			book.note("landmark:river")
		if float(info.get("lake", 0.0)) > 0.3:
			book.note("landmark:lake")
		if float(info.get("spring", 0.0)) > 0.3:
			book.note("landmark:spring")
		if float(info.get("marsh", 0.0)) > 0.3:
			book.note("landmark:marsh")
	elif not main.player.in_liquid:
		book.note("landmark:cave")
	if main.mobs != null:
		var r2 := Settings.m2v(ANIMAL_M) * Settings.m2v(ANIMAL_M)
		for m in main.mobs.active():
			if m.dying <= 0.0 and m.position.distance_squared_to(p) < r2:
				book.note("animal:" + m.species)
	if slow:
		var lm := Settings.m2v(LANDMARK_M)
		if not book.is_found("landmark:ruin"):
			var r: Vector3i = gen.find_nearest("ruin", x, z, int(lm))
			if r.y >= 0 and Vector2(r.x - x, r.z - z).length() <= lm:
				book.note("landmark:ruin")
		if not book.is_found("landmark:waterfall") and not gen.waterfalls_near(x, z, int(lm)).is_empty():
			book.note("landmark:waterfall")


## Ores, minerals and stones count once one is held.
func check_inventory() -> void:
	var inv: PlayerInventory = main.player_inv
	for i in inv.inv.size():
		var s: Dictionary = inv.inv.get_slot(i)
		if not s.is_empty():
			_note_item(String(s["id"]))
	if not inv.cursor.is_empty():
		_note_item(String(inv.cursor["id"]))


func _note_item(id: String) -> void:
	for k in ["ore:", "stone:"]:
		if book.has(k + id):
			book.note(k + id)


func _on_discovered(id: String) -> void:
	stats["noted"] += 1
	var e: Dictionary = book.entries[id]
	var kind := ""
	for k in book.kinds:
		if k["id"] == e["kind"]:
			kind = k["title"]
	if main.get("quest_toast") != null:
		main.quest_toast.push(e["icon"], e["title"], false, "Discovered!")
	if main.get("chat") != null:
		main.chat.add_message("Discovered (%s): %s. %s" % [kind, e["title"], e["text"]])
