extends SceneTree
## Kernel equivalence probe (ADR 0137): a hash of every cell after 400 ticks of five scenes (spill, water bombs, lava into
## water with steam, water at -6 C, a fire past a puddle) for 1 and 6 threads. A speed-up of the native kernel must print
## the same hashes as the DLL before it:  godot --headless --path game --script res://tools/sim_equiv_probe.gd
func _w(n: Vector3i, threads: int, states: bool, amb: float) -> SimWorld:
	var w := SimWorld.new()
	w.configure(n, threads, 7)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(true)
	w.set_cascade(true)
	w.set_gravity(3.0 * 9.8 / 0.125 / 900.0)
	w.set_states(states)
	w.set_ambient_c(amb, true)
	return w
func _hash(w: SimWorld, n: Vector3i) -> int:
	var h := 0
	for z in n.z:
		for y in n.y:
			for x in n.x:
				h = hash([h, w.get_chunk_bytes(Vector3i(x, y, z), 2)])
	return h
func _init() -> void:
	for threads in [1, 6]:
		var res := []
		for scene in ["spill", "bombs", "lava", "cold", "fire"]:
			var n := Vector3i(6, 3, 6)
			var w := _w(n, threads, scene != "spill", -6.0 if scene == "cold" else 18.0)
			w.fill_box(Vector3i(0, 0, 0), Vector3i(96, 6, 96), GameMaterials.STONE)
			w.fill_box(Vector3i(30, 6, 30), Vector3i(34, 14, 60), GameMaterials.STONE)
			match scene:
				"spill": w.fill_box(Vector3i(10, 20, 10), Vector3i(26, 30, 26), GameMaterials.WATER)
				"bombs":
					for k in 6:
						w.fill_sphere(Vector3(15 + k * 12, 30, 40 + (k % 2) * 20), 6.0, GameMaterials.WATER, PackedInt32Array())
						for t in 10: w.tick(1)
				"lava":
					w.fill_box(Vector3i(10, 6, 10), Vector3i(40, 9, 40), GameMaterials.WATER)
					w.fill_sphere(Vector3(25, 20, 25), 5.0, GameMaterials.LAVA, PackedInt32Array())
				"cold": w.fill_box(Vector3i(10, 20, 10), Vector3i(26, 26, 26), GameMaterials.WATER)
				"fire":
					w.fill_box(Vector3i(8, 6, 30), Vector3i(40, 7, 31), GameMaterials.WOOD)
					w.fill_box(Vector3i(20, 6, 31), Vector3i(21, 7, 32), GameMaterials.WATER_STATIC)
					w.ignite(Vector3i(8, 6, 30))
			for t in 400: w.tick(1)
			res.append("%s=%d" % [scene, _hash(w, n)])
		print("threads ", threads, " ", " ".join(res))
	quit()
