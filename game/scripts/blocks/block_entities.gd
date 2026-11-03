class_name BlockEntities
extends RefCounted

## Block entities by voxel position (ADR 0007): the inventories and state
## that a voxel id alone cannot hold (chests, furnaces, beds and chairs, work
## pieces). Every entity covers a footprint of 12.5 cm voxels (ADR 0034): a
## BLOCK^3 cube with its anchor (pos) at the min corner, furniture its own
## cells, a torch the voxels of its piece (ADR 0053); every covered voxel maps to the anchor through
## _cells, so at() finds the entity from any of them. Created when such a
## block is placed (material "entity" key), lazily when one is opened that
## has none (generated or debug-placed blocks), removed with their contents
## when the block breaks. Ticked at Minecraft's 20 ticks per second by Main.
## Saved as versioned JSON next to the world's SQLite file (save_file /
## load_file; Main writes it every 30 s while dirty and on exit).

const TICK_S := 0.05
const SAVE_VERSION := 1

var items: ItemRegistry
var recipes: RecipeRegistry
var _by_pos := {}            # Vector3i -> ChestEntity / FurnaceEntity / FurnitureEntity / work pieces
var _ticking: Array = []     # entities with state over time (furnaces, firepits, pit kilns, molds)
var _cells := {}             # Vector3i -> anchor of the entity whose footprint covers it
var ticks := 0
var dirty := false           # something changed since the last save


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry) -> void:
	items = p_items
	recipes = p_recipes


func create(pos: Vector3i, kind: String) -> RefCounted:
	remove(pos)
	var e: RefCounted = null
	match kind:
		"chest", "storage_pot", "barrel", "storage_shelf":
			e = ChestEntity.new(items, pos, kind)
		"torch":
			e = TorchEntity.new(pos)
		"station":
			e = StationEntity.new(pos)
		"furnace":
			e = FurnaceEntity.new(items, recipes, pos)
			_ticking.append(e)
		"furniture":
			e = FurnitureEntity.new(pos)
		# Vintage Story work pieces (ADR 0023): drawn by Main from view_parts()
		"knap":
			e = KnapEntity.new(pos)
		"clayform":
			e = ClayFormEntity.new(pos)
		"ground":
			e = GroundEntity.new(recipes, pos)
			_ticking.append(e)
		"firepit":
			e = FirepitEntity.new(items, recipes, pos)
			_ticking.append(e)
		# the bronze and iron ages (ADR 0070)
		"forge":
			e = ForgeEntity.new(items, recipes, pos)
			_ticking.append(e)
		"bloomery":
			e = BloomeryEntity.new(items, recipes, pos)
			_ticking.append(e)
		"sieve":
			e = SieveEntity.new(recipes, pos)
		# the steam age (ADR 0097)
		"boiler":
			e = BoilerEntity.new(items, recipes, pos)
			_ticking.append(e)
		"pipe":
			e = PipeEntity.new(pos)
		"steam_hammer":
			e = SteamHammerEntity.new(recipes, pos)
			_ticking.append(e)
		"steam_sieve":
			e = SteamSieveEntity.new(recipes, pos)
			_ticking.append(e)
		# stage 2 and the turbine (ADR 0099)
		"blast_furnace":
			e = BlastFurnaceEntity.new(items, recipes, pos)
			_ticking.append(e)
		"ore_crusher":
			e = OreCrusherEntity.new(recipes, pos)
			_ticking.append(e)
		"tank":
			e = TankEntity.new(pos)
		"turbine":
			e = TurbineEntity.new(pos)
			_ticking.append(e)
		"compressor":
			e = CompressorEntity.new(pos)
			_ticking.append(e)
		# the energy network (ADR 0113)
		"cable":
			e = CableEntity.new(pos)
		"electric_furnace":
			e = ElectricFurnaceEntity.new(items, recipes, pos)
			_ticking.append(e)
		_ when kind in MachineEntity.DOOR:   # LV machines: the item's "machine" key and process recipes (ADR 0117, 0131)
			e = MachineEntity.new(items, recipes, pos, kind)
			_ticking.append(e)
		"battery_box":
			e = BatteryBoxEntity.new(pos)
		"pump":
			e = PumpEntity.new(pos)
			_ticking.append(e)
		"coke_oven":   # the first multiblock (ADR 0117)
			e = CokeOvenEntity.new(items, recipes, pos)
			_ticking.append(e)
		"electric_lamp":
			e = ElectricLampEntity.new(pos)
		# stage 3 transport (ADR 0100)
		"hopper":
			e = HopperEntity.new(self, pos)
			_ticking.append(e)
		"belt":
			e = BeltEntity.new(self, pos)
			_ticking.append(e)
		"rail":
			e = RailEntity.new(self, pos)
			_ticking.append(e)   # a chest cart rolls by itself
		"firewood_pile":
			e = FirewoodPileEntity.new(recipes, pos)
			_ticking.append(e)
		# crafting in the world (ADR 0042)
		"assembly":
			e = AssemblyEntity.new(recipes, pos)
		"anvil", "bronze_anvil":
			e = AnvilEntity.new(recipes, pos, kind)
			_ticking.append(e)
		"laid_tool", "leaning_tool", "ingot_pile":
			e = LaidEntity.new(items, pos, kind)   # tools and ingots set down (ADR 0095)
		"grave":
			e = GraveEntity.new(items, pos)   # what a dead player carried (plan item D12)
		_:
			e = StoneAge.create_entity(self, pos, kind)   # the Stone Age pieces (ADR 0059)
			if e == null:
				push_error("unknown block entity kind " + kind)
				return null
			if StoneAge.ticks(kind):
				_ticking.append(e)
	_by_pos[pos] = e
	_register_cells(e)
	e.changed.connect(_mark_dirty)
	dirty = true
	return e


const SINGLE := []   # entities that are one voxel (torches have their own cells since ADR 0053)
## Flat pieces are lower than a block, so they never hide the ground behind them from the aim ray:
## a knapping stone or a mold on the ground is one voxel high, a firepit two.
const HEIGHT := {"knap": 1, "ground": 1, "firepit": 2, "forge": 8, "assembly": 1, "laid_tool": 1, "anvil": 6, "bronze_anvil": 6,
	"boiler": 4, "steam_hammer": 9, "pipe": 2, "ore_crusher": 5, "tank": 6, "belt": 2, "rail": 1, "cable": 1, "electric_lamp": 12}
## Work pieces bigger than one 0.5 m block: their footprint width and height multiply by this many blocks
## (the stations' 1 m scale, ADR 0076; the steam age's 1.5 m hammer and 2.5 m boiler, ADR 0097). A view
## of 8 cells still spans the width, so a 1 m piece's cell is a voxel; a bigger piece names its own grid.
const SCALE := {"firepit": 2, "forge": 2, "bloomery": 2, "anvil": 2, "bronze_anvil": 2, "steam_hammer": 3, "boiler": 5,
	"blast_furnace": 3, "ore_crusher": 2, "tank": 2, "turbine": 3, "compressor": 2, "electric_furnace": 2, "lathe": 2, "bending_press": 2, "wiremill": 2, "macerator": 2, "alloy_smelter": 2, "assembler": 2, "battery_box": 2, "pump": 2}


## Footprint width in voxels of a work piece kind.
static func width_of(p_kind: String) -> int:
	return Settings.BLOCK * scale_of(p_kind)


## Blocks per side of a work piece: SCALE, or 2 for every LV machine (1 m like the lathe, ADR 0117).
static func scale_of(p_kind: String) -> int:
	return int(SCALE.get(p_kind, 2 if MachineEntity.DOOR.has(p_kind) else 1))


## The voxels an entity covers (ADR 0034).
static func footprint(e: RefCounted) -> Array[Vector3i]:
	if e is FurnitureEntity or e is TorchEntity:
		return e.cells
	if e.has_method("footprint_cells"):
		return e.footprint_cells()   # a megalith's three blocks (ADR 0059)
	if e.kind in SINGLE:
		return [e.pos]
	return footprint_of(e.kind, e.pos)


## The voxels a station of `p_kind` would cover from `anchor` (placement checks).
static func footprint_of(p_kind: String, anchor: Vector3i) -> Array[Vector3i]:
	if StationEntity.SIZES.has(p_kind):
		return box(anchor, StationEntity.SIZES[p_kind])   # the 1 m workbench, chest and furnace (ADR 0053, 0076)
	var w := width_of(p_kind)
	return box(anchor, Vector3i(w, height_of(p_kind), w))


## size.x x size.y x size.z voxels from `anchor` (min corner).
static func box(anchor: Vector3i, size: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	for z in size.z:
		for y in size.y:
			for x in size.x:
				out.append(anchor + Vector3i(x, y, z))
	return out


## Footprint height in voxels of a block entity kind (a wide piece's doubles with its width).
static func height_of(p_kind: String) -> int:
	var h := int(HEIGHT.get(p_kind, StoneAge.HEIGHTS.get(p_kind, Settings.BLOCK)))
	return h * scale_of(p_kind)


## BLOCK x height x BLOCK voxels from `anchor` (min corner): a station's or work piece's footprint.
static func cube(anchor: Vector3i, height: int = Settings.BLOCK) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var n := Settings.BLOCK
	for z in n:
		for y in height:
			for x in n:
				out.append(anchor + Vector3i(x, y, z))
	return out


## Where a block-sized thing goes when placed against a face (ADR 0034): x and z
## snap to the block grid, y is the voxel in front of the face, so it stands on
## the real ground.
static func anchor_for(front: Vector3i) -> Vector3i:
	var n := Settings.BLOCK
	return Vector3i(floori(float(front.x) / n) * n, front.y, floori(float(front.z) / n) * n)


func _mark_dirty() -> void:
	dirty = true


## A multi-voxel torch whose flame-side piece starts at `pos` (PlantStamps.torch); `wall` is the
## direction to the wall it hangs on, ZERO for a floor.
func create_torch(pos: Vector3i, wall: Vector3i) -> TorchEntity:
	remove(pos)
	var e := TorchEntity.new(pos)
	e.set_mount(wall)
	_by_pos[pos] = e
	_register_cells(e)
	e.changed.connect(_mark_dirty)
	dirty = true
	return e


## A bed or chair anchored at `anchor`; registers every voxel it covers.
func create_furniture(anchor: Vector3i, furniture: String, facing: int) -> FurnitureEntity:
	remove(anchor)
	var e := FurnitureEntity.new(anchor, furniture, facing)
	_by_pos[anchor] = e
	_register_cells(e)
	e.changed.connect(_mark_dirty)
	dirty = true
	return e


## Swings a door, trapdoor or gate open or shut (ADR 0094): its voxels move to the other box. Returns
## the cells it covered before; Main empties those it no longer covers and fills the new ones.
func set_open(e: FurnitureEntity, p_open: bool) -> Array[Vector3i]:
	var old := e.cells
	for c in old:
		if _cells.get(c) == e.pos:
			_cells.erase(c)
	e.open = p_open and e.can_open()
	e.cells = FurnitureEntity.footprint(e.furniture, e.pos, e.facing, e.open)
	_register_cells(e)
	dirty = true
	return old


func _register_cells(e: RefCounted) -> void:
	for c in footprint(e):
		_cells[c] = e.pos


## An entity's footprint changed (a multiblock formed or broke, ADR 0117): cover its cells again.
func refit(e: RefCounted) -> void:
	for c in _cells.keys():
		if _cells[c] == e.pos:
			_cells.erase(c)
	_register_cells(e)
	dirty = true


## The furniture covering voxel p, or null.
func furniture_at(p: Vector3i) -> FurnitureEntity:
	var a = _cells.get(p)
	return _by_pos.get(a) as FurnitureEntity if a != null else null


func all_furniture() -> Array:
	var out := []
	for p in _by_pos:
		if _by_pos[p] is FurnitureEntity:
			out.append(_by_pos[p])
	return out


const WORK_KINDS := ["knap", "clayform", "ground", "firepit", "sieve", "firewood_pile", "assembly", "anvil", "grave",
	"forge", "bloomery", "bronze_anvil", "laid_tool", "leaning_tool", "ingot_pile", "boiler", "pipe", "steam_hammer", "steam_sieve",
	"blast_furnace", "ore_crusher", "tank", "turbine", "compressor", "hopper", "belt", "rail", "cable", "electric_furnace", "lathe", "bending_press", "wiremill", "macerator", "alloy_smelter", "assembler", "battery_box", "electric_lamp", "pump", "coke_oven"]


## The machines a boiler's pipes feed (ADR 0097): everything with a `boiler` slot.
func all_steam_machines() -> Array:
	var out := []
	for e in _ticking:
		if "boiler" in e:
			out.append(e)
	return out


func all_entities() -> Array:
	return _by_pos.values()


func all_of(p_kind: String) -> Array:
	var out := []
	for p in _by_pos:
		if _by_pos[p].kind == p_kind:
			out.append(_by_pos[p])
	return out


## Entities burning now (furnaces, firepits, kilns, firewood piles): TorchLights lights them.
func lit() -> Array:
	var out := []
	for e in _ticking:
		if e.is_lit():
			out.append(e)
	return out


static func is_work(e: RefCounted) -> bool:
	return e != null and (e.kind in WORK_KINDS or e.kind in StoneAge.KINDS or MachineEntity.DOOR.has(e.kind))


func all_work() -> Array:
	var out := []
	for p in _by_pos:
		if is_work(_by_pos[p]):
			out.append(_by_pos[p])
	return out


## The entity whose footprint covers voxel `pos`, or null.
func at(pos: Vector3i) -> RefCounted:
	return _by_pos.get(_cells.get(pos, pos))


func has(pos: Vector3i) -> bool:
	return at(pos) != null


func get_or_create(pos: Vector3i, kind: String) -> RefCounted:
	var e: RefCounted = _by_pos.get(pos)
	if e != null and e.kind == kind:
		return e
	return create(pos, kind)


## Removes the entity; returns its stacks (to drop as item entities).
func remove(pos: Vector3i) -> Array:
	if _cells.has(pos):
		pos = _cells[pos]   # any voxel of a bed or chair removes all of it
	var e: RefCounted = _by_pos.get(pos)
	if e == null:
		return []
	for c in footprint(e):
		if _cells.get(c) == pos:
			_cells.erase(c)
	_by_pos.erase(pos)
	_ticking.erase(e)
	dirty = true
	return e.contents()


func size() -> int:
	return _by_pos.size()


## Advances every ticking entity by one tick. Returns the positions whose
## lit state flipped.
func tick() -> Array[Vector3i]:
	ticks += 1
	var flipped: Array[Vector3i] = []
	if ticks % 20 == 0:
		Networks.solve(self, "energy")   # power over the cables (ADR 0113)
		Networks.solve(self, "steam")    # steam over the pipes: every machine with a boiler slot (ADR 0122)
		Networks.solve(self, "water")    # water over the pipes: pumps, tanks, boilers
	for e in _ticking.duplicate():   # a bursting boiler removes itself
		if e.tick():
			flipped.append(e.pos)
		if e.is_lit():
			dirty = true     # burn and cook progress move
	return flipped


# ---------------- persistence ----------------

func to_dict() -> Dictionary:
	var list := []
	for p in _by_pos:
		list.append(_by_pos[p].to_dict())
	return {"version": SAVE_VERSION, "entities": list}


## Replaces everything with the saved entities. Returns false on bad data.
func load_dict(d: Dictionary) -> bool:
	if int(d.get("version", 0)) != SAVE_VERSION:
		push_error("block entities: unsupported save version %s" % d.get("version"))
		return false
	_by_pos.clear()
	_ticking.clear()
	_cells.clear()
	for ed in d.get("entities", []):
		var pa: Array = ed.get("pos", [])
		if pa.size() != 3:
			continue
		var e := create(Vector3i(int(pa[0]), int(pa[1]), int(pa[2])), String(ed.get("kind", "")))
		if e != null:
			if e is KnapEntity or e is ClayFormEntity:
				e.load_from(ed, recipes)   # they keep their recipe by id
			else:
				e.load_from(ed)
			_register_cells(e)   # furniture knows its cells only after load_from
	dirty = false
	return true


## Writes atomically (temp file + rename) so a crash never leaves half a save.
func save_file(path: String) -> bool:
	DirAccess.make_dir_recursive_absolute(path.get_base_dir())
	var tmp := path + ".tmp"
	var f := FileAccess.open(tmp, FileAccess.WRITE)
	if f == null:
		push_error("block entities: cannot write %s" % tmp)
		return false
	f.store_string(JSON.stringify(to_dict()))
	f.close()
	if DirAccess.rename_absolute(tmp, path) != OK:
		push_error("block entities: cannot rename %s" % tmp)
		return false
	dirty = false
	return true


func load_file(path: String) -> bool:
	if not FileAccess.file_exists(path):
		return false
	var data = JSON.parse_string(FileAccess.get_file_as_string(path))
	if not (data is Dictionary):
		push_error("block entities: %s is not valid JSON" % path)
		return false
	return load_dict(data)


func summary() -> Dictionary:
	var list := []
	for p in _by_pos:
		list.append(_by_pos[p].summary())
	return {"count": _by_pos.size(), "ticks": ticks, "entities": list.slice(0, 8)}
