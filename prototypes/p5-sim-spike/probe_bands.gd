extends SceneTree

## Debug probe: after N ticks of the mixed scenario, print where the sand is:
## count and x/z bounding box per 8-voxel height band.

func _init() -> void:
	var w := SimWorld.new()
	w.configure(Vector3i(8, 8, 8), 6, 42)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(128, 4, 128), SimWorld.STONE)
	w.fill_box(Vector3i(16, 70, 16), Vector3i(56, 110, 56), SimWorld.SAND)
	w.fill_box(Vector3i(72, 40, 72), Vector3i(112, 80, 112), SimWorld.WATER)
	w.fill_box(Vector3i(50, 8, 50), Vector3i(78, 20, 78), SimWorld.GAS)
	for i in 6:
		w.fill_box(Vector3i(20 + i * 16, 4, 60), Vector3i(24 + i * 16, 40, 64), SimWorld.STONE)
	w.tick(900)
	var bands := []
	for y0 in range(0, 128, 8):
		var n := w.count_in_box(Vector3i(0, y0, 0), Vector3i(128, y0 + 8, 128), SimWorld.SAND)
		if n == 0:
			continue
		# bounding box by scanning 8x8 xz tiles
		var minx := 128; var maxx := -1; var minz := 128; var maxz := -1
		for z0 in range(0, 128, 8):
			for x0 in range(0, 128, 8):
				if w.count_in_box(Vector3i(x0, y0, z0), Vector3i(x0 + 8, y0 + 8, z0 + 8), SimWorld.SAND) > 0:
					minx = mini(minx, x0); maxx = maxi(maxx, x0 + 8)
					minz = mini(minz, z0); maxz = maxi(maxz, z0 + 8)
		bands.append({"y": "%d-%d" % [y0, y0 + 8], "sand": n, "x": [minx, maxx], "z": [minz, maxz]})
	print(JSON.stringify({"bands": bands, "unsupported": w.count_unsupported(SimWorld.SAND)}))
	quit(0)
