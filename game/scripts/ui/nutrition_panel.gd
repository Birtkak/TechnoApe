class_name NutritionPanel
extends Control

## The diet in the inventory screen (ADR 0061): one bar per food group (Nutrition), the "good" mark
## on each, and what the diet does to max health. Docked right of the inventory panel, drawn in its
## style (grey panel, dark border). Only draws; Survival owns the numbers.

const W := 214.0
const BAR_H := 10.0
const ROW := 28.0
const HEAD := 34.0
const INK := Color(0.2, 0.2, 0.2)

var nutrition: Nutrition
var base_health := 20


func _init() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(W, HEAD + 5 * ROW + 30)


func _process(_d: float) -> void:
	if is_visible_in_tree():
		queue_redraw()


func _draw() -> void:
	if nutrition == null:
		return
	size.y = HEAD + nutrition.groups.size() * ROW + 30
	var font := ThemeDB.fallback_font
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.776, 0.776, 0.776)
	sb.border_color = INK
	sb.set_border_width_all(3)
	sb.set_corner_radius_all(6)
	draw_style_box(sb, Rect2(Vector2.ZERO, size))
	draw_string(font, Vector2(14, 24), "Diet", HORIZONTAL_ALIGNMENT_LEFT, -1, 16, Color(0.25, 0.25, 0.25))
	var y := HEAD
	var bx := 92.0
	var bw := W - bx - 14.0
	for g in nutrition.groups:
		var v := float(nutrition.bars.get(g["id"], 0.0))
		var f := clampf(v / nutrition.max_points, 0.0, 1.0)
		var col := Color(String(g.get("color", "#cccccc")))
		var low := v < nutrition.empty
		draw_string(font, Vector2(14, y + 11), String(g["name"]), HORIZONTAL_ALIGNMENT_LEFT, bx - 18, 13, Color(0.6, 0.1, 0.08) if low else INK)
		# a sunken trough like the inventory slots: dark top-left, white bottom-right
		draw_rect(Rect2(bx - 2, y, bw + 4, BAR_H + 4), Color(0.22, 0.22, 0.22))
		draw_rect(Rect2(bx, y + 2, bw, BAR_H), Color(0.1, 0.1, 0.12))
		draw_rect(Rect2(bx, y + 2, bw * f, BAR_H), col.darkened(0.5) if low else col)
		var gx := bx + bw * nutrition.good / nutrition.max_points
		draw_line(Vector2(gx, y), Vector2(gx, y + BAR_H + 4), Color(1, 1, 1, 0.9), 1.0)
		draw_string(font, Vector2(bx, y + BAR_H + 13), "%s / %s" % [_num(v), _num(nutrition.max_points)], HORIZONTAL_ALIGNMENT_LEFT, bw, 9, Color(0.35, 0.35, 0.35))
		y += ROW
	var d := nutrition.max_health(base_health) - base_health
	var hearts := absf(d) / 2.0
	var text := "Balanced: +%s hearts" % _num(hearts) if d > 0 else ("Missing groups: -%s hearts" % _num(hearts) if d < 0 else "Eat 2+ groups for more hearts")
	draw_string(font, Vector2(14, y + 14), text, HORIZONTAL_ALIGNMENT_LEFT, W - 24, 11,
		Color(0.1, 0.45, 0.1) if d > 0 else (Color(0.6, 0.1, 0.08) if d < 0 else Color(0.3, 0.3, 0.3)))


static func _num(v: float) -> String:
	return str(int(v)) if is_equal_approx(v, roundf(v)) else "%.1f" % v
