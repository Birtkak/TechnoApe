class_name Firedamp
extends Node

## Firedamp (ADR 0148): the explosive gas sealed in coal lumps. The sim burns it (a fast flash: fire_hp 1, every
## attempt lights) and tallies the burning cells (SimWorld::TALLY_GAS_BURN); a tick with enough of them burning
## is a blast at their centre through Tnt.explode (the crater, the rigid piece check, damage, drops), after the
## gas round it is vacuumed so one pocket makes one blast. Torches are not fire cells: a few times a second the
## lights round the player (TorchLights.sources, the held torch too) get a cell of fire where firedamp touches
## them, unless the item is a safety lamp ("safe_light"). The safety lamp held reads the gas instead: its light
## flares and turns blue with the firedamp sampled round the eye (never a fire cell, so it never lights it).

const TALLY_GAS_MOVE := 6   # SimWorld::TALLY_GAS_MOVE: firedamp cells moving (the hiss of a pocket opening)
const TALLY_GAS_BURN := 7   # SimWorld::TALLY_GAS_BURN: firedamp cells burning (the flash)
const FLARE := Color(0.55, 0.75, 1.0)   # the lamp's flame in firedamp

var main: Node
var stats := {"blasts": 0, "flash_max": 0, "lit_by_light": 0, "lamp_level": 0}
var _cooldown := 0.0
var _accum := 0.0
var _mask := PackedByteArray()
var _seed := 1
var _warned := false


func setup(p_main: Node) -> void:
	main = p_main
	_mask.resize(256)
	_mask[GameMaterials.FIREDAMP] = 1


## The blast share (1 = a whole TNT block) of `cells` burning in one tick.
static func blast_share(cells: int) -> float:
	return clampf(float(cells) / float(Settings.firedamp_full_cells), 0.25, 1.5)


func _process(delta: float) -> void:
	if main == null or main.bubble == null:
		return
	_cooldown = maxf(0.0, _cooldown - delta)
	var f: Array = main.bubble.falls(TALLY_GAS_BURN)
	stats["flash_max"] = maxi(int(stats["flash_max"]), int(f[0]))
	if int(f[0]) >= Settings.firedamp_blast_cells and _cooldown <= 0.0 and main.tnt != null:
		_cooldown = 1.0
		var at: Vector3 = f[1]
		main.bubble.blast(at, Settings.m2v(Settings.firedamp_vacuum_m), GameMaterials.AIR, PackedInt32Array([GameMaterials.FIREDAMP, GameMaterials.FIRE]))
		main.tnt.explode(at, blast_share(int(f[0])))
		stats["blasts"] += 1
	_accum += delta
	if _accum < 0.25:
		return
	_accum = 0.0
	_light_lamp()
	_ignite_at_lights()


## The held safety lamp: hits of firedamp round the eye set its flare (TorchLights.tint_held) and a one-time warning.
func _light_lamp() -> void:
	var tl = main.get("torch_lights")
	if tl == null or main.player == null or main.player_inv == null:
		return
	var held := String(main.player_inv.held().get("id", ""))
	if not TorchLights.light_of(main.items, held).get("safe_light", false):
		held = String(main.player_inv.offhand().get("id", ""))
	if not TorchLights.light_of(main.items, held).get("safe_light", false):
		stats["lamp_level"] = 0
		_warned = false
		return
	var eye := Vector3i(main.player.position.floor()) + Vector3i(0, int(Settings.m2v(1.5)), 0)
	var hits: int = main.bubble.sample_cells(eye, int(Settings.m2v(Settings.safety_lamp_reach_m)), 96, _seed, _mask).size() / 7
	_seed += 1
	stats["lamp_level"] = hits
	var t := clampf(hits / 6.0, 0.0, 1.0)
	tl.tint_held(Color(1, 1, 1).lerp(FLARE, t), 1.0 + 1.5 * t)
	if hits > 0:   # the gauze sings: a crackle that climbs in pitch and loudness with the gas
		Sound.play("lamp/flare", Vector3.INF, {"pitch": 0.8 + 0.8 * t, "db": -6.0 + 8.0 * t})
	if hits >= 2 and not _warned and main.game_hud != null:
		_warned = true
		main.game_hud.show_toast("The safety lamp flares blue: firedamp!")
	elif hits == 0:
		_warned = false


## Open flames near the player light firedamp they touch: one fire cell where a sample hits.
func _ignite_at_lights() -> void:
	var tl = main.get("torch_lights")
	if tl == null or main.player == null or not main.bubble.can_sample():
		return
	var r := int(Settings.m2v(0.5))
	for s in tl.sources(main.player.position, Settings.m2v(Settings.torch_light_radius_m) ** 2):
		var def: Dictionary = s[2]
		if def.get("safe_light", false):
			continue
		var hits: PackedInt32Array = main.bubble.sample_cells(Vector3i((s[1] as Vector3).floor()), r, 8, _seed, _mask)
		_seed += 1
		if hits.size() >= 7:
			main.bubble.set_voxel(Vector3i(hits[0], hits[1], hits[2]), GameMaterials.FIRE)
			stats["lit_by_light"] += 1
	var held := String(main.player_inv.held().get("id", "")) if main.player_inv != null else ""
	var def := TorchLights.light_of(main.items, held)
	if not def.is_empty() and not def.get("safe_light", false):
		var hand: Vector3i = Vector3i(main.player.position.floor()) + Vector3i(0, int(Settings.m2v(1.3)), 0)
		var hits: PackedInt32Array = main.bubble.sample_cells(hand, r * 2, 8, _seed, _mask)
		_seed += 1
		if hits.size() >= 7:
			main.bubble.set_voxel(Vector3i(hits[0], hits[1], hits[2]), GameMaterials.FIRE)
			stats["lit_by_light"] += 1
