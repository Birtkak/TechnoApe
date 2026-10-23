class_name TorchLights
extends Node3D

## Real light for placed torches (ADR 0030). A pool of Settings.torch_lights_max
## shadowless OmniLight3Ds goes to the torches nearest the player within
## Settings.torch_light_radius_m, re-assigned four times a second from the
## block-entity list (every torch is a TorchEntity). The flames flicker a
## little; the flame cells glow by themselves through the terrain shader's
## fire tile, so a torch beyond the pool still reads as lit.

const COLOR := Color(1.0, 0.72, 0.42)
const ENERGY := 1.5

var main: Node            # Main: blocks, player
var _pool: Array[OmniLight3D] = []
var _accum := 0.0
var stats := {"torches": 0, "lit": 0}


func _ready() -> void:
	for i in Settings.torch_lights_max:
		var l := OmniLight3D.new()
		l.omni_range = Settings.m2v(Settings.torch_light_range_m)
		l.omni_attenuation = 1.4
		l.light_color = COLOR
		l.light_energy = ENERGY
		l.shadow_enabled = false
		l.visible = false
		add_child(l)
		_pool.append(l)


func _process(delta: float) -> void:
	_accum += delta
	if _accum >= 0.25:
		_accum = 0.0
		_assign()
	var t := Time.get_ticks_msec() / 1000.0
	for i in _pool.size():
		if _pool[i].visible:
			_pool[i].light_energy = ENERGY * (0.9 + 0.06 * sin(t * 11.0 + i * 1.7) + 0.04 * sin(t * 23.0 + i))


func _assign() -> void:
	var torches: Array = main.blocks.all_of("torch")
	var eye: Vector3 = main.player.position
	var r2 := Settings.m2v(Settings.torch_light_radius_m) ** 2
	var near: Array = []
	for e in torches:
		var at := Vector3(e.pos) + Vector3(0.5, 0.85, 0.5)
		var d2 := at.distance_squared_to(eye)
		if d2 <= r2:
			near.append([d2, at])
	near.sort_custom(func(a: Array, b: Array) -> bool: return a[0] < b[0])
	for i in _pool.size():
		_pool[i].visible = i < near.size()
		if i < near.size():
			_pool[i].position = near[i][1]
	stats["torches"] = torches.size()
	stats["lit"] = mini(near.size(), _pool.size())
