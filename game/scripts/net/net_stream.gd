class_name NetStream
extends VoxelStreamScript

## The multiplayer client's terrain stream (ADR 0169): the server owns the edited chunks. This mirror holds
## every chunk the server sent (at the join, then live) plus this client's own snapshots, and serves them to
## VoxelLodTerrain before it generates. Snapshots come from the sim (SimBubble -> Net.snapshot_chunk), never
## from the terrain's save: the terrain can lag behind the sim's budgeted pushes. Generated terrain is never
## stored or sent: the seed reproduces it. godot_voxel calls _load from its streaming thread, so every store
## access takes the mutex.

var blocks := {}            # Vector3i chunk -> serialized VoxelBuffer (VoxelBlockSerializer, LZ4)
var snap_n := {}            # Vector3i chunk -> server journal ops baked into that snapshot
var known_n := 0            # server journal ops this client has (Net keeps it current)
var outbox: Array = []      # [pos, bytes, n] snapshotted here, not sent yet
var _mutex := Mutex.new()


func _get_used_channels_mask() -> int:
	return 1 << VoxelBuffer.CHANNEL_TYPE


func _load_voxel_block(out_buffer: VoxelBuffer, position_in_blocks: Vector3i, lod: int) -> int:
	if lod != 0:
		return VoxelStream.RESULT_BLOCK_NOT_FOUND
	_mutex.lock()
	var b: Variant = blocks.get(position_in_blocks)
	_mutex.unlock()
	if b == null:
		return VoxelStream.RESULT_BLOCK_NOT_FOUND
	VoxelBlockSerializer.deserialize_from_byte_array(b, out_buffer, true)
	return VoxelStream.RESULT_BLOCK_FOUND


func _save_voxel_block(_buffer: VoxelBuffer, _position_in_blocks: Vector3i, _lod: int) -> void:
	pass   # the sim snapshots its own chunks (put_own); the terrain's copy may miss pushes


## A chunk from the server. False when this client's copy is as new (its own snapshot).
func put(pos: Vector3i, bytes: PackedByteArray, n: int) -> bool:
	_mutex.lock()
	var newer: bool = n >= int(snap_n.get(pos, -1))
	if newer:
		blocks[pos] = bytes
		snap_n[pos] = n
	_mutex.unlock()
	return newer


## This client's own snapshot of a chunk its sim holds, with every op it knows applied.
func put_own(pos: Vector3i, bytes: PackedByteArray) -> void:
	_mutex.lock()
	blocks[pos] = bytes
	snap_n[pos] = known_n
	outbox.append([pos, bytes, known_n])
	_mutex.unlock()


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
