class_name Contraptions
extends Node

## Moving contraptions (ADR 0160): lifts, drawbridges and cranes, moved as rigid pieces. A drive
## (ContraptionEntity) rests with its part as plain voxels in the world. When it has power and somewhere to go,
## the part (the seed's material, 6-connected, inside the kind's box) is cut out as a *driven* RigidPieces piece:
## FallingPieces draws it like any piece, the native step takes its pose from here instead of the physics, and at
## the end it is pasted back into voxels (RigidPieces.release). No second physics path: the crane's cargo,
## let go, simply becomes an ordinary falling piece and settles itself.
##
## Bodies: the player, mobs and dropped items standing on a moving part ride it (each physics frame they follow
## the drawn part's transform onto its top). A part that would push the player into terrain, or run into the
## player or a mob, stops for that tick (a mob in the way gets hurt); it goes on when the way is clear.
##
## Ticked by FallingPieces.tick between collect() and queue_step(), when the driver thread is idle.
## shortcut: a part only moves inside the main sim window (rigid pieces live there), and riding uses the part's
## bounding box, not each voxel; per-voxel tops and parts that leave the window need native support first.

const TOP_BAND := Vector2(-2.0, 1.5)   # voxels below / above a part's top that still stand on it
const RIDE_TILT := 0.7                 # a part tilted further than this (its up . world up) throws riders off
const CRUSH_EVERY_S := 0.5

var main: Node
var fp: FallingPieces
var stats := {"started": 0, "arrived": 0, "stalls": 0, "refused": 0, "cargo": 0, "rides": 0}
var _drives: Array = []
var _scan := 0
var _probe := VoxelBody.new(AABB())
var _last := {}             # piece id -> its drawn transform the last physics frame
var _crush_t := 0.0
var _hook: MeshInstance3D   # the crane's rope and hook, while one works
var _rope: MeshInstance3D


func setup(p_main: Node) -> void:
	main = p_main
	fp = main.pieces
	process_physics_priority = 100   # after the player, mobs and items moved: riders end on the part
	if main.toys:   # the climbing rope's warmed material and the grappling hook (ADR 0151): no new pipeline
		_rope = MeshInstance3D.new()
		var c := CylinderMesh.new()
		c.top_radius = 0.25
		c.bottom_radius = 0.25
		c.height = 1.0
		c.material = main.toys._rope_mat
		_rope.mesh = c
		_hook = MeshInstance3D.new()
		_hook.mesh = main.toys._hook_node.mesh
		_hook.scale = Vector3.ONE * 2.5
		for n in [_rope, _hook]:
			n.visible = false
			n.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
			add_child(n)


func busy() -> bool:
	for e in _drives:
		if e.piece != 0:
			return true
	return false


# ---------------- geometry (world voxels; the home frame is the world with the drive at pose 0) ----------------

static func _f(e: ContraptionEntity) -> Vector3:
	return Vector3(Transport.dir_of(e.facing)) / Settings.BLOCK


static func _centre(e: ContraptionEntity) -> Vector3:
	return Vector3(e.pos) + Vector3(Settings.BLOCK * 0.5, 0.0, Settings.BLOCK * 0.5)


## The pose: home frame -> world.
static func pose_of(e: ContraptionEntity, v: float) -> Transform3D:
	var B := Settings.BLOCK
	match e.kind:
		"lift_winch":
			return Transform3D(Basis(), Vector3(0.0, v, 0.0))
		"drawbridge_hinge":   # about the top edge of the drive's front face: the span's far end rises
			var f := _f(e)
			var pivot := _centre(e) + f * (B * 0.5) + Vector3(0.0, B, 0.0)
			var r := Basis(f.cross(Vector3.UP), deg_to_rad(v))
			return Transform3D(Basis(), pivot) * Transform3D(r, Vector3.ZERO) * Transform3D(Basis(), -pivot)
		_:   # crane: about the drive's middle column
			var c := _centre(e)
			return Transform3D(Basis(), c) * Transform3D(Basis(Vector3.UP, deg_to_rad(v)), Vector3.ZERO) * Transform3D(Basis(), -c)


## The end the lever names (the lift's top is found on the way: wherever it meets something).
static func end_of(e: ContraptionEntity) -> float:
	if not e.lever:
		return 0.0
	return Settings.m2v(Settings.lift_max_m) if e.kind == "lift_winch" else 90.0


## {seed, lo, hi} at home: where the part starts and the box it must keep inside.
static func part_box(e: ContraptionEntity) -> Dictionary:
	var B := Settings.BLOCK
	var c := _centre(e)
	if e.kind == "crane":   # the jib, built up from the top of the drive
		var reach := Settings.m2v(6.0)
		return {"seed": Vector3i(c.floor()) + Vector3i(0, B, 0), "scan_up": true,
			"lo": Vector3i(int(c.x - reach), e.pos.y + B, int(c.z - reach)), "hi": Vector3i(int(c.x + reach) - 1, e.pos.y + B + Settings.m2v(4.0), int(c.z + reach) - 1)}
	var f := _f(e)
	var side := f.cross(Vector3.UP) * Settings.m2v(1.5)
	var out := Settings.m2v(3.0 if e.kind == "lift_winch" else 6.0)
	var a := c + f * (B * 0.5) - side
	var b := c + f * (B * 0.5 + out) + side
	var lo := Vector3i(a.min(b).round())
	var hi := Vector3i(a.max(b).round()) - Vector3i.ONE
	return {"seed": Vector3i((c + f * B).floor()), "scan_up": true, "lo": Vector3i(lo.x, e.pos.y, lo.z), "hi": Vector3i(hi.x, e.pos.y + B - 1, hi.z)}


## The world box of a grid of `size` under transform `t`.
static func world_box(t: Transform3D, size: Vector3i) -> AABB:
	return t * AABB(Vector3.ZERO, Vector3(size))


# ---------------- the tick ----------------

func tick(dt: float) -> void:
	_scan -= 1
	if _scan <= 0:   # the drives, twice a second (all_of walks every entity)
		_scan = 15
		var now := []
		for k in ContraptionEntity.KINDS:
			now.append_array(main.blocks.all_of(k))
		for e: ContraptionEntity in _drives:
			if not e in now:   # broken while it moved: the part drops where it is, the cargo falls
				if e.piece != 0:
					fp.rp.release(e.piece, false)
				if e.cargo != 0:
					fp.rp.release(e.cargo, false)
				e.piece = 0
				e.cargo = 0
		_drives = now
	_crush_t -= dt
	for e: ContraptionEntity in _drives:
		if e.piece == 0 and (e.arrived or not e.load.on):
			continue
		if e.piece == 0 and not _start(e):
			continue
		if e.load.on:
			_advance(e, dt)
	if _crush_t <= 0.0:
		_crush_t = CRUSH_EVERY_S


func _say(e: ContraptionEntity, msg: String) -> void:
	e.status = msg
	e.changed.emit()


## Cuts the part out as a driven piece. False (and the drive stops asking for power) when there is none or it
## is too big: the status line says why.
func _start(e: ContraptionEntity) -> bool:
	var box := part_box(e)
	var now := pose_of(e, e.pose)
	var seed: Vector3i = box["seed"]
	var found := false
	for k in Settings.BLOCK:   # the part may sit anywhere in the block's height
		var s := Vector3i((now * (Vector3(seed + Vector3i(0, k, 0)) + Vector3.ONE * 0.5)).floor())
		if main.get_world_voxel(s) > 0:
			seed = s
			found = true
			break
	if not found:
		return _refuse(e, "Nothing built %s it to move." % ("on top of" if e.kind == "crane" else "in front of"))
	var wb := now * AABB(Vector3(box["lo"]), Vector3(box["hi"] - box["lo"] + Vector3i.ONE))
	var r: Dictionary = fp.rp.lift(seed, Vector3i(wb.position.round()), Vector3i(wb.end.round()) - Vector3i.ONE, Settings.contraption_max_cells, false)
	if not r.has("id"):
		return _refuse(e, {"edge": "The part runs on past its reach (%s m): build it of a material the floor around it is not." % ("6" if e.kind != "lift_winch" else "3"),
			"big": "The part is too big to move.", "outside": "Too far away to move."}.get(r.get("error", ""), "Nothing to move."))
	e.piece = int(r["id"])
	e.size = r["size"]
	e.base = now.affine_inverse() * Transform3D(Basis(), r["origin"])
	e.stage = ""
	if e.kind == "crane":   # the jib's underside at its far end: the first piece cell up from under the box there
		e.tip_y = -INF
		var b := world_box(e.base, e.size)
		var tip := _tip_home(e)
		var hit: Dictionary = fp.rp.raycast(now * Vector3(tip.x, b.position.y - 1.0, tip.z), Vector3.UP, float(e.size.y) + 2.0)
		e.tip_y = (now.affine_inverse() * hit["position"]).y if hit.has("position") else tip.y
	stats["started"] += 1
	if e.kind == "crane" and e.lever and e.pose == 0.0:
		_hook_cargo(e)
	return true


func _refuse(e: ContraptionEntity, msg: String) -> bool:
	e.arrived = true
	stats["refused"] += 1
	_say(e, msg)
	return false


## The crane's hook: the wood straight under the jib's tip (a log, a cut piece), lifted up under the jib.
func _hook_cargo(e: ContraptionEntity) -> void:
	var tip := _tip(e, 0.0)
	for d in Settings.m2v(Settings.crane_hook_m):
		var p := Vector3i(tip.floor()) - Vector3i(0, d + 1, 0)
		var m: int = main.get_world_voxel(p)
		if m == 0:
			continue
		var reach := Settings.m2v(1.5)
		var r: Dictionary = fp.rp.lift(p, p - Vector3i(reach, Settings.m2v(3.0), reach), p + Vector3i(reach, 0, reach), Settings.contraption_max_cells, true)
		if r.has("id"):
			e.cargo = int(r["id"])
			e.cargo_size = r["size"]
			e.cargo_base = Transform3D(Basis(), r["origin"])
			e.cargo_up = 0.0
			e.stage = "raise"
			stats["cargo"] += 1
		else:
			e.status = "The hook found no loose wood under the jib." if r.get("error", "") == "nothing" else "The wood under the hook is too big or still grows on."
		return


func _tip(e: ContraptionEntity, v: float) -> Vector3:
	return pose_of(e, v) * _tip_home(e)


## The jib's tip at home: the far end of its box along the facing, at its underside.
func _tip_home(e: ContraptionEntity) -> Vector3:
	var b := world_box(e.base, e.size)
	var f := _f(e)
	var c := _centre(e)
	var reach := 0.0
	for i in 8:
		reach = maxf(reach, (b.get_endpoint(i) - c).dot(f))
	var mid := b.get_center()
	var home := c + f * (reach - 0.5)
	home += (mid - c) - f * (mid - c).dot(f)
	home.y = e.tip_y if e.tip_y > -INF else b.position.y
	return home


## The cargo at pose v, `up` voxels above where the hook took it.
func _cargo_t(e: ContraptionEntity, v: float, up: float) -> Transform3D:
	return pose_of(e, v) * Transform3D(Basis(), Vector3(0.0, up, 0.0)) * e.cargo_base


func _advance(e: ContraptionEntity, dt: float) -> void:
	var cur := pose_of(e, e.pose) * e.base
	if e.stage == "raise" or e.stage == "lower":
		var step := Settings.m2v(Settings.contraption_lift_mps) * dt
		var raise := e.stage == "raise"
		var target := maxf(_tip(e, 0.0).y - 1.0 - (e.cargo_base.origin.y + e.cargo_size.y), 0.0)   # just under the jib
		var nu := move_toward(e.cargo_up, target, step) if raise else e.cargo_up - step
		var was := _cargo_t(e, e.pose, e.cargo_up)
		var nt := _cargo_t(e, e.pose, nu)
		if _blocked_bodies(nt, e.cargo_size, was):
			return
		var hit := not raise and (_hits_terrain(world_box(was, e.cargo_size), Vector3.DOWN, step) or e.cargo_up < -Settings.m2v(Settings.crane_hook_m))
		if not hit:
			e.cargo_up = nu
			fp.rp.drive(e.cargo, nt)
		if raise and nu == target:
			e.stage = ""
		elif hit:   # set down: an ordinary rigid piece again, it settles by itself
			fp.rp.release(e.cargo, false)
			e.cargo = 0
			e.stage = ""
			_finish(e)
		return
	var end := end_of(e)
	var speed := Settings.m2v(Settings.contraption_lift_mps) if e.kind == "lift_winch" else Settings.contraption_swing_dps
	var next := move_toward(e.pose, end, speed * dt)
	var arrive := next == end
	if e.kind == "lift_winch":
		var dir := signf(next - e.pose)
		if dir != 0.0 and _hits_terrain(world_box(cur, e.size), Vector3(0, dir, 0), absf(next - e.pose)):
			next = floorf(e.pose) if dir > 0.0 else ceilf(e.pose)   # stop on a whole voxel against what it met
			arrive = true
	var nt := pose_of(e, next) * e.base
	if _blocked_bodies(nt, e.size, cur):
		return
	if e.cargo != 0 and _blocked_bodies(_cargo_t(e, next, e.cargo_up), e.cargo_size, _cargo_t(e, e.pose, e.cargo_up)):
		return
	e.pose = next
	fp.rp.drive(e.piece, nt)
	if e.cargo != 0:
		fp.rp.drive(e.cargo, _cargo_t(e, next, e.cargo_up))
	if arrive:
		if e.cargo != 0:
			e.stage = "lower"
		else:
			_finish(e)


func _finish(e: ContraptionEntity) -> void:
	fp.rp.release(e.piece, true)
	_last.erase(e.piece)
	e.piece = 0
	e.arrived = true
	e.status = ""
	stats["arrived"] += 1
	e.changed.emit()


## Does the box moved `dist` voxels along `dir` (an axis) run into terrain? Only the swept slab ahead of it is
## tested: the part's own voxels may still show in the terrain for a frame or two after the cut.
func _hits_terrain(b: AABB, dir: Vector3, dist: float) -> bool:
	var shrink := 0.02
	var slab := AABB(b.position + Vector3.ONE * shrink, b.size - Vector3.ONE * 2.0 * shrink)
	if dir.y > 0.0:
		slab = AABB(Vector3(slab.position.x, b.end.y, slab.position.z), Vector3(slab.size.x, dist, slab.size.z))
	else:
		slab = AABB(Vector3(slab.position.x, b.position.y - dist, slab.position.z), Vector3(slab.size.x, dist, slab.size.z))
	_probe.aabb = slab
	return _probe.intersects(Vector3.ZERO, main.world.terrain)


## Would the part at `nt` (from `cur`) run into the player or a mob, or push a rider into terrain? Then it waits.
func _blocked_bodies(nt: Transform3D, size: Vector3i, cur: Transform3D) -> bool:
	var p: Node3D = main.player
	if not _rides(cur, size, p.position):
		if _overlaps(nt, size, p.position, p.body.aabb):
			return _stall("player")
	else:
		var to := _carried(cur, nt, size, p.position)
		if p.body.intersects(to, main.world.terrain):
			return _stall("ceiling")
	if main.mobs:
		for m: Mob in main.mobs.active():
			if not _rides(cur, size, m.position) and _overlaps(nt, size, m.position, m._body.aabb):
				if _crush_t <= 0.0:
					m.hurt(2, nt * (Vector3(size) * 0.5))
				return _stall("mob")
	return false


func _stall(_why: String) -> bool:
	stats["stalls"] += 1
	return true


## Feet at `feet` stand on the part (grid `size` under `t`)?
static func _rides(t: Transform3D, size: Vector3i, feet: Vector3) -> bool:
	if t.basis.y.normalized().y < RIDE_TILT:
		return false
	var l := t.affine_inverse() * feet
	return l.x >= 0.0 and l.z >= 0.0 and l.x <= size.x and l.z <= size.z and l.y >= size.y + TOP_BAND.x and l.y <= size.y + TOP_BAND.y


## Where feet riding the part go when it moves from `from` to `to`: the same spot on its top.
static func _carried(from: Transform3D, to: Transform3D, size: Vector3i, feet: Vector3) -> Vector3:
	var l := from.affine_inverse() * feet
	return to * Vector3(l.x, size.y, l.z)


## A body's box (`box` local to `at`) overlaps the part's grid under `t` (the body's box grown by its half size
## in the part's frame: generous for a turned part, which only stops a little early).
static func _overlaps(t: Transform3D, size: Vector3i, at: Vector3, box: AABB) -> bool:
	var half := box.size * 0.5
	var l := t.affine_inverse() * (at + box.get_center())
	var r := half.length()
	var ex := Vector3(r, r, r) if t.basis.y.normalized().y < 0.999 else half
	return l.x > -ex.x and l.y > -ex.y and l.z > -ex.z and l.x < size.x + ex.x and l.y < size.y + ex.y and l.z < size.z + ex.z


# ---------------- riders, every physics frame ----------------

func _physics_process(_delta: float) -> void:
	var parts := []
	for e: ContraptionEntity in _drives:
		if e.piece != 0:
			parts.append([e.piece, e.size])
		if e.cargo != 0:
			parts.append([e.cargo, e.cargo_size])
	_draw_hook()
	if parts.is_empty():
		return
	var p: Node3D = main.player
	for pa in parts:
		var node: Node3D = fp._pieces.get(pa[0])
		if node == null:
			continue
		var now := node.transform
		var was: Transform3D = _last.get(pa[0], now)
		_last[pa[0]] = now
		if _rides(was, pa[1], p.position) and not p.noclip:
			var to := _carried(was, now, pa[1], p.position)
			if not p.body.intersects(to, main.world.terrain):
				p.position = to
				p.velocity.y = maxf(p.velocity.y, 0.0)
				p._on_floor = true
				stats["rides"] += 1
		if main.mobs:
			for m: Mob in main.mobs.active():
				if _rides(was, pa[1], m.position):
					m.position = _carried(was, now, pa[1], m.position)
					m.velocity.y = maxf(m.velocity.y, 0.0)
		for it: ItemEntity in main._item_entities:
			if is_instance_valid(it) and _rides(was, pa[1], it.position):
				it.position = _carried(was, now, pa[1], it.position)
				it.velocity.y = maxf(it.velocity.y, 0.0)
				it.sleeping = false


## The crane's rope from the jib's tip to its cargo (or a short hook when it swings empty).
func _draw_hook() -> void:
	if _rope == null:
		return
	var crane: ContraptionEntity = null
	for e: ContraptionEntity in _drives:
		if e.kind == "crane" and e.piece != 0:
			crane = e
	_rope.visible = crane != null
	_hook.visible = crane != null
	if crane == null:
		return
	var node: Node3D = fp._pieces.get(crane.piece)
	var drawn: Transform3D = node.transform * crane.base.affine_inverse() if node else pose_of(crane, crane.pose)
	var tip := drawn * _tip_home(crane)
	var low := tip - Vector3(0, Settings.m2v(1.0), 0)
	var cn: Node3D = fp._pieces.get(crane.cargo)
	if cn:
		low = Vector3(tip.x, (cn.transform * (Vector3(crane.cargo_size) * Vector3(0.5, 1.0, 0.5))).y, tip.z)
	var len := maxf(tip.y - low.y, 1.0)
	_rope.transform = Transform3D(Basis().scaled(Vector3(1.0, len, 1.0)), (tip + low) * 0.5)
	_hook.position = low


# ---------------- debug ----------------

## /contraptions (ADR 0160): one of each on a stone floor `base` ahead, the part facing `facing`, a hand crank
## turning behind each drive: a 1.5 m plank lift under a stone ring 3.5 m up (where it stops), a 4 m plank
## drawbridge over a dug moat, and a crane with a plank mast and 3 m jib over an oak log. Levers off (home).
## Returns {laid, lift (its platform's top centre), look}.
func debug_build(base: Vector3i, facing: int) -> Dictionary:
	var B := Settings.BLOCK
	var fw := Vector3(Transport.dir_of(facing)) / B
	var rt := fw.cross(Vector3.UP)
	var stone := GameMaterials.id_of("stone")
	var planks := GameMaterials.id_of("planks")
	var bubble: SimBubble = main.bubble
	var at := func(r: float, u: float, f: float) -> Vector3:
		return Vector3(base) + rt * r + Vector3.UP * u + fw * f
	var failed := [0]
	var box := func(a: Vector3, b: Vector3, m: int) -> void:   # the cells between two corners (any order)
		if bubble.fill_box(Vector3i(a.min(b).round()), Vector3i(a.max(b).round()), m) < 0:
			failed[0] += 1
	box.call(at.call(-72, -B, -2 * B), at.call(72, 0, 56), stone)   # the floor
	box.call(at.call(-72, 0, -2 * B), at.call(72, 48, 56), 0)       # air above it
	var out := {}
	var slots := {"lift_winch": -56, "drawbridge_hinge": -24, "crane": 40}
	for kind in slots:
		var anchor := Vector3i(at.call(slots[kind], 0, 0).round())
		anchor = BlockEntities.anchor_for(anchor)
		var e: ContraptionEntity = main.work._create(anchor, kind, GameMaterials.WORK_BLOCK)
		e.facing = facing
		var crank: PowerSourceEntity = main.work._create(anchor - Transport.dir_of(facing), "hand_crank", GameMaterials.WORK_BLOCK)
		crank.cranked_until = Inventory.clock_s + 900.0
		var c := _centre(e)
		match kind:
			"lift_winch":   # 12 x 12 x 1 planks, a stone ring over its rim
				var a: Vector3 = c + fw * (B * 0.5) - rt * 6
				var b: Vector3 = c + fw * (B * 0.5 + 12) + rt * 6 + Vector3.UP
				box.call(a, b, planks)
				var ring_y := Vector3.UP * Settings.m2v(3.5)
				box.call(a - rt + ring_y - fw, b + rt + ring_y + fw, stone)
				box.call(a + rt + ring_y + fw, b - rt + ring_y - fw, 0)
				out["lift"] = (a + b) * 0.5 + Vector3(0, 0.5, 0)
			"drawbridge_hinge":   # 8 wide, 32 out, over a moat dug 28 out
				var a: Vector3 = c + fw * (B * 0.5) - rt * 4
				box.call(a - Vector3.UP * B, a + fw * 28 + rt * 8, 0)
				box.call(a, a + fw * 32 + rt * 8 + Vector3.UP, planks)
			"crane":   # a 2 x 2 mast 2 m up, a 2 x 2 jib 3 m out, an oak log under its tip
				var top := c + Vector3.UP * (B + 16)
				box.call(c + Vector3(-1, B, -1), top + Vector3(1, 0, 1), planks)
				box.call(top - Vector3.UP * 2 - fw - rt, top + fw * 24 + rt, planks)
				var under := c + fw * 22
				box.call(under - rt * 8, under + rt * 8 + fw * 4 + Vector3.UP * 4, GameMaterials.id_of("wood"))
	out["laid"] = "a lift, a drawbridge and a crane, each with a hand crank turning (right click a drive, or /contraptions toggle)"
	if failed[0] > 0:
		out["laid"] += "; %d fills were outside the loaded sim window" % failed[0]
	out["look"] = at.call(-40, 16, 10)   # the lift and the drawbridge
	out["cam"] = at.call(-36, 20, -30)
	_scan = 0
	return out


# ---------------- saving ----------------

## Before a save on quit: every moving part goes into the voxels at the end it heads for (a lift where it is, on
## a whole voxel), the crane's cargo where it hangs. True when anything moved.
func settle_all() -> bool:
	var any := false
	for e: ContraptionEntity in _drives:
		if e.cargo != 0:
			fp.rp.release(e.cargo, true)
			e.cargo = 0
		if e.piece == 0:
			continue
		e.pose = roundf(e.pose) if e.kind == "lift_winch" else (90.0 if e.pose >= 45.0 else 0.0)
		fp.rp.drive(e.piece, pose_of(e, e.pose) * e.base)
		e.arrived = false   # it goes on (or back) when the world loads
		fp.rp.release(e.piece, true)
		e.piece = 0
		e.stage = ""
		e.changed.emit()
		any = true
	return any


func summary() -> Dictionary:
	var d := stats.duplicate()
	d["drives"] = _drives.map(func(e: ContraptionEntity) -> Dictionary: return e.summary())
	return d
