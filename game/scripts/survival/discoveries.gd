class_name Discoveries
extends RefCounted

## The discovery journal's model (ADR 0061), data in data/discoveries.json. Kinds list their entries
## from other data (biomes, landforms, mobs, items by tag, ore veins, a fixed list), so new content
## shows up without code. note("kind:name") marks an entry found the first time and emits
## `discovered`; the reward is the entry itself. Saved per world beside the quests.

signal discovered(id: String)

const SAVE_VERSION := 1

var kinds: Array = []        # [{id, title, icon, entries: [entry ids]}] in data order
var entries := {}            # "kind:name" -> {id, kind, name, title, text, icon}
var found := {}              # entry id -> {"day": game days, "date": text}
var dirty := false
var errors: Array[String] = []
var date_text: Callable      # () -> String, the calendar's date (Main sets it)
var clock_days: Callable     # () -> float


static func load_default(p_items: ItemRegistry) -> Discoveries:
	var d := Discoveries.new()
	d.build(_json("res://data/discoveries.json"), {
		"biomes": _json("res://data/biomes.json"),
		"mobs": _json("res://data/mobs.json").get("mobs", {}),
		"ore_veins": _json("res://data/ore_veins.json").get("veins", []),
		"items": p_items})
	return d


static func _json(path: String) -> Dictionary:
	var fa := FileAccess.open(path, FileAccess.READ)
	if fa == null:
		return {}
	var d = JSON.parse_string(fa.get_as_text())
	return d if d is Dictionary else {}


## Builds the kinds from `data` (discoveries.json) over `sources`: {biomes: biomes.json, mobs: {species:
## def}, ore_veins: [vein], items: ItemRegistry}.
func build(data: Dictionary, sources: Dictionary) -> void:
	kinds.clear()
	entries.clear()
	var texts: Dictionary = data.get("texts", {})
	var icons: Dictionary = data.get("icons", {})
	for k in data.get("kinds", []):
		var kid := String(k["id"])
		var kind := {"id": kid, "title": String(k.get("title", kid.capitalize())), "icon": String(k.get("icon", "")), "entries": []}
		for e in _source_entries(k, sources):
			var id := "%s:%s" % [kid, e["name"]]
			if entries.has(id):
				continue
			var text := String(texts.get(id, e.get("lore", "")))
			if text == "":
				var t := String(k.get("text", "%s"))
				text = t % e["title"] if "%s" in t else t
			entries[id] = {"id": id, "kind": kid, "name": e["name"], "title": e["title"], "text": text,
				"icon": String(icons.get(id, e.get("icon", kind["icon"])))}
			kind["entries"].append(id)
		kinds.append(kind)


func _source_entries(k: Dictionary, sources: Dictionary) -> Array:
	var out := []
	match String(k.get("from", "list")):
		"biomes":
			var skip: Array = k.get("skip_tags", [])
			var only: Array = k.get("only_tags", [])
			for b in sources.get("biomes", {}).get("biomes", []):
				var tags: Array = b.get("tags", [])
				if tags.any(func(t: String) -> bool: return t in skip):
					continue
				if not only.is_empty() and not tags.any(func(t: String) -> bool: return t in only):
					continue
				out.append({"name": String(b["name"]), "title": String(b.get("display", String(b["name"]).capitalize())), "lore": String(b.get("lore", ""))})
		"landforms":   # part B's biomes.json "landforms": names or {name, display, lore}
			for l in sources.get("biomes", {}).get("landforms", []):
				if l is Dictionary:
					out.append({"name": String(l["name"]), "title": String(l.get("display", String(l["name"]).capitalize())), "lore": String(l.get("lore", ""))})
				else:
					out.append({"name": String(l), "title": String(l).capitalize()})
		"mobs":
			var mobs: Dictionary = sources.get("mobs", {})
			for sp in mobs:
				var m: Dictionary = mobs[sp]
				out.append({"name": String(sp), "title": String(m.get("name", String(sp).capitalize())), "lore": String(m.get("lore", ""))})
		"items":
			var reg: ItemRegistry = sources.get("items")
			if reg != null:
				for id in reg.all_ids():
					var def := reg.get_def(id)
					var tags: Array = def.get("tags", [])
					if k.get("tags", []).any(func(t: String) -> bool: return t in tags):
						out.append({"name": id, "title": reg.display_name(id), "lore": String(def.get("lore", "")), "icon": id})
		"ore_veins":
			for v in sources.get("ore_veins", []):
				out.append({"name": String(v["name"]), "title": Prospecting.vein_title(String(v["name"]))})
		_:
			for n in k.get("entries", []):
				out.append({"name": String(n), "title": String(n).capitalize()})
	return out


func has(id: String) -> bool:
	return entries.has(id)


func is_found(id: String) -> bool:
	return found.has(id)


## Marks an entry found; true the first time. Unknown ids are ignored (content not in the data).
func note(id: String) -> bool:
	if not entries.has(id) or found.has(id):
		return false
	found[id] = {"day": snappedf(float(clock_days.call()) if clock_days.is_valid() else 0.0, 0.01),
		"date": String(date_text.call()) if date_text.is_valid() else ""}
	dirty = true
	discovered.emit(id)
	return true


func kind_progress(kind_id: String) -> Vector2i:
	for k in kinds:
		if k["id"] == kind_id:
			var n := 0
			for id in k["entries"]:
				if found.has(id):
					n += 1
			return Vector2i(n, k["entries"].size())
	return Vector2i.ZERO


func total_progress() -> Vector2i:
	var n := 0
	for id in found:
		if entries.has(id):
			n += 1
	return Vector2i(n, entries.size())


func to_dict() -> Dictionary:
	return {"version": SAVE_VERSION, "found": found.duplicate(true)}


func load_dict(d: Dictionary) -> void:
	found.clear()
	if int(d.get("version", 0)) != SAVE_VERSION:
		return
	var f: Dictionary = d.get("found", {})
	for id in f:
		found[String(id)] = f[id] if f[id] is Dictionary else {}   # ids no longer in the data stay saved
	dirty = false


func summary() -> Dictionary:
	var by_kind := {}
	for k in kinds:
		var p := kind_progress(k["id"])
		by_kind[k["id"]] = [p.x, p.y]
	var t := total_progress()
	return {"found": t.x, "total": t.y, "kinds": by_kind}
