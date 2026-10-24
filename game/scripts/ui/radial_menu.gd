class_name RadialMenu
extends Control

## A ring of choices around the screen centre (ADR 0038, the build menu on V): the mouse stays
## captured, its motion moves a pointer from the centre, and the slice the pointer points at is the
## choice; past DEAD_ZONE px, closing takes it, inside it nothing is chosen.

const RADIUS := 150.0
const INNER := 52.0
const DEAD_ZONE := 28.0

var items: Array = []    # [{id, label}]
var title := ""
var selected := -1
var _pointer := Vector2.ZERO
var _font: Font


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	visible = false
	_font = ThemeDB.fallback_font


func open(p_items: Array, p_title := "", keep := -1) -> void:
	items = p_items
	title = p_title
	selected = keep
	_pointer = Vector2.ZERO
	if keep >= 0 and keep < items.size():
		_pointer = Vector2.from_angle(_angle_of(keep)) * (DEAD_ZONE + 10.0)
	visible = true
	queue_redraw()


## Mouse motion while open (the pointer is clamped to the ring).
func feed(relative: Vector2) -> void:
	_pointer = (_pointer + relative).limit_length(RADIUS)
	selected = -1 if _pointer.length() < DEAD_ZONE or items.is_empty() else _slice_at(_pointer.angle())
	queue_redraw()


## Closes the menu; returns the chosen item's id, or "" for none.
func close() -> String:
	visible = false
	return String(items[selected]["id"]) if selected >= 0 and selected < items.size() else ""


func _angle_of(i: int) -> float:
	return -PI / 2.0 + TAU * i / maxf(items.size(), 1)   # the first item at the top, clockwise


func _slice_at(angle: float) -> int:
	var a := fposmod(angle + PI / 2.0 + PI / items.size(), TAU)
	return int(a / TAU * items.size()) % items.size()


func _draw() -> void:
	if items.is_empty():
		return
	var c := size / 2.0
	var n := items.size()
	draw_circle(c, RADIUS + 18.0, Color(0.03, 0.03, 0.05, 0.55))
	for i in n:
		var a0 := _angle_of(i) - PI / n
		var pts := PackedVector2Array()
		for k in 13:
			pts.append(c + Vector2.from_angle(a0 + TAU / n * k / 12.0) * RADIUS)
		for k in range(12, -1, -1):
			pts.append(c + Vector2.from_angle(a0 + TAU / n * k / 12.0) * INNER)
		var on := i == selected
		draw_colored_polygon(pts, Color(0.95, 0.75, 0.3, 0.55) if on else Color(1, 1, 1, 0.10))
		draw_polyline(pts, Color(1, 1, 1, 0.25), 1.0)
		var label: String = items[i]["label"]
		var at := c + Vector2.from_angle(_angle_of(i)) * (RADIUS + INNER) * 0.5
		var w := _font.get_string_size(label, HORIZONTAL_ALIGNMENT_LEFT, -1, 16).x
		draw_string(_font, at + Vector2(-w / 2.0, 6.0), label, HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color.WHITE if on else Color(1, 1, 1, 0.8))
	if title != "":
		var tw := _font.get_string_size(title, HORIZONTAL_ALIGNMENT_LEFT, -1, 15).x
		draw_string(_font, c + Vector2(-tw / 2.0, 5.0), title, HORIZONTAL_ALIGNMENT_LEFT, -1, 15, Color(1, 1, 1, 0.7))
	draw_circle(c + _pointer, 4.0, Color.WHITE)
