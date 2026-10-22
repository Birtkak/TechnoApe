extends SceneTree

## Correctness check for BrickStore.get_chunk_bytes against get_cell, on a
## generated 128-brick store with surfaces expanded and a few cell edits.
##   godot_console.exe --headless --path prototypes/p7-raycast-near --script res://brick_check.gd


func _init() -> void:
	var bricks := 128
	var store := BrickStore.new()
	store.configure(Vector3i(bricks, bricks, bricks))
	var gen := WorldGen.new()
	gen.configure(1337, 256, 768)
	gen.column_info(0, 0)
	var y0 := 256 + 40 - bricks / 2
	for bz in range(0, bricks, 16):
		for by in range(0, bricks, 16):
			for bx in range(0, bricks, 16):
				var bytes: PackedByteArray = gen.generate_block(Vector3i(bx - bricks / 2, y0 + by, bz - bricks / 2), Vector3i(16, 16, 16), 0, 2)
				if not bytes.is_empty():
					store.load_voxels(Vector3i(bx, by, bz), Vector3i(16, 16, 16), bytes)
	store.expand_surfaces()
	for i in 500:
		store.set_cell(Vector3i(500 + i, 300 + (i % 7), 510 + (i % 5)), 1 + (i % 4))
	var mismatches := 0
	var checked := 0
	var first := []
	for c in [Vector3i(32, 18, 32), Vector3i(32, 19, 32), Vector3i(31, 18, 31), Vector3i(16, 16, 16), Vector3i(40, 20, 33)]:
		var bytes: PackedByteArray = store.get_chunk_bytes(c)
		for z in 16:
			for x in 16:
				for y in 16:
					var a := bytes[y + 16 * (x + 16 * z)]
					var b := store.get_cell(c * 16 + Vector3i(x, y, z))
					checked += 1
					if a != b:
						mismatches += 1
						if first.size() < 5:
							first.append([c, x, y, z, a, b])
	print(JSON.stringify({"test": "brick_check", "checked": checked, "mismatches": mismatches, "first": first, "stats": store.stats()}))
	quit(0)
