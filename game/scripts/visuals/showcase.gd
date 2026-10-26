class_name Showcase
extends RefCounted

## Look check (2026-10-26: "we need a better way to playtest"): sets the named things down side by
## side on a bare pad floating 5 m above the player and films them from fixed angles into one sheet,
## so judging a look takes one run and one image instead of hunting the world for an example.
##   showcase=flint_stones,pebbles,granite_stones [showcase_on=grass] (launch: runs after load, quits)
##   /showcase flint_stones pebbles [on=grass]                         (chat, while playing)
## A name is a material: stone pickups get the generator's own stamp (three patches each, front to
## back), anything else is placed as a click would. Item looks have their own sheet (tools/item_sheet.gd):
## an item id is listed as "item" and not drawn (a dropped item on the pad never showed in the shots).
## Left to right in every shot and in the JSON line "debug showcase {...}": the order given.
## Sheet: front, top, then a close-up of each entry (up to six).

const SLOT := 12          # voxels between entries (1.5 m)
const ROW := 8            # voxels between the variants of a pickup (1 m)
const VARIANTS := 3
const CLOSE_MAX := 6      # close-ups for up to this many entries
const TILE := Vector2i(640, 360)
const COLS := 3

static var _running: Array[Showcase] = []   # holds a run through its awaits (a RefCounted no one keeps dies at the first)

var main: Node
var r := {}


func run(p_main: Node, names: PackedStringArray, ground: String = "grass", quit_after: bool = false) -> void:
	main = p_main
	_running.append(self)
	var tree: SceneTree = main.get_tree()
	main.chat.send("/gamemode creative")
	main.chat.send("/time set noon")
	main.player.flying = true
	main.player.autopilot_action = "look"
	main.player.autopilot_speed_m = 0.0
	var n := names.size()
	var o := Vector3i(main.player.position.floor()) + Vector3i(-SLOT * n / 2, 40, -ROW * VARIANTS - 12)   # the pad's corner, surface at o.y
	var size := Vector3i(SLOT * n + 4, 0, ROW * VARIANTS + 4)
	var pad := GameMaterials.id_of(ground + "_static")   # sand and gravel fall: their still kind holds a pad up
	if pad == 0:
		pad = GameMaterials.id_of(ground)
	if pad == 0:
		pad = GameMaterials.STONE
	for z in size.z + ROW:   # a metre of bare pad in front (+z): the cameras look over it toward -z
		for x in size.x:
			for y in range(-2, 16):
				main.set_world_voxel(o + Vector3i(x, y, z), pad if y < 0 else GameMaterials.AIR)
	var gen: FineGen = main.world.generator.gen
	var placed := []
	for i in n:
		var name := names[i]
		var c := o + Vector3i(2 + SLOT * i + SLOT / 2 - 2, 0, 2)   # the entry's first patch corner
		var id := GameMaterials.id_of(name)
		var how := "missing"
		if id > 0 and (id in GameMaterials.GROUND_PICKUPS or name.ends_with("_stones")) and id != GameMaterials.TWIGS:
			how = "pickup"
			for v in VARIANTS:
				var hs: PackedInt32Array = gen.pickup_heights(id, i * 31 + v * 7 + 1)
				for k in hs.size():
					for y in hs[k]:
						main.set_world_voxel(c + Vector3i(k % FineGen.FS, y, v * ROW + k / FineGen.FS), id)
		elif id > 0:
			how = "placed"
			main._on_placed(c + Vector3i(1, 0, ROW + 1), id)
		elif main.items.has(name):
			how = "item"
		placed.append({"name": name, "as": how})
	r["entries"] = placed
	await tree.create_timer(2.0).timeout   # remesh
	var mid := Vector3(o) + Vector3(size.x / 2.0, 0.5, size.z / 2.0)
	var span := maxf(size.x, 24.0)
	var shots: Array[Image] = []
	# the cameras stand on the +z side looking toward -z, so +x (the order given) runs left to right
	shots.append(await _shot(tree, mid + Vector3(0, 6 + span * 0.15, span * 0.55 + 4), mid))   # front
	shots.append(await _shot(tree, mid + Vector3(0, span * 0.95, span * 0.25), mid))          # top
	if n <= CLOSE_MAX:
		for i in n:
			var e := Vector3(o) + Vector3(2 + SLOT * i + SLOT / 2.0, 1.0, 2 + ROW * (VARIANTS - 1) + 2)   # the front patch
			shots.append(await _shot(tree, e + Vector3(-3, 9, 13), e))
	main.chat.send("/cam off")
	var dir := DebugCapture.out_dir if DebugCapture.autopilot else "res://out/showcase"
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(dir))
	var path := dir + "/showcase.png"
	_sheet(shots).save_png(path)
	r["sheet"] = ProjectSettings.globalize_path(path)
	r["shots"] = ["front", "top"] + (names as Array if n <= CLOSE_MAX else [])
	r["ok"] = placed.all(func(p: Dictionary) -> bool: return p["as"] != "missing")
	print("debug showcase ", JSON.stringify(r))
	_running.erase(self)
	if quit_after:
		tree.quit()


func _shot(tree: SceneTree, at: Vector3, look: Vector3) -> Image:
	main.chat.send("/cam %.1f %.1f %.1f %.1f %.1f %.1f" % [at.x, at.y, at.z, look.x, look.y, look.z])
	await tree.create_timer(0.6).timeout
	var layers: Array = [main.get("game_hud"), main.get("chat")]
	for l in layers:
		if l != null:
			l.visible = false
	await RenderingServer.frame_post_draw
	await RenderingServer.frame_post_draw
	var img: Image = main.get_viewport().get_texture().get_image()
	for l in layers:
		if l != null:
			l.visible = true
	img.resize(TILE.x, TILE.y, Image.INTERPOLATE_BILINEAR)
	return img


func _sheet(shots: Array[Image]) -> Image:
	var rows := ceili(shots.size() / float(COLS))
	var sheet := Image.create(TILE.x * mini(shots.size(), COLS), TILE.y * rows, false, shots[0].get_format())
	for i in shots.size():
		sheet.blit_rect(shots[i], Rect2i(Vector2i.ZERO, TILE), Vector2i(i % COLS * TILE.x, i / COLS * TILE.y))
	return sheet
