extends SceneTree

## Lists the TechnoApe servers in reach (localhost, LAN, Tailscale) as one JSON line (ADR 0145).
##   godot --headless --path game --script res://tools/net_find.gd -- [port=7777] [tailnet=0]


func _init() -> void:
	var port := Net.PORT
	var use_tailnet := true
	for a in OS.get_cmdline_user_args():
		if a.begins_with("port="):
			port = int(a.substr(5))
		if a == "tailnet=0":
			use_tailnet = false
	var f := ServerFinder.new()
	if use_tailnet:
		f.read_tailnet()
	for round in 3:
		f.probe(port)
		for i in 20:
			OS.delay_msec(25)
			f.poll()
	print(JSON.stringify({"servers": f.servers, "tailnet_peers": f.tailnet.size()}))
	quit(0)
