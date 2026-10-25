class_name Trackers
extends RefCounted

## HUD trackers (ADR 0061): recipes pinned from the recipe browser and quests tracked from the book,
## each shown as a small checklist (TrackerHud) that ticks off as the inventory fills. Up to MAX at
## once; pinning another drops the oldest. A tracked quest leaves the list when it completes. Saved
## with the player.

signal changed

const MAX := 3

var pins: Array = []         # [{type: "recipe" | "quest", id}], oldest first
var browser: RecipeBrowser   # recipes and have / need counts (its `have` reads the inventory)
var book: QuestBook


func _init(p_browser: RecipeBrowser = null, p_book: QuestBook = null) -> void:
	browser = p_browser
	book = p_book


func is_pinned(type: String, id: String) -> bool:
	return _find(type, id) >= 0


func _find(type: String, id: String) -> int:
	for i in pins.size():
		if pins[i]["type"] == type and pins[i]["id"] == id:
			return i
	return -1


## Pins or unpins; returns whether it is pinned now. Unknown ids are refused.
func toggle(type: String, id: String) -> bool:
	var i := _find(type, id)
	if i >= 0:
		pins.remove_at(i)
		changed.emit()
		return false
	if not _valid(type, id):
		return false
	pins.append({"type": type, "id": id})
	while pins.size() > MAX:
		pins.pop_front()
	changed.emit()
	return true


func pin(type: String, id: String) -> bool:
	return is_pinned(type, id) or toggle(type, id)


func _valid(type: String, id: String) -> bool:
	match type:
		"recipe":
			return browser != null and not browser.recipe(id).is_empty() and browser.recipe(id).get("type", "") != "mining"
		"quest":
			return book != null and book.quests.has(id)
	return false


func clear() -> void:
	pins.clear()
	changed.emit()


## Drops tracked quests that are complete (Main calls it when a quest completes).
func prune() -> void:
	var n := pins.size()
	pins = pins.filter(func(p: Dictionary) -> bool: return p["type"] != "quest" or book == null or not book.is_done(p["id"]))
	if pins.size() != n:
		changed.emit()


## What a tracker shows: {type, id, title, icon, rows: [{text, have, need, done}], done}.
func view(p: Dictionary) -> Dictionary:
	var rows := []
	var title := ""
	var icon := ""
	if p["type"] == "recipe":
		var r := browser.recipe(p["id"])
		var l := browser.layout(r)
		icon = String(l["output"]["id"])
		var out_n := browser.output_amount(l)
		title = browser.items.display_name(icon) + (" x%d" % out_n if out_n > 1 and browser.items.units(icon) == 1 else "")
		for n in browser.needs_list(l):
			var have := maxi(0, int(n["have"]))
			rows.append({"text": n["name"], "have": have, "need": int(n["need"]), "done": have >= int(n["need"])})
	else:
		var q: Dictionary = book.quests[p["id"]]
		title = q["title"]
		icon = String(q.get("icon", ""))
		var tasks: Array = q["tasks"]
		for i in tasks.size():
			rows.append({"text": QuestBookView.task_text_for(browser.items if browser != null else book.items, tasks[i]),
				"have": book.task_progress(p["id"], i), "need": QuestBook.task_count(tasks[i]), "done": book.task_done(p["id"], i)})
	var done := not rows.is_empty() and rows.all(func(r: Dictionary) -> bool: return r["done"])
	if p["type"] == "quest" and q_any(p["id"]):
		done = rows.any(func(r: Dictionary) -> bool: return r["done"])
	return {"type": p["type"], "id": p["id"], "title": title, "icon": icon, "rows": rows, "done": done}


func q_any(id: String) -> bool:
	return book != null and book.quests.has(id) and book.quests[id].get("mode", "all") == "any"


func views() -> Array:
	return pins.map(view)


func to_dict() -> Dictionary:
	return {"pins": pins.duplicate(true)}


func load_dict(d: Dictionary) -> void:
	pins.clear()
	for p in d.get("pins", []):
		if p is Dictionary and _valid(String(p.get("type", "")), String(p.get("id", ""))):
			pins.append({"type": String(p["type"]), "id": String(p["id"])})
	changed.emit()
