class_name ServerFinder
extends RefCounted

## Finds TechnoApe servers (ADR 0145): a UDP probe to localhost, the LAN broadcast and every online
## Tailscale peer (`tailscale status --json`); a server answers on its game port + 1 with its name, seed,
## mode and player count. `servers` is address -> info after `poll()`.

const PROBE := "technoape?"
const TAILSCALE := ["tailscale", "C:/Program Files/Tailscale/tailscale.exe", "/usr/bin/tailscale", "/Applications/Tailscale.app/Contents/MacOS/Tailscale"]

var servers := {}            # "ip:port" -> {name, seed, game_mode, players, port, proto, seen_s}
var tailnet: PackedStringArray = []   # the online peers' IPv4s, from the Tailscale CLI (empty without it)
var tailnet_checked := false
var _udp := PacketPeerUDP.new()


func _init() -> void:
	_udp.set_broadcast_enabled(true)
	_udp.bind(0)


## Asks the Tailscale CLI once for the online peers (100-300 ms, blocking: call it once per menu visit).
func read_tailnet() -> void:
	tailnet_checked = true
	tailnet = PackedStringArray()
	for exe in TAILSCALE:
		var out := []
		if OS.execute(exe, ["status", "--json"], out, true) != 0 or out.is_empty():
			continue
		var d: Variant = JSON.parse_string(String(out[0]))
		if not (d is Dictionary):
			continue
		var nodes: Array = [d.get("Self", {})]
		nodes.append_array((d.get("Peer", {}) as Dictionary).values())
		for n in nodes:
			if not (n is Dictionary) or not bool(n.get("Online", false)):
				continue
			for ip in n.get("TailscaleIPs", []):
				if String(ip).contains("."):
					tailnet.append(String(ip))
		return


## One probe round to every candidate on `port + 1`.
func probe(port: int = Net.PORT) -> void:
	var targets := PackedStringArray(["127.0.0.1", "255.255.255.255"])
	targets.append_array(tailnet)
	for ip in targets:
		_udp.set_dest_address(ip, port + 1)
		_udp.put_packet(PROBE.to_utf8_buffer())


## Reads the answers that came in; a server unheard for 6 s drops off.
func poll() -> void:
	while _udp.get_available_packet_count() > 0:
		var bytes := _udp.get_packet()
		var d: Variant = JSON.parse_string(bytes.get_string_from_utf8())
		if not (d is Dictionary) or not d.has("port"):
			continue
		var key := "%s:%d" % [_udp.get_packet_ip(), int(d["port"])]
		d["seen_s"] = Time.get_ticks_msec() / 1000.0
		# one server answers on every route it is reachable by (localhost, LAN, tailnet): keep one row, localhost first
		var dup := ""
		for k in servers:
			if k != key and servers[k].get("id", "") == d.get("id", "-"):
				dup = k
		if dup != "" and not key.begins_with("127.0.0.1:"):
			servers[dup]["seen_s"] = d["seen_s"]
			continue
		if dup != "":
			servers.erase(dup)
		servers[key] = d
	var now := Time.get_ticks_msec() / 1000.0
	for k in servers.keys():
		if now - float(servers[k]["seen_s"]) > 6.0:
			servers.erase(k)


## The server side: the answer to one probe, as the bytes to send back.
static func answer(info: Dictionary) -> PackedByteArray:
	return JSON.stringify(info).to_utf8_buffer()
