class_name Calendar
extends RefCounted

## The game calendar and the sky above it (ADR 0025): 12 months of
## `month_days` days, four seasons, and where the sun, moon and stars stand for
## an observer at `latitude_deg`, with real geometry:
## - the sun moves along the ecliptic once a year (obliquity 23.44 deg), so days
##   are long in summer, short in winter, and the noon sun is high or low;
## - one month is one lunar cycle: new moon at noon on the 1st, full moon at
##   midnight before the (month_days / 2 + 1)th, so the phase follows the day of
##   the month and moonrise, crescents and the terminator behave like the real moon;
## - the moon's orbit is tilted 5.145 deg to the ecliptic and its nodes regress
##   once per 18.6 years, so eclipses happen only when a new or full moon falls
##   near a node: rare, with real limits;
## - the stars turn with sidereal time, so the evening constellations change
##   through the year.
## Pure model: no nodes, no rendering (scripts/visuals/atmosphere.gd draws it).
## `day` counts whole days since the world began, `time` is local solar time
## (0 midnight, 0.5 noon).

const MONTHS := ["January", "February", "March", "April", "May", "June", "July",
	"August", "September", "October", "November", "December"]
const SEASONS := ["Winter", "Spring", "Summer", "Autumn"]
## Traditional full moon names, by month.
const FULL_MOONS := ["Wolf Moon", "Snow Moon", "Worm Moon", "Pink Moon", "Flower Moon", "Strawberry Moon",
	"Buck Moon", "Sturgeon Moon", "Harvest Moon", "Hunter's Moon", "Beaver Moon", "Cold Moon"]

const OBLIQUITY := deg_to_rad(23.44)
const MOON_INCLINATION := deg_to_rad(5.145)
const NODE_PERIOD_YEARS := 18.61          # the nodes regress once per Saros-scale cycle
const ANOMALISTIC_RATIO := 27.5546 / 29.5306   # perigee comes round a little faster than the phases
## The moon's mean apparent size against the sun's (0.518 / 0.533 deg): a central
## eclipse is total when the moon is near perigee, annular when it is far.
const MOON_SUN_RATIO := 0.97
## Eclipse limits on the moon's ecliptic latitude at new / full moon (degrees):
## the sun and moon discs touch (solar), the moon touches / fits inside the
## earth's umbra (lunar partial / total).
const SOLAR_LIMIT_DEG := 0.55
const LUNAR_LIMIT_DEG := 0.96
const LUNAR_TOTAL_DEG := 0.44
## Half the length of an eclipse in days: real eclipses last a few hours; at
## 20-minute days an honest contact time would be 30 s, so the sky plays it slower.
const ECLIPSE_HALF_DAYS := 0.06

var month_days := 9
var latitude_deg := 45.0
var start_day_of_year := 27       # the world begins on 1 April of year 1
var day := 0
var time := 0.4
var node_offset_deg := 137.0      # where the nodes stand on day 0 (decides which months get eclipses)


func year_days() -> int:
	return 12 * month_days


## Days since the world began, with the time of day as the fraction.
func total() -> float:
	return float(day) + time


func day_of_year() -> int:
	return (day + start_day_of_year) % year_days()


func year() -> int:
	return (day + start_day_of_year) / year_days() + 1


func month() -> int:
	return day_of_year() / month_days


## 0-based; the date shows it as 1..month_days.
func day_of_month() -> int:
	return day_of_year() % month_days


## 0 Winter (Dec-Feb), 1 Spring, 2 Summer, 3 Autumn.
func season() -> int:
	return ((month() + 1) % 12) / 3


func season_name() -> String:
	return SEASONS[season()]


## Fraction of the year, 0 at 1 January.
func year_fraction() -> float:
	return (float(day_of_year()) + time) / float(year_days())


func date_text() -> String:
	return "%d %s, year %d (%s)" % [day_of_month() + 1, MONTHS[month()], year(), season_name()]


## Advances the clock by `days` (fractions allowed), rolling the day counter.
func advance(days: float) -> void:
	var t := time + days
	var whole := floori(t)
	day += whole
	time = t - float(whole)


## Moves forward to the next time `t` (0..1) comes round; never back in time,
## so sleeping and /time set move the moon and the seasons on.
func advance_to(t: float) -> void:
	t = fposmod(t, 1.0)
	var dt := fposmod(t - time, 1.0)
	advance(dt)


func set_date(month_index: int, day_index: int, p_time := -1.0) -> void:
	var target := clampi(month_index, 0, 11) * month_days + clampi(day_index, 0, month_days - 1)
	var cur := day_of_year()
	day += posmod(target - cur, year_days())
	if p_time >= 0.0:
		time = fposmod(p_time, 1.0)


# ---------------- the sun ----------------

## Day of the year (fractional) of the March equinox: two thirds into March, as on 20 March.
func equinox_day() -> float:
	return 2.0 * month_days + month_days * 2.0 / 3.0


func sun_longitude_at(t_total: float) -> float:
	var doy := fposmod(t_total + start_day_of_year, float(year_days()))
	return TAU * (doy - equinox_day()) / float(year_days())


## Ecliptic (longitude, latitude) -> equatorial (right ascension, declination), radians.
static func ecliptic_to_equatorial(lon: float, lat: float) -> Vector2:
	var ra := atan2(sin(lon) * cos(OBLIQUITY) - tan(lat) * sin(OBLIQUITY), cos(lon))
	var dec := asin(sin(lat) * cos(OBLIQUITY) + cos(lat) * sin(OBLIQUITY) * sin(lon))
	return Vector2(fposmod(ra, TAU), dec)


func sun_equatorial_at(t_total: float) -> Vector2:
	return ecliptic_to_equatorial(sun_longitude_at(t_total), 0.0)


## Local sidereal time (radians): the sun's right ascension plus its hour angle.
func sidereal_time_at(t_total: float) -> float:
	var tod := t_total - floorf(t_total)
	return fposmod(sun_equatorial_at(t_total).x + (tod - 0.5) * TAU, TAU)


## Celestial (equatorial cartesian: x to RA 0, y to RA 90 deg, z to the north pole)
## -> world (x east, y up, z south; north is -z as in the generator).
func celestial_to_world_at(t_total: float) -> Basis:
	var lst := sidereal_time_at(t_total)
	var phi := deg_to_rad(latitude_deg)
	# rotate by -lst about the pole: (x', y', z') then to (east, up, south)
	var c := cos(lst)
	var s := sin(lst)
	var rz := Basis(Vector3(c, -s, 0.0), Vector3(s, c, 0.0), Vector3(0, 0, 1))   # columns: images of x, y, z
	var local := Basis(Vector3(0.0, cos(phi), sin(phi)), Vector3(1, 0, 0), Vector3(0.0, sin(phi), -cos(phi)))
	return local * rz


func celestial_to_world() -> Basis:
	return celestial_to_world_at(total())


static func equatorial_vector(radec: Vector2) -> Vector3:
	return Vector3(cos(radec.y) * cos(radec.x), cos(radec.y) * sin(radec.x), sin(radec.y))


func sun_dir_at(t_total: float) -> Vector3:
	return celestial_to_world_at(t_total) * equatorial_vector(sun_equatorial_at(t_total))


func sun_dir() -> Vector3:
	return sun_dir_at(total())


## Time of day (0..1) of sunrise and sunset today, from the sun's declination at
## noon (-0.833 deg: refraction and the disc's radius, as almanacs use).
func sunrise() -> float:
	return 0.5 - _half_day()


func sunset() -> float:
	return 0.5 + _half_day()


func _half_day() -> float:
	var dec := sun_equatorial_at(float(day) + 0.5).y
	var phi := deg_to_rad(latitude_deg)
	var c := (sin(deg_to_rad(-0.833)) - sin(phi) * sin(dec)) / (cos(phi) * cos(dec))
	return acos(clampf(c, -1.0, 1.0)) / TAU


func day_length_hours() -> float:
	return _half_day() * 48.0


# ---------------- the moon ----------------

## Moon age as a fraction of the cycle: 0 new (noon on the 1st), 0.5 full.
func moon_age_at(t_total: float) -> float:
	var doy := fposmod(t_total + start_day_of_year, float(year_days()))
	var dom := fposmod(doy, float(month_days))
	return fposmod((dom - 0.5) / float(month_days), 1.0)


func moon_age() -> float:
	return moon_age_at(total())


## Lit fraction of the disc (0 new .. 1 full).
func moon_illumination() -> float:
	return (1.0 - cos(moon_age() * TAU)) * 0.5


func node_longitude_at(t_total: float) -> float:
	return deg_to_rad(node_offset_deg) - TAU * t_total / (NODE_PERIOD_YEARS * year_days())


func moon_ecliptic_at(t_total: float) -> Vector2:
	var lon := sun_longitude_at(t_total) + moon_age_at(t_total) * TAU
	var lat := MOON_INCLINATION * sin(lon - node_longitude_at(t_total))
	return Vector2(lon, lat)


func moon_dir_at(t_total: float) -> Vector3:
	var e := moon_ecliptic_at(t_total)
	return celestial_to_world_at(t_total) * equatorial_vector(ecliptic_to_equatorial(e.x, e.y))


func moon_dir() -> Vector3:
	return moon_dir_at(total())


## Apparent size against its mean: the perigee cycle (+-5.5 %, a "supermoon" when it
## meets a full moon), then the named moons of autumn drawn larger near full.
func moon_distance_factor_at(t_total: float) -> float:
	return 1.0 + 0.055 * cos(TAU * t_total / (month_days * ANOMALISTIC_RATIO))


func moon_size() -> float:
	var t := total()
	var perigee := moon_distance_factor_at(t)
	var full := smoothstep(0.75, 0.95, moon_illumination())
	var boost := 0.0
	match month():
		8: boost = 0.30      # Harvest Moon
		9: boost = 0.15      # Hunter's Moon
	return perigee * (1.0 + boost * full)


## The name of tonight's moon when it is (nearly) full, else "".
func moon_name() -> String:
	if moon_illumination() < 0.9:
		return ""
	var e := eclipse_at(total())
	if e["kind"] == "lunar" and e["total"]:
		return "Blood Moon"
	var perigee := cos(TAU * total() / (month_days * ANOMALISTIC_RATIO))
	var n: String = FULL_MOONS[month()]
	return ("Super " + n) if perigee > 0.82 else n


func phase_name() -> String:
	var a := moon_age()
	var names := ["New Moon", "Waxing Crescent", "First Quarter", "Waxing Gibbous", "Full Moon",
		"Waning Gibbous", "Last Quarter", "Waning Crescent"]
	return names[int(floor(a * 8.0 + 0.5)) % 8]


# ---------------- eclipses ----------------

## The new-moon (solar) or full-moon (lunar) instant nearest `t_total`, and the
## moon's ecliptic latitude there. Day indices are whole: new moon at noon on the
## 1st, full moon at midnight in the middle of the month.
func _syzygy_near(t_total: float) -> Dictionary:
	var md := float(month_days)
	var doy0 := float(start_day_of_year)
	# start of the month that holds t
	var abs_days := t_total + doy0
	var month_start := floorf(abs_days / md) * md - doy0
	var new_t := month_start + 0.5
	var full_t := new_t + md * 0.5
	var cands := [[new_t - md, "solar"], [new_t, "solar"], [new_t + md, "solar"], [full_t - md, "lunar"], [full_t, "lunar"]]
	var best: Array = cands[0]
	for c in cands:
		if absf(c[0] - t_total) < absf(best[0] - t_total):
			best = c
	return {"t": best[0], "kind": best[1], "lat": rad_to_deg(moon_ecliptic_at(best[0]).y)}


## {kind: "solar" | "lunar" | "", progress: -1..1 through the eclipse, miss: closest
## approach / limit (0 = central), total: bool, annular: bool (central, moon too far to cover the sun)}.
func eclipse_at(t_total: float) -> Dictionary:
	var none := {"kind": "", "progress": 0.0, "miss": 1.0, "total": false, "annular": false}
	var s := _syzygy_near(t_total)
	var dt: float = t_total - s["t"]
	if absf(dt) > ECLIPSE_HALF_DAYS:
		return none
	var lat := absf(float(s["lat"]))
	var limit := SOLAR_LIMIT_DEG if s["kind"] == "solar" else LUNAR_LIMIT_DEG
	if lat >= limit:
		return none
	var tot := _is_total(s["kind"], lat, s["t"])
	var ann: bool = s["kind"] == "solar" and lat < 0.06 and not tot
	return {"kind": s["kind"], "progress": dt / ECLIPSE_HALF_DAYS, "miss": lat / limit, "total": tot, "annular": ann}


func _is_total(kind: String, lat: float, t_total: float) -> bool:
	if kind == "lunar":
		return lat < LUNAR_TOTAL_DEG
	return lat < 0.06 and moon_distance_factor_at(t_total) * MOON_SUN_RATIO >= 1.0


func eclipse() -> Dictionary:
	return eclipse_at(total())


## The next eclipse after now: {kind, t (total days), total, date}; searches `years` ahead.
func next_eclipse(kind := "", years := 60) -> Dictionary:
	var md := float(month_days)
	var t := total()
	var month_start := floorf((t + start_day_of_year) / md) * md - start_day_of_year
	for i in years * 12 + 1:
		var ms := month_start + i * md
		for c in [[ms + 0.5, "solar"], [ms + 0.5 + md * 0.5, "lunar"]]:
			if c[0] <= t + ECLIPSE_HALF_DAYS or (kind != "" and kind != c[1]):
				continue
			var lat := absf(rad_to_deg(moon_ecliptic_at(c[0]).y))
			if lat < (SOLAR_LIMIT_DEG if c[1] == "solar" else LUNAR_LIMIT_DEG):
				return {"kind": c[1], "t": c[0], "total": _is_total(c[1], lat, c[0])}
	return {}


# ---------------- seasons for the world ----------------

## 0..1 how far autumn colour has come on deciduous leaves: turns through
## September, full in October and November, browns off through December.
func autumn_colour() -> float:
	var m := year_fraction() * 12.0
	if m < 8.0 or m >= 12.0:
		return 0.0
	return smoothstep(8.0, 9.5, m)


## 0..1 bare-winter look of deciduous leaves and grass (December to March).
func winter_dullness() -> float:
	var m := year_fraction() * 12.0
	if m >= 11.0:
		return smoothstep(11.0, 11.6, m)
	return 1.0 - smoothstep(2.0, 3.0, m)


## 0..1 snow cover on temperate ground: falls in early December, melts in early March.
func snow_cover() -> float:
	var m := year_fraction() * 12.0
	if m >= 11.0:
		return smoothstep(11.05, 11.5, m)
	return 1.0 - smoothstep(2.0, 2.45, m)


## Crop growth multiplier (ADR 0022 random ticks): spring and summer 1, autumn
## half, winter none.
func crop_growth() -> float:
	return [0.0, 1.0, 1.0, 0.5][season()]


# ---------------- save ----------------

func to_dict() -> Dictionary:
	return {"version": 1, "day": day, "time": time}


func load_dict(d: Dictionary) -> void:
	if int(d.get("version", 0)) != 1:
		return
	day = maxi(0, int(d.get("day", 0)))
	time = fposmod(float(d.get("time", time)), 1.0)
