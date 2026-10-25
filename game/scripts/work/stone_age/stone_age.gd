class_name StoneAge
extends Node

## The four Stone Age stages in the world (ADR 0059), on top of the work pieces of ADR 0023/0030/0042.
## Player calls use() on every right click before anything else (its `item_use` hook); it returns
## true when the click was taken:
## - the atlatl throws a spear from the inventory harder and further, the bow shoots arrows;
## - clothing goes on (Clothing, read by Survival);
## - its own work pieces: earth ovens (birch tar, tempered flint), drying racks (dried and smoked
##   meat and fish), stroke work (polishing an axe on sandstone, grinding grain and pigments on a
##   flat rock, sewing on the ground, weaving on a loom) and things set in water (retting flax,
##   fish traps);
## - world actions: a knife peels birch bark off a birch trunk, tinder fungus takes an ember from a
##   lit fire, a shovel digs an earth oven, a cooking pot fills with brine at the sea, flour wetted
##   at the water makes dough, paint goes on walls (CavePaintings), canoes launch and board
##   (Canoes), megaliths are quarried and moved (Megaliths), fish bite (fishing line, net).
## Slow work runs in game hours of the calendar (now_days), so sleeping counts. Its block entities
## are created by BlockEntities through create_entity().

const SURFACES := ["ground", "stone", "sandstone", "loom"]
const KINDS := ["earth_oven", "rack", "stroke", "loom", "water_work", "megalith", "dugout"]
const HEIGHTS := {"earth_oven": 2, "water_work": 1, "stroke": 1}   # voxels; the rest are full blocks
## Items the world gives without a recipe (the tests' list of sources, beside ground pickups and drops).
const WORLD_SOURCES := {"birch_bark": "a knife on a birch trunk's bark", "tinder_fungus": "peeling birch bark, now and then",
	"fire_ember": "tinder fungus held into a lit fire", "flatbread_dough": "flour wetted at water",
	"brine_pot": "a cooking pot dipped in the sea", "dugout_canoe": "an adze on a lying trunk, burnt out and scraped",
	"standing_stone": "split out of rock with wooden wedges (a megalith, never carried)",
	"einkorn": "sown einkorn seeds", "emmer": "sown emmer seeds", "flax": "sown flax seeds", "raw_fish": "a fishing line or net"}
## Blueprints for the build menu (tools/make_blueprints.py), copied into the prefab folder once.
const BLUEPRINTS := ["longhouse", "small longhouse"]
const DIGGABLE := ["dirt", "grass", "sand", "sand_static", "gravel", "gravel_static", "clay", "farmland", "podzol", "coarse_dirt", "mud"]

## The calendar's days since the world began, with the time of day (set every frame by the node; tests
## set it themselves). Entities measure game hours against it.
static var now_days := 0.0

var main: Node
var clothing: Clothing
var paintings: CavePaintings
var canoes: Canoes
var megaliths: Megaliths
var _shot_cd := 0.0
var _sec := 0.0
var _rng := RandomNumberGenerator.new()
var _fish_until := -1.0         # a line in the water: the bite comes at this game day
var _fish_at := Vector3.ZERO
var stats := {"shots": 0, "launched": 0, "peeled": 0, "ovens_dug": 0, "ovens_lit": 0, "baked": [], "cured": [],
	"strokes": 0, "worked": [], "set_in_water": 0, "caught": [], "worn": [], "embers": 0, "brine": 0, "dough": 0, "fish": 0}


static func create_entity(blocks: BlockEntities, pos: Vector3i, kind: String) -> RefCounted:
	match kind:
		"earth_oven":
			return EarthOvenEntity.new(blocks.recipes, pos)
		"rack":
			return RackEntity.new(blocks.recipes, pos)
		"stroke", "loom":
			return StrokeEntity.new(blocks.recipes, pos, kind)
		"water_work":
			return WaterWorkEntity.new(blocks.recipes, pos)
		"megalith":
			return MegalithEntity.new(pos)
		"dugout":
			return DugoutEntity.new(pos)
	return null


static func ticks(kind: String) -> bool:
	return kind in ["earth_oven", "rack", "water_work", "dugout"]


## The durability multiplier a tool assembled from `parts` gets (ADR 0059): the best binding's haft_x
## (birch tar 2) times the head's dur_x (tempered flint 1.5), the other parts' dur_x ignored.
static func hafted_x(items: ItemRegistry, parts: Array, part_data: Array) -> float:
	var haft := 1.0
	for id in parts:
		haft = maxf(haft, float(items.get_def(String(id)).get("haft_x", 1.0)))
	var head := 1.0
	if not part_data.is_empty() and part_data[0] is Dictionary:
		head = float(part_data[0].get("dur_x", 1.0))
	return snappedf(haft * head, 0.01)


func setup(p_main: Node) -> void:
	main = p_main
	_rng.seed = 5959
	clothing = Clothing.new(main.items)
	if main.survival:
		main.survival.clothing = clothing
	paintings = CavePaintings.new()
	paintings.name = "CavePaintings"
	add_child(paintings)
	canoes = Canoes.new()
	canoes.name = "Canoes"
	add_child(canoes)
	canoes.setup(main)
	megaliths = Megaliths.new()
	megaliths.name = "Megaliths"
	add_child(megaliths)
	megaliths.setup(main)
	for e in main.blocks.all_work():
		_watch(e)
	if Settings.persist_world:
		load_world(main._load_json(main._save_path("stone_age")))
	now_days = main.atmosphere.calendar.total()
	if main.get("builder") != null:
		install_blueprints(main.builder.prefabs.base)


## Copies the shipped blueprints into `base`/blueprints/ where the build menu's Prefabs list finds
## them; a player's own edit of one (same name) is kept.
static func install_blueprints(base: String) -> int:
	var dir := base.path_join("blueprints")
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(dir))
	var n := 0
	for name in BLUEPRINTS:
		var to := dir.path_join(name + ".json")
		if FileAccess.file_exists(to):
			continue
		var text := FileAccess.get_file_as_string("res://data/prefabs/blueprints/%s.json" % name)
		var f := FileAccess.open(to, FileAccess.WRITE)
		if text != "" and f != null:
			f.store_string(text)
			n += 1
	return n


func _watch(e: RefCounted) -> void:
	if e.has_signal("baked") and not e.baked.is_connected(_on_baked):
		e.baked.connect(_on_baked)
	if e.has_signal("cured") and not e.cured.is_connected(_on_cured):
		e.cured.connect(_on_cured)
	if e.has_signal("caught") and not e.caught.is_connected(_on_caught):
		e.caught.connect(_on_caught)


func _on_baked(item: String) -> void:
	stats["baked"].append(item)
	if item == "dugout_canoe":
		main.work._say("The hollow of the canoe has burnt out: scrape it clean with the adze.")
		return
	main.work._note("baked", item)
	main.work._say("An earth oven has burnt down: dig out the %s." % main.items.display_name(item).to_lower())


func _on_cured(item: String, mode: String) -> void:
	stats["cured"].append(item)
	main.work._note("smoked" if mode == "smoke" else "dried", item)


func _on_caught(item: String) -> void:
	stats["caught"].append(item)
	main.work._note("caught", item)


func _process(delta: float) -> void:
	now_days = main.atmosphere.calendar.total()
	_shot_cd = maxf(0.0, _shot_cd - delta)
	_sec += delta
	if _sec >= 1.0:
		_sec = 0.0
		_each_second()
	if _fish_until > 0.0 and now_days >= _fish_until:
		_fish_until = -1.0
		_land_fish()


## Racks smoke over a lit firepit; carried embers go out.
func _each_second() -> void:
	for e in main.blocks.all_of("rack"):
		_watch(e)
		var was: bool = e.smoky
		e.smoky = _fire_near(e.pos)
		if e.smoky:
			main.work._particles("smoke", WorkSystem._at(e.pos, Vector3(0.5, 1.1, 0.5)), 0)
		if was != e.smoky:
			e.changed.emit()
	var inv: Inventory = main.player_inv.inv
	for i in inv.size():
		var s: Dictionary = inv.get_slot(i)
		if not s.is_empty() and s.has("out_at") and float(s["out_at"]) <= now_days:
			inv.set_slot(i, {})
			main.work._say("Your smouldering fungus has gone out.")


## A lit firepit beside the block at p, under it, or one block lower beside it.
func _fire_near(p: Vector3i) -> bool:
	var b := Settings.BLOCK
	for dy in [0, -1]:
		for dz in [-1, 0, 1]:
			for dx in [-1, 0, 1]:
				var e: RefCounted = main.blocks.at(p + Vector3i(dx, dy, dz) * b)
				if e is FirepitEntity and e.is_lit():
					return true
	return false


# ---------------- the right click ----------------

func _consumes() -> bool:
	return GameMode.rule(main.player.game_mode, "consumes_items")


func _sneaking() -> bool:
	return main.player.has_method("sneaking") and main.player.sneaking() or Input.is_action_pressed("sprint")


func use(hit: VoxelRaycastResult, first_press: bool, from: Vector3, dir: Vector3) -> bool:
	var held: Dictionary = main.player_inv.held_def()
	var id: String = held.get("id", "")
	if held.has("launch") or held.has("shoots"):
		if first_press:
			_shoot(held, from, dir)
		return true
	if held.has("clothing"):
		if first_press:
			_wear_clothing()
		return true
	if first_press and canoes.use(held, hit, from, dir):
		return true
	if first_press and main.items.has_tag(id, "meat") and _feed_wolf(hit):
		return true
	if first_press and "fishing" in held.get("tags", []):
		return _fish(hit, held, from, dir)
	if hit == null:
		return false
	var p: Vector3i = main.work._work_at(hit)
	var e: RefCounted = main.blocks.at(p) if p.y >= 0 else null
	if e != null and e.kind in KINDS:
		_use_piece(e, held, first_press, hit, from, dir)
		return true
	if e is FirepitEntity and id == "tinder_fungus" and e.is_lit():
		if first_press:
			_take_ember()
		return true
	if not first_press and not ("knife" in held.get("tags", []) and main.get_world_voxel(hit.position) == GameMaterials.BIRCH_BARK):
		return false
	if "knife" in held.get("tags", []) and main.get_world_voxel(hit.position) == GameMaterials.BIRCH_BARK:
		_peel(hit.position)
		return true
	if held.has("paint"):
		return paintings.paint(main, hit, held, from, dir)
	if "adze" in held.get("tags", []) and _is_wood(main.get_world_voxel(hit.position)):
		start_dugout(hit.position)
		return true
	if megaliths.use(held, hit, from, dir, true):
		return true
	if "shovel" in held.get("tags", []) and _sneaking() == false and _dig_oven(hit):
		return true
	if not WaterWorkEntity.recipe_for(main.recipes, id).is_empty() and _set_in_water(hit, id):
		return true
	if id == "cooking_pot" and _water_bed(hit).y >= 0:
		return _fill_brine(hit)
	if id == "flour" and _water_bed(hit).y >= 0:
		return _wet_flour()
	if id != "" and not _sneaking() and _place_stroke(hit, id):
		return true
	return false


# ---------------- shooting (atlatl, bow) ----------------

## The first slot of the inventory (hotbar first) holding something `spec` matches, or -1.
func _ammo_slot(spec: String) -> int:
	var inv: Inventory = main.player_inv.inv
	for i in inv.size():
		var s: Dictionary = inv.get_slot(i)
		if not s.is_empty() and main.items.matches(s["id"], spec) and i != main.player_inv.selected:
			return i
	return -1


func _shoot(held: Dictionary, from: Vector3, dir: Vector3) -> void:
	if _shot_cd > 0.0:
		return
	var spec: Dictionary = held.get("launch", held.get("shoots", {}))
	var ammo := String(spec.get("ammo", "#arrow"))
	var i := _ammo_slot(ammo)
	var stack := {}
	if i >= 0:
		var inv: Inventory = main.player_inv.inv
		stack = inv.get_slot(i).duplicate(true)
		stack["count"] = 1
		if _consumes():
			stack = Inventory.wear_stack(main.items, inv.take(i, 1))   # a spear wears on each throw
	elif not _consumes():
		var ids: Array = main.items.items_with_tag(ammo.substr(1)) if ammo.begins_with("#") else [ammo]
		stack = {"id": ids[0], "count": 1}
	if stack.is_empty():
		main.work._say("No %s to %s." % [ammo.substr(1) + "s" if ammo.begins_with("#") else main.items.display_name(ammo), "throw" if held.has("launch") else "shoot"])
		return
	main.projectiles.throw(stack, from + dir * 0.6, dir, float(spec.get("speed_x", 1.0)), float(spec.get("damage_x", 1.0)))
	_shot_cd = float(spec.get("cooldown_s", 0.6))
	stats["shots"] += 1
	main.work._wear_held()
	main.work._note("launched", held["id"])
	if main.player.viewmodel:
		main.player.viewmodel.swing()


# ---------------- clothing ----------------

func _wear_clothing() -> void:
	var stack: Dictionary = main.player_inv.held().duplicate(true)
	stack["count"] = 1
	var old := clothing.wear(stack)
	main.player_inv.consume_held(1)
	if not old.is_empty():
		main.work._give([old], main.player.position)
	stats["worn"].append(stack["id"])
	main.work._note("worn", stack["id"])
	main.work._say("You put on the %s (+%d C against the cold, %d C in all)." % [main.items.display_name(stack["id"]).to_lower(),
		int(main.items.get_def(stack["id"])["clothing"].get("warmth_c", 0.0)), int(clothing.warmth_c())])


# ---------------- birch bark and embers ----------------

## A knife takes the bark off one voxel of a birch trunk; the white wood stays. Tinder fungus grows
## on birch and comes off with it now and then.
func _peel(v: Vector3i) -> void:
	main.set_world_voxel(v, GameMaterials.BIRCH_LOG)
	var give := [{"id": "birch_bark", "count": 1}]
	if _rng.randf() < Settings.tinder_fungus_chance:
		give.append({"id": "tinder_fungus", "count": 1})
	main.work._give(give, Vector3(v) + Vector3(0.5, 0.5, 0.5))
	main.work._wear_held()
	main.work._particles("block_hit", Vector3(v) + Vector3(0.5, 0.5, 0.5), GameMaterials.BIRCH_BARK)
	stats["peeled"] += 1


## Tinder fungus held into a lit fire takes an ember that smoulders for ember_hours game hours.
func _take_ember() -> void:
	main.player_inv.consume_held(1)
	var hours := float(main.items.get_def("fire_ember").get("ember_hours", 12.0))
	main.work._give([{"id": "fire_ember", "count": 1, "out_at": now_days + hours / 24.0}], main.player.position)
	stats["embers"] += 1
	main.work._note("ember", "fire_ember")
	main.work._say("The fungus smoulders: it carries the fire for %d game hours." % int(hours))


# ---------------- placing pieces ----------------

## True when the ray hit the top face of its voxel.
static func _top_face(hit: VoxelRaycastResult) -> bool:
	return hit.previous_position == hit.position + Vector3i.UP


func _is_free(cells: Array[Vector3i]) -> bool:
	for c in cells:
		var m: int = main.get_world_voxel(c)
		if m != GameMaterials.AIR and not (m > 0 and GameMaterials.defs()[m - 1].get("replaceable", false)):
			return false
	return true


func _new_piece(p: Vector3i, kind: String) -> RefCounted:
	var e: RefCounted = main.work._create(p, kind, GameMaterials.WORK_SPOT)
	_watch(e)
	return e


## A shovel on the top of soil or sand digs a pit two voxels deep: an earth oven.
func _dig_oven(hit: VoxelRaycastResult) -> bool:
	if not _top_face(hit) or not GameMaterials.name_of(main.get_world_voxel(hit.position)) in DIGGABLE:
		return false   # a shovel on rock does nothing here
	var a := BlockEntities.anchor_for(hit.position)
	var p := Vector3i(a.x, hit.position.y - 1, a.z)
	for c in BlockEntities.cube(p, 2):
		if not GameMaterials.name_of(main.get_world_voxel(c)) in DIGGABLE:
			main.work._say("Dig an earth oven into flat soil or sand (a whole block of it).")
			return true
	for c in BlockEntities.cube(p + Vector3i(0, 2, 0), 1):
		if not _is_free([c]):
			main.work._say("Clear the ground above the pit first.")
			return true
	_new_piece(p, "earth_oven")
	main.work._wear_held()
	main.work._particles("block_hit", WorkSystem._at(p, Vector3(0.5, 0.5, 0.5)), GameMaterials.DIRT)
	stats["ovens_dug"] += 1
	main.work._say("An earth oven: lay birch bark (for tar) or flint (to temper it) in it.")
	return true


## The surface a stroke piece laid on this top face works on: sandstone, stone or ground.
func _surface(m: int) -> String:
	var name := GameMaterials.name_of(GameMaterials.material_of_voxel(m))
	if name.contains("sandstone"):
		return "sandstone"
	var e: Dictionary = main.mining.entry(name) if main.mining else {}
	if e.get("tool", "") == "pickaxe":
		return "stone"
	return "ground"


## Stroke recipes a piece of `id` laid on `surface` may follow: the surface's own, stone ones on
## sandstone too, and ground ones anywhere.
func stroke_options(surface: String, id: String) -> Array:
	var out := StrokeEntity.options(main.recipes, surface, id)
	if surface == "sandstone":
		out.append_array(StrokeEntity.options(main.recipes, "stone", id))
	if surface != "ground" and surface != "loom":
		out.append_array(StrokeEntity.options(main.recipes, "ground", id))
	return out


func _place_stroke(hit: VoxelRaycastResult, id: String) -> bool:
	if not _top_face(hit) or not MobManager.is_solid(main.get_world_voxel(hit.position)):
		return false
	var surface := _surface(main.get_world_voxel(hit.position))
	var opts := stroke_options(surface, id)
	if opts.is_empty():
		return false
	var p := BlockEntities.anchor_for(hit.previous_position)
	if not _is_free(BlockEntities.cube(p, 1)):
		return true
	var e: StrokeEntity = _new_piece(p, "stroke")
	e.station = surface
	e.water_ok = _water_within(p)
	_load_stroke(e, main.player_inv.held_def(), opts)
	return true


## A water voxel within a block of the piece at p (its block and the ring of blocks around it).
func _water_within(p: Vector3i) -> bool:
	var b := Settings.BLOCK
	for dz in range(-b, 2 * b, 2):
		for dx in range(-b, 2 * b, 2):
			for dy in range(-b, b, 2):
				if WorkSystem._is_water(main.get_world_voxel(p + Vector3i(dx, dy, dz))):
					return true
	return false


## The first ingredient goes onto a stroke piece without a recipe: the one recipe it can be, or a
## picker over several (the ingredient lies there meanwhile).
func _load_stroke(e: StrokeEntity, held: Dictionary, opts: Array) -> void:
	var id: String = held.get("id", "")
	if opts.size() == 1:
		e.set_recipe(opts[0])
		_apply(e, e.use(held), held)
		return
	e.loaded.append(id)
	if _consumes():
		main.player_inv.consume_held(1)
	e.changed.emit()
	_pick(e, opts)


func _pick(e: StrokeEntity, opts: Array) -> void:
	main.work._picking = e
	main.player.set_ui_open(true, false)
	main.work.picker.open("Work", opts, main.items, func(r: Dictionary) -> String: return "%d strokes" % int(r["strokes"]))
	if main.player.autopilot:
		main.work.picker.close()   # automation picks through choose()


## Sets a stroke piece's recipe without the picker (automation).
func choose(p: Vector3i, rid: String) -> bool:
	var e: RefCounted = main.blocks.at(p)
	if not (e is StrokeEntity) or not main.recipes.recipes.has(rid):
		return false
	e.set_recipe(main.recipes.recipes[rid])
	if main.work._picking == e:
		main.work._picking = null
		main.work.picker.close()
	return true


## The bed voxel under the water the ray reached (it passes through water), or (-1, -1, -1).
func _water_bed(hit: VoxelRaycastResult) -> Vector3i:
	for w in [hit.previous_position, hit.position]:
		if not WorkSystem._is_water(main.get_world_voxel(w)):
			continue
		for d in range(0, 8):
			var m: int = main.get_world_voxel(w - Vector3i(0, d, 0))
			if not WorkSystem._is_water(m):
				return w - Vector3i(0, d, 0) if MobManager.is_solid(m) else Vector3i(-1, -1, -1)
	if MobManager.is_solid(main.get_world_voxel(hit.position)) and WorkSystem._is_water(main.get_world_voxel(hit.position + Vector3i.UP)):
		return hit.position
	return Vector3i(-1, -1, -1)


## Flax or a fish trap goes onto the bed under the water the player aims at.
func _set_in_water(hit: VoxelRaycastResult, id: String) -> bool:
	var bed := _water_bed(hit)
	if bed.y < 0:
		return false
	var a := BlockEntities.anchor_for(bed + Vector3i.UP)
	var p := Vector3i(a.x, bed.y + 1, a.z)
	for c in BlockEntities.cube(p, 1):
		if not WorkSystem._is_water(main.get_world_voxel(c)):
			main.work._say("Set it on a flat bed under the water (a whole block of water).")
			return true
	var e: WaterWorkEntity = _new_piece(p, "water_work")
	var info: Dictionary = main.world.generator.column_info(p.x, p.z)
	e.river = float(info.get("river", 0.0)) > 0.05
	_apply(e, e.use(main.player_inv.held_def()), main.player_inv.held_def())
	stats["set_in_water"] += 1
	return true


## A cooking pot dipped in the sea comes up with brine (fresh water gives nothing to boil down).
func _fill_brine(hit: VoxelRaycastResult) -> bool:
	var bed := _water_bed(hit)
	var info: Dictionary = main.world.generator.column_info(bed.x, bed.z)
	if int(info.get("height", Settings.SEA_LEVEL_VOXEL)) > Settings.SEA_LEVEL_VOXEL - Settings.m2v(1.0) or bed.y > Settings.SEA_LEVEL_VOXEL:
		main.work._say("This is fresh water. Fill the pot at the sea for brine.")
		return true
	main.player_inv.replace_held_one("brine_pot")
	stats["brine"] += 1
	main.work._particles("splash", Vector3(bed) + Vector3(0.5, 1.5, 0.5), GameMaterials.WATER)
	main.work._say("A pot of brine: boil it dry on a firepit.")
	return true


func _wet_flour() -> bool:
	main.player_inv.consume_held(1)
	main.work._give([{"id": "flatbread_dough", "count": 1}], main.player.position)
	stats["dough"] += 1
	return true


# ---------------- working the pieces ----------------

func _use_piece(e: RefCounted, held: Dictionary, first_press: bool, hit: VoxelRaycastResult, from: Vector3, dir: Vector3) -> void:
	_watch(e)   # pieces WorkSystem placed (racks, looms) report to the book too
	if e is MegalithEntity:
		megaliths.use_stone(e, held, first_press, from, dir)
		return
	if e is DugoutEntity:
		if not first_press and not (e.burnt and "adze" in held.get("tags", [])):
			return   # holding right click keeps scraping
		var res: Dictionary = e.use(held)
		_apply(e, res, held, first_press)
		if res["lit"]:
			main.work._wear_held()   # the firestarter
		return
	if e is StrokeEntity:
		if not first_press and not (e.ready() and main.items.matches(held.get("id", ""), String(e.recipe.get("tool", "")))):
			return   # holding right click keeps stroking
		if not e.has_recipe() and held.get("id", "") != "":
			var opts := stroke_options(e.station, held["id"]) if e.loaded.is_empty() else stroke_options(e.station, e.loaded[0])
			if opts.is_empty():
				main.work._say("Nothing to work with that here.")
			elif e.loaded.is_empty():
				_load_stroke(e, held, opts)
			else:
				_pick(e, opts)
			return
		_apply(e, e.use(held), held, first_press)
		return
	if not first_press:
		return
	if e is EarthOvenEntity:
		var res: Dictionary = e.use(held)
		_apply(e, res, held)
		if res["lit"]:
			stats["ovens_lit"] += 1
			main.work._wear_held()   # the firestarter
		return
	_apply(e, e.use(held), held)


## Applies what a piece's use() answered: pay for what went on, hand out what came off, stroke
## effects and wear, a finished piece popping its result out, a removed piece, a message.
func _apply(e: RefCounted, res: Dictionary, held: Dictionary, say: bool = true) -> void:
	if int(res.get("consume", 0)) > 0 and _consumes():
		main.player_inv.consume_held(int(res["consume"]))
	main.work._give(res.get("give", []), WorkSystem._at(e.pos, Vector3(0.5, 0.5, 0.5)))
	if res.get("stroke", false):
		stats["strokes"] += 1
		main.work._wear_held()
		var mat := GameMaterials.BIRCH_LOG if e is DugoutEntity else (GameMaterials.SANDSTONE if e.station == "sandstone" else (GameMaterials.STONE if e.station == "stone" else GameMaterials.DIRT))
		main.work._particles("block_hit", WorkSystem._at(e.pos, Vector3(0.5, 0.1, 0.5)), mat)
	if res.get("pick", false):
		_pick(e, stroke_options(e.station, e.loaded[0]))
	var done: Dictionary = res.get("done", {})
	if not done.is_empty():
		stats["worked"].append(done["id"])
		main.work._note("worked", done["id"])
		main.spawn_item(done, WorkSystem._at(e.pos, Vector3(0.5, 0.3, 0.5)), Vector3(0, 1.5, 0))
	if res.get("remove", false):
		main.work.remove_piece(e, GameMaterials.WATER_STATIC if e is WaterWorkEntity else GameMaterials.AIR)
	if say and String(res.get("msg", "")) != "":
		main.work._say(res["msg"])


# ---------------- the dugout canoe ----------------

## A voxel of a tree's wood (logs, bark): what an axe must cut.
func _is_wood(m: int) -> bool:
	var e: Dictionary = main.mining.entry(GameMaterials.name_of(GameMaterials.material_of_voxel(m))) if main.mining and m > 0 else {}
	return e.get("tool", "") == "axe" and e.get("requires", false)


## An adze on a lying trunk: finds a WIDE x HIGH box of wood at least Settings.dugout_min_m long through
## the clicked voxel, along x or z, and makes it a dugout blank. Bounded scans (a few thousand voxels,
## once per click).
func start_dugout(v: Vector3i) -> bool:
	var need := int(Settings.m2v(Settings.dugout_min_m))
	var best := {}
	for ax in [Vector3i(1, 0, 0), Vector3i(0, 0, 1)]:
		var side := Vector3i(0, 0, 1) if ax.x != 0 else Vector3i(1, 0, 0)
		for up in range(-DugoutEntity.HIGH + 1, 1):
			for across in range(-DugoutEntity.WIDE + 1, 1):
				var o := v + side * across + Vector3i(0, up, 0)
				var run := _wood_run(o, ax, side, need * 2)
				if run["len"] >= need and run["len"] > int(best.get("len", 0)):
					best = {"len": run["len"], "start": run["start"], "ax": ax}
	if best.is_empty():
		main.work._say("Mark out a canoe on a lying trunk at least %.1f m long and about 0.6 m thick (or logs laid in a row)." % Settings.dugout_min_m)
		return false
	var ax: Vector3i = best["ax"]
	var e: DugoutEntity = main.blocks.create(best["start"], "dugout")
	e.axis = "x" if ax.x != 0 else "z"
	e.length = mini(int(best["len"]), need + Settings.BLOCK)
	main.blocks._register_cells(e)
	for c in e.footprint_cells():
		main.set_world_voxel(c, GameMaterials.WORK_BLOCK)
	main.work._watch(e)
	_watch(e)
	main.work._wear_held()
	main.work._say("A canoe blank. Lay four dry grass along it and light it to burn out the hollow.")
	return true


## The longest run along `ax` through `o` whose WIDE x HIGH cross-sections (across `side`, up) are all
## wood: {start, len}.
func _wood_run(o: Vector3i, ax: Vector3i, side: Vector3i, cap: int) -> Dictionary:
	var ok := func(p: Vector3i) -> bool:
		for y in DugoutEntity.HIGH:
			for s in DugoutEntity.WIDE:
				if not _is_wood(main.get_world_voxel(p + side * s + Vector3i(0, y, 0))):
					return false
		return true
	if not ok.call(o):
		return {"start": o, "len": 0}
	var back := 0
	while back < cap and ok.call(o - ax * (back + 1)):
		back += 1
	var fwd := 0
	while back + fwd < cap and ok.call(o + ax * (fwd + 1)):
		fwd += 1
	return {"start": o - ax * back, "len": back + fwd + 1}


# ---------------- the dog ----------------

## Raw meat held out to a wolf (part E's species): after Settings.tame_meat pieces it is tamed. The
## mob keeps "tamed_by" as meta; following and hunting with the player are the mob code's (part E,
## Mob.set_companion when it exists).
func _feed_wolf(hit: VoxelRaycastResult) -> bool:
	var mob = main.player.aimed_mob(hit)
	if mob == null or String(mob.species) != "wolf" or mob.is_dead():
		return false
	if mob.has_meta("tamed_by"):
		main.work._say("Your dog wags its tail.")
		return true
	var fed := int(mob.get_meta("tame_fed", 0)) + 1
	mob.set_meta("tame_fed", fed)
	if _consumes():
		main.player_inv.consume_held(1)
	if fed < Settings.tame_meat:
		main.work._say("The wolf takes the meat, warily (%d/%d)." % [fed, Settings.tame_meat])
		return true
	mob.set_meta("tamed_by", "player")
	if mob.has_method("set_companion"):
		mob.set_companion(main.player)
	stats["tamed"] = int(stats.get("tamed", 0)) + 1
	main.work._note("tamed", "raw_meat")
	main.work._say("The wolf trusts you now: it is your dog.")
	return true


# ---------------- fishing (line and net) ----------------

## A fishing line or net cast at water: the line bites after a few game minutes, the net now and then
## brings a fish up at once. Rivers and lakes alike; part E's fish swim there.
func _fish(hit: VoxelRaycastResult, held: Dictionary, _from: Vector3, _dir: Vector3) -> bool:
	if hit == null or _water_bed(hit).y < 0:
		return false
	var at := Vector3(_water_bed(hit)) + Vector3(0.5, 1.0, 0.5)
	main.work._particles("splash", at, GameMaterials.WATER)
	main.work._wear_held()
	if "net" in held.get("tags", []):
		if _rng.randf() < Settings.net_catch_chance:
			_give_fish(at)
		else:
			main.work._say("The net comes up empty.")
		return true
	if _fish_until > 0.0:
		main.work._say("Your line is already in the water.")
		return true
	_fish_at = at
	_fish_until = now_days + _rng.randf_range(Settings.fish_bite_min_h, Settings.fish_bite_max_h) / 24.0
	main.work._say("You cast your line. Wait for a bite.")
	return true


func _land_fish() -> void:
	if main.player.position.distance_to(_fish_at) > Settings.m2v(12.0):
		main.work._say("The fish got away: you walked off from your line.")
		return
	_give_fish(main.player.position)


func _give_fish(at: Vector3) -> void:
	main.work._give([{"id": "raw_fish", "count": 1}], at)
	stats["fish"] += 1
	main.work._note("caught", "raw_fish")
	main.work._say("A fish!")


# ---------------- saving ----------------

func world_dict() -> Dictionary:
	return {"version": 1, "paintings": paintings.to_array(), "canoes": canoes.to_array(), "megaliths": megaliths.to_dict()}


func load_world(d: Dictionary) -> void:
	if d.is_empty():
		return
	paintings.load_array(main, d.get("paintings", []))
	canoes.load_array(d.get("canoes", []))
	megaliths.load_dict(d.get("megaliths", {}))


func save_world() -> void:
	if Settings.persist_world:
		main._save_json(main._save_path("stone_age"), world_dict())


func player_dict() -> Dictionary:
	return {"clothing": clothing.to_dict()}


func load_player(d: Dictionary) -> void:
	clothing.load_dict(d.get("clothing", {}))


func summary() -> Dictionary:
	var s := stats.duplicate(true)
	s["clothing"] = clothing.summary()
	s["paintings"] = paintings.count()
	s["canoes"] = canoes.summary()
	s["megaliths"] = megaliths.summary()
	s["now_days"] = snappedf(now_days, 0.001)
	return s
