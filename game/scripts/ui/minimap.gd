class_name Minimap
extends Control

## The HUD minimap (plan item D10): a square, north-up MapView of the explored map around the player
## with waypoints (off-view ones on the rim), and under it the voxel coordinates, the biome, the season with the outside temperature, and the date and time in small
## text. Corner and size come from Settings (minimap_corner, minimap_size_px; the Settings screen);
## redrawn 10 times a second, not every frame. With F3 open it moves down below F3's text.

const MARGIN := 10.0
const TEXT_H := 68.0
const F3_DROP := 280.0
var _f3 := false
var main: Node
var view: MapView
var _text: Label
var _t := 0.0
var _biome_t := 0.0
var _biome := ""


func setup(p_main: Node) -> void:
	main = p_main
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	view = MapView.new()
	view.edge_pins = true
	add_child(view)
	view.world_map = main.world_map
	view.player = main.player
	_text = Label.new()
	_text.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_text.add_theme_font_size_override("font_size", 12)
	_text.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.9))
	_text.add_theme_constant_override("outline_size", 4)
	_text.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	add_child(_text)
	main.world_map.waypoints.changed.connect(view.refresh)
	Settings.user_settings_changed.connect(_place)
	_place()


## The screen y just under the minimap and its text (0 when hidden or not on the right): other HUD
## pieces on the right edge start below it (part G's trackers).
func bottom_y() -> float:
	if not visible or not Settings.minimap_corner.ends_with("right") or Settings.minimap_corner.begins_with("bottom"):
		return 0.0
	return global_position.y + size.y + 6.0


## Corner and size from Settings.
func _place() -> void:
	var s := float(Settings.minimap_size_px)
	view.size = Vector2(s, s)
	_text.position = Vector2(0, s + 2)
	_text.size = Vector2(s, TEXT_H)
	size = Vector2(s, s + TEXT_H)
	var vp := get_viewport_rect().size if is_inside_tree() else Vector2(1280, 720)
	var right := Settings.minimap_corner.ends_with("right")
	var bottom := Settings.minimap_corner.begins_with("bottom")
	var top := MARGIN + (F3_DROP if _f3 else 0.0)   # below F3's text columns while it is open
	position = Vector2(vp.x - s - MARGIN if right else MARGIN, vp.y - size.y - 70.0 if bottom else top)
	visible = Settings.minimap_size_px > 0


func _process(delta: float) -> void:
	if main == null or main.player == null:
		return
	_f3 = main.get("hud") != null and main.hud.visible_hud
	visible = Settings.minimap_size_px > 0
	if not visible:
		return
	_t -= delta
	if _t > 0.0:
		return
	_t = 0.1
	_place()
	var p: Vector3 = main.player.position
	view.center = Vector2(p.x, p.z)
	view.refresh()
	_biome_t -= 0.1
	if _biome_t <= 0.0:
		_biome_t = 1.0
		var info: Dictionary = main.world.generator.column_info(int(p.x), int(p.z)) if main.world and main.world.generator else {}
		_biome = String(info.get("biome_name", "")).replace("_", " ")
	var cal: Calendar = main.atmosphere.calendar
	var hours := cal.time * 24.0
	_text.text = "%d  %d  %d\n%s\n%s  %d C\n%d %s  %02d:%02d\n%s" % [int(floor(p.x)), int(floor(p.y)), int(floor(p.z)), _biome,
		cal.season_name(), roundi(main.survival.outside_c) if main.survival else 0, cal.day_of_month() + 1, Calendar.MONTHS[cal.month()],
		int(hours), int(fmod(hours, 1.0) * 60.0), _cell_line(p)]


## ADR 0119 D: the aimed cell, else the cell the player stands in: material, state, temperature and pressure, so
## a player sees why water boils or steam rains. Empty outside the simulation window.
func _cell_line(p: Vector3) -> String:
	if main.bubble == null or main.player == null or main.world == null:
		return ""
	var cell := Vector3i(p.floor())
	var hit = main.world.raycast(main.player.camera.global_position, -main.player.camera.global_transform.basis.z, Settings.m2v(Settings.reach_m))
	if hit:
		cell = hit.position
	var info: Dictionary = main.bubble.cell_info(cell)
	if info.is_empty():
		return ""
	var mat := int(info.get("material", 0))
	var mname := "air" if mat == 0 else GameMaterials.name_of(mat).replace("_", " ")
	var state: String = ["solid", "liquid", "gas", "fire", "powder"][clampi(int(info.get("cell_type", 0)), 0, 4)]
	if bool(info.get("still", false)):
		state = "still " + state
	elif int(info.get("eighths", 0)) > 0:
		state = "%s %d/8" % [state, int(info["eighths"])]
	if bool(info.get("burning", false)):
		state += " burning"
	if mat == 0:
		return "air  %d C  p %.2f" % [int(info.get("temp_c", 0)), float(info.get("pressure", 0.0))]
	return "%s (%s)  %d C  p %.2f" % [mname, state, int(info.get("temp_c", 0)), float(info.get("pressure", 0.0))]
