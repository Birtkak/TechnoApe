class_name ClientUpdate
extends Control

## Client self-update through the server (ADR 0166). The server serves the exported Windows client
## (exe, .pck, DLLs: one folder, `client_dir=`) over plain HTTP on game port + 2, with a manifest of
## name, size and sha256 per file and the build version. A client whose build version differs is
## refused at hello with "update <version>"; the title screen then shows this screen, which downloads
## every file whose hash differs into `update/` beside the exe, checks each hash, renames the current
## files to `*.old` (the rollback), moves the new ones in and restarts the game with the same command
## line plus connect= so it joins straight away. Nothing is swapped until every file arrived intact.

const UPDATE_PREFIX := "update "   # Net's refusal: "update <server version>"
const PORT_OFFSET := 2             # game port + 1 is the ServerFinder probe

signal finished(ok: bool, message: String)

var _base := ""
var _address := ""
var _pname := ""
var _dir := ""
var _todo: Array = []      # manifest entries still to download
var _ready_files: Array = []
var _total := 0
var _done := 0
var _http: HTTPRequest
var _title: Label
var _label: Label
var _sub: Label
var _bar: ProgressBar
var _quiet := false        # the title screen's startup check: stays invisible unless there is something to fetch
var _join := true          # restart straight into the server (a join), else back to the title screen
var _t0 := 0
var _from := ""            # this build's version, read before the swap (res:// then reads the new .pck)


## The build version stamped by `tools/deploy_server.sh` into game/build_version.txt (exported in the
## .pck); "dev" for a checkout that was never deployed.
static func version() -> String:
	var f := FileAccess.open("res://build_version.txt", FileAccess.READ)
	return f.get_as_text().strip_edges() if f else "dev"


## A plain file name in the exe's folder: what a server may name in a manifest (trust boundary).
static func safe_name(n: String) -> bool:
	return n != "" and n.is_valid_filename() and not n.begins_with(".") and n != "update"


## Only an exported game can replace itself (an editor run would overwrite the Godot install), and not
## again after a restart that left it on the same build (no update loop).
static func can_update() -> bool:
	return OS.has_feature("template") and not OS.get_cmdline_user_args().has("updated_from=" + version())


## The updated build joined: the rollback copies (*.old) and the staging folder have done their job.
static func clean_up() -> void:
	if not OS.has_feature("template") or not Array(OS.get_cmdline_user_args()).any(func(x: String) -> bool: return x.begins_with("updated_from=")):
		return
	var dir := OS.get_executable_path().get_base_dir()
	for n in DirAccess.get_files_at(dir):
		if n.ends_with(".old"):
			DirAccess.remove_absolute(dir.path_join(n))
	var up := dir.path_join("update")
	for n in DirAccess.get_files_at(up):
		DirAccess.remove_absolute(up.path_join(n))
	DirAccess.remove_absolute(up)


## Shows the update screen over `parent` and starts. `address` is the server (host[:port]). `quiet`: a
## startup check that shows nothing and frees itself when the build is current or the server is away.
static func run(parent: Control, address: String, pname: String, quiet := false) -> ClientUpdate:
	var u := ClientUpdate.new()
	u._address = address
	u._pname = pname
	u._quiet = quiet
	u._join = not quiet
	parent.add_child(u)
	return u


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	visible = not _quiet
	modulate.a = 0.0
	var bg := ColorRect.new()
	bg.color = Color(0, 0, 0, 0.6)
	bg.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(bg)
	var box: VBoxContainer = UiStyle.centered_panel(self, 560.0)[1]
	_title = UiStyle.label("Updating TechnoApe", 28)
	box.add_child(_title)
	_sub = UiStyle.label("The server has a newer build. Fetching it...", 15, UiStyle.DIM)
	box.add_child(_sub)
	_bar = ProgressBar.new()
	_bar.custom_minimum_size = Vector2(0, 22)
	_bar.show_percentage = false
	var fill := StyleBoxFlat.new()
	fill.bg_color = UiStyle.ACCENT
	fill.set_corner_radius_all(5)
	var back := StyleBoxFlat.new()
	back.bg_color = Color(1, 1, 1, 0.08)
	back.set_corner_radius_all(5)
	_bar.add_theme_stylebox_override("fill", fill)
	_bar.add_theme_stylebox_override("background", back)
	box.add_child(_bar)
	_label = UiStyle.label(" ", 14, UiStyle.DIM)
	box.add_child(_label)
	var host := _address
	var port := Net.PORT
	if _address.contains(":") and not _address.contains("]"):
		host = _address.get_slice(":", 0)
		port = int(_address.get_slice(":", 1))
	_base = "http://%s:%d" % [host, port + PORT_OFFSET]
	_dir = OS.get_executable_path().get_base_dir()
	_from = version()
	_http = HTTPRequest.new()
	_http.use_threads = true
	_http.timeout = 4.0 if _quiet else 30.0
	add_child(_http)
	_http.request_completed.connect(_on_manifest, CONNECT_ONE_SHOT)
	if _http.request(_base + "/manifest.json") != OK:
		_fail("cannot reach " + _base)


func _process(delta: float) -> void:
	if visible:
		modulate.a = minf(modulate.a + delta * 4.0, 1.0)
	if _http and _http.get_body_size() > 0 and _todo.size() > 0:
		var got := _done + _http.get_downloaded_bytes()
		_bar.value = lerpf(_bar.value, 100.0 * got / maxf(_total, 1), minf(delta * 10.0, 1.0))
		var secs := maxf((Time.get_ticks_msec() - _t0) / 1000.0, 0.1)
		var rate := got / secs
		_label.text = "%.1f of %.1f MB  ·  %.1f MB/s  ·  %s" % [got / 1048576.0, _total / 1048576.0, rate / 1048576.0,
			"%d s left" % ceili((_total - got) / maxf(rate, 1.0)) if secs > 1.0 else "starting"]


func _on_manifest(result: int, code: int, _h: PackedStringArray, body: PackedByteArray) -> void:
	if result != HTTPRequest.RESULT_SUCCESS or code != 200:
		_fail("The server did not answer. Is Tailscale connected and the server up?" if result != HTTPRequest.RESULT_SUCCESS
			else "The server has no game build to hand out (HTTP %d)." % code)
		return
	var m: Variant = JSON.parse_string(body.get_string_from_utf8())
	if not (m is Dictionary and m.get("files") is Array):
		_fail("the server's manifest is broken")
		return
	print(JSON.stringify({"update": "manifest", "from": _from, "to": m.get("version", "?"), "files": (m["files"] as Array).size()}))
	if String(m.get("version", "")) == _from:
		queue_free()   # current already (the startup check's usual outcome)
		return
	for e in m["files"]:
		if not (e is Dictionary and safe_name(String(e.get("name", "")))):
			_fail("the server's manifest names a bad file")
			return
		var local := _dir.path_join(e["name"])
		var staged := _dir.path_join("update").path_join(e["name"])
		if FileAccess.file_exists(local) and FileAccess.get_sha256(local) == String(e["sha256"]):
			continue
		if FileAccess.file_exists(staged) and FileAccess.get_sha256(staged) == String(e["sha256"]):
			_ready_files.append(e["name"])   # an earlier, interrupted try already fetched it
		else:
			_todo.append(e)
			_total += int(e["size"])
	if _todo.is_empty() and _ready_files.is_empty():
		_fail("the server runs build %s but its files match this one" % m.get("version", "?"))
		return
	visible = true
	_sub.text = "Build %s  →  %s  ·  %d file%s" % [_from, m.get("version", "?"), _todo.size() + _ready_files.size(),
		"" if _todo.size() + _ready_files.size() == 1 else "s"]
	_t0 = Time.get_ticks_msec()
	DirAccess.make_dir_recursive_absolute(_dir.path_join("update"))
	_next()


func _next() -> void:
	if _todo.is_empty():
		_swap()
		return
	var e: Dictionary = _todo[0]
	_http.download_file = _dir.path_join("update").path_join(e["name"])
	_http.request_completed.connect(_on_file, CONNECT_ONE_SHOT)
	if _http.request(_base + "/" + String(e["name"]).uri_encode()) != OK:
		_fail("cannot download " + e["name"])


func _on_file(result: int, code: int, _h: PackedStringArray, _b: PackedByteArray) -> void:
	var e: Dictionary = _todo.pop_front()
	var path := _dir.path_join("update").path_join(e["name"])
	if result != HTTPRequest.RESULT_SUCCESS or code != 200 or FileAccess.get_sha256(path) != String(e["sha256"]):
		_fail("%s arrived damaged (HTTP %d, result %d): the game is unchanged" % [e["name"], code, result])
		return
	_done += int(e["size"])
	_ready_files.append(e["name"])
	_next()


## Every file is here and checked: current -> *.old, new -> current. A rename that fails undoes the ones
## before it, so the game is either all old or all new. Windows lets a running exe and loaded DLLs be renamed.
func _swap() -> void:
	_bar.value = 100.0
	_label.text = "Installing and restarting..."
	var moved: Array = []
	for n in _ready_files:
		var cur := _dir.path_join(n)
		var err := OK
		if FileAccess.file_exists(cur):
			DirAccess.remove_absolute(cur + ".old")
			err = DirAccess.rename_absolute(cur, cur + ".old")
		if err == OK:
			err = DirAccess.rename_absolute(_dir.path_join("update").path_join(n), cur)
			if err != OK and FileAccess.file_exists(cur + ".old"):
				DirAccess.rename_absolute(cur + ".old", cur)
		if err != OK:
			for m in moved:
				DirAccess.remove_absolute(_dir.path_join(m))
				DirAccess.rename_absolute(_dir.path_join(m) + ".old", _dir.path_join(m))
			_fail("cannot replace %s (%d): the game is unchanged" % [n, err])
			return
		moved.append(n)
	var user := OS.get_cmdline_user_args()
	var keep := PackedStringArray(["updated_from=" + _from] + (["connect=" + _address, "name=" + _pname] if _join else []))
	for a in user:
		if not (a.begins_with("connect=") or a.begins_with("name=") or a.begins_with("updated_from=")):
			keep.append(a)
	var args := PackedStringArray(["--headless"] if DisplayServer.get_name() == "headless" else [])   # get_cmdline_args() is empty in an export
	args.append("--")
	args.append_array(keep)
	print(JSON.stringify({"update": "installed", "files": moved, "downloaded_bytes": _done, "restart": args}))
	OS.create_process(OS.get_executable_path(), args)
	finished.emit(true, "")
	get_tree().quit()   # at once: res:// now reads the new .pck at the old one's offsets


func _fail(message: String) -> void:
	print(JSON.stringify({"update": "failed", "reason": message}))
	if _quiet and not visible:
		queue_free()   # nothing shown, nothing to say: the server is away or has no build
		return
	_title.text = "Update failed"
	_label.text = message
	_label.add_theme_color_override("font_color", UiStyle.ACCENT)
	_sub.text = "Nothing was changed: this build still plays single-player."
	_bar.visible = false
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 10)
	_label.get_parent().add_child(row)
	row.add_child(UiStyle.button("Try again", func() -> void:
		ClientUpdate.run(get_parent(), _address, _pname)
		queue_free(), 200))
	if Net.connect_arg == "":
		row.add_child(UiStyle.button("Back", queue_free, 120))
	else:
		row.add_child(UiStyle.button("Quit", get_tree().quit, 120))
	set_process(false)
	finished.emit(false, message)


# ---------------- server ----------------

## The server's side: the client folder's manifest and a tiny HTTP/1.1 GET server for it. Net polls it.
class Host:
	extends RefCounted

	var version := ""
	var manifest := {}
	var _dir := ""
	var _names := {}
	var _tcp := TCPServer.new()
	var _conns: Array = []

	func _init(client_dir: String, port: int, fallback_version: String) -> void:
		_dir = client_dir
		var vf := FileAccess.open(_dir.path_join("version.txt"), FileAccess.READ)
		version = vf.get_as_text().strip_edges() if vf else fallback_version
		var files: Array = []
		for n in DirAccess.get_files_at(_dir):
			if n == "version.txt" or n.ends_with(".old") or not ClientUpdate.safe_name(n):
				continue
			var f := FileAccess.open(_dir.path_join(n), FileAccess.READ)
			files.append({"name": n, "size": f.get_length(), "sha256": FileAccess.get_sha256(_dir.path_join(n))})
			_names[n] = true
		manifest = {"version": version, "files": files}
		if _tcp.listen(port) != OK:
			push_warning("net: port %d is taken, clients cannot update from this server" % port)
		print(JSON.stringify({"net_server": "client_updates", "port": port, "version": version, "files": files.size(), "dir": _dir}))

	func poll() -> void:
		while _tcp.is_listening() and _tcp.is_connection_available():
			_conns.append({"peer": _tcp.take_connection(), "req": PackedByteArray(), "out": PackedByteArray(), "f": null,
				"sent": false, "t": Time.get_ticks_msec()})
		for c in _conns.duplicate():
			var peer: StreamPeerTCP = c["peer"]
			peer.poll()
			if peer.get_status() != StreamPeerTCP.STATUS_CONNECTED or Time.get_ticks_msec() - int(c["t"]) > 600000:
				_conns.erase(c)
				continue
			if not c["sent"]:
				var n := peer.get_available_bytes()
				if n > 0:
					c["req"].append_array(peer.get_partial_data(n)[1])
				var head: String = c["req"].get_string_from_ascii()
				if not head.contains("\r\n\r\n"):
					if c["req"].size() > 8192:
						peer.disconnect_from_host()
						_conns.erase(c)
					continue
				_respond(c, head.get_slice(" ", 1))
			for i in 16:   # up to 1 MB per connection per frame
				if c["out"].is_empty():
					var f: FileAccess = c["f"]
					if f == null or f.eof_reached() or f.get_position() >= f.get_length():
						break
					c["out"] = f.get_buffer(65536)
				var r: Array = peer.put_partial_data(c["out"])
				if r[0] != OK:
					break
				c["out"] = c["out"].slice(int(r[1]))
				if int(r[1]) == 0:
					break
			var fd: FileAccess = c["f"]
			if c["out"].is_empty() and (fd == null or fd.get_position() >= fd.get_length()):
				peer.disconnect_from_host()
				_conns.erase(c)

	func _respond(c: Dictionary, path: String) -> void:
		c["sent"] = true
		var body := PackedByteArray()
		var size := -1
		var n := path.trim_prefix("/").uri_decode()
		if path == "/manifest.json":
			body = JSON.stringify(manifest).to_utf8_buffer()
			size = body.size()
		elif _names.has(n):
			c["f"] = FileAccess.open(_dir.path_join(n), FileAccess.READ)
			if c["f"]:
				size = c["f"].get_length()
		if size < 0:
			c["out"] = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_ascii_buffer()
			return
		c["out"] = ("HTTP/1.1 200 OK\r\nContent-Length: %d\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n" % size).to_ascii_buffer()
		c["out"].append_array(body)
