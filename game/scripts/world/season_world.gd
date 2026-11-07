class_name SeasonWorld
extends Node

## Seasons on the world (ADR 0156), run by SurvivalLoop. GDScript picks the mode; SimWorld.season_pass does the voxels:
##   - winter: while it snows below 0 C a snow layer (145) builds up on open ground; when the day's mean outside is
##     under Settings.ice_form_c still water under the sky freezes to walkable lake ice (146);
##   - the thaw: above Settings.thaw_c the layer goes and the lake ice turns back into still water (never moving
##     water, so a spring wakes no lake);
##   - summer drought: fewer showers, more storms (Weather asks rain_mult / storm_mult), fire spreads more readily
##     (SimWorld.set_dryness) and dry farmland grows nothing (Farming.drought);
##   - the first frost of autumn is announced Settings.frost_warn_days ahead.
## Only the sim bubble gets snow and ice: a few chunk columns per pass (Settings.season_columns every season_pass_s),
## in a shuffled order, static cells marked for the paste and nothing woken, so the bubble never wakes at once.

const SUMMER := 2

var main: Node
var stats := {"passes": 0, "cells": 0, "busy": 0, "pass_ms_max": 0.0, "mode": "", "frost_warned_year": 0}
var _t := 0.0
var _order: Array[Vector2i] = []
var _cursor := 0
var _seed := 0
var _origin := Vector3i(1 << 30, 0, 0)
var _ids := PackedInt32Array()
var _lies_on := PackedByteArray()
var _dry := -1.0
var _warn_day := -1
var _rush_flags := 0         # /season now: the whole bubble over the next frames, Settings.season_rush_ms a frame
var _rush_left := 0
var _rush_cells := 0
var _done := 0               # chunk columns the last sweep got through
var rush_done: Callable      # (cells) called when a rush ends


func setup(p_main: Node) -> void:
	main = p_main
	_ids = PackedInt32Array([GameMaterials.SNOW_LAYER, GameMaterials.LAKE_ICE, GameMaterials.WATER_STATIC])
	_lies_on = lies_on_mask(GameMaterials.defs())


## Which tops snow settles on: whole opaque ground and building blocks; not plants, crops, models, machines, liquids, ice.
static func lies_on_mask(defs: Array) -> PackedByteArray:
	var mask := PackedByteArray()
	mask.resize(256)
	for i in defs.size():
		var d: Dictionary = defs[i]
		var ct: String = d.get("cell_type", "")
		if (ct != "solid" and ct != "powder") or d.get("transparent", false) or d.has("micro") or d.has("model") \
				or d.has("entity") or d.has("interact") or d.has("crop") or d.get("replaceable", false):
			continue
		if d["name"] in ["snow_layer", "lake_ice", "ice", "packed_ice"]:
			continue
		if i + 1 < 256:
			mask[i + 1] = 1
	return mask


## The day's mean outside temperature at the player (the hour's swing taken out: ice follows the season, not the night).
func mean_c() -> float:
	var cal: Calendar = main.atmosphere.calendar
	return Survival.environment_c(_climate(), cal.day_of_year(), cal.year_days(), cal.month_days, 0.33, 0.0, false, false, 0.0)


func _climate() -> float:
	var gen = main.world.generator.gen if main.world and main.world.generator else null
	if gen == null:
		return 0.0
	var p: Vector3 = main.player.position
	return float(gen.column_info(int(p.x), int(p.z)).get("temperature", 0.0))


## The pass flags now (SimWorld.season_pass): 1 snow (chance in bits 8..15), 2 freeze, 4 thaw.
static func flags_for(mean: float, outside: float, snowing: bool) -> int:
	if mean > Settings.thaw_c:
		return 4
	var f := 0
	if mean < Settings.ice_form_c:
		f |= 2
	if snowing and outside < 0.0:
		f |= 1 | (clampi(roundi(Settings.snow_stick_chance * 255.0), 0, 255) << 8)
	return f


func _process(delta: float) -> void:
	if main == null or main.world == null or main.player == null or main.bubble == null:
		return
	var season: int = main.atmosphere.calendar.season()
	var raining: bool = main.weather != null and main.weather.is_raining()
	# drought: fire catches easier in a dry summer
	var dry := Settings.drought_fire_mult if season == SUMMER and not raining else 1.0
	if main.farming:
		main.farming.drought = season == SUMMER and not raining
	var bubble: SimBubble = main.bubble
	if bubble.sim and dry != _dry and bubble.sim.has_method("set_dryness"):
		bubble.sim.set_dryness(dry)
		_dry = dry
	if _rush_left != 0:
		_rush_cells += sweep(_rush_flags, _rush_left if _rush_left > 0 else 1 << 20, Settings.season_rush_ms)
		if _rush_left < 0 and not _order.is_empty():
			_rush_left = _order.size()
		if _rush_left > 0:
			_rush_left = maxi(0, _rush_left - _done)
		if _rush_left == 0 and rush_done.is_valid():
			rush_done.call(_rush_cells)
		return
	_t -= delta
	if _t > 0.0:
		return
	_t = Settings.season_pass_s
	_frost_warning()
	var flags := flags_for(mean_c(), float(main.survival.outside_c), raining)
	stats["mode"] = ("snow " if flags & 1 else "") + ("freeze " if flags & 2 else "") + ("thaw" if flags & 4 else "")
	if flags != 0:
		sweep(flags, Settings.season_columns)


## Lays or clears the season over the whole bubble within a few frames (/season now), never more than
## Settings.season_rush_ms a frame (all at once was a 45 ms frame).
func rush(flags: int) -> void:
	_rush_flags = flags
	_rush_cells = 0
	_rush_left = -1   # the whole order, once the first sweep has built it


## Runs the season over the next `columns` chunk columns of the bubble, stopping early once budget_ms (> 0) has
## gone; _done says how many it got through. Returns the cells changed.
func sweep(flags: int, columns: int, budget_ms := 0.0) -> int:
	_done = 0
	var bubble: SimBubble = main.bubble
	if bubble == null or bubble.sim == null or not bubble.sim.has_method("season_pass") or not bubble.can_sample():
		return 0
	if bubble.origin != _origin or _order.is_empty():
		_origin = bubble.origin
		var size: Vector3i = bubble.sim.get_size_voxels()
		_order.clear()
		for cz in size.z / Settings.CHUNK:
			for cx in size.x / Settings.CHUNK:
				_order.append(Vector2i(cx, cz))
		var rng := RandomNumberGenerator.new()
		rng.seed = 156
		for i in range(_order.size() - 1, 0, -1):   # a fixed shuffle: the snow lands here and there, not in rows
			var j := rng.randi_range(0, i)
			var tmp := _order[i]
			_order[i] = _order[j]
			_order[j] = tmp
		_cursor = 0
	var n := mini(columns, _order.size())
	var t0 := Time.get_ticks_usec()
	var changed := 0
	for i in n:
		if budget_ms > 0.0 and i > 0 and Time.get_ticks_usec() - t0 > budget_ms * 1000.0:
			break
		_seed += 1
		var r: int = bubble.sim.season_pass(_order[_cursor], flags, _ids, _lies_on, _seed)
		if r < 0:
			stats["busy"] += 1
			break
		changed += r
		_done += 1
		_cursor = (_cursor + 1) % _order.size()
	if n == 0:
		return 0
	stats["passes"] += 1
	stats["cells"] += changed
	stats["pass_ms_max"] = maxf(stats["pass_ms_max"], (Time.get_ticks_usec() - t0) / 1000.0)
	return changed


## Days until the first frost night at the player's column within Settings.frost_warn_days (a night under 0 C after a
## frost-free one), or -1.
func frost_in_days() -> int:
	var cal: Calendar = main.atmosphere.calendar
	var cl := _climate()
	var night := func(ahead: int) -> float:
		return Survival.environment_c(cl, (cal.day_of_year() + ahead) % cal.year_days(), cal.year_days(), cal.month_days, 0.08, 0.0, false, false, 0.0)
	if night.call(0) < 0.0:
		return -1   # frost is already here
	for d in range(1, Settings.frost_warn_days + 1):
		if night.call(d) < 0.0:
			return d
	return -1


func _frost_warning() -> void:
	var cal: Calendar = main.atmosphere.calendar
	if cal.day == _warn_day or cal.season() != 3:
		return
	_warn_day = cal.day
	if int(stats["frost_warned_year"]) == cal.year():
		return
	var d := frost_in_days()
	if d > 0 and main.get("chat") != null:
		stats["frost_warned_year"] = cal.year()
		main.chat.add_message("The nights are turning: first frost in about %d day%s. Harvest tender crops or roof them over." % [d, "" if d == 1 else "s"])
