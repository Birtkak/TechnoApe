class_name ItemIcon
extends Control

## Draws one item stack: its pixel-art sprite (ItemSprites, data/item_sprites.json;
## block-face sprites as an isometric cube), else block items as an isometric
## cube built from the material's atlas tiles (Minecraft-style), else a
## coloured tag with initials. Count in the bottom-right corner.

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
	var sprites := ItemSprites.get_default()
	var mat := material_id_for(items, id)
	var heat := Inventory.heat_of(stack)
	if sprites.has(id):
		var tex := sprites.texture(id)
		var full := Rect2(Vector2.ZERO, tex.get_size())
		if sprites.is_cube(id):
			_draw_iso(tex, full, full, s)
		else:
			draw_texture_rect(tex, Rect2(Vector2.ZERO, s), false)
			if heat > 0.0:
				# hot metal (ADR 0087): the sprite itself glows, dull red to yellow-white (modulate above 1
				# brightens it); warm but not glowing yet, a faint red cast
				var g := Inventory.glow_color(heat)
				var tint := Color(g.r * 2.6, g.g * 2.6, g.b * 2.6, g.a) if g.a > 0.0 else Color(1.5, 0.7, 0.5, 0.35)
				draw_texture_rect(tex, Rect2(Vector2.ZERO, s), false, tint)
	elif mat > 0:
		_draw_cube(mat, s)
	else:
		_draw_tag(id, s)
	if stack.has("spoil") and float(stack["spoil"]) > 0.02:
		# freshness (ADR 0040): a bar along the bottom, green while fresh, yellow when stale, brown when rotting
		var sp := float(stack["spoil"])
		var col := Color(0.3, 0.85, 0.3).lerp(Color(0.95, 0.85, 0.2), clampf(sp * 2.0, 0.0, 1.0))
		if sp > 0.5:
			col = col.lerp(Color(0.45, 0.3, 0.12), clampf((sp - 0.5) * 2.0, 0.0, 1.0))
		draw_rect(Rect2(Vector2(1, s.y - 3), Vector2(s.x - 2, 2)), Color(0, 0, 0, 0.8))
		draw_rect(Rect2(Vector2(1, s.y - 3), Vector2((s.x - 2) * (1.0 - sp), 2)), col)
	if int(stack.get("wear", 0)) > 0:
		# tool wear (ADR 0042): Minecraft's durability bar, green to red
		var dur := float(maxi(Inventory.durability_of(items, stack), 1))   # with dur_x (ADR 0059)
		var left := clampf(1.0 - float(stack["wear"]) / dur, 0.0, 1.0)
		draw_rect(Rect2(Vector2(1, s.y - 3), Vector2(s.x - 2, 2)), Color(0, 0, 0, 0.8))
		draw_rect(Rect2(Vector2(1, s.y - 3), Vector2((s.x - 2) * left, 2)), Color.from_hsv(left / 3.0, 0.9, 0.9))
	if heat > 0.0 and (not sprites.has(id) or sprites.is_cube(id)):
		# hot metal without a flat sprite (ADR 0042): a glow over the slot
		draw_rect(Rect2(Vector2.ZERO, s), Color(1.0, 0.35 + 0.35 * heat, 0.05, 0.15 + 0.3 * heat))
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
	var model := StationEntity.icon(d["name"])
	if model:
		draw_texture_rect(model, Rect2(Vector2.ZERO, s), false)   # the placed model's own cells (ADR 0084)
		return
	var t: Array = d["tiles"]
	if d.get("cross", false):
		# flat sprite for vegetation / cutout tiles
		draw_texture_rect_region(atlas, Rect2(s.x * 0.07, s.y * 0.04, s.x * 0.86, s.y * 0.92), BlockMeshes.tile_region(int(t[0]), atlas), Color.WHITE)
		return
	_draw_iso(atlas, BlockMeshes.tile_region(int(t[0]), atlas), BlockMeshes.tile_region(int(t[1]), atlas), s)


func _draw_iso(tex: Texture2D, top_region: Rect2, side_region: Rect2, s: Vector2) -> void:
	var w := s.x * 0.86
	var h := s.y * 0.92
	var ox := (s.x - w) / 2.0
	var oy := (s.y - h) / 2.0
	var top := Vector2(ox + w / 2.0, oy)
	var left := Vector2(ox, oy + h * 0.25)
	var right := Vector2(ox + w, oy + h * 0.25)
	var mid := Vector2(ox + w / 2.0, oy + h * 0.5)
	var down := Vector2(0, h * 0.5)
	_face(tex, top_region, top, right - top, left - top, Color(1, 1, 1))
	_face(tex, side_region, left, mid - left, down, Color(0.8, 0.8, 0.8))
	_face(tex, side_region, mid, right - mid, down, Color(0.6, 0.6, 0.6))
	draw_set_transform_matrix(Transform2D.IDENTITY)


func _face(tex: Texture2D, region: Rect2, origin: Vector2, x_axis: Vector2, y_axis: Vector2, tint: Color) -> void:
	draw_set_transform_matrix(Transform2D(x_axis, y_axis, origin))
	draw_texture_rect_region(tex, Rect2(0, 0, 1, 1), region, tint)


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
