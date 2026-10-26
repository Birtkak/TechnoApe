class_name FarColors
extends RefCounted

## The far layer's colour table (ADR 0074): one colour per material id, the average of its top atlas
## tile, linear (vertex colours are linear; the texels are sRGB like the tile averages far_fade blends
## to). Separate from FarLayer so headless probes (tools/far_probe.gd) can use it without autoloads.


## Average colour of every material's top tile, index = material id, linear (vertex colours are
## linear; the atlas texels are sRGB like the tile averages far_fade blends to).
static func material_colors() -> PackedColorArray:
	var img: Image = (load("res://assets/textures/atlas.png") as Texture2D).get_image()
	var tiles: Vector2i = GameMaterials.ATLAS_TILES
	var tw: int = img.get_width() / tiles.x
	var th: int = img.get_height() / tiles.y
	var defs := GameMaterials.defs()
	var out := PackedColorArray()
	out.resize(defs.size() + 1)
	out[0] = Color(0, 0, 0, 0)
	for i in defs.size():
		var d: Dictionary = defs[i]
		var c := Color(0.5, 0.5, 0.5)
		if d.has("tiles"):
			var tile: int = int(d["tiles"][0])
			var x0: int = (tile % tiles.x) * tw
			var y0: int = (tile / tiles.x) * th
			var sum := Vector3.ZERO
			var n := 0
			for y in th:
				for x in tw:
					var p := img.get_pixel(x0 + x, y0 + y)
					if p.a < 0.5:
						continue
					sum += Vector3(p.r, p.g, p.b)
					n += 1
			if n > 0:
				sum /= float(n)
				c = Color(sum.x, sum.y, sum.z).srgb_to_linear()
		out[i + 1] = c
	return out


