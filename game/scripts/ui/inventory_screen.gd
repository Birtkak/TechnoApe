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
## ADR 0055: the offhand slot beside the bag slot, a trash slot left of the hotbar, a click outside
## the panel drops the cursor stack (left: all, right: one) and Q over a slot drops from it (Ctrl: all).

signal closed(dropped: Array)
signal drop_requested(stack: Dictionary)   # thrown out into the world in front of the player
## Plan item D11: Alt + click marks a favourite slot (sort, quick-stack, shift-click and refill leave it),
## Sort sorts the main grid or the open chest, Stack moves what nearby chests already hold into them,
## and a chest's name can be typed over its title.
var nearby_chests: Callable   # () -> Array of chest entities within reach (Main)
var _sort_button: Button
var _stack_button: Button
var _label_edit: LineEdit
var _side_panels: Array = []   # [Control, side] docked beside the panel (add_side_panel)

const S := 36          # slot pitch (vanilla 18 px at 2x)
const PANEL := Vector2(352, 332)

var player_inv: PlayerInventory
var is_open := false
var _root: Control
var _panel: Control
var _slots: Array[SlotView] = []
var _craft_slots: Array[SlotView] = []
var _output: SlotView
var _craft_hint: Label                    # why the grid makes nothing (ADR 0046)
var _clear_button: Button                 # the grid's contents back to the inventory (bug report 2026-10-25)
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
var _offhand_slot: SlotView              # ADR 0055, personal inventory only
var _trash_slot: SlotView                # left of the hotbar, on every screen
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
	browser.have = func(id: String) -> int:
		return player_inv.inv.count_of(id) + (player_inv.bag.count_of(id) if player_inv.has_bag() else 0)
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
		_root.move_child(quest_view, -1)   # over the side panels docked after it (bug report 2026-10-25)
		_root.move_child(_cursor_icon, -1)
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
	_root.gui_input.connect(_on_root_input)
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
		sv.alt_clicked.connect(func() -> void: player_inv.toggle_favourite(i))
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
	if _offhand_slot == null:
		_offhand_slot = SlotView.new()
		_offhand_slot.position = Vector2(162, 120)
		_offhand_slot.hint = "Off"
		_offhand_slot.group = "offhand"
		_offhand_slot.clicked.connect(func(button: int, shift: bool) -> void: player_inv.click_offhand(button, shift))
		_panel.add_child(_offhand_slot)
	if _sort_button == null:
		_sort_button = _small_button("Sort", func() -> void:
			if _container != null and _container.kind in ["chest", "storage_pot"]:
				player_inv.sort_container()
			else:
				player_inv.sort_inventory())
		_stack_button = _small_button("Stack", func() -> void:
			if nearby_chests.is_valid():
				player_inv.quick_stack(nearby_chests.call()))
		_stack_button.tooltip_text = "Move what nearby chests already hold into them (favourites stay)"
		_label_edit = LineEdit.new()
		_label_edit.placeholder_text = "Name this chest"
		_label_edit.add_theme_font_size_override("font_size", 13)
		_label_edit.custom_minimum_size = Vector2(150, 0)
		_label_edit.visible = false
		_label_edit.text_changed.connect(func(t: String) -> void:
			if _container != null and "label" in _container:
				_container.label = t.strip_edges())
		_panel.add_child(_label_edit)
	if _trash_slot == null:
		_trash_slot = SlotView.new()
		_trash_slot.position = Vector2(-S - 12, 284)
		_trash_slot.hint = "Trash"
		_trash_slot.clicked.connect(func(_b: int, _s: bool) -> void: player_inv.click_trash())
		_panel.add_child(_trash_slot)


## Docks another part's control beside the panel while the screen is open: "right" (right of the panel,
## top-aligned; the diet, the recipe browser list moves over), "below" (under the panel, left-aligned) or "left" (left of the panel, top-aligned; clothing slots, part F).
func add_side_panel(ctrl: Control, side: String = "below") -> void:
	_root.add_child(ctrl)
	_side_panels.append([ctrl, side])
	_place_side_panels()
	_root.move_child(_cursor_icon, -1)
	_root.move_child(_tooltip, -1)


func _place_side_panels() -> void:
	# from the viewport, not the anchored panel: it is not laid out yet when a side panel is added to an open screen
	var r := Rect2(get_viewport().get_visible_rect().size / 2.0 - _panel.size / 2.0, _panel.size)
	var inset := 0.0
	for sp in _side_panels:
		var c: Control = sp[0]
		if sp[1] == "right":
			c.global_position = Vector2(r.end.x + 12.0, r.position.y)
			inset = maxf(inset, c.size.x + 8.0)
		elif sp[1] == "left":
			c.global_position = Vector2(r.position.x - S - 24.0 - c.size.x, r.position.y)   # clear of the trash slot
		else:
			c.global_position = Vector2(r.position.x, r.end.y + 8.0)
	if browser_view and not is_equal_approx(browser_view.right_inset, inset):
		browser_view.right_inset = inset
		browser_view.refresh()


func _small_button(text: String, cb: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.focus_mode = Control.FOCUS_NONE
	b.add_theme_font_size_override("font_size", 12)
	b.pressed.connect(cb)
	_panel.add_child(b)
	return b


func _clear_mode_slots() -> void:
	for sv in _craft_slots + _container_slots:
		sv.queue_free()
	_craft_slots.clear()
	_container_slots.clear()
	if _output:
		_output.queue_free()
		_output = null
	if _craft_hint:
		_craft_hint.queue_free()
		_craft_hint = null
	if _clear_button:
		_clear_button.queue_free()
		_clear_button = null
	if browser_view:
		browser_view.grid_fill = Callable()
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
	_craft_hint = Label.new()
	_craft_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_craft_hint.add_theme_font_size_override("font_size", 12)
	_craft_hint.add_theme_color_override("font_color", Color(0.6, 0.1, 0.08))
	_craft_hint.autowrap_mode = TextServer.AUTOWRAP_WORD
	_craft_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_craft_hint.size = Vector2(150, 40)
	_craft_hint.position = _output.position + Vector2(S / 2.0 - 75.0, S + 2)
	_panel.add_child(_craft_hint)
	_clear_button = _small_button("Clear", func() -> void: player_inv.clear_craft_grid())
	_clear_button.tooltip_text = "Put everything in the grid back in the inventory"
	_clear_button.position = Vector2(140, 54) if width == 2 else Vector2(12, 34 + S)
	if browser_view:
		browser_view.grid_fill = _grid_fill   # the recipe view's Fill button


## The recipe view's Fill button: recipe r laid out in the open grid (all: Shift, as many as possible).
func _grid_fill(r: Dictionary, all: bool, dry_run: bool) -> Dictionary:
	return player_inv.fill_craft_grid(r, all, dry_run)


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
	_title.text = c.label if "label" in c and c.label != "" else c.title
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
		_slots[i].favourite = player_inv.favourites.has(i)
	var chest: bool = _container != null and _container.kind in ["chest", "storage_pot"]
	_sort_button.position = Vector2(PANEL.x - 56, 140) if not chest else Vector2(PANEL.x - 56, 4)
	_stack_button.position = Vector2(PANEL.x - 112, 140)
	_stack_button.visible = nearby_chests.is_valid()
	_label_edit.visible = chest
	if chest:
		_label_edit.position = Vector2(PANEL.x - 220, 4)
		if not _label_edit.has_focus():
			_label_edit.text = _container.label
	var short := player_inv.craft.short_slots()
	for i in _craft_slots.size():
		var st := player_inv.craft.grid.get_slot(i)
		_craft_slots[i].need = player_inv.items.units(st["id"]) if i in short else 0
		_craft_slots[i].set_stack(player_inv.items, st)
	if _output:
		_output.set_stack(player_inv.items, player_inv.craft.preview_output())
	if _craft_hint:
		_craft_hint.text = player_inv.craft.hint()
	if _container:
		for i in _container_slots.size():
			_container_slots[i].set_stack(player_inv.items, _container.inv.get_slot(i))
	var worn := player_inv.has_bag()
	_bag_slot.visible = _container == null and player_inv.craft.width == 2
	_bag_slot.set_stack(player_inv.items, player_inv.bag_item)
	_offhand_slot.visible = _bag_slot.visible
	_offhand_slot.set_stack(player_inv.items, player_inv.offhand())
	_trash_slot.set_stack(player_inv.items, player_inv.trash)
	_trash_slot.position.y = _slots[0].position.y
	for i in _bag_slots.size():
		_bag_slots[i].visible = worn
		_bag_slots[i].set_stack(player_inv.items, player_inv.bag.get_slot(i))
	var h := PANEL.y + (48.0 if worn else 0.0)
	if _panel.size.y != h:
		_panel.size = Vector2(PANEL.x, h)
		_panel.position = -_panel.size / 2.0
		_panel.queue_redraw()
	_place_side_panels()
	_cursor_icon.items = player_inv.items
	_cursor_icon.stack = player_inv.cursor
	_cursor_icon.visible = not player_inv.cursor.is_empty()


func _process(_delta: float) -> void:
	if is_open:
		_cursor_icon.position = _root.get_local_mouse_position() - Vector2(16, 16)
		if _container and _container.kind in ["furnace", "firepit"]:
			_panel.queue_redraw()   # flame and arrow gauges move every tick
		_update_tooltip()


var debug_hover: Control   # captures: stands in for the mouse (main.gd hover_slot=)


func _hovered_control() -> Control:
	return debug_hover if is_instance_valid(debug_hover) else get_viewport().gui_get_hovered_control()


## The item under the mouse (inventory slot or browser cell), or "".
func hovered_item() -> String:
	var c := _hovered_control()
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
	var c := _hovered_control()
	if c and c.has_method("hover_tag") and c.hover_tag() != "":
		text += "\nany " + c.hover_tag()
	var stack := {}
	var w := c
	while w != null and w != _root:   # the hovered slot's stack, if it has one
		var st: Variant = w.hover_stack() if w.has_method("hover_stack") else w.get("stack")   # a slot keeps its stack on its icon child
		if st is Dictionary and not st.is_empty():
			stack = st
			break
		w = w.get_parent() as Control
	if player_inv.items.is_voxel_item(id):
		# voxel items count 12.5 cm voxels; recipes take them a block (BLOCK^3) at a time (ADR 0046)
		var u := player_inv.items.units(id)
		var n := int(stack.get("count", 0))
		text += "
A block is %d voxels" % u if n == 0 else "
%d voxels = %d block%s%s" % [n, n / u, "" if n / u == 1 else "s", "" if n % u == 0 else " + %d" % (n % u)]
	var days := float(player_inv.items.get_def(id).get("spoil_days", 0.0))
	if days > 0.0:
		var sp := float(stack.get("spoil", 0.0))
		text += "\n%s (%.1f days left at 16 C)" % [Survival.freshness_name(sp), (1.0 - sp) * days]
	var cloth: Dictionary = player_inv.items.get_def(id).get("clothing", {})
	if cloth.get("armor", 0) > 0:
		text += "
Armor %d" % int(cloth["armor"])
	if cloth.get("warmth_c", 0.0) > 0.0:
		text += "
Warmth +%.1f C" % float(cloth["warmth_c"])
	var dur := Inventory.durability_of(player_inv.items, stack)   # with dur_x (ADR 0059)
	if dur > 0:
		if stack.is_empty():   # a browser cell: the maximum only
			text += "\nDurability %d" % dur
		else:
			text += "\nDurability %d / %d" % [dur - int(stack.get("wear", 0)), dur]   # ADR 0042
	if float(stack.get("dur_x", 1.0)) > 1.0:
		text += "\nLasts %.1fx as long" % float(stack["dur_x"])
	if Inventory.heat_of(stack) > 0.0:
		text += "\nHot: %d °C (%d s to cool)" % [Inventory.temp_c(stack), int(ceil(float(stack["hot_until"]) - Inventory.clock_s))]   # ADR 0087
	if player_inv.items.get_def(id).has("empties_to"):
		text += "\nHolds %d / %d voxels" % [Bucket.fill_of(stack), Bucket.capacity()]   # ADR 0091
	if stack.has("work"):
		text += "\nWork piece: %s" % player_inv.items.display_name(String(stack["work"].get("output", "")))
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
	for sv in _slots + _bag_slots + _craft_slots + _container_slots + [_offhand_slot]:
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
	if event.is_action("drop"):
		# Q over a slot drops one item from it, Ctrl + Q the stack (Minecraft)
		var sv := _slot_at(_root.get_global_mouse_position())
		if sv != null and player_inv.cursor.is_empty():
			drop_requested.emit(player_inv.drop_from(sv.group, sv.index, event.ctrl_pressed))
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


## A click on the dimmed screen outside the panel (and the recipe browser) with a stack on the cursor
## throws it out: left all of it, right one (Minecraft).
func _on_root_input(event: InputEvent) -> void:
	if not (event is InputEventMouseButton) or not event.pressed or player_inv.cursor.is_empty():
		return
	if event.button_index != MOUSE_BUTTON_LEFT and event.button_index != MOUSE_BUTTON_RIGHT:
		return
	if _panel.get_global_rect().has_point(event.global_position) or is_book_open():
		return
	drop_requested.emit(player_inv.drop_cursor(event.button_index == MOUSE_BUTTON_LEFT))
	_root.accept_event()


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
	signal alt_clicked
	var index := 0
	var favourite := false:   # a gold corner (plan item D11)
		set(v):
			if v != favourite:
				favourite = v
				queue_redraw()
	var group := ""           # PlayerInventory slot group ("inv", "bag", "craft", "container"); "" = no drags
	var is_output := false
	var hint := ""            # drawn faintly while the slot is empty (the bag slot)
	var need := 0:            # crafting grid: a voxel item short of one block; marked with what it needs (ADR 0046)
		set(v):
			if v != need:
				need = v
				_need_mark.queue_redraw()
	var _need_mark: Control   # above the icon
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
		_need_mark = Control.new()
		_need_mark.mouse_filter = Control.MOUSE_FILTER_IGNORE
		_need_mark.size = size
		_need_mark.draw.connect(func() -> void:
			if need > 0:
				_need_mark.draw_rect(Rect2(2, 2, size.x - 4, size.y - 4), Color(0.85, 0.1, 0.05, 0.35))
				var f := ThemeDB.fallback_font
				var txt := "/%d" % need
				_need_mark.draw_string(f, Vector2(4, 14), txt, HORIZONTAL_ALIGNMENT_LEFT, -1, 12, Color(0, 0, 0, 0.9))
				_need_mark.draw_string(f, Vector2(3, 13), txt, HORIZONTAL_ALIGNMENT_LEFT, -1, 12, Color(1.0, 0.8, 0.75)))
		add_child(_need_mark)
		mouse_entered.connect(func() -> void: _hover = true; queue_redraw())
		mouse_exited.connect(func() -> void: _hover = false; queue_redraw())

	func set_stack(items: ItemRegistry, stack: Dictionary) -> void:
		_icon.items = items
		_icon.stack = stack
		if hint != "":
			queue_redraw()

	func hover_item_id() -> String:
		return _icon.stack.get("id", "")

	func hover_stack() -> Dictionary:
		return _icon.stack

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
		if favourite:
			draw_colored_polygon(PackedVector2Array([Vector2(size.x - 12, 2), Vector2(size.x - 2, 2), Vector2(size.x - 2, 12)]), Color(1.0, 0.8, 0.2))
		if hint != "" and _icon.stack.is_empty():
			draw_string(ThemeDB.fallback_font, Vector2(4, size.y / 2.0 + 5), hint, HORIZONTAL_ALIGNMENT_CENTER, size.x - 8, 12, Color(0.3, 0.3, 0.3))

	func _gui_input(event: InputEvent) -> void:
		if event is InputEventMouseButton and event.pressed and (event.button_index == MOUSE_BUTTON_LEFT or event.button_index == MOUSE_BUTTON_RIGHT):
			if event.alt_pressed and event.button_index == MOUSE_BUTTON_LEFT:
				alt_clicked.emit()
			else:
				clicked.emit(event.button_index, event.shift_pressed)
			accept_event()
