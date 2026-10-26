class_name Tnt
extends Node3D

## TNT (ADR 0065, bug report 2026-10-25_21-10-33). TNT is a plain voxel material placed like any
## block (the brush); a charge is one 0.5 m block cell (BLOCK^3 voxels on the block grid) holding TNT
## voxels, and it blasts with the cube root of how full it is. A charge is lit by
##   - fire or lava touching it (checked a few charges at a time, Settings-free: FIRE_CHECK_S),
##   - flint and steel or the blasting igniter aimed at it (sneak + igniter: every charge in range),
##   - another blast reaching it (a short random fuse, Minecraft's chain).
## Lit, its voxels become a primed block (a blinking box that hisses, sparks and falls) for
## Settings.tnt_fuse_s, then it explodes: three native spheres (SimBubble.blast) by hardness - soft
## ground the full radius, stone 3/4, hard rock 1/2, obsidian and bedrock never - then the rim seeds
## go to the rigid piece check, entities near it take damage and knockback, and a share of what it
## took drops (estimated from samples, never by reading every voxel). Fuses that run out together
## wait their turn: Settings.tnt_blasts_per_frame craters per frame, so a chain spreads over frames.

const FIRE_CHECK_S := 0.2          # how often the fire check runs
const FIRE_CHECKS_PER_RUN := 48    # charges checked per run (round robin)
const DROP_SAMPLES := 64           # points sampled in the sphere to estimate what it took
const SPARK_EVERY_S := 0.08
const HARD_HP := 1200.0            # above this: the hard rock shell (1/2 radius)
const SOFT_HP := 400.0             # below this: the soft shell (full radius); between: stone (3/4)
const NEVER_HP := 5000.0           # at or above this: never blasted (obsidian, bedrock)

var main: Node
var charges := {}                  # Vector3i block anchor (low corner, on the BLOCK grid) -> true
var primed: Array[Dictionary] = [] # {node, mat, pos (voxels, the box centre), fuse, share, vy, spark_t}
var stats := {"lit": 0, "chain_lit": 0, "fire_lit": 0, "blasts": 0, "voxels": 0, "blast_ms_max": 0.0, "blast_ms_last": 0.0,
	"drops": 0, "queued_max": 0, "frames_with_blast": 0, "entities_hit": 0, "parts_max_ms": {}}
var _queue: Array[Dictionary] = [] # {pos, share}: fuses that ran out, waiting for their frame
var _fire_t := 0.0
var _fire_keys: Array = []
var _fire_i := 0
var _rng := RandomNumberGenerator.new()
var _st := ShapeTool.new()
var _base_mat: StandardMaterial3D
var _box: BoxMesh
var _shells := []                  # [[radius factor, PackedInt32Array of the ids it takes]]
var _drop_carry := {}


func setup(p_main: Node) -> void:
	main = p_main
	_rng.randomize()
	_base_mat = StandardMaterial3D.new()
	_base_mat.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_base_mat.uv1_triplanar = true
	_base_mat.uv1_scale = Vector3.ONE / 16.0   # one texel per voxel, like the terrain's patterned tiles
	_base_mat.emission_enabled = true
	_base_mat.emission = Color(1, 1, 1)
	_base_mat.emission_energy_multiplier = 0.0
	_base_mat.albedo_texture = _tile_texture(171)
	_box = BoxMesh.new()
	_box.size = Vector3.ONE * Settings.BLOCK
	_box.material = _base_mat
	_build_shells()


## MaterialWarmup draws the primed box under the loading screen (a first draw mid-game compiled
## 4 pipelines in a 43 ms frame: the box's vertex format, not just the material, has to be drawn).
func warm_meshes() -> Array:
	return [_box]


static func _tile_texture(tile: int) -> Texture2D:
	var atlas: Texture2D = load("res://assets/textures/atlas.png")
	if atlas == null:
		return null
	var img := atlas.get_image()
	if img.is_compressed():
		img.decompress()
	var tw := img.get_width() / GameMaterials.ATLAS_TILES.x
	var r := Rect2i((tile % GameMaterials.ATLAS_TILES.x) * tw, (tile / GameMaterials.ATLAS_TILES.x) * tw, tw, tw)
	return ImageTexture.create_from_image(img.get_region(r))


## Which materials each shell of a blast takes, by hardness ("hp"; powders, wood and plants have
## none and count as soft). Liquids, gases and fire stay (a crater under water fills with it);
## stations, furniture and work spots stay (SimBubble.blast keeps them anyway).
func _build_shells() -> void:
	var soft := PackedInt32Array()
	var stone := PackedInt32Array()
	var hard := PackedInt32Array()
	var defs := GameMaterials.defs()
	for i in mini(defs.size(), 255):
		var d: Dictionary = defs[i]
		var id := i + 1
		if d.get("cell_type", "solid") not in ["solid", "powder"] or d.has("interact") or d.has("entity"):
			continue
		var hp := float(d.get("hp", 0.0))
		if hp >= NEVER_HP:
			continue
		hard.append(id)
		if hp <= HARD_HP:
			stone.append(id)
		if hp < SOFT_HP:
			soft.append(id)
	_shells = [[1.0, soft], [0.75, stone], [0.5, hard]]


static func shell_factor(material: int) -> float:
	if material <= 0 or material > GameMaterials.defs().size():
		return 0.0
	var d: Dictionary = GameMaterials.defs()[material - 1]
	if d.get("cell_type", "solid") not in ["solid", "powder"] or d.has("interact") or d.has("entity"):
		return 0.0
	var hp := float(d.get("hp", 0.0))
	if hp >= NEVER_HP:
		return 0.0
	if hp < SOFT_HP:
		return 1.0
	return 0.75 if hp <= HARD_HP else 0.5


## Blast radius in voxels of a charge `share` full (1 = a whole block).
static func radius_of(share: float) -> float:
	return Settings.m2v(Settings.tnt_radius_m) * pow(clampf(share, 1.0 / 64.0, 8.0), 1.0 / 3.0)


static func anchor_of(p: Vector3i) -> Vector3i:
	var b := Settings.BLOCK
	return Vector3i(floori(float(p.x) / b) * b, floori(float(p.y) / b) * b, floori(float(p.z) / b) * b)


# ---------------- charges ----------------

## TNT voxels were placed: their block cells become charges.
func register_cells(cells: Array) -> void:
	for c in cells:
		charges[anchor_of(c)] = true


## The TNT voxels in a charge's block cell (BLOCK^3 reads, only when it is lit).
func _tnt_cells(anchor: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var b := Settings.BLOCK
	for y in b:
		for z in b:
			for x in b:
				var c := anchor + Vector3i(x, y, z)
				if main.get_world_voxel(c) == GameMaterials.TNT:
					out.append(c)
	return out


## Lights the charge at `anchor`: its TNT voxels become a primed block with `fuse` seconds to go.
## False when no TNT is left there.
func light(anchor: Vector3i, fuse: float = -1.0) -> bool:
	return _light_cells(anchor, _tnt_cells(anchor), fuse)


func _light_cells(anchor: Vector3i, cells: Array[Vector3i], fuse: float) -> bool:
	charges.erase(anchor)
	if cells.is_empty():
		return false
	var centre := Vector3.ZERO
	for c in cells:
		main.set_world_voxel(c, GameMaterials.AIR)
		centre += Vector3(c) + Vector3(0.5, 0.5, 0.5)
	centre /= cells.size()
	var share := cells.size() / float(Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)
	_prime(centre, share, Settings.tnt_fuse_s if fuse < 0.0 else fuse)
	stats["lit"] += 1
	return true


func _prime(pos: Vector3, share: float, fuse: float) -> void:
	var mi := MeshInstance3D.new()
	mi.mesh = _box
	var mat: StandardMaterial3D = _base_mat.duplicate()   # same shader: no new pipeline, own blink
	mi.material_override = mat
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF   # a shadow pass would compile a pipeline mid-game
	mi.scale = Vector3.ONE * pow(share, 1.0 / 3.0)
	add_child(mi)
	mi.global_position = pos
	primed.append({"node": mi, "mat": mat, "pos": pos, "fuse": fuse, "fuse0": fuse, "share": share, "vy": 0.0, "spark_t": 0.0})
	Sound.play("tnt/light", pos)


## Flint and steel or the igniter aimed at a voxel: lights the charge when it is TNT.
func light_at(hit: VoxelRaycastResult) -> bool:
	if hit == null or main.get_world_voxel(hit.position) != GameMaterials.TNT:
		return false
	return light(anchor_of(hit.position))


## The igniter's sneak use: every charge within `radius_m` of `from` (the nearest first get the
## shortest fuse, so a row goes off in order). Returns how many were lit.
func light_near(from: Vector3, radius_m: float) -> int:
	var r2 := pow(Settings.m2v(radius_m), 2)
	var near := []
	for a in charges.keys():
		var d := (Vector3(a) + Vector3.ONE * Settings.BLOCK * 0.5).distance_squared_to(from)
		if d <= r2:
			near.append([d, a])
	near.sort_custom(func(x, y) -> bool: return x[0] < y[0])
	var n := 0
	for i in near.size():
		if light(near[i][1], Settings.tnt_fuse_s + 0.15 * i):
			n += 1
	return n


func to_array() -> Array:
	var out := []
	for a in charges:
		out.append([a.x, a.y, a.z])
	return out


func load_array(arr: Array) -> void:
	for v in arr:
		if v is Array and v.size() == 3:
			charges[Vector3i(int(v[0]), int(v[1]), int(v[2]))] = true


# ---------------- every frame ----------------

func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("tnt", _pt0)


func _process_timed(delta: float) -> void:
	if main == null or main.bubble == null:
		return
	var t0 := Time.get_ticks_usec()
	_fire_check(delta)
	var i := 0
	while i < primed.size():
		var p := primed[i]
		if _tick_primed(p, delta):
			i += 1
			continue
		p["node"].queue_free()
		primed.remove_at(i)
		_queue.append({"pos": p["pos"], "share": p["share"]})
	stats["queued_max"] = maxi(stats["queued_max"], _queue.size())
	var n := 0
	while not _queue.is_empty() and n < Settings.tnt_blasts_per_frame:
		var q: Dictionary = _queue[0]
		if not q.get("prepared", false):
			_prepare(q)   # this frame: the chain search; the next: the blast
		else:
			_queue.pop_front()
			_blast(q)
		n += 1
	if n > 0:
		stats["frames_with_blast"] += 1
	stats["process_ms_max"] = maxf(float(stats.get("process_ms_max", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)


## Fuse, blink, sparks and falling of one primed block; false once the fuse ran out.
func _tick_primed(p: Dictionary, delta: float) -> bool:
	p["fuse"] = float(p["fuse"]) - delta
	if p["fuse"] <= 0.0:
		return false
	# blink white, faster as the fuse burns down (Minecraft)
	var period := lerpf(0.12, 0.5, clampf(float(p["fuse"]) / maxf(float(p["fuse0"]), 0.01), 0.0, 1.0))
	var on := fmod(float(p["fuse"]), period) < period * 0.5
	(p["mat"] as StandardMaterial3D).emission_energy_multiplier = 0.9 if on else 0.0
	# falls while nothing solid is under it
	var half := Settings.BLOCK * 0.5 * pow(float(p["share"]), 1.0 / 3.0)
	var pos: Vector3 = p["pos"]
	if not _solid_at(Vector3(pos.x, pos.y - half - 0.01, pos.z)):
		p["vy"] = maxf(float(p["vy"]) - Settings.m2v(Settings.gravity_m) * delta, -Settings.m2v(30.0))
		var bottom := pos.y - half + float(p["vy"]) * delta
		if _solid_at(Vector3(pos.x, bottom, pos.z)):
			bottom = floorf(bottom) + 1.0   # landed on that voxel
			p["vy"] = 0.0
		pos.y = bottom + half
		p["pos"] = pos
		(p["node"] as Node3D).global_position = pos
	else:
		p["vy"] = 0.0
	p["spark_t"] = float(p["spark_t"]) - delta
	if p["spark_t"] <= 0.0:
		p["spark_t"] = SPARK_EVERY_S
		var top := pos + Vector3(0, half + 0.2, 0)
		if main.fx:
			main.fx.burst("tnt_fuse", top)
		Sound.play("tnt/fuse", top)
	return true


func _solid_at(p: Vector3) -> bool:
	var m: int = main.get_world_voxel(Vector3i(p.floor()))
	return m < 0 or (m > 0 and GameMaterials.blocks_bodies(m))   # unloaded counts as solid: never fall out of the world


## A few charges per run: fire or lava on a face of the block lights it.
func _fire_check(delta: float) -> void:
	_fire_t -= delta
	if _fire_t > 0.0 or charges.is_empty():
		return
	_fire_t = FIRE_CHECK_S
	if _fire_i >= _fire_keys.size():
		_fire_keys = charges.keys()
		_fire_i = 0
	var b := Settings.BLOCK
	var end := mini(_fire_i + FIRE_CHECKS_PER_RUN, _fire_keys.size())
	for k in range(_fire_i, end):
		var a: Vector3i = _fire_keys[k]
		if not charges.has(a):
			continue
		if _touches_fire(a, b):
			if light(a):
				stats["fire_lit"] += 1
	_fire_i = end


func _touches_fire(a: Vector3i, b: int) -> bool:
	var m := b / 2
	for u in [m - 1, m]:
		for v in [m - 1, m]:
			for c in [a + Vector3i(-1, u, v), a + Vector3i(b, u, v), a + Vector3i(u, -1, v), a + Vector3i(u, b, v),
					a + Vector3i(u, v, -1), a + Vector3i(u, v, b)]:
				var mat: int = main.get_world_voxel(c)
				if mat == GameMaterials.FIRE or mat == GameMaterials.LAVA:
					return true
	return false


# ---------------- the blast ----------------

## One explosion of a charge `share` full at `pos` (voxels), both stages at once. Returns the voxels
## it removed. In play the stages run in two frames (_process): the chain search is the costly half.
func explode(pos: Vector3, share: float) -> int:
	var q := {"pos": pos, "share": share}
	_prepare(q)
	return _blast(q)


## Stage one: light the charges the blast will reach and sample what it will take (before the
## voxels go). Fills q.r, q.drops, q.first.
func _prepare(q: Dictionary) -> void:
	var t0 := Time.get_ticks_usec()
	var pos: Vector3 = q["pos"]
	var r := radius_of(float(q["share"]))
	_chain(pos, r)
	var t := _part("chain", t0)
	q["r"] = r
	q["drops"] = _estimate_drops(pos, r)
	q["first"] = main.get_world_voxel(Vector3i(pos.floor() + Vector3(0, -1, 0)))
	_part("sample", t)
	q["prepared"] = true


## Stage two: the spheres, the rigid piece check, entities, drops, particles and sound.
func _blast(q: Dictionary) -> int:
	var t0 := Time.get_ticks_usec()
	var t := t0
	var pos: Vector3 = q["pos"]
	var share: float = q["share"]
	var r: float = q["r"]
	var drops: Dictionary = q["drops"]
	var first: int = q["first"]
	var n := 0
	for sh in _shells:
		n += main.bubble.blast(pos, r * float(sh[0]), GameMaterials.AIR, sh[1])
	t = _part("spheres", t)
	if main.pieces:
		main.pieces.check(PlaytestTools.rim_seeds(pos, r), pos)
	t = _part("pieces", t)
	_hit_entities(pos, r)
	_spawn_drops(drops, n, pos, r)
	t = _part("entities_drops", t)
	if main.fx:
		main.fx.burst("tnt_flash", pos)
		main.fx.burst("tnt_smoke", pos)
		main.fx.burst("tnt_sparks", pos)
		if first > 0:
			main.fx.block_break(Vector3i(pos.floor()), first)
	Sound.play("tnt/explode", pos, {"db": clampf(4.0 * log(maxf(share, 0.02)) / log(8.0), -6.0, 4.0)})
	_part("fx_sound", t)
	var ms := (Time.get_ticks_usec() - t0) / 1000.0
	stats["blasts"] += 1
	stats["voxels"] += n
	stats["blast_ms_last"] = ms
	stats["blast_ms_max"] = maxf(stats["blast_ms_max"], ms)
	return n


## Notes the slowest time of one part of a blast (stats.parts_max_ms); returns now.
func _part(name: String, since: int) -> int:
	var now := Time.get_ticks_usec()
	var pm: Dictionary = stats["parts_max_ms"]
	pm[name] = maxf(float(pm.get(name, 0.0)), snappedf((now - since) / 1000.0, 0.001))
	return now


## Every TNT voxel the blast reaches (found natively, registered or not) lights its charge with a
## short random fuse, before the blast takes the voxels.
func _chain(pos: Vector3, r: float) -> void:
	var reach := r + Settings.BLOCK
	var lo := Vector3i((pos - Vector3.ONE * reach).floor())
	var size := Vector3i(Vector3.ONE * (2.0 * reach + 1.0))
	var mask := PackedByteArray()
	mask.resize(256)
	mask[GameMaterials.TNT] = 1
	var cells: PackedInt32Array = _st.select(main.bubble.sim, lo, size, main.bubble.origin * Settings.CHUNK, mask)
	var anchors := {}   # anchor -> its TNT cells
	for i in range(0, cells.size(), 3):
		var c := Vector3i(cells[i], cells[i + 1], cells[i + 2])
		var a := anchor_of(c)
		if not anchors.has(a):
			anchors[a] = [] as Array[Vector3i]
		anchors[a].append(c)
	for a in anchors:
		var near := false
		for c in anchors[a]:
			if (Vector3(c) + Vector3(0.5, 0.5, 0.5)).distance_to(pos) <= reach:
				near = true
				break
		if near and _light_cells(a, anchors[a], _rng.randf_range(Settings.tnt_chain_fuse_s.x, Settings.tnt_chain_fuse_s.y)):
			stats["chain_lit"] += 1


## What the blast will take, from DROP_SAMPLES points in the sphere (each taken when it lies inside
## its material's shell): {material name: estimated voxels}.
func _estimate_drops(pos: Vector3, r: float) -> Dictionary:
	var counts := {}
	var vol := 4.0 / 3.0 * PI * r * r * r
	for i in DROP_SAMPLES:
		var d := Vector3(_rng.randf_range(-1, 1), _rng.randf_range(-1, 1), _rng.randf_range(-1, 1))
		while d.length_squared() > 1.0:
			d = Vector3(_rng.randf_range(-1, 1), _rng.randf_range(-1, 1), _rng.randf_range(-1, 1))
		var m: int = main.bubble.get_voxel(Vector3i((pos + d * r).floor()))
		if m <= 0 or d.length() > shell_factor(m):
			continue
		var name := GameMaterials.name_of(m)
		counts[name] = float(counts.get(name, 0.0)) + vol / DROP_SAMPLES
	return counts


## Settings.tnt_drop_fraction of the estimate drops, as the material's mining drop (no tool needed:
## a blast is not a tool). Voxel items count voxels; other drops one per block.
func _spawn_drops(counts: Dictionary, removed: int, pos: Vector3, r: float) -> void:
	if removed <= 0 or main.player == null or not GameMode.rule(main.player.game_mode, "drops") or main.player.mining == null:
		return
	var mining: MiningTable = main.player.mining
	var per := float(Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)
	var stacks := []
	for name in counts:
		var e := mining.entry(name)
		var drop = e.get("drops", null)
		if drop is Array:
			drop = drop[0].get("id", null) if not drop.is_empty() else null
		if drop == null or String(drop) == "" or not main.items.has(String(drop)):
			continue
		var id := String(drop)
		var amount: float = float(counts[name]) * Settings.tnt_drop_fraction
		if main.items.units(id) <= 1:
			amount /= per
		var total: float = float(_drop_carry.get(id, 0.0)) + amount
		var k := floori(total)
		_drop_carry[id] = total - k
		while k > 0:
			var s := mini(k, main.items.max_stack(id))
			stacks.append({"id": id, "count": s})
			k -= s
	for s in stacks.slice(0, 4):   # a few stacks: each item entity costs a node
		var dir := Vector3(_rng.randf_range(-1, 1), _rng.randf_range(0.2, 1.0), _rng.randf_range(-1, 1)).normalized()
		main.spawn_item(s, pos + dir * r * 0.3, dir * Settings.m2v(_rng.randf_range(2.0, 5.0)))
		stats["drops"] += int(s["count"])


## Damage and knockback, full at the centre and none at twice the radius; walls do not shield yet.
func _hit_entities(pos: Vector3, r: float) -> void:
	var reach := 2.0 * r
	var p = main.player
	if p == null:
		return
	var centre: Vector3 = p.position + Vector3(0, Settings.m2v(Settings.player_height_m) * 0.5, 0)
	var d := centre.distance_to(pos)
	if d < reach and GameMode.rule(p.game_mode, "interacts"):
		var f := 1.0 - d / reach
		var away := (centre - pos).normalized() if d > 0.01 else Vector3.UP
		p.knock((away + Vector3(0, 0.6, 0)).normalized() * Settings.m2v(Settings.tnt_knockback_m) * f)
		var dmg := roundi(Settings.tnt_damage * f)
		if dmg > 0 and GameMode.rule(p.game_mode, "takes_damage") and main.player_inv:
			main.player_inv.damage(dmg)
			if main.player_inv.is_dead():
				p.died.emit()
		stats["entities_hit"] += 1
	if main.mobs == null:
		return
	for m in main.mobs.active():
		var md: float = m.position.distance_to(pos)
		if md >= reach:
			continue
		var f := 1.0 - md / reach
		m.hurt(maxi(1, roundi(Settings.tnt_damage * f)), pos)
		var away: Vector3 = (m.position - pos).normalized() if md > 0.01 else Vector3.UP
		m.velocity += (away + Vector3(0, 0.5, 0)) * Settings.m2v(Settings.tnt_knockback_m) * f * 0.5
		stats["entities_hit"] += 1


func snapshot() -> Dictionary:
	var s := stats.duplicate()
	s["charges"] = charges.size()
	s["primed"] = primed.size()
	s["queued"] = _queue.size()
	return s
