class_name DiscoveryView
extends Control

## The Discoveries tab of the progression book (ADR 0061): every kind with its found / total count and
## a card per entry, found ones with their icon, date and line, the rest as "???". Lives over the
## book's graph canvas while the tab is open (QuestBookView); Discoveries holds the data.

const CARD := Vector2(250, 64)
const C_TEXT := Color(0.9, 0.9, 0.92)
const C_DIM := Color(0.55, 0.55, 0.6)
const C_FOUND := Color(0.45, 0.7, 1.0)

var book: Discoveries
var items: ItemRegistry
var _scroll: ScrollContainer
var _list: VBoxContainer


func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP
	_scroll = ScrollContainer.new()
	_scroll.set_anchors_preset(Control.PRESET_FULL_RECT)
	_scroll.offset_top = 44
	_scroll.offset_left = 12
	_scroll.offset_right = -8
	_scroll.offset_bottom = -8
	_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	add_child(_scroll)
	_list = VBoxContainer.new()
	_list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_list.add_theme_constant_override("separation", 6)
	_scroll.add_child(_list)


func _draw() -> void:
	if book == null:
		return
	var font := ThemeDB.fallback_font
	var p := book.total_progress()
	draw_string(font, Vector2(16, 30), "Discoveries", HORIZONTAL_ALIGNMENT_LEFT, -1, 22, C_TEXT)
	var bar := Rect2(size.x - 196, 16, 180, 12)
	draw_rect(bar, Color(0.2, 0.2, 0.24))
	draw_rect(Rect2(bar.position, Vector2(bar.size.x * p.x / maxf(1.0, p.y), bar.size.y)), C_FOUND)
	draw_string(font, Vector2(bar.position.x - 64, 28), "%d/%d" % [p.x, p.y], HORIZONTAL_ALIGNMENT_LEFT, -1, 15, C_DIM)


func refresh() -> void:
	queue_redraw()
	if _list == null or book == null:
		return
	for c in _list.get_children():
		c.queue_free()
	var cols := maxi(1, int((size.x - 32) / (CARD.x + 8)))
	for k in book.kinds:
		if k["entries"].is_empty():
			continue   # a kind whose content has not landed yet (part E's stones)
		var pr := book.kind_progress(k["id"])
		var head := Label.new()
		head.text = "%s  %d/%d" % [k["title"], pr.x, pr.y]
		head.add_theme_font_size_override("font_size", 17)
		head.add_theme_color_override("font_color", C_TEXT)
		_list.add_child(head)
		var grid := GridContainer.new()
		grid.columns = cols
		grid.add_theme_constant_override("h_separation", 8)
		grid.add_theme_constant_override("v_separation", 8)
		_list.add_child(grid)
		for id in k["entries"]:
			var card := Card.new()
			card.items = items
			card.entry = book.entries[id]
			card.found = book.found.get(id, {}) if book.is_found(id) else {}
			card.known = book.is_found(id)
			card.custom_minimum_size = CARD
			grid.add_child(card)


## One entry: icon, name and line when found, a dark "???" when not.
class Card:
	extends Control
	var items: ItemRegistry
	var entry := {}
	var found := {}
	var known := false

	func _ready() -> void:
		mouse_filter = Control.MOUSE_FILTER_PASS
		if known:
			tooltip_text = "%s\n%s" % [entry["title"], entry["text"]]
			var ic := ItemIcon.new()
			ic.items = items
			ic.mouse_filter = Control.MOUSE_FILTER_IGNORE
			ic.position = Vector2(8, 14)
			ic.size = Vector2(32, 32)
			ic.stack = {"id": entry["icon"], "count": 1} if items != null and items.has(entry["icon"]) else {}
			add_child(ic)

	func _draw() -> void:
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.17, 0.19, 0.24) if known else Color(0.11, 0.11, 0.13)
		sb.border_color = C_FOUND.darkened(0.3) if known else Color(0.25, 0.25, 0.28)
		sb.set_border_width_all(1)
		sb.set_corner_radius_all(5)
		draw_style_box(sb, Rect2(Vector2.ZERO, size))
		var font := ThemeDB.fallback_font
		if not known:
			draw_string(font, Vector2(48, 36), "???", HORIZONTAL_ALIGNMENT_LEFT, -1, 16, C_DIM)
			return
		draw_string(font, Vector2(48, 20), String(entry["title"]), HORIZONTAL_ALIGNMENT_LEFT, size.x - 52, 15, C_TEXT)
		draw_string(font, Vector2(48, 37), String(entry["text"]), HORIZONTAL_ALIGNMENT_LEFT, size.x - 52, 11, Color(0.78, 0.78, 0.82))
		var when := String(found.get("date", ""))
		if when != "":
			draw_string(font, Vector2(48, 54), when, HORIZONTAL_ALIGNMENT_LEFT, size.x - 52, 10, C_DIM)
