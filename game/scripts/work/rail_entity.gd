class_name RailEntity
extends RefCounted

## A rail (ADR 0100): a flat piece along one axis that joins the rails next to it, straight on or
## round a corner, or a **slope** (`rise`: the block step it climbs along; a wrench cycles flat, up
## one way, up the other) that joins the rail one block higher at its top. A cart on a rail is the
## rail's: `cart` ("" none, "minecart" to ride, "chest_cart" with CARGO slots of cargo). A ridden
## minecart is moved by Player.ride; a chest cart rolls by itself once pushed (`rolling`, a block
## step, and `t` along it) at minecart_speed_m until the line ends, and hoppers load and empty it.

signal changed

const CARGO := 9

var kind := "rail"
var pos: Vector3i
var axis := 0   # 0 along z, 1 along x (like a pipe)
var rise := Vector3i.ZERO   # a slope climbs this block step (ZERO: flat)
var cart := ""
var cargo: Inventory
var rolling := Vector3i.ZERO
var t := 0.5
var _store: BlockEntities


func _init(p_store: BlockEntities, p_pos: Vector3i) -> void:
	_store = p_store
	pos = p_pos
	cargo = Inventory.new(p_store.items, CARGO)
	cargo.changed.connect(func(_i: int) -> void: changed.emit())


func is_lit() -> bool:
	return rolling != Vector3i.ZERO   # a rolling cart: BlockEntities.tick redraws it


## The wrench: flat along z, up +z, up -z, flat along x, up +x, up -x, round again.
func turn() -> void:
	var n := Settings.BLOCK
	var states := [[0, Vector3i.ZERO], [0, Vector3i(0, 0, n)], [0, Vector3i(0, 0, -n)], [1, Vector3i.ZERO], [1, Vector3i(n, 0, 0)], [1, Vector3i(-n, 0, 0)]]
	var i := 0
	for k in states.size():
		if states[k][0] == axis and states[k][1] == rise:
			i = k
	axis = states[(i + 1) % states.size()][0]
	rise = states[(i + 1) % states.size()][1]
	changed.emit()


## The rail reached from `from` going `dir` (a block step): straight on at this level, up the slope
## this rail climbs, down onto a slope whose top meets this level, else round either corner on the
## flat. Returns {rail, dir}, {} at the end of the line.
static func next_of(store: BlockEntities, from: RailEntity, dir: Vector3i) -> Dictionary:
	var n := Settings.BLOCK
	var up := Vector3i(0, n, 0)
	var steps := [from.pos + dir + (up if from.rise == dir else Vector3i.ZERO)]
	if from.rise == Vector3i.ZERO:
		steps.append(from.pos + dir - up)   # a slope below whose top meets us
	for p in steps:
		var e: RefCounted = store.at(p)
		if e is RailEntity and (e.pos.y == from.pos.y + (n if from.rise == dir else 0) or e.rise == -dir):
			return {"rail": e, "dir": dir}
	if from.rise == Vector3i.ZERO:
		var corners := [Vector3i(0, 0, n), Vector3i(0, 0, -n)] if dir.x != 0 else [Vector3i(n, 0, 0), Vector3i(-n, 0, 0)]
		for d in corners:
			var e: RefCounted = store.at(from.pos + d)
			if e is RailEntity and e.rise == Vector3i.ZERO:
				return {"rail": e, "dir": d}
	return {}


## How high above the rail's floor a cart is at `t` along `dir`: climbing a slope, descending it, or flat.
static func height_on(rail: RailEntity, dir: Vector3i, at: float) -> float:
	if rail.rise == Vector3i.ZERO:
		return 0.0
	return Settings.BLOCK * (at if rail.rise == dir else 1.0 - at)


## The chest cart's own motion: along `rolling` at minecart_speed_m, rail to rail, until the line ends.
func tick() -> bool:
	if cart != "chest_cart" or rolling == Vector3i.ZERO:
		return false
	t += Settings.m2v(Settings.minecart_speed_m) * BlockEntities.TICK_S / Settings.BLOCK
	if t >= 1.0:
		var nxt := next_of(_store, self, rolling)
		if nxt.is_empty():
			t = 1.0
			rolling = Vector3i.ZERO   # the end of the line: hoppers do the rest
		else:
			var to: RailEntity = nxt["rail"]
			to.cart = cart
			to.rolling = nxt["dir"]
			to.t = t - 1.0
			for i in CARGO:
				to.cargo.set_slot(i, cargo.get_slot(i))
				cargo.set_slot(i, {})
			cart = ""
			rolling = Vector3i.ZERO
			t = 0.5
			to.changed.emit()
	changed.emit()
	return false


func contents() -> Array:
	var out := [{"id": "rail", "count": 1}]
	if cart != "":
		out.append({"id": cart, "count": 1})
	for s in cargo.to_array():
		if not s.is_empty():
			out.append(s)
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	match cart:
		"minecart":
			out.append("A minecart: right click to ride, sneak and right click to take it")
		"chest_cart":
			out.append("A chest cart%s: items go in, an empty hand pushes it, sneak and right click takes it" % (" (rolling)" if rolling != Vector3i.ZERO else ""))
			for s in cargo.to_array():
				if not s.is_empty():
					out.append("%d %s" % [s["count"], _store.items.display_name(s["id"])])
		_:
			out.append("Sloped rail" if rise != Vector3i.ZERO else "Rail")
	return out


## Two iron rails on wooden sleepers, stepping up a slope; with a cart on it, an iron tub on four
## wheels (a chest cart has a wooden lid), drawn where the cart is along the rail.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 12, 8)
	var c := WorkPalette.new_cells(size)
	var slope := rise != Vector3i.ZERO
	for z in 8:   # the slope is drawn climbing +z, then turned to its rise
		var y := z if slope else 0
		if z % 3 == 1:
			WorkPalette.box(c, size, Vector3i(0, y, z), Vector3i(8, y + 1, z + 1), WorkPalette.WOOD_DARK)   # the sleepers
		WorkPalette.box(c, size, Vector3i(1, y + 1, z), Vector3i(2, y + 2, z + 1), WorkPalette.IRON_DARK)
		WorkPalette.box(c, size, Vector3i(6, y + 1, z), Vector3i(7, y + 2, z + 1), WorkPalette.IRON_DARK)
	if cart != "":
		var cz := 0   # the cart's cell along the rail (a rolling chest cart moves along it)
		if rolling != Vector3i.ZERO:
			cz = clampi(int(t * 8.0) - 4, -3, 3)
		var cy := (4 if cart == "chest_cart" and slope else 0) + (int(t * 8.0) if slope and rolling == rise else 0)
		cy = mini(cy, 7)
		for q in [Vector3i(1, 1, 1), Vector3i(6, 1, 1), Vector3i(1, 1, 6), Vector3i(6, 1, 6)]:
			WorkPalette.box(c, size, q + Vector3i(0, cy, 0), q + Vector3i(1, cy + 2, 1), WorkPalette.ANVIL)   # the wheels
		WorkPalette.wall_box(c, size, Vector3i(1, cy + 3, 0), Vector3i(7, cy + 6, 8), [WorkPalette.IRON_DARK], WorkPalette.IRON_DARK)
		WorkPalette.box(c, size, Vector3i(1, cy + 3, 0), Vector3i(7, cy + 4, 8), WorkPalette.IRON_DARK)   # the tub floor
		if cart == "chest_cart":
			WorkPalette.box(c, size, Vector3i(1, cy + 6, 0), Vector3i(7, cy + 7, 8), WorkPalette.WOOD)   # the lid
	var quarters := axis
	if slope:
		quarters = Transport.DIRS.find(rise / Settings.BLOCK)   # +z is 2, -z 0, +x 1, -x 3
		quarters = (quarters + 2) % 4   # drawn climbing +z
	return {"solid": {"cells": WorkPalette.rotate_y(c, size, quarters), "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "axis": axis, "rise": [rise.x, rise.y, rise.z], "cart": cart,
		"cargo": cargo.to_array(), "rolling": [rolling.x, rolling.y, rolling.z], "t": t}


func load_from(d: Dictionary) -> void:
	axis = int(d.get("axis", 0))
	var r: Array = d.get("rise", [0, 0, 0])
	rise = Vector3i(int(r[0]), int(r[1]), int(r[2])) if r.size() == 3 else Vector3i.ZERO
	var c = d.get("cart", "")
	cart = ("minecart" if c else "") if c is bool else String(c)
	var slots: Array = d.get("cargo", [])
	for i in mini(slots.size(), CARGO):
		cargo.set_slot(i, slots[i] if slots[i] is Dictionary else {})
	var ro: Array = d.get("rolling", [0, 0, 0])
	rolling = Vector3i(int(ro[0]), int(ro[1]), int(ro[2])) if ro.size() == 3 else Vector3i.ZERO
	t = float(d.get("t", 0.5))


func summary() -> Dictionary:
	return {"kind": kind, "axis": axis, "slope": rise != Vector3i.ZERO, "cart": cart, "rolling": rolling != Vector3i.ZERO}
