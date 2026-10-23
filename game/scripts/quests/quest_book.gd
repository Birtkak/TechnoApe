class_name QuestBook
extends RefCounted

## The progression book's quests and the player's progress (ADR 0028),
## loaded from data/quests.json. Engine-agnostic, testable headless.
## Progress comes from two sources: check_inventory() (item tasks: the
## stacks the player holds, called when the inventory changes) and note()
## (event tasks, e.g. "fired" when a pit kiln finishes). Task progress
## latches to the best count seen and counts while a quest is locked; a
## quest completes once its tasks are done and its deps are complete,
## which can complete the quests after it in the same call.

signal quest_completed(id: String)

const EVENTS := ["fired", "charcoal_pit", "panned"]

var items: ItemRegistry
var chapters: Array = []   # raw chapter dicts, quests inside, in file order
var quests := {}           # id -> quest dict
var errors: Array[String] = []
var done := {}             # quest id -> true
var progress := {}         # "quest_id/task_index" -> best count
var dirty := false         # progress changed since the last save


static func load_default(p_items: ItemRegistry) -> QuestBook:
	var b := QuestBook.new(p_items)
	b.load_file("res://data/quests.json")
	return b


func _init(p_items: ItemRegistry) -> void:
	items = p_items


func load_file(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(f.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("chapters"):
		errors.append("quests.json: expected {chapters: [...]}")
		return false
	chapters = data["chapters"]
	for c in chapters:
		for q in c.get("quests", []):
			if quests.has(q["id"]):
				errors.append("duplicate quest %s" % q["id"])
			q["chapter"] = c["id"]
			quests[q["id"]] = q
	_validate()
	return errors.is_empty()


func _validate() -> void:
	for id in quests:
		var q: Dictionary = quests[id]
		if not items.has(q.get("icon", "")):
			errors.append("%s: icon %s is not an item" % [id, q.get("icon", "")])
		for d in q.get("deps", []):
			if not quests.has(d):
				errors.append("%s: unknown dep %s" % [id, d])
		if q.get("tasks", []).is_empty():
			errors.append("%s: no tasks" % id)
		for t in q.get("tasks", []):
			for spec in task_specs(t):
				if spec.begins_with("#") and items.items_with_tag(spec.substr(1)).is_empty():
					errors.append("%s: no item has tag %s" % [id, spec])
				elif not spec.begins_with("#") and not items.has(spec):
					errors.append("%s: unknown item %s" % [id, spec])
			if t.get("type") == "event" and not t.get("event") in EVENTS:
				errors.append("%s: unknown event %s" % [id, t.get("event")])
			elif not t.get("type") in ["item", "event"]:
				errors.append("%s: unknown task type %s" % [id, t.get("type")])
	for id in quests:
		if _has_cycle(id, {}):
			errors.append("%s: deps form a cycle" % id)
			return


func _has_cycle(id: String, seen: Dictionary) -> bool:
	if seen.has(id):
		return true
	seen[id] = true
	for d in quests[id].get("deps", []):
		if quests.has(d) and _has_cycle(d, seen.duplicate()):
			return true
	return false


## The item specs (ids or #tags) a task counts.
static func task_specs(t: Dictionary) -> Array:
	return t["items"] if t.has("items") else [t.get("item", "")]


static func task_count(t: Dictionary) -> int:
	return int(t.get("count", 1))


func chapter_quests(chapter_id: String) -> Array:
	for c in chapters:
		if c["id"] == chapter_id:
			return c.get("quests", [])
	return []


func is_done(id: String) -> bool:
	return done.has(id)


## Every dep is complete (roots always are).
func is_unlocked(id: String) -> bool:
	for d in quests[id].get("deps", []):
		if not done.has(d):
			return false
	return true


func task_progress(id: String, i: int) -> int:
	return mini(progress.get("%s/%d" % [id, i], 0), task_count(quests[id]["tasks"][i]))


func task_done(id: String, i: int) -> bool:
	return is_done(id) or task_progress(id, i) >= task_count(quests[id]["tasks"][i])


func tasks_done(id: String) -> bool:
	var tasks: Array = quests[id]["tasks"]
	var n := 0
	for i in tasks.size():
		if task_done(id, i):
			n += 1
	return n == tasks.size() or (quests[id].get("mode", "all") == "any" and n > 0)


## [done, total] for a chapter.
func chapter_progress(chapter_id: String) -> Vector2i:
	var qs := chapter_quests(chapter_id)
	var n := 0
	for q in qs:
		if done.has(q["id"]):
			n += 1
	return Vector2i(n, qs.size())


## Item tasks: counts what `inv` holds (plus the stack on the cursor and
## any `more` inventories: the worn bag).
func check_inventory(inv: Inventory, cursor: Dictionary = {}, more: Array = []) -> void:
	var held := {}
	for src in [inv] + more:
		for i in src.size():
			var s: Dictionary = src.get_slot(i)
			if not s.is_empty():
				held[s["id"]] = held.get(s["id"], 0) + int(s["count"])
	if not cursor.is_empty():
		held[cursor["id"]] = held.get(cursor["id"], 0) + int(cursor["count"])
	for id in quests:
		if done.has(id):
			continue
		var tasks: Array = quests[id]["tasks"]
		for i in tasks.size():
			if tasks[i].get("type") != "item":
				continue
			var n := 0
			for item_id in held:
				for spec in task_specs(tasks[i]):
					if items.matches(item_id, spec):
						n += held[item_id]
						break
			_latch(id, i, n)
	_complete_ready()


## Event tasks: `event` happened to `item_id` (e.g. "fired", "pickaxe_mold").
func note(event: String, item_id: String, count: int = 1) -> void:
	for id in quests:
		if done.has(id):
			continue
		var tasks: Array = quests[id]["tasks"]
		for i in tasks.size():
			var t: Dictionary = tasks[i]
			if t.get("type") != "event" or t.get("event") != event:
				continue
			for spec in task_specs(t):
				if items.matches(item_id, spec):
					_latch(id, i, progress.get("%s/%d" % [id, i], 0) + count)
					break
	_complete_ready()


func _latch(id: String, i: int, n: int) -> void:
	var key := "%s/%d" % [id, i]
	if n > progress.get(key, 0):
		progress[key] = n
		dirty = true


## Completes every quest whose tasks and deps are done, until nothing changes.
func _complete_ready() -> void:
	var changed := true
	while changed:
		changed = false
		for id in quests:
			if not done.has(id) and is_unlocked(id) and tasks_done(id):
				complete(id)
				changed = true


## Marks a quest complete (also /quests complete).
func complete(id: String) -> void:
	if done.has(id) or not quests.has(id):
		return
	done[id] = true
	dirty = true
	quest_completed.emit(id)


func reset() -> void:
	done.clear()
	progress.clear()
	dirty = true


func to_dict() -> Dictionary:
	return {"version": 1, "done": done.keys(), "progress": progress}


func load_dict(d: Dictionary) -> void:
	done.clear()
	progress.clear()
	for id in d.get("done", []):
		if quests.has(id):
			done[id] = true
	var p: Dictionary = d.get("progress", {})
	for k in p:
		progress[k] = int(p[k])
	dirty = false


func summary() -> Dictionary:
	var out := {}
	for c in chapters:
		var p := chapter_progress(c["id"])
		out[c["id"]] = "%d/%d" % [p.x, p.y]
	out["done"] = done.keys()
	return out
