class_name SkyCommands
extends RefCounted

## Calendar and sky commands (ADR 0025), registered by GameCommands:
##   /date [set <month> <day>]   the date, season, moon, day length; jump to a date
##   /moon                       tonight's phase, name, size and rise
##   /eclipse [solar|lunar]      when the next one is; "/eclipse goto" jumps to it
##   /constellations [on|off]    draw the constellation figures over the stars

var atmo: Node   # Atmosphere


func _init(p_atmo: Node) -> void:
	atmo = p_atmo


func register(reg: CommandRegistry) -> void:
	var months: Array = []
	for m in Calendar.MONTHS:
		months.append(String(m).to_lower())
	reg.register("date", "/date [set <month> <day>]", "Show the date, season and moon, or jump forward to a date", _date, [["set"], months])
	reg.register("moon", "/moon", "Tonight's moon: phase, name, size", _moon)
	reg.register("eclipse", "/eclipse [solar|lunar] [goto]", "When the next eclipse is; goto jumps to it", _eclipse, [["solar", "lunar", "goto"], ["goto"]])
	reg.register("constellations", "/constellations [on|off]", "Draw the constellation figures between the stars", _constellations, [["on", "off"]])


func _cal() -> Calendar:
	return atmo.calendar if atmo != null else null


func _date(args: PackedStringArray) -> Dictionary:
	var c := _cal()
	if c == null:
		return CommandRegistry.fail("No calendar")
	if args.is_empty():
		return CommandRegistry.ok("%s. Day %.1f h (sunrise %s, sunset %s)." % [c.date_text(), c.day_length_hours(), _clock(c.sunrise()), _clock(c.sunset())])
	if args[0] != "set" or args.size() < 3:
		return CommandRegistry.fail("Usage: /date set <month> <day>")
	var m := Calendar.MONTHS.find(args[1].capitalize())
	if m < 0 and args[1].is_valid_int():
		m = int(args[1]) - 1
	if m < 0 or m > 11 or not args[2].is_valid_int():
		return CommandRegistry.fail("Usage: /date set <month> <day>, e.g. /date set september 5")
	var d := int(args[2])
	if d < 1 or d > c.month_days:
		return CommandRegistry.fail("Days run 1..%d in every month" % c.month_days)
	c.set_date(m, d - 1)
	atmo.set_time_of_day(c.time)   # refresh the sky
	return CommandRegistry.ok("It is now %s" % c.date_text())


func _moon(_args: PackedStringArray) -> Dictionary:
	var c := _cal()
	if c == null:
		return CommandRegistry.fail("No calendar")
	var name := c.moon_name()
	var up := rad_to_deg(asin(clampf(c.moon_dir().y, -1.0, 1.0)))
	return CommandRegistry.ok("%s, %d%% lit%s, size %.2f, %s" % [c.phase_name(), int(c.moon_illumination() * 100.0),
		(" - the %s" % name) if name != "" else "", c.moon_size(), ("%d deg up" % int(up)) if up > 0.0 else "below the horizon"])


func _eclipse(args: PackedStringArray) -> Dictionary:
	var c := _cal()
	if c == null:
		return CommandRegistry.fail("No calendar")
	var kind := ""
	var go := false
	for a in args:
		match a.to_lower():
			"solar", "lunar": kind = a.to_lower()
			"goto": go = true
			_: return CommandRegistry.fail("Usage: /eclipse [solar|lunar] [goto]")
	var e := c.next_eclipse(kind)
	if e.is_empty():
		return CommandRegistry.fail("No eclipse in the next 60 years")
	var when := Calendar.new()
	when.month_days = c.month_days
	when.start_day_of_year = c.start_day_of_year
	when.time = 0.0
	when.advance(float(e["t"]))
	var text := "%s %s eclipse on %s at %s" % ["Total" if e["total"] else "Partial", e["kind"], when.date_text(), _clock(when.time)]
	if go:
		# arrive a little before first contact
		c.advance(float(e["t"]) - c.total() - Calendar.ECLIPSE_HALF_DAYS * 1.2)
		atmo.set_time_of_day(c.time)
		return CommandRegistry.ok("Jumped ahead: " + text)
	return CommandRegistry.ok("Next: " + text)


func _constellations(args: PackedStringArray) -> Dictionary:
	var on: bool = not atmo.show_constellations if args.is_empty() else args[0].to_lower() == "on"
	atmo.set_show_constellations(on)
	return CommandRegistry.ok("Constellations %s" % ("shown" if on else "hidden"))


static func _clock(t: float) -> String:
	var h := fposmod(t * 24.0, 24.0)
	return "%02d:%02d" % [int(h), int(fmod(h, 1.0) * 60.0)]
