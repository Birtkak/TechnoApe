extends Node

## Multiplayer (ADR 0145): one ENet autoload on every side. The server (`server=1`, headless, no world)
## owns the edit journal, the player saves and the position relay; clients generate the world from the
## seed, apply their own edits at once, send them as ops and replay the ops they get. Single-player
## never touches this: `active` stays false and the node does not process.
##
##   server:  godot --headless --path game -- server=1 seed=1337 [port=7777] [duration=<s>]
##   client:  godot --path game -- connect=<host[:port]> name=<player>

const PROTO := 1
const PORT := 7777
const STATE_HZ := 20.0
const SAVE_S := 30.0
const SLICE := 200           # journal ops per welcome message

signal welcomed
signal failed(reason: String)
signal peer_joined(id: int, pname: String)
signal peer_left(id: int)
signal peer_state(id: int, pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool)

var active := false
var is_server := false
var applying := false          # an op from the wire is being applied: the edit hooks stay quiet
var recording := false         # a player action is writing voxels (Main's dig / place handlers, /fill, gadgets, TNT): SimBubble's
                               # set_voxel / fill_box / blast send ops only then; world systems (monolith, saplings, weather, mobs) run on every client
var player_name := "player"
var seed_value := 0
var game_mode := "survival"
var player_save := {}          # the client's saved player from the server, {} for a new one
var journal: Array = []        # ops in order: the world's edits (server), what this client knows (client)
var peers := {}                # id -> name of the other players
var main: Node                 # the game scene (set by Main._ready)
var stats := {"ops_sent": 0, "ops_recv": 0, "ops_replayed": 0, "states_sent": 0, "states_recv": 0, "verify_ok": 0, "verify_bad": 0,
	"sent_bytes": 0, "recv_bytes": 0, "journal": 0, "peers": 0}

var _by_chunk := {}            # Vector3i world chunk -> PackedInt32Array journal indices (client)
var _players := {}             # server: name -> player dict
var _players_dirty := false
var _journal_saved := 0
var _pending := PackedInt32Array()      # single voxels this frame, one op at the end of it
var _pending_mats := PackedByteArray()
var _state_accum := 0.0
var _save_accum := 0.0
var _lifetime := 0.0
var _t0 := 0
var _st: ShapeTool
var _port := PORT
var connect_arg := ""


func _ready() -> void:
	set_process(false)
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() != 2:
			continue
		match kv[0]:
			"connect": connect_arg = kv[1]
			"name": player_name = kv[1]
			"port": _port = int(kv[1])
			"duration": _lifetime = float(kv[1])
	if OS.get_cmdline_user_args().has("server=1"):
		_start_server.call_deferred()


# ---------------- server ----------------

func _start_server() -> void:
	if get_tree().current_scene:
		get_tree().current_scene.queue_free()   # the title screen: a server has no screen
	Engine.max_fps = 60
	seed_value = Settings.world_seed
	game_mode = Settings.game_mode
	var peer := ENetMultiplayerPeer.new()
	var err := peer.create_server(_port, 16)
	if err != OK:
		push_error("net: cannot listen on port %d (%d)" % [_port, err])
		get_tree().quit(1)
		return
	multiplayer.multiplayer_peer = peer
	multiplayer.peer_disconnected.connect(_on_peer_left)
	var f := FileAccess.open_compressed(_journal_path(), FileAccess.READ, FileAccess.COMPRESSION_ZSTD)
	if f:
		var v: Variant = f.get_var()
		if v is Array:
			journal = v
	_journal_saved = journal.size()
	var pf := FileAccess.open(_players_path(), FileAccess.READ)
	if pf:
		var d: Variant = JSON.parse_string(pf.get_as_text())
		if d is Dictionary:
			_players = d
	active = true
	is_server = true
	_t0 = Time.get_ticks_msec()
	set_process(true)
	print(JSON.stringify({"net_server": "listening", "port": _port, "seed": seed_value, "game_mode": game_mode, "journal": journal.size(), "players": _players.size()}))


func _journal_path() -> String:
	return "user://worlds/net_%d_g%d.ops" % [seed_value, TerrainGenerator.GEN_VERSION]


func _players_path() -> String:
	return "user://worlds/net_%d_g%d_players.json" % [seed_value, TerrainGenerator.GEN_VERSION]


func _save_files() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("user://worlds"))
	if journal.size() != _journal_saved:
		var f := FileAccess.open_compressed(_journal_path() + ".tmp", FileAccess.WRITE, FileAccess.COMPRESSION_ZSTD)
		if f:
			f.store_var(journal)
			f.close()
			DirAccess.rename_absolute(_journal_path() + ".tmp", _journal_path())
			_journal_saved = journal.size()
	if _players_dirty:
		var f := FileAccess.open(_players_path(), FileAccess.WRITE)
		if f:
			f.store_string(JSON.stringify(_players))
			_players_dirty = false


@rpc("any_peer", "reliable")
func _s_hello(proto: int, pname: String) -> void:
	if not is_server:
		return
	var id := multiplayer.get_remote_sender_id()
	if proto != PROTO:
		_c_refused.rpc_id(id, "protocol %d, this server speaks %d" % [proto, PROTO])
		return
	_c_welcome.rpc_id(id, seed_value, game_mode, _players.get(pname, {}), peers.duplicate())
	var at := 0
	while true:
		var part: Array = journal.slice(at, mini(at + SLICE, journal.size()))
		at += SLICE
		_c_journal.rpc_id(id, var_to_bytes(part), at >= journal.size())
		if at >= journal.size():
			break
	for other in peers:
		_c_peer_joined.rpc_id(other, id, pname)
	peers[id] = pname
	print("net: %s joined as peer %d (%d players)" % [pname, id, peers.size()])


func _on_peer_left(id: int) -> void:
	if not is_server:
		return
	print("net: %s left (peer %d)" % [peers.get(id, "?"), id])
	peers.erase(id)
	for other in peers:
		_c_peer_left.rpc_id(other, id)


@rpc("any_peer", "reliable")
func _s_op(bytes: PackedByteArray) -> void:
	if not is_server:
		return
	var id := multiplayer.get_remote_sender_id()
	var op := NetOps.decode(bytes)
	if op.is_empty():
		return
	journal.append(op)
	stats["ops_recv"] += 1
	for other in peers:
		if other != id:
			_c_op.rpc_id(other, bytes)


@rpc("any_peer", "unreliable_ordered")
func _s_state(pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool) -> void:
	if not is_server:
		return
	var id := multiplayer.get_remote_sender_id()
	stats["states_recv"] += 1
	for other in peers:
		if other != id:
			_c_state.rpc_id(other, id, pos, yaw, pitch, speed, flying)


@rpc("any_peer", "reliable")
func _s_save(d: Dictionary) -> void:
	if not is_server:
		return
	var pname: String = peers.get(multiplayer.get_remote_sender_id(), "")
	if pname != "":
		_players[pname] = d
		_players_dirty = true


# ---------------- client ----------------

## Connects and asks for the world; `welcomed` fires with the seed, the saved player and the journal in
## hand, `failed` with a reason.
func join(address: String, pname: String) -> void:
	player_name = pname if pname != "" else "player"
	var host := address
	var port := PORT
	if address.contains(":") and not address.contains("]"):   # host:port (an IPv6 literal keeps its colons)
		host = address.get_slice(":", 0)
		port = int(address.get_slice(":", 1))
	var peer := ENetMultiplayerPeer.new()
	var err := peer.create_client(host, port)
	if err != OK:
		failed.emit("cannot connect to %s (%d)" % [address, err])
		return
	multiplayer.multiplayer_peer = peer
	multiplayer.connected_to_server.connect(func() -> void: _s_hello.rpc_id(1, PROTO, player_name))
	multiplayer.connection_failed.connect(func() -> void: failed.emit("no server at " + address))
	multiplayer.server_disconnected.connect(func() -> void:
		print("net: the server went away")
		failed.emit("the server went away"))
	_t0 = Time.get_ticks_msec()


## Back to single-player (quit to title).
func leave() -> void:
	if not active:
		return
	flush()
	active = false
	set_process(false)
	multiplayer.multiplayer_peer = null
	journal.clear()
	_by_chunk.clear()
	peers.clear()
	main = null


@rpc("authority", "reliable")
func _c_refused(reason: String) -> void:
	failed.emit(reason)


@rpc("authority", "reliable")
func _c_welcome(s: int, mode: String, save: Dictionary, others: Dictionary) -> void:
	seed_value = s
	game_mode = mode
	player_save = save
	peers = others
	journal.clear()
	_by_chunk.clear()


@rpc("authority", "reliable")
func _c_journal(bytes: PackedByteArray, last: bool) -> void:
	var part: Variant = bytes_to_var(bytes)
	if part is Array:
		for op in part:
			_add(op)
	if last:
		active = true
		set_process(true)
		print("net: joined as %s: seed %d, %d ops, %d others" % [player_name, seed_value, journal.size(), peers.size()])
		welcomed.emit()


@rpc("authority", "reliable")
func _c_op(bytes: PackedByteArray) -> void:
	var op := NetOps.decode(bytes)
	if op.is_empty():
		return
	stats["ops_recv"] += 1
	_add(op)
	_apply(op, main.bubble if main else null)


@rpc("authority", "unreliable_ordered")
func _c_state(id: int, pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool) -> void:
	stats["states_recv"] += 1
	peer_state.emit(id, pos, yaw, pitch, speed, flying)


@rpc("authority", "reliable")
func _c_peer_joined(id: int, pname: String) -> void:
	peers[id] = pname
	peer_joined.emit(id, pname)


@rpc("authority", "reliable")
func _c_peer_left(id: int) -> void:
	peers.erase(id)
	peer_left.emit(id)


func _add(op: Dictionary) -> void:
	var i := journal.size()
	journal.append(op)
	for wc in NetOps.chunks_of(op):
		if not _by_chunk.has(wc):
			_by_chunk[wc] = PackedInt32Array()
		_by_chunk[wc].append(i)


func _apply(op: Dictionary, bubble: SimBubble) -> bool:
	if _st == null:
		_st = ShapeTool.new()
	applying = true
	var ok := NetOps.apply(op, bubble, _st)
	applying = false
	if ok and op["k"] in ["box", "voxels"]:   # the scripted proof reads these: the first cell holds the op's material
		var p: Vector3i = op["lo"] if op["k"] == "box" else Vector3i(op["cells"][0], op["cells"][1], op["cells"][2])
		var want: int = op["m"] if op["k"] == "box" else op["mats"][0]
		stats["verify_ok" if bubble.get_voxel(p) == want else "verify_bad"] += 1
	return ok


## Runs a player action with `recording` on: its voxel writes become ops.
func record(cb: Callable) -> void:
	recording = true
	cb.call()
	recording = false


## An edit made here: journaled and sent. The edit hooks call this unless `applying`.
func send_op(op: Dictionary) -> void:
	if not active or is_server or applying:
		return
	_add(op)
	stats["ops_sent"] += 1
	_s_op.rpc_id(1, NetOps.encode(op))


## A single voxel written here: batched into one `voxels` op per frame.
func note_voxel(p: Vector3i, m: int) -> void:
	if not active or is_server or applying:
		return
	_pending.append(p.x)
	_pending.append(p.y)
	_pending.append(p.z)
	_pending_mats.append(m)


func flush() -> void:
	if _pending.is_empty():
		return
	send_op(NetOps.voxels(_pending, _pending_mats))
	_pending = PackedInt32Array()
	_pending_mats = PackedByteArray()


## A bubble loaded `count` chunks of the column at world chunk `w0` from the terrain: the ops that touch
## them are applied again, in journal order. Late join, far edits and regenerated terrain all come here.
func replay_column(bubble: SimBubble, w0: Vector3i, count: int) -> void:
	if not active or is_server:
		return
	var idx := {}
	for k in count:
		var l: PackedInt32Array = _by_chunk.get(Vector3i(w0.x, w0.y + k, w0.z), PackedInt32Array())
		for i in l:
			idx[i] = true
	if idx.is_empty():
		return
	var order := idx.keys()
	order.sort()
	for i in order:
		if _apply(journal[i], bubble):
			stats["ops_replayed"] += 1


func save_player(d: Dictionary) -> void:
	if active and not is_server:
		_s_save.rpc_id(1, d)
		var host: ENetConnection = multiplayer.multiplayer_peer.get("host")
		if host:
			host.flush()   # save-and-quit leaves right after this: the packet must be on the wire


func _process(delta: float) -> void:
	var host: ENetConnection = multiplayer.multiplayer_peer.get("host") if multiplayer.multiplayer_peer else null
	if host:
		stats["sent_bytes"] += host.pop_statistic(ENetConnection.HOST_TOTAL_SENT_DATA)
		stats["recv_bytes"] += host.pop_statistic(ENetConnection.HOST_TOTAL_RECEIVED_DATA)
	_save_accum += delta
	if is_server:
		if _save_accum >= SAVE_S:
			_save_accum = 0.0
			_save_files()
		if _lifetime > 0.0 and (Time.get_ticks_msec() - _t0) / 1000.0 >= _lifetime:
			_save_files()
			print(JSON.stringify({"net_server": "done", "stats": snapshot()}))
			get_tree().quit()
		return
	flush()
	if main == null or main.player == null:
		return
	_state_accum += delta
	if _state_accum >= 1.0 / STATE_HZ:
		_state_accum = 0.0
		var p: Node = main.player
		_s_state.rpc_id(1, p.position, p._yaw, p._pitch, Vector2(p.velocity.x, p.velocity.z).length(), p.flying)
		stats["states_sent"] += 1


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST and is_server:
		_save_files()


func _exit_tree() -> void:
	if is_server:
		_save_files()


## For DebugCapture's JSON: counts plus bytes per second since the connection.
func snapshot() -> Dictionary:
	var s := stats.duplicate()
	s["journal"] = journal.size()
	s["peers"] = peers.size()
	var secs := maxf((Time.get_ticks_msec() - _t0) / 1000.0, 0.001)
	s["sent_bytes_per_s"] = snappedf(s["sent_bytes"] / secs, 0.1)
	s["recv_bytes_per_s"] = snappedf(s["recv_bytes"] / secs, 0.1)
	s["seconds"] = snappedf(secs, 0.1)
	return s
