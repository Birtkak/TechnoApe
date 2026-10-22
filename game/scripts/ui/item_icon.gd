class_name ItemIcon
extends Control

## Draws one item stack: block items as an isometric cube built from the
## material's atlas tiles (Minecraft-style), other items as a coloured tag
## with initials until real icons exist. Count in the bottom-right corner.

static var atlas: Texture2D
static var _font: Font

var stack := {}:
	set(v):
		stack = v
		queue_redraw()
var items: ItemRegistry


static func color_for(item_id: String) -> Color:
	var h := float(item_id.hash() % 360) / 360.0
	return Color.from_hsv(h, 0.55, 0.85)


static func material_id_for(items_reg: ItemRegistry, item_id: String) -> int:
	var d := items_reg.get_def(item_id)
	if not d.has("material"):
		return 0
	var defs := GameMaterials.defs()
	for i in defs.size():
		if defs[i]["name"] == d["material"]:
			return i + 1
	return 0


func _draw() -> void:
	if stack.is_empty() or items == null:
		return
	if atlas == null:
		atlas = load("res://assets/textures/atlas.png")
	if _font == null:
		_font = ThemeDB.fallback_font
	var id: String = stack["id"]
	var s := size
	var mat := material_id_for(items, id)
	if mat > 0:
		_draw_cube(mat, s)
	else:
		_draw_tag(id, s)
	var count := int(stack.get("count", 1))
	if count > 1:
		var txt := str(count)
		var fs := int(s.y * 0.45)
		var w := _font.get_string_size(txt, HORIZONTAL_ALIGNMENT_RIGHT, -1, fs).x
		var pos := Vector2(s.x - w - 1, s.y - 2)
		draw_string(_font, pos + Vector2(1, 1), txt, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, Color(0, 0, 0, 0.9))
		draw_string(_font, pos, txt, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, Color.WHITE)


## Isometric cube: top rhombus, left and right faces, shaded like Minecraft's
## inventory icons. Each face is a unit square under an affine transform.
func _draw_cube(mat: int, s: Vector2) -> void:
	var d: Dictionary = GameMaterials.defs()[mat - 1]
	var t: Array = d["tiles"]
	var w := s.x * 0.86
	var h := s.y * 0.92
	var ox := (s.x - w) / 2.0
	var oy := (s.y - h) / 2.0
	var top := Vector2(ox + w / 2.0, oy)
	var left := Vector2(ox, oy + h * 0.25)
	var right := Vector2(ox + w, oy + h * 0.25)
	var mid := Vector2(ox + w / 2.0, oy + h * 0.5)
	var down := Vector2(0, h * 0.5)
	var cross: bool = d.get("cross", false)
	if cross:
		# flat sprite for vegetation / cutout tiles
		draw_texture_rect_region(atlas, Rect2(ox, oy, w, h), BlockMeshes.tile_region(int(t[0]), atlas), Color.WHITE)
		return
	_face(int(t[0]), top, right - top, left - top, Color(1, 1, 1))
	_face(int(t[1]), left, mid - left, down, Color(0.8, 0.8, 0.8))
	_face(int(t[1]), mid, right - mid, down, Color(0.6, 0.6, 0.6))
	draw_set_transform_matrix(Transform2D.IDENTITY)


func _face(tile: int, origin: Vector2, x_axis: Vector2, y_axis: Vector2, tint: Color) -> void:
	draw_set_transform_matrix(Transform2D(x_axis, y_axis, origin))
	draw_texture_rect_region(atlas, Rect2(0, 0, 1, 1), BlockMeshes.tile_region(tile, atlas), tint)


func _draw_tag(id: String, s: Vector2) -> void:
	var c := color_for(id)
	var d := items.get_def(id)
	var r := Rect2(s.x * 0.12, s.y * 0.12, s.x * 0.76, s.y * 0.76)
	var sb := StyleBoxFlat.new()
	sb.bg_color = c
	sb.border_color = c.darkened(0.45)
	sb.set_border_width_all(maxi(1, int(s.x / 16)))
	sb.set_corner_radius_all(int(s.x * 0.12))
	draw_style_box(sb, r)
	var name: String = d.get("name", id)
	var initials := ""
	for part in name.split(" ", false):
		initials += part.substr(0, 1).to_upper()
		if initials.length() >= 2:
			break
	var fs := int(s.y * 0.4)
	var tw := _font.get_string_size(initials, HORIZONTAL_ALIGNMENT_LEFT, -1, fs).x
	var pos := Vector2((s.x - tw) / 2.0, s.y * 0.5 + fs * 0.35)
	draw_string(_font, pos + Vector2(1, 1), initials, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, Color(0, 0, 0, 0.6))
	draw_string(_font, pos, initials, HORIZONTAL_ALIGNMENT_LEFT, -1, fs, Color.WHITE)
	if d.has("tier"):
		# tier pip for tools
		var pip := Color(0.75, 0.55, 0.3) if int(d["tier"]) == 0 else (Color(0.6, 0.6, 0.62) if int(d["tier"]) == 1 else Color(0.9, 0.9, 0.95))
		draw_rect(Rect2(s.x * 0.15, s.y * 0.15, s.x * 0.18, s.y * 0.18), pip)
