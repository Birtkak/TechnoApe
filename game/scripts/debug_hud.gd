class_name DebugHud
extends CanvasLayer

## F3 debug screen laid out like Minecraft's (ADR 0018): game and world facts
## on the left, system and the targeted block on the right, each line on a
## grey strip; an RGB axis crosshair in the middle (X red = east, Y green =
## up, Z blue = south) and a frame-time graph bottom right. Text refreshes at
## 10 Hz, the graph every frame. The same facts are in DebugCapture's JSON
## (Main.snapshot()), so nothing here needs eyes.

const REFRESH_S := 0.1
const GRAPH_N := 200
const GRAPH_H := 70.0
const FONT_SIZE := 14
const LINE_H := 17.0
const TARGET_RANGE_M := 20.0   # vanilla's targeted-block range is 20 blocks
const STRIP := Color(0.31, 0.31, 0.31, 0.56)   # vanilla 0x90505050
const TEXT := Color(0.88, 0.88, 0.88)

var main: Node
var visible_hud := true
var _left: Array[String] = []
var _right: Array[String] = []
var _text_view: Control
var _graph: Control
var _axes: Control
var _font: Font
var _accum := 0.0
var _frames := PackedFloat32Array()   # ring buffer of frame times (ms), filled even while hidden
var _frame_i := 0
var _last_us := 0


func _ready() -> void:
	layer = 6   # above the game HUD (5), below chat (8)
	_font = ThemeDB.fallback_font
	_frames.resize(GRAPH_N)
	for c in 3:
		var ctl := Control.new()
		ctl.set_anchors_preset(Control.PRESET_FULL_RECT)
		ctl.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(ctl)
		match c:
			0:
				_text_view = ctl
				ctl.draw.connect(_draw_text)
			1:
				_graph = ctl
				ctl.draw.connect(_draw_graph)
			2:
				_axes = ctl
				ctl.draw.connect(_draw_axes)


func toggle() -> void:
	visible_hud = not visible_hud
	visible = visible_hud
	if main != null and main.get("game_hud") != null:
		main.game_hud.set_crosshair_visible(not visible_hud)
	if visible_hud:
		_accum = REFRESH_S


## Recent frame times in ms, oldest first (bug reports).
func frame_times() -> Array:
	var n := mini(_frame_i, GRAPH_N)
	var out := []
	for k in n:
		out.append(snappedf(_frames[(_frame_i - n + k) % GRAPH_N], 0.01))
	return out


func _process(delta: float) -> void:
	var now := Time.get_ticks_usec()
	if _last_us > 0:
		_frames[_frame_i % GRAPH_N] = (now - _last_us) / 1000.0
		_frame_i += 1
	_last_us = now
	if not visible_hud or main == null or main.get("player") == null:
		return
	_accum += delta
	if _accum >= REFRESH_S:
		_accum = 0.0
		_collect()
		_text_view.queue_redraw()
	_graph.queue_redraw()
	_axes.queue_redraw()


# ---------------- what is shown ----------------

func _collect() -> void:
	var p: Player = main.player
	var pos := p.position
	var v := Vector3i(pos.floor())
	var chunk := Vector3i((pos / Settings.CHUNK).floor())
	var local := v - chunk * Settings.CHUNK
	var cam := p.camera
	var fwd := -cam.global_transform.basis.z
	var info: Dictionary = main.world.generator.column_info(v.x, v.z)
	var ver := Engine.get_version_info()
	var times := frame_times().slice(-60)
	var avg := 0.0
	var worst := 0.0
	for t in times:
		avg += t
		worst = maxf(worst, t)
	avg /= maxf(times.size(), 1)
	var sim: Dictionary = main.bubble.stats()
	var atmo: Node = main.atmosphere
	var hours := fposmod(atmo.time_of_day * 24.0, 24.0)
	var inv: PlayerInventory = main.player_inv
	var held := inv.held()
	var states: Array[String] = ["flying" if p.flying else "walking"]
	if p.is_on_floor():
		states.append("on floor")
	if p.in_liquid:
		states.append("swimming")
	_left = [
		"TechnoApe dev (Godot %s, %s, render thread %s)" % [ver["string"], ProjectSettings.get_setting("rendering/renderer/rendering_method", "?"), _thread_mode()],
		"%d fps  %.2f ms avg  %.2f ms worst (last 60)  vsync %s" % [Engine.get_frames_per_second(), avg, worst,
			"off" if DisplayServer.window_get_vsync_mode() == DisplayServer.VSYNC_DISABLED else "on"],
		"Sim: tick %.2f ms, %d active chunks, %d burning, push queue %d, %d loads pending" % [
			float(sim.get("tick_ms", 0.0)), int(sim.get("active_chunks", 0)), int(sim.get("burning", 0)),
			int(sim.get("push_queue", 0)), int(sim.get("pending_loads", 0))],
		"C: view %d m (one full-detail box, no LOD), mesh block %d" % [Settings.view_distance_m, Settings.terrain_mesh_block],
		"E: %d items, %d block entities" % [main._item_entities.size(), main.blocks.size()],
		"",
		"XYZ: %.3f / %.3f / %.3f  (%.1f / %.1f / %.1f m)" % [pos.x, pos.y, pos.z, Settings.v2m(pos.x), Settings.v2m(pos.y), Settings.v2m(pos.z)],
		"Block: %d %d %d" % [v.x, v.y, v.z],
		"Chunk: %d %d %d in %d %d %d" % [local.x, local.y, local.z, chunk.x, chunk.y, chunk.z],
		"Facing: %s (%.1f / %.1f)" % [_facing(fwd), rad_to_deg(p._yaw), rad_to_deg(p._pitch)],
		"Biome: %s" % info.get("biome_name", "?"),
		"Column: height %d, water %s, temp %.2f, humidity %.2f, river %.2f, slope %.2f" % [
			int(info.get("height", 0)), "none" if int(info.get("water_level", -1)) < 0 else str(int(info["water_level"])), float(info.get("temperature", 0.0)),
			float(info.get("humidity", 0.0)), float(info.get("river", 0.0)), float(info.get("slope", 0.0))],
		"Time: %02d:%02d (%.3f), day cycle %s" % [int(hours), int(fmod(hours, 1.0) * 60.0), atmo.time_of_day,
			"frozen" if atmo.day_length_s <= 0.0 else "%d s" % int(atmo.day_length_s)],
		_sky_line(atmo.calendar),
		"Seed: %d%s" % [Settings.world_seed, (" (\"%s\")" % Settings.world_seed_text) if Settings.world_seed_text != "" else ""],
		"Mode: %s, %s" % [p.game_mode, ", ".join(states)],
		"Health: %d / %d" % [inv.health, inv.max_health],
		"Held: %s" % ("nothing" if held.is_empty() else "%s x%d" % [held["id"], held["count"]]),
		"",
		"F3 this screen   F4 playtest menu   F9 bug report   F2 screenshot   T chat   / commands",
	]
	_right = [
		"Godot %s  %s %s" % [ver["string"], OS.get_name(), OS.get_version()],
		"Mem: %d MB static" % int(OS.get_static_memory_usage() / 1048576.0),
		"CPU: %dx %s" % [OS.get_processor_count(), OS.get_processor_name()],
		"",
		"Display: %dx%d" % [get_viewport().get_visible_rect().size.x, get_viewport().get_visible_rect().size.y],
		RenderingServer.get_video_adapter_name(),
		"%s %s" % [RenderingServer.get_video_adapter_vendor(), RenderingServer.get_video_adapter_api_version()],
		"VRAM: %d MB" % int(Performance.get_monitor(Performance.RENDER_VIDEO_MEM_USED) / 1048576.0),
		"Draw calls %d, primitives %.2f M" % [Performance.get_monitor(Performance.RENDER_TOTAL_DRAW_CALLS_IN_FRAME),
			Performance.get_monitor(Performance.RENDER_TOTAL_PRIMITIVES_IN_FRAME) / 1e6],
		"",
	]
	_right.append_array(_target_lines(cam))


func _target_lines(cam: Camera3D) -> Array[String]:
	var out: Array[String] = []
	var hit: VoxelRaycastResult = main.world.raycast(cam.global_position, -cam.global_transform.basis.z, Settings.m2v(TARGET_RANGE_M))
	if hit == null:
		return out
	var t: Vector3i = hit.position
	var m: int = main.get_world_voxel(t)
	var name := GameMaterials.name_of(m)
	out.append("Targeted Block: %d, %d, %d" % [t.x, t.y, t.z])
	out.append("%s (id %d), %.1f m" % [name, m, Settings.v2m(hit.distance)])
	if m > 0:
		var d: Dictionary = GameMaterials.defs()[m - 1]
		var props: Array[String] = []
		for k in d:
			if k in ["name", "tiles", "micro", "color"] or typeof(d[k]) == TYPE_ARRAY or typeof(d[k]) == TYPE_DICTIONARY:
				continue
			props.append("%s: %s" % [k, str(d[k])])
		for i in range(0, props.size(), 3):
			out.append(", ".join(props.slice(i, i + 3)))
	var e: Dictionary = main.mining.entry(name)
	if e.is_empty():
		out.append("unbreakable")
	else:
		out.append("hardness %s, tool %s, drops %s" % [str(e.get("hardness", "?")), str(e.get("tool", "any")), str(e.get("drops", "nothing"))])
	var ent: RefCounted = main.blocks.at(t)
	if ent != null and ent.has_method("summary"):
		out.append("entity: %s" % str(ent.summary()).left(80))
	return out


static func _facing(f: Vector3) -> String:
	if absf(f.x) > absf(f.z):
		return "east (Towards positive X)" if f.x > 0.0 else "west (Towards negative X)"
	return "south (Towards positive Z)" if f.z > 0.0 else "north (Towards negative Z)"


static func _thread_mode() -> String:
	var args := OS.get_cmdline_args()
	var i := args.find("--render-thread")
	if i >= 0 and i + 1 < args.size():
		return args[i + 1]
	return "separate" if int(ProjectSettings.get_setting("rendering/driver/threads/thread_model", 1)) == 2 else "safe"


# ---------------- drawing ----------------

func _draw_text() -> void:
	var w := _text_view.size.x
	var y := 4.0
	for line in _left:
		_strip(line, 4.0, y, false)
		y += LINE_H
	y = 4.0
	for line in _right:
		_strip(line, w - 4.0, y, true)
		y += LINE_H


func _strip(s: String, x: float, y: float, right: bool) -> void:
	if s == "":
		return
	var sw := _font.get_string_size(s, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE).x
	var x0 := x - sw if right else x
	_text_view.draw_rect(Rect2(x0 - 2.0, y, sw + 4.0, LINE_H), STRIP)
	_text_view.draw_string(_font, Vector2(x0, y + LINE_H - 4.0), s, HORIZONTAL_ALIGNMENT_LEFT, -1, FONT_SIZE, TEXT)


## Bars per frame: green within the 8.3 ms budget (ADR 0009), yellow to 16.7, red above.
func _draw_graph() -> void:
	var vs := _graph.size
	var bw := 2.0
	var w := GRAPH_N * bw
	var o := Vector2(vs.x - w - 8.0, vs.y - GRAPH_H - 8.0)
	_graph.draw_rect(Rect2(o, Vector2(w, GRAPH_H)), Color(0, 0, 0, 0.45))
	var n := mini(_frame_i, GRAPH_N)
	var px_per_ms := GRAPH_H / 33.3
	for k in n:
		var ms := _frames[(_frame_i - n + k) % GRAPH_N]
		var bh := minf(ms * px_per_ms, GRAPH_H)
		var col := Color(0.35, 0.9, 0.35) if ms <= 8.3 else (Color(0.95, 0.85, 0.2) if ms <= 16.7 else Color(0.95, 0.25, 0.2))
		_graph.draw_rect(Rect2(o.x + k * bw, o.y + GRAPH_H - bh, bw, bh), col)
	for limit in [8.3, 16.7]:
		var ly: float = o.y + GRAPH_H - limit * px_per_ms
		_graph.draw_line(Vector2(o.x, ly), Vector2(o.x + w, ly), Color(1, 1, 1, 0.5), 1.0)
		_graph.draw_string(_font, Vector2(o.x + 3.0, ly - 2.0), "%.1f ms" % limit, HORIZONTAL_ALIGNMENT_LEFT, -1, 11, Color(1, 1, 1, 0.8))


## Vanilla's debug crosshair: the world axes as seen from the camera.
func _draw_axes() -> void:
	var c := _axes.size / 2.0
	var inv: Basis = main.player.camera.global_transform.basis.inverse()
	var axes := [[Vector3.BACK, Color(0.3, 0.45, 1.0)], [Vector3.UP, Color(0.3, 1.0, 0.3)], [Vector3.RIGHT, Color(1.0, 0.3, 0.3)]]
	for a in axes:
		var l: Vector3 = inv * (a[0] as Vector3)
		_axes.draw_line(c, c + Vector2(l.x, -l.y) * 14.0, a[1], 2.0)


## Date, moon and sun (ADR 0025): "Date: 5 September, year 1 (Autumn), Full Moon 99% (Harvest Moon), day 11.9 h".
func _sky_line(c: Calendar) -> String:
	var name := c.moon_name()
	var e := c.eclipse()
	var ecl := ""
	if e["kind"] != "":
		ecl = ", %s eclipse%s" % [e["kind"], " (total)" if e["total"] else ""]
	return "Date: %s, %s %d%%%s, day %.1f h%s" % [c.date_text(), c.phase_name(), int(c.moon_illumination() * 100.0),
		(" (%s)" % name) if name != "" else "", c.day_length_hours(), ecl]
