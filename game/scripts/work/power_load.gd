class_name PowerLoad
extends RefCounted

## An electrical load on the LV network (ADR 0123): rated `watts` at Settings.lv_volts. Every machine, lamp and
## pump owns one and delegates its `net("energy")` / `net_in("energy", d)` to it. Physics decides what it gets:
## the solver reports the voltage at its node; it starts only when that voltage holds (lv_start_frac of rated)
## and one load at a time per circuit (Networks admits one start a second), stops under lv_stop_frac, and waits
## lv_retry_s before trying again, so a weak dynamo settles on the loads it can carry instead of flapping.

var watts: float
var on := false          # drawing and running
var volts := 0.0         # at its node, last solve
var drawn := 0.0         # watts taken, last solve
var retry := 0           # solves to wait after an undervoltage stop
var wants := false       # it has work this second


func _init(p_watts: float) -> void:
	watts = p_watts


## The node for the solver: a real load while running, a reading-only sliver while waiting (so it sees the
## voltage it would join), and with no work a plain conductor: its terminals still pass the line through to
## whatever is cabled beyond it.
func net(p_wants: bool) -> Dictionary:
	wants = p_wants
	if not wants:
		on = false
		return {"kind": "conductor", "ohms": Settings.circuit_device_ohms}
	var v := Settings.lv_volts
	return {"kind": "load", "watts": watts if on else 0.01, "volts": v}


## The solver's reading. True when it started this solve (Networks then admits no other start on that circuit).
func net_in(d: Dictionary) -> bool:
	volts = float(d.get("volts", 0.0))
	drawn = float(d.get("watts", 0.0))
	if on:
		if volts < Settings.lv_stop_frac * Settings.lv_volts:
			on = false
			retry = Settings.lv_retry_s
		return false
	if retry > 0:
		retry -= 1
		return false
	if wants and bool(d.get("may_start", true)) and volts >= Settings.lv_start_frac * Settings.lv_volts:
		on = true
		return true
	return false


func status() -> String:
	if on:
		return "%.0f W at %.1f V" % [drawn, volts]
	if retry > 0:
		return "stopped: voltage sagged to %.1f V, retrying" % volts
	if wants:
		return "waiting for %.0f V (has %.1f V)" % [Settings.lv_start_frac * Settings.lv_volts, volts]
	return "idle (%.0f W when working)" % watts
