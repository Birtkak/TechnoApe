extends RefCounted

## ADR 0155 `/pollute`: lays out what pollution does, on a stone slab 1 m over the ground ahead (+z) of the player:
## a pond with fish, its west third fouled (fish there gasp at the surface, then float belly-up); a clean channel
## watering a ripe wheat plot, and a second plot beside waste ground that turns sickly and wilts to dead bushes.
## `/pollute smoke` instead shuts the player in a stone booth full of smoke (cough, grey edge, then hurt; a cloth
## mask stops it). Crops take minutes at the normal random tick: run with `farm_speed=40` to watch them.

static func lay(main: Node, what: String) -> String:
	var player: Node3D = main.player
	var B := Settings.BLOCK
	if what == "smoke":
		var f := Vector3i(player.position.floor())
		main._debug_fill(f + Vector3i(-B - 1, -1, -B - 1), f + Vector3i(B + 2, 3 * B + 2, B + 2), GameMaterials.STONE)
		main._debug_fill(f + Vector3i(-B, 0, -B), f + Vector3i(B + 1, 3 * B + 1, B + 1), GameMaterials.SMOKE)
		return "a smoke booth round you (break out, or wear a cloth mask)"
	var at := player.position + Vector3(0, 0, Settings.m2v(3.0))
	var o := Vector3i(floori(at.x / B) * B, 0, floori(at.z / B) * B)
	o.y = main.world.surface_height(o.x, o.z) + int(Settings.m2v(1.0))
	main._debug_fill(o + Vector3i(-24, -4, 0), o + Vector3i(24, 0, 32), GameMaterials.STONE)   # the slab
	main._debug_fill(o + Vector3i(-24, 0, 0), o + Vector3i(24, 3 * B, 32), GameMaterials.AIR)   # room over it
	# the pond: a stone tub 1.75 m wide inside, water 0.6 m deep, the west third fouled
	main._debug_fill(o + Vector3i(-22, 0, 4), o + Vector3i(-2, 6, 28), GameMaterials.STONE)
	main._debug_fill(o + Vector3i(-21, 5, 5), o + Vector3i(-3, 6, 27), GameMaterials.AIR)   # open to the sky
	main._debug_fill(o + Vector3i(-21, 0, 5), o + Vector3i(-3, 5, 27), GameMaterials.WATER)
	main._debug_fill(o + Vector3i(-21, 0, 5), o + Vector3i(-15, 5, 27), GameMaterials.FOULED_WATER)
	for i in 4:
		var x := -19.0 if i < 2 else -7.0
		main.mobs.spawn("fish", Vector3(o) + Vector3(x, 2.5, 9.0 + 8.0 * (i % 2)))
	# the farm: a still channel, a clean wheat plot, grass, waste ground, a sick wheat plot (west to east)
	main._debug_fill(o + Vector3i(2, -1, 4), o + Vector3i(22, 0, 28), GameMaterials.GRASS)
	main._debug_fill(o + Vector3i(2, -1, 4), o + Vector3i(6, 0, 28), GameMaterials.WATER_STATIC)
	main._debug_fill(o + Vector3i(14, -1, 8), o + Vector3i(18, 0, 24), GameMaterials.WASTE_GROUND)
	var planted := 0
	for x0 in [6, 18]:
		for x in range(x0, x0 + B):
			for z in range(8, 24):
				var p := o + Vector3i(x, -1, z)
				if main.farming.till(p) and main.farming.plant(p, "wheat"):
					planted += 1
	for p in main.farming.crops.keys():
		if p.y == o.y and p.x >= o.x and p.x < o.x + 24 and p.z >= o.z and p.z < o.z + 32:
			main.farming.grow(p, 7)   # ripe: the wilt shows on a full plant
	return "a fouled pond with 4 fish and two wheat plots (one beside waste ground, %d planted) at %s" % [planted, o]
