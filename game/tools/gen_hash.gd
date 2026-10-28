extends SceneTree

## Hashes FineGen's output over a spread of blocks (sky, surface, forests, water, deep rock; LOD 0-3), so a
## generator speed-up can be checked to change no voxel (ADR 0105): run before and after, the hashes match.
##   godot --headless --path . --script res://tools/gen_hash.gd -- seed=272905368 n=400

const SEA_LEVEL_VOXEL := 1024       # Settings.SEA_LEVEL_VOXEL (autoloads do not exist in --script runs)
const WORLD_HEIGHT_VOXELS := 5120   # Settings.WORLD_HEIGHT_VOXELS


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var g := FineGen.new()
	g.configure(int(args.get("seed", "272905368")), SEA_LEVEL_VOXEL, WORLD_HEIGHT_VOXELS)
	var n := int(args.get("n", "400"))
	var ctx := HashingContext.new()
	ctx.start(HashingContext.HASH_SHA256)
	var rng := RandomNumberGenerator.new()
	rng.seed = 77
	var t0 := Time.get_ticks_msec()
	var blocks := 0
	for i in n:
		var x := rng.randi_range(-2500, 2500) * 16
		var z := rng.randi_range(-2500, 2500) * 16
		var h := g.height_at(x, z)
		var lod := i % 4
		# a stack through the column: deep rock, the ground, the canopy and the sky above it
		for dy in [-1200, -400, -96, -48, -16, 0, 16, 48, 96, 400]:
			var y := floori((h + dy) / 16.0) * 16
			var b := g.generate_block(Vector3i(x, y, z), Vector3i(16, 16, 16), lod, 2)
			ctx.update(PackedByteArray([b.size() & 255, (b.size() >> 8) & 255]))
			if not b.is_empty():
				ctx.update(b)
			blocks += 1
	print("gen_hash %s blocks=%d ms=%d" % [ctx.finish().hex_encode(), blocks, Time.get_ticks_msec() - t0])
	quit()
