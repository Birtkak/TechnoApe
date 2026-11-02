class_name ParticleFx
extends Node3D

## Minecraft-style particles (ADR 0026): block chips when mining and breaking,
## landing and sprint dust, splashes and bubbles, smoke and flames, hearts,
## lava pops, cave drips, spores, falling leaves and the spray at the foot of
## every waterfall. Pure presentation: nothing here touches the world or the
## save. Effects are data (data/particles.json); scripts ask for them by name.
##
## Two GPUParticles3D pools draw everything: "chips" samples the terrain atlas
## (a 4x4 pixel square of the block's tile, like Minecraft's breaking
## particles) and "sprites" samples a sheet of 8x8 pixel art built from the
## JSON. No particle is emitted automatically: each one is an emit_particle
## call carrying its whole state, and the process shader moves it (gravity,
## drag, a floor height found once per burst). Ambient effects are Minecraft's
## animateTick: SimWorld.sample_cells picks random cells around the player
## natively and only the hits come back here.

const EMIT_ALL := 31   # GPUParticles3D.EMIT_FLAG_POSITION | ROTATION_SCALE | VELOCITY | COLOR | CUSTOM
const MAX_MOTIONS := 16
const NO_FLOOR := -1.0e6
const SPRITE_PX := 8
const AMBIENT_HZ := 20.0   # Minecraft's client tick
const WHERE := ["any", "air_above", "air_below", "water_above", "furnace_front"]

signal burst_played(name: String, pos: Vector3, opts: Dictionary)   # only for names in sound_bursts (ADR 0044)

var errors: Array[String] = []
var effects := {}       # name -> effect dict, voxel units (see _compile_effect)
var motion_index := {}  # name -> index into the shader's motion arrays
var sprite_rows := {}   # name -> {row, frames}
var ambient := {}       # material id (-1 = burning) -> Array of {effect, chance, where}
var tile_override := {} # material name -> "top" | "bottom" | tile index

var get_voxel: Callable   # world voxel reader (Main.get_world_voxel): floors, furnace fronts
var bubble: SimBubble     # ambient sampling
var spray: WaterfallSpray # the waterfalls near the camera
var player: Player
var main_blocks: BlockEntities   # lit firepits and pit kilns smoke
var sound_bursts := {}           # burst name -> true: GameAudio hears these (the ambient ones stay silent and free)
var stats := {"emitted": 0, "dropped": 0, "culled": 0, "bursts": {}, "ambient_hits": 0, "ambient_samples": 0,
	"waterfalls": 0, "ms_max": 0.0, "ms_avg": 0.0, "ms_worst_part": ""}
var _ms_sum := 0.0
var _ms_frames := 0

var _sheet: Image
var _motion_a := PackedVector4Array()
var _motion_b := PackedVector4Array()
var _motion_floor := PackedInt32Array()   # per motion: 0 none, 1 rest, 2 die
var _chips: GPUParticles3D
var _sprites: GPUParticles3D
var _rng := RandomNumberGenerator.new()
var _defs: Array = []
var _ids := {}                          # material name -> id
var _tile := PackedInt32Array()         # material id -> chip tile
var _tile_color := PackedColorArray()   # atlas tile -> average colour
var _solid := PackedByteArray()         # material id -> 1 when chips come to rest on it
var _liquid := PackedByteArray()        # material id -> 1 for liquids
var _mask := PackedByteArray()          # SimWorld.sample_cells: materials with ambient rules
var _item_colors := {}                  # item id -> average sprite colour (eating crumbs)
var _budget := 0                        # particles left this frame
var _ambient_accum := 0.0
var _ambient_seed := 1
var _fall_accum := {}                   # waterfall key -> fractional particles owed per kind
var _warm_frames := 0
# player watch
var _sprint_accum := 0.0
var _bubble_t := 0.0
var _hit_accum := 0.0
var _eat_accum := 0.0
var _was_in_liquid := false
var _last_vel := Vector3.ZERO


static func load_default() -> ParticleFx:
	var fx := ParticleFx.new()
	fx.load_file("res://data/particles.json")
	return fx


func load_file(path: String) -> bool:
	var fa := FileAccess.open(path, FileAccess.READ)
	if fa == null:
		errors.append("cannot open %s" % path)
		return false
	var data = JSON.parse_string(fa.get_as_text())
	if not (data is Dictionary):
		errors.append("%s: not a JSON object" % path)
		return false
	return load_dict(data)


func load_dict(data: Dictionary) -> bool:
	_defs = GameMaterials.defs()
	var ids := _ids
	for i in _defs.size():
		ids[_defs[i]["name"]] = i + 1
	_load_sprites(data.get("sprites", {}))
	_load_motions(data.get("motions", {}))
	for k in data.get("tiles", {}):
		if k != "_doc":
			if not ids.has(k):
				errors.append("tiles: unknown material %s" % k)
			tile_override[k] = data["tiles"][k]
	var fx: Dictionary = data.get("effects", {})
	for name in fx:
		var e = _compile_effect(name, fx[name])
		if e != null:
			effects[name] = e
	var amb: Dictionary = data.get("ambient", {})
	for k in amb:
		if k == "_doc":
			continue
		var id: int = -1 if k == "burning" else int(ids.get(k, 0))
		if id == 0:
			errors.append("ambient: unknown material %s" % k)
			continue
		var rules: Array = []
		for r in amb[k]:
			if not effects.has(r.get("effect", "")):
				errors.append("ambient %s: unknown effect %s" % [k, r.get("effect", "")])
			elif not (r.get("where", "any") in WHERE):
				errors.append("ambient %s: unknown where %s" % [k, r.get("where", "")])
			else:
				rules.append({"effect": r["effect"], "chance": float(r.get("chance", 1.0)), "where": r.get("where", "any")})
		ambient[id] = rules
	_build_material_tables()
	return errors.is_empty()


func _load_sprites(sprites: Dictionary) -> void:
	var names := sprites.keys()
	_sheet = Image.create(SPRITE_PX * 8, SPRITE_PX * maxi(1, names.size()), false, Image.FORMAT_RGBA8)
	for row in names.size():
		var s: Dictionary = sprites[names[row]]
		var frames: Array = s.get("frames", [])
		if frames.is_empty() or frames.size() > 8:
			errors.append("sprite %s: 1..8 frames" % names[row])
			continue
		var palette := {}
		for ch in s.get("palette", {}):
			palette[ch] = Color.html(s["palette"][ch])
		for f in frames.size():
			var rows: Array = frames[f]
			if rows.size() != SPRITE_PX:
				errors.append("sprite %s frame %d: %d rows, want 8" % [names[row], f, rows.size()])
				continue
			for y in SPRITE_PX:
				var line: String = rows[y]
				if line.length() != SPRITE_PX:
					errors.append("sprite %s frame %d row %d: want 8 pixels" % [names[row], f, y])
					continue
				for x in SPRITE_PX:
					var ch := line[x]
					var c := Color(0, 0, 0, 0)
					if ch == ".":
						pass
					elif ch.is_valid_int():
						var g := int(ch) / 9.0
						c = Color(g, g, g, 1.0)
					elif palette.has(ch):
						c = palette[ch]
					else:
						errors.append("sprite %s: unknown pixel '%s'" % [names[row], ch])
					_sheet.set_pixel(f * SPRITE_PX + x, row * SPRITE_PX + y, c)
		sprite_rows[names[row]] = {"row": row, "frames": frames.size()}


func _load_motions(motions: Dictionary) -> void:
	_motion_a.resize(MAX_MOTIONS)
	_motion_b.resize(MAX_MOTIONS)
	_motion_floor.resize(MAX_MOTIONS)
	var i := 0
	for name in motions:
		if i >= MAX_MOTIONS:
			errors.append("more than %d motions" % MAX_MOTIONS)
			break
		var m: Dictionary = motions[name]
		var fl: int = ["none", "rest", "die"].find(m.get("floor", "none"))
		if fl < 0:
			errors.append("motion %s: floor must be none, rest or die" % name)
			fl = 0
		_motion_a[i] = Vector4(Settings.m2v(float(m.get("gravity", 0.0))), float(m.get("drag", 0.0)), float(fl), Settings.m2v(float(m.get("sway", 0.0))))
		_motion_b[i] = Vector4(float(m.get("glow", 0.0)), float(m.get("size_end", 1.0)), 0.0, 0.0)
		_motion_floor[i] = fl
		motion_index[name] = i
		i += 1


## Effect JSON (metres) -> the same dict in voxels, checked.
func _compile_effect(name: String, d: Dictionary):
	var e := d.duplicate(true)
	if not (e.get("channel", "") in ["chips", "sprites"]):
		errors.append("effect %s: channel must be chips or sprites" % name)
		return null
	if not motion_index.has(e.get("motion", "")):
		errors.append("effect %s: unknown motion %s" % [name, e.get("motion", "")])
		return null
	if e["channel"] == "sprites" and not sprite_rows.has(e.get("sprite", "")):
		errors.append("effect %s: unknown sprite %s" % [name, e.get("sprite", "")])
		return null
	if not (e.get("dir", "up") in ["up", "down", "out", "out_flat", "face", "any"]):
		errors.append("effect %s: unknown dir %s" % [name, e.get("dir", "")])
		return null
	for k in ["count", "life", "size", "speed"]:
		if not (e.get(k) is Array) or e[k].size() != 2:
			errors.append("effect %s: %s must be [min, max]" % [name, k])
			return null
	if float(e["life"][1]) >= 100.0:
		errors.append("effect %s: life under 100 s" % name)
		return null
	e["motion_i"] = motion_index[e["motion"]]
	e["size_v"] = [Settings.m2v(float(e["size"][0])), Settings.m2v(float(e["size"][1]))]
	e["speed_v"] = [Settings.m2v(float(e["speed"][0])), Settings.m2v(float(e["speed"][1]))]
	e["lift_v"] = Settings.m2v(float(e.get("lift", 0.0)))
	var b: Array = e.get("box", [0, 0, 0])
	e["box_v"] = Vector3(Settings.m2v(float(b[0])), Settings.m2v(float(b[1])), Settings.m2v(float(b[2])))
	e["cos_spread"] = cos(deg_to_rad(clampf(float(e.get("spread", 0.0)), 0.0, 180.0)))
	var cols: Array = []
	for c in e.get("colors", ["#ffffff"]):
		cols.append(Color.html(c))
	e["colors_c"] = cols
	if e["channel"] == "sprites":
		var s: Dictionary = sprite_rows[e["sprite"]]
		e["code"] = float(s["row"] * 8 + s["frames"] - 1)
	return e


func _build_material_tables() -> void:
	var n := _defs.size() + 1
	_tile.resize(n)
	_solid.resize(n)
	_liquid.resize(n)
	_mask.resize(256)
	_mask.fill(0)
	for i in _defs.size():
		var d: Dictionary = _defs[i]
		var tiles: Array = d.get("tiles", [0, 0, 0])
		var o = tile_override.get(d["name"], "side")
		_tile[i + 1] = int(tiles[0]) if o is String and o == "top" else (int(tiles[2]) if o is String and o == "bottom" else (int(o) if not (o is String) else int(tiles[1])))
		_solid[i + 1] = 1 if MobManager.is_solid(i + 1) else 0
		_liquid[i + 1] = 1 if d.get("cell_type", "") == "liquid" else 0
	for id in ambient:
		if id > 0 and id < 256:
			_mask[id] = 1


## The GPU side: both pools, the sprite sheet and the atlas. Call once after load.
func setup(atlas: Texture2D, atlas_tiles: Vector2i) -> void:
	_rng.randomize()
	var atlas_img := atlas.get_image()
	if atlas_img.is_compressed():
		atlas_img.decompress()
	_tile_color.resize(atlas_tiles.x * atlas_tiles.y)
	var tw := atlas_img.get_width() / atlas_tiles.x
	var th := atlas_img.get_height() / atlas_tiles.y
	for t in _tile_color.size():
		_tile_color[t] = _average(atlas_img, Rect2i((t % atlas_tiles.x) * tw, (t / atlas_tiles.x) * th, tw, th))
	var process := ShaderMaterial.new()
	process.shader = load("res://assets/shaders/particle_process.gdshader")
	process.set_shader_parameter("motion_a", _motion_a)
	_chips = _make_pool("Chips", process, atlas, Vector2(atlas_tiles), true, Settings.particle_pool_chips)
	var sheet := ImageTexture.create_from_image(_sheet)
	_sprites = _make_pool("Sprites", process, sheet, Vector2(8, _sheet.get_height() / SPRITE_PX), false, Settings.particle_pool_sprites)


func _make_pool(n: String, process: ShaderMaterial, tex: Texture2D, grid: Vector2, chips: bool, amount: int) -> GPUParticles3D:
	var draw := ShaderMaterial.new()
	draw.shader = load("res://assets/shaders/particle_draw.gdshader")
	draw.set_shader_parameter("tex", tex)
	draw.set_shader_parameter("grid", grid)
	draw.set_shader_parameter("chips", chips)
	draw.set_shader_parameter("motion_b", _motion_b)
	var quad := QuadMesh.new()
	quad.size = Vector2.ONE
	quad.material = draw
	var p := GPUParticles3D.new()
	p.name = n
	p.amount = amount
	p.lifetime = 100.0          # each particle's own lifetime lives in CUSTOM; the system never recycles
	p.emitting = false          # emit_particle only (it switches emitting off anyway)
	p.one_shot = false
	p.explosiveness = 0.0
	p.fixed_fps = 0             # move every frame: 30 fps particles stutter at 120 Hz
	p.interpolate = false
	p.local_coords = false
	p.process_material = process
	p.draw_pass_1 = quad
	p.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	p.visibility_aabb = AABB(Vector3(-1, -1, -1) * 1.0e5, Vector3.ONE * 2.0e5)   # world-space particles: never culled as a node
	add_child(p)
	return p


static func _average(img: Image, r: Rect2i) -> Color:
	var sum := Color(0, 0, 0, 0)
	var n := 0
	for y in range(r.position.y, r.end.y):
		for x in range(r.position.x, r.end.x):
			var c := img.get_pixel(x, y)
			if c.a > 0.5:
				sum += c
				n += 1
	return Color(sum.r / n, sum.g / n, sum.b / n, 1.0) if n > 0 else Color.WHITE


# ---------------- emitting ----------------

## Spawns effect `name` at `pos` (world, voxel units). opts: material (chips tile,
## tile tint, floor), normal (face direction), fall_m / speed_ms (extra count),
## velocity (added), floor (y; else found below pos), color (overrides the colours).
func burst(name: String, pos: Vector3, opts: Dictionary = {}) -> int:
	if sound_bursts.has(name):
		burst_played.emit(name, pos, opts)
	var e: Dictionary = effects.get(name, {})
	if e.is_empty() or _chips == null:
		return 0
	var cam := get_viewport().get_camera_3d()
	if cam and cam.global_position.distance_squared_to(pos) > pow(Settings.m2v(Settings.particle_distance_m), 2):
		stats["culled"] += 1
		return 0
	var n := _rng.randi_range(int(e["count"][0]), int(e["count"][1]))
	n += int(float(e.get("per_m", 0.0)) * float(opts.get("fall_m", 0.0)))
	n += int(float(e.get("per_ms", 0.0)) * float(opts.get("speed_ms", 0.0)))
	n += int(opts.get("count_bonus", 0))
	n = mini(n, int(e.get("max", 1 << 20)))
	var floor_y := NO_FLOOR
	if _motion_floor[e["motion_i"]] > 0:
		floor_y = float(opts["floor"]) if opts.has("floor") else _floor_below(pos)
	var code := -1.0
	var tint := Color.WHITE
	var mat := int(opts.get("material", 0))
	if e["channel"] == "chips" or e.get("tint", "") == "tile":
		if mat <= 0 or mat >= _tile.size():
			return 0
		if e["channel"] == "chips":
			code = float(_tile[mat])
		else:
			tint = _tile_color[_tile[mat]] if _tile[mat] < _tile_color.size() else Color.WHITE
	var done := 0
	for i in n:
		if _budget <= 0:
			stats["dropped"] += n - i
			break
		_emit_one(e, pos, opts, floor_y, code, tint)
		done += 1
	stats["bursts"][name] = int(stats["bursts"].get(name, 0)) + 1
	return done


func _emit_one(e: Dictionary, centre: Vector3, opts: Dictionary, floor_y: float, chip_tile: float, tint: Color) -> void:
	var box: Vector3 = e["box_v"]
	var off := Vector3(_rng.randf_range(-box.x, box.x), _rng.randf_range(-box.y, box.y), _rng.randf_range(-box.z, box.z))
	var base := Vector3.UP
	match e.get("dir", "up"):
		"down":
			base = Vector3.DOWN
		"out":
			base = off.normalized() if off.length_squared() > 1e-4 else _random_unit()
		"out_flat":
			var f := Vector3(off.x, 0, off.z)
			base = f.normalized() if f.length_squared() > 1e-4 else Vector3(cos(_rng.randf() * TAU), 0, sin(_rng.randf() * TAU))
		"face":
			base = Vector3(opts.get("normal", Vector3.UP)).normalized()
		"any":
			base = _random_unit()
	var dir := _cone(base, float(e["cos_spread"]))
	var speed := _rng.randf_range(e["speed_v"][0], e["speed_v"][1]) * float(opts.get("power", 1.0))
	var vel := dir * speed + Vector3(0, e["lift_v"], 0) + Vector3(opts.get("velocity", Vector3.ZERO))
	var size := _rng.randf_range(e["size_v"][0], e["size_v"][1])
	var shade: Array = e.get("shade", [1.0, 1.0])
	var s := _rng.randf_range(float(shade[0]), float(shade[1]))
	var col: Color
	if opts.has("color"):
		col = opts["color"]
	elif chip_tile >= 0.0:
		col = Color.WHITE
	else:
		var cols: Array = e["colors_c"]
		col = cols[_rng.randi() % cols.size()] * tint if e.get("tint", "") != "tile" else tint
	col = Color(col.r * s, col.g * s, col.b * s, 1.0)
	var code: float = chip_tile + float(_rng.randi() % 16) / 16.0 if chip_tile >= 0.0 else float(e["code"])
	var life := _rng.randf_range(float(e["life"][0]), float(e["life"][1]))
	var fy := floor_y + size * 0.5 if floor_y > NO_FLOOR else NO_FLOOR
	var pool := _chips if e["channel"] == "chips" else _sprites
	pool.emit_particle(Transform3D(Basis.from_scale(Vector3.ONE * size), centre + off), vel, col,
		Color(code, 0.0, life + 100.0 * float(e["motion_i"]), fy), EMIT_ALL)
	_budget -= 1
	stats["emitted"] += 1


func _random_unit() -> Vector3:
	var z := _rng.randf_range(-1.0, 1.0)
	var a := _rng.randf() * TAU
	var r := sqrt(1.0 - z * z)
	return Vector3(r * cos(a), z, r * sin(a))


## A random direction within the cone of cos(half angle) `cos_max` around `axis`.
func _cone(axis: Vector3, cos_max: float) -> Vector3:
	var ct := _rng.randf_range(cos_max, 1.0)
	var st := sqrt(maxf(0.0, 1.0 - ct * ct))
	var a := _rng.randf() * TAU
	var t := axis.cross(Vector3.RIGHT if absf(axis.x) < 0.9 else Vector3.FORWARD).normalized()
	var b := axis.cross(t)
	return (axis * ct + t * (st * cos(a)) + b * (st * sin(a))).normalized()


## Top of the first solid voxel at or below `pos` (8 voxels down at most).
func _floor_below(pos: Vector3) -> float:
	if not get_voxel.is_valid():
		return NO_FLOOR
	var v := Vector3i(pos.floor())
	for i in 8:
		var m: int = get_voxel.call(v)
		if m > 0 and m < _solid.size() and _solid[m] == 1:
			return float(v.y + 1)
		if m > 0 and m < _liquid.size() and _liquid[m] == 1:
			return float(v.y + 1)   # chips land on the water surface and sink out of sight
		v.y -= 1
	return NO_FLOOR


# ---------------- game events ----------------

## A material's id by name (an item id that names a block works too), 0 if none.
func material_id(name: String) -> int:
	return int(_ids.get(name, 0))


## Minecraft's block breaking particles: chips of the block's tile all over its cube.
func block_break(p: Vector3i, material: int) -> void:
	if material > 0 and material < _liquid.size() and _liquid[material] == 0 and _defs[material - 1].get("model", "") != "empty":
		burst("block_break", Vector3(p) + Vector3(0.5, 0.5, 0.5), {"material": material})


## While mining: a chip or two off the face the player is looking at.
func block_hit(p: Vector3i, material: int, from: Vector3) -> void:
	var c := Vector3(p) + Vector3(0.5, 0.5, 0.5)
	var d := from - c
	var n := Vector3(signf(d.x), 0, 0)
	if absf(d.y) >= absf(d.x) and absf(d.y) >= absf(d.z):
		n = Vector3(0, signf(d.y), 0)
	elif absf(d.z) >= absf(d.x):
		n = Vector3(0, 0, signf(d.z))
	burst("block_hit", c + n * 0.55, {"material": material, "normal": n, "floor": _floor_below(c + n * 0.55)})


## Dust of the block landed on, more for longer falls.
func landing(feet_voxel: Vector3i, fall_m: float, at: Vector3) -> void:
	if not get_voxel.is_valid():
		return
	var m: int = get_voxel.call(feet_voxel)
	if m > 0 and m < _solid.size() and _solid[m] == 1:
		burst("landing", Vector3(at.x, feet_voxel.y + 1.02, at.z), {"material": m, "fall_m": fall_m, "floor": float(feet_voxel.y + 1)})


func _process(delta: float) -> void:
	var _pt0 := Time.get_ticks_usec()
	_process_timed(delta)
	DebugCapture.note_proc("particles", _pt0)


func _process_timed(delta: float) -> void:
	if _chips == null:
		return
	var t0 := Time.get_ticks_usec()
	_budget = Settings.particle_budget
	_prewarm()
	var t1 := Time.get_ticks_usec()
	if player:
		_watch_player(delta)
	var t2 := Time.get_ticks_usec()
	_ambient_tick(delta)
	var t3 := Time.get_ticks_usec()
	_waterfalls(delta)
	var t4 := Time.get_ticks_usec()
	var ms := (t4 - t0) / 1000.0
	if ms > stats["ms_max"]:
		stats["ms_max"] = ms
		stats["ms_worst_part"] = "prewarm %.2f player %.2f ambient %.2f waterfalls %.2f, frame %d" % [(t1 - t0) / 1000.0, (t2 - t1) / 1000.0, (t3 - t2) / 1000.0, (t4 - t3) / 1000.0, Engine.get_process_frames()]
	_ms_sum += ms
	_ms_frames += 1
	stats["ms_avg"] = snappedf(_ms_sum / _ms_frames, 0.001)


## The first draw of each pool compiles its pipelines (~17 ms for the waterfall
## mist): one particle per pool in front of the camera during the first frames,
## while the loading screen still covers the view.
func _prewarm() -> void:
	if _warm_frames >= 8:
		return
	_warm_frames += 1
	var cam := get_viewport().get_camera_3d()
	if cam == null:
		return
	var at := cam.global_position - cam.global_transform.basis.z * 2.0
	_chips.emit_particle(Transform3D(Basis.from_scale(Vector3.ONE * 0.1), at), Vector3.ZERO, Color.WHITE, Color(0, 0, 0.05, NO_FLOOR), EMIT_ALL)
	_sprites.emit_particle(Transform3D(Basis.from_scale(Vector3.ONE * 0.1), at), Vector3.ZERO, Color.WHITE, Color(0, 0, 0.05, NO_FLOOR), EMIT_ALL)


# ---------------- the player ----------------

func _watch_player(delta: float) -> void:
	var feet := player.position
	var on_floor := player.is_on_floor()
	var flat := Vector2(player.velocity.x, player.velocity.z).length()
	# sprint dust (Minecraft: one block particle per tick while sprinting on the ground)
	if on_floor and not player.flying and not player.in_liquid and flat > Settings.m2v((Settings.walk_speed_m + Settings.sprint_speed_m) * 0.5):
		_sprint_accum += delta * AMBIENT_HZ
		var below := Vector3i((feet + Vector3(0, -0.1, 0)).floor())
		var m: int = get_voxel.call(below) if get_voxel.is_valid() else 0
		while _sprint_accum >= 1.0:
			_sprint_accum -= 1.0
			if m > 0 and m < _solid.size() and _solid[m] == 1:
				burst("sprint", Vector3(feet.x, below.y + 1.05, feet.z) - player.velocity.normalized() * 0.3, {"material": m, "floor": float(below.y + 1)})
	else:
		_sprint_accum = 0.0
	# splash on entering water, faster falls splash more
	if player.in_liquid and not _was_in_liquid:
		var speed_ms := Settings.v2m(maxf(-_last_vel.y, 0.0))
		if speed_ms > 1.0:
			burst("splash", Vector3(feet.x, _liquid_surface(feet), feet.z), {"speed_ms": speed_ms, "floor": _liquid_surface(feet) - 0.2})
	_was_in_liquid = player.in_liquid
	_last_vel = player.velocity
	# breath bubbles while the head is under
	var eye := player.camera.global_position
	var em: int = get_voxel.call(Vector3i(eye.floor())) if get_voxel.is_valid() else 0
	if em > 0 and em < _liquid.size() and _liquid[em] == 1:
		_bubble_t -= delta
		if _bubble_t <= 0.0:
			_bubble_t = _rng.randf_range(0.4, 1.4)
			burst("bubbles", eye - player.camera.global_transform.basis.z * 1.6 + Vector3(0, -0.4, 0))
	# chips off the block being mined, ~10 per second
	var target: Vector3i = player.mining_target()
	if target.x != 1 << 30 and get_voxel.is_valid():
		_hit_accum += delta * 10.0
		while _hit_accum >= 1.0:
			_hit_accum -= 1.0
			block_hit(target, get_voxel.call(target), eye)
	else:
		_hit_accum = 0.0
	# crumbs of the food being eaten
	var eating: String = player.eating_item()
	if eating != "":
		_eat_accum += delta
		if _eat_accum >= 0.2:
			_eat_accum = 0.0
			var fwd := -player.camera.global_transform.basis.z
			burst("crumbs", eye + fwd * 0.7 + Vector3(0, -0.35, 0), {"normal": fwd + Vector3(0, 0.3, 0), "color": _item_color(eating), "velocity": player.velocity})
	else:
		_eat_accum = 0.0


func _liquid_surface(p: Vector3) -> float:
	var v := Vector3i(p.floor())
	for i in 6:
		var m: int = get_voxel.call(v + Vector3i.UP)
		if m <= 0 or m >= _liquid.size() or _liquid[m] == 0:
			return float(v.y + 1)
		v.y += 1
	return float(v.y)


func _item_color(id: String) -> Color:
	if not _item_colors.has(id):
		var sprites := ItemSprites.get_default()
		var img: Image = sprites.image(id) if sprites.has(id) else null
		_item_colors[id] = _average(img, Rect2i(0, 0, img.get_width(), img.get_height())) if img else Color(0.8, 0.7, 0.5)
	return _item_colors[id]


# ---------------- ambient (Minecraft's animateTick) ----------------

func _ambient_tick(delta: float) -> void:
	if bubble == null or ambient.is_empty() or not Settings.particle_ambient:
		return
	_ambient_accum = minf(_ambient_accum + delta * AMBIENT_HZ, 3.0)
	if _ambient_accum < 1.0:
		return
	var cam := get_viewport().get_camera_3d()
	if cam == null:
		return
	var centre := Vector3i(cam.global_position.floor())
	while _ambient_accum >= 1.0:
		var hits := PackedInt32Array()
		var got := true
		for band in [[Settings.particle_ambient_near_m, Settings.particle_ambient_near_samples], [Settings.particle_ambient_far_m, Settings.particle_ambient_far_samples]]:
			_ambient_seed += 1
			var r := bubble.sample_cells(centre, int(Settings.m2v(band[0])), int(band[1]), _ambient_seed, _mask)
			if r.is_empty() and not bubble.can_sample():
				got = false   # a tick is running: try again next frame
				break
			stats["ambient_samples"] += int(band[1])
			hits.append_array(r)
		if not got:
			return
		_ambient_accum -= 1.0
		for i in range(0, hits.size(), 7):
			_ambient_hit(Vector3i(hits[i], hits[i + 1], hits[i + 2]), hits[i + 3], hits[i + 4], hits[i + 5], hits[i + 6])
	# lit stations (firepits, furnaces, forges, bloomeries...) are block entities, not materials: a few, checked
	# directly; the smoke leaves the top of the model, not its anchor corner (bug 2026-11-02_18-51-53)
	if main_blocks != null and Engine.get_process_frames() % 6 == 0:
		for e in main_blocks.lit():
			if not (e is SieveEntity or e is TorchEntity):
				var top := station_top(e)
				if e.kind == "firepit":   # forges and bloomeries are firepits too, but smoke from their top
					top.y = e.pos.y + 1.0   # the flames sit on the bed, not on the stone ring's rim
					if _rng.randf() < 0.8:
						burst("flame", top)
					if _rng.randf() < 0.4:
						burst("smoke", top + Vector3(0, 0.3, 0))
					if _rng.randf() < 0.1:
						burst("ember", top)
				elif _rng.randf() < 0.5:
					burst("large_smoke", top + Vector3(0, 0.4, 0))


## The centre of a block entity's footprint top, in voxels.
static func station_top(e: RefCounted) -> Vector3:
	var w := BlockEntities.width_of(e.kind)
	var size: Vector3i = StationEntity.SIZES.get(e.kind, Vector3i(w, BlockEntities.height_of(e.kind), w))
	return Vector3(e.pos) + Vector3(size.x * 0.5, size.y, size.z * 0.5)


func _ambient_hit(p: Vector3i, m: int, above: int, below: int, flags: int) -> void:
	var rules: Array = ambient.get(m, [])
	if flags & 1 and ambient.has(-1):   # SimWorld::FLAG_BURNING
		rules = rules + ambient[-1]
	for r in rules:
		if _rng.randf() >= float(r["chance"]):
			continue
		var at := Vector3(p) + Vector3(0.5, 0.5, 0.5)
		var opts := {"material": m}
		match r["where"]:
			"air_above":
				if above != GameMaterials.AIR:
					continue
				at.y = p.y + 1.02
			"air_below":
				if below != GameMaterials.AIR:
					continue
				at.y = p.y + 0.05
			"water_above":
				if below != GameMaterials.AIR or above < 0 or above >= _liquid.size() or _liquid[above] == 0:
					continue
				at.y = p.y - 0.05
			"furnace_front":
				var sides := [Vector3i(1, 0, 0), Vector3i(-1, 0, 0), Vector3i(0, 0, 1), Vector3i(0, 0, -1)]
				var s: Vector3i = sides[_rng.randi() % 4]
				if not get_voxel.is_valid() or get_voxel.call(p + s) != GameMaterials.AIR:
					continue
				at = Vector3(p) + Vector3(0.5, 0.3, 0.5) + Vector3(s) * 0.53
		burst(r["effect"], at, opts)
		stats["ambient_hits"] += 1


# ---------------- waterfalls ----------------

## The foot of every waterfall near the camera: white water thrown up along the
## whole width of the sheet, foam sliding out over the pool and mist rising.
## Rates grow with the sheet's width and the square root of its height (the
## water's speed at the bottom), so a thin cascade sprinkles and a wide, tall
## fall roars.
func _waterfalls(delta: float) -> void:
	if spray == null:
		return
	var cam := get_viewport().get_camera_3d()
	if cam == null:
		return
	var max_d := Settings.m2v(Settings.particle_waterfall_m)
	var n := 0
	for f in spray.falls():
		var foot := Vector3(f["x"] + 0.5, f["y"] + 1.0, f["z"] + 0.5)
		var d := cam.global_position.distance_to(foot)
		if d > max_d:
			continue
		n += 1
		var width: float = maxf(1.0, float(f.get("width", 3.0)))
		var drop_m := Settings.v2m(maxf(1.0, float(f["top"]) - float(f["y"])))
		var energy := width * sqrt(drop_m)                  # voxels * sqrt(m)
		var near := clampf(1.0 - (d - max_d * 0.4) / (max_d * 0.6), 0.2, 1.0)
		var power := clampf(sqrt(drop_m) / 2.5, 0.6, 2.2)   # how hard the water is thrown
		var dir := Vector3(float(f.get("dir_x", 0.0)), 0, float(f.get("dir_z", 1.0))).normalized()
		var side := Vector3(-dir.z, 0, dir.x)
		var impact := foot - dir * 1.0                      # the sheet comes down just upstream of the foot
		var key: Vector3i = Vector3i(f["x"], f["y"], f["z"])
		var acc: Dictionary = _fall_accum.get(key, {"spray": 0.0, "burst": 0.0, "mist": 0.0, "foam": 0.0})
		var rates := {
			"spray": minf(Settings.particle_waterfall_spray * energy, 1500.0) * near,
			"burst": minf(Settings.particle_waterfall_burst * energy, 300.0) * near,
			"mist": minf(Settings.particle_waterfall_mist * energy, 240.0) * near,
			"foam": minf(Settings.particle_waterfall_foam * energy, 400.0) * near,
		}
		for kind in rates:
			acc[kind] += rates[kind] * delta
			var e: Dictionary = effects.get("waterfall_" + kind, {})
			while acc[kind] >= 1.0 and _budget > 0 and not e.is_empty():
				acc[kind] -= 1.0
				var along := side * _rng.randf_range(-0.5, 0.5) * width
				match kind:
					"spray", "burst":
						# thrown up and out on both sides of the sheet, mostly downstream
						var out := (dir * _rng.randf_range(-0.5, 1.0) + side * _rng.randf_range(-0.4, 0.4)).normalized()
						_emit_one(e, impact + along, {"power": power, "velocity": out * Settings.m2v(_rng.randf_range(0.5, 2.5)) * power}, foot.y - 0.3, -1.0, Color.WHITE)
					"mist":
						_emit_one(e, impact + along + Vector3(0, 0.3, 0), {"power": power, "velocity": dir * Settings.m2v(0.8)}, NO_FLOOR, -1.0, Color.WHITE)
					"foam":
						_emit_one(e, impact + along + Vector3(0, 0.05, 0), {"power": 1.0, "velocity": dir * Settings.m2v(1.2)}, NO_FLOOR, -1.0, Color.WHITE)
			acc[kind] = minf(acc[kind], 4.0)
		_fall_accum[key] = acc
	stats["waterfalls"] = n
