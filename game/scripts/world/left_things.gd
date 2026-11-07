class_name LeftThings
extends RefCounted

## What friends leave for each other (ADR 0164), glued into Main: the text screen for signs and notes,
## a note read or written in the hand, trading posts (owner screen, customer screen, a sale offline or
## through the server) and the events the server sends about all of them (Net._c_ent -> net_event).

var main: Node
var screen: TextScreen
var applying := false            # an event from the server is being applied: nothing goes back out
var _target := Vector3i.MAX      # the sign or note whose text the screen holds
var _in_hand := false
var _post: TradingPostEntity     # the post whose owner screen is open
var _pending := {}               # rid -> the payment taken for a sale the server has not answered yet
var _asked_open := Vector3i.MAX  # online: the owner asked to open this post; its screen opens on the answer


func _init(p_main: Node) -> void:
	main = p_main
	screen = TextScreen.new()
	main.add_child(screen)
	screen.done.connect(_on_done)
	screen.closed.connect(_on_closed)


func _me() -> String:
	return Net.player_name if Net.active else ""


func _say(t: String) -> void:
	if main.chat != null and t != "":
		main.chat.add_message(t)


func is_open() -> bool:
	return screen.is_open


# ---------------- signs and notes ----------------

func edit_furniture(e: FurnitureEntity) -> void:
	_target = e.pos
	_in_hand = false
	var note: bool = e.furniture == "note"
	main.player.set_ui_open(true, false)
	screen.open("Note" if note else "Sign", e.text, SignText.NOTE_LINES if note else SignText.SIGN_LINES,
		SignText.NOTE_COLS if note else SignText.SIGN_COLS)


## A note in the hand: read it, write on it.
func use_note(_def: Dictionary) -> bool:
	var h: Dictionary = main.player_inv.held()
	if h.is_empty() or h["id"] != "note":
		return false
	_in_hand = true
	_target = Vector3i.MAX
	main.player.set_ui_open(true, false)
	screen.open("Note", String(h.get("text", "")), SignText.NOTE_LINES, SignText.NOTE_COLS)
	return true


func _on_done(text: String) -> void:
	if _in_hand:
		var h: Dictionary = main.player_inv.held().duplicate()
		if not h.is_empty() and h["id"] == "note":
			h["text"] = SignText.note(text)
			main.player_inv.inv.set_slot(main.player_inv.selected, h)
			main.player_inv.changed.emit()
		return
	var e: FurnitureEntity = main.blocks.furniture_at(_target)
	if e != null:
		e.set_text(text)
		refresh(e)
		Net.ent_text(e.pos, e.text)


func _on_closed() -> void:
	main.player.set_ui_open(false)


## Redraws a sign or note after its text changed.
func refresh(e: FurnitureEntity) -> void:
	if main._furniture_views.has(e.pos):
		main._furniture_views[e.pos].queue_free()
		main._furniture_views.erase(e.pos)
	main._add_furniture_view(e)


## A sign or note was just placed here (by the player: `text` comes from the note in the hand).
func furniture_placed(e: FurnitureEntity) -> void:
	if not FurnitureEntity.KINDS[e.furniture].get("write", false) or applying:
		return
	if e.furniture == "note":
		var h: Dictionary = main.player_inv.held()
		e.set_text(String(h.get("text", "")) if not h.is_empty() and h["id"] == "note" else "")
		refresh(e)
	Net.ent_put({"type": "note" if e.furniture == "note" else "sign", "pos": e.pos, "furniture": e.furniture, "facing": e.facing, "text": e.text})


func furniture_removed(e: FurnitureEntity) -> void:
	if FurnitureEntity.KINDS[e.furniture].get("write", false) and not applying:
		Net.ent_del(e.pos)


# ---------------- trading posts ----------------

func post_placed(e: TradingPostEntity) -> void:
	e.owner_name = Net.player_name if Net.active else ""
	if not applying:
		Net.ent_put({"type": "post", "pos": e.pos, "facing": e.facing, "slots": e.slots_array()})


func post_removed(e: TradingPostEntity) -> void:
	if not applying:
		Net.ent_del(e.pos)


## Right click on a post: the owner gets the stock screen (sneaking: the customer's, to try it), anyone else the offers.
func use_post(e: TradingPostEntity, sneaking: bool) -> void:
	if e.is_owner(_me()) and not sneaking:
		if Net.active:
			_asked_open = e.pos
			Net.post_open(e.pos)   # the server locks sales and answers with its slots; the screen opens then
		else:
			_show_owner(e)
		return
	main.player.set_ui_open(true, false)
	main.inv_screen.open_container(TradeView.new(e, _buy.bind(e)))
	_say("%s: click an offer to trade (pay on the left, get on the right)" % e.look_lines()[0])


func _show_owner(e: TradingPostEntity) -> void:
	_post = e
	main.player.set_ui_open(true, false)
	main.inv_screen.open_container(e)
	_say("Top row: what you give | what you want. Middle: your stock. Bottom: the payments.")


## The inventory screen closed: an owner's edits go to the server and sales may go on.
func screen_closed() -> void:
	if _post != null:
		if Net.active:
			Net.post_save(_post.pos, _post.slots_array())
		_post = null


func _buy(k: int, e: TradingPostEntity) -> void:
	var inv: Inventory = main.player_inv.inv
	if not Net.active:
		var r := e.buy_local(k, inv)
		if r["ok"]:
			main.player_inv.changed.emit()
			_say("Traded %d %s for %d %s" % [r["want"]["count"], r["want"]["id"], r["give"]["count"], r["give"]["id"]])
		else:
			_say(r["why"])
		return
	var o := e.offer(k)
	var why := TradingPostEntity.buyer_check(e.items, o, inv)
	if why == "":
		why = "The owner is busy at the stock" if e.busy else ""
	if why != "":
		_say(why)
		return
	inv.remove(o["want"]["id"], int(o["want"]["count"]))   # taken now; given back if the server refuses
	main.player_inv.changed.emit()
	_pending[Net.post_trade(e.pos, k)] = o["want"]


func _give(stacks: Array) -> void:
	for s in stacks:
		if not (s is Dictionary) or s.is_empty():
			continue
		var left: int = main.player_inv.pickup(s["id"], int(s["count"]), Inventory.data_of(s))
		if left > 0:
			var rest: Dictionary = s.duplicate(true)
			rest["count"] = left
			main.spawn_item(rest, main.player.position + Vector3(0, 1.0, 0), Vector3(0, 1.5, 0))


# ---------------- events from the server ----------------

func net_event(ev: Dictionary) -> void:
	var pos: Vector3i = ev.get("pos", Vector3i.ZERO)
	applying = true
	match String(ev["k"]):
		"put":
			_net_put(ev, pos)
		"del":
			var f: FurnitureEntity = main.blocks.furniture_at(pos)
			if f != null:
				main._remove_furniture(f, false)
			var p = main.blocks.at(pos)
			if p is TradingPostEntity:
				main.work.remove_piece(p)
		"text":
			var f2: FurnitureEntity = main.blocks.furniture_at(pos)
			if f2 != null:
				f2.set_text(String(ev.get("text", "")))
				refresh(f2)
		"post":
			var p2 = main.blocks.at(pos)
			if p2 is TradingPostEntity and (p2 != _post or ev.get("reopen", false)):
				p2.load_slots(ev.get("slots", []))
				if ev.get("reopen", false) and pos == _asked_open:
					_asked_open = Vector3i.MAX
					_show_owner(p2)
		"busy":
			var p3 = main.blocks.at(pos)
			if p3 is TradingPostEntity:
				p3.busy = bool(ev.get("busy", false))
		"res":
			_sale_result(ev)
		"deny":
			_asked_open = Vector3i.MAX
			_say(String(ev.get("why", "")))
	applying = false


func _net_put(ev: Dictionary, pos: Vector3i) -> void:
	var type := String(ev.get("type", ""))
	if type == "post":
		var e = main.blocks.at(pos)
		if not (e is TradingPostEntity):
			e = main.work._create(pos, "trading_post", GameMaterials.WORK_BLOCK)
		e.owner_name = SignText.name_of(String(ev.get("owner", "")))
		e.facing = clampi(int(ev.get("facing", 0)), 0, 3)
		e.load_slots(ev.get("slots", []))
		main.work._mark(pos)
		return
	var kind := String(ev.get("furniture", "sign"))
	if not FurnitureEntity.KINDS.has(kind) or not FurnitureEntity.KINDS[kind].get("write", false):
		return
	var f: FurnitureEntity = main.blocks.furniture_at(pos)
	if f == null:
		main._place_furniture(kind, pos, clampi(int(ev.get("facing", 0)), 0, 3))
		f = main.blocks.furniture_at(pos)
	if f != null:
		f.set_text(String(ev.get("text", "")))
		refresh(f)


## The server's answer to a sale (or one it kept for us from before a dropped link).
func _sale_result(ev: Dictionary) -> void:
	var rid := int(ev["rid"])
	if ev.get("ok", false):
		if not Net.applied.has(rid):
			_give(ev.get("give", []))
			_say("Trade done")
		_pending.erase(rid)
		Net.delivered(rid)
		main.player_inv.changed.emit()
	else:
		var pay: Dictionary = _pending.get(rid, {})
		if not pay.is_empty():
			_give([pay])   # the payment taken at the click comes back
		_pending.erase(rid)
		_say(String(ev.get("why", "The trade failed")))
