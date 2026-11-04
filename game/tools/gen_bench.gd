extends SceneTree

## Generates the blocks a flight asks for (ADR 0135): every 16-voxel footprint of a lane as wide as the render box,
## from FineGen.ground_range's bottom (the shell, ADR 0105/0124) up to its top, and prints the time per block, the
## gen_stats breakdown and a hash of every voxel (single thread), so a generator speed-up is measured and checked
## to change nothing without a game run. threads=N runs the same blocks on N WorkerThreadPool tasks (no hash).
##   godot --headless --path . --script res://tools/gen_bench.gd -- seed=1935341982 x=1148 z=5955 len=40 half=36
## prof=1 (single thread) samples where the time goes: res://out/gen_prof.json, read by tools/gen_prof.py.

const SEA_LEVEL_VOXEL := 1024       # Settings.SEA_LEVEL_VOXEL (autoloads do not exist in --script runs)
const WORLD_HEIGHT_VOXELS := 5120   # Settings.WORLD_HEIGHT_VOXELS
const B := 16                       # data block edge

var g: FineGen
var jobs: Array[Vector3i] = []


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	g = FineGen.new()
	g.configure(int(args.get("seed", "1935341982")), SEA_LEVEL_VOXEL, WORLD_HEIGHT_VOXELS)
	var bx0 := floori(int(args.get("x", "1148")) / float(B))
	var bz0 := floori(int(args.get("z", "5955")) / float(B))
	var half := int(args.get("half", "36"))    # blocks either side of the lane: 36 = the 72 m box
	var length := int(args.get("len", "40"))   # blocks along -z (the flight's heading)
	var threads := int(args.get("threads", "1"))
	# the shell's band per column as voxel_patch_shell.py takes it: the ground range of the 9 x 9 columns a 64-voxel
	# mesh block can touch, 4 m under and 64 m over it, widened to whole mesh blocks plus one data block
	var ground := {}
	for bz in range(bz0 + 4, bz0 - length - 4, -1):
		for bx in range(bx0 - half - 4, bx0 + half + 4):
			ground[Vector2i(bx, bz)] = g.ground_range(bx * B, bz * B, B)
	for bz in range(bz0, bz0 - length, -1):
		for bx in range(bx0 - half, bx0 + half):
			var lo := 1 << 30
			var hi := -(1 << 30)
			for dz in range(-4, 5):
				for dx in range(-4, 5):
					var r: Vector2i = ground[Vector2i(bx + dx, bz + dz)]
					lo = mini(lo, r.x)
					hi = maxi(hi, r.y)
			var y0 := floori(floori((lo - 32) / float(B)) / 4.0) * 4 - 1
			var y1 := (floori((floori((hi + 512) / float(B)) + 1 - 1) / 4.0) + 1) * 4 + 1
			for by in range(y0, y1):
				jobs.append(Vector3i(bx * B, by * B, bz * B))
	FineGen.gen_stats()   # drop what ground_range counted
	var t0 := Time.get_ticks_usec()
	var c0 := FineGen.thread_cpu_us()
	var hash := ""
	var prof: bool = args.get("prof", "0") == "1"
	if prof:
		FineGen.prof_start()
	if threads <= 1:
		var ctx := HashingContext.new()
		ctx.start(HashingContext.HASH_SHA256)
		for o in jobs:
			var b := g.generate_block(o, Vector3i(B, B, B), 0, 2)
			ctx.update(PackedByteArray([b.size() & 255, (b.size() >> 8) & 255]))
			if not b.is_empty():
				ctx.update(b)
		hash = ctx.finish().hex_encode()
	else:
		var per := ceili(jobs.size() / float(threads))
		var id := WorkerThreadPool.add_group_task(func(k: int) -> void:
			for i in range(k * per, mini(jobs.size(), (k + 1) * per)):
				g.generate_block(jobs[i], Vector3i(B, B, B), 0, 2), threads, threads)
		WorkerThreadPool.wait_for_group_task_completion(id)
	var us := Time.get_ticks_usec() - t0
	var cpu := FineGen.thread_cpu_us() - c0   # single thread: the generator's own CPU time
	if prof:
		var f := FileAccess.open("res://out/gen_prof.json", FileAccess.WRITE)
		f.store_string(JSON.stringify(Array(FineGen.prof_stop())))
	var s := FineGen.gen_stats()
	var keep := {}
	for k in s:
		if s[k] != 0:
			keep[k] = s[k]
	print(JSON.stringify({"gen_bench": hash, "blocks": jobs.size(), "threads": threads, "ms": us / 1000,
		"us_per_block": snappedf(float(us) * maxi(1, threads) / jobs.size(), 0.1), "blocks_per_s": roundi(jobs.size() * 1e6 / us),
		"cpu_us_per_block": snappedf(float(cpu) / jobs.size(), 0.1), "stats": keep}))
	quit()
