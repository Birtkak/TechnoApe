class_name UiStyle
extends RefCounted

## The look of the game's menus (ADR 0032): the title screen, the pause menu
## and the Settings screen share the playtest menu's dark panels and raised
## buttons, with a warmer accent for the button you are on.

const ACCENT := Color(0.98, 0.80, 0.30)
const TEXT := Color(0.92, 0.92, 0.94)
const DIM := Color(0.62, 0.62, 0.68)
const PANEL := Color(0.06, 0.06, 0.08, 0.94)

static var _theme: Theme


static func theme() -> Theme:
	if _theme:
		return _theme
	var t := Theme.new()
	t.default_font_size = 17
	var states := {"normal": Color(0.17, 0.17, 0.21), "hover": Color(0.27, 0.26, 0.31),
		"pressed": Color(0.42, 0.36, 0.2), "hover_pressed": Color(0.48, 0.42, 0.24), "focus": Color(0, 0, 0, 0),
		"disabled": Color(0.12, 0.12, 0.14)}
	for type in ["Button", "CheckBox", "OptionButton", "LineEdit", "ItemList"]:
		for state in states:
			var sb := StyleBoxFlat.new()
			sb.bg_color = states[state]
			sb.set_corner_radius_all(5)
			sb.set_content_margin_all(7)
			sb.content_margin_left = 12
			sb.content_margin_right = 12
			if state == "focus":
				sb.draw_center = false
				sb.border_color = Color(ACCENT, 0.7)
				sb.set_border_width_all(2)
			else:
				sb.border_color = Color(ACCENT, 0.55) if state == "hover" else Color(1, 1, 1, 0.12)
				sb.set_border_width_all(1)
			t.set_stylebox(state, type, sb)
	t.set_stylebox("panel", "ItemList", t.get_stylebox("normal", "ItemList"))
	for c in ["font_color", "font_hover_color", "font_focus_color", "font_pressed_color"]:
		t.set_color(c, "Button", TEXT)
	t.set_color("font_disabled_color", "Button", Color(0.4, 0.4, 0.44))
	t.set_color("font_color", "Label", TEXT)
	var sel := StyleBoxFlat.new()
	sel.bg_color = Color(0.42, 0.36, 0.2)
	sel.set_corner_radius_all(4)
	t.set_stylebox("selected", "ItemList", sel)
	t.set_stylebox("selected_focus", "ItemList", sel)
	t.set_constant("v_separation", "ItemList", 8)
	var tab := StyleBoxFlat.new()
	tab.bg_color = Color(0.17, 0.17, 0.21)
	tab.set_content_margin_all(8)
	tab.content_margin_left = 16
	tab.content_margin_right = 16
	tab.set_corner_radius_all(5)
	var tab_sel := tab.duplicate()
	tab_sel.bg_color = Color(0.42, 0.36, 0.2)
	t.set_stylebox("tab_unselected", "TabContainer", tab)
	t.set_stylebox("tab_hovered", "TabContainer", tab)
	t.set_stylebox("tab_selected", "TabContainer", tab_sel)
	var pane := StyleBoxFlat.new()
	pane.bg_color = Color(0.1, 0.1, 0.12)
	pane.set_content_margin_all(14)
	pane.set_corner_radius_all(6)
	t.set_stylebox("panel", "TabContainer", pane)
	_theme = t
	return t


static func panel() -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = PANEL
	sb.border_color = Color(1, 1, 1, 0.18)
	sb.set_border_width_all(1)
	sb.set_corner_radius_all(10)
	sb.set_content_margin_all(22)
	return sb


static func label(text: String, size: int = 17, color: Color = TEXT) -> Label:
	var l := Label.new()
	l.text = text
	l.add_theme_font_size_override("font_size", size)
	l.add_theme_color_override("font_color", color)
	return l


static func button(text: String, cb: Callable, width: float = 320.0) -> Button:
	var b := Button.new()
	b.text = text
	b.custom_minimum_size = Vector2(width, 44)
	b.pressed.connect(cb)
	return b


## A centred panel holding a vertical box; returns [panel, box].
static func centered_panel(parent: Control, min_width: float = 420.0) -> Array:
	var center := CenterContainer.new()
	center.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	center.mouse_filter = Control.MOUSE_FILTER_IGNORE
	parent.add_child(center)
	var p := PanelContainer.new()
	p.add_theme_stylebox_override("panel", panel())
	p.custom_minimum_size = Vector2(min_width, 0)
	center.add_child(p)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 10)
	p.add_child(box)
	return [center, box]
