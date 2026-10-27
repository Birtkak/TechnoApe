class_name FurnitureEntity
extends RefCounted

## A bed, straw bed (ADR 0030) or chair (ADR 0022): furniture is bigger than one 0.5 m block, so it
## is a block entity that fills its footprint with invisible FURNITURE voxels
## (full collision, the aim ray hits them) and draws one rotated micro-voxel
## mesh (8 cells per voxel, the chisel grid of ADR 0006) with its own palette.
## Breaking any of its voxels removes all of them and drops the item.
## facing: 0 +Z, 1 +X, 2 -Z, 3 -X; the model's front (bed head, chair seat
## side) points that way.
## Base building and decoration (ADR 0094) are furniture too: a kind with "box" fills only that box of
## local voxels (x across, y up, z along the facing) instead of its whole footprint, so a door or a
## fence is a thin slab the player and mobs bump into; a kind with "open" (door, trapdoor, fence gate)
## swings its model about "hinge" (local voxels) by "swing" on a right click and moves its voxels to
## that box, freeing the doorway. "wall" kinds stand in the clicked voxel's depth layer instead of on
## the block grid (a door in a wall, a painting on one); "climb" kinds are ladders.

signal changed

const DIRS: Array[Vector3i] = [Vector3i(0, 0, 1), Vector3i(1, 0, 0), Vector3i(0, 0, -1), Vector3i(-1, 0, 0)]
## kind -> footprint in 0.5 m blocks (x across, y up, z along the facing; BLOCK voxels each,
## ADR 0034) and the model's cell grid size (8 cells per block).
const KINDS := {
	"bed": {"size": Vector3i(2, 1, 4), "cells": Vector3i(16, 12, 32), "item": "bed", "sleep": true},
	"straw_bed": {"size": Vector3i(2, 1, 4), "cells": Vector3i(16, 6, 32), "item": "straw_bed", "sleep": true},
	"chair": {"size": Vector3i(1, 2, 1), "cells": Vector3i(8, 16, 8), "item": "chair"},
	# 1 x 2 m: the player is 0.6 m wide and 1.8 m tall, so a door is two blocks wide and four tall (ADR 0094)
	"door": {"size": Vector3i(2, 4, 1), "cells": Vector3i(16, 32, 2), "item": "door", "wall": true,
		"box": [Vector3i(0, 0, 0), Vector3i(8, 16, 1)], "open": [Vector3i(0, 0, 0), Vector3i(1, 16, 8)],
		"hinge": Vector3(0.5, 0, 0.5), "swing": Vector3(0, -PI / 2, 0)},
	"trapdoor": {"size": Vector3i(2, 1, 2), "cells": Vector3i(16, 2, 16), "item": "trapdoor",
		"box": [Vector3i(0, 0, 0), Vector3i(8, 1, 8)], "open": [Vector3i(0, 0, 0), Vector3i(8, 8, 1)],
		"hinge": Vector3(0, 0.5, 0.5), "swing": Vector3(-PI / 2, 0, 0)},
	# 1 m fences that collide to 1.5 m, above the 1.25 m jump (Minecraft's fence trick)
	"fence": {"size": Vector3i(2, 2, 1), "cells": Vector3i(16, 16, 4), "item": "fence", "wall": true,
		"box": [Vector3i(0, 0, 0), Vector3i(8, 12, 2)]},
	"fence_gate": {"size": Vector3i(2, 2, 1), "cells": Vector3i(16, 16, 4), "item": "fence_gate", "wall": true,
		"box": [Vector3i(0, 0, 0), Vector3i(8, 12, 2)], "open": [Vector3i(0, 0, 0), Vector3i(2, 12, 8)],
		"hinge": Vector3(1, 0, 1), "swing": Vector3(0, -PI / 2, 0)},
	"ladder": {"size": Vector3i(1, 4, 1), "cells": Vector3i(8, 32, 2), "item": "ladder", "wall": true, "climb": true,
		"box": [Vector3i(0, 0, 0), Vector3i(4, 16, 1)]},
	"window": {"size": Vector3i(2, 2, 1), "cells": Vector3i(16, 16, 2), "item": "window", "wall": true,
		"box": [Vector3i(0, 0, 0), Vector3i(8, 8, 1)], "glass": true},
	"painting": {"size": Vector3i(2, 2, 1), "cells": Vector3i(16, 16, 2), "item": "painting", "wall": true,
		"box": [Vector3i(0, 0, 0), Vector3i(8, 8, 1)]},
	"table": {"size": Vector3i(2, 2, 2), "cells": Vector3i(16, 12, 16), "item": "table",
		"box": [Vector3i(0, 0, 0), Vector3i(8, 6, 8)]},
	"shelf": {"size": Vector3i(2, 2, 1), "cells": Vector3i(16, 16, 8), "item": "shelf"},
	"lantern": {"size": Vector3i(1, 1, 1), "cells": Vector3i(8, 8, 8), "item": "lantern",
		"box": [Vector3i(1, 0, 1), Vector3i(3, 4, 3)]},
	"flower_pot": {"size": Vector3i(1, 1, 1), "cells": Vector3i(8, 8, 8), "item": "flower_pot",
		"box": [Vector3i(1, 0, 1), Vector3i(3, 4, 3)]},
}

var kind := "furniture"      # BlockEntities kind
var furniture := ""          # bed | chair
var pos := Vector3i.ZERO     # anchor voxel (local cell 0, 0, 0)
var facing := 0
var open := false            # doors, trapdoors, gates: swung open, their voxels in the "open" box
var cells: Array[Vector3i] = []


func _init(p_pos: Vector3i, p_furniture: String = "", p_facing: int = 0) -> void:
	pos = p_pos
	furniture = p_furniture
	facing = p_facing
	if furniture != "":
		cells = footprint(furniture, pos, facing)


## The voxels a piece of furniture covers when anchored at `anchor` (the min
## corner of its anchor block): whole blocks of BLOCK^3 voxels laid out from
## the anchor block across (right), up and along the facing (ADR 0034), or
## only its "box" ("open" box when swung open) of local voxels (ADR 0094).
static func footprint(p_kind: String, anchor: Vector3i, p_facing: int, p_open: bool = false) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var k: Dictionary = KINDS[p_kind]
	var size: Vector3i = k["size"]
	var fwd: Vector3i = DIRS[p_facing]
	var right := Vector3i(fwd.z, 0, -fwd.x)
	if not k.has("box"):
		for lx in size.x:
			for ly in size.y:
				for lz in size.z:
					out.append_array(BlockEntities.cube(anchor + (right * lx + Vector3i.UP * ly + fwd * lz) * Settings.BLOCK))
		return out
	var b: Array = k["open"] if p_open and k.has("open") else k["box"]
	var h := Settings.BLOCK * 0.5
	var centre := Vector3(anchor) + Vector3(h, 0, h)
	for lz in range(b[0].z, b[1].z):
		for ly in range(b[0].y, b[1].y):
			for lx in range(b[0].x, b[1].x):
				# voxel centres turned about the anchor block's centre, the way the model turns
				var w := centre + Vector3(right) * (lx + 0.5 - h) + Vector3(0, ly + 0.5, 0) + Vector3(fwd) * (lz + 0.5 - h)
				out.append(Vector3i(w.floor()))
	return out


## Facing toward the player's look (beds: the head goes away from you) or back
## at the player (chairs: you see the seat).
static func facing_for(p_kind: String, look: Vector3) -> int:
	var f := 0
	if absf(look.x) > absf(look.z):
		f = 1 if look.x > 0 else 3
	else:
		f = 0 if look.z > 0 else 2
	return (f + 2) % 4 if p_kind == "chair" else f


func item_id() -> String:
	return KINDS.get(furniture, {}).get("item", furniture)


func contents() -> Array:
	return [{"id": item_id(), "count": 1}]


## Where a sitter's seat surface is (chairs) or the sleeper lies (beds), world voxels.
func seat() -> Vector3:
	var fwd := Vector3(DIRS[facing])
	var right := Vector3(fwd.z, 0, -fwd.x)
	var b := float(Settings.BLOCK)
	var base := _block_centre()
	if KINDS.get(furniture, {}).get("sleep", false):
		return base + (right * 0.5 + fwd * 1.5 + Vector3(0, 1.0, 0)) * b
	return base + Vector3(0, 1.0, 0) * b


## The floor centre of the anchor block (pos is its min corner).
func _block_centre() -> Vector3:
	return Vector3(pos) + Vector3(0.5, 0, 0.5) * Settings.BLOCK


func yaw() -> float:
	var d := DIRS[facing]
	return atan2(float(d.x), float(d.z))


## Where a lantern's flame is, world voxels (TorchLights lights furniture whose item has "light").
func light_at() -> Vector3:
	return _block_centre() + Vector3(0, 1.5, 0)


func can_open() -> bool:
	return KINDS.get(furniture, {}).has("open")


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "furniture": furniture, "facing": facing, "open": open}


func load_from(d: Dictionary) -> void:
	furniture = String(d.get("furniture", "chair"))
	facing = int(d.get("facing", 0))
	open = bool(d.get("open", false)) and can_open()
	cells = footprint(furniture, pos, facing, open)


func summary() -> Dictionary:
	return {"kind": kind, "furniture": furniture, "pos": pos, "facing": facing, "open": open}


# ---------------- the model ----------------

const ATLAS := Vector2i(WorkPalette.SHADES, 16)   # tone columns per paint (WorkPalette.shade_cells)
const PLANK := 0
const PLANK_DARK := 1
const WOOL := 2
const WOOL_SHADE := 3
const RED := 4
const RED_SHADE := 5
const STRAW := 6
const STRAW_SHADE := 7
const IRON := 8
const LEAF := 9
const BLOOM := 10
const TERRACOTTA := 11
const SKY := 12
const FLAME := 13
const BLUE := 14
const SOIL := 15
const PALETTE: Array = [[184, 142, 86], [128, 92, 52], [238, 238, 232], [214, 212, 204], [184, 40, 44], [146, 28, 34],
	[208, 178, 88], [168, 136, 60], [64, 66, 72], [70, 132, 48], [226, 196, 64], [178, 92, 54], [128, 172, 222],
	[255, 214, 120], [52, 72, 150], [72, 50, 34]]

static var _material: StandardMaterial3D
static var _glass: StandardMaterial3D
static var _tiles: PackedInt32Array
static var _meshes := {}


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var img := Image.create(ATLAS.x * 4, ATLAS.y * 4, false, Image.FORMAT_RGBA8)
	for t in PALETTE.size():
		for k in WorkPalette.SHADES:
			img.fill_rect(Rect2i(k * 4, t * 4, 4, 4), WorkPalette.tone_color(PALETTE[t], k))
	_material = StandardMaterial3D.new()
	_material.albedo_texture = ImageTexture.create_from_image(img)
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST
	_material.roughness = 0.9
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() * WorkPalette.SHADES + 1)
	_tiles[0] = -1
	for i in PALETTE.size() * WorkPalette.SHADES:
		_tiles[i + 1] = i
	return _material


## A node showing this furniture in the world (Main adds it under its root). A door, trapdoor or gate
## hangs its model on a "Hinge" child (pivot at the hinge, block units) that Main turns by "swing".
func make_view() -> Node3D:
	var root := Node3D.new()
	root.position = _block_centre()
	root.rotation.y = yaw()
	root.scale = Vector3.ONE * Settings.BLOCK
	var mi := MeshInstance3D.new()
	mi.mesh = mesh_for(furniture)
	mi.material_override = material()
	mi.visibility_range_end = 160.0
	var k: Dictionary = KINDS.get(furniture, {})
	if k.has("hinge"):
		var hinge := Node3D.new()
		hinge.name = "Hinge"
		hinge.position = Vector3(k["hinge"]) / Settings.BLOCK - Vector3(0.5, 0, 0.5)
		mi.position = -hinge.position
		hinge.add_child(mi)
		root.add_child(hinge)
		if open:
			hinge.rotation = k["swing"]
	else:
		root.add_child(mi)
	if k.get("glass", false):
		root.add_child(glass_pane())
	return root


## The pane of a window: a thin see-through box inside the frame (the micro model is opaque).
static func glass_pane() -> MeshInstance3D:
	if _glass == null:
		_glass = StandardMaterial3D.new()
		_glass.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
		_glass.albedo_color = Color(0.72, 0.88, 0.95, 0.28)
		_glass.roughness = 0.05
		_glass.metallic_specular = 0.9
	var box := BoxMesh.new()
	box.size = Vector3(1.5, 1.5, 0.04)
	var g := MeshInstance3D.new()
	g.mesh = box
	g.material_override = _glass
	g.position = Vector3(0.5, 1.0, -0.375)   # cells (2..14, 2..14) of the frame, at its middle cell layer
	g.visibility_range_end = 160.0
	return g


static func mesh_for(p_kind: String) -> ArrayMesh:
	if not _meshes.has(p_kind):
		material()
		var size: Vector3i = KINDS[p_kind]["cells"]
		var c := cells_for(p_kind)
		# cell centres of voxel (0, 0, 0) sit on the node origin, so rotations keep the footprint on the grid
		_meshes[p_kind] = MobModel.outward(MicroMesher.build(WorkPalette.shade_cells(c, size, []), size, GameMaterials.MICRO_GRID, _tiles, ATLAS, Vector3(-0.5, 0, -0.5)))
	return _meshes[p_kind]


static func cells_for(p_kind: String) -> PackedInt32Array:
	var size: Vector3i = KINDS[p_kind]["cells"]
	var c := PackedInt32Array()
	c.resize(size.x * size.y * size.z)
	c.fill(0)
	var box := func(a: Vector3i, b: Vector3i, paint: int) -> void:
		for z in range(a.z, b.z):
			for y in range(a.y, b.y):
				for x in range(a.x, b.x):
					if x >= 0 and y >= 0 and z >= 0 and x < size.x and y < size.y and z < size.z:
						c[x + size.x * (y + size.y * z)] = paint + 1
	if p_kind == "straw_bed":
		# a heap of dry grass on the floor, bound at the edges, a rolled-up pillow at the head end (+z)
		box.call(Vector3i(0, 0, 0), Vector3i(16, 2, 32), STRAW_SHADE)
		box.call(Vector3i(1, 2, 1), Vector3i(15, 3, 31), STRAW)
		for z in range(1, 31, 3):
			box.call(Vector3i(1, 3, z), Vector3i(15, 4, z + 1), STRAW_SHADE)
		for x in range(2, 14, 4):
			box.call(Vector3i(x, 3, 2), Vector3i(x + 2, 4, 26), STRAW)
		box.call(Vector3i(2, 3, 26), Vector3i(14, 6, 31), STRAW)
		box.call(Vector3i(2, 5, 26), Vector3i(14, 6, 27), STRAW_SHADE)
	elif p_kind == "bed":
		# 1 x 2 m: legs, plank frame, white mattress, red blanket draped over the foot two thirds,
		# a pillow and a headboard at the head end (+z)
		for q in [Vector3i(0, 0, 0), Vector3i(14, 0, 0), Vector3i(0, 0, 29), Vector3i(14, 0, 29)]:
			box.call(q, q + Vector3i(2, 2, 3), PLANK_DARK)
		box.call(Vector3i(0, 2, 0), Vector3i(16, 4, 32), PLANK)
		box.call(Vector3i(0, 4, 0), Vector3i(16, 7, 30), WOOL)
		box.call(Vector3i(0, 7, 0), Vector3i(16, 8, 20), RED)
		box.call(Vector3i(0, 4, 0), Vector3i(1, 7, 20), RED_SHADE)
		box.call(Vector3i(15, 4, 0), Vector3i(16, 7, 20), RED_SHADE)
		box.call(Vector3i(0, 4, 0), Vector3i(16, 7, 1), RED_SHADE)
		box.call(Vector3i(0, 7, 19), Vector3i(16, 8, 20), WOOL_SHADE)   # folded edge
		box.call(Vector3i(2, 7, 22), Vector3i(14, 9, 29), WOOL)
		box.call(Vector3i(3, 9, 23), Vector3i(13, 10, 28), WOOL_SHADE)
		box.call(Vector3i(0, 0, 30), Vector3i(16, 12, 32), PLANK_DARK)
		box.call(Vector3i(1, 9, 30), Vector3i(15, 11, 31), PLANK)
	elif p_kind == "door":
		# a plank door: vertical boards, a dark frame and three rails, an iron latch on both faces
		box.call(Vector3i(0, 0, 0), Vector3i(16, 32, 2), PLANK)
		for x in range(4, 13, 4):
			box.call(Vector3i(x, 2, 0), Vector3i(x + 1, 30, 2), PLANK_DARK)
		box.call(Vector3i(0, 0, 0), Vector3i(2, 32, 2), PLANK_DARK)
		box.call(Vector3i(14, 0, 0), Vector3i(16, 32, 2), PLANK_DARK)
		for y in [0, 15, 30]:
			box.call(Vector3i(0, y, 0), Vector3i(16, y + 2, 2), PLANK_DARK)
		box.call(Vector3i(11, 13, 0), Vector3i(13, 15, 2), IRON)
		for y in [4, 26]:
			box.call(Vector3i(0, y, 0), Vector3i(4, y + 1, 2), IRON)   # hinge straps
	elif p_kind == "trapdoor":
		box.call(Vector3i(0, 0, 0), Vector3i(16, 2, 16), PLANK)
		for z in range(4, 13, 4):
			box.call(Vector3i(1, 1, z), Vector3i(15, 2, z + 1), PLANK_DARK)
		box.call(Vector3i(2, 1, 1), Vector3i(4, 2, 15), PLANK_DARK)
		box.call(Vector3i(12, 1, 1), Vector3i(14, 2, 15), PLANK_DARK)
		box.call(Vector3i(7, 1, 13), Vector3i(9, 2, 15), IRON)
	elif p_kind == "fence":
		for x in [0, 13]:
			box.call(Vector3i(x, 0, 0), Vector3i(x + 3, 16, 4), PLANK_DARK)
		for y in [5, 11]:
			box.call(Vector3i(3, y, 1), Vector3i(13, y + 2, 3), PLANK)
	elif p_kind == "fence_gate":
		box.call(Vector3i(0, 1, 1), Vector3i(2, 15, 3), PLANK_DARK)
		box.call(Vector3i(14, 1, 1), Vector3i(16, 15, 3), PLANK_DARK)
		for y in [3, 11]:
			box.call(Vector3i(2, y, 1), Vector3i(14, y + 2, 3), PLANK)
		for i in 6:   # a brace from the hinge foot up to the latch side
			box.call(Vector3i(2 + i * 2, 5 + i, 1), Vector3i(4 + i * 2, 6 + i, 3), PLANK)
	elif p_kind == "ladder":
		box.call(Vector3i(0, 0, 0), Vector3i(2, 32, 2), PLANK_DARK)
		box.call(Vector3i(6, 0, 0), Vector3i(8, 32, 2), PLANK_DARK)
		for y in range(3, 32, 5):
			box.call(Vector3i(2, y, 0), Vector3i(6, y + 1, 2), PLANK)
	elif p_kind == "window":
		box.call(Vector3i(0, 0, 0), Vector3i(16, 16, 2), PLANK_DARK)
		for z in 2:   # four lights between the mullions: FurnitureEntity.glass_pane fills them
			for y in range(2, 14):
				for x in range(2, 14):
					if x != 7 and x != 8 and y != 7 and y != 8:
						c[x + size.x * (y + size.y * z)] = 0
		box.call(Vector3i(0, 0, 0), Vector3i(16, 1, 2), PLANK)   # sill
	elif p_kind == "painting":
		# a landscape on canvas in a dark frame, hung on the wall side (back cell layer)
		box.call(Vector3i(0, 0, 1), Vector3i(16, 16, 2), PLANK_DARK)
		box.call(Vector3i(1, 7, 1), Vector3i(15, 15, 2), SKY)
		box.call(Vector3i(1, 1, 1), Vector3i(15, 7, 2), LEAF)
		box.call(Vector3i(4, 6, 1), Vector3i(10, 9, 2), LEAF)
		box.call(Vector3i(1, 1, 1), Vector3i(15, 3, 2), BLUE)
		box.call(Vector3i(11, 11, 1), Vector3i(13, 13, 2), BLOOM)
		box.call(Vector3i(0, 0, 0), Vector3i(16, 1, 1), PLANK_DARK)   # the frame stands proud of the canvas
		box.call(Vector3i(0, 15, 0), Vector3i(16, 16, 1), PLANK_DARK)
		box.call(Vector3i(0, 0, 0), Vector3i(1, 16, 1), PLANK_DARK)
		box.call(Vector3i(15, 0, 0), Vector3i(16, 16, 1), PLANK_DARK)
	elif p_kind == "table":
		for q in [Vector3i(1, 0, 1), Vector3i(13, 0, 1), Vector3i(1, 0, 13), Vector3i(13, 0, 13)]:
			box.call(q, q + Vector3i(2, 10, 2), PLANK_DARK)
		box.call(Vector3i(1, 8, 1), Vector3i(15, 10, 15), PLANK_DARK)
		box.call(Vector3i(0, 10, 0), Vector3i(16, 12, 16), PLANK)
	elif p_kind == "shelf":
		# a bookcase: open to the front (-z), boards at three heights, books of a few colours
		box.call(Vector3i(0, 0, 0), Vector3i(1, 16, 8), PLANK_DARK)
		box.call(Vector3i(15, 0, 0), Vector3i(16, 16, 8), PLANK_DARK)
		box.call(Vector3i(0, 0, 7), Vector3i(16, 16, 8), PLANK_DARK)
		for y in [0, 7, 15]:
			box.call(Vector3i(1, y, 0), Vector3i(15, y + 1, 7), PLANK)
		var books := [RED, BLUE, LEAF, STRAW_SHADE, RED_SHADE, PLANK_DARK, BLUE]
		for row: int in [1, 8]:
			var x := 1
			var i := row
			while x < 15:
				var w := 1 + (i % 2)
				var h := 4 + (i * 7) % 3
				box.call(Vector3i(x, row, 2), Vector3i(mini(x + w, 15), row + h, 7), books[i % books.size()])
				x += w
				i += 1
	elif p_kind == "lantern":
		box.call(Vector3i(2, 0, 2), Vector3i(6, 1, 6), IRON)
		box.call(Vector3i(2, 5, 2), Vector3i(6, 6, 6), IRON)
		for q in [Vector3i(2, 1, 2), Vector3i(5, 1, 2), Vector3i(2, 1, 5), Vector3i(5, 1, 5)]:
			box.call(q, q + Vector3i(1, 4, 1), IRON)
		box.call(Vector3i(3, 1, 3), Vector3i(5, 4, 5), FLAME)
		box.call(Vector3i(3, 6, 3), Vector3i(5, 7, 5), IRON)
		box.call(Vector3i(3, 7, 4), Vector3i(5, 8, 5), IRON)   # the bail
	elif p_kind == "flower_pot":
		box.call(Vector3i(2, 0, 2), Vector3i(6, 3, 6), TERRACOTTA)
		box.call(Vector3i(1, 3, 1), Vector3i(7, 4, 7), TERRACOTTA)
		box.call(Vector3i(2, 3, 2), Vector3i(6, 4, 6), SOIL)
		box.call(Vector3i(4, 4, 4), Vector3i(5, 7, 5), LEAF)
		box.call(Vector3i(2, 5, 4), Vector3i(4, 6, 5), LEAF)
		box.call(Vector3i(5, 4, 3), Vector3i(6, 5, 5), LEAF)
		box.call(Vector3i(3, 7, 3), Vector3i(6, 8, 6), BLOOM)
		box.call(Vector3i(4, 7, 4), Vector3i(5, 8, 5), RED)
	else:
		# chair: four legs, plank seat with a wool cushion, slatted back on the -z side
		for q in [Vector3i(0, 0, 0), Vector3i(7, 0, 0), Vector3i(0, 0, 7), Vector3i(7, 0, 7)]:
			box.call(q, q + Vector3i(1, 6, 1), PLANK_DARK)
		box.call(Vector3i(0, 6, 0), Vector3i(8, 7, 8), PLANK)
		box.call(Vector3i(1, 7, 1), Vector3i(7, 8, 7), WOOL)
		box.call(Vector3i(0, 7, 0), Vector3i(1, 16, 1), PLANK_DARK)
		box.call(Vector3i(7, 7, 0), Vector3i(8, 16, 1), PLANK_DARK)
		box.call(Vector3i(1, 10, 0), Vector3i(7, 11, 1), PLANK)
		box.call(Vector3i(1, 13, 0), Vector3i(7, 15, 1), PLANK)
		box.call(Vector3i(0, 15, 0), Vector3i(8, 16, 1), PLANK_DARK)
	return c
