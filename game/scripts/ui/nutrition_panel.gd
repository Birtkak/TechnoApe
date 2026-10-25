class_name NutritionPanel
extends Control

## The diet in the inventory screen (ADR 0061): one bar per food group (Nutrition), the "good" mark
## on each, and what the diet does to max health. Only draws; Survival owns the numbers.

const W := 190.0
const BAR_H := 7.0
const ROW := 16.0

var nutrition: Nutrition
var base_health := 20


func _init() -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	size = Vector2(W, 26 + 5 * ROW + 20)


func _process(_d: float) -> void:
	if is_visible_in_tree():
		queue_redraw()


func _draw() -> void:
	if nutrition == null:
		return
	size.y = 26 + nutrition.groups.size() * ROW + 20
	var font := ThemeDB.fallback_font
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.78, 0.78, 0.78, 0.96)
	sb.border_color = Color(0.25, 0.25, 0.25)
	sb.set_border_width_all(2)
	sb.set_corner_radius_all(4)
	draw_style_box(sb, Rect2(Vector2.ZERO, size))
	draw_string(font, Vector2(10, 18), "Diet", HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(0.25, 0.25, 0.25))
	var y := 26.0
	var bx := 82.0
	var bw := W - bx - 10.0
	for g in nutrition.groups:
		var v := float(nutrition.bars.get(g["id"], 0.0))
		var f := clampf(v / nutrition.max_points, 0.0, 1.0)
		var col := Color(String(g.get("color", "#cccccc")))
		draw_string(font, Vector2(10, y + 9), String(g["name"]), HORIZONTAL_ALIGNMENT_LEFT, bx - 14, 11, Color(0.2, 0.2, 0.2))
		draw_rect(Rect2(bx, y + 2, bw, BAR_H), Color(0.15, 0.15, 0.17))
		draw_rect(Rect2(bx, y + 2, bw * f, BAR_H), col if v >= nutrition.empty else col.darkened(0.5))
		var gx := bx + bw * nutrition.good / nutrition.max_points
		draw_line(Vector2(gx, y), Vector2(gx, y + BAR_H + 4), Color(1, 1, 1, 0.9), 1.0)
		y += ROW
	var d := nutrition.max_health(base_health) - base_health
	var hearts := absf(d) / 2.0
	var text := "Balanced: +%s hearts" % _num(hearts) if d > 0 else ("Missing groups: -%s hearts" % _num(hearts) if d < 0 else "Eat from 2+ groups for more hearts")
	draw_string(font, Vector2(10, y + 12), text, HORIZONTAL_ALIGNMENT_LEFT, W - 16, 11,
		Color(0.1, 0.45, 0.1) if d > 0 else (Color(0.6, 0.1, 0.08) if d < 0 else Color(0.3, 0.3, 0.3)))


static func _num(v: float) -> String:
	return str(int(v)) if is_equal_approx(v, roundf(v)) else "%.1f" % v
