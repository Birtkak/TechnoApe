class_name ArtModels
extends RefCounted

## Hand-built cell models (game/art/README.md): data/art_models/<name>.json replaces the cells a model
## part is built from, at the part's own size so its pivot and animation stay: `work_<kind>` for a work
## piece's solid part (facing 0), `mob_<species>_<part>` for an animal part, `ape_<part>` for the player.
## `layers` go bottom to top, each `size.z` rows of `size.x` characters: "." empty, "*" keep the game's
## cell there (the boiler's fire and gauge stay live), any other character = paint PAINTS.find(ch) of
## that model's palette (WorkPalette, MobModel or ApeModel PALETTE).

const DIR := "res://data/art_models/"
const PAINTS := "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz"
const LIVE := -1

static var _models := {}   # name -> {"size": Vector3i, "cells": PackedInt32Array} or null


## The model for a name, or null when there is none or it does not parse (pushed as an error once).
static func model(name: String) -> Variant:
	if _models.has(name):
		return _models[name]
	_models[name] = null
	var path := DIR + name + ".json"
	if not FileAccess.file_exists(path):
		return null
	var d = JSON.parse_string(FileAccess.get_file_as_string(path))
	var m = parse(d) if d is Dictionary else null
	if m == null:
		push_error("art model %s: needs size [x, y, z] and size.y layers of size.z rows of size.x paint characters" % path)
	_models[name] = m
	return m


static func parse(d: Dictionary) -> Variant:
	var s: Array = d.get("size", [])
	var layers: Array = d.get("layers", [])
	if s.size() != 3 or layers.size() != int(s[1]):
		return null
	var size := Vector3i(int(s[0]), int(s[1]), int(s[2]))
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	for y in size.y:
		var rows: Array = layers[y]
		if rows.size() != size.z:
			return null
		for z in size.z:
			var r := String(rows[z])
			if r.length() != size.x:
				return null
			for x in size.x:
				var ch := r[x]
				var p := PAINTS.find(ch)
				if ch != "." and ch != "*" and p < 0:
					return null
				c[x + size.x * (y + size.y * z)] = 0 if ch == "." else (LIVE if ch == "*" else p + 1)
	return {"size": size, "cells": c}


## The cells of model part `name`: the art model's if there is one of this size (the game's cells
## filling its "*" cells), else `game` unchanged. A paint past the palette's end draws empty.
static func cells(name: String, game: PackedInt32Array, size: Vector3i, palette_size: int) -> PackedInt32Array:
	var m = model(name)
	if m == null:
		return game
	if m["size"] != size:
		push_warning("art model %s is %s, the part is %s: not used" % [name, m["size"], size])
		_models[name] = null
		return game
	var out: PackedInt32Array = m["cells"].duplicate()
	for i in out.size():
		if out[i] == LIVE:
			out[i] = game[i]
		elif out[i] > palette_size:
			out[i] = 0
	return out


## A work piece's view_parts() with its solid part from the art model `work_<kind>`, turned to its facing.
static func apply(e: RefCounted, parts: Dictionary) -> Dictionary:
	var solid: Dictionary = parts.get("solid", {})
	if solid.is_empty() or model("work_" + e.kind) == null:
		return parts
	var size: Vector3i = solid["size"]
	var q: int = e.facing if "facing" in e else 0
	var game := WorkPalette.rotate_y(solid["cells"], size, -q)
	solid = solid.duplicate()
	solid["cells"] = WorkPalette.rotate_y(cells("work_" + e.kind, game, size, WorkPalette.PALETTE.size()), size, q)
	parts = parts.duplicate()
	parts["solid"] = solid
	return parts
