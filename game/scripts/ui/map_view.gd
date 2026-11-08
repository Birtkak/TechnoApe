class_name MapView
extends Control

## Draws the explored map (WorldMap tiles through map_relief.gdshader) around `center`, with the
## waypoints as coloured pins and the player as an arrow. North (-Z) is up. Used by the minimap
## (edge pins: waypoints off the view sit on its rim) and the full map (names, picking).

var world_map: WorldMap
var player: Node3D
var others: RemotePlayers        # the other players (multiplayer): a dot in their coat colour and the name
var center := Vector2.ZERO        # world voxel x, z at the view's middle
var zoom := 1.0                   # screen pixels per map pixel (0.5 m)
var show_names := false
var edge_pins := false
var _tiles: Control
var _over: Control
var _mat: ShaderMaterial
const BG := Color(0.06, 0.07, 0.09, 0.92)


func _ready() -> void:
	clip_contents = true
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_mat = ShaderMaterial.new()
	_mat.shader = load("res://assets/shaders/map_relief.gdshader")
	_mat.set_shader_parameter("px_voxels", float(WorldMap.PX))
	_tiles = Control.new()
	_tiles.set_anchors_preset(Control.PRESET_FULL_RECT)
	_tiles.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_tiles.material = _mat
	_tiles.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	_tiles.draw.connect(_draw_tiles)
	add_child(_tiles)
	_over = Control.new()
	_over.set_anchors_preset(Control.PRESET_FULL_RECT)
	_over.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_over.draw.connect(_draw_over)
	add_child(_over)


func refresh() -> void:
	if world_map and _mat.get_shader_parameter("palette") == null:
		_mat.set_shader_parameter("palette", world_map.palette)
	_tiles.queue_redraw()
	_over.queue_redraw()


## Screen position (local) of world voxel column (x, z).
func to_screen(world_xz: Vector2) -> Vector2:
	return size / 2.0 + (world_xz - center) / WorldMap.PX * zoom


## World voxel column (x, z) under a local screen position.
func to_world(local: Vector2) -> Vector2:
	return center + (local - size / 2.0) / zoom * WorldMap.PX


func _draw() -> void:
	draw_rect(Rect2(Vector2.ZERO, size), BG)


func _draw_tiles() -> void:
	if world_map == null:
		return
	var lo := to_world(Vector2.ZERO) / WorldMap.PX
	var hi := to_world(size) / WorldMap.PX
	var t0 := WorldMap.tile_of_px(Vector2i(floori(lo.x), floori(lo.y)))
	var t1 := WorldMap.tile_of_px(Vector2i(floori(hi.x), floori(hi.y)))
	var side := WorldMap.TILE * zoom
	for tz in range(t0.y, t1.y + 1):
		for tx in range(t0.x, t1.x + 1):
			var tex := world_map.tile_texture(Vector2i(tx, tz))
			if tex == null:
				continue
			var at := to_screen(Vector2(tx, tz) * WorldMap.TILE * WorldMap.PX)
			_tiles.draw_texture_rect(tex, Rect2(at, Vector2(side, side)), false)


func _draw_over() -> void:
	var font := ThemeDB.fallback_font
	if world_map:
		for w in world_map.waypoints.list:
			var p: Vector3i = w["pos"]
			var s := to_screen(Vector2(p.x, p.z))
			var inside := Rect2(Vector2.ZERO, size).grow(-6.0).has_point(s)
			if not inside:
				if not edge_pins:
					continue
				s = _to_rim(s)
			var c: Color = w["colour"]
			_over.draw_circle(s, 6.0 if inside else 4.5, Color(0, 0, 0, 0.85))
			_over.draw_circle(s, 4.5 if inside else 3.2, c)
			if show_names and inside:
				var txt: String = w["name"]
				_over.draw_string_outline(font, s + Vector2(8, 5), txt, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, 4, Color(0, 0, 0, 0.9))
				_over.draw_string(font, s + Vector2(8, 5), txt, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, c.lightened(0.3))
	if others:
		var o := others.others()
		for n in o:
			var at: Vector3 = o[n][0]
			var s := to_screen(Vector2(at.x, at.z))
			var inside := Rect2(Vector2.ZERO, size).grow(-6.0).has_point(s)
			if not inside:
				if not edge_pins:
					continue
				s = _to_rim(s)
			var rgb: Array = ApeModel.VARIANTS[posmod(int(o[n][2]), ApeModel.VARIANTS.size())].get(ApeModel.COAT, ApeModel.PALETTE[ApeModel.COAT])
			var c := Color8(rgb[0], rgb[1], rgb[2])
			_over.draw_rect(Rect2(s - Vector2(5, 5), Vector2(10, 10)), Color(0, 0, 0, 0.9))
			_over.draw_rect(Rect2(s - Vector2(3.5, 3.5), Vector2(7, 7)), c)
			if inside:
				_over.draw_string_outline(font, s + Vector2(8, 5), n, HORIZONTAL_ALIGNMENT_LEFT, -1, 13, 4, Color(0, 0, 0, 0.9))
				_over.draw_string(font, s + Vector2(8, 5), n, HORIZONTAL_ALIGNMENT_LEFT, -1, 13, c.lightened(0.4))
	if player:
		var pp := to_screen(Vector2(player.position.x, player.position.z))
		var yaw: float = player.rotation.y
		var fwd := Vector2(-sin(yaw), -cos(yaw))   # the player looks along -Z of its yaw
		var right := Vector2(-fwd.y, fwd.x)
		var pts := PackedVector2Array([pp + fwd * 8.0, pp - fwd * 5.0 + right * 5.5, pp - fwd * 2.5, pp - fwd * 5.0 - right * 5.5])
		_over.draw_colored_polygon(pts, Color(1, 1, 1))
		_over.draw_polyline(pts + PackedVector2Array([pts[0]]), Color(0, 0, 0, 0.9), 1.5)


## A point outside the view moved onto its rim, along the line from the middle.
func _to_rim(s: Vector2) -> Vector2:
	var c := size / 2.0
	var d := s - c
	var k := minf((c.x - 6.0) / maxf(absf(d.x), 0.001), (c.y - 6.0) / maxf(absf(d.y), 0.001))
	return c + d * minf(k, 1.0)
