class_name TorchLights
extends Node3D

## Real light for placed torches (ADR 0030), lit fires (firepits, furnaces, kilns, firewood piles) and
## for light sources carried or dropped (ADR 0053,
## bug report 2026-10-25 02-01-49: "like Minecraft shader packs' dynamic lights"). A pool of
## Settings.torch_lights_max shadowless OmniLight3Ds goes to the sources nearest the player within
## Settings.torch_light_radius_m, re-assigned four times a second: placed torches (TorchEntity, at
## their flame), lit block entities and dropped stacks of items with a "light" key. Range, energy and
## falloff are Settings tunables (torch_light_*, fire_light_*). The held item's light is one more
## OmniLight3D that follows the hand every frame. The flames flicker a little; flame voxels glow by
## themselves through the terrain shader's fire tile, so a torch beyond the pool still reads as lit.
##
## An item gives light with "light": {} in items.json, optionally {"range_m": 16.0, "energy": 1.3,
## "color": "#ffb86b"} (the torch's Settings values are the defaults).

const COLOR := Color(1.0, 0.72, 0.42)
const FIRE_COLOR := Color(1.0, 0.62, 0.32)
const HELD_ENERGY := 0.85                  # a share of a placed torch's: it is right at the eye
const HAND := Vector3(0.32, -0.28, -0.45)  # metres from the camera: the viewmodel's hand

var main: Node            # Main: blocks, player, player_inv, items, _item_entities
var _pool: Array[OmniLight3D] = []
var _held: OmniLight3D
var _held_id := ""
var _accum := 0.0
var stats := {"torches": 0, "lit": 0, "dropped": 0, "held": false}


func _ready() -> void:
	for i in Settings.torch_lights_max:
		var l := _light()
		add_child(l)
		_pool.append(l)
	_held = _light()
	add_child(_held)


func _light() -> OmniLight3D:
	var l := OmniLight3D.new()
	l.omni_range = Settings.m2v(Settings.torch_light_range_m)
	l.omni_attenuation = Settings.torch_light_attenuation
	l.light_color = COLOR
	l.light_energy = Settings.torch_light_energy
	l.shadow_enabled = false
	l.visible = false
	return l


## The "light" of an item id with the defaults filled in, or {} when it gives none.
static func light_of(items: ItemRegistry, id: String) -> Dictionary:
	if items == null or id == "" or not items.has(id):
		return {}
	var l = items.get_def(id).get("light")
	if not (l is Dictionary):
		return {}
	var out := {"range_m": Settings.torch_light_range_m, "energy": Settings.torch_light_energy}   # the torch's light: the defaults
	out.merge(l, true)
	return out


static func _apply(l: OmniLight3D, def: Dictionary) -> void:
	l.omni_range = Settings.m2v(float(def.get("range_m", Settings.torch_light_range_m)))
	l.light_color = Color.html(String(def["color"])) if def.has("color") else COLOR
	l.set_meta("energy", float(def.get("energy", Settings.torch_light_energy)))


func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("torches", _pt0)


func _process_timed(delta: float) -> void:
	_accum += delta
	if _accum >= 0.25:
		_accum = 0.0
		_assign()
	_follow_hand()
	var t := Time.get_ticks_msec() / 1000.0
	for i in _pool.size():
		if _pool[i].visible:
			_pool[i].light_energy = float(_pool[i].get_meta("energy", Settings.torch_light_energy)) * _flicker(t, i)
	if _held.visible:
		_held.light_energy = float(_held.get_meta("energy", HELD_ENERGY)) * _flicker(t, 31)
	if TorchEntity.flame_material != null:   # the held and dropped torch models' flames flicker with their light
		TorchEntity.flame_material.emission_energy_multiplier = 2.0 * _flicker(t, 31)


static func _flicker(t: float, i: int) -> float:
	return 0.9 + 0.06 * sin(t * 11.0 + i * 1.7) + 0.04 * sin(t * 23.0 + i)


func _follow_hand() -> void:
	var id := ""
	if main.player_inv != null and main.player != null and main.player.camera != null:
		id = String(main.player_inv.held().get("id", ""))
	var def := light_of(main.items, id)
	_held.visible = not def.is_empty()
	stats["held"] = _held.visible
	if not _held.visible:
		return
	if id != _held_id:
		_held_id = id
		_apply(_held, def)
		_held.set_meta("energy", float(def["energy"]) * HELD_ENERGY)
	var cam: Transform3D = main.player.camera.global_transform
	_held.global_position = cam * (HAND / Settings.VOXEL_M)


func _assign() -> void:
	var torches: Array = main.blocks.all_of("torch")
	var eye: Vector3 = main.player.position
	var r2 := Settings.m2v(Settings.torch_light_radius_m) ** 2
	var near: Array = []
	for e in torches:
		var at: Vector3 = e.flame()
		var d2 := at.distance_squared_to(eye)
		if d2 <= r2:
			near.append([d2, at, {}])
	var fire := {"range_m": Settings.fire_light_range_m, "energy": Settings.fire_light_energy, "color": FIRE_COLOR.to_html(false)}
	for e in main.blocks.lit():
		var at := Vector3(e.pos) + Vector3(Settings.BLOCK * 0.5, Settings.m2v(0.4), Settings.BLOCK * 0.5)
		var d2 := at.distance_squared_to(eye)
		if d2 <= r2:
			near.append([d2, at, fire])
	var dropped := 0
	for it in main._item_entities:
		if not is_instance_valid(it):
			continue
		var def := light_of(main.items, String(it.stack.get("id", "")))
		if def.is_empty():
			continue
		var at: Vector3 = it.global_position + Vector3(0, Settings.m2v(0.25), 0)
		var d2 := at.distance_squared_to(eye)
		if d2 <= r2:
			near.append([d2, at, def])
			dropped += 1
	near.sort_custom(func(a: Array, b: Array) -> bool: return a[0] < b[0])
	for i in _pool.size():
		_pool[i].visible = i < near.size()
		if i < near.size():
			_pool[i].position = near[i][1]
			_apply(_pool[i], near[i][2])
	stats["torches"] = torches.size()
	stats["dropped"] = dropped
	stats["lit"] = mini(near.size(), _pool.size())
