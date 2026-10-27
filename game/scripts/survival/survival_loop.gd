class_name SurvivalLoop
extends Node

## Part G's systems in the game (ADR 0061), set up by Main after Survival, so Main needs one call:
##   - seasons: Seasons.season for the drop tables, frost checks over the farm every frost_check_s;
##   - the prospecting pick: wraps the player's work_use so a right click with a pick on rock reads it;
##   - the discovery journal: Discoveries + its watcher, the book's Discoveries tab;
##   - HUD trackers: Trackers + TrackerHud, the recipe view's Pin and the book's Track buttons;
##   - the diet panel in the inventory screen.
## Saves its own per-world file (journal, trackers, marked veins) beside the quests, every 30 s and on
## leaving the world. Commands: /diet, /eat, /frost, /prospect, /discover, /pin, /track.

var main: Node
var seasons := Seasons.new()
var prospecting := Prospecting.load_default()
var prospect_menu: ProspectMenu   # Shift + right click with a pick (ADR 0091)
var discoveries: Discoveries
var journal: DiscoveryJournal
var trackers: Trackers
var tracker_hud: TrackerHud
var diet_panel: NutritionPanel
var commands: LoopCommands   # held: the registry keeps only Callables, which die with a freed RefCounted
var _frost_t := 0.0
var _diet_follows_panel := false
var _save_t := 30.0
var stats := {"frost_ms_max": 0.0}


func setup(p_main: Node) -> void:
	main = p_main
	var cal: Calendar = main.atmosphere.calendar
	# seasons and frost
	seasons.farming = main.farming
	seasons.temp_at = func(p: Vector3i) -> float:
		var gen: FineGen = main.world.generator.gen
		var cl := float(gen.column_info(p.x, p.z)["temperature"])
		return Survival.environment_c(cl, cal.day_of_year(), cal.year_days(), cal.month_days, cal.time, 0.0, false, false, 0.0)
	seasons.sheltered = func(p: Vector3i) -> bool: return main.survival.room_at(p)
	Seasons.season = cal.season()
	# the discovery journal
	discoveries = Discoveries.load_default(main.items)
	for e in discoveries.errors:
		push_error("data: " + e)
	discoveries.date_text = cal.date_text
	discoveries.clock_days = cal.total
	journal = DiscoveryJournal.new()
	journal.name = "DiscoveryJournal"
	add_child(journal)
	journal.setup(main, discoveries)
	# trackers
	trackers = Trackers.new(main.inv_screen.browser, main.quests)
	tracker_hud = TrackerHud.new()
	tracker_hud.name = "TrackerHud"
	add_child(tracker_hud)
	tracker_hud.minimap = main.get("minimap")   # part D's; null until it lands
	tracker_hud.setup(trackers, main.items, main.player_inv.changed)
	main.quests.quest_completed.connect(func(_id: String) -> void: trackers.prune())
	main.inv_screen.browser_view.trackers = trackers
	var qv: QuestBookView = main.inv_screen.quest_view
	qv.discoveries = discoveries
	qv.trackers = trackers
	# the diet, under the inventory panel: part D's side-panel dock, else a child moved with the panel
	diet_panel = NutritionPanel.new()
	diet_panel.nutrition = main.survival.nutrition
	diet_panel.base_health = Settings.max_health
	if main.inv_screen.has_method("add_side_panel"):
		main.inv_screen.add_side_panel(diet_panel, "below")
	else:
		main.inv_screen._panel.add_child(diet_panel)
		_diet_follows_panel = true
	# the prospecting pick rides on the work system's right click
	var inner: Callable = main.player.work_use
	main.player.work_use = func(hit, first_press: bool, from: Vector3, dir: Vector3, sneaking := false) -> bool:
		var def: Dictionary = main.player_inv.held_def()
		if first_press and def.has("prospects") and sneaking:
			prospect_menu.open(def)
			return true
		if first_press and def.has("prospects") and prospecting.use(main, hit, def, discoveries):
			return true
		return inner.call(hit, first_press, from, dir, sneaking) if inner.is_valid() else false
	_load()
	prospect_menu = ProspectMenu.new()
	prospect_menu.name = "ProspectMenu"
	prospect_menu.main = main
	prospect_menu.prospecting = prospecting
	prospect_menu.can_open = main._can_open_overlay
	add_child(prospect_menu)
	prospect_menu.opened.connect(main._on_overlay_opened)
	prospect_menu.closed.connect(main._on_overlay_closed)
	commands = LoopCommands.new(self)
	commands.register(main.commands)
	for a in OS.get_cmdline_user_args():
		if a.begins_with("pin="):        # debug: pin=<recipe id>[,<recipe id>]
			for rid in a.substr(4).split(",", false):
				trackers.pin("recipe", rid)
		elif a.begins_with("track="):    # debug: track=<quest id>
			trackers.pin("quest", a.substr(6))


func _process(delta: float) -> void:
	if main == null or main.world == null:
		return
	var cal: Calendar = main.atmosphere.calendar
	Seasons.season = cal.season()
	if tracker_hud.minimap == null:
		tracker_hud.minimap = main.get("minimap")   # part D's, whenever it is made
	if _diet_follows_panel:
		diet_panel.position = Vector2(0, diet_panel.get_parent().size.y + 8)
	_frost_t -= delta * DebugCapture.farm_speed
	if _frost_t <= 0.0:
		_frost_t = Settings.frost_check_s
		frost_now()
	_save_t -= delta
	if _save_t <= 0.0:
		_save_t = 30.0
		save()


## One frost check over the farm now. Returns the crops it killed.
func frost_now() -> Array:
	var t0 := Time.get_ticks_usec()
	var killed := seasons.check_frost(main.atmosphere.calendar.total())
	stats["frost_ms_max"] = maxf(stats["frost_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	if not killed.is_empty() and main.get("chat") != null:
		main.chat.add_message("Frost killed %d crop voxels." % killed.size())
	return killed


# ---------------- saving ----------------

func _path() -> String:
	return main._save_path("journal")


func save() -> void:
	if not Settings.persist_world or main == null:
		return
	if not discoveries.dirty and not _trackers_dirty:
		return
	main._save_json(_path(), {"discoveries": discoveries.to_dict(), "trackers": trackers.to_dict(), "prospecting": prospecting.to_dict()})
	discoveries.dirty = false
	_trackers_dirty = false


var _trackers_dirty := false

func _load() -> void:
	trackers.changed.connect(func() -> void: _trackers_dirty = true)
	if not Settings.persist_world:
		return
	var d: Dictionary = main._load_json(_path())
	discoveries.load_dict(d.get("discoveries", {}))
	trackers.load_dict(d.get("trackers", {}))
	prospecting.load_dict(d.get("prospecting", {}))
	_trackers_dirty = false


func _exit_tree() -> void:
	save()


func snapshot() -> Dictionary:
	return {"season": Seasons.season, "seasons": seasons.stats, "frost_ms_max": snappedf(stats["frost_ms_max"], 0.01),
		"discoveries": discoveries.summary(), "journal": journal.stats, "trackers": tracker_hud.summary(),
		"prospecting": {"taps": prospecting.taps, "last": prospecting.last, "marked": prospecting.marked.size()}}
