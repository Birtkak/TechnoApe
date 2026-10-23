class_name BugReport
extends RefCounted

## Bug reports (ADR 0018): one folder per report holding everything needed to
## look into it without having watched it happen. Files: report.md (the
## description plus the key facts), state.json (Main.snapshot() plus recent
## frame times and chat), screenshot.png (the frame when the report was
## started) and log.txt (the tail of Godot's log). From the editor they go to
## game/out/bug_reports/ (git-ignored, next to the autopilot runs), from an
## exported build to user://bug_reports/. Autopilot runs (`commands=/bug ...`)
## write into their own run folder (`out=`), so game/out/bug_reports/ only holds
## reports a person filed and its newest folder is the one to read.

const LOG_LINES := 300


static func folder() -> String:
	var args := OS.get_cmdline_user_args()
	if args.has("autopilot=1"):
		for a in args:
			if a.begins_with("out="):
				return a.substr(4) + "/bug_reports"
		return "res://out/bug_reports_autopilot"
	return "res://out/bug_reports" if OS.has_feature("editor") else "user://bug_reports"


## Writes a report; returns the absolute folder path, or "" on failure.
## shot = the frame to keep (taken when the report was started); null = the
## frame on screen now.
static func write(main: Node, description: String, shot: Image) -> String:
	var stamp := Time.get_datetime_string_from_system().replace(":", "-").replace("T", "_")
	var dir := "%s/%s" % [folder(), stamp]
	var abs_dir := ProjectSettings.globalize_path(dir)
	if DirAccess.make_dir_recursive_absolute(abs_dir) != OK:
		push_error("bug report: cannot create %s" % abs_dir)
		return ""
	if shot == null:
		shot = main.get_viewport().get_texture().get_image()
	if shot != null:
		shot.save_png(dir + "/screenshot.png")
	var state: Dictionary = main.snapshot()
	state["description"] = description
	state["time"] = Time.get_datetime_string_from_system()
	state["commit"] = _commit()
	state["godot"] = Engine.get_version_info()["string"]
	state["gpu"] = RenderingServer.get_video_adapter_name()
	state["seed"] = Settings.world_seed
	state["seed_text"] = Settings.world_seed_text
	var hud: Node = main.get("hud")
	if hud != null and hud.has_method("frame_times"):
		state["frame_times_ms"] = hud.frame_times()
	var chat: Node = main.get("chat")
	if chat != null:
		state["chat"] = chat.recent(30)
	_save_text(dir + "/state.json", JSON.stringify(state, "  "))
	_save_text(dir + "/log.txt", _log_tail())
	var lines := PackedStringArray([
		"# Bug report %s" % stamp,
		"",
		"**What went wrong:** %s" % (description if description.strip_edges() != "" else "(no description)"),
		"",
		"- Seed: %d%s" % [Settings.world_seed, (" (\"%s\")" % Settings.world_seed_text) if Settings.world_seed_text != "" else ""],
		"- Position: voxel %s (%s m), %s, %s" % [str(state.get("pos_voxel", "?")), str(state.get("pos_m", "?")), state.get("game_mode", "?"), state.get("mode", "?")],
		"- Looking at: %s" % str(state.get("look", {})),
		"- Health: %s" % str(state.get("player", {}).get("health", "?")),
		"- Build: commit %s, Godot %s, %s" % [state["commit"], state["godot"], state["gpu"]],
		"- FPS: %s" % str(state.get("fps", "?")),
		"",
		"Recent chat and commands:",
	])
	for c in state.get("chat", []):
		lines.append("    " + str(c))
	lines.append("")
	lines.append("Files: screenshot.png, state.json, log.txt")
	_save_text(dir + "/report.md", "\n".join(lines) + "\n")
	print("bug report saved: %s" % abs_dir)
	return abs_dir


static func _save_text(path: String, text: String) -> void:
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f != null:
		f.store_string(text)


## Last LOG_LINES lines of Godot's log file (file logging is on by default on desktop).
static func _log_tail() -> String:
	var path: String = ProjectSettings.get_setting("debug/file_logging/log_path", "user://logs/godot.log")
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		return "(no log file at %s)" % path
	var all := f.get_as_text().split("\n")
	return "\n".join(all.slice(maxi(0, all.size() - LOG_LINES)))


## The checked-out commit, read straight from .git (no process spawn).
static func _commit() -> String:
	var git := ProjectSettings.globalize_path("res://").path_join("../.git")
	var head := FileAccess.get_file_as_string(git.path_join("HEAD")).strip_edges()
	if head.begins_with("ref: "):
		var ref := head.substr(5)
		var sha := FileAccess.get_file_as_string(git.path_join(ref)).strip_edges()
		if sha == "":
			for line in FileAccess.get_file_as_string(git.path_join("packed-refs")).split("\n"):
				if line.ends_with(" " + ref):
					sha = line.split(" ")[0]
		return "%s (%s)" % [sha.left(10), ref.trim_prefix("refs/heads/")]
	return head.left(10) if head != "" else "unknown"
