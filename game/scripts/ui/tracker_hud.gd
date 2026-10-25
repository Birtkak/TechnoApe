class_name TrackerHud
extends CanvasLayer

## The HUD trackers (ADR 0061): one small checklist per pinned recipe or tracked quest, at the right
## edge under part D's minimap (its bottom_y(), which moves with its size setting, corner and F3), each
## row ticked as the inventory fills. Draws Trackers' views; redrawn when the inventory or the pins change
## and once a second.

const TOP := 230.0           # before part D's minimap reports its edge
const WIDTH := 240.0
const ROW := 17.0
const HEAD := 26.0
const GAP := 8.0

var trackers: Trackers
var items: ItemRegistry
var _box: Control
var _icons: Array[ItemIcon] = []
var _views: Array = []
var _t := 0.0
var _pending := false
var minimap: Object          # part D's minimap: bottom_y() is its lower edge when top right, else 0


func setup(p_trackers: Trackers, p_items: ItemRegistry, inv_changed: Signal) -> void:
	trackers = p_trackers
	items = p_items
	trackers.changed.connect(refresh)
	inv_changed.connect(func() -> void:   # once per frame at most: a shift-click craft changes slots hundreds of times
		if not _pending:
			_pending = true
			refresh.call_deferred())
	refresh()


func _ready() -> void:
	layer = 5
	_box = Control.new()
	_box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_box.draw.connect(_draw_box)
	add_child(_box)


func _process(delta: float) -> void:
	_place()
	_t -= delta
	if _t <= 0.0:
		_t = 1.0
		refresh()


func refresh() -> void:
	_pending = false
	if trackers == null or _box == null:
		return
	_views = trackers.views()
	var h := 0.0
	for v in _views:
		h += HEAD + v["rows"].size() * ROW + 8.0 + GAP
	_box.size = Vector2(WIDTH, h)
	_place()
	while _icons.size() < _views.size():
		var ic := ItemIcon.new()
		ic.mouse_filter = Control.MOUSE_FILTER_IGNORE
		ic.size = Vector2(20, 20)
		_box.add_child(ic)
		_icons.append(ic)
	var y := 0.0
	for i in _icons.size():
		var ic := _icons[i]
		ic.visible = i < _views.size()
		if not ic.visible:
			continue
		ic.items = items
		ic.stack = {"id": _views[i]["icon"], "count": 1} if items != null and items.has(_views[i]["icon"]) else {}
		ic.position = Vector2(6, y + 3)
		y += HEAD + _views[i]["rows"].size() * ROW + 8.0 + GAP
	_box.queue_redraw()


## Right edge, under the minimap when it sits top right (each frame: F3 and settings move it).
func _place() -> void:
	if _box == null:
		return
	var top := TOP
	if minimap != null and minimap.has_method("bottom_y"):
		top = maxf(float(minimap.bottom_y()) + 8.0, 12.0)
	_box.position = Vector2(_box.get_viewport_rect().size.x - WIDTH - 12.0, top)


## What the trackers show (stats JSON, tests).
func summary() -> Array:
	return _views.map(func(v: Dictionary) -> Dictionary:
		return {"type": v["type"], "id": v["id"], "done": v["done"],
			"rows": v["rows"].map(func(r: Dictionary) -> String: return "%s %d/%d" % [r["text"], r["have"], r["need"]])})


func _draw_box() -> void:
	var font := ThemeDB.fallback_font
	var y := 0.0
	for v in _views:
		var h: float = HEAD + v["rows"].size() * ROW + 8.0
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.03, 0.03, 0.05, 0.62)
		sb.border_color = Color(0.42, 0.85, 0.42, 0.8) if v["done"] else Color(1, 1, 1, 0.14)
		sb.set_border_width_all(1)
		sb.set_corner_radius_all(6)
		_box.draw_style_box(sb, Rect2(0, y, WIDTH, h))
		var kind := "Quest" if v["type"] == "quest" else "Craft"
		_box.draw_string(font, Vector2(30, y + 18), "%s: %s" % [kind, v["title"]], HORIZONTAL_ALIGNMENT_LEFT, WIDTH - 36, 14,
			Color(0.55, 0.92, 0.5) if v["done"] else Color(0.97, 0.95, 0.9))
		var ry: float = y + HEAD
		for r in v["rows"]:
			var c := Color(0.5, 0.9, 0.45) if r["done"] else Color(0.85, 0.85, 0.88)
			var x := 12.0
			if r["done"]:   # a tick
				_box.draw_polyline(PackedVector2Array([Vector2(x, ry + 8), Vector2(x + 3, ry + 11), Vector2(x + 9, ry + 4)]), c, 2.0)
			else:           # an empty box
				_box.draw_rect(Rect2(x, ry + 3, 9, 9), Color(1, 1, 1, 0.5), false, 1.0)
			var count := "%d/%d" % [mini(r["have"], r["need"]), r["need"]]
			var cw := font.get_string_size(count, HORIZONTAL_ALIGNMENT_LEFT, -1, 12).x
			_box.draw_string(font, Vector2(x + 16, ry + 12), String(r["text"]), HORIZONTAL_ALIGNMENT_LEFT, WIDTH - 40 - cw, 12, c)
			_box.draw_string(font, Vector2(WIDTH - 8 - cw, ry + 12), count, HORIZONTAL_ALIGNMENT_LEFT, -1, 12, c)
			ry += ROW
		y += h + GAP
