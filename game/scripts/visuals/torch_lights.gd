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
const LAVA_COLOR := Color(1.0, 0.45, 0.15)
const LAVA_TTL_S := 15.0                   # a lava spot not sampled again for this long is dropped (quenched, dug out)
const LAVA_SAMPLES := 4096                 # SimBubble.sample_cells per assign; a pool collects hits over a few seconds
const LAVA_SAMPLE_M := 24.0
const HELD_ENERGY := 0.85                  # a share of a placed torch's: it is right at the eye
const HAND := Vector3(0.32, -0.28, -0.45)  # metres from the camera: the viewmodel's hand

var main: Node            # Main: blocks, player, player_inv, items, _item_entities
var _pool: Array[OmniLight3D] = []
var _held: OmniLight3D
var _held_id := ""
var _held_base := COLOR   # the held light's own colour; tint_held (the safety lamp in firedamp, ADR 0148) scales it
var _held_boost := 1.0
var _accum := 0.0
var stats := {"torches": 0, "lit": 0, "dropped": 0, "held": false, "lava": 0}
var _lava := {}           # grid cell -> [expires_s, light position]: lava found by random sampling
var _lava_mask := PackedByteArray()
var _lava_seed := 1


func _ready() -> void:
	_lava_mask.resize(256)
	_lava_mask[GameMaterials.LAVA] = 1
	_lava_mask[GameMaterials.LAVA_STATIC] = 1
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
		_held.light_energy = float(_held.get_meta("energy", HELD_ENERGY)) * _flicker(t, 31) * _held_boost * (1.0 + 0.3 * (_held_boost - 1.0) * sin(t * 16.0))   # a flaring lamp pulses (ADR 0148)
	if TorchEntity.flame_material != null:   # the held and dropped torch models' flames flicker with their light
		TorchEntity.flame_material.emission_energy_multiplier = 2.0 * _flicker(t, 31)


## The held light's flare (ADR 0148): its colour times `tint`, its energy times `boost`, until the item changes.
func tint_held(tint: Color, boost: float) -> void:
	_held.light_color = _held_base * tint
	_held_boost = boost


static func _flicker(t: float, i: int) -> float:
	return 0.9 + 0.06 * sin(t * 11.0 + i * 1.7) + 0.04 * sin(t * 23.0 + i)


func _follow_hand() -> void:
	var id := ""
	var hand := HAND
	if main.player_inv != null and main.player != null and main.player.camera != null:
		id = String(main.player_inv.held().get("id", ""))
		if light_of(main.items, id).is_empty():   # a torch in the left hand lights too (bug 2026-10-27_15-41-51)
			id = String(main.player_inv.offhand().get("id", ""))
			hand.x = -hand.x
		if light_of(main.items, id).is_empty() and main.get("stone_age") != null:   # a miner's helmet lights hands-free (ADR 0099)
			id = String(main.stone_age.clothing.worn.get("head", {}).get("id", ""))
			hand = Vector3(0.0, 0.15, -0.1)
	var def := light_of(main.items, id)
	_held.visible = not def.is_empty()
	stats["held"] = _held.visible
	if not _held.visible:
		return
	if id != _held_id:
		_held_id = id
		_apply(_held, def)
		_held.set_meta("energy", float(def["energy"]) * HELD_ENERGY)
		_held_base = _held.light_color
		_held_boost = 1.0
	var cam: Transform3D = main.player.camera.global_transform
	_held.global_position = cam * (hand / Settings.VOXEL_M)


func _assign() -> void:
	var eye: Vector3 = main.player.position
	var near := sources(eye, Settings.m2v(Settings.torch_light_radius_m) ** 2)
	near.append_array(_lava_lights(eye, Time.get_ticks_msec() / 1000.0))
	near.sort_custom(func(a: Array, b: Array) -> bool: return a[0] < b[0])
	for i in _pool.size():
		_pool[i].visible = i < near.size()
		if i < near.size():
			_pool[i].position = near[i][1]
			_apply(_pool[i], near[i][2])
	stats["lit"] = mini(near.size(), _pool.size())


## The placed light sources within sqrt(r2) voxels of `eye` as [d2, position, light def ({} = a torch's)]:
## torches, lit fires, lanterns and dropped light items (no lava). The light overlay reads them too (ADR 0130).
func sources(eye: Vector3, r2: float) -> Array:
	var torches: Array = main.blocks.all_of("torch")
	var near: Array = []
	for e in torches:
		var at: Vector3 = e.flame()
		var d2 := at.distance_squared_to(eye)
		if d2 <= r2:
			near.append([d2, at, {}])
	var fire := {"range_m": Settings.fire_light_range_m, "energy": Settings.fire_light_energy, "color": FIRE_COLOR.to_html(false)}
	var lamp := {"range_m": Settings.lamp_light_range_m, "energy": Settings.lamp_light_energy, "color": "fff1c8", "safe_light": true}   # ADR 0117: an electric lamp, paler than fire; no flame (ADR 0148)
	for e in main.blocks.lit():
		var w := BlockEntities.width_of(e.kind) * 0.5
		var is_lamp := e is ElectricLampEntity
		var at := Vector3(e.pos) + Vector3(w, Settings.m2v(1.2 if is_lamp else 0.4), w)
		var d2 := at.distance_squared_to(eye)
		if d2 <= r2:
			near.append([d2, at, lamp if is_lamp else fire])
	for e in main.blocks.all_furniture():   # placed lanterns (ADR 0094): furniture whose item has "light"
		var ldef := light_of(main.items, e.item_id())
		if not ldef.is_empty():
			var at: Vector3 = e.light_at()
			var d2 := at.distance_squared_to(eye)
			if d2 <= r2:
				near.append([d2, at, ldef])
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
	stats["torches"] = torches.size()
	stats["dropped"] = dropped
	return near


## Light candidates [d2, at, def] for the lava nearest `eye`, after sampling the bubble for more (ADR 0092 follow-up:
## lava glowed but lit nothing around it). Random sampling, like the ambient particles: a pool is found within
## a second or two and each spot stays lit while sampling keeps hitting it.
func _lava_lights(eye: Vector3, now: float) -> Array:
	var bubble: SimBubble = main.bubble
	if bubble != null and bubble.can_sample():
		_lava_seed += 1
		note_lava(bubble.sample_cells(Vector3i(eye.floor()), int(Settings.m2v(LAVA_SAMPLE_M)), LAVA_SAMPLES, _lava_seed, _lava_mask), now)
	var def := {"range_m": Settings.lava_light_range_m, "energy": Settings.lava_light_energy, "color": LAVA_COLOR.to_html(false)}
	var out: Array = []
	for k in _lava.keys():
		if _lava[k][0] < now:
			_lava.erase(k)
			continue
		var at: Vector3 = _lava[k][1]
		out.append([at.distance_squared_to(eye), at, def])
	out.sort_custom(func(a: Array, b: Array) -> bool: return a[0] < b[0])
	out.resize(mini(out.size(), Settings.lava_lights_max))
	stats["lava"] = _lava.size()
	return out


## Folds SimBubble.sample_cells hits (7 ints each) into lava spots: one per lava_light_grid_m cell, lit half a
## metre over the highest lava cell sampled in it. Burning cells come back too and are skipped.
func note_lava(hits: PackedInt32Array, now: float) -> void:
	var g := Settings.m2v(Settings.lava_light_grid_m)
	for i in range(0, hits.size(), 7):
		var m := hits[i + 3]
		if m != GameMaterials.LAVA and m != GameMaterials.LAVA_STATIC:
			continue
		var at := Vector3(hits[i] + 0.5, hits[i + 1] + 1.0 + Settings.m2v(0.5), hits[i + 2] + 0.5)
		var k := Vector3i((at / g).floor())
		k.y = 0   # one light per column: the top of a deep pool, not every layer of it
		var old: Array = _lava.get(k, [])
		_lava[k] = [now + LAVA_TTL_S, at if old.is_empty() or at.y > old[1].y else old[1]]
