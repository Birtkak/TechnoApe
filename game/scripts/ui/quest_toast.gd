class_name QuestToast
extends CanvasLayer

## Minecraft advancement-style toast, top right: "Quest complete!" with the
## quest's icon and title (ADR 0028). Several completions queue up and show
## one after another; each slides in, stays TOAST_S seconds, slides out.

const TOAST_S := 3.5
const SLIDE_S := 0.25
const SIZE := Vector2(300, 64)

var items: ItemRegistry
var _queue: Array = []      # [icon id, title, final]
var _box: Control
var _icon: ItemIcon
var _t := -1.0              # time into the shown toast, -1 = none
var _current: Array = []
var shown := 0              # toasts started (stats)


func _ready() -> void:
	layer = 12
	_box = Control.new()
	_box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_box.size = SIZE
	_box.visible = false
	_box.draw.connect(_draw_box)
	add_child(_box)
	_icon = ItemIcon.new()
	_icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_icon.position = Vector2(14, 14)
	_icon.size = Vector2(36, 36)
	_box.add_child(_icon)


func push(icon_id: String, title: String, final := false) -> void:
	if _queue.size() < 3:   # a burst (/quests complete all) shows its first few, not a minute of toasts
		_queue.append([icon_id, title, final])


func _process(delta: float) -> void:
	if _t < 0.0:
		if _queue.is_empty():
			return
		_current = _queue.pop_front()
		_icon.items = items
		_icon.stack = {"id": _current[0], "count": 1}
		_t = 0.0
		_box.visible = true
		_box.queue_redraw()
		shown += 1
	_t += delta
	var total := TOAST_S + SLIDE_S * 2.0
	if _t >= total:
		_t = -1.0
		_box.visible = false
		return
	var slide := minf(1.0, minf(_t, total - _t) / SLIDE_S)
	var vw := _box.get_viewport_rect().size.x
	_box.position = Vector2(vw - SIZE.x * ease(slide, 0.4) - 8.0 * slide, 8)


func _draw_box() -> void:
	if _current.is_empty():
		return
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.13, 0.13, 0.16, 0.96)
	sb.border_color = Color(0.98, 0.80, 0.30) if _current[2] else Color(0.42, 0.85, 0.42)
	sb.set_border_width_all(3)
	sb.set_corner_radius_all(6)
	_box.draw_style_box(sb, Rect2(Vector2.ZERO, SIZE))
	var font := ThemeDB.fallback_font
	_box.draw_string(font, Vector2(62, 26), "Chapter complete!" if _current[2] else "Quest complete!", HORIZONTAL_ALIGNMENT_LEFT, -1, 15, sb.border_color)
	_box.draw_string(font, Vector2(62, 48), _current[1], HORIZONTAL_ALIGNMENT_LEFT, SIZE.x - 70, 17, Color(0.95, 0.95, 0.97))
