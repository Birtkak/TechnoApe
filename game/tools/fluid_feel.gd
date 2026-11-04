extends SceneTree

## Fluid feel (ADR 0138, headless): the fluid_film scenes (scripts/visuals/fluid_scenes.gd) on the bare kernel, set up
## as SimBubble sets it up, ticked at 30 Hz. Per scene one JSON line: how fast the target fills (ticks to half and
## nine tenths of the cells it should end with), ticks until quiet, mass kept, chunks still awake, and on the middle
## slice the cells that hang (moving liquid over air, unchanged since the last sample) and specks (a lone cell of two
## eighths or less). slices=1 also prints the slice every `every` ticks.
##   godot --headless --path game --script res://tools/fluid_feel.gd -- [scenes=pit_lake,dam] [ticks=300] [slices=1] [every=15] [column=0]

const HZ := 30.0
const O := Vector3i(16, 24, 16)   # the patch corner inside the bare world
## where each scene should end up holding water: [min, max) relative to the patch, and the cells it holds when full
const TARGET := {
	"pit_lake": [Vector3i(32, -10, 16), Vector3i(46, 0, 32)],
	"pour_pit": [Vector3i(24, -12, 16), Vector3i(40, 1, 32)],
	"fall": [Vector3i(26, -6, 14), Vector3i(50, 0, 34)],
	"cave": [Vector3i(12, -16, 20), Vector3i(63, -7, 28)],
	"dam": [Vector3i(20, 0, 2), Vector3i(62, 12, 46)],
	"slope": [Vector3i(48, -4, 2), Vector3i(64, 0, 46)],
	"channel": [Vector3i(28, -4, 22), Vector3i(60, 0, 26)],
	"trench": [Vector3i(28, -4, 21), Vector3i(40, 0, 28)],
}

var w: SimWorld


func _box(a: Vector3i, b: Vector3i, m: int) -> void:
	w.fill_box(O + a, O + b, m)


func _cell(p: Vector3i) -> Dictionary:
	return w.cell_info(O + p)


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		if "=" in a:
			args[a.get_slice("=", 0)] = a.get_slice("=", 1)
	var scenes: PackedStringArray = String(args.get("scenes", ",".join(FluidScenes.ALL))).split(",", false)
	var ticks := int(args.get("ticks", "300"))
	var every := int(args.get("every", "15"))
	var slices: bool = args.get("slices", "0") == "1"
	for s in scenes:
		print(JSON.stringify(_scene(s, ticks, every, slices, args.get("column", "1") == "1")))
	quit()


func _scene(name: String, ticks: int, every: int, slices: bool, column: bool) -> Dictionary:
	w = SimWorld.new()
	w.configure(Vector3i(6, 4, 6), maxi(2, OS.get_processor_count() - 2), 7)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(column)
	w.set_states(true)
	w.set_ambient_c(18.0, true)
	w.set_cascade(true)
	w.set_gravity(3.0 * 9.8 / 0.125 / (HZ * HZ))   # Settings.sim_gravity_x; Settings is an autoload, not here
	_box(Vector3i(-16, -O.y, -16), Vector3i(FluidScenes.W + 16, -FluidScenes.DEEP, FluidScenes.D + 16), GameMaterials.STONE)   # dirt is a powder: it needs a floor
	_box(Vector3i(-16, -FluidScenes.DEEP, -16), Vector3i(FluidScenes.W + 16, 0, FluidScenes.D + 16), GameMaterials.DIRT)
	FluidScenes.build(name, _box)
	for i in 45:   # the 1.5 s the film waits before it rolls
		w.tick(1)
	var tg: Array = TARGET[name]
	var lo: Vector3i = O + tg[0]
	var hi: Vector3i = O + tg[1]
	var out := {"scene": name, "column": column}
	var mass0 := w.count_liquid_eighths(GameMaterials.WATER)
	var done := {}
	var fill := PackedInt32Array()
	var quiet := 0
	var quiet_at := -1
	var prev := PackedStringArray()
	var hanging_max := 0
	var awake_max := 0
	for t in ticks:
		FluidScenes.act(name, t / HZ, done, _box)
		var s: Dictionary = w.tick(1)
		awake_max = maxi(awake_max, int(s.get("active_chunks_per_tick", 0)))
		fill.append(w.count_in_box(lo, hi, GameMaterials.WATER))
		if int(s.get("moved_cells_per_tick", 1)) < 4:
			quiet += 1
			if quiet == 15 and quiet_at < 0:
				quiet_at = t - 14
		else:
			quiet = 0
			quiet_at = -1
		if t % every == 0 or t == ticks - 1:
			var sl := FluidScenes.slice(_cell)
			if not prev.is_empty():
				hanging_max = maxi(hanging_max, _hanging(prev, sl))
			prev = sl
			if slices:
				print("t=%d (%.2f s)" % [t, t / HZ])
				print("\n".join(sl))
	var end_fill := fill[fill.size() - 1]
	out["fill_end"] = end_fill
	out["fill_half_tick"] = _first(fill, end_fill / 2)
	out["fill_90_tick"] = _first(fill, end_fill * 9 / 10)
	out["quiet_tick"] = quiet_at
	out["awake_end"] = w.get_active_chunk_count()
	out["awake_max"] = awake_max
	out["mass_start"] = mass0
	out["mass_end"] = w.count_liquid_eighths(GameMaterials.WATER)
	out["hanging_max"] = hanging_max
	out["specks_end"] = _specks(prev)
	out["surface_spread"] = _spread(prev)
	return out


func _first(a: PackedInt32Array, v: int) -> int:
	for i in a.size():
		if a[i] >= v:
			return i
	return -1


## Slice cells of moving water over air that did not change since the last sample.
func _hanging(a: PackedStringArray, b: PackedStringArray) -> int:
	var n := 0
	for r in range(0, b.size() - 1):
		for x in range(5, b[r].length()):
			var c := b[r][x]
			if c in "12345678W" and b[r + 1][x] == "." and a[r][x] == c:
				n += 1
	return n


## Lone moving cells of one or two eighths on the slice (no water on the four slice sides).
func _specks(b: PackedStringArray) -> int:
	var n := 0
	for r in range(1, b.size() - 1):
		for x in range(6, b[r].length() - 1):
			if b[r][x] in "12":
				var near := false
				for q in [b[r - 1][x], b[r + 1][x], b[r][x - 1], b[r][x + 1]]:
					near = near or q in "12345678WS"
				if not near:
					n += 1
	return n


## Highest minus lowest surface top (eighths) over the slice's columns that hold moving water at their top.
func _spread(b: PackedStringArray) -> int:
	var lo := 1 << 30
	var hi := -(1 << 30)
	for x in range(5, b[0].length()):
		for r in b.size():
			var c := b[r][x]
			if c in "12345678W":
				var y := FluidScenes.SLICE_Y.y - r
				var h := 8 * y + (8 if c == "W" else int(c))
				lo = mini(lo, h)
				hi = maxi(hi, h)
				break
			if c != ".":
				break
	return hi - lo if hi >= lo else 0
