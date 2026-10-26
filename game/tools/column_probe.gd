extends SceneTree

## Prints FineGen's column_info for a list of columns (x,z pairs): where does a river terrace come from?
##   godot --headless --path game --script res://tools/column_probe.gd -- seed=N cols=x1,z1;x2,z2

const Settings := preload("res://scripts/settings.gd")


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var gen := FineGen.new()
	gen.configure(int(args.get("seed", "1337")), Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	for c in String(args.get("cols", "")).split(";", false):
		var xz := c.split(",")
		var x := int(xz[0])
		var z := int(xz[1])
		print("column ", Vector2i(x, z), " height ", gen.height_at(x, z), ": ", gen.column_info(x, z))
	quit(0)
