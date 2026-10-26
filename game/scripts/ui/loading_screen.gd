class_name LoadingScreen
extends CanvasLayer

## Loading cover (ADR 0020): a dark panel with a progress bar until the
## simulation bubble holds its chunks and godot_voxel's workers have drained
## the streaming burst, then a short fade. Main puts it up at the start, after a
## respawn and after a long teleport, and freezes the player and the animals
## under it: when it lifts, the world is there. Progress is 1 - pending / peak
## pending tasks (monotonic); a generous cap only guards against a hang.
## A teleport cover (`world` and `player` set, ADR 0066) waits only for the ground: voxel data under the
## feet and visible meshes around them (World.near_ready, also its bar), not the 2 km horizon (that
## streams in behind it under the normal upload budget). Its title and bar show only if it lasts.

signal finished   # the cover starts fading: the terrain around the player is in

const CAP_S := 120.0       # generation may take its time (the user); the cap only guards against a hang
const IDLE_FRAMES := 20        # frames with no voxel task before the fade starts
const FADE_S := 0.6
const NEAR_CAP_S := 20.0      # a teleport cover lifts after this even if some ground never shows (an edited surface)
const NEAR_IDLE_FRAMES := 3
const NEAR_SHOWN := 0.95      # share of the ground samples around the player that must be drawn (World.near_ready)
const NEAR_FADE_S := 0.35
const NEAR_SHOW_TEXT_S := 0.6  # a teleport cover shows its title and bar only after this
const NEAR_BUBBLE_R := 1       # sim chunks (2 m) around the player a teleport cover waits for; the rest loads behind it

var bubble: SimBubble
var world: Node        # set: a teleport cover (near ground only)
var player: Node3D
var title := "Generating world"
var _cover: ColorRect
var _bar: ColorRect
var _bar_bg: ColorRect
var _label: Label
var _elapsed := 0.0
var _peak := 0
var _idle := 0
var _progress := 0.0
var _fading := false


func _ready() -> void:
	layer = 20
	_cover = ColorRect.new()
	_cover.color = Color(0.03, 0.035, 0.05, 1.0)
	_cover.set_anchors_preset(Control.PRESET_FULL_RECT)
	_cover.mouse_filter = Control.MOUSE_FILTER_STOP
	add_child(_cover)
	_label = Label.new()
	_label.text = title
	_label.add_theme_font_size_override("font_size", 22)
	_label.add_theme_color_override("font_color", Color(0.85, 0.85, 0.8))
	_label.set_anchors_preset(Control.PRESET_CENTER)
	_label.position = Vector2(-90, -40)
	_cover.add_child(_label)
	_bar_bg = ColorRect.new()
	_bar_bg.color = Color(0.2, 0.2, 0.22)
	_bar_bg.set_anchors_preset(Control.PRESET_CENTER)
	_bar_bg.position = Vector2(-150, 4)
	_bar_bg.size = Vector2(300, 6)
	_cover.add_child(_bar_bg)
	_bar = ColorRect.new()
	_bar.color = Color(0.55, 0.75, 0.35)
	_bar.position = Vector2.ZERO
	_bar.size = Vector2(0, 6)
	_bar_bg.add_child(_bar)
	if world != null:
		_label.visible = false
		_bar_bg.visible = false


func _process(delta: float) -> void:
	if _fading:
		return
	_elapsed += delta
	var t: Dictionary = VoxelEngine.get_stats().get("tasks", {})
	var pending: int = int(t.get("generation", 0)) + int(t.get("meshing", 0)) + int(t.get("streaming", 0)) + int(t.get("main_thread", 0))
	_peak = maxi(_peak, pending)
	var near: float = world.near_ready(player.position) if world != null else 0.0
	if world != null:
		_progress = maxf(_progress, near)
	elif _peak > 0:
		_progress = maxf(_progress, 1.0 - float(pending) / float(_peak))
	_bar.size.x = 300.0 * _progress
	var bubble_ready := bubble == null or bubble.is_loaded(NEAR_BUBBLE_R if world != null else 4)
	var near_ok := false
	if world != null:
		if _elapsed >= NEAR_SHOW_TEXT_S:
			_label.visible = true
			_bar_bg.visible = true
		near_ok = bubble_ready and near >= NEAR_SHOWN
		_idle = _idle + 1 if near_ok else 0
	else:
		_idle = _idle + 1 if (pending == 0 and bubble_ready and _elapsed > 0.5) else 0
	var cap := NEAR_CAP_S if world != null else CAP_S
	if _idle >= (NEAR_IDLE_FRAMES if world != null else IDLE_FRAMES) or _elapsed >= cap:
		_fading = true
		_bar.size.x = 300.0
		print("loading cover '%s' lifted after %.2f s%s" % [title, _elapsed, "" if _elapsed < cap else " (cap)"])
		finished.emit()
		var tw := create_tween()
		tw.tween_property(_cover, "modulate:a", 0.0, NEAR_FADE_S if world != null else FADE_S)
		tw.tween_callback(queue_free)


func is_done() -> bool:
	return _fading
