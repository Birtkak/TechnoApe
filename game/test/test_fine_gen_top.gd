extends GdUnitTestSuite

## Budget hunt 2026-11-03: a block of a column stack records the stack's exact top in the shared column context, and
## blocks above it return empty at once. The shortcut must never change what is generated: a stack generated bottom
## up (the shortcut armed by the first block) must match the same stack generated top down by a fresh generator,
## byte for byte, and the blocks above the top must be empty either way.

const SEED := 1015715378


func _stack(gen: FineGen, origin_xz: Vector2i, blocks: Array) -> Dictionary:
	var out := {}
	for by in blocks:
		out[by] = gen.generate_block(Vector3i(origin_xz.x, by * 16, origin_xz.y), Vector3i(16, 16, 16), 0, 2)
	return out


func test_a_stack_generates_the_same_in_either_order() -> void:
	var up := TerrainGenerator.new()
	up.set_seed(SEED)
	var down := TerrainGenerator.new()
	down.set_seed(SEED)
	# forest spawn area: ground, trees, plants over it; eight stacks across a few blocks
	var empties_up := 0
	for sx in range(-64, 64, 32):
		for sz in range(-64, 64, 32):
			var h := up.gen.height_at(sx + 8, sz + 8)
			var b0 := floori(h / 16.0) - 2
			var blocks := range(b0, b0 + 12)
			var a := _stack(up.gen, Vector2i(sx, sz), blocks)   # bottom up: arms the shortcut
			var rev := blocks.duplicate()
			rev.reverse()
			var b := _stack(down.gen, Vector2i(sx, sz), rev)   # top down, fresh generator: no shortcut yet
			for by in blocks:
				assert_bool(a[by] == b[by]).override_failure_message("stack (%d, %d) block y %d differs by order" % [sx, sz, by]).is_true()
				if a[by].is_empty() and by > b0 + 4:
					empties_up += 1
	assert_int(empties_up).override_failure_message("no block above the ground came back empty").is_greater(0)
