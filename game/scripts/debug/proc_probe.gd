class_name ProcProbe
extends Node
## proc_probe=1: a marker node after each child of the root and of Main (new children get theirs every 30 frames); the
## gap between two markers is that subtree's _process time, so a slow frame names the node, godot_voxel's own updater
## included. Prints the top subtrees, our godot_voxel build's ta_* step timers over 1 ms (read, so reset, every frame)
## and the task queues of every process phase over 12 ms (ADR 0137 follow-up, 2026-11-04).

static var _marks: Array = []   # [name, usec] this frame
static var _main: Node
var label := ""


static func install(main: Node) -> void:
	_main = main
	var first := ProcProbe.new()
	first.label = "<start>"
	first.process_priority = -100000
	main.get_tree().root.add_child(first)
	_add_markers()
	var last := ProcProbe.new()
	last.label = "<end>"
	last.process_priority = 100000
	main.get_tree().root.add_child(last)


static func _add_markers() -> void:
	for parent: Node in [_main.get_tree().root, _main]:
		for c: Node in parent.get_children():
			if c is ProcProbe or c.get_index() + 1 < parent.get_child_count() and parent.get_child(c.get_index() + 1) is ProcProbe:
				continue
			var p := ProcProbe.new()
			p.label = c.name
			parent.add_child(p)
			parent.move_child(p, c.get_index() + 1)


func _process(_d: float) -> void:
	var now := Time.get_ticks_usec()
	if label == "<start>":
		_marks = [[label, now]]
		return
	_marks.append([label, now])
	if label != "<end>":
		return
	if Engine.get_process_frames() % 30 == 0:
		_add_markers.call_deferred()
	var total := (now - int(_marks[0][1])) / 1000.0
	var ts: Dictionary = _main.world.terrain.get_statistics()   # resets our build's ta_* worst-step counters each frame
	if total < 12.0:
		return
	var ta := {}
	for k: String in ts:
		if k.begins_with("ta_") and int(ts[k]) > 1000:
			ta[k] = ts[k]
	var gaps: Array = []
	for i in range(1, _marks.size()):
		gaps.append([_marks[i][0], snappedf((int(_marks[i][1]) - int(_marks[i - 1][1])) / 1000.0, 0.01)])
	gaps.sort_custom(func(a: Array, b: Array) -> bool: return a[1] > b[1])
	print("debug proc_probe frame=%d total_ms=%.1f top=%s ta=%s tasks=%s" % [Engine.get_process_frames(), total, str(gaps.slice(0, 6)), ta, VoxelEngine.get_stats().get("tasks", {})])
