class_name DebugHud
extends CanvasLayer

## F3-style overlay. Everything shown here is also available as JSON through
## DebugCapture so Claude can read it without eyes.

var label: Label
var crosshair: Label
var main: Node
var visible_hud := true


func _ready() -> void:
	label = Label.new()
	label.position = Vector2(8, 8)
	label.add_theme_font_size_override("font_size", 14)
	label.add_theme_color_override("font_color", Color(1, 1, 1))
	label.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.8))
	label.add_theme_constant_override("shadow_offset_x", 1)
	label.add_theme_constant_override("shadow_offset_y", 1)
	add_child(label)
	crosshair = Label.new()
	crosshair.text = "+"
	crosshair.add_theme_font_size_override("font_size", 22)
	crosshair.set_anchors_preset(Control.PRESET_CENTER)
	crosshair.position = Vector2(-6, -14)
	add_child(crosshair)


func _process(_delta: float) -> void:
	if not visible_hud or main == null:
		return
	var s: Dictionary = main.snapshot()
	var lines := [
		"TechnoApe dev  |  %d fps  frame %.2f ms  |  seed %d%s" % [s["fps"], s["frame_ms"], Settings.world_seed, (" (\"%s\")" % Settings.world_seed_text) if Settings.world_seed_text != "" else ""],
		"pos %s m  voxel %s  chunk %s" % [s["pos_m"], s["pos_voxel"], s["chunk"]],
		"mode %s   F fly  F3 hud  Esc mouse  LMB break  RMB place" % s["mode"],
		"terrain: lod0 %d m, view %d m, lods %d | stats %s" % [Settings.render_distance_m, Settings.view_distance_m, Settings.lod_count, s["terrain"]],
		"sim: %s" % str(s["sim"]),
		"look: %s" % str(s["look"]),
		"mem %.0f MB  draw calls %d  prims %d" % [s["mem_mb"], s["draw_calls"], s["primitives"]],
	]
	label.text = "\n".join(lines)


func toggle() -> void:
	visible_hud = not visible_hud
	label.visible = visible_hud
