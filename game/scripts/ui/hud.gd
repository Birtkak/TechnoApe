class_name GameHud
extends CanvasLayer

## The in-game HUD (ADR 0005): crosshair, 9-slot hotbar with item icons and
## counts, selected-slot highlight with the item name fading above it, a
## hearts row, and the break progress bar. Reads everything from the
## PlayerInventory model; the F3 overlay (DebugHud) is separate.

const SLOT := 44
const GAP := 2

var player_inv: PlayerInventory
var _hotbar: Control
var _icons: Array[ItemIcon] = []
var _hearts: Control
var _name_label: Label
var _name_timer := 0.0
var _crosshair: Control
var _progress: Control
var break_progress := 0.0   # 0..1, set by the player while mining


func setup(p_inv: PlayerInventory) -> void:
	player_inv = p_inv
	player_inv.changed.connect(_refresh)
	player_inv.selection_changed.connect(_on_selection)
	player_inv.health_changed.connect(func(_h: int) -> void: _hearts.queue_redraw())
	_refresh()


func _ready() -> void:
	layer = 5
	_crosshair = Control.new()
	_crosshair.set_anchors_preset(Control.PRESET_CENTER)
	_crosshair.draw.connect(_draw_crosshair)
	add_child(_crosshair)

	_progress = Control.new()
	_progress.set_anchors_preset(Control.PRESET_CENTER)
	_progress.draw.connect(_draw_progress)
	add_child(_progress)

	_hotbar = Control.new()
	_hotbar.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	var w := 9 * SLOT + 8 * GAP
	_hotbar.size = Vector2(w, SLOT)
	_hotbar.position = Vector2(-w / 2.0, -SLOT - 8)
	_hotbar.draw.connect(_draw_hotbar)
	add_child(_hotbar)
	for i in 9:
		var icon := ItemIcon.new()
		icon.position = Vector2(i * (SLOT + GAP) + 6, 6)
		icon.size = Vector2(SLOT - 12, SLOT - 12)
		icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_hotbar.add_child(icon)
		_icons.append(icon)

	_hearts = Control.new()
	_hearts.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_hearts.position = Vector2(-w / 2.0, -SLOT - 8 - 22)
	_hearts.size = Vector2(w / 2.0, 20)
	_hearts.draw.connect(_draw_hearts)
	add_child(_hearts)

	_name_label = Label.new()
	_name_label.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_name_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_name_label.add_theme_font_size_override("font_size", 18)
	_name_label.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.8))
	_name_label.add_theme_constant_override("shadow_offset_x", 1)
	_name_label.add_theme_constant_override("shadow_offset_y", 1)
	_name_label.modulate.a = 0.0
	add_child(_name_label)


func _process(delta: float) -> void:
	if _name_timer > 0.0:
		_name_timer -= delta
		_name_label.modulate.a = clampf(_name_timer / 0.5, 0.0, 1.0)
	_progress.queue_redraw()


func _refresh() -> void:
	if player_inv == null:
		return
	for i in 9:
		_icons[i].items = player_inv.items
		_icons[i].stack = player_inv.inv.get_slot(i)
	_hotbar.queue_redraw()
	_hearts.queue_redraw()


func _on_selection(_i: int) -> void:
	_hotbar.queue_redraw()
	var h := player_inv.held()
	if h.is_empty():
		_name_timer = 0.0
		_name_label.modulate.a = 0.0
		return
	_name_label.text = player_inv.items.display_name(h["id"])
	_name_label.size = Vector2(400, 24)
	_name_label.position = Vector2(-200, -SLOT - 8 - 22 - 26)
	_name_timer = 2.0
	_name_label.modulate.a = 1.0


func _draw_hotbar() -> void:
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0, 0, 0, 0.55)
	sb.border_color = Color(0.35, 0.35, 0.35, 0.9)
	sb.set_border_width_all(2)
	var sel := StyleBoxFlat.new()
	sel.bg_color = Color(0, 0, 0, 0)
	sel.border_color = Color(1, 1, 1, 0.95)
	sel.set_border_width_all(3)
	sel.set_expand_margin_all(3)
	for i in 9:
		var r := Rect2(i * (SLOT + GAP), 0, SLOT, SLOT)
		_hotbar.draw_style_box(sb, r)
	if player_inv:
		_hotbar.draw_style_box(sel, Rect2(player_inv.selected * (SLOT + GAP), 0, SLOT, SLOT))


# 7x6 pixel heart, drawn at 2x like the vanilla 9x9 sprite
const HEART := ["0110110", "1111111", "1111111", "0111110", "0011100", "0001000"]

func _draw_hearts() -> void:
	if player_inv == null:
		return
	var px := 2
	for i in 10:
		var hp := player_inv.health - i * 2
		var fill := Color(0.86, 0.1, 0.1) if hp >= 2 else (Color(0.86, 0.1, 0.1) if hp == 1 else Color(0.25, 0.25, 0.25))
		var ox := i * 18
		for y in HEART.size():
			for x in 7:
				if HEART[y][x] != "1":
					continue
				var c := fill
				if hp == 1 and x >= 4:
					c = Color(0.25, 0.25, 0.25)
				_hearts.draw_rect(Rect2(ox + x * px, 2 + y * px, px, px), c)
				# outline below/right for a pixel-art edge
			_hearts.draw_rect(Rect2(ox, 2 + y * px, 0, 0), Color.BLACK)


func _draw_crosshair() -> void:
	var c := Color(1, 1, 1, 0.9)
	var o := Color(0, 0, 0, 0.6)
	_crosshair.draw_line(Vector2(-9, 0), Vector2(9, 0), o, 4.0)
	_crosshair.draw_line(Vector2(0, -9), Vector2(0, 9), o, 4.0)
	_crosshair.draw_line(Vector2(-8, 0), Vector2(8, 0), c, 2.0)
	_crosshair.draw_line(Vector2(0, -8), Vector2(0, 8), c, 2.0)


func _draw_progress() -> void:
	if break_progress <= 0.0:
		return
	var w := 60.0
	_progress.draw_rect(Rect2(-w / 2.0 - 1, 15, w + 2, 8), Color(0, 0, 0, 0.7))
	_progress.draw_rect(Rect2(-w / 2.0, 16, w * clampf(break_progress, 0.0, 1.0), 6), Color(1, 1, 1, 0.9))
