class_name RecipeBrowserView
extends Control

## JEI-style browser drawn beside the inventory screen (ADR 0008): the item
## list right of the panel (search box below, pages above, wheel turns
## pages) and the recipe view drawn over the panel. Left click = recipes,
## right click = uses, also on ingredients inside a recipe. Bookmarks (A over
## any item) list left of the panel, as in JEI. Logic lives in RecipeBrowser;
## this only lays out and forwards clicks.

const S := 36
const PANEL := Vector2(352, 332)
const BM_TOP := 80.0   # bookmarks start below the progression book button (ADR 0028)

var model: RecipeBrowser
var items: ItemRegistry
var _list_panel: Control
var _cells: Array[BrowserCell] = []
var _search: LineEdit
var _page_label: Label
var _prev_page: Button
var _next_page: Button
var _overlay: Control           # dimmer + recipe panel
var _recipe_panel: Control
var _recipe_nodes: Array[Node] = []
var _spec_cells: Array[BrowserCell] = []   # cells showing a (possibly tag) spec, cycled each second
var _cycle_t := -1
var _list_rect := Rect2()
var _cols := 0
var cheat_give: Callable        # creative (ADR 0018): fn(item_id, shift) -> bool; LMB on the list takes items
var _rows := 0
var right_inset := 0.0         # room a right-docked side panel (the diet) takes beside the inventory
var _bm_panel: Control          # bookmarks, left of the panel
var _bm_cells: Array[BrowserCell] = []
var _bm_title: Label
var _bm_cols := 0
var _bm_rows := 0
var trackers: Trackers          # HUD trackers (ADR 0061): the recipe view's Pin button
var grid_fill: Callable         # fn(recipe, all, dry_run) -> CraftingGrid.fill's answer, while a crafting grid is on screen
var container_fill: Callable    # the same for a furnace's input slot while its screen is open (ADR 0093)
var grid_craft: Callable        # fn(recipe, all) -> {crafts, error}: the Craft button makes it into the inventory (ADR 0130)
var craft_button: Button        # below Fill while grid_craft is set, or null
var fill_button: Button         # the recipe view's Fill button, or null (tests and menu_test press it)
var bookmark_button: Button     # the recipe view's Mark button (bookmarks the shown recipe, ADR 0093)
var _tab_x := -1.0              # the selected station tab's left edge, for its highlight
const TABS := 7


func setup(p_model: RecipeBrowser) -> void:
	model = p_model
	items = model.items


func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_list_panel = Control.new()
	_list_panel.mouse_filter = Control.MOUSE_FILTER_STOP
	_list_panel.draw.connect(func() -> void:
		_list_panel.draw_rect(Rect2(Vector2.ZERO, _list_panel.size), Color(0, 0, 0, 0.35)))
	_list_panel.gui_input.connect(_on_list_input)
	add_child(_list_panel)
	_search = LineEdit.new()
	_search.placeholder_text = "Search (#tag)"
	_search.text_changed.connect(func(t: String) -> void:
		model.set_query(t)
		_rebuild_list())
	_list_panel.add_child(_search)
	_prev_page = _button("<", func() -> void: _turn_page(-1))
	_next_page = _button(">", func() -> void: _turn_page(1))
	_list_panel.add_child(_prev_page)
	_list_panel.add_child(_next_page)
	_page_label = Label.new()
	_page_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_list_panel.add_child(_page_label)
	_bm_panel = Control.new()
	_bm_panel.mouse_filter = Control.MOUSE_FILTER_STOP
	_bm_panel.draw.connect(func() -> void:
		_bm_panel.draw_rect(Rect2(Vector2.ZERO, _bm_panel.size), Color(0, 0, 0, 0.35)))
	add_child(_bm_panel)
	_bm_title = Label.new()
	_bm_title.position = Vector2(6, 6)
	_bm_title.add_theme_font_size_override("font_size", 14)
	_bm_panel.add_child(_bm_title)
	_overlay = Control.new()
	_overlay.set_anchors_preset(Control.PRESET_FULL_RECT)
	_overlay.mouse_filter = Control.MOUSE_FILTER_STOP
	_overlay.visible = false
	_overlay.gui_input.connect(func(e: InputEvent) -> void:
		if e is InputEventMouseButton and e.pressed and e.button_index == MOUSE_BUTTON_LEFT:
			close_recipes())
	add_child(_overlay)
	_recipe_panel = Control.new()
	_recipe_panel.mouse_filter = Control.MOUSE_FILTER_STOP
	_recipe_panel.size = PANEL
	_recipe_panel.draw.connect(_draw_recipe_panel)
	_overlay.add_child(_recipe_panel)
	move_child(_list_panel, -1)   # the item list and bookmarks stay clickable while a recipe is open (JEI)
	move_child(_bm_panel, -1)
	get_viewport().size_changed.connect(_layout)
	Settings.user_settings_changed.connect(_layout)


func _button(text: String, cb: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.focus_mode = Control.FOCUS_NONE
	b.custom_minimum_size = Vector2(28, 26)
	b.pressed.connect(cb)
	return b


## Called when the inventory screen opens.
func refresh() -> void:
	_search.release_focus()
	_layout()


func _layout() -> void:
	if model == null:
		return
	var vw := get_viewport_rect().size   # own size is still 0 while the screen is hidden
	var panel_right := vw.x / 2.0 + PANEL.x / 2.0
	var x0 := panel_right + 12.0 + right_inset
	var w := vw.x - x0 - 8.0
	_cols = int(w / S)
	if Settings.jei_cols > 0:
		_cols = mini(_cols, Settings.jei_cols)
	_rows = int((vw.y - 16.0 - 34.0 - 40.0) / S)
	_list_panel.visible = Settings.jei_shown and _cols >= 2 and _rows >= 2      # too narrow (phone): no list, R/U still work
	_list_rect = Rect2(x0, 8, _cols * S + 8, _rows * S + 34 + 40 + 8)
	_list_panel.position = _list_rect.position
	_list_panel.size = _list_rect.size
	_prev_page.position = Vector2(4, 4)
	_next_page.position = Vector2(_list_rect.size.x - 32, 4)
	_page_label.position = Vector2(36, 6)
	_page_label.size = Vector2(_list_rect.size.x - 72, 22)
	_search.position = Vector2(4, _list_rect.size.y - 38)
	_search.size = Vector2(_list_rect.size.x - 8, 32)
	model.page_size = maxi(1, _cols * _rows)
	model.page = clampi(model.page, 0, model.page_count() - 1)
	_recipe_panel.position = vw / 2.0 - PANEL / 2.0
	# bookmarks: mirror of the item list on the left
	var bw := vw.x / 2.0 - PANEL.x / 2.0 - 12.0 - 8.0
	_bm_cols = int((bw - 8.0) / S)
	_bm_rows = int((vw.y - BM_TOP - 8.0 - 34.0 - 8.0) / S)
	_bm_panel.position = Vector2(8, BM_TOP)
	_rebuild_list()
	refresh_bookmarks()
	if model.is_open():
		_build_recipe()


func _rebuild_list() -> void:
	for c in _cells:
		c.queue_free()
	_cells.clear()
	if not _list_panel.visible:
		return
	var ids := model.page_items()
	for i in ids.size():
		var c := BrowserCell.new(items, ids[i])
		c.position = Vector2(4 + (i % _cols) * S, 34 + (i / _cols) * S)
		c.marked = model.is_bookmarked(ids[i])
		c.clicked.connect(_on_list_cell_clicked)
		_list_panel.add_child(c)
		_cells.append(c)
	_page_label.text = "%d / %d" % [model.page + 1, model.page_count()]


## Rebuilds the bookmark panel (after A, or a layout change) and the list's marks.
func refresh_bookmarks() -> void:
	for c in _bm_cells:
		c.queue_free()
	_bm_cells.clear()
	for c in _cells:
		c.marked = model.is_bookmarked(c.item_id)
	_bm_panel.visible = _bm_cols >= 1 and _bm_rows >= 1
	if not _bm_panel.visible:
		return
	var ids := model.bookmarks.slice(0, _bm_cols * _bm_rows)
	var used_rows := maxi(1, ceili(float(ids.size()) / float(_bm_cols)))
	_bm_panel.size = Vector2(_bm_cols * S + 8, used_rows * S + 34 + 8 if not ids.is_empty() else 52.0)
	_bm_panel.queue_redraw()
	_bm_title.text = "Bookmarks" if not ids.is_empty() else "A over an item\nbookmarks it"
	for i in ids.size():
		var id := String(ids[i])
		var rid := id.trim_prefix(RecipeBrowser.RECIPE_MARK) if id.begins_with(RecipeBrowser.RECIPE_MARK) else ""
		var c := BrowserCell.new(items, id if rid == "" else String(model.recipe(rid).get("output", {}).get("id", "")))
		c.position = Vector2(4 + (i % _bm_cols) * S, 34 + (i / _bm_cols) * S)
		if rid == "":
			c.clicked.connect(_on_list_cell_clicked)
		else:
			c.marked = true   # a gold corner: this one opens its recipe (ADR 0093)
			c.clicked.connect(func(_id: String, _b: int) -> void:
				if model.show_recipe(rid):
					_build_recipe())
		_bm_panel.add_child(c)
		_bm_cells.append(c)


func _turn_page(d: int) -> void:
	model.turn_page(d)
	_rebuild_list()


func _on_list_input(e: InputEvent) -> void:
	if e is InputEventMouseButton and e.pressed:
		if e.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			_turn_page(1)
		elif e.button_index == MOUSE_BUTTON_WHEEL_UP:
			_turn_page(-1)


## The item list: in creative a left click takes a stack (shift: into the
## inventory), like JEI's cheat mode; right click still shows uses.
func _on_list_cell_clicked(item_id: String, button: int) -> void:
	if button == MOUSE_BUTTON_LEFT and cheat_give.is_valid() and cheat_give.call(item_id, Input.is_key_pressed(KEY_SHIFT)):
		return
	_on_cell_clicked(item_id, button)


func _on_cell_clicked(item_id: String, button: int) -> void:
	show_item(item_id, "uses" if button == MOUSE_BUTTON_RIGHT else "recipes")


## Opens recipes / uses of an item. Returns false when there are none.
func show_item(item_id: String, mode: String) -> bool:
	if item_id == "" or not model.show(item_id, mode):
		return false
	_build_recipe()
	return true


func back() -> void:
	model.back()
	if model.is_open():
		_build_recipe()
	else:
		_overlay.visible = false


func close_recipes() -> void:
	model.close()
	_overlay.visible = false


func is_showing_recipes() -> bool:
	return _overlay.visible


# ---------------- recipe view ----------------

func _build_recipe() -> void:
	for n in _recipe_nodes:
		n.queue_free()
	_recipe_nodes.clear()
	_spec_cells.clear()
	fill_button = null
	craft_button = null
	bookmark_button = null
	_overlay.visible = true
	var r := model.current_recipe()
	var cat := model.current_category()
	var l := model.layout(r)
	var v: Dictionary = model.view
	_label(("Recipes for " if v["mode"] == "recipes" else "Uses of ") + items.display_name(v["item"]), Vector2(16, 8), 16)
	# station tabs (ADR 0093): one per category, its station's icon; a click opens it, < > page past TABS
	var ncat: int = v["categories"].size()
	if ncat > TABS:
		_add(_button("<", func() -> void: model.turn_category(-1); _build_recipe()), Vector2(16, 34))
		_add(_button(">", func() -> void: model.turn_category(1); _build_recipe()), Vector2(PANEL.x - 44, 34))
	var first := clampi(int(v["cat"]) - TABS / 2, 0, maxi(0, ncat - TABS))
	_tab_x = -1.0
	for i in range(first, mini(first + TABS, ncat)):
		var c0: Dictionary = v["categories"][i]
		var st := model.catalysts(c0["id"])
		var tab := BrowserCell.new(items, st[0] if not st.is_empty() else String(model.recipe(c0["recipes"][0]).get("output", {}).get("id", "")))
		tab.clicked.connect(func(_id: String, _b: int) -> void: model.select_category(i); _build_recipe())
		var x := 48.0 + (i - first) * S
		if i == int(v["cat"]):
			_tab_x = x
		_add(tab, Vector2(x, 30))
	var title := _label(cat["title"], Vector2(16, 66), 13)
	title.size = Vector2(PANEL.x - 100, 18)
	# inputs
	var origin := Vector2(40, 84)
	match l["kind"]:
		"smelt", "mine":
			origin = Vector2(40 + S, 84)
		"pattern":
			origin = Vector2(24, 120)
		"row":
			origin = Vector2(16, 120)
	for inp in l["inputs"]:
		var cell: Vector2i = inp["cell"]
		var c := BrowserCell.new(items, "", inp["spec"])
		c.count = model.cell_amount(inp["spec"])
		c.position = origin + Vector2(cell.x * S if l["kind"] != "row" else cell.x * (S + 4), cell.y * S)
		c.clicked.connect(_on_cell_clicked)
		_add(c, c.position)
		_spec_cells.append(c)
	var out := BrowserCell.new(items, l["output"]["id"])
	out.count = model.output_amount(l)
	out.clicked.connect(_on_cell_clicked)
	_add(out, Vector2(244, 120))
	if trackers != null and r.get("type", "") != "mining":
		var rid := String(r["id"])
		var pin := _button("Unpin" if trackers.is_pinned("recipe", rid) else "Pin", func() -> void: pass)
		pin.tooltip_text = "Show this recipe's ingredients on the HUD"
		pin.pressed.connect(func() -> void:
			trackers.toggle("recipe", rid)
			pin.text = "Unpin" if trackers.is_pinned("recipe", rid) else "Pin")
		_add(pin, Vector2(PANEL.x - 70, 76))
	if grid_fill.is_valid() and CraftingGrid.is_grid_recipe(r):
		_add_fill_button(r, grid_fill)
	elif container_fill.is_valid() and r.get("type", "") == "smelt":
		_add_fill_button(r, container_fill)
	if r.get("type", "") != "mining":
		var mark := RecipeBrowser.RECIPE_MARK + String(r["id"])
		var bm := _button("Unmark" if model.is_bookmarked(mark) else "Mark", func() -> void: pass)
		bm.tooltip_text = "Bookmark this recipe at the left of the screen"
		bm.pressed.connect(func() -> void:
			model.toggle_bookmark(mark)
			bm.text = "Unmark" if model.is_bookmarked(mark) else "Mark"
			refresh_bookmarks())
		_add(bm, Vector2(PANEL.x - 70, 252))
		bookmark_button = bm
	# have / need per ingredient, in the inventory's own counts (ADR 0046)
	if l["kind"] in ["grid", "row", "smelt"] and r["type"] != "mining":
		var y := 162.0
		for n in model.needs_list(l):
			if n["have"] < 0:
				break
			var ok: bool = n["have"] >= n["need"]
			var lb := _label("%s %d/%d" % [n["name"], n["have"], n["need"]], Vector2(196, y), 12)
			lb.add_theme_color_override("font_color", Color(0.1, 0.4, 0.1) if ok else Color(0.65, 0.1, 0.08))
			lb.clip_text = true
			lb.size = Vector2(PANEL.x - 200 - (74 if craft_button else 0), 15)
			y += 15.0
	# info, station requirement, catalysts
	if l["info"] != "":
		_label(l["info"], Vector2(40, 200), 14)
	if l["needs"] != "":
		_label(("Tool: " if l["kind"] == "mine" else "Needs: ") + l["needs"], Vector2(40, 220), 14)
	var cats := model.catalysts(cat["id"])
	if not cats.is_empty():
		_label("Made in:", Vector2(16, 252), 14)
		for i in cats.size():
			var cc := BrowserCell.new(items, cats[i])
			cc.clicked.connect(_on_cell_clicked)
			_add(cc, Vector2(90 + i * (S + 4), 244))
	# recipe pager
	var n: int = cat["recipes"].size()
	if n > 1:
		_add(_button("<", func() -> void: model.turn_recipe(-1); _build_recipe()), Vector2(16, PANEL.y - 34))
		_add(_button(">", func() -> void: model.turn_recipe(1); _build_recipe()), Vector2(PANEL.x - 44, PANEL.y - 34))
	var pl := _label("%d / %d" % [v["index"] + 1, n], Vector2(48, PANEL.y - 32), 14)
	pl.size = Vector2(PANEL.x - 96, 22)
	pl.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	var hint := Label.new()
	hint.text = "LMB recipes  RMB uses  R / U / A (bookmark) over an item, also in the world  Bksp back  Esc close"
	hint.add_theme_font_size_override("font_size", 12)
	hint.autowrap_mode = TextServer.AUTOWRAP_WORD
	hint.custom_minimum_size = Vector2(PANEL.x, 0)
	hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	hint.position = Vector2(0, PANEL.y + 6)
	_add(hint, hint.position)
	_cycle_t = -1
	_update_cycle()
	_recipe_panel.queue_redraw()


## Fill (bug report 2026-10-25): right of the output, lays the shown recipe out in the open crafting
## grid from the inventory; Shift fills as many crafts as the inventory and the stacks allow. Greyed
## out, with the reason as its tooltip, when the grid cannot take it or ingredients are short.
func _add_fill_button(r: Dictionary, fill: Callable) -> void:
	var b := _button("Fill", func() -> void: pass)
	var check: Dictionary = fill.call(r, true, true)
	b.disabled = check["error"] != ""
	b.tooltip_text = check["error"] if b.disabled else \
		"Put the ingredients in (Shift: enough for %d)" % int(check["crafts"])
	b.pressed.connect(func() -> void:
		var res: Dictionary = fill.call(r, Input.is_key_pressed(KEY_SHIFT), false)
		if res["error"] == "":
			close_recipes()   # show the filled grid
		else:
			b.disabled = true
			b.tooltip_text = res["error"])
	_add(b, Vector2(PANEL.x - 70, 124))
	fill_button = b
	if fill == grid_fill and grid_craft.is_valid():   # ADR 0130: Fill and take the output in one press
		var c := _button("Craft", func() -> void: pass)
		c.disabled = b.disabled
		c.tooltip_text = b.tooltip_text if b.disabled else "Craft one into the inventory (Shift: %d)" % int(check["crafts"])
		c.pressed.connect(func() -> void:
			grid_craft.call(r, Input.is_key_pressed(KEY_SHIFT))
			_build_recipe())   # the have / need counts and the buttons again
		_add(c, Vector2(PANEL.x - 70, 158))
		craft_button = c


func _add(n: Control, pos: Vector2) -> void:
	n.position = pos
	_recipe_panel.add_child(n)
	_recipe_nodes.append(n)


func _label(text: String, pos: Vector2, fs: int) -> Label:
	var lb := Label.new()
	lb.text = text
	lb.add_theme_color_override("font_color", Color(0.25, 0.25, 0.25))
	lb.add_theme_font_size_override("font_size", fs)
	_add(lb, pos)
	return lb


## Clicking anywhere outside the search box gives the keys back to the game.
func _input(e: InputEvent) -> void:
	if e is InputEventMouseButton and e.pressed and _search.has_focus() and not _search.get_global_rect().has_point(e.position):
		_search.release_focus()


func _process(_d: float) -> void:
	if _overlay.visible:
		_update_cycle()


## Tag ingredients show their members in turn, one per second (JEI).
func _update_cycle() -> void:
	var t := int(Time.get_ticks_msec() / 1000)
	if t == _cycle_t:
		return
	_cycle_t = t
	for c in _spec_cells:
		c.set_item(model.display_item(c.spec, t))


func _draw_recipe_panel() -> void:
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.776, 0.776, 0.776)
	sb.border_color = Color(0.2, 0.2, 0.2)
	sb.set_border_width_all(3)
	sb.set_corner_radius_all(6)
	_recipe_panel.draw_style_box(UiStyle.skin("inventory", sb), Rect2(Vector2.ZERO, PANEL))
	if not model.is_open():
		return
	if _tab_x >= 0.0:   # the open station tab: a frame round it
		_recipe_panel.draw_rect(Rect2(Vector2(_tab_x - 2, 28), Vector2(S + 4, S + 4)), Color(0.15, 0.35, 0.8), false, 3.0)
	var l := model.layout(model.current_recipe())
	var c := Color(0.5, 0.5, 0.5)
	# empty grid slots behind the ingredients
	if l["kind"] == "grid":
		for y in 3:
			for x in 3:
				BrowserCell.draw_slot(_recipe_panel, Rect2(Vector2(40 + x * S, 84 + y * S), Vector2(S, S)))
	elif l["kind"] == "mine":
		BrowserCell.draw_slot(_recipe_panel, Rect2(Vector2(40 + S, 84), Vector2(S, S)))
		# a pick head under the block: "break it"
		var ph := Vector2(40 + S + 4, 84 + S + 8)
		_recipe_panel.draw_line(ph + Vector2(0, 8), ph + Vector2(14, 0), Color(0.45, 0.45, 0.5), 4.0)
		_recipe_panel.draw_line(ph + Vector2(14, 0), ph + Vector2(28, 8), Color(0.45, 0.45, 0.5), 4.0)
		_recipe_panel.draw_line(ph + Vector2(14, 0), ph + Vector2(14, 22), Color(0.55, 0.38, 0.2), 4.0)
	elif l["kind"] == "pattern":
		# the shape from above, 8x8 cells of 10 px; clay shades darker with height
		var hs: PackedInt32Array = l["heights"]
		var top := 1
		for v in hs:
			top = maxi(top, v)
		var o0 := Vector2(84, 98)
		_recipe_panel.draw_rect(Rect2(o0 - Vector2(2, 2), Vector2(84, 84)), Color(0.35, 0.35, 0.35))
		for i in 64:
			var r := Rect2(o0 + Vector2((i % 8) * 10, (i / 8) * 10), Vector2(9, 9))
			if hs[i] == 0:
				_recipe_panel.draw_rect(r, Color(0.55, 0.55, 0.55))
			else:
				var k := float(hs[i]) / float(top)
				_recipe_panel.draw_rect(r, Color(0.55, 0.6, 0.7).lerp(Color(0.25, 0.28, 0.36), k))
	elif l["kind"] == "smelt":
		var f := Rect2(Vector2(40 + S + 4, 84 + S + 4), Vector2(28, 28))
		_recipe_panel.draw_rect(f, Color(1.0, 0.55, 0.1))
		_recipe_panel.draw_rect(Rect2(f.position + Vector2(7, 11), Vector2(14, 17)), Color(1.0, 0.85, 0.3))
	# arrow to the output
	var o := Vector2(184, 121)
	_recipe_panel.draw_rect(Rect2(o + Vector2(0, 10), Vector2(30, 14)), c)
	_recipe_panel.draw_colored_polygon(PackedVector2Array([o + Vector2(30, 0), o + Vector2(48, 17), o + Vector2(30, 34)]), c)


## One clickable item: a slot square with an icon, hover highlight, tooltip id.
class BrowserCell:
	extends Control
	signal clicked(item_id: String, button: int)
	var item_id := ""
	var spec := ""           # set for recipe ingredients ("#tag" cycles)
	var marked := false:     # bookmarked: a gold corner in the item list
		set(v):
			marked = v
			if _mark:
				_mark.visible = v
	var _mark: Control
	var count := 1:
		set(v):
			count = v
			if _icon:
				_icon.stack = {"id": item_id, "count": count} if item_id != "" else {}
	var _icon: ItemIcon
	var _hover := false

	func _init(p_items: ItemRegistry, p_id: String, p_spec: String = "") -> void:
		size = Vector2(S, S)
		mouse_filter = Control.MOUSE_FILTER_STOP
		spec = p_spec
		_icon = ItemIcon.new()
		_icon.items = p_items
		_icon.position = Vector2(2, 2)
		_icon.size = Vector2(S - 4, S - 4)
		_icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(_icon)
		_mark = Control.new()   # above the icon
		_mark.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_mark.size = size
		_mark.visible = marked
		_mark.draw.connect(func() -> void:
			_mark.draw_colored_polygon(PackedVector2Array([Vector2(S - 13, 2), Vector2(S - 2, 2), Vector2(S - 2, 13)]), Color(1.0, 0.8, 0.2)))
		add_child(_mark)
		set_item(p_id)
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())

	func set_item(id: String) -> void:
		item_id = id
		_icon.stack = {"id": id, "count": count} if id != "" else {}

	## What the tooltip / R / U act on.
	func hover_item_id() -> String:
		return item_id

	func hover_tag() -> String:
		return spec if spec.begins_with("#") else ""

	static func draw_slot(ci: CanvasItem, r: Rect2) -> void:
		var skin := UiStyle.skin("slot", StyleBoxEmpty.new())
		if skin is StyleBoxTexture:   # the inventory's slot skin (ADR 0129)
			ci.draw_style_box(skin, r)
			return
		ci.draw_rect(r, Color(0.545, 0.545, 0.545))
		ci.draw_rect(Rect2(r.position, Vector2(r.size.x, 2)), Color(0.22, 0.22, 0.22))
		ci.draw_rect(Rect2(r.position, Vector2(2, r.size.y)), Color(0.22, 0.22, 0.22))
		ci.draw_rect(Rect2(r.position + Vector2(0, r.size.y - 2), Vector2(r.size.x, 2)), Color.WHITE)
		ci.draw_rect(Rect2(r.position + Vector2(r.size.x - 2, 0), Vector2(2, r.size.y)), Color.WHITE)

	func _draw() -> void:
		draw_slot(self, Rect2(Vector2.ZERO, size))
		if _hover:
			draw_rect(Rect2(2, 2, size.x - 4, size.y - 4), Color(1, 1, 1, 0.35))

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and (event.button_index == MOUSE_BUTTON_LEFT or event.button_index == MOUSE_BUTTON_RIGHT):
			clicked.emit(item_id, event.button_index)
			accept_event()
