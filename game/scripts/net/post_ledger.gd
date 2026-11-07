class_name PostLedger
extends RefCounted

## What the server keeps of the things players leave for each other (ADR 0164): signs and notes
## (their words) and trading posts (owner, offers, stock, takings). The server is the one place a sale
## happens: it runs TradingPostEntity.sell() on its own copy, caches the answer by (player, request id) so
## a repeated request is answered, never sold twice, and keeps the goods as a delivery until the buyer
## acks it, so a dropped link gives them back at the next join. Pure logic, no networking: Net feeds it
## events and sends what it returns. Every word and slot that comes in is checked here again.
##
## An op returns {to: [events for the sender], others: [events for everyone else], all: [events for everyone]}.

const KINDS := ["sign", "note", "post"]
const MAX_DONE := 512

var items: ItemRegistry
var ents := {}          # "x,y,z" -> entity dictionary
var deliveries := {}    # player -> {rid(str): [stacks]}: sold, not yet acked
var _done := {}         # "player:rid" -> the reply sent (dedupes a repeated request)
var _locked := {}       # key -> player whose owner screen is open
var dirty := false


func _init(p_items: ItemRegistry) -> void:
	items = p_items


static func key_of(p: Vector3i) -> String:
	return "%d,%d,%d" % [p.x, p.y, p.z]


func _post_of(key: String) -> TradingPostEntity:
	var e: Dictionary = ents[key]
	var post := TradingPostEntity.new(items, e["pos"])
	post.owner_name = e["owner"]
	post.load_slots(e["slots"])
	post.busy = _locked.has(key)
	return post


## Slots from the wire: exactly SIZE, known tradable ids, counts within a stack, nothing else kept.
func clean_slots(raw) -> Array:
	var out := []
	var probe := TradingPostEntity.new(items, Vector3i.ZERO)
	for i in TradingPostEntity.SIZE:
		var s = raw[i] if raw is Array and i < raw.size() else {}
		if not (s is Dictionary) or s.is_empty() or not s.has("id") or not s.has("count"):
			out.append({})
			continue
		var id := String(s["id"])
		var n := int(s["count"])
		if not probe.tradable(id) or n < 1:
			out.append({})
			continue
		out.append({"id": id, "count": mini(n, items.max_stack(id))})
	return out


func put(sender: String, ev: Dictionary) -> Dictionary:
	var type := String(ev.get("type", ""))
	var pos = ev.get("pos")
	if type not in KINDS or not (pos is Vector3i):
		return {}
	var key := key_of(pos)
	if ents.has(key) and ents[key]["owner"] != sender:
		return {}
	var e := {"type": type, "pos": pos, "owner": sender, "furniture": String(ev.get("furniture", "")).left(24),
		"facing": clampi(int(ev.get("facing", 0)), 0, 3), "text": "", "slots": []}
	if type == "post":
		e["slots"] = clean_slots(ev.get("slots", []))
	else:
		e["text"] = SignText.note(String(ev.get("text", ""))) if type == "note" else SignText.sign(String(ev.get("text", "")))
	ents[key] = e
	dirty = true
	return {"others": [_put_event(e)]}


func _put_event(e: Dictionary) -> Dictionary:
	var ev: Dictionary = e.duplicate(true)
	ev["k"] = "put"
	return ev


func remove(sender: String, pos: Vector3i) -> Dictionary:
	var key := key_of(pos)
	if not ents.has(key):
		return {}
	var e: Dictionary = ents[key]
	if e["type"] == "post" and e["owner"] != sender:
		return {"to": [{"k": "deny", "why": "That trading post is %s's" % e["owner"]}]}
	ents.erase(key)
	_locked.erase(key)
	dirty = true
	return {"others": [{"k": "del", "pos": pos}]}


func text(sender: String, pos: Vector3i, t: String) -> Dictionary:
	var key := key_of(pos)
	if not ents.has(key) or ents[key]["type"] == "post":
		return {}
	var e: Dictionary = ents[key]
	e["text"] = SignText.note(t) if e["type"] == "note" else SignText.sign(t)
	dirty = true
	return {"others": [{"k": "text", "pos": pos, "text": e["text"]}]}


## The owner wants the stock screen: the post is locked against sales and the server's slots come back.
func post_open(sender: String, pos: Vector3i) -> Dictionary:
	var key := key_of(pos)
	if not ents.has(key) or ents[key]["type"] != "post":
		return {"to": [{"k": "deny", "why": "No such trading post"}]}
	if ents[key]["owner"] != sender:
		return {"to": [{"k": "deny", "why": "That trading post is %s's" % ents[key]["owner"]}]}
	_locked[key] = sender
	return {"to": [{"k": "post", "pos": pos, "slots": ents[key]["slots"], "reopen": true}], "all": [{"k": "busy", "pos": pos, "busy": true}]}


## The owner closes the stock screen with their slots; the lock goes.
func post_save(sender: String, pos: Vector3i, slots) -> Dictionary:
	var key := key_of(pos)
	if not ents.has(key) or ents[key]["type"] != "post" or ents[key]["owner"] != sender:
		return {}
	if _locked.has(key) and _locked[key] != sender:
		return {}
	ents[key]["slots"] = clean_slots(slots)
	_locked.erase(key)
	dirty = true
	return {"all": [{"k": "post", "pos": pos, "slots": ents[key]["slots"], "reopen": false}, {"k": "busy", "pos": pos, "busy": false}]}


## `sender` buys offer k of the post at pos; rid is theirs, unique per request. The answer to a repeated
## rid is the first answer, and nothing moves a second time.
func trade(sender: String, pos: Vector3i, k: int, rid: int) -> Dictionary:
	var dkey := "%s:%d" % [sender, rid]
	if _done.has(dkey):
		return {"to": [_done[dkey]]}
	var key := key_of(pos)
	var res := {"k": "res", "rid": rid, "ok": false, "why": "No such trading post", "give": []}
	var out := {}
	if ents.has(key) and ents[key]["type"] == "post":
		var post := _post_of(key)
		var sale := post.sell(k)
		if sale.is_empty():
			res["why"] = post.check_sale(k)
		else:
			res["ok"] = true
			res["why"] = ""
			res["give"] = sale["give"]
			res["want"] = sale["want"]
			ents[key]["slots"] = post.slots_array()
			if not deliveries.has(sender):
				deliveries[sender] = {}
			deliveries[sender][str(rid)] = sale["give"]
			dirty = true
			out["all"] = [{"k": "post", "pos": pos, "slots": ents[key]["slots"], "reopen": false}]
	_done[dkey] = res
	if _done.size() > MAX_DONE:
		_done.erase(_done.keys()[0])
	out["to"] = [res]
	return out


func ack(sender: String, rid: int) -> void:
	if deliveries.has(sender) and deliveries[sender].erase(str(rid)):
		dirty = true
		if deliveries[sender].is_empty():
			deliveries.erase(sender)


func left(sender: String) -> Array:
	var unlocked := []
	for key in _locked.keys():
		if _locked[key] == sender:
			_locked.erase(key)
			unlocked.append({"k": "busy", "pos": ents[key]["pos"] if ents.has(key) else Vector3i.ZERO, "busy": false})
	return unlocked


## Everything a joining player needs: every entity, and the sales they never acked.
func snapshot_for(player: String) -> Array:
	var out := []
	for key in ents:
		out.append(_put_event(ents[key]))
		if ents[key]["type"] == "post":
			out.append({"k": "post", "pos": ents[key]["pos"], "slots": ents[key]["slots"], "reopen": false})
			if _locked.has(key):
				out.append({"k": "busy", "pos": ents[key]["pos"], "busy": true})
	for rid in deliveries.get(player, {}):
		out.append({"k": "res", "rid": int(rid), "ok": true, "why": "", "give": deliveries[player][rid], "late": true})
	return out


# ---- persistence (JSON next to the player saves) ----

func to_dict() -> Dictionary:
	var list := []
	for key in ents:
		var e: Dictionary = ents[key].duplicate(true)
		e["pos"] = [e["pos"].x, e["pos"].y, e["pos"].z]
		list.append(e)
	return {"ents": list, "deliveries": deliveries}


func load_dict(d: Dictionary) -> void:
	ents.clear()
	for e in d.get("ents", []):
		var pa: Array = e.get("pos", [])
		if pa.size() != 3 or String(e.get("type", "")) not in KINDS:
			continue
		var pos := Vector3i(int(pa[0]), int(pa[1]), int(pa[2]))
		e["pos"] = pos
		e["facing"] = int(e.get("facing", 0))
		e["owner"] = SignText.name_of(String(e.get("owner", "")))
		e["slots"] = clean_slots(e.get("slots", [])) if e["type"] == "post" else []
		ents[key_of(pos)] = e
	deliveries = d.get("deliveries", {})
