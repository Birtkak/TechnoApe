extends Node

## Multiplayer (ADR 0145): one ENet autoload on every side. The server (`server=1`, headless, no world)
## owns the edit journal, the shared items, the player saves and the position relay; clients generate
## the world from the seed, apply their own edits at once, send them as ops and replay the ops they get.
## Everything a client sends in a frame goes as one batch (hundreds of reliable packets in one frame
## broke a link in the first proof run). Single-player never touches this: `active` stays false and the
## node does not process.
##
##   server:  godot --headless --path game -- server=1 seed=1337 [port=7777] [duration=<s>]
##   client:  godot --path game -- connect=<host[:port]> name=<player>

const PROTO := 3
const PORT := 7777
const STATE_HZ := 20.0
const SAVE_S := 30.0
const SLICE := 200           # journal ops per welcome message
const EDIT_KINDS := ["fill", "voxels", "box", "sphere"]

signal welcomed
signal failed(reason: String)
signal peer_joined(id: int, pname: String)
signal peer_left(id: int)
signal peer_state(id: int, pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool, pet: Array)

var active := false
var is_server := false
var applying := false          # an op from the wire is being applied: the edit hooks stay quiet
var recording := false         # a player action is writing voxels or spawning items (Main's dig / place / drop handlers, /fill,
                               # gadgets, TNT): SimBubble's set_voxel / fill_box / blast and Main.spawn_item share only then;
                               # world systems (monolith, saplings, weather, mobs, farming drops) run on every client
var items_ready := false       # Main can spawn items (set at the end of its _ready); item events wait until then
var player_name := "player"
var seed_value := 0
var game_mode := "survival"
var player_save := {}          # the client's saved player from the server, {} for a new one
var journal: Array = []        # ops in order: the world's edits (server), what this client knows (client)
var peers := {}                # id -> name of the other players
var main: Node                 # the game scene (set by Main._ready)
var stats := {"ops_sent": 0, "ops_recv": 0, "ops_replayed": 0, "states_sent": 0, "states_recv": 0, "verify_ok": 0, "verify_bad": 0,
	"items_sent": 0, "items_recv": 0, "picks_sent": 0, "picks_got": 0, "picked_recv": 0, "batches_sent": 0,
	"sent_bytes": 0, "recv_bytes": 0, "journal": 0, "peers": 0}

var _by_chunk := {}            # Vector3i world chunk -> PackedInt32Array journal indices (client)
var _players := {}             # server: name -> player dict
var _players_dirty := false
var _journal_saved := 0
var _items := {}               # server: uid -> spawn event of every shared item still lying around
var _pending := PackedInt32Array()      # single voxels this frame, one op at the end of it
var _pending_mats := PackedByteArray()
var _out: Array = []           # client: this frame's ops and item events, one batch
var _queued: Array = []        # client: item events that came before Main could spawn items
var _uid_n := 0
var _state_accum := 0.0
var _save_accum := 0.0
var _lifetime := 0.0
var _t0 := 0
var ledger: PostLedger         # server: signs, notes and trading posts (ADR 0164)
var applied: Array = []        # client: sale ids already delivered to this player (saved with them: a rejoin never delivers twice)
var _ent_queue: Array = []     # client: sign / post events that came before Main could place them
var _rid_n := 0
var _st: RefCounted            # a ShapeTool (clients only: the server has no voxel extension)
var _port := PORT
var connect_arg := ""
var server_name := ""          # server_name= on the command line, else the machine's name
var _discovery: PacketPeerUDP   # server: answers ServerFinder probes on port + 1
var _server_id := "%08x" % (randi() & 0x7fffffff)   # tells one server's answers apart on every route it is reachable by


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
			"server_name": server_name = kv[1]
	if OS.get_cmdline_user_args().has("server=1"):
		_start_server.call_deferred()


# ---------------- server ----------------

func _start_server() -> void:
	if get_tree().current_scene:
		get_tree().current_scene.queue_free()   # the title screen: a server has no screen
	Engine.max_fps = 60
	Settings.apply_cmdline()   # seed=, gamemode= (DebugCapture does this too, but not where the voxel extension is missing)
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
	if server_name == "":
		server_name = OS.get_environment("COMPUTERNAME") if OS.has_environment("COMPUTERNAME") else OS.get_environment("HOSTNAME")
	_discovery = PacketPeerUDP.new()
	if _discovery.bind(_port + 1) != OK:
		push_warning("net: port %d is taken, the server will not show in the Multiplayer list" % (_port + 1))
	var f := FileAccess.open_compressed(_journal_path(), FileAccess.READ, FileAccess.COMPRESSION_ZSTD)
	if f:
		var v: Variant = f.get_var()
		if v is Array:
			journal = v
	_journal_saved = journal.size()
	ledger = PostLedger.new(ItemRegistry.load_default())
	var lf := FileAccess.open(_ents_path(), FileAccess.READ)
	if lf:
		var ld: Variant = JSON.parse_string(lf.get_as_text())
		if ld is Dictionary:
			ledger.load_dict(ld)
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
	return "user://worlds/net_%d_g%d.ops" % [seed_value, gen_version()]


func _players_path() -> String:
	return "user://worlds/net_%d_g%d_players.json" % [seed_value, gen_version()]


## TerrainGenerator.GEN_VERSION, read from the script's text: the class extends VoxelGeneratorScript, which the
## server (no voxel extension) cannot load. test_net_ops checks it matches the constant.
static func gen_version() -> int:
	var re := RegEx.create_from_string("GEN_VERSION\\s*:=\\s*(\\d+)")
	var m := re.search(FileAccess.get_file_as_string("res://scripts/generator.gd"))
	return int(m.get_string(1)) if m else 0


func _ents_path() -> String:
	return "user://worlds/net_%d_g%d_ents.json" % [seed_value, gen_version()]


func _save_files() -> void:
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path("user://worlds"))
	if ledger != null and ledger.dirty:
		var lf := FileAccess.open(_ents_path(), FileAccess.WRITE)
		if lf:
			lf.store_string(JSON.stringify(ledger.to_dict()))
			ledger.dirty = false
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
	if not _items.is_empty():
		_c_batch.rpc_id(id, var_to_bytes(_items.values()))   # what lies around, dropped where it was spawned
	_c_ent.rpc_id(id, var_to_bytes(ledger.snapshot_for(pname)))
	for other in peers:
		_c_peer_joined.rpc_id(other, id, pname)
	peers[id] = pname
	print("net: %s joined as peer %d (%d players)" % [pname, id, peers.size()])


func _on_peer_left(id: int) -> void:
	if not is_server:
		return
	print("net: %s left (peer %d)" % [peers.get(id, "?"), id])
	var unlocked: Array = ledger.left(String(peers.get(id, "")))
	peers.erase(id)
	if not unlocked.is_empty():
		for other in peers:
			_c_ent.rpc_id(other, var_to_bytes(unlocked))
	for other in peers:
		_c_peer_left.rpc_id(other, id)


## A client's frame: edit ops go to the journal and the others; item spawns to the registry and the
## others; a pick is granted to the first asker (`picked` goes to everyone, the asker included).
@rpc("any_peer", "reliable")
func _s_batch(bytes: PackedByteArray) -> void:
	if not is_server:
		return
	var id := multiplayer.get_remote_sender_id()
	var batch: Variant = bytes_to_var(bytes)
	if not (batch is Array):
		return
	var fwd := []
	var all := []
	for ev in batch:
		if not (ev is Dictionary) or not ev.has("k"):
			continue
		var k: String = ev["k"]
		if k in EDIT_KINDS:
			if not NetOps.decode(var_to_bytes(ev)).is_empty():
				journal.append(ev)
				stats["ops_recv"] += 1
				fwd.append(ev)
		elif k == "item":
			ev["t"] = Time.get_ticks_msec()
			_items[int(ev["uid"])] = ev
			stats["items_recv"] += 1
			fwd.append(ev)
		elif k == "pick":
			if _items.erase(int(ev["uid"])):
				all.append({"k": "picked", "uid": int(ev["uid"]), "by": id})
	if not all.is_empty():
		_c_batch.rpc_id(id, var_to_bytes(all))
	if fwd.is_empty() and all.is_empty():
		return
	var out := var_to_bytes(fwd + all)
	for other in peers:
		if other != id:
			_c_batch.rpc_id(other, out)


## Signs, notes and trading posts (ADR 0164): one event per call, answered by PostLedger.
@rpc("any_peer", "reliable")
func _s_ent(bytes: PackedByteArray) -> void:
	if not is_server:
		return
	var id := multiplayer.get_remote_sender_id()
	var who: String = peers.get(id, "")
	var ev: Variant = bytes_to_var(bytes)
	if who == "" or not (ev is Dictionary) or not (ev.get("pos", Vector3i.ZERO) is Vector3i):
		return
	var pos: Vector3i = ev.get("pos", Vector3i.ZERO)
	var r := {}
	match String(ev.get("k", "")):
		"put": r = ledger.put(who, ev)
		"del": r = ledger.remove(who, pos)
		"text": r = ledger.text(who, pos, String(ev.get("text", "")))
		"open": r = ledger.post_open(who, pos)
		"save": r = ledger.post_save(who, pos, ev.get("slots", []))
		"trade": r = ledger.trade(who, pos, int(ev.get("n", -1)), int(ev.get("rid", 0)))
		"ack": ledger.ack(who, int(ev.get("rid", 0)))
	if r.is_empty():
		return
	if r.has("to"):
		_c_ent.rpc_id(id, var_to_bytes(r["to"]))
	var everyone: Array = r.get("all", [])
	var others: Array = r.get("others", [])
	for other in peers:
		var evs: Array = everyone + (others if other != id else [])
		if not evs.is_empty():
			_c_ent.rpc_id(other, var_to_bytes(evs))


@rpc("any_peer", "unreliable_ordered")
func _s_state(pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool, pet: Array) -> void:
	if not is_server:
		return
	var id := multiplayer.get_remote_sender_id()
	stats["states_recv"] += 1
	for other in peers:
		if other != id:
			_c_state.rpc_id(other, id, pos, yaw, pitch, speed, flying, pet)


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
	items_ready = false
	set_process(false)
	multiplayer.multiplayer_peer = null
	journal.clear()
	_by_chunk.clear()
	_queued.clear()
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
func _c_batch(bytes: PackedByteArray) -> void:
	var batch: Variant = bytes_to_var(bytes)
	if not (batch is Array):
		return
	for ev in batch:
		if not (ev is Dictionary) or not ev.has("k"):
			continue
		if ev["k"] in EDIT_KINDS:
			var op := NetOps.decode(var_to_bytes(ev))
			if op.is_empty():
				continue
			stats["ops_recv"] += 1
			_add(op)
			_apply(op, main.bubble if main else null)
		elif items_ready:
			_item_event(ev)
		else:
			_queued.append(ev)


@rpc("authority", "reliable")
func _c_ent(bytes: PackedByteArray) -> void:
	var evs: Variant = bytes_to_var(bytes)
	if not (evs is Array):
		return
	for ev in evs:
		if not (ev is Dictionary) or not ev.has("k"):
			continue
		if items_ready:
			_ent_event(ev)
		else:
			_ent_queue.append(ev)


func _ent_event(ev: Dictionary) -> void:
	if ev["k"] == "res" and applied.has(int(ev["rid"])) and ev.get("late", false):
		_send_ent({"k": "ack", "rid": int(ev["rid"]), "pos": Vector3i.ZERO})   # delivered before the link broke: only forget it
		return
	main.net_ent(ev)


func _send_ent(ev: Dictionary) -> void:
	if active and not is_server:
		_s_ent.rpc_id(1, var_to_bytes(ev))


## Placed / broken / written / opened here (Main, WorkSystem). No-ops offline.
func ent_put(ev: Dictionary) -> void:
	ev["k"] = "put"
	_send_ent(ev)


func ent_del(pos: Vector3i) -> void:
	_send_ent({"k": "del", "pos": pos})


func ent_text(pos: Vector3i, text: String) -> void:
	_send_ent({"k": "text", "pos": pos, "text": text})


func post_open(pos: Vector3i) -> void:
	_send_ent({"k": "open", "pos": pos})


func post_save(pos: Vector3i, slots: Array) -> void:
	_send_ent({"k": "save", "pos": pos, "slots": slots})


## Asks the server for offer n of the post: its id, unique across rejoins.
func post_trade(pos: Vector3i, n: int) -> int:
	_rid_n += 1
	var rid := (int(Time.get_unix_time_from_system()) % 100000000) * 1000 + (_rid_n % 1000)
	_send_ent({"k": "trade", "pos": pos, "n": n, "rid": rid})
	return rid


## A sale's goods are in the inventory: remember it and tell the server to stop keeping them.
func delivered(rid: int) -> void:
	applied.append(rid)
	while applied.size() > 64:
		applied.pop_front()
	_send_ent({"k": "ack", "rid": rid, "pos": Vector3i.ZERO})


func _item_event(ev: Dictionary) -> void:
	if ev["k"] == "item":
		stats["items_recv"] += 1
		main.net_item_spawn(ev)
	elif ev["k"] == "picked":
		stats["picked_recv"] += 1
		if int(ev["by"]) == multiplayer.get_unique_id():
			stats["picks_got"] += 1
		main.net_item_picked(int(ev["uid"]), int(ev["by"]) == multiplayer.get_unique_id())


## Main can spawn items now: the events that came during its _ready are delivered.
func items_now_ready() -> void:
	items_ready = true
	for ev in _queued:
		_item_event(ev)
	_queued.clear()
	for ev in _ent_queue:
		_ent_event(ev)
	_ent_queue.clear()


@rpc("authority", "unreliable_ordered")
func _c_state(id: int, pos: Vector3, yaw: float, pitch: float, speed: float, flying: bool, pet: Array) -> void:
	stats["states_recv"] += 1
	peer_state.emit(id, pos, yaw, pitch, speed, flying, pet)


@rpc("authority", "reliable")
func _c_peer_joined(id: int, pname: String) -> void:
	peers[id] = pname
	peer_joined.emit(id, pname)


@rpc("authority", "reliable")
func _c_peer_left(id: int) -> void:
	peer_left.emit(id)   # before the name goes: the chat says who left
	peers.erase(id)


func _add(op: Dictionary) -> void:
	var i := journal.size()
	journal.append(op)
	for wc in NetOps.chunks_of(op):
		if not _by_chunk.has(wc):
			_by_chunk[wc] = PackedInt32Array()
		_by_chunk[wc].append(i)


func _apply(op: Dictionary, bubble: SimBubble) -> bool:
	if _st == null:
		_st = ClassDB.instantiate("ShapeTool")
	applying = true
	var ok := NetOps.apply(op, bubble, _st)
	applying = false
	if ok and op["k"] in ["box", "voxels"]:   # the scripted proof reads these: the first cell holds the op's material
		var p: Vector3i = op["lo"] if op["k"] == "box" else Vector3i(op["cells"][0], op["cells"][1], op["cells"][2])
		var want: int = op["m"] if op["k"] == "box" else op["mats"][0]
		stats["verify_ok" if bubble.get_voxel(p) == want else "verify_bad"] += 1
	return ok


## Runs a player action with `recording` on: its voxel writes and item spawns are shared.
func record(cb: Callable) -> void:
	recording = true
	cb.call()
	recording = false


## An edit made here: journaled and sent with this frame's batch. The edit hooks call this unless `applying`.
func send_op(op: Dictionary) -> void:
	if not active or is_server or applying:
		return
	_add(op)
	stats["ops_sent"] += 1
	_out.append(op)


## A single voxel written here: batched into one `voxels` op per frame.
func note_voxel(p: Vector3i, m: int) -> void:
	if not active or is_server or applying:
		return
	_pending.append(p.x)
	_pending.append(p.y)
	_pending.append(p.z)
	_pending_mats.append(m)


## A uid for an item spawned here (this peer's id, then a counter).
func next_uid() -> int:
	_uid_n += 1
	return (multiplayer.get_unique_id() << 20) + _uid_n


## A shared item spawned by this player's action.
func send_item(uid: int, stack: Dictionary, pos: Vector3, velocity: Vector3) -> void:
	if not active or is_server:
		return
	stats["items_sent"] += 1
	_out.append({"k": "item", "uid": uid, "stack": stack, "pos": pos, "vel": velocity})


## This player wants a shared item: the server answers `picked` for whoever asked first.
func send_pick(uid: int) -> void:
	if not active or is_server:
		return
	stats["picks_sent"] += 1
	_out.append({"k": "pick", "uid": uid})


func flush() -> void:
	if not _pending.is_empty():
		send_op(NetOps.voxels(_pending, _pending_mats))
		_pending = PackedInt32Array()
		_pending_mats = PackedByteArray()
	if _out.is_empty() or not active or is_server:
		return
	_s_batch.rpc_id(1, var_to_bytes(_out))
	stats["batches_sent"] += 1
	_out = []


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
		flush()
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
		_answer_probes()
		if _save_accum >= SAVE_S:
			_save_accum = 0.0
			_save_files()
			var now := Time.get_ticks_msec()
			for uid in _items.keys():
				if now - int(_items[uid]["t"]) > Settings.item_despawn_s * 1000.0:
					_items.erase(uid)   # the clients despawned it at the same age
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
		_s_state.rpc_id(1, p.position, p._yaw, p._pitch, Vector2(p.velocity.x, p.velocity.z).length(), p.flying, main.mobs.pet_state("player") if main.get("mobs") != null else [])   # the companion rides along (ADR 0152)
		stats["states_sent"] += 1


func _notification(what: int) -> void:
	if what == NOTIFICATION_WM_CLOSE_REQUEST and is_server:
		_save_files()


func _exit_tree() -> void:
	if is_server:
		_save_files()


## The Multiplayer menu's probes (ServerFinder): who we are, so the server shows in its list.
func _answer_probes() -> void:
	while _discovery and _discovery.get_available_packet_count() > 0:
		if _discovery.get_packet().get_string_from_utf8() != ServerFinder.PROBE:
			continue
		_discovery.set_dest_address(_discovery.get_packet_ip(), _discovery.get_packet_port())
		_discovery.put_packet(ServerFinder.answer({"name": server_name, "seed": seed_value, "game_mode": game_mode, "players": peers.size(), "port": _port, "proto": PROTO, "id": _server_id}))


## For DebugCapture's JSON: counts plus bytes per second since the connection.
func snapshot() -> Dictionary:
	var s := stats.duplicate()
	s["journal"] = journal.size()
	s["peers"] = peers.size()
	s["items"] = _items.size()
	var secs := maxf((Time.get_ticks_msec() - _t0) / 1000.0, 0.001)
	s["sent_bytes_per_s"] = snappedf(s["sent_bytes"] / secs, 0.1)
	s["recv_bytes_per_s"] = snappedf(s["recv_bytes"] / secs, 0.1)
	s["seconds"] = snappedf(secs, 0.1)
	return s
