class_name RadialMenu
extends Control

## A ring of choices around the screen centre (ADR 0038, the build menu on V, the brush on C), after Lay of the Land
## (ADR 0144): small tiles spread wide over the running game, a pointer at the centre, a key list at the
## bottom right. The mouse stays captured, its motion moves the pointer, and the slice it points at is
## the choice; past DEAD_ZONE px, closing takes it, inside it nothing is chosen.

const DEAD_ZONE := 28.0
const LABEL_PX := 14

var items: Array = []    # [{id, label}]
var title := ""
var extra := ""          # one more line for the key list
var selected := -1
var _pointer := Vector2.ZERO
var _font: Font


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	visible = false
	_font = ThemeDB.fallback_font


func open(p_items: Array, p_title := "", keep := -1, p_extra := "") -> void:
	items = p_items
	title = p_title
	extra = p_extra
	selected = keep
	_pointer = Vector2.ZERO
	if keep >= 0 and keep < items.size():
		_pointer = Vector2.from_angle(_angle_of(keep)) * (DEAD_ZONE + 10.0)
	visible = true
	queue_redraw()


## Mouse motion while open (the pointer is clamped to the ring).
func feed(relative: Vector2) -> void:
	_pointer = (_pointer + relative).limit_length(_radius())
	selected = -1 if _pointer.length() < DEAD_ZONE or items.is_empty() else _slice_at(_pointer.angle())
	queue_redraw()


## Closes the menu; returns the chosen item's id, or "" for none.
func close() -> String:
	visible = false
	return String(items[selected]["id"]) if selected >= 0 and selected < items.size() else ""


func _radius() -> float:
	return maxf(size.y, 480.0) * 0.25


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
	var radius := _radius()
	var tile0 := minf(radius * 0.32, TAU * radius / n * 0.62)
	for i in n:
		var at := c + Vector2.from_angle(_angle_of(i)) * radius
		var on := i == selected
		var tile := tile0 * (1.22 if on else 1.0)
		var r := Rect2(at - Vector2(tile, tile) * 0.5, Vector2(tile, tile))
		draw_rect(r, Color(0.09, 0.09, 0.09, 0.72 if on else 0.6))
		draw_rect(r, Color(0.92, 0.92, 0.9, 1.0 if on else 0.6), false, 2.0 if on else 1.0)
		BuildIcon.draw_icon(self, String(items[i]["id"]), r.grow(-tile * 0.2), Color.WHITE, 1.5)
		_label(String(items[i]["label"]).left(16), Vector2(at.x, r.end.y + LABEL_PX + 1.0), HORIZONTAL_ALIGNMENT_CENTER)
	if selected >= 0 and selected < n:
		_label(items[selected]["label"], c + Vector2(0.0, 5.0), HORIZONTAL_ALIGNMENT_CENTER)
		var d := Vector2.from_angle(_angle_of(selected))
		var tip := c + d * DEAD_ZONE * 1.5
		var side := d.orthogonal() * 7.0
		draw_colored_polygon(PackedVector2Array([tip, tip - d * 11.0 + side, tip - d * 11.0 - side]), Color(0.92, 0.92, 0.9, 0.9))
	else:
		_label(title.get_slice("  ", 0), c + Vector2(0.0, 5.0), HORIZONTAL_ALIGNMENT_CENTER)
	var y := size.y - 16.0
	for line in ["(Esc) Back", "(LMB) Select", extra, title]:
		if line != "":
			_label(line, Vector2(size.x - 16.0, y), HORIZONTAL_ALIGNMENT_RIGHT)
			y -= LABEL_PX + 4.0


## One line of outlined text; `at` is its baseline's centre or right end.
func _label(text: String, at: Vector2, align: HorizontalAlignment) -> void:
	var w := _font.get_string_size(text, HORIZONTAL_ALIGNMENT_LEFT, -1, LABEL_PX).x
	var o := Vector2(at.x - (w if align == HORIZONTAL_ALIGNMENT_RIGHT else w * 0.5), at.y)
	draw_string_outline(_font, o, text, HORIZONTAL_ALIGNMENT_LEFT, -1, LABEL_PX, 4, Color.BLACK)
	draw_string(_font, o, text, HORIZONTAL_ALIGNMENT_LEFT, -1, LABEL_PX, Color.WHITE)
