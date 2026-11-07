class_name BuildIcon
extends RefCounted

## The build and brush modes' line icons (ADR 0144), for the rings.


## Draws the icon of a menu id ("shape:sphere", "mine:cube", "box", ...) into `r`. False when it has none.
static func draw_icon(c: CanvasItem, id: String, r: Rect2, col: Color, width := 2.0) -> bool:
	var lines := _lines(id)
	for line: PackedVector2Array in lines:
		var pts := PackedVector2Array()
		for p in line:
			pts.append(r.position + p * r.size)
		c.draw_polyline(pts, col, width, true)
	return not lines.is_empty()


static func _lines(id: String) -> Array:
	var n := id.get_slice(":", id.get_slice_count(":") - 1)
	if id.begins_with("prefab:"):
		n = "prefabs"
	elif id == "box":
		n = "fill"
	match n:
		"box", "cube":
			return _cube(0.14, 0.86, 0.1, 0.9)
		"wall":
			return [_p([0.08, 0.2, 0.92, 0.2, 0.92, 0.8, 0.08, 0.8, 0.08, 0.2]), _p([0.08, 0.4, 0.92, 0.4]), _p([0.08, 0.6, 0.92, 0.6]),
				_p([0.36, 0.2, 0.36, 0.4]), _p([0.64, 0.2, 0.64, 0.4]), _p([0.5, 0.4, 0.5, 0.6]), _p([0.22, 0.4, 0.22, 0.6]), _p([0.78, 0.4, 0.78, 0.6]),
				_p([0.36, 0.6, 0.36, 0.8]), _p([0.64, 0.6, 0.64, 0.8])]
		"sphere":
			return [_ellipse(0.5, 0.5, 0.4, 0.4), _ellipse(0.5, 0.5, 0.4, 0.13), _ellipse(0.5, 0.5, 0.13, 0.4)]
		"cylinder":
			return [_ellipse(0.5, 0.2, 0.34, 0.11), _arc(0.5, 0.8, 0.34, 0.11, 0.0, PI), _p([0.16, 0.2, 0.16, 0.8]), _p([0.84, 0.2, 0.84, 0.8])]
		"cone":
			return [_arc(0.5, 0.8, 0.36, 0.11, 0.0, PI), _p([0.14, 0.8, 0.5, 0.12, 0.86, 0.8])]
		"flat":
			return [_p([0.5, 0.34, 0.94, 0.56, 0.5, 0.78, 0.06, 0.56, 0.5, 0.34])]
		"column":
			return _cube(0.34, 0.66, 0.06, 0.94)
		"tunnel":
			var arch := _arc(0.5, 0.45, 0.32, 0.32, PI, TAU)
			arch.append(Vector2(0.82, 0.88))
			arch.insert(0, Vector2(0.18, 0.88))
			return [arch, _p([0.08, 0.88, 0.92, 0.88])]
		"stairs":
			return [_p([0.1, 0.88, 0.1, 0.68, 0.3, 0.68, 0.3, 0.48, 0.5, 0.48, 0.5, 0.28, 0.7, 0.28, 0.7, 0.12, 0.9, 0.12, 0.9, 0.88, 0.1, 0.88])]
		"slope":
			return [_p([0.1, 0.86, 0.9, 0.16, 0.9, 0.86, 0.1, 0.86])]
		"brush":
			return [_ellipse(0.5, 0.5, 0.38, 0.38), _ellipse(0.5, 0.5, 0.08, 0.08)]
		"fill":
			var c := _cube(0.14, 0.86, 0.1, 0.9)
			c.append_array([_p([0.2, 0.42, 0.5, 0.58, 0.5, 0.84]), _p([0.8, 0.42, 0.5, 0.58])])
			return c
		"dig":
			return [_arc(0.5, 0.42, 0.42, 0.26, PI, TAU), _p([0.5, 0.18, 0.5, 0.92]), _p([0.44, 0.92, 0.56, 0.92])]
		"select":
			return [_p([0.12, 0.3, 0.12, 0.12, 0.3, 0.12]), _p([0.7, 0.12, 0.88, 0.12, 0.88, 0.3]), _p([0.88, 0.7, 0.88, 0.88, 0.7, 0.88]), _p([0.3, 0.88, 0.12, 0.88, 0.12, 0.7])]
		"prefabs":
			return [_p([0.1, 0.5, 0.5, 0.14, 0.9, 0.5]), _p([0.2, 0.42, 0.2, 0.88, 0.8, 0.88, 0.8, 0.42]), _p([0.42, 0.88, 0.42, 0.62, 0.58, 0.62, 0.58, 0.88])]
	return []


static func _p(a: Array) -> PackedVector2Array:
	var out := PackedVector2Array()
	for i in range(0, a.size(), 2):
		out.append(Vector2(a[i], a[i + 1]))
	return out


static func _arc(cx: float, cy: float, rx: float, ry: float, a0: float, a1: float) -> PackedVector2Array:
	var out := PackedVector2Array()
	for k in 25:
		var a := lerpf(a0, a1, k / 24.0)
		out.append(Vector2(cx + cos(a) * rx, cy + sin(a) * ry))
	return out


static func _ellipse(cx: float, cy: float, rx: float, ry: float) -> PackedVector2Array:
	return _arc(cx, cy, rx, ry, 0.0, TAU)


## An isometric box in x0..x1, y0..y1: a hexagon and the three edges to the near corner.
static func _cube(x0: float, x1: float, y0: float, y1: float) -> Array:
	var mx := (x0 + x1) * 0.5
	var q := (y1 - y0) * 0.24
	return [_p([mx, y0, x1, y0 + q, x1, y1 - q, mx, y1, x0, y1 - q, x0, y0 + q, mx, y0]),
		_p([x0, y0 + q, mx, y0 + 2.0 * q, x1, y0 + q]), _p([mx, y0 + 2.0 * q, mx, y1])]
