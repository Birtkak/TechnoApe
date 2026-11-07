extends GdUnitTestSuite

## Signs, notes and trading posts (ADR 0164): text is cleaned at the boundary and survives a save; a trade
## moves exactly the right stacks both ways and refuses when short; the server ledger validates, dedupes a
## repeated request and keeps goods until they are acked.

var items: ItemRegistry
var recipes: RecipeRegistry


func before_test() -> void:
	items = ItemRegistry.load_default()
	recipes = RecipeRegistry.load_default(items)


func _post(owner := "ann") -> TradingPostEntity:
	var p := TradingPostEntity.new(items, Vector3i(8, 20, 8))
	p.owner_name = owner
	return p


## Offer 0: gives 4 oak planks for 2 iron ingots; 20 planks in stock.
func _stocked() -> TradingPostEntity:
	var p := _post()
	p.inv.set_slot(0, {"id": "oak_planks", "count": 4})
	p.inv.set_slot(1, {"id": "iron_ingot", "count": 2})
	p.inv.set_slot(TradingPostEntity.STOCK_FROM, {"id": "oak_planks", "count": 20})
	return p


func _buyer(planks := 0, ingots := 0) -> Inventory:
	var b := Inventory.new(items, 36)
	if planks > 0:
		b.add("oak_planks", planks)
	if ingots > 0:
		b.add("iron_ingot", ingots)
	return b


# ---------------- text ----------------

func test_text_is_stripped_and_capped() -> void:
	assert_str(SignText.sign("a" + char(1) + "b" + char(0x202E) + "c" + char(0x200B) + "d" + char(0x7F))).is_equal("abcd")
	assert_str(SignText.sign("1234567890123456789")).is_equal("123456789012345")
	assert_str(SignText.sign("a\nb\nc\nd\ne\nf")).is_equal("a\nb\nc\nd")
	assert_str(SignText.sign("hi  \r\n\r\n\r\n")).is_equal("hi")
	assert_str(SignText.note("x\ty")).is_equal("x y")
	assert_int(SignText.note("0123456789".repeat(30)).length()).is_equal(SignText.NOTE_COLS)
	assert_str(SignText.name_of("Ann\nBob")).is_equal("Ann")


func test_sign_and_note_text_save_and_load() -> void:
	var e := FurnitureEntity.new(Vector3i.ZERO, "sign")
	e.set_text("Mine ->" + char(10) + "Gold below" + char(1))
	assert_str(e.text).is_equal("Mine ->\nGold below")
	var back := FurnitureEntity.new(Vector3i.ZERO)
	back.load_from(e.to_dict())
	assert_str(back.text).is_equal("Mine ->\nGold below")
	var n := FurnitureEntity.new(Vector3i(4, 0, 0), "note")
	n.set_text("dear friend\nthe key is under the stone")
	var nb := FurnitureEntity.new(Vector3i(4, 0, 0))
	nb.load_from(n.to_dict())
	assert_str(nb.text).is_equal(n.text)
	var held: Array = nb.contents()
	assert_str(held[0]["id"]).is_equal("note")
	assert_str(held[0]["text"]).is_equal(n.text)   # coming down, the page keeps its words
	var evil := FurnitureEntity.new(Vector3i.ZERO)
	evil.load_from({"furniture": "sign", "text": "ok" + char(7) + "
1
2
3
4
5"})   # a hand-edited save is cleaned too
	assert_str(evil.text).is_equal("ok\n1\n2\n3")


func test_label_only_when_there_are_words() -> void:
	var e := FurnitureEntity.new(Vector3i.ZERO, "sign")
	assert_object(e.text_label()).is_null()
	e.set_text("Hi")
	var l := e.text_label()
	assert_str(l.text).is_equal("Hi")
	l.free()


func test_note_and_paper_craft() -> void:
	assert_str(recipes.match_grid(["paper", "charcoal", "", "", "", "", "", "", ""], "").get("id", "")).is_equal("note")
	assert_str(recipes.match_grid(["birch_bark", "birch_bark", "", "", "", "", "", "", ""], "").get("id", "")).is_equal("paper")
	assert_int(items.max_stack("note")).is_equal(1)   # each page keeps its own words, so no two ever merge


# ---------------- trading ----------------

func test_trade_moves_exactly_the_right_stacks_both_ways() -> void:
	var p := _stocked()
	var b := _buyer(0, 12)
	var r := p.buy_local(0, b)
	assert_bool(r["ok"]).is_true()
	assert_int(b.count_of("oak_planks")).is_equal(4)
	assert_int(b.count_of("iron_ingot")).is_equal(10)
	assert_int(p.stock_count("oak_planks")).is_equal(16)
	var paid := 0
	for i in range(TradingPostEntity.TAKE_FROM, TradingPostEntity.SIZE):
		var s := p.inv.get_slot(i)
		if not s.is_empty():
			assert_str(s["id"]).is_equal("iron_ingot")
			paid += int(s["count"])
	assert_int(paid).is_equal(2)
	# five more trades empty the 20 planks; the sixth finds nothing
	for i in 4:
		assert_bool(p.buy_local(0, b)["ok"]).is_true()
	assert_int(p.stock_count("oak_planks")).is_equal(0)
	assert_str(p.buy_local(0, _buyer(0, 9))["why"]).is_equal("Out of stock")


func test_trade_refuses_when_short_and_moves_nothing() -> void:
	var p := _stocked()
	var poor := _buyer(0, 1)
	var r := p.buy_local(0, poor)
	assert_bool(r["ok"]).is_false()
	assert_str(r["why"]).contains("You need 2")
	assert_int(poor.count_of("iron_ingot")).is_equal(1)
	assert_int(p.stock_count("oak_planks")).is_equal(20)
	# a full bag cannot take the goods
	var full := Inventory.new(items, 36)
	for i in 36:
		full.set_slot(i, {"id": "cobblestone", "count": 64})
	full.set_slot(0, {"id": "iron_ingot", "count": 3})   # paying leaves one, so the slot stays taken
	assert_str(p.buy_local(0, full)["why"]).is_equal("No room in your inventory")
	assert_int(full.count_of("iron_ingot")).is_equal(3)
	# an unset offer, a busy shop
	assert_str(p.buy_local(1, poor)["why"]).is_equal("That offer is not set")
	p.busy = true
	assert_str(p.buy_local(0, _buyer(0, 5))["why"]).contains("busy")


func test_full_takings_stop_sales() -> void:
	var p := _stocked()
	for i in range(TradingPostEntity.TAKE_FROM, TradingPostEntity.SIZE):
		p.inv.set_slot(i, {"id": "cobblestone", "count": 64})
	assert_str(p.check_sale(0)).is_equal("The takings are full")


func test_tools_are_not_traded() -> void:
	var p := _post()
	assert_bool(p.tradable("iron_pickaxe")).is_false()
	assert_bool(p.click_override(0, MOUSE_BUTTON_LEFT, false, {"id": "iron_pickaxe", "count": 1})).is_true()
	assert_bool(p.inv.get_slot(0).is_empty()).is_true()
	assert_bool(p.may_place(TradingPostEntity.STOCK_FROM, "iron_pickaxe")).is_false()


func test_offer_slots_set_from_the_cursor_without_taking_it() -> void:
	var p := _post()
	var cursor := {"id": "oak_planks", "count": 7}
	p.click_override(0, MOUSE_BUTTON_LEFT, false, cursor)
	assert_int(int(p.inv.get_slot(0)["count"])).is_equal(7)
	assert_int(int(cursor["count"])).is_equal(7)
	p.click_override(0, MOUSE_BUTTON_RIGHT, false, cursor)
	assert_int(int(p.inv.get_slot(0)["count"])).is_equal(8)
	p.click_override(0, MOUSE_BUTTON_LEFT, false, {})   # an empty hand clears it
	assert_bool(p.inv.get_slot(0).is_empty()).is_true()
	assert_bool(p.click_override(TradingPostEntity.STOCK_FROM, MOUSE_BUTTON_LEFT, false, cursor)).is_false()   # stock is a normal slot


func test_the_screen_can_not_pull_a_template_out_as_an_item() -> void:
	var pi := PlayerInventory.new(items, recipes)
	var p := _stocked()
	pi.open_container(p)
	pi.cursor = {"id": "oak_planks", "count": 3}
	pi.drag_collect("container", 0)
	assert_int(int(pi.cursor["count"])).is_equal(3)
	pi.wheel_move("container", 0, true)
	assert_int(pi.inv.count_of("oak_planks")).is_equal(0)
	pi.cursor = {}
	pi.click_container_slot(0, MOUSE_BUTTON_LEFT, true)   # shift-click on an offer clears it, nothing lands in the bag
	assert_int(pi.inv.count_of("oak_planks")).is_equal(0)
	pi.drag_shift("container", 1)
	assert_int(pi.inv.count_of("iron_ingot")).is_equal(0)
	p.inv.set_slot(5, {"id": "cobblestone", "count": 5})
	pi.sort_container()   # the Sort button must not shuffle the offers
	assert_str(p.inv.get_slot(5)["id"]).is_equal("cobblestone")


func test_customer_view_pays_left_gets_right_and_clicks_trade() -> void:
	var p := _stocked()
	var clicked := []
	var v := TradeView.new(p, func(k: int) -> void: clicked.append(k))
	assert_str(v.inv.get_slot(0)["id"]).is_equal("iron_ingot")
	assert_str(v.inv.get_slot(1)["id"]).is_equal("oak_planks")
	assert_bool(v.click_override(1, MOUSE_BUTTON_LEFT, false, {})).is_true()
	assert_bool(v.click_override(2, MOUSE_BUTTON_LEFT, false, {})).is_true()   # an empty pair does nothing
	assert_that(clicked).is_equal([0])


func test_post_survives_a_save() -> void:
	var store := BlockEntities.new(items, recipes)
	var p := store.create(Vector3i(8, 20, 8), "trading_post") as TradingPostEntity
	p.owner_name = "ann"
	p.inv.set_slot(0, {"id": "oak_planks", "count": 4})
	p.inv.set_slot(1, {"id": "iron_ingot", "count": 2})
	p.inv.set_slot(TradingPostEntity.STOCK_FROM, {"id": "oak_planks", "count": 20})
	var again := BlockEntities.new(items, recipes)
	assert_bool(again.load_dict(JSON.parse_string(JSON.stringify(store.to_dict())))).is_true()
	var q := again.at(Vector3i(8, 20, 8)) as TradingPostEntity
	assert_str(q.owner_name).is_equal("ann")
	assert_int(q.offer(0)["give"]["count"]).is_equal(4)
	assert_int(q.stock_count("oak_planks")).is_equal(20)
	assert_bool(q.is_owner("bob")).is_false()


# ---------------- the server ledger ----------------

func _ledger() -> PostLedger:
	var l := PostLedger.new(items)
	l.put("ann", {"type": "post", "pos": Vector3i(8, 20, 8), "facing": 1, "slots": _stocked().slots_array()})
	return l


func test_ledger_cleans_text_and_slots_from_the_wire() -> void:
	var l := PostLedger.new(items)
	l.put("ann", {"type": "sign", "pos": Vector3i(1, 2, 3), "furniture": "sign", "text": "hi" + char(1) + "





there"})
	assert_str(l.ents["1,2,3"]["text"]).is_equal("hi")
	var junk := []
	for i in 40:
		junk.append({"id": "iron_pickaxe" if i == 0 else ("no_such_item" if i == 1 else "oak_planks"), "count": 9999 if i == 2 else -3 if i == 3 else 5})
	var s := l.clean_slots(junk)
	assert_int(s.size()).is_equal(TradingPostEntity.SIZE)
	assert_bool(s[0].is_empty()).is_true()   # a tool
	assert_bool(s[1].is_empty()).is_true()   # unknown
	assert_int(int(s[2]["count"])).is_equal(items.max_stack("oak_planks"))
	assert_bool(s[3].is_empty()).is_true()


func test_ledger_sale_is_exactly_once_per_request_id() -> void:
	var l := _ledger()
	var pos := Vector3i(8, 20, 8)
	var r1: Dictionary = l.trade("bob", pos, 0, 111)
	assert_bool(r1["to"][0]["ok"]).is_true()
	assert_int(int(r1["to"][0]["give"][0]["count"])).is_equal(4)
	var r2: Dictionary = l.trade("bob", pos, 0, 111)   # a repeated request (lag): the same answer, nothing moves
	assert_bool(r2["to"][0]["ok"]).is_true()
	assert_bool(r2.has("all")).is_false()
	assert_int(l._post_of("8,20,8").stock_count("oak_planks")).is_equal(16)
	var r3: Dictionary = l.trade("bob", pos, 0, 112)   # a new request sells again
	assert_int(l._post_of("8,20,8").stock_count("oak_planks")).is_equal(12)
	assert_bool(r3["to"][0]["ok"]).is_true()
	assert_bool(l.trade("bob", pos, 7, 113)["to"][0]["ok"]).is_false()   # no such offer


func test_ledger_keeps_goods_until_acked_and_returns_them_after_a_reconnect() -> void:
	var l := _ledger()
	l.trade("bob", Vector3i(8, 20, 8), 0, 5)
	var snap := l.snapshot_for("bob")
	var late := snap.filter(func(e: Dictionary) -> bool: return e["k"] == "res")
	assert_int(late.size()).is_equal(1)
	assert_int(int(late[0]["rid"])).is_equal(5)
	assert_bool(l.snapshot_for("carl").any(func(e: Dictionary) -> bool: return e["k"] == "res")).is_false()
	l.ack("bob", 5)
	assert_bool(l.snapshot_for("bob").any(func(e: Dictionary) -> bool: return e["k"] == "res")).is_false()


func test_ledger_only_the_owner_edits_or_removes_a_post() -> void:
	var l := _ledger()
	var pos := Vector3i(8, 20, 8)
	assert_bool(l.post_open("bob", pos)["to"][0]["k"] == "deny").is_true()
	assert_bool(l.post_save("bob", pos, []).is_empty()).is_true()
	assert_bool(l.post_save("ann", pos, []).is_empty()).is_true()   # the owner too, without opening first
	assert_bool(l.remove("bob", pos)["to"][0]["k"] == "deny").is_true()
	assert_bool(l.ents.has("8,20,8")).is_true()
	assert_bool(l.put("bob", {"type": "post", "pos": pos}).is_empty()).is_true()   # nobody takes over the cell
	assert_bool(l.remove("ann", pos).has("others")).is_true()
	assert_bool(l.ents.has("8,20,8")).is_false()


func test_ledger_owner_lock_stops_sales_and_leaving_unlocks() -> void:
	var l := _ledger()
	var pos := Vector3i(8, 20, 8)
	var open: Dictionary = l.post_open("ann", pos)
	assert_bool(open["to"][0]["reopen"]).is_true()
	var r: Dictionary = l.trade("bob", pos, 0, 1)
	assert_bool(r["to"][0]["ok"]).is_false()
	assert_str(r["to"][0]["why"]).contains("busy")
	assert_int(l.left("ann").size()).is_equal(1)
	assert_bool(l.trade("bob", pos, 0, 2)["to"][0]["ok"]).is_true()
	# the owner's closing snapshot replaces the slots and ends the lock
	l.post_open("ann", pos)
	var slots := _stocked().slots_array()
	slots[TradingPostEntity.STOCK_FROM] = {"id": "oak_planks", "count": 3}
	l.post_save("ann", pos, slots)
	assert_int(l._post_of("8,20,8").stock_count("oak_planks")).is_equal(3)
	assert_bool(l.trade("bob", pos, 0, 3)["to"][0]["ok"]).is_false()   # 3 < 4: out of stock


func test_ledger_survives_a_save() -> void:
	var l := _ledger()
	l.put("ann", {"type": "note", "pos": Vector3i(2, 2, 2), "furniture": "note", "text": "hello"})
	l.trade("bob", Vector3i(8, 20, 8), 0, 9)
	var l2 := PostLedger.new(items)
	l2.load_dict(JSON.parse_string(JSON.stringify(l.to_dict())))
	assert_str(l2.ents["2,2,2"]["text"]).is_equal("hello")
	assert_int(l2._post_of("8,20,8").stock_count("oak_planks")).is_equal(16)
	assert_int(l2.snapshot_for("bob").filter(func(e: Dictionary) -> bool: return e["k"] == "res").size()).is_equal(1)
