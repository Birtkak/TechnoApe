class_name GameHud
extends CanvasLayer

## The in-game HUD (ADR 0005): crosshair, 9-slot hotbar with item icons and
## counts, selected-slot highlight with the item name fading above it, a
## hearts row, and the break progress bar. Reads everything from the
## PlayerInventory model; the F3 overlay (DebugHud) is separate.

const SLOT := 44
const GAP := 2
const LOOK_ICON := 40.0

var player_inv: PlayerInventory
var _hotbar: Control
var _icons: Array[ItemIcon] = []
var _off_icon: ItemIcon      # ADR 0055: the offhand stack, left of the thermometer
const OFF_X := -38.0         # its cell's right edge, from the hotbar's left edge
var _hearts: Control
var _hunger_row: Control     # ADR 0040: ten food icons, right half above the hotbar (vanilla's place)
var _thermo: Control         # body temperature, left of the hotbar
var survival: Survival       # set by Main
var _shown_hunger := -1
var _shown_body := -1
var _name_label: Label
var _brush_label: Label     # the dig/build brush and its keys, right of the hotbar
var _pill: Control          # rounded backdrop behind the item name
var _font: Font
var _name_timer := 0.0
var _crosshair: Control
var _progress: Control
var break_progress := 0.0   # 0..1, set by the player while mining
var player_stains: PlayerStains   # Noita stains: icons right of the hotbar, a screen tint while burning or toxic
var _stain_bar: Control
var _screen_tint: ColorRect
var air := 1.0              # breath 0..1, set by the player; bubbles show while it is below 1
var _air_row: Control
var _look: Control          # the look-at panel, top centre (like the Jade mod)
var _look_info := {}
var _look_icon: ItemIcon    # the block or item's picture, left of the panel's text (ADR 0093)
var _shown_air := -1


func setup(p_inv: PlayerInventory) -> void:
	player_inv = p_inv
	_look_icon.items = p_inv.items
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
	_off_icon = ItemIcon.new()
	_off_icon.position = Vector2(OFF_X - SLOT + 6, 6)
	_off_icon.size = Vector2(SLOT - 12, SLOT - 12)
	_off_icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_hotbar.add_child(_off_icon)

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

	_air_row = Control.new()
	_air_row.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_air_row.position = Vector2(w / 2.0 - 10 * 18, -SLOT - 8 - 42)   # right half, above the hunger row (vanilla)
	_air_row.size = Vector2(10 * 18, 20)
	_air_row.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_air_row.draw.connect(_draw_air)
	add_child(_air_row)
	_hunger_row = Control.new()
	_hunger_row.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_hunger_row.position = Vector2(w / 2.0 - 10 * 18, -SLOT - 8 - 22)
	_hunger_row.size = Vector2(10 * 18, 20)
	_hunger_row.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_hunger_row.draw.connect(_draw_hunger)
	add_child(_hunger_row)
	_thermo = Control.new()
	_thermo.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_thermo.position = Vector2(-w / 2.0 - 38, -SLOT - 8 - 30)   # reaches above the hotbar: reading, tube, bulb
	_thermo.size = Vector2(32, SLOT + 30)
	_thermo.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_thermo.draw.connect(_draw_thermo)
	add_child(_thermo)

	_look = Control.new()
	_look.set_anchors_preset(Control.PRESET_CENTER_TOP)
	_look.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_look.draw.connect(_draw_look)
	add_child(_look)
	_look_icon = ItemIcon.new()
	_look_icon.size = Vector2(LOOK_ICON, LOOK_ICON)
	_look_icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_look.add_child(_look_icon)

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

	_brush_label = Label.new()
	_brush_label.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	_brush_label.add_theme_font_size_override("font_size", 13)
	_brush_label.add_theme_color_override("font_color", Color(0.92, 0.9, 0.84, 0.9))
	_brush_label.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.8))
	_brush_label.add_theme_constant_override("shadow_offset_x", 1)
	_brush_label.add_theme_constant_override("shadow_offset_y", 1)
	_brush_label.position = Vector2((9 * SLOT + 8 * GAP) / 2.0 + SLOT + 20, -SLOT - 6)
	_brush_label.size = Vector2(260, 40)
	add_child(_brush_label)


## What the crosshair is on: {title, icon? (item id), mat? (material id when no item), line? ("\n" for
## more lines), tool?, ok?, mine? (hardness and drops), drops?}; {} hides the panel.
func set_look(info: Dictionary) -> void:
	_look_info = info
	if info.get("icon", "") != "":
		_look_icon.stack = {"id": info["icon"]}
	elif int(info.get("mat", 0)) > 0:
		_look_icon.stack = {"id": "", "mat": info["mat"]}
	else:
		_look_icon.stack = {}
	_look.queue_redraw()


func _draw_look() -> void:
	if _look_info.is_empty():
		return
	if _font == null:
		_font = ThemeDB.fallback_font
	var lines := []   # [text, size, colour, mark: "" | "ok" | "no"]
	lines.append([String(_look_info["title"]), 17, Color(0.97, 0.95, 0.9), ""])
	if _look_info.has("line"):
		for l in String(_look_info["line"]).split("\n"):   # a station's contents: one line each (ADR 0078)
			lines.append([l, 13, Color(0.8, 0.82, 0.86), ""])
	if _look_info.has("tool"):
		var ok: bool = _look_info.get("ok", false)
		lines.append([String(_look_info["tool"]), 13, Color(0.5, 0.9, 0.45) if ok else Color(0.95, 0.45, 0.4), "ok" if ok else "no"])
	if _look_info.has("mine"):
		lines.append([String(_look_info["mine"]), 12, Color(0.7, 0.72, 0.76), ""])
	if not _look_info.get("drops", true):
		lines.append(["Drops nothing with this", 12, Color(0.95, 0.7, 0.4), ""])
	var w := 0.0
	var h := 10.0
	for l in lines:
		w = maxf(w, _font.get_string_size(l[0], HORIZONTAL_ALIGNMENT_LEFT, -1, l[1]).x + (16.0 if l[3] != "" else 0.0))
		h += l[1] + 5
	w += 28
	var pic := 0.0 if _look_icon.stack.is_empty() else LOOK_ICON + 10.0
	w += pic
	h = maxf(h, LOOK_ICON + 12.0 if pic > 0.0 else 0.0)
	var box := Rect2(-w / 2.0, 10, w, h)
	_look_icon.position = box.position + Vector2(12, 6)
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.03, 0.03, 0.05, 0.72)
	sb.border_color = Color(1, 1, 1, 0.14)
	sb.set_border_width_all(1)
	sb.set_corner_radius_all(8)
	_look.draw_style_box(UiStyle.skin("hud_box", sb), box)
	var y := box.position.y + 6
	for l in lines:
		y += l[1]
		var x := box.position.x + 14 + pic
		if l[3] == "ok":   # a tick
			_look.draw_polyline(PackedVector2Array([Vector2(x, y - 5), Vector2(x + 3, y - 2), Vector2(x + 9, y - 9)]), l[2], 2.0)
			x += 16
		elif l[3] == "no":   # a cross
			_look.draw_line(Vector2(x, y - 9), Vector2(x + 8, y - 1), l[2], 2.0)
			_look.draw_line(Vector2(x + 8, y - 9), Vector2(x, y - 1), l[2], 2.0)
			x += 16
		_look.draw_string(_font, Vector2(x, y), l[0], HORIZONTAL_ALIGNMENT_LEFT, -1, l[1], l[2])
		y += 5


## The F3 overlay draws its own axis crosshair instead.
func set_crosshair_visible(v: bool) -> void:
	_crosshair.visible = v


## Creative hides the hearts (no damage), like vanilla.
func set_hearts_visible(v: bool) -> void:
	_hearts.visible = v
	_hunger_row.visible = v   # creative: no hunger or temperature either
	_thermo.visible = v


# ---------------- the pickup feed (plan item D11) ----------------

const FEED_S := 3.0
const FEED_LINES := 6
var _feed: VBoxContainer
var _feed_rows: Array = []   # [{key, label: Label, count, t}]
var _full_t := 0.0


## "+3 Flint" in the bottom right corner; the same item within FEED_S adds to its line.
func note_pickup(item_name: String, n: int) -> void:
	_feed_line(item_name, n, Color(0.95, 0.95, 0.9))


## "Inventory full", at most every 3 s.
func note_full() -> void:
	if _full_t > 0.0:
		return
	_full_t = 3.0
	_feed_line("Inventory full", 0, Color(1.0, 0.45, 0.35))


func _feed_line(key: String, n: int, c: Color) -> void:
	if _feed == null:
		_feed = VBoxContainer.new()
		_feed.set_anchors_preset(Control.PRESET_BOTTOM_RIGHT)
		_feed.alignment = BoxContainer.ALIGNMENT_END
		_feed.grow_horizontal = Control.GROW_DIRECTION_BEGIN
		_feed.grow_vertical = Control.GROW_DIRECTION_BEGIN
		_feed.position = Vector2(-16, -96)
		_feed.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(_feed)
	for r in _feed_rows:
		if r["key"] == key:
			r["count"] += n
			r["t"] = FEED_S
			(r["label"] as Label).text = _feed_text(key, int(r["count"]))
			(r["label"] as Label).modulate.a = 1.0
			return
	var l := Label.new()
	l.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	l.add_theme_font_size_override("font_size", 15)
	l.add_theme_color_override("font_color", c)
	l.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.85))
	l.add_theme_constant_override("outline_size", 4)
	l.text = _feed_text(key, n)
	_feed.add_child(l)
	_feed_rows.append({"key": key, "label": l, "count": n, "t": FEED_S})
	while _feed_rows.size() > FEED_LINES:
		(_feed_rows.pop_front()["label"] as Node).queue_free()


static func _feed_text(key: String, n: int) -> String:
	return key if n <= 0 else "+%d %s" % [n, key]


func _update_feed(delta: float) -> void:
	_full_t = maxf(0.0, _full_t - delta)
	for i in range(_feed_rows.size() - 1, -1, -1):
		var r: Dictionary = _feed_rows[i]
		r["t"] -= delta
		(r["label"] as Label).modulate.a = clampf(r["t"] / 0.6, 0.0, 1.0)
		if r["t"] <= 0.0:
			(r["label"] as Node).queue_free()
			_feed_rows.remove_at(i)


func _process(delta: float) -> void:
	if not _feed_rows.is_empty() or _full_t > 0.0:
		_update_feed(delta)
	if _name_timer > 0.0:
		_name_timer -= delta
		_name_label.modulate.a = clampf(_name_timer / 0.5, 0.0, 1.0)
		_pill.modulate.a = _name_label.modulate.a
	_progress.queue_redraw()
	if survival != null:
		var hu := ceili(survival.hunger)
		if hu != _shown_hunger:
			_shown_hunger = hu
			_hunger_row.queue_redraw()
		var bc := roundi(survival.body_c * 10.0)
		if bc != _shown_body:
			_shown_body = bc
			_thermo.queue_redraw()
	var bubbles := ceili(air * 10.0) if air < 1.0 else -1
	if bubbles != _shown_air:
		_shown_air = bubbles
		_air_row.queue_redraw()
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
	_off_icon.items = player_inv.items
	_off_icon.stack = player_inv.offhand()
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
	show_toast(player_inv.items.display_name(h["id"]))


## The brush line right of the hotbar ("" hides it): shape, size and the keys that change them.
func set_brush_info(text: String) -> void:
	if _brush_label and _brush_label.text != text:
		_brush_label.text = text


## A short line above the hotbar (the held item's name, the brush size), fading after 2 s.
func show_toast(text: String) -> void:
	_name_label.text = text
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
	_pill.draw_style_box(UiStyle.skin("hud_box", sb), Rect2(Vector2.ZERO, _pill.size))


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
	tray = UiStyle.skin("hotbar", tray)
	_hotbar.draw_style_box(tray, Rect2(0, 0, _hotbar.size.x, SLOT))
	var cell := StyleBoxFlat.new()
	cell.bg_color = Color(0, 0, 0, 0.38)
	cell.border_color = Color(1, 1, 1, 0.07)
	cell.set_border_width_all(1)
	cell.set_corner_radius_all(4)
	cell = UiStyle.skin("hotbar_slot", cell)
	for i in 9:
		_hotbar.draw_style_box(cell, Rect2(i * (SLOT + GAP), 0, SLOT, SLOT))
	if player_inv and not player_inv.offhand().is_empty():
		var off := Rect2(OFF_X - SLOT, 0, SLOT, SLOT)
		_hotbar.draw_style_box(tray, off)
		_hotbar.draw_style_box(cell, off)
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
		_hotbar.draw_style_box(UiStyle.skin("hotbar_selected", sel), r)
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
	for i in ceili(player_inv.max_health / 2.0):   # a balanced diet adds hearts (ADR 0061): ten to a row, more rows upward
		var hp := player_inv.health - i * 2
		var ox := (i % 10) * 18
		var oy := -(i / 10) * 20
		# outline pass: every heart pixel grown by one HUD pixel in each direction
		for y in HEART.size():
			for x in 7:
				if HEART[y][x] != "1":
					continue
				_hearts.draw_rect(Rect2(ox + x * px - 1, oy + 2 + y * px - 1, px + 2, px + 2), outline)
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
				_hearts.draw_rect(Rect2(ox + x * px, oy + 2 + y * px, px, px), c)


# 7x7 pixel air bubble: rim, a light upper-left, a dark lower-right
const BUBBLE := ["0011100", "0122210", "1233321", "1233341", "1233441", "0144410", "0011100"]

func _draw_air() -> void:
	if _shown_air < 0:
		return
	var px := 2
	var cols := [Color(0, 0, 0, 0), Color(0.12, 0.2, 0.45), Color(0.85, 0.95, 1.0), Color(0.45, 0.7, 1.0), Color(0.25, 0.45, 0.85)]
	for i in _shown_air:
		var ox := (9 - i) * 18   # bubbles pop from the left, like vanilla's from the right edge inwards
		for y in BUBBLE.size():
			for x in 7:
				var k := int(BUBBLE[y][x])
				if k > 0:
					_air_row.draw_rect(Rect2(ox + x * px, 2 + y * px, px, px), cols[k])


# 7x7 food icon (a drumstick): o outline, b bone, m meat, l light, d dark
const FOOD := ["0000bb0", "000bbb0", "00ommo0", "0omlmo0", "omllmo0", "ommmdo0", "0oooo00"]

func _draw_hunger() -> void:
	if survival == null:
		return
	var px := 2
	var cols := {"o": Color(0.08, 0.04, 0.02, 0.95), "b": Color(0.92, 0.9, 0.82), "m": Color(0.72, 0.38, 0.16),
		"l": Color(0.92, 0.6, 0.32), "d": Color(0.5, 0.24, 0.1)}
	var empty := Color(0.2, 0.18, 0.18)
	var h := survival.hunger
	for i in 10:
		var ox := (9 - i) * 18   # full from the right, emptying toward the left (vanilla)
		var v := h - i * 2.0
		for y in FOOD.size():
			for x in 7:
				var k: String = FOOD[y][x]
				if k == "0":
					continue
				var c: Color = cols[k]
				var filled := v >= 2.0 or (v >= 1.0 and x >= 3)
				if not filled and k != "o":
					c = empty
				_hunger_row.draw_rect(Rect2(ox + x * px, 2 + y * px, px, px), c)


## The body temperature as a pixel thermometer: a glass tube from 31 to 43 C over a round bulb, the
## fill blue when cold, red when hot, green between, a tick at 37 and the reading written above it
## (a bare bar read as nothing, playtest 2026-10-27).
func _draw_thermo() -> void:
	if survival == null:
		return
	var b := survival.body_c
	var c := Color(0.35, 0.8, 0.35)
	if b < Settings.cold_c:
		c = Color(0.3, 0.55, 1.0)
	elif b > Settings.hot_c:
		c = Color(1.0, 0.35, 0.2)
	var outline := Color(0.05, 0.03, 0.04, 0.95)
	var cx := roundf(_thermo.size.x / 2.0)
	var bulb_y := _thermo.size.y - 12.0
	var tube := Rect2(cx - 3, 18, 6, bulb_y - 16)
	# the bulb is a 10 x 10 pixel disc: a plus of two rects
	var bulb := [Rect2(cx - 5, bulb_y + 2, 10, 6), Rect2(cx - 3, bulb_y, 6, 10)]
	for r in [tube] + bulb:
		_thermo.draw_rect(r.grow(2), outline)
	_thermo.draw_rect(tube, Color(0.82, 0.86, 0.9, 0.35))   # empty glass
	for r in bulb:
		_thermo.draw_rect(r, c)
	var h := roundf(tube.size.y * clampf((b - 31.0) / 12.0, 0.0, 1.0))
	_thermo.draw_rect(Rect2(tube.position.x, tube.end.y - h, tube.size.x, h), c)
	_thermo.draw_rect(Rect2(cx - 3, bulb_y + 2, 2, 2), Color(1, 1, 1, 0.55))   # glint on the bulb
	var mark := roundf(tube.position.y + tube.size.y * (1.0 - (37.0 - 31.0) / 12.0))
	_thermo.draw_rect(Rect2(tube.end.x + 2, mark - 1, 4, 2), Color(1, 1, 1, 0.85))
	var font := ThemeDB.fallback_font
	_thermo.draw_string_outline(font, Vector2(0, 12), "%d°" % roundi(b), HORIZONTAL_ALIGNMENT_CENTER, _thermo.size.x, 13, 4, outline)
	_thermo.draw_string(font, Vector2(0, 12), "%d°" % roundi(b), HORIZONTAL_ALIGNMENT_CENTER, _thermo.size.x, 13, c)


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
