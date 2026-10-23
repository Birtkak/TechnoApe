class_name FurnitureEntity
extends RefCounted

## A bed, straw bed (ADR 0030) or chair (ADR 0022): furniture is bigger than one 0.5 m block, so it
## is a block entity that fills its footprint with invisible FURNITURE voxels
## (full collision, the aim ray hits them) and draws one rotated micro-voxel
## mesh (8 cells per voxel, the chisel grid of ADR 0006) with its own palette.
## Breaking any of its voxels removes all of them and drops the item.
## facing: 0 +Z, 1 +X, 2 -Z, 3 -X; the model's front (bed head, chair seat
## side) points that way.

signal changed

const DIRS: Array[Vector3i] = [Vector3i(0, 0, 1), Vector3i(1, 0, 0), Vector3i(0, 0, -1), Vector3i(-1, 0, 0)]
## kind -> footprint in 0.5 m blocks (x across, y up, z along the facing; BLOCK voxels each,
## ADR 0034) and the model's cell grid size (8 cells per block).
const KINDS := {
	"bed": {"size": Vector3i(2, 1, 4), "cells": Vector3i(16, 12, 32), "item": "bed", "sleep": true},
	"straw_bed": {"size": Vector3i(2, 1, 4), "cells": Vector3i(16, 6, 32), "item": "straw_bed", "sleep": true},
	"chair": {"size": Vector3i(1, 2, 1), "cells": Vector3i(8, 16, 8), "item": "chair"},
}

var kind := "furniture"      # BlockEntities kind
var furniture := ""          # bed | chair
var pos := Vector3i.ZERO     # anchor voxel (local cell 0, 0, 0)
var facing := 0
var cells: Array[Vector3i] = []


func _init(p_pos: Vector3i, p_furniture: String = "", p_facing: int = 0) -> void:
	pos = p_pos
	furniture = p_furniture
	facing = p_facing
	if furniture != "":
		cells = footprint(furniture, pos, facing)


## The voxels a piece of furniture covers when anchored at `anchor` (the min
## corner of its anchor block): whole blocks of BLOCK^3 voxels laid out from
## the anchor block across (right), up and along the facing (ADR 0034).
static func footprint(p_kind: String, anchor: Vector3i, p_facing: int) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var size: Vector3i = KINDS[p_kind]["size"]
	var fwd: Vector3i = DIRS[p_facing]
	var right := Vector3i(fwd.z, 0, -fwd.x)
	for lx in size.x:
		for ly in size.y:
			for lz in size.z:
				out.append_array(BlockEntities.cube(anchor + (right * lx + Vector3i.UP * ly + fwd * lz) * Settings.BLOCK))
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


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "furniture": furniture, "facing": facing}


func load_from(d: Dictionary) -> void:
	furniture = String(d.get("furniture", "chair"))
	facing = int(d.get("facing", 0))
	cells = footprint(furniture, pos, facing)


func summary() -> Dictionary:
	return {"kind": kind, "furniture": furniture, "pos": pos, "facing": facing}


# ---------------- the model ----------------

const ATLAS := Vector2i(8, 1)
const PLANK := 0
const PLANK_DARK := 1
const WOOL := 2
const WOOL_SHADE := 3
const RED := 4
const RED_SHADE := 5
const STRAW := 6
const STRAW_SHADE := 7
const PALETTE: Array = [[184, 142, 86], [128, 92, 52], [238, 238, 232], [214, 212, 204], [184, 40, 44], [146, 28, 34],
	[208, 178, 88], [168, 136, 60]]

static var _material: StandardMaterial3D
static var _tiles: PackedInt32Array
static var _meshes := {}


static func material() -> StandardMaterial3D:
	if _material:
		return _material
	var img := Image.create(ATLAS.x * 16, ATLAS.y * 16, false, Image.FORMAT_RGBA8)
	var rng := RandomNumberGenerator.new()
	rng.seed = 5151
	for t in PALETTE.size():
		var base: Array = PALETTE[t]
		for y in 16:
			for x in 16:
				var k := rng.randi_range(-6, 6)
				if t <= PLANK_DARK and y % 4 == 0:
					k -= 10   # plank grain
				img.set_pixel(t * 16 + x, y, Color8(clampi(base[0] + k, 0, 255), clampi(base[1] + k, 0, 255), clampi(base[2] + k, 0, 255)))
	_material = StandardMaterial3D.new()
	_material.albedo_texture = ImageTexture.create_from_image(img)
	_material.texture_filter = BaseMaterial3D.TEXTURE_FILTER_NEAREST_WITH_MIPMAPS
	_material.roughness = 0.9
	_tiles = PackedInt32Array()
	_tiles.resize(PALETTE.size() + 1)
	_tiles[0] = -1
	for i in PALETTE.size():
		_tiles[i + 1] = i
	return _material


## A node showing this furniture in the world (Main adds it under its root).
func make_view() -> MeshInstance3D:
	var mi := MeshInstance3D.new()
	mi.mesh = mesh_for(furniture)
	mi.material_override = material()
	mi.position = _block_centre()
	mi.rotation.y = yaw()
	mi.scale = Vector3.ONE * Settings.BLOCK
	mi.visibility_range_end = 160.0
	return mi


static func mesh_for(p_kind: String) -> ArrayMesh:
	if not _meshes.has(p_kind):
		material()
		var size: Vector3i = KINDS[p_kind]["cells"]
		var c := cells_for(p_kind)
		# cell centres of voxel (0, 0, 0) sit on the node origin, so rotations keep the footprint on the grid
		_meshes[p_kind] = MobModel.outward(MicroMesher.build(c, size, GameMaterials.MICRO_GRID, _tiles, ATLAS, Vector3(-0.5, 0, -0.5)))
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
