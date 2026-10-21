extends SceneTree

## GDScript reference kernel: same sand rule as SimWorld, single thread, no
## chunk skipping, 32^3 world. Exists only to put the native numbers in
## perspective. Prints one JSON line.

const N := 32
var cells := PackedByteArray()
var moved := PackedByteArray()


func _init() -> void:
	cells.resize(N * N * N)
	moved.resize(N * N * N)
	for z in N:
		for x in N:
			for y in 2:
				cells[x + N * (y + N * z)] = 1  # stone floor
	for z in range(8, 24):
		for y in range(12, 28):
			for x in range(8, 24):
				cells[x + N * (y + N * z)] = 2  # 16^3 sand block
	var sand_before := cells.count(2)
	var t0 := Time.get_ticks_usec()
	var ticks := 100
	var moved_total := 0
	for t in ticks:
		moved.fill(0)
		for y in N:
			for z in N:
				for x in N:
					var i := x + N * (y + N * z)
					if cells[i] != 2 or moved[i] == 1:
						continue
					if _try(i, x, y - 1, z):
						moved_total += 1
						continue
					var d := (x * 7 + z * 13 + t) & 3
					for k in 4:
						var dd := (d + k) & 3
						var nx: int = x + [1, -1, 0, 0][dd]
						var nz: int = z + [0, 0, 1, -1][dd]
						if _try(i, nx, y - 1, nz):
							moved_total += 1
							break
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	print(JSON.stringify({
		"lang": "gdscript", "world": "32^3", "ticks": ticks,
		"ms_per_tick": snappedf(ms / ticks, 0.01),
		"cells_per_ms": snappedf(N * N * N * ticks / ms, 0.1),
		"moved_cells_per_tick": moved_total / ticks,
		"sand_conserved": cells.count(2) == sand_before,
	}))
	quit(0)


func _try(i: int, tx: int, ty: int, tz: int) -> bool:
	if tx < 0 or ty < 0 or tz < 0 or tx >= N or ty >= N or tz >= N:
		return false
	var ti := tx + N * (ty + N * tz)
	if cells[ti] != 0 or moved[ti] == 1:
		return false
	cells[ti] = 2
	cells[i] = 0
	moved[ti] = 1
	moved[i] = 1
	return true
