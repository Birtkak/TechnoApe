class_name StainPanel
extends Control

## The active stains as icons in a column under the clothing slots, docked left of the inventory
## (InventoryScreen.add_side_panel). Hover one for what it does. While the inventory is open they
## are drawn here instead of beside the hotbar (bug report 2026-10-27_17-55-24).

const ICON := 40.0
const WHAT := {
	"wet": "Wet: puts fire out and keeps it off.",
	"bloody": "Bloody: cosmetic, drips on the ground.",
	"oily": "Oily: catching fire burns twice as long.",
	"toxic": "Toxic: hurts half a heart every 2 s while it lasts.",
	"burning": "Burning: hurts half a heart a second. Water puts it out.",
}

var stains: PlayerStains


func _init() -> void:
	size = Vector2(ICON, ICON * PlayerStains.KINDS.size())


func _process(_delta: float) -> void:
	if visible:
		queue_redraw()


func _draw() -> void:
	var sprites := ItemSprites.get_default()
	var i := 0
	for k in stains.active():
		var r := Rect2(0, i * ICON, ICON - 4, ICON - 4)
		draw_rect(r, Color(0.08, 0.08, 0.1, 0.85))
		var tex := sprites.texture("stain_" + k)
		if tex:
			draw_texture_rect(tex, r.grow(-2), false)
		draw_rect(Rect2(r.position.x, r.end.y - 4, r.size.x * clampf(float(stains.amount[k]), 0.0, 1.0), 4), PlayerStains.COLORS[k])
		i += 1


func _get_tooltip(at_position: Vector2) -> String:
	var active := stains.active()
	var i := int(at_position.y / ICON)
	return WHAT[active[i]] if i < active.size() and at_position.x < ICON - 4 else ""
