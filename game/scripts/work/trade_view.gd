class_name TradeView
extends RefCounted

## What a customer sees of a trading post (ADR 0164): the inventory screen's container layout with the
## post's offers as read-only pairs, "you pay" on the left, "you get" on the right. A click on a pair
## is one trade (`buy` is called with the offer number). Nothing here can be taken out or put in.

signal changed

var kind := "trade_view"
var title := "Trading Post"
var pos: Vector3i
var inv: Inventory
var post: TradingPostEntity
var buy: Callable            # (k: int) -> void


func _init(p_post: TradingPostEntity, p_buy: Callable) -> void:
	post = p_post
	pos = p_post.pos
	buy = p_buy
	inv = Inventory.new(p_post.items, TradingPostEntity.OFFERS * 2)
	title = "%s's Trading Post" % p_post.owner_name if p_post.owner_name != "" else "Trading Post"
	refresh()
	post.changed.connect(refresh)


## Re-reads the offers (the post's slots changed).
func refresh() -> void:
	for k in TradingPostEntity.OFFERS:
		var o := post.offer(k)
		inv.set_slot(k * 2, {} if o.is_empty() else o["want"])
		inv.set_slot(k * 2 + 1, {} if o.is_empty() else o["give"])
	changed.emit()


func is_template(_i: int) -> bool:
	return true


func may_place(_slot: int, _id: String) -> bool:
	return false


func is_output(_slot: int) -> bool:
	return false


func shift_targets(_id: String) -> Array:
	return []


func shift_out_reverse(_slot: int) -> bool:
	return false


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func contents() -> Array:
	return []


func slot_positions() -> Array:
	var out := []
	for i in inv.size():
		var k := i / 2
		out.append(Vector2(30 + (k % 2) * 160 + (i % 2) * 52, 50 + (k / 2) * 52))
	return out


func click_override(i: int, _button: int, _shift: bool, _cursor: Dictionary) -> bool:
	if buy.is_valid() and i < inv.size() and not inv.get_slot(i).is_empty():
		buy.call(i / 2)
	return true
