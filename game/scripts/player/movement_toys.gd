class_name MovementToys
extends Node3D

## Movement toys (ADR 0151): the glider, the climbing rope and the grappling hook; an item's
## "move_toy" names its kind. Player-local: Player hands every on-foot physics step to step() and a
## right click to use(). A toy only sets Player.velocity: the body still moves through Player._move
## (VoxelBoxMover, swept against the voxels), so nothing tunnels. Hung ropes are saved with the
## player (to_array / load_array), like the TNT charges. Other players see none of it (ADR 0151).
##   glider   jump (or right click) while falling opens it: a slow sink, steered by the camera, a
##            dive's speed kept for the pull-up; ground, water, jump again or another item folds it
##   rope     right click a cliff top (it hangs over the edge you face), a wall or a ceiling: it hangs
##            to the ground, rope_max_m at most. Against it W climbs, S goes down, nothing held hangs,
##            jump lets go; a right click on it takes it back
##   grapple  right click fires the hook; it catches on a solid voxel (not sand, gravel, leaves), held
##            right click reels in, released the line holds a pendulum (WASD pumps the swing); jump
##            lets go with the swing's speed, another right click or a voxel dug away unhooks it

const G_M := 9.8                 # the glider's and the swing's gravity (the walking body falls at 2 g)
const GLIDE_TURN := 4.0          # the glider's heading follows the camera at this rate (1/s)
const STALL_M := 5.0             # slower than this the nose drops, whatever the camera does
const ROLL_MAX := 0.35           # radians of camera roll in a hard turn
const ROPE_RADIUS_M := 0.035
const ROPE_GRAB_M := 0.3         # the rope is in reach this far past the body's side
const SWING_PUMP_M := 5.0        # WASD pushes a swing this hard (m/s^2)
const CARRY_DRAG_M := 1.5        # a let-go's speed dies away this fast in the air (m/s^2)
const HOOK_HOLD := ["sand", "gravel", "snow", "dirt_loose"]   # name parts a hook slips off (leaves hold: swing from a crown)

var main: Node
var p: Player
var gliding := false
var roll := 0.0                  # camera roll (Player._apply_look adds it)
var climbing := false
var hook := ""                   # "" | "flying" | "caught"
var reeling := false
var ropes: Array = []            # {x, z, top, bottom} in voxels: the rope's line, its top and its foot
var stats := {"glides": 0, "ropes_hung": 0, "hooks_caught": 0, "hooks_slipped": 0}
var _gs := 0.0                   # the glider's airspeed, m/s
var _heading := 0.0
var _glide_v := Vector3.ZERO     # the glide's last velocity (voxels/s): it carries on when the glider folds
var _jump_was := false
var _let_go_t := 0.0             # after letting go of a rope it is not grabbed again for a moment
var _v := Vector3.ZERO           # the body's velocity while the hook holds it (voxels/s)
var _carry := Vector3.ZERO       # sideways speed a toy left the body with: kept in the air, a slide on landing
var _hook_pos := Vector3.ZERO
var _hook_dir := Vector3.ZERO
var _hook_flown := 0.0
var _anchor := Vector3.ZERO
var _anchor_voxel := Vector3i.ZERO
var _line := 0.0                 # the line's length (voxels)
var _rope_nodes: Array[Node3D] = []
var _anchor_t := 0.0
var _push := 0.0                 # a climber under a lip leans this far out from the face (voxels)
var _rope_mat: StandardMaterial3D
var _line_node: MeshInstance3D
var _hook_node: MeshInstance3D
var _wing: MeshInstance3D
var _wind: AudioStreamPlayer


func setup(p_main: Node) -> void:
	main = p_main
	p = main.player
	_rope_mat = StandardMaterial3D.new()
	_rope_mat.albedo_color = Color(0.62, 0.48, 0.28)
	_rope_mat.roughness = 1.0
	_line_node = _mesh(_cylinder(0.012), "GrappleLine")
	var hm := StandardMaterial3D.new()
	hm.albedo_color = Color(0.25, 0.26, 0.28)
	hm.metallic = 0.8
	var hb := BoxMesh.new()
	hb.size = Vector3.ONE * Settings.m2v(0.12)
	hb.material = hm
	_hook_node = _mesh(hb, "GrappleHook")
	var wm := StandardMaterial3D.new()
	wm.albedo_color = Color(0.86, 0.8, 0.66)
	wm.cull_mode = BaseMaterial3D.CULL_DISABLED
	var wing := PrismMesh.new()   # a delta wing: the prism's ridge points forward
	wing.size = Vector3(Settings.m2v(2.6), Settings.m2v(1.4), Settings.m2v(0.04))
	wing.material = wm
	_wing = MeshInstance3D.new()
	_wing.name = "GliderWing"
	_wing.mesh = wing
	_wing.visible = false
	_wing.rotation = Vector3(-PI / 2, 0, 0)
	_wing.position = Vector3(0, Settings.m2v(2.05), Settings.m2v(-0.1))
	p.add_child(_wing)
	_wind = AudioStreamPlayer.new()
	var st = load("res://assets/sounds/loop/wind.ogg")
	if st is AudioStreamOggVorbis:
		st = st.duplicate()
		st.loop = true
		_wind.stream = st
	_wind.bus = "World"
	add_child(_wind)


func warm_materials() -> Array:
	return [_rope_mat, (_hook_node.mesh as BoxMesh).material, (_wing.mesh as PrismMesh).material]


func _cylinder(radius_m: float) -> CylinderMesh:
	var c := CylinderMesh.new()
	c.top_radius = Settings.m2v(radius_m)
	c.bottom_radius = c.top_radius
	c.height = 1.0
	c.radial_segments = 6
	c.rings = 1
	c.material = _rope_mat
	return c


func _mesh(m: Mesh, n: String) -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.name = n
	mi.mesh = m
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	mi.visible = false
	add_child(mi)
	return mi


func held_kind() -> String:
	return str(p.inventory.held_def().get("move_toy", "")) if p.inventory else ""


# ---------------- right click ----------------

## A right click (first press). True when a toy took it.
func use(hit: VoxelRaycastResult, from: Vector3, dir: Vector3) -> bool:
	var r := rope_at(from, dir, hit.distance if hit else Settings.m2v(p.reach()))
	if r >= 0:
		take_rope(r)
		return true
	match held_kind():
		"glider":
			if not p.is_on_floor() and not p.in_liquid:
				open_glider()
			return true
		"rope":
			if hang_rope(hit, dir) and GameMode.rule(p.game_mode, "consumes_items"):
				p.inventory.consume_held(1)
			return true
		"grapple":
			if hook != "":
				unhook()
			else:
				fire_hook(from, dir)
			return true
	return false


# ---------------- the physics step ----------------

## One on-foot physics step, after Player set the walking velocity and gravity and before it moves.
func step(delta: float, dir: Vector3, input: Vector2, jump: bool) -> void:
	var jump_edge := jump and not _jump_was
	_jump_was = jump
	_let_go_t = maxf(0.0, _let_go_t - delta)
	_anchor_t -= delta
	if _anchor_t <= 0.0:
		_anchor_t = 0.5
		_drop_loose_ropes()
	if hook == "flying":
		_fly_hook(delta)
	if hook == "caught":
		if held_kind() != "grapple" or not _holds(main.get_world_voxel(_anchor_voxel)):
			unhook()   # the voxel it held was dug away, or the hook was put away
		elif jump_edge:
			_let_go_with_swing()
		else:
			_swing(delta, dir)
			_close_glider()
			_wind_at(Settings.v2m(_v.length()))
			return
	if gliding:
		if jump_edge or p.is_on_floor() or p.in_liquid or held_kind() != "glider":
			_close_glider()
		else:
			_glide(delta)
			return
	elif jump_edge and held_kind() == "glider" and not p.is_on_floor() and not p.in_liquid and p.velocity.y < 0.0:
		open_glider()
		return
	_carry_on(delta, dir)
	_wind_at(Settings.v2m(_carry.length()))
	_climb(delta, dir, input, jump_edge)


## The speed a fold, a let-go or a jump off a rope left: in the air it carries on (the walk steers a
## little), on landing or in water it becomes Player's knock (a short slide).
func _carry_on(delta: float, dir: Vector3) -> void:
	if _carry == Vector3.ZERO:
		return
	if p.body.blocked.x > 0.0:
		_carry.x = 0.0
	if p.body.blocked.z > 0.0:
		_carry.z = 0.0
	if p.is_on_floor() or p.in_liquid:
		p._knock = _carry
		_carry = Vector3.ZERO
		return
	p.velocity.x = _carry.x + dir.x * Settings.m2v(1.5)
	p.velocity.z = _carry.z + dir.z * Settings.m2v(1.5)
	_carry = _carry.move_toward(Vector3.ZERO, Settings.m2v(CARRY_DRAG_M) * delta)


# ---------------- glider ----------------

func open_glider() -> void:
	if gliding:
		return
	gliding = true
	stats["glides"] += 1
	_gs = clampf(Settings.v2m(p.velocity.length()), Settings.glider_trim_speed_m, Settings.glider_max_speed_m)   # it opens flying
	_heading = p._yaw
	_wing.visible = true


func _close_glider() -> void:
	if not gliding:
		return
	gliding = false
	_wing.visible = false
	_carry = Vector3(_glide_v.x, 0.0, _glide_v.z)   # the glide's speed carries on
	roll = 0.0
	p._apply_look()


## Glide physics in metres: the path follows the camera's pitch less the glide angle; gravity along
## the path speeds a dive and slows a climb; drag (k v^2) settles the speed at trim when looking level.
func _glide(delta: float) -> void:
	var scraped := p.body.blocked.x > 0.0 or p.body.blocked.z > 0.0
	if scraped:
		_gs *= 0.8   # a crown, a lip or a wall: a scrape costs a fifth a step, a head-on stop takes a few
	var glide_angle := atan(1.0 / Settings.glider_glide_ratio)
	var k := G_M * sin(glide_angle) / (Settings.glider_trim_speed_m * Settings.glider_trim_speed_m)
	var climb_max := 0.5 * clampf((_gs - STALL_M) / 6.0, 0.0, 1.0)   # slow, it cannot climb
	var gamma := clampf(p._pitch, -1.35, climb_max) - glide_angle
	if _gs < STALL_M:
		gamma = minf(gamma, -0.5)   # stalled: the nose drops until it has speed again
	_gs = clampf(_gs + (-G_M * sin(gamma) - k * _gs * _gs) * delta, 2.0, Settings.glider_max_speed_m)
	var turn := wrapf(p._yaw - _heading, -PI, PI)
	_heading += turn * (1.0 - exp(-GLIDE_TURN * delta))
	roll = lerpf(roll, clampf(turn * 0.8, -ROLL_MAX, ROLL_MAX), 1.0 - exp(-6.0 * delta))
	var v := Vector3(-sin(_heading) * cos(gamma), sin(gamma), -cos(_heading) * cos(gamma)) * Settings.m2v(_gs)
	if scraped:
		v.y = maxf(v.y, Settings.m2v(2.0))   # ride up over a lip instead of snagging on it
	p.velocity = v
	_glide_v = v
	# a landing hurts by its downward speed, not by the height glided down
	var down := maxf(0.0, -v.y)
	p._fall_peak_y = p.position.y + down * down / (2.0 * Settings.m2v(Settings.gravity_m))
	_wing.rotation = Vector3(-PI / 2 + gamma, wrapf(_heading - p._yaw, -PI, PI), -roll * 1.5)
	p._apply_look()
	_wind_at(_gs)


func _wind_at(speed_m: float) -> void:
	if _wind.stream == null:
		return
	if speed_m < 3.0:
		if _wind.playing:
			_wind.stop()
		return
	var f := clampf((speed_m - 3.0) / 22.0, 0.0, 1.0)
	_wind.volume_db = lerpf(-26.0, -4.0, f)
	_wind.pitch_scale = lerpf(0.8, 1.5, f)
	if not _wind.playing:
		_wind.play()


# ---------------- climbing rope ----------------

## Hangs a rope from the aimed face: on a top face it goes out along the look to the first column
## with no ground under it (a cliff's edge, 1.5 m at most); on a wall or a ceiling it hangs there.
func hang_rope(hit: VoxelRaycastResult, look: Vector3) -> bool:
	if hit == null:
		return false
	var col: Vector3i = hit.previous_position
	var n := col - hit.position
	var wall := -Vector3(n)                     # the rope lies against this side of its column
	var s := Vector3i.ZERO
	var lip := 0
	if n == Vector3i.UP:
		s = Vector3i(signi(int(signf(look.x))), 0, 0) if absf(look.x) > absf(look.z) else Vector3i(0, 0, signi(int(signf(look.z))))
		wall = -Vector3(s)
		var found := false
		var crest := col.y   # the highest ground on the way out: the climber has to get over it at the top
		var bump := int(Settings.m2v(1.0))
		for _i in int(Settings.m2v(1.5)):
			for _u in bump:   # a real rim is bumpy: over a lip of up to a metre
				if not _solid(col):
					break
				col.y += 1
			crest = maxi(crest, col.y)
			if _solid(col):
				return _fail("Something is in the way")
			var dip := 0   # and dips: a hollow under a metre is walked through, a drop is the edge
			while dip < int(Settings.m2v(1.0)) and not _solid(col + Vector3i.DOWN * (dip + 1)):
				dip += 1
			if dip >= int(Settings.m2v(1.0)):
				found = true
				break
			col += s + Vector3i.DOWN * dip
		if not found:
			return _fail("Hang a rope over an edge")
		lip = crest - col.y
	elif n == Vector3i.DOWN:
		wall = Vector3.ZERO
	if _solid(col):
		return _fail("Something is in the way")
	var bottom := _rope_bottom(col)
	var stake := Vector3(col) + Vector3(0.5, 0.0, 0.5) + wall * 0.35
	var c := stake
	# a real cliff is not a wall: the column at the rim meets a ledge or the slope soon. Out to 0.75 m
	# past the rim the rope hangs where it falls furthest (a short piece back to its stake)
	for k in range(1, int(Settings.m2v(0.75)) + 1):
		var c2 := col + s * k
		if s == Vector3i.ZERO or _solid(c2):
			break
		var b2 := _rope_bottom(c2)
		if b2 < bottom - int(Settings.m2v(0.5)):
			bottom = b2
			c = Vector3(c2) + Vector3(0.5, 0.0, 0.5)
	if col.y - bottom < int(Settings.m2v(1.0)):
		return _fail("No drop to hang a rope down")
	var top := float(col.y + lip) + (Settings.m2v(0.3) if n == Vector3i.UP else 0.9)
	var r := {"x": c.x, "z": c.z, "top": top, "bottom": float(bottom), "ax": hit.position.x, "ay": hit.position.y, "az": hit.position.z}
	if c != stake:
		r["sx"] = stake.x
		r["sz"] = stake.z
	add_rope(r)
	Sound.play("craft/assemble", c + Vector3(0, top - c.y, 0))
	stats["ropes_hung"] += 1
	return true


## Where a rope hanging in column `col` (from its top cell down) meets the ground, rope_max_m at most.
func _rope_bottom(col: Vector3i) -> int:
	var bottom := col.y
	var lowest := col.y - int(Settings.m2v(Settings.rope_max_m))
	while bottom > lowest and not _solid(Vector3i(col.x, bottom - 1, col.z)):
		bottom -= 1
	return bottom


func add_rope(r: Dictionary) -> void:
	ropes.append(r)
	var holder := Node3D.new()
	holder.name = "Rope"
	add_child(holder)
	var mi := _mesh(_cylinder(ROPE_RADIUS_M), "Line")
	mi.reparent(holder)
	var h := float(r["top"]) - float(r["bottom"])
	mi.transform = Transform3D(Basis.from_scale(Vector3(1.0, h, 1.0)), Vector3(r["x"], float(r["bottom"]) + h * 0.5, r["z"]))
	mi.visible = true
	if r.has("ax"):   # the voxel it was hung on: dug away, the rope falls (_drop_loose_ropes)
		r["anchor"] = Vector3i(int(r["ax"]), int(r["ay"]), int(r["az"]))
	if r.has("sx"):   # the piece from the stake on the rim out to where it hangs
		var a := Vector3(r["sx"], r["top"], r["sz"])
		var v := Vector3(r["x"], r["top"], r["z"]) - a
		var arm := _mesh(_cylinder(ROPE_RADIUS_M), "Arm")
		arm.reparent(holder)
		var y := v.normalized()
		var x := y.cross(Vector3.UP).normalized()
		arm.transform = Transform3D(Basis(x, y * v.length(), x.cross(y)), a + v * 0.5)
		arm.visible = true
	_rope_nodes.append(holder)


## A rope whose anchor voxel no longer holds a body (dug, blasted, burnt) falls: the item drops at its foot.
func _drop_loose_ropes() -> void:
	for i in range(ropes.size() - 1, -1, -1):
		var r: Dictionary = ropes[i]
		if r.has("anchor") and not _solid(r["anchor"]):
			p.item_dropped.emit(Inventory.make_stack(p.inventory.items, "climbing_rope", 1), Vector3(r["x"], r["bottom"] + 1.0, r["z"]), Vector3.ZERO)
			_remove_rope(i)


func _remove_rope(i: int) -> void:
	ropes.remove_at(i)
	_rope_nodes[i].queue_free()
	_rope_nodes.remove_at(i)
	climbing = false


func take_rope(i: int) -> void:
	_remove_rope(i)
	if GameMode.rule(p.game_mode, "consumes_items"):
		p.inventory.pickup("climbing_rope", 1)
	climbing = false


## The rope the aim ray passes within reach of (closer than `max_dist`), or -1.
func rope_at(from: Vector3, dir: Vector3, max_dist: float) -> int:
	var best := -1
	var best_d := max_dist
	for i in ropes.size():
		var r: Dictionary = ropes[i]
		var flat := Vector2(dir.x, dir.z)
		var to := Vector2(r["x"] - from.x, r["z"] - from.z)
		if flat.length_squared() < 1e-6:
			continue
		var t := to.dot(flat) / flat.length_squared()   # ray parameter of the closest approach (dir is unit)
		if t <= 0.0 or t > best_d + Settings.m2v(0.5):   # a rope lies against the face the aim hits
			continue
		var y := from.y + dir.y * t
		if (flat * t - to).length() < Settings.m2v(0.25) and y >= float(r["bottom"]) and y <= float(r["top"]) + 1.0:
			best = i
			best_d = minf(best_d, t)
	return best


func _near_rope() -> int:
	var reach := Settings.m2v(Settings.player_radius_m + ROPE_GRAB_M) + _push
	for i in ropes.size():
		var r: Dictionary = ropes[i]
		if Vector2(r["x"] - p.position.x, r["z"] - p.position.z).length() <= reach \
				and p.position.y >= float(r["bottom"]) - 1.0 and p.position.y <= float(r["top"]) + Settings.m2v(0.6):
			return i
	return -1


func _climb(delta: float, dir: Vector3, input: Vector2, jump_edge: bool) -> void:
	var i := -1 if _let_go_t > 0.0 or p.in_liquid else _near_rope()
	var fwd := -input.y
	if i >= 0:
		var r: Dictionary = ropes[i]
		var at_top := p.position.y >= float(r["top"]) - Settings.m2v(0.6)
		if p.is_on_floor() and (fwd <= 0.0 or at_top):
			i = -1   # standing at its foot or on the ledge beside its top
	climbing = i >= 0
	if not climbing:
		_push = 0.0
		return
	var r: Dictionary = ropes[i]
	var to := Vector3(r["x"] - p.position.x, 0.0, r["z"] - p.position.z)
	if jump_edge:
		_let_go_t = 0.5
		climbing = false
		p.velocity.y = Settings.m2v(4.0)
		_carry = -to.normalized() * Settings.m2v(3.0)   # off the rope, away from it
		return
	var climb := Settings.m2v(Settings.climb_speed_m)
	p.velocity.y = fwd * climb   # nothing pressed: hang
	if p.position.y >= float(r["top"]) - Settings.m2v(0.9) and fwd > 0.0:
		p.velocity.y = climb   # over the top: keep walking (dir) onto the ledge
	else:
		# hold the body at the rope: a spring to its side, the walk turned into sideways shuffles only
		# a lip overhead: lean out from the face until the head clears it, then back in
		if fwd > 0.0 and p.body.hit_ceiling:
			_push = minf(_push + Settings.m2v(1.5) * delta, Settings.m2v(0.8))
		elif not p.body.hit_ceiling:
			_push = maxf(0.0, _push - Settings.m2v(0.3) * delta)
		var hold := Settings.m2v(Settings.player_radius_m) + _push
		var pull := to - to.normalized() * hold if to.length() > hold else Vector3.ZERO
		p.velocity.x = pull.x * 6.0 + dir.x * Settings.m2v(0.5)
		p.velocity.z = pull.z * 6.0 + dir.z * Settings.m2v(0.5)
	p._fall_peak_y = p.position.y


# ---------------- grappling hook ----------------

func fire_hook(from: Vector3, dir: Vector3) -> void:
	hook = "flying"
	_hook_pos = from
	_hook_dir = dir.normalized()
	_hook_flown = 0.0
	Sound.play("toy/hook_throw")


func unhook() -> void:
	hook = ""
	reeling = false
	_line_node.visible = false
	_hook_node.visible = false


func _holds(m: int) -> bool:
	if not GameMaterials.blocks_bodies(m) or GameMaterials.is_powder(m):
		return false
	var n := GameMaterials.name_of(m)
	for part in HOOK_HOLD:
		if part in n:
			return false
	return true


func _fly_hook(delta: float) -> void:
	var step := Settings.m2v(Settings.grapple_speed_m) * delta
	var hit: VoxelRaycastResult = main.world.raycast(_hook_pos, _hook_dir, step, VoxelBody.MASK_SOLID)
	if hit:
		var m: int = main.get_world_voxel(hit.position)
		if _holds(m):
			hook = "caught"
			stats["hooks_caught"] += 1
			_anchor = _hook_pos + _hook_dir * hit.distance
			_anchor_voxel = hit.position
			_hook_pos = _anchor
			_line = (_anchor - _tie()).length()
			_v = p.velocity
			Sound.play_material("hit", m, hit.position)
		else:
			stats["hooks_slipped"] += 1
			_fail("The hook slips off the %s" % GameMaterials.name_of(m).replace("_", " "))
			unhook()
		return
	_hook_pos += _hook_dir * step
	_hook_flown += step
	if _hook_flown > Settings.m2v(Settings.grapple_range_m):
		unhook()


## Where the line ties to the body: the chest.
func _tie() -> Vector3:
	return p.position + Vector3(0, Settings.m2v(1.2), 0)


## The hook holds: the body keeps its own velocity (so a swing carries on), gravity and WASD act on
## it, held right click shortens the line and pulls; the line never stretches.
func _swing(delta: float, dir: Vector3) -> void:
	if p.body.blocked.x > 0.0:
		_v.x = 0.0
	if p.body.blocked.z > 0.0:
		_v.z = 0.0
	if (p.body.on_floor and _v.y < 0.0) or (p.body.hit_ceiling and _v.y > 0.0):
		_v.y = 0.0
	reeling = p.autopilot_hold or (not p.autopilot and not p.ui_open and Input.is_action_pressed("action_place"))
	var to := _anchor - _tie()
	var d := to.length()
	var in_dir := to / maxf(d, 0.001)
	if p.is_on_floor() and not reeling:
		_v = p.velocity   # on the ground with the line slack: walking as usual
	else:
		_v.y -= Settings.m2v(G_M) * delta
		_v += dir * Settings.m2v(SWING_PUMP_M) * delta
		_v *= 1.0 - 0.05 * delta   # air drag, so a swing dies down at last
	var reel := Settings.m2v(Settings.grapple_reel_m)
	if reeling:
		_line = maxf(Settings.m2v(1.5), minf(_line, d) - reel * delta)
	# taut: where the step would end is pulled back onto the line's sphere, and the velocity is what
	# reaches it (position-based: a swing keeps its energy, a stretch never builds up)
	var q := _tie() + _v * delta
	if (q - _anchor).length() > _line:
		q = _anchor + (q - _anchor).normalized() * _line
		_v = (q - _tie()) / delta
	var inward := _v.dot(in_dir)
	if reeling and inward > reel:
		_v -= in_dir * (inward - reel)   # the reel pulls no faster than it winds
	var cap := Settings.m2v(40.0)
	if _v.length() > cap:
		_v = _v.normalized() * cap
	p.velocity = _v
	p._fall_peak_y = p.position.y


func _let_go_with_swing() -> void:
	_carry = Vector3(_v.x, 0.0, _v.z)
	p.velocity.y = maxf(_v.y, 0.0) + Settings.m2v(2.0)
	unhook()


## The line and the hook follow the frame, not the physics step.
func _process(_delta: float) -> void:
	if hook == "":
		return
	var cam := p.camera
	var b := cam.global_transform.basis
	var hand := cam.global_position + b.x * Settings.m2v(0.25) - b.y * Settings.m2v(0.25) - b.z * Settings.m2v(0.3)
	if p.perspective != 0:
		hand = _tie() + p.global_transform.basis.x * Settings.m2v(0.3)   # seen from outside: from the body's hand
	var v := _hook_pos - hand
	_hook_node.position = _hook_pos
	_hook_node.visible = true
	if v.length() < 0.05:
		_line_node.visible = false
		return
	var y := v.normalized()
	var x := y.cross(Vector3.UP if absf(y.y) < 0.99 else Vector3.RIGHT).normalized()
	_line_node.global_transform = Transform3D(Basis(x, y * v.length(), x.cross(y)), hand + v * 0.5)
	_line_node.visible = true


# ---------------- helpers ----------------

func _solid(v: Vector3i) -> bool:
	return GameMaterials.blocks_bodies(main.get_world_voxel(v))


func _fail(text: String) -> bool:
	print("toys: " + text)
	if p.hud:
		p.hud.show_toast(text)
	return false


## The hung ropes, for the player's save (the anchor as its three numbers).
func to_array() -> Array:
	var a := []
	for r in ropes:
		var d: Dictionary = r.duplicate()
		d.erase("anchor")
		a.append(d)
	return a


func load_array(a: Array) -> void:
	for r in a:
		if r is Dictionary and r.has_all(["x", "z", "top", "bottom"]):
			var d := {"x": float(r["x"]), "z": float(r["z"]), "top": float(r["top"]), "bottom": float(r["bottom"])}
			for k in ["sx", "sz", "ax", "ay", "az"]:
				if r.has(k):
					d[k] = float(r[k])
			add_rope(d)


func snapshot() -> Dictionary:
	return {"gliding": gliding, "glide_speed_m": snappedf(_gs, 0.1), "climbing": climbing, "hook": hook, "reeling": reeling,
		"line_m": snappedf(Settings.v2m(_line), 0.1), "ropes": ropes.size(), "stats": stats}
