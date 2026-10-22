extends SceneTree

## Walks from the nearest lake's centre outwards along +x and -z and prints the
## hydrology fields of each column around the waterline, to check the `shore`
## flag that places reeds.
##   godot_console.exe --headless --path game --script res://tools/shore_probe.gd -- seed=1337


func _init() -> void:
	var seed_value := 1337
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "seed":
			seed_value = int(kv[1])
	var gen := WorldGen.new()
	gen.configure(seed_value, 256, 768)
	gen.column_info(0, 0)
	var c: Vector3i = gen.find_nearest("lake", 0, 0, 4096)
	print("lake at ", c)
	for dir: Vector2i in [Vector2i(1, 0), Vector2i(0, -1), Vector2i(-1, 0)]:
		var rows := []
		var left_lake := -1
		for i in 200:
			var p := Vector2i(c.x, c.z) + dir * i
			var info: Dictionary = gen.column_info(p.x, p.y)
			if left_lake < 0 and not info["lake"]:
				left_lake = i
			if left_lake >= 0 and i > left_lake + 8:
				break
			if left_lake < 0 and i < 190:
				continue
			rows.append("%d,%d h=%d wl=%d lake=%s shore=%s river=%.2f t=%.2f biome=%s" % [p.x, p.y, info["height"], info["water_level"], info["lake"], info.get("shore", "?"), info["river"], info.get("temperature", 0.0), info.get("biome_name", info["biome"])])
		print("dir ", dir, " waterline at step ", left_lake)
		for r in rows:
			print("  ", r)
	quit(0)
