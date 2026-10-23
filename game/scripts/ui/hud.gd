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
var _pill: Control          # rounded backdrop behind the item name
var _font: Font
var _name_timer := 0.0
var _crosshair: Control
var _progress: Control
var break_progress := 0.0   # 0..1, set by the player while mining
var player_stains: PlayerStains   # Noita stains: icons right of the hotbar, a screen tint while burning or toxic
var _stain_bar: Control
var _screen_tint: ColorRect


func setup(p_inv: PlayerInventory) -> void:
	player_inv = p_inv
	player_inv.changed.connect(_refresh)
	player_inv.selection_changed.connect(_on_selection)
	player_inv.health_changed.connect(func(_h: int) -> void: _hearts.queue_redraw())
	_refresh()


func _ready() -> void:
	layer = 5
	_screen_tint = ColorRect.new()
	_screen_tint.set_anchors_preset(Control.PRESET_FULL_RECT)
	_screen_tint.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_screen_tint.color = Color(0, 0, 0, 0)
	add_child(_screen_tint)
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

	_stain_bar = Control.new()
	_stain_bar.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_stain_bar.position = Vector2(w / 2.0 + 10, -SLOT - 8)   # just right of the hotbar
	_stain_bar.size = Vector2(5 * 40, SLOT)
	_stain_bar.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_stain_bar.draw.connect(_draw_stains)
	add_child(_stain_bar)

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
	_name_label.add_theme_color_override("font_color", Color(0.97, 0.95, 0.9))
	_name_label.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.8))
	_name_label.add_theme_constant_override("shadow_offset_x", 1)
	_name_label.add_theme_constant_override("shadow_offset_y", 1)
	_name_label.modulate.a = 0.0
	add_child(_name_label)
	_pill = Control.new()
	_pill.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_pill.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_pill.draw.connect(_draw_pill)
	_pill.modulate.a = 0.0
	add_child(_pill)
	move_child(_pill, _name_label.get_index())


## The F3 overlay draws its own axis crosshair instead.
func set_crosshair_visible(v: bool) -> void:
	_crosshair.visible = v


## Creative hides the hearts (no damage), like vanilla.
func set_hearts_visible(v: bool) -> void:
	_hearts.visible = v


func _process(delta: float) -> void:
	if _name_timer > 0.0:
		_name_timer -= delta
		_name_label.modulate.a = clampf(_name_timer / 0.5, 0.0, 1.0)
		_pill.modulate.a = _name_label.modulate.a
	_progress.queue_redraw()
	if player_stains != null:
		_stain_bar.queue_redraw()
		var burn := clampf(float(player_stains.amount["burning"]), 0.0, 1.0)
		var tox := clampf(float(player_stains.amount["toxic"]), 0.0, 1.0)
		var tint := Color(1.0, 0.45, 0.1, 0.16 * burn) if burn > 0.0 else Color(0.4, 0.9, 0.15, 0.10 * tox)
		_screen_tint.color = tint


## Active stains as Noita-style status icons next to the hotbar, each with a
## bar that empties as the coat fades and its name above it.
func _draw_stains() -> void:
	if player_stains == null:
		return
	if _font == null:
		_font = ThemeDB.fallback_font
	var sprites := ItemSprites.get_default()
	var x := 0.0
	for k in player_stains.active():
		var r := Rect2(x, 0, 36, 36)
		_stain_bar.draw_rect(r, Color(0.08, 0.08, 0.1, 0.75))
		var tex := sprites.texture("stain_" + k)
		if tex:
			_stain_bar.draw_texture_rect(tex, r.grow(-2), false)
		var a := clampf(float(player_stains.amount[k]), 0.0, 1.0)
		_stain_bar.draw_rect(Rect2(x, 39, 36 * a, 4), PlayerStains.COLORS[k])
		_stain_bar.draw_string(_font, Vector2(x, -4), k, HORIZONTAL_ALIGNMENT_CENTER, 36, 11, Color(0.95, 0.95, 0.95, 0.9))
		x += 40.0


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
		_pill.modulate.a = 0.0
		return
	_name_label.text = player_inv.items.display_name(h["id"])
	_name_label.size = Vector2(400, 24)
	_name_label.position = Vector2(-200, -SLOT - 8 - 22 - 28)
	if _font == null:
		_font = ThemeDB.fallback_font
	var tw := _font.get_string_size(_name_label.text, HORIZONTAL_ALIGNMENT_LEFT, -1, 18).x
	_pill.size = Vector2(tw + 28, 30)
	_pill.position = Vector2(-_pill.size.x / 2.0, -SLOT - 8 - 22 - 31)
	_pill.queue_redraw()
	_name_timer = 2.0
	_name_label.modulate.a = 1.0
	_pill.modulate.a = 1.0


func _draw_pill() -> void:
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.03, 0.03, 0.05, 0.72)
	sb.border_color = Color(1, 1, 1, 0.14)
	sb.set_border_width_all(1)
	sb.set_corner_radius_all(8)
	_pill.draw_style_box(sb, Rect2(Vector2.ZERO, _pill.size))


func _draw_hotbar() -> void:
	# one rounded tray behind the row; slots are recessed cells; the selected
	# slot lifts out with a bright rim and a soft glow
	var tray := StyleBoxFlat.new()
	tray.bg_color = Color(0.04, 0.04, 0.06, 0.62)
	tray.border_color = Color(1, 1, 1, 0.16)
	tray.set_border_width_all(1)
	tray.set_corner_radius_all(7)
	tray.set_expand_margin_all(5)
	tray.shadow_color = Color(0, 0, 0, 0.45)
	tray.shadow_size = 8
	tray.shadow_offset = Vector2(0, 2)
	_hotbar.draw_style_box(tray, Rect2(0, 0, _hotbar.size.x, SLOT))
	var cell := StyleBoxFlat.new()
	cell.bg_color = Color(0, 0, 0, 0.38)
	cell.border_color = Color(1, 1, 1, 0.07)
	cell.set_border_width_all(1)
	cell.set_corner_radius_all(4)
	for i in 9:
		_hotbar.draw_style_box(cell, Rect2(i * (SLOT + GAP), 0, SLOT, SLOT))
	if player_inv:
		var r := Rect2(player_inv.selected * (SLOT + GAP), 0, SLOT, SLOT)
		var glow := StyleBoxFlat.new()
		glow.bg_color = Color(1, 0.95, 0.8, 0.10)
		glow.set_corner_radius_all(5)
		glow.set_expand_margin_all(2)
		glow.shadow_color = Color(1, 0.9, 0.6, 0.35)
		glow.shadow_size = 7
		_hotbar.draw_style_box(glow, r)
		var sel := StyleBoxFlat.new()
		sel.bg_color = Color(0, 0, 0, 0)
		sel.border_color = Color(1, 0.98, 0.92, 0.95)
		sel.set_border_width_all(2)
		sel.set_corner_radius_all(5)
		sel.set_expand_margin_all(2)
		_hotbar.draw_style_box(sel, r)
	# slot numbers, small, top-left (vanilla has none; they help new players)
	if _font == null:
		_font = ThemeDB.fallback_font
	for i in 9:
		var pos := Vector2(i * (SLOT + GAP) + 4, 11)
		_hotbar.draw_string(_font, pos + Vector2(1, 1), str(i + 1), HORIZONTAL_ALIGNMENT_LEFT, -1, 10, Color(0, 0, 0, 0.7))
		_hotbar.draw_string(_font, pos, str(i + 1), HORIZONTAL_ALIGNMENT_LEFT, -1, 10, Color(1, 1, 1, 0.45))


# 7x6 pixel heart, drawn at 2x like the vanilla 9x9 sprite
const HEART := ["0110110", "1111111", "1111111", "0111110", "0011100", "0001000"]

func _draw_hearts() -> void:
	if player_inv == null:
		return
	var px := 2
	var red := Color(0.88, 0.12, 0.14)
	var dark := Color(0.22, 0.2, 0.22)
	var outline := Color(0.05, 0.02, 0.03, 0.95)
	for i in 10:
		var hp := player_inv.health - i * 2
		var ox := i * 18
		# outline pass: every heart pixel grown by one HUD pixel in each direction
		for y in HEART.size():
			for x in 7:
				if HEART[y][x] != "1":
					continue
				_hearts.draw_rect(Rect2(ox + x * px - 1, 2 + y * px - 1, px + 2, px + 2), outline)
		for y in HEART.size():
			for x in 7:
				if HEART[y][x] != "1":
					continue
				var filled := hp >= 2 or (hp == 1 and x < 4)
				var c := red if filled else dark
				if filled:
					# shading: a highlight on the upper-left lobe, deeper red on the lower rows
					if y == 1 and (x == 1 or x == 2):
						c = Color(1.0, 0.55, 0.55)
					elif y >= 3:
						c = Color(0.68, 0.07, 0.1)
				_hearts.draw_rect(Rect2(ox + x * px, 2 + y * px, px, px), c)


func _draw_crosshair() -> void:
	# four short ticks with a gap in the middle and a dark edge, plus a centre dot
	var c := Color(1, 1, 1, 0.92)
	var o := Color(0, 0, 0, 0.55)
	var gap := 3.0
	var len := 8.0
	for d in [Vector2(1, 0), Vector2(-1, 0), Vector2(0, 1), Vector2(0, -1)]:
		_crosshair.draw_line(d * gap, d * (gap + len), o, 4.0)
	for d in [Vector2(1, 0), Vector2(-1, 0), Vector2(0, 1), Vector2(0, -1)]:
		_crosshair.draw_line(d * gap, d * (gap + len), c, 2.0)
	_crosshair.draw_circle(Vector2.ZERO, 2.2, o)
	_crosshair.draw_circle(Vector2.ZERO, 1.2, c)


func _draw_progress() -> void:
	if break_progress <= 0.0:
		return
	var w := 64.0
	var p := clampf(break_progress, 0.0, 1.0)
	var bg := StyleBoxFlat.new()
	bg.bg_color = Color(0, 0, 0, 0.65)
	bg.border_color = Color(1, 1, 1, 0.25)
	bg.set_border_width_all(1)
	bg.set_corner_radius_all(4)
	_progress.draw_style_box(bg, Rect2(-w / 2.0 - 2, 16, w + 4, 9))
	var fg := StyleBoxFlat.new()
	fg.bg_color = Color(1, 0.92, 0.7, 0.95)
	fg.set_corner_radius_all(3)
	_progress.draw_style_box(fg, Rect2(-w / 2.0, 18, maxf(3.0, w * p), 5))
