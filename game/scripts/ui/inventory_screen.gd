class_name InventoryScreen
extends CanvasLayer

## Minecraft-layout inventory screen (ADR 0005, 0006): the vanilla 176x166
## panel at 2x with a 2x2 personal crafting grid + output, a 3x3 crafting
## table grid, a chest's 27 slots or a furnace (input, fuel, output, flame
## and arrow gauges) above 27 main slots and the hotbar row. The JEI-style
## recipe browser (ADR 0008) sits beside it; item names show on hover. The
## progression book (ADR 0028) opens from the button in the top left corner. Clicks go to PlayerInventory's
## cursor operations; the cursor stack follows the mouse. Closing returns
## grid contents; what does not fit is emitted for dropping.

signal closed(dropped: Array)

const S := 36          # slot pitch (vanilla 18 px at 2x)
const PANEL := Vector2(352, 332)

var player_inv: PlayerInventory
var is_open := false
var _root: Control
var _panel: Control
var _slots: Array[SlotView] = []
var _craft_slots: Array[SlotView] = []
var _output: SlotView
var _cursor_icon: ItemIcon
var _title: Label
var _inv_label: Label
var _container: RefCounted = null          # ChestEntity / FurnaceEntity in container mode
var _container_slots: Array[SlotView] = []
var browser: RecipeBrowser
var browser_view: RecipeBrowserView
var _tooltip: Label
var _bag_slot: SlotView                  # the worn bag (ADR 0030), personal inventory only
var _bag_slots: Array[SlotView] = []     # its 9 slots, a row under the hotbar while a bag is worn
var quest_view: QuestBookView
var _book_button: QuestBookView.BookButton
# Mouse Tweaks: a button held after a press on a slot drags over more slots
var _drag_button := 0
var _drag_shift := false
var _drag_seen := {}


func setup(p_inv: PlayerInventory) -> void:
	player_inv = p_inv
	player_inv.changed.connect(_refresh)
	browser = RecipeBrowser.new(player_inv.items, player_inv.recipes)
	browser.set_mining(MiningTable.load_default())
	# bookmarks are the player's (user://); autopilot runs stay reproducible and take bookmarks=a,b instead
	var args := OS.get_cmdline_user_args()
	if not args.has("autopilot=1"):
		browser.load_bookmarks()
	for a in args:
		if a.begins_with("bookmarks="):
			browser.set_bookmarks(Array(a.substr(10).split(",", false)))
	browser_view = RecipeBrowserView.new()
	browser_view.setup(browser)
	_root.add_child(browser_view)
	_root.move_child(_cursor_icon, -1)   # the held stack draws above the browser
	_root.move_child(_tooltip, -1)


## The progression book: its button top left, the book over everything else.
func setup_quests(book: QuestBook) -> void:
	_book_button = QuestBookView.BookButton.new()
	_book_button.book = book
	_book_button.position = Vector2(8, 8)
	_book_button.pressed.connect(open_book)
	_root.add_child(_book_button)
	book.quest_completed.connect(func(_id: String) -> void: _book_button.queue_redraw())
	quest_view = QuestBookView.new()
	quest_view.visible = false
	quest_view.setup(book, player_inv.items)
	quest_view.show_recipes.connect(func(id: String) -> void:
		close_book()
		browser_view.show_item(id, "recipes"))
	_root.add_child(quest_view)
	_root.move_child(_cursor_icon, -1)
	_root.move_child(_tooltip, -1)


func open_book() -> void:
	if quest_view and is_open:
		_tooltip.visible = false
		quest_view.open()


func close_book() -> void:
	if quest_view:
		quest_view.close()


func is_book_open() -> bool:
	return quest_view != null and quest_view.visible


func _ready() -> void:
	layer = 10
	visible = false
	_root = Control.new()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_root.draw.connect(func() -> void: _root.draw_rect(Rect2(Vector2.ZERO, _root.size), Color(0, 0, 0, 0.45)))
	add_child(_root)
	_panel = Control.new()
	_panel.set_anchors_preset(Control.PRESET_CENTER)
	_panel.size = PANEL
	_panel.position = -PANEL / 2.0
	_panel.draw.connect(_draw_panel)
	_root.add_child(_panel)
	_title = Label.new()
	_title.position = Vector2(16, 10)
	_title.add_theme_color_override("font_color", Color(0.25, 0.25, 0.25))
	_title.add_theme_font_size_override("font_size", 16)
	_panel.add_child(_title)
	_inv_label = Label.new()
	_inv_label.text = "Inventory"
	_inv_label.position = Vector2(16, 142)
	_inv_label.add_theme_color_override("font_color", Color(0.25, 0.25, 0.25))
	_inv_label.add_theme_font_size_override("font_size", 16)
	_panel.add_child(_inv_label)
	_cursor_icon = ItemIcon.new()
	_cursor_icon.size = Vector2(32, 32)
	_cursor_icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_cursor_icon.z_index = 10
	_root.add_child(_cursor_icon)
	_tooltip = Label.new()
	_tooltip.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tooltip.z_index = 20
	_tooltip.add_theme_font_size_override("font_size", 14)
	var tsb := StyleBoxFlat.new()
	tsb.bg_color = Color(0.08, 0.02, 0.12, 0.94)
	tsb.border_color = Color(0.3, 0.1, 0.6)
	tsb.set_border_width_all(2)
	tsb.set_content_margin_all(5)
	_tooltip.add_theme_stylebox_override("normal", tsb)
	_tooltip.visible = false
	_root.add_child(_tooltip)
	_build_player_slots()


func _build_player_slots() -> void:
	for sv in _slots:
		sv.queue_free()
	_slots.clear()
	# main 3x9 at vanilla (8,84), hotbar at (8,142)
	for i in PlayerInventory.SIZE:
		var sv := SlotView.new()
		if i < PlayerInventory.HOTBAR:
			sv.position = Vector2(16 + i * S, 284)
		else:
			var j := i - PlayerInventory.HOTBAR
			sv.position = Vector2(16 + (j % 9) * S, 168 + (j / 9) * S)
		sv.index = i
		sv.group = "inv"
		sv.clicked.connect(_on_slot_clicked.bind(i))
		_panel.add_child(sv)
		_slots.append(sv)
	for sv in _bag_slots:
		sv.queue_free()
	_bag_slots.clear()
	for i in PlayerInventory.BAG:
		var sv := SlotView.new()
		sv.position = Vector2(16 + i * S, 332)
		sv.index = i
		sv.group = "bag"
		sv.clicked.connect(func(button: int, shift: bool) -> void: player_inv.click_bag_slot(i, button, shift))
		sv.visible = false
		_panel.add_child(sv)
		_bag_slots.append(sv)
	if _bag_slot == null:
		_bag_slot = SlotView.new()
		_bag_slot.position = Vector2(126, 120)
		_bag_slot.hint = "Bag"
		_bag_slot.clicked.connect(func(_b: int, _s: bool) -> void: player_inv.click_bag_item())
		_panel.add_child(_bag_slot)


func _clear_mode_slots() -> void:
	for sv in _craft_slots + _container_slots:
		sv.queue_free()
	_craft_slots.clear()
	_container_slots.clear()
	if _output:
		_output.queue_free()
		_output = null
	if _container:
		_container.changed.disconnect(_refresh)
		_container = null


func _build_craft_slots(width: int) -> void:
	_clear_mode_slots()
	var origin := Vector2(196, 36) if width == 2 else Vector2(60, 34)
	for i in width * width:
		var sv := SlotView.new()
		sv.position = origin + Vector2((i % width) * S, (i / width) * S)
		sv.index = i
		sv.group = "craft"
		sv.clicked.connect(_on_craft_clicked.bind(i))
		_panel.add_child(sv)
		_craft_slots.append(sv)
	_output = SlotView.new()
	_output.position = Vector2(308, 56) if width == 2 else Vector2(248, 70)
	_output.is_output = true
	_output.clicked.connect(_on_output_clicked)
	_panel.add_child(_output)


## Opens with the personal 2x2 grid, or a station grid (3 = crafting table).
func open(width: int = 2, station: String = "", title: String = "Inventory") -> void:
	if player_inv.craft.width != width:
		var dropped: Array = player_inv.set_crafting_grid(width, station)
		if not dropped.is_empty():
			closed.emit(dropped)   # could not fit while swapping grids: drop now
	_build_craft_slots(width)
	_title.text = title
	_title.position = Vector2(196, 8) if width == 2 else Vector2(16, 8)
	_inv_label.visible = width != 2
	_show()


## Opens a chest or furnace (vanilla generic_54 / furnace layouts at 2x).
func open_container(c: RefCounted) -> void:
	_clear_mode_slots()
	_container = c
	_container.changed.connect(_refresh)
	player_inv.open_container(c)
	var positions: Array = []
	if c.kind == "furnace":
		# vanilla input (56,17), fuel (56,53), result (116,35)
		positions = [Vector2(112, 34), Vector2(112, 106), Vector2(232, 70)]
	elif c.kind == "firepit":
		# the furnace layout plus the vessel (crucible) slot left of the input (ADR 0023)
		positions = [Vector2(112, 34), Vector2(112, 106), Vector2(232, 70), Vector2(52, 34)]
	else:
		for i in c.inv.size():
			positions.append(Vector2(16 + (i % 9) * S, 36 + (i / 9) * S))
	for i in positions.size():
		var sv := SlotView.new()
		sv.position = positions[i]
		sv.index = i
		sv.is_output = c.is_output(i)
		sv.group = "container"
		sv.clicked.connect(_on_container_clicked.bind(i))
		_panel.add_child(sv)
		_container_slots.append(sv)
	_title.text = c.title
	_title.position = Vector2(16, 8) if c.kind == "chest" or c.kind == "storage_pot" else Vector2(176 - c.title.length() * 4, 8)
	_inv_label.visible = true
	_show()


func _show() -> void:
	if browser_view:
		browser_view.refresh()
	is_open = true
	visible = true
	_refresh()


func close() -> void:
	if not is_open:
		return
	is_open = false
	visible = false
	_tooltip.visible = false
	close_book()
	if browser_view:
		browser_view.close_recipes()
	_clear_mode_slots()
	var dropped: Array = player_inv.close_screen()
	if player_inv.craft.width != 2:
		dropped.append_array(player_inv.set_crafting_grid(2, ""))
	closed.emit(dropped)


func _refresh() -> void:
	if not is_open or player_inv == null:
		return
	for i in _slots.size():
		_slots[i].set_stack(player_inv.items, player_inv.inv.get_slot(i))
	for i in _craft_slots.size():
		_craft_slots[i].set_stack(player_inv.items, player_inv.craft.grid.get_slot(i))
	if _output:
		_output.set_stack(player_inv.items, player_inv.craft.preview_output())
	if _container:
		for i in _container_slots.size():
			_container_slots[i].set_stack(player_inv.items, _container.inv.get_slot(i))
	var worn := player_inv.has_bag()
	_bag_slot.visible = _container == null and player_inv.craft.width == 2
	_bag_slot.set_stack(player_inv.items, player_inv.bag_item)
	for i in _bag_slots.size():
		_bag_slots[i].visible = worn
		_bag_slots[i].set_stack(player_inv.items, player_inv.bag.get_slot(i))
	var h := PANEL.y + (48.0 if worn else 0.0)
	if _panel.size.y != h:
		_panel.size = Vector2(PANEL.x, h)
		_panel.position = -_panel.size / 2.0
		_panel.queue_redraw()
	_cursor_icon.items = player_inv.items
	_cursor_icon.stack = player_inv.cursor
	_cursor_icon.visible = not player_inv.cursor.is_empty()


func _process(_delta: float) -> void:
	if is_open:
		_cursor_icon.position = _root.get_local_mouse_position() - Vector2(16, 16)
		if _container and _container.kind in ["furnace", "firepit"]:
			_panel.queue_redraw()   # flame and arrow gauges move every tick
		_update_tooltip()


## The item under the mouse (inventory slot or browser cell), or "".
func hovered_item() -> String:
	var c := get_viewport().gui_get_hovered_control()
	while c != null and c != _root:
		if c.has_method("hover_item_id"):
			return c.hover_item_id()
		c = c.get_parent() as Control
	return ""


func _update_tooltip() -> void:
	var id := hovered_item() if player_inv.cursor.is_empty() else ""
	_tooltip.visible = id != ""
	if id == "":
		return
	var text := player_inv.items.display_name(id)
	var c := get_viewport().gui_get_hovered_control()
	if c and c.has_method("hover_tag") and c.hover_tag() != "":
		text += "\nany " + c.hover_tag()
	var days := float(player_inv.items.get_def(id).get("spoil_days", 0.0))
	if days > 0.0:
		var sp := 0.0
		var w := c
		while w != null and w != _root:   # the hovered slot's stack, if it has one
			var st: Variant = w.get("stack")
			if st is Dictionary and st.has("spoil"):
				sp = float(st["spoil"])
				break
			w = w.get_parent() as Control
		text += "\n%s (%.1f days left at 16 C)" % [Survival.freshness_name(sp), (1.0 - sp) * days]
	_tooltip.text = text
	_tooltip.reset_size()
	var p := _root.get_local_mouse_position() + Vector2(14, -10)
	p.x = minf(p.x, _root.size.x - _tooltip.size.x - 4)
	_tooltip.position = p


## Mouse Tweaks: after a press on a slot, dragging with the right button puts
## one item into each slot passed, with the left button collects that item
## from them into the cursor, and with shift held shift-clicks each of them.
## The wheel over a slot moves single items (down: away, up: back in).
func _input(event: InputEvent) -> void:
	if not is_open or is_book_open():
		return
	if event is InputEventMouseButton:
		if event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN] and event.pressed:
			var sv := _slot_at(event.position)
			if sv != null:
				player_inv.wheel_move(sv.group, sv.index, event.button_index == MOUSE_BUTTON_WHEEL_DOWN)
				get_viewport().set_input_as_handled()
		elif event.button_index in [MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT]:
			if event.pressed:
				var sv := _slot_at(event.position)
				_drag_button = event.button_index if sv != null else 0
				_drag_shift = event.shift_pressed
				_drag_seen = {sv: true}
			elif event.button_index == _drag_button:
				_drag_button = 0
	elif event is InputEventMouseMotion and _drag_button != 0:
		var sv := _slot_at(event.position)
		if sv == null or _drag_seen.has(sv):
			return
		_drag_seen[sv] = true
		if _drag_shift and _drag_button == MOUSE_BUTTON_LEFT:
			player_inv.drag_shift(sv.group, sv.index)
		elif _drag_button == MOUSE_BUTTON_RIGHT:
			player_inv.drag_place_one(sv.group, sv.index)
		else:
			player_inv.drag_collect(sv.group, sv.index)


## The visible slot (not an output) under a screen position, or null.
func _slot_at(screen_pos: Vector2) -> SlotView:
	for sv in _slots + _bag_slots + _craft_slots + _container_slots:
		if sv.visible and not sv.is_output and sv.group != "" and sv.get_global_rect().has_point(screen_pos):
			return sv
	return null


## JEI keys while the screen is open: R recipes, U uses, A bookmark (over
## any item), Backspace back, Esc closes the recipe view before the screen. Typing in
## the search box never reaches here (the LineEdit has focus).
func _unhandled_input(event: InputEvent) -> void:
	if not is_open or not (event is InputEventKey) or not event.pressed or event.echo:
		return
	if event.keycode == KEY_ESCAPE and is_book_open():
		close_book()
		get_viewport().set_input_as_handled()
		return
	match event.keycode:
		KEY_R, KEY_U:
			var id := hovered_item()
			if id != "" and is_book_open():
				close_book()   # the recipe view draws under the book
			if browser_view.show_item(id, "recipes" if event.keycode == KEY_R else "uses"):
				get_viewport().set_input_as_handled()
		KEY_A:
			var id := hovered_item()
			if id != "":
				browser.toggle_bookmark(id)
				if not OS.get_cmdline_user_args().has("autopilot=1"):
					browser.save_bookmarks()
				browser_view.refresh_bookmarks()
				get_viewport().set_input_as_handled()
		KEY_BACKSPACE:
			if browser_view.is_showing_recipes():
				browser_view.back()
				get_viewport().set_input_as_handled()
		KEY_ESCAPE:
			if browser_view.is_showing_recipes():
				browser_view.close_recipes()
				get_viewport().set_input_as_handled()


func _on_slot_clicked(button: int, shift: bool, i: int) -> void:
	player_inv.click_slot(i, button, shift)


func _on_craft_clicked(button: int, shift: bool, i: int) -> void:
	player_inv.click_craft_slot(i, button, shift)


func _on_container_clicked(button: int, shift: bool, i: int) -> void:
	player_inv.click_container_slot(i, button, shift)


func _on_output_clicked(_button: int, shift: bool) -> void:
	player_inv.click_craft_output(shift)


func _draw_panel() -> void:
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.776, 0.776, 0.776)
	sb.border_color = Color(0.2, 0.2, 0.2)
	sb.set_border_width_all(3)
	sb.set_corner_radius_all(6)
	_panel.draw_style_box(sb, Rect2(Vector2.ZERO, _panel.size))
	if _container:
		if _container.kind in ["furnace", "firepit"]:
			_draw_furnace_gauges()
		return
	# player "portrait" area like vanilla's model box
	if player_inv and player_inv.craft.width == 2:
		_panel.draw_rect(Rect2(16, 16, 104, 140), Color(0.1, 0.1, 0.1))
		# arrow from grid to output
		_panel.draw_line(Vector2(272, 74), Vector2(300, 74), Color(0.3, 0.3, 0.3), 4.0)
	else:
		_panel.draw_line(Vector2(180, 88), Vector2(236, 88), Color(0.3, 0.3, 0.3), 4.0)


## Vanilla furnace gauges: flame (56,36) 14x14 burns down, arrow (79,34)
## 24x17 fills left to right with the cook progress.
func _draw_furnace_gauges() -> void:
	var f: Rect2 = Rect2(114, 74, 28, 28)
	_panel.draw_rect(f, Color(0.45, 0.45, 0.45))
	var lit: float = _container.flame()
	if lit > 0.0:
		var h := f.size.y * lit
		_panel.draw_rect(Rect2(f.position.x, f.end.y - h, f.size.x, h), Color(1.0, 0.55, 0.1))
		_panel.draw_rect(Rect2(f.position.x + 7, f.end.y - h * 0.6, f.size.x - 14, h * 0.6), Color(1.0, 0.85, 0.3))
	_draw_arrow(Vector2(160, 70), 48.0, Color(0.5, 0.5, 0.5))
	var prog: float = _container.progress()
	if prog > 0.0:
		_draw_arrow(Vector2(160, 70), 48.0 * prog, Color(1, 1, 1))


## Arrow 48x34 at `o` (shaft 30 wide, head 18), drawn up to `w` px wide.
func _draw_arrow(o: Vector2, w: float, c: Color) -> void:
	_panel.draw_rect(Rect2(o + Vector2(0, 10), Vector2(minf(w, 30.0), 14)), c)
	if w > 30.5:
		var x := minf(w, 48.0)
		var dy := 17.0 * (x - 30.0) / 18.0
		_panel.draw_colored_polygon(PackedVector2Array([o + Vector2(30, 0), o + Vector2(x, dy), o + Vector2(x, 34.0 - dy), o + Vector2(30, 34)]), c)


## One slot: dark inset square, item icon, hover highlight, click -> signal.
class SlotView:
	extends Control
	signal clicked(button: int, shift: bool)
	var index := 0
	var group := ""           # PlayerInventory slot group ("inv", "bag", "craft", "container"); "" = no drags
	var is_output := false
	var hint := ""            # drawn faintly while the slot is empty (the bag slot)
	var _icon: ItemIcon
	var _hover := false

	func _init() -> void:
		size = Vector2(S, S)
		mouse_filter = Control.MOUSE_FILTER_STOP
		_icon = ItemIcon.new()
		_icon.position = Vector2(2, 2)
		_icon.size = Vector2(S - 4, S - 4)
		_icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(_icon)
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())

	func set_stack(items: ItemRegistry, stack: Dictionary) -> void:
		_icon.items = items
		_icon.stack = stack
		if hint != "":
			queue_redraw()

	func hover_item_id() -> String:
		return _icon.stack.get("id", "")

	func _draw() -> void:
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.545, 0.545, 0.545)
		sb.border_color = Color(0.22, 0.22, 0.22)
		sb.border_width_top = 2
		sb.border_width_left = 2
		sb.border_width_bottom = 0
		sb.border_width_right = 0
		var r := Rect2(Vector2.ZERO, size)
		draw_style_box(sb, r)
		draw_rect(Rect2(0, size.y - 2, size.x, 2), Color.WHITE)
		draw_rect(Rect2(size.x - 2, 0, 2, size.y), Color.WHITE)
		if _hover:
			draw_rect(Rect2(2, 2, size.x - 4, size.y - 4), Color(1, 1, 1, 0.35))
		if hint != "" and _icon.stack.is_empty():
			draw_string(ThemeDB.fallback_font, Vector2(4, size.y / 2.0 + 5), hint, HORIZONTAL_ALIGNMENT_CENTER, size.x - 8, 12, Color(0.3, 0.3, 0.3))

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and (event.button_index == MOUSE_BUTTON_LEFT or event.button_index == MOUSE_BUTTON_RIGHT):
			clicked.emit(event.button_index, event.shift_pressed)
			accept_event()
