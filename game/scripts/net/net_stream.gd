class_name NetStream
extends RefCounted

## The multiplayer client's mirror of the server-owned chunks (ADR 0169): every chunk the server sent (at the
## join, then live) plus this client's own snapshots. TerrainGenerator lays them over what it generates
## (`overlay`, on godot_voxel's worker threads), so the terrain never asks a stream: with any VoxelStream set,
## our godot_voxel build queues one serial I/O task per block before generating (voxel_patch_stream.py) and a
## windowed run loaded nothing for 20 s. Snapshots come from the sim (SimBubble -> Net.snapshot_chunk): the
## terrain can lag behind the sim's budgeted pushes. Generated terrain is never stored or sent: the seed
## reproduces it. The main thread writes, worker threads read: every access takes the mutex, and a chunk is
## replaced by a new buffer, never written in place.

var blocks := {}            # Vector3i chunk -> VoxelBuffer (16^3)
var snap_n := {}            # Vector3i chunk -> server journal ops baked into that snapshot
var known_n := 0            # server journal ops this client has (Net keeps it current)
var outbox: Array = []      # [pos, bytes, n] snapshotted here, not sent yet
var native: Callable        # FineGen.ground_range (World sets it): the ground range the shell widens with the mirror
var _col_top := {}          # Vector2i (chunk x, chunk z) -> highest chunk y held in that column
var _mutex := Mutex.new()
const NONE := -1073741824   # no chunk in the column (below any world y)


## A chunk from the server (VoxelBlockSerializer bytes). Null when this client's copy is as new (its own snapshot).
func put(pos: Vector3i, bytes: PackedByteArray, n: int) -> VoxelBuffer:
	var vb := to_buffer(bytes)
	if vb == null:
		return null
	_mutex.lock()
	var newer: bool = n >= int(snap_n.get(pos, -1))
	if newer:
		blocks[pos] = vb
		snap_n[pos] = n
		_note_top(pos)
	_mutex.unlock()
	return vb if newer else null


## This client's own snapshot of a chunk its sim holds, with every op it knows applied.
func put_own(pos: Vector3i, vb: VoxelBuffer) -> void:
	var bytes := VoxelBlockSerializer.serialize_to_byte_array(vb, VoxelBlockSerializer.COMPRESSION_LZ4)
	_mutex.lock()
	blocks[pos] = vb
	snap_n[pos] = known_n
	_note_top(pos)
	outbox.append([pos, bytes, known_n])
	_mutex.unlock()


func _note_top(pos: Vector3i) -> void:
	var k := Vector2i(pos.x, pos.z)
	_col_top[k] = maxi(int(_col_top.get(k, NONE)), pos.y)


## The ground shell's column bound (VoxelLodTerrain.set_ground_shell, called on its update thread): the generator's
## range, raised to the top of what the mirror holds in the column. Blocks over the bound are made air without a
## generation task (voxel_patch_sky.py), so a built thing in the sky would never be generated or overlaid.
## shortcut: the terrain memoises the bound per column, so a chunk that arrives later above it is pasted on arrival
## only (Net._chunk_in) and is air again after that block unloads and reloads; drop the memo when that shows.
func ground_range(x: int, z: int, size: int) -> Vector2i:
	var r: Vector2i = native.call(x, z, size)
	if size != Settings.CHUNK:
		return r
	_mutex.lock()
	var top: int = _col_top.get(Vector2i(floori(float(x) / Settings.CHUNK), floori(float(z) / Settings.CHUNK)), NONE)
	_mutex.unlock()
	return Vector2i(r.x, maxi(r.y, (top + 1) * Settings.CHUNK))


## The chunk held for `pos`, or null.
func block(pos: Vector3i) -> VoxelBuffer:
	_mutex.lock()
	var vb: VoxelBuffer = blocks.get(pos)
	_mutex.unlock()
	return vb


## Copies the stored chunks into a generated block ([origin, origin + size) world voxels; any size, any alignment).
## True when one was. Worker threads call this.
func overlay(out_buffer: VoxelBuffer, origin: Vector3i, size: Vector3i) -> bool:
	var c0 := Vector3i((Vector3(origin) / Settings.CHUNK).floor())
	var c1 := Vector3i((Vector3(origin + size - Vector3i.ONE) / Settings.CHUNK).floor())
	var hit := false
	for z in range(c0.z, c1.z + 1):
		for y in range(c0.y, c1.y + 1):
			for x in range(c0.x, c1.x + 1):
				var wc := Vector3i(x, y, z)
				_mutex.lock()
				var vb: VoxelBuffer = blocks.get(wc)
				_mutex.unlock()
				if vb == null:
					continue
				var lo := (wc * Settings.CHUNK).max(origin)           # the overlap, world voxels
				var hi := (wc * Settings.CHUNK + Vector3i.ONE * Settings.CHUNK).min(origin + size)
				out_buffer.copy_channel_from_area(vb, lo - wc * Settings.CHUNK, hi - wc * Settings.CHUNK, lo - origin, VoxelBuffer.CHANNEL_TYPE)
				hit = true
	return hit


## Server ops baked into the chunk's snapshot (-1: none).
func baked(pos: Vector3i) -> int:
	_mutex.lock()
	var n: int = snap_n.get(pos, -1)
	_mutex.unlock()
	return n


func take_outbox() -> Array:
	_mutex.lock()
	var out := outbox
	outbox = []
	_mutex.unlock()
	return out


func size() -> int:
	_mutex.lock()
	var n := blocks.size()
	_mutex.unlock()
	return n


static func to_buffer(bytes: PackedByteArray) -> VoxelBuffer:
	if bytes.is_empty():
		return null
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	VoxelBlockSerializer.deserialize_from_byte_array(bytes, vb, true)
	return vb
