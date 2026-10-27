class_name BlastFurnaceEntity
extends BloomeryEntity

## The blast furnace (ADR 0099): a fire-brick shaft with steel bands and steam bellows. Iron ingots
## go in and coal under, like the bloomery, but the charge only works while a boiler on its pipes
## blows the fire ("smelt" recipes with station "blast_furnace": an iron ingot to a steel ingot).
## Without steam the fuel burns on and the charge waits. Draws steam_per_use once a second.

var boiler: BoilerEntity = null
var _draw := 0


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos)
	kind = "blast_furnace"
	title = "Blast furnace"
	station = "blast_furnace"


func powered() -> bool:
	return boiler != null and boiler.pressure > 0.0


func tick() -> bool:
	var was := cook
	var made_was := made
	super.tick()
	if is_lit() and made == made_was and not _blast():
		cook = was   # no blast: the charge waits (a charge on its very last tick still finishes)
	return false


## True while steam keeps the blast up; one draw a second.
func _blast() -> bool:
	if not powered():
		return false
	_draw = (_draw + 1) % 20
	return _draw != 0 or boiler.draw(Settings.steam_per_use)


func look_lines() -> Array[String]:
	var out: Array[String] = []
	if boiler == null:
		out.append("No steam: lay pipes from a boiler to blow it")
	elif boiler.pressure <= 0.0:
		out.append("No pressure: the boiler is cold")
	else:
		out.append("Blast at %.2f bar" % boiler.pressure)
	out.append_array(super())
	return out


## The bloomery's stepped shaft with two steel bands round it and the bellows pipe up the back.
func view_parts() -> Dictionary:
	var parts := super()
	var size: Vector3i = parts["solid"]["size"]
	var c: PackedInt32Array = parts["solid"]["cells"]
	var band := WorkPalette.ANVIL if powered() else WorkPalette.IRON_DARK
	WorkPalette.wall_box(c, size, Vector3i(0, 3, 0), Vector3i(8, 4, 8), [band], band)
	WorkPalette.wall_box(c, size, Vector3i(1, 7, 1), Vector3i(7, 8, 7), [band], band)
	WorkPalette.box(c, size, Vector3i(3, 0, 0), Vector3i(5, 6, 1), WorkPalette.BRONZE)   # the bellows pipe, at the back
	return parts
