extends SceneTree

## The dedicated server's main loop (ADR 0145): no title screen, no world, no voxel extension needed
## (the Docker image has none). The Net autoload does the work once it sees server=1.
##   godot --headless --path game -s res://scripts/net/server_main.gd -- server=1 seed=1337 port=7777 server_name=Tower


func _initialize() -> void:
	if not OS.get_cmdline_user_args().has("server=1"):
		print("server_main: add server=1 (and seed=, port=, gamemode=, server_name=)")
		quit(2)
