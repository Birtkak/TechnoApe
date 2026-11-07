class_name BlastDebris
extends Node3D

## Flung debris (ADR 0150): a blast throws a few of the voxels it removed out as small tumbling chunks that fly
## ballistically and, where they land, turn back into voxels: rock as gravel, grass as dirt, the rest as itself.
## The matter comes out of what the blast removed and did not drop (not extra), at most
## Settings.tnt_debris_max chunks a blast and Settings.tnt_debris_live_max in the air, so a chain stays cheap.
## Each chunk is the primed TNT box (warmed under the loading screen) scaled down, with its material's tile:
## same shader, no new pipeline.

const SIDE := 2                 # a chunk is SIDE^3 voxels
const VOXELS := SIDE * SIDE * SIDE
const REMOVED_PER_CHUNK := 64   # a blast that removed fewer voxels than this per chunk throws fewer
const SPEED_MPS := Vector2(5.0, 13.0)
const MAX_FLIGHT_S := 5.0       # lands wherever it is after this (flew out of the loaded world)
const SPIN := 9.0               # rad/s at most
const HURT_SPEED := 24.0        # voxels/s (3 m/s): slower chunks only bump
const HIT_M := 0.5              # a chunk this close to a body's middle hits it

var main: Node
var stats := {"thrown": 0, "landed": 0, "voxels": 0, "live_max": 0}
var _base: StandardMaterial3D
var _box: Mesh
var _atlas: Image
var _mats := {}                 # material id -> StandardMaterial3D
var _live: Array[Dictionary] = []   # {node, mat (voxel id), v, spin, t}
var _rng := RandomNumberGenerator.new()


func setup(p_main: Node, base: StandardMaterial3D, box: Mesh) -> void:
	main = p_main
	_base = base
	_box = box
	_rng.randomize()
	var tex: Texture2D = load("res://assets/textures/atlas.png")
	if tex:
		_atlas = tex.get_image()   # once, at load: a decompress mid-game would cost a frame
		if _atlas.is_compressed():
			_atlas.decompress()


## What a chunk of material `m` leaves where it lands: loose ground the sim lets settle.
static func loose_of(m: int) -> int:
	if GameMaterials.is_grass(m) or GameMaterials.is_soil(m):
		return GameMaterials.DIRT
	return GameMaterials.GRAVEL if Tnt.shell_factor(m) < 1.0 else m   # rock (stone shell and harder) breaks to gravel


## A blast at `pos` (voxels) of radius `r` that removed `removed` voxels, `drops` the sampled estimate
## {material name: voxels}: throws chunks of those materials out of the crater. Returns how many.
func throw(pos: Vector3, r: float, drops: Dictionary, removed: int) -> int:
	var names: Array = []
	var weights := PackedFloat32Array()
	for name in drops:
		var m := GameMaterials.id_of(String(name))
		if m <= 0 or m == GameMaterials.TNT or m == GameMaterials.FUSE_CORD or not GameMaterials.is_plain_voxel(m):
			continue
		names.append(m)
		weights.append(float(drops[name]))
	var n := mini(mini(Settings.tnt_debris_max, Settings.tnt_debris_live_max - _live.size()), removed / REMOVED_PER_CHUNK)
	if names.is_empty() or n <= 0:
		return 0
	for i in n:
		var m: int = names[_rng.rand_weighted(weights)]
		var dir := Vector3(_rng.randf_range(-1, 1), _rng.randf_range(0.35, 1.0), _rng.randf_range(-1, 1)).normalized()
		var mi := MeshInstance3D.new()
		mi.mesh = _box
		mi.material_override = _material(m)
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF   # a shadow pass would compile a pipeline mid-game
		mi.scale = Vector3.ONE * float(SIDE) / Settings.BLOCK
		add_child(mi)
		mi.global_position = pos + dir * r * 0.5
		var spin := Vector3(_rng.randf_range(-1, 1), _rng.randf_range(-1, 1), _rng.randf_range(-1, 1)) * SPIN
		_live.append({"node": mi, "mat": m, "v": dir * Settings.m2v(_rng.randf_range(SPEED_MPS.x, SPEED_MPS.y)), "spin": spin, "t": 0.0})
	stats["thrown"] += n
	stats["live_max"] = maxi(stats["live_max"], _live.size())
	return n


func _material(m: int) -> StandardMaterial3D:
	if _mats.has(m):
		return _mats[m]
	var mat: StandardMaterial3D = _base.duplicate()
	mat.emission_energy_multiplier = 0.0
	var tiles: Array = GameMaterials.defs()[m - 1].get("tiles", [0, 0, 0])
	if _atlas:
		var tw := _atlas.get_width() / GameMaterials.ATLAS_TILES.x
		var tile := int(tiles[1])
		var rect := Rect2i((tile % GameMaterials.ATLAS_TILES.x) * tw, (tile / GameMaterials.ATLAS_TILES.x) * tw, tw, tw)
		mat.albedo_texture = ImageTexture.create_from_image(_atlas.get_region(rect))
	_mats[m] = mat
	return mat


func _process(delta: float) -> void:
	if _live.is_empty() or main == null:
		return
	var g := Settings.m2v(Settings.gravity_m)
	var landed: Array[Dictionary] = []
	for d in _live:
		var node: Node3D = d["node"]
		var v: Vector3 = d["v"]
		v.y = maxf(v.y - g * delta, -Settings.m2v(40.0))
		d["v"] = v
		d["t"] = float(d["t"]) + delta
		var next := node.global_position + v * delta
		if _solid(next) or float(d["t"]) > MAX_FLIGHT_S or (v.length() > HURT_SPEED and _hits(next, v)):
			landed.append(d)
			continue
		node.global_position = next
		node.rotation += (d["spin"] as Vector3) * delta
	if landed.is_empty():
		return
	Net.recording = true   # ADR 0145: the landed voxels reach the other players
	for d in landed:
		_land(d)
		_live.erase(d)
	Net.recording = false


## The chunk turns into up to SIDE^3 loose voxels in the air cells where it stopped.
func _land(d: Dictionary) -> void:
	var node: Node3D = d["node"]
	var at := Vector3i((node.global_position - Vector3.ONE * SIDE * 0.5).round())
	var m := loose_of(int(d["mat"]))
	for y in SIDE:
		for z in SIDE:
			for x in SIDE:
				var c := at + Vector3i(x, y, z)
				if main.get_world_voxel(c) == GameMaterials.AIR:
					main.set_world_voxel(c, m)
					stats["voxels"] += 1
	stats["landed"] += 1
	if main.get("fx"):
		main.fx.block_break(at, m)
	node.queue_free()


## A chunk at `p` flying at `v` hits the player or a mob there: it takes Settings.tnt_debris_damage and a shove,
## and the chunk drops where it is.
func _hits(p: Vector3, v: Vector3) -> bool:
	var reach := Settings.m2v(HIT_M)
	var pl = main.get("player")
	if pl != null and GameMode.rule(pl.game_mode, "interacts"):
		var centre: Vector3 = pl.position + Vector3(0, Settings.m2v(Settings.player_height_m) * 0.5, 0)
		if absf(p.y - centre.y) < Settings.m2v(Settings.player_height_m) * 0.5 and Vector2(p.x - centre.x, p.z - centre.z).length() < reach:
			pl.knock(v * 0.15)
			if GameMode.rule(pl.game_mode, "takes_damage") and main.player_inv:
				main.player_inv.damage(Settings.tnt_debris_damage, "explosion")
				if main.player_inv.is_dead():
					pl.died.emit()
			stats["hits"] = int(stats.get("hits", 0)) + 1
			return true
	var mobs = main.get("mobs")
	if mobs == null:
		return false
	for m in mobs.active():
		if (m.position + Vector3(0, reach, 0)).distance_to(p) < reach * 1.5:
			m.hurt(Settings.tnt_debris_damage, p - v.normalized())
			m.velocity += v * 0.1
			stats["hits"] = int(stats.get("hits", 0)) + 1
			return true
	return false


func _solid(p: Vector3) -> bool:
	var m: int = main.get_world_voxel(Vector3i(p.floor()))
	return m < 0 or (m > 0 and GameMaterials.blocks_bodies(m))


func live_count() -> int:
	return _live.size()
