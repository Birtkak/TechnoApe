class_name QuestBookView
extends Control

## The progression book (ADR 0028), FTB Quests style, opened from the book
## button top left of the inventory screen: chapters on the left, the open
## chapter's quests as a dependency graph in the middle, the selected
## quest's text and tasks on the right. Clicking a task's item closes the
## book and opens that item's recipes in the browser. The graph opens fitted
## to the canvas; the wheel zooms and a drag pans it (FTB Quests). Optional
## side quests are round. Logic lives in QuestBook; this only lays out and draws.

signal show_recipes(item_id: String)

const SIDEBAR := 210.0
const DETAIL := 330.0
const NODE := 50.0
const HEADER := 44.0

const C_BG := Color(0.09, 0.09, 0.11, 0.97)
const C_PANE := Color(0.13, 0.13, 0.16)
const C_EDGE := Color(0.32, 0.30, 0.38)
const C_TEXT := Color(0.9, 0.9, 0.92)
const C_DIM := Color(0.55, 0.55, 0.6)
const C_DONE := Color(0.42, 0.85, 0.42)
const C_OPEN := Color(0.98, 0.80, 0.30)

var book: QuestBook
var items: ItemRegistry
var chapter := ""
var selected := ""
var _frame: Control
var _canvas: Control
var _detail: VBoxContainer
var _chapters: VBoxContainer
var _close: Button
var _nodes := {}          # quest id -> QuestNode
var _cell := 80.0
var _origin := Vector2.ZERO
var _zoom := 1.0          # 1 = the chapter fits the canvas
var _pan := Vector2.ZERO  # pixels the graph is dragged by
var _drag_from := Vector2.INF
var _dragged := false
var _cycle: Array = []    # [ItemIcon, ids] task icons that show several items, one per second
var _cycle_t := 0.0
## The Discoveries tab (ADR 0061): a sidebar entry above the chapters that shows DiscoveryView over the canvas.
const JOURNAL := "#discoveries"
var discoveries: Discoveries
var trackers: Trackers       # the HUD trackers: a Track button on each quest
var _journal: DiscoveryView


func setup(p_book: QuestBook, p_items: ItemRegistry) -> void:
	book = p_book
	items = p_items
	if not book.chapters.is_empty():
		chapter = book.chapters[0]["id"]
	book.quest_completed.connect(func(_id: String) -> void:
		if visible:
			_rebuild())


func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_STOP   # nothing under the book takes clicks
	draw.connect(func() -> void: draw_rect(Rect2(Vector2.ZERO, size), Color(0, 0, 0, 0.5)))
	_frame = Control.new()
	_frame.mouse_filter = Control.MOUSE_FILTER_PASS
	_frame.draw.connect(_draw_frame)
	add_child(_frame)
	_canvas = Control.new()
	_canvas.clip_contents = true
	_canvas.mouse_filter = Control.MOUSE_FILTER_STOP
	_canvas.draw.connect(_draw_canvas)
	_canvas.gui_input.connect(_on_canvas_input)
	_frame.add_child(_canvas)
	_journal = DiscoveryView.new()
	_journal.visible = false
	_canvas.add_child(_journal)
	_chapters = VBoxContainer.new()
	_chapters.add_theme_constant_override("separation", 6)
	_frame.add_child(_chapters)
	var scroll := ScrollContainer.new()
	scroll.name = "DetailScroll"
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	_frame.add_child(scroll)
	_detail = VBoxContainer.new()
	_detail.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_detail.add_theme_constant_override("separation", 8)
	scroll.add_child(_detail)
	_close = Button.new()
	_close.text = "X"
	_close.focus_mode = Control.FOCUS_NONE
	_close.custom_minimum_size = Vector2(30, 28)
	_close.pressed.connect(close)
	_frame.add_child(_close)
	get_viewport().size_changed.connect(func() -> void:
		if visible:
			_rebuild())


func open() -> void:
	visible = true
	if selected == "" or not book.quests.has(selected):
		selected = _first_open()
	_rebuild()


func close() -> void:
	visible = false


## The first quest you can work on now, for the detail pane on open: the
## main road before the optional side quests.
func _first_open() -> String:
	for want_optional in [false, true]:
		for q in book.chapter_quests(chapter):
			if q.get("optional", false) == want_optional and not book.is_done(q["id"]) and book.is_unlocked(q["id"]):
				return q["id"]
	return ""


func select(id: String) -> void:
	selected = id
	for qid in _nodes:
		_nodes[qid].selected = qid == id
		_nodes[qid].queue_redraw()
	_build_detail()


func open_chapter(id: String) -> void:
	chapter = id
	selected = _first_open()
	_zoom = 1.0
	_pan = Vector2.ZERO
	_rebuild()


## Wheel: zoom about the mouse; drag the background: pan; a click on it deselects.
func _on_canvas_input(e: InputEvent) -> void:
	if e is InputEventMouseButton and e.pressed and e.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		var z := clampf(_zoom * (1.15 if e.button_index == MOUSE_BUTTON_WHEEL_UP else 1.0 / 1.15), 1.0, 3.0)
		var k := z / _zoom
		var mid := _mid()
		_pan = (_pan + mid - e.position) * k + e.position - mid   # the point under the mouse stays put
		_zoom = z
		if _zoom == 1.0:
			_pan = Vector2.ZERO
		_place_nodes()
		_canvas.accept_event()
	elif e is InputEventMouseButton and e.button_index == MOUSE_BUTTON_LEFT:
		if e.pressed:
			_drag_from = e.position
			_dragged = false
		else:
			if not _dragged:
				select("")
			_drag_from = Vector2.INF
	elif e is InputEventMouseMotion and _drag_from != Vector2.INF:
		if _dragged or e.position.distance_to(_drag_from) > 4.0:
			_dragged = true
			_pan += e.relative
			_place_nodes()


func state_of(id: String) -> String:
	if book.is_done(id):
		return "done"
	return "open" if book.is_unlocked(id) else "locked"


func _process(delta: float) -> void:
	if not visible:
		return
	for qid in _nodes:
		if _nodes[qid].state == "open":
			_nodes[qid].queue_redraw()   # the available border pulses
	_cycle_t += delta
	if _cycle_t >= 1.0:
		_cycle_t = 0.0
		for c in _cycle:
			var ids: Array = c[1]
			var i := (ids.find(c[0].stack.get("id", "")) + 1) % ids.size()
			c[0].stack = {"id": ids[i], "count": c[0].stack.get("count", 1)}


# ---------------- layout ----------------

func _rebuild() -> void:
	var vw := get_viewport_rect().size
	var fr := Rect2(16, 16, vw.x - 32, vw.y - 32)
	_frame.position = fr.position
	_frame.size = fr.size
	_frame.queue_redraw()
	_chapters.position = Vector2(12, 56)
	_chapters.size = Vector2(SIDEBAR - 24, fr.size.y - 68)
	var cx := SIDEBAR
	var cw := fr.size.x - SIDEBAR - DETAIL
	_canvas.position = Vector2(cx, 8)
	_canvas.size = Vector2(cw, fr.size.y - 16)
	var scroll: ScrollContainer = _frame.get_node("DetailScroll")
	scroll.position = Vector2(fr.size.x - DETAIL + 14, 48)
	scroll.size = Vector2(DETAIL - 28, fr.size.y - 60)
	_detail.custom_minimum_size.x = DETAIL - 40
	_close.position = fr.size - Vector2(44, 40)   # bottom right: quest toasts slide in over the top right
	_build_chapters()
	_build_nodes()
	_build_detail()


func _build_chapters() -> void:
	for c in _chapters.get_children():
		c.queue_free()
	if discoveries != null:   # the Discoveries tab, above the chapters (not a chapter: quests.json stays part F's)
		var j := ChapterButton.new()
		j.items = items
		j.icon_id = "copper_prospecting_pick"
		j.title = "Discoveries"
		j.progress = discoveries.total_progress()
		j.active = chapter == JOURNAL
		j.custom_minimum_size = Vector2(SIDEBAR - 24, 52)
		j.pressed.connect(open_chapter.bind(JOURNAL))
		_chapters.add_child(j)
		_chapters.add_child(HSeparator.new())
	for c in book.chapters:
		var b := ChapterButton.new()
		b.items = items
		b.icon_id = c["icon"]
		b.title = c["title"]
		b.progress = book.chapter_progress(c["id"])
		b.active = c["id"] == chapter
		b.custom_minimum_size = Vector2(SIDEBAR - 24, 52)
		b.pressed.connect(open_chapter.bind(c["id"]))
		_chapters.add_child(b)


func _build_nodes() -> void:
	for n in _nodes.values():
		n.queue_free()
	_nodes.clear()
	for q in book.chapter_quests(chapter):
		var n := QuestNode.new()
		n.items = items
		n.id = q["id"]
		n.final = q.get("final", false)
		n.optional = q.get("optional", false)
		n.state = state_of(q["id"])
		n.selected = q["id"] == selected
		n.tooltip_text = q["title"] + (" (optional)" if n.optional else "")
		n.pressed.connect(select)
		_canvas.add_child(n)
		_nodes[q["id"]] = n
	_place_nodes()
	for q in book.chapter_quests(chapter):
		_nodes[q["id"]].set_icon(q["icon"])
	_journal.visible = chapter == JOURNAL and discoveries != null
	if _journal.visible:
		_journal.book = discoveries
		_journal.items = items
		_journal.size = _canvas.size
		_journal.refresh()


func _mid() -> Vector2:
	return Vector2(_canvas.size.x / 2.0, HEADER + (_canvas.size.y - HEADER) / 2.0)


## Cell size and origin from the fit, zoom and pan; nodes follow.
func _place_nodes() -> void:
	var qs := book.chapter_quests(chapter)
	if qs.is_empty():
		return
	var lo := Vector2(INF, INF)
	var hi := Vector2(-INF, -INF)
	for q in qs:
		var p := Vector2(q["pos"][0], q["pos"][1])
		lo = lo.min(p)
		hi = hi.max(p)
	var span := (hi - lo).max(Vector2.ONE)
	var area := _canvas.size - Vector2(0, HEADER) - Vector2(NODE + 48, NODE + 48)
	_cell = minf(96.0, minf(area.x / span.x, area.y / span.y)) * _zoom
	_origin = _mid() - (lo + hi) / 2.0 * _cell + _pan
	var node := clampf(_cell * 0.7, 34.0, NODE * _zoom)
	for q in qs:
		var n: QuestNode = _nodes[q["id"]]
		var s := node * (1.2 if n.final else 1.0)
		n.size = Vector2(s, s)
		n.position = _center(q) - n.size / 2.0
		n.fit_icon()
	_canvas.queue_redraw()


func _center(q: Dictionary) -> Vector2:
	return _origin + Vector2(q["pos"][0], q["pos"][1]) * _cell


func _label(text: String, size_px: int, color: Color, wrap := false) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", size_px)
	l.add_theme_color_override("font_color", color)
	if wrap:
		l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		l.custom_minimum_size.x = DETAIL - 40
	return l


func _build_detail() -> void:
	for c in _detail.get_children():
		c.queue_free()
	_cycle.clear()
	if selected == "" and chapter == JOURNAL and discoveries != null:
		var t := discoveries.total_progress()
		_detail.add_child(_label("Discoveries", 22, C_TEXT))
		_detail.add_child(_label("The first time you find a biome, a landmark, an animal, an ore or a vein, it is written here.", 15, C_DIM, true))
		_detail.add_child(_label("%d of %d found" % [t.x, t.y], 15, C_DONE if t.x == t.y else C_TEXT))
		for k in discoveries.kinds:
			var kp := discoveries.kind_progress(k["id"])
			if kp.y > 0:
				_detail.add_child(_label("%s: %d / %d" % [k["title"], kp.x, kp.y], 14, C_DONE if kp.x == kp.y else C_DIM))
		return
	if selected == "":
		for c in book.chapters:
			if c["id"] == chapter:
				var p := book.chapter_progress(chapter)
				_detail.add_child(_label(c["title"], 22, C_TEXT))
				_detail.add_child(_label(c.get("description", ""), 15, C_DIM, true))
				_detail.add_child(_label("%d of %d quests complete" % [p.x, p.y], 15, C_DONE if p.x == p.y else C_TEXT))
				_detail.add_child(_label("Click a quest to read it.", 14, C_DIM, true))
		return
	var q: Dictionary = book.quests[selected]
	var st := state_of(selected)
	_detail.add_child(_label(q["title"], 22, C_DONE if st == "done" else (C_TEXT if st == "open" else C_DIM), true))
	var status := "Complete" if st == "done" else "Available"
	if q.get("optional", false):
		status += " - optional"
	if st == "locked":
		var need := []
		for d in q.get("deps", []):
			if not book.is_done(d):
				need.append(book.quests[d]["title"])
		status = "Locked: finish " + ", ".join(need) + " first"
	_detail.add_child(_label(status, 14, C_DONE if st == "done" else (C_OPEN if st == "open" else C_DIM), true))
	_detail.add_child(_label(q.get("description", ""), 15, Color(0.82, 0.82, 0.86), true))
	if trackers != null and st != "done":
		var tb := Button.new()
		tb.focus_mode = Control.FOCUS_NONE
		tb.text = "Untrack" if trackers.is_pinned("quest", selected) else "Track on the HUD"
		var qid := selected
		tb.pressed.connect(func() -> void:
			trackers.toggle("quest", qid)
			_build_detail())
		_detail.add_child(tb)
	_detail.add_child(_label("Tasks (any one)" if q.get("mode", "all") == "any" else "Tasks", 16, C_TEXT))
	var tasks: Array = q["tasks"]
	for i in tasks.size():
		var row := TaskRow.new()
		row.items = items
		row.text = task_text(tasks[i])
		row.count = QuestBook.task_count(tasks[i])
		row.have = book.task_progress(selected, i)
		row.done = book.task_done(selected, i)
		row.custom_minimum_size = Vector2(DETAIL - 40, 40)
		var ids := task_item_ids(tasks[i])
		row.clicked.connect(func(id: String) -> void: show_recipes.emit(id))
		_detail.add_child(row)
		row.set_icon(ids[0] if not ids.is_empty() else "")
		if ids.size() > 1:
			_cycle.append([row.icon, ids])
	_detail.add_child(_label("Click a task's item to see its recipes.", 13, C_DIM, true))


## The task as a line of text: its own "text", or the item's name / "Any <tag>".
func task_text(t: Dictionary) -> String:
	return task_text_for(items, t)


## task_text without a view (the HUD trackers, ADR 0061).
static func task_text_for(items: ItemRegistry, t: Dictionary) -> String:
	if t.has("text"):
		return t["text"]
	var specs := QuestBook.task_specs(t)
	var names := []
	for s in specs:
		names.append("Any " + s.substr(1).replace("_", " ") if s.begins_with("#") else items.display_name(s))
	var verb := "Fire: " if t.get("type") == "event" and t.get("event") == "fired" else ""
	return verb + (" or ".join(names) if names.size() <= 2 else names[0] + " or another")


## The item ids a task's icon cycles through.
func task_item_ids(t: Dictionary) -> Array:
	var ids := []
	for s in QuestBook.task_specs(t):
		if s.begins_with("#"):
			var tagged := items.items_with_tag(s.substr(1))
			tagged.sort()
			ids.append_array(tagged)
		else:
			ids.append(s)
	return ids


# ---------------- drawing ----------------

func _draw_frame() -> void:
	var r := Rect2(Vector2.ZERO, _frame.size)
	var sb := StyleBoxFlat.new()
	sb.bg_color = C_BG
	sb.border_color = C_EDGE
	sb.set_border_width_all(3)
	sb.set_corner_radius_all(8)
	_frame.draw_style_box(sb, r)
	# sidebar and detail panes
	_frame.draw_rect(Rect2(6, 6, SIDEBAR - 12, r.size.y - 12), C_PANE)
	_frame.draw_rect(Rect2(r.size.x - DETAIL + 6, 6, DETAIL - 12, r.size.y - 12), C_PANE)
	var font := ThemeDB.fallback_font
	_frame.draw_string(font, Vector2(18, 36), "Progression", HORIZONTAL_ALIGNMENT_LEFT, -1, 20, C_TEXT)


func _draw_canvas() -> void:
	var s := _canvas.size
	# faint grid, FTB style
	var g := Color(1, 1, 1, 0.035)
	var step := _cell / 2.0
	if step > 4.0:
		var x := fposmod(_origin.x, step)
		while x < s.x:
			_canvas.draw_line(Vector2(x, HEADER), Vector2(x, s.y), g)
			x += step
		var y := HEADER + fposmod(_origin.y - HEADER, step)
		while y < s.y:
			_canvas.draw_line(Vector2(0, y), Vector2(s.x, y), g)
			y += step
	var font := ThemeDB.fallback_font
	for c in book.chapters:
		if c["id"] == chapter:
			var p := book.chapter_progress(chapter)
			_canvas.draw_string(font, Vector2(16, 30), c["title"], HORIZONTAL_ALIGNMENT_LEFT, -1, 22, C_TEXT)
			var bar := Rect2(s.x - 196, 16, 180, 12)
			_canvas.draw_rect(bar, Color(0.2, 0.2, 0.24))
			_canvas.draw_rect(Rect2(bar.position, Vector2(bar.size.x * p.x / maxf(1.0, p.y), bar.size.y)), C_DONE)
			_canvas.draw_string(font, Vector2(bar.position.x - 52, 28), "%d/%d" % [p.x, p.y], HORIZONTAL_ALIGNMENT_LEFT, -1, 15, C_DIM)
	# dependency lines under the nodes
	for q in book.chapter_quests(chapter):
		for d in q.get("deps", []):
			if not book.quests.has(d) or book.quests[d]["chapter"] != chapter:
				continue
			var col := Color(0.3, 0.3, 0.34)
			if book.is_done(d):
				col = C_DONE.darkened(0.25) if book.is_done(q["id"]) else C_OPEN.darkened(0.3)
			var a := _center(book.quests[d])
			var b := _center(q)
			_canvas.draw_line(a, b, col, 3.0, true)
			# arrow head halfway, pointing at the quest the dep unlocks
			var m := a.lerp(b, 0.5)
			var dir := (b - a).normalized()
			var nrm := Vector2(-dir.y, dir.x)
			_canvas.draw_colored_polygon(PackedVector2Array([m + dir * 7.0, m - dir * 5.0 + nrm * 6.0, m - dir * 5.0 - nrm * 6.0]), col)


## One quest on the graph: a rounded square (final: bigger, gold) holding the
## quest's icon; green with a check when done, a pulsing gold edge and "!"
## when available, dark when locked.
class QuestNode:
	extends Control
	signal pressed(id: String)
	var items: ItemRegistry
	var id := ""
	var state := "locked"
	var final := false
	var optional := false
	var selected := false
	var icon: ItemIcon
	var _hover := false

	func _init() -> void:
		mouse_filter = Control.MOUSE_FILTER_STOP
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())

	func set_icon(item_id: String) -> void:
		icon = ItemIcon.new()
		icon.items = items
		icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		icon.stack = {"id": item_id, "count": 1}
		icon.modulate = Color(0.38, 0.38, 0.42) if state == "locked" else Color.WHITE
		add_child(icon)
		fit_icon()

	func fit_icon() -> void:
		if icon:
			var s := size.x * 0.66
			icon.size = Vector2(s, s)
			icon.position = (size - icon.size) / 2.0

	func _draw() -> void:
		var r := Rect2(Vector2.ZERO, size)
		var sb := StyleBoxFlat.new()
		sb.set_corner_radius_all(int(size.x * (0.5 if optional else 0.18)))
		sb.set_border_width_all(3)
		match state:
			"done":
				sb.bg_color = Color(0.14, 0.27, 0.15)
				sb.border_color = Color(0.42, 0.85, 0.42)
			"open":
				var pulse := 0.65 + 0.35 * sin(Time.get_ticks_msec() / 250.0)
				sb.bg_color = Color(0.24, 0.22, 0.18)
				sb.border_color = Color(0.55, 0.40, 0.12).lerp(Color(0.98, 0.80, 0.30), pulse)
			_:
				sb.bg_color = Color(0.12, 0.12, 0.14)
				sb.border_color = Color(0.3, 0.3, 0.33)
		if final:
			sb.set_border_width_all(5)
			if state != "done":
				sb.border_color = Color(0.85, 0.65, 0.2) if state == "open" else Color(0.45, 0.38, 0.2)
		if _hover:
			sb.bg_color = sb.bg_color.lightened(0.15)
		draw_style_box(sb, r)
		if selected:
			draw_rect(r.grow(4), Color(1, 1, 1, 0.85), false, 2.0)
		var font := ThemeDB.fallback_font
		if state == "done":
			# check mark badge, bottom right
			var c := Vector2(size.x - 6, size.y - 6)
			draw_circle(c, 9, Color(0.2, 0.6, 0.25))
			draw_polyline(PackedVector2Array([c + Vector2(-5, 0), c + Vector2(-1, 4), c + Vector2(5, -4)]), Color.WHITE, 2.5, true)
		elif state == "open":
			var c := Vector2(size.x - 6, 6)
			draw_circle(c, 9, Color(0.85, 0.55, 0.1))
			draw_string(font, c + Vector2(-3, 6), "!", HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color.WHITE)

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
			pressed.emit(id)
			accept_event()


## A task line in the detail pane: item icon, text, "have / need" and a check.
## Hovering shows the item's name (InventoryScreen's tooltip); clicking opens its recipes.
class TaskRow:
	extends Control
	signal clicked(item_id: String)
	var items: ItemRegistry
	var text := ""
	var count := 1
	var have := 0
	var done := false
	var icon: ItemIcon
	var _hover := false

	func _init() -> void:
		mouse_filter = Control.MOUSE_FILTER_STOP
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())

	func set_icon(item_id: String) -> void:
		icon = ItemIcon.new()
		icon.items = items
		icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		icon.position = Vector2(4, 4)
		icon.size = Vector2(32, 32)
		icon.stack = {"id": item_id, "count": 1} if item_id != "" else {}
		add_child(icon)

	func hover_item_id() -> String:
		return icon.stack.get("id", "") if icon else ""

	func _draw() -> void:
		var r := Rect2(Vector2.ZERO, size)
		draw_rect(r, Color(0.2, 0.2, 0.24) if _hover else Color(0.17, 0.17, 0.2))
		var font := ThemeDB.fallback_font
		var c := Color(0.42, 0.85, 0.42) if done else Color(0.9, 0.9, 0.92)
		draw_string(font, Vector2(44, 25), text, HORIZONTAL_ALIGNMENT_LEFT, size.x - 44 - 70, 14, c)
		var tally := "%d/%d" % [count if done else have, count]
		draw_string(font, Vector2(size.x - 66, 25), tally, HORIZONTAL_ALIGNMENT_RIGHT, 36, 14, c)
		var b := Vector2(size.x - 16, size.y / 2.0)
		if done:
			draw_polyline(PackedVector2Array([b + Vector2(-6, 0), b + Vector2(-2, 5), b + Vector2(6, -5)]), c, 2.5, true)
		else:
			draw_rect(Rect2(b - Vector2(6, 6), Vector2(12, 12)), Color(0.45, 0.45, 0.5), false, 2.0)

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT and hover_item_id() != "":
			clicked.emit(hover_item_id())
			accept_event()


## A chapter in the sidebar: icon, title and a progress bar.
class ChapterButton:
	extends Control
	signal pressed
	var items: ItemRegistry
	var icon_id := ""
	var title := ""
	var progress := Vector2i.ZERO
	var active := false
	var _hover := false

	func _ready() -> void:
		mouse_filter = Control.MOUSE_FILTER_STOP
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())
		var ic := ItemIcon.new()
		ic.items = items
		ic.mouse_filter = Control.MOUSE_FILTER_IGNORE
		ic.position = Vector2(8, 10)
		ic.size = Vector2(32, 32)
		ic.stack = {"id": icon_id, "count": 1}
		add_child(ic)

	func _draw() -> void:
		var r := Rect2(Vector2.ZERO, size)
		var bg := Color(0.24, 0.22, 0.30) if active else Color(0.16, 0.16, 0.19)
		draw_rect(r, bg.lightened(0.08) if _hover else bg)
		if active:
			draw_rect(Rect2(0, 0, 3, size.y), Color(0.98, 0.80, 0.30))
		var font := ThemeDB.fallback_font
		draw_string(font, Vector2(48, 22), title, HORIZONTAL_ALIGNMENT_LEFT, size.x - 52, 16, Color(0.92, 0.92, 0.94))
		var bar := Rect2(48, 32, size.x - 60, 8)
		draw_rect(bar, Color(0.1, 0.1, 0.12))
		draw_rect(Rect2(bar.position, Vector2(bar.size.x * progress.x / maxf(1.0, progress.y), bar.size.y)), Color(0.42, 0.85, 0.42))

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
			pressed.emit()
			accept_event()


## The book button top left of the inventory screen: a leather book with a
## gold clasp, a "!" while a quest is available, the chapter's progress below.
class BookButton:
	extends Control
	signal pressed
	var book: QuestBook
	var _hover := false

	func _init() -> void:
		mouse_filter = Control.MOUSE_FILTER_STOP
		size = Vector2(56, 64)
		tooltip_text = "Progression book"
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())

	func _draw() -> void:
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.776, 0.776, 0.776) if not _hover else Color(0.86, 0.86, 0.86)
		sb.border_color = Color(0.2, 0.2, 0.2)
		sb.set_border_width_all(3)
		sb.set_corner_radius_all(6)
		draw_style_box(sb, Rect2(Vector2.ZERO, size))
		# 16x16 pixel book at 2.5x: pages, cover, spine, clasp, ribbon
		var o := Vector2(8, 6)
		var p := 2.5
		var px := func(x: int, y: int, w: int, h: int, c: Color) -> void:
			draw_rect(Rect2(o + Vector2(x, y) * p, Vector2(w, h) * p), c)
		px.call(4, 1, 11, 14, Color(0.93, 0.89, 0.76))        # page block
		px.call(4, 2, 11, 1, Color(0.80, 0.75, 0.62))
		px.call(4, 13, 11, 1, Color(0.80, 0.75, 0.62))
		px.call(2, 0, 12, 15, Color(0.30, 0.16, 0.08))        # cover edge
		px.call(3, 1, 10, 13, Color(0.52, 0.29, 0.14))        # cover
		px.call(2, 0, 3, 15, Color(0.36, 0.19, 0.09))         # spine
		px.call(3, 2, 1, 1, Color(0.85, 0.68, 0.25))
		px.call(3, 12, 1, 1, Color(0.85, 0.68, 0.25))
		px.call(7, 5, 4, 4, Color(0.85, 0.68, 0.25))          # gold emblem
		px.call(8, 6, 2, 2, Color(0.45, 0.85, 0.42))
		px.call(10, 13, 2, 4, Color(0.75, 0.15, 0.15))        # ribbon
		if book == null or book.chapters.is_empty():
			return
		var ch: String = book.chapters[0]["id"]
		var prog := book.chapter_progress(ch)
		var bar := Rect2(6, size.y - 10, size.x - 12, 5)
		draw_rect(bar, Color(0.3, 0.3, 0.3))
		draw_rect(Rect2(bar.position, Vector2(bar.size.x * prog.x / maxf(1.0, prog.y), bar.size.y)), Color(0.3, 0.75, 0.3))
		var any_open := false
		for q in book.chapter_quests(ch):
			if not book.is_done(q["id"]) and book.is_unlocked(q["id"]):
				any_open = true
				break
		if any_open:
			var c := Vector2(size.x - 8, 8)
			draw_circle(c, 8, Color(0.85, 0.55, 0.1))
			draw_string(ThemeDB.fallback_font, c + Vector2(-3, 6), "!", HORIZONTAL_ALIGNMENT_LEFT, -1, 15, Color.WHITE)

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
			pressed.emit()
			accept_event()
