class_name CommandRegistry
extends RefCounted

## Slash commands (ADR 0018). Pure logic, no nodes: the chat box hands over a
## line ("/give oak_log 64"), gets back {ok, text}; Tab completion comes from
## per-argument candidate lists. Handlers take the argument tokens and return
## CommandRegistry.ok(text) or CommandRegistry.fail(text). /help is built in.

var _commands := {}   # name -> {name, usage, help, handler: Callable, args: Array}
var _aliases := {}    # alias -> name


func _init() -> void:
	register("help", "/help [command]", "List commands, or show how to use one", _help, [names], ["?"])


## args: one entry per argument position, each an Array of candidates or a
## Callable returning one (evaluated on Tab, so item lists stay current).
func register(name: String, usage: String, help: String, handler: Callable, args: Array = [], aliases: Array = []) -> void:
	_commands[name] = {"name": name, "usage": usage, "help": help, "handler": handler, "args": args}
	for a in aliases:
		_aliases[a] = name


static func ok(text: String = "") -> Dictionary:
	return {"ok": true, "text": text}


static func fail(text: String) -> Dictionary:
	return {"ok": false, "text": text}


## The command for a name or alias (case-insensitive), or {}.
func find(name: String) -> Dictionary:
	var n := name.to_lower()
	return _commands.get(_aliases.get(n, n), {})


func names() -> Array:
	var out := _commands.keys()
	out.sort()
	return out


## Splits on whitespace; "double quotes" keep spaces inside one token.
static func tokenize(line: String) -> PackedStringArray:
	var out := PackedStringArray()
	var cur := ""
	var quoted := false
	var started := false
	for ch in line:
		if ch == "\"":
			quoted = not quoted
			started = true
		elif (ch == " " or ch == "\t") and not quoted:
			if started:
				out.append(cur)
			cur = ""
			started = false
		else:
			cur += ch
			started = true
	if started:
		out.append(cur)
	return out


## Runs one line ("/name args..." or "name args..."). Never throws: unknown
## commands and bad arguments come back as fail().
func execute(line: String) -> Dictionary:
	var tokens := tokenize(line.strip_edges().trim_prefix("/"))
	if tokens.is_empty():
		return fail("Type /help for a list of commands")
	var cmd := find(tokens[0])
	if cmd.is_empty():
		return fail("Unknown command: /%s. Type /help for a list of commands" % tokens[0])
	var r: Variant = (cmd["handler"] as Callable).call(tokens.slice(1))
	if typeof(r) == TYPE_DICTIONARY:
		return r
	return ok(str(r) if r != null else "")


## Full-line completions of `line` for the chat's Tab key, sorted. Completes
## the command name, then each argument from its candidate list.
func complete(line: String) -> Array[String]:
	var out: Array[String] = []
	if not line.begins_with("/"):
		return out
	var body := line.substr(1)
	var tokens := tokenize(body)
	var trailing := body == "" or body.ends_with(" ")
	if tokens.is_empty() or (tokens.size() == 1 and not trailing):
		var prefix := tokens[0].to_lower() if tokens.size() > 0 else ""
		for n in names():
			if String(n).begins_with(prefix):
				out.append("/" + n)
		return out
	var cmd := find(tokens[0])
	if cmd.is_empty():
		return out
	var arg_token := tokens.size() if trailing else tokens.size() - 1   # token index being completed
	var specs: Array = cmd["args"]
	if arg_token - 1 >= specs.size():
		return out
	var spec: Variant = specs[arg_token - 1]
	var options: Array = (spec as Callable).call() if spec is Callable else spec
	var prefix := "" if trailing else tokens[tokens.size() - 1].to_lower()
	var head := "/" + " ".join(tokens.slice(0, arg_token)) + " "
	for o in options:
		if String(o).to_lower().begins_with(prefix):
			out.append(head + String(o))
	out.sort()
	return out


func _help(args: PackedStringArray) -> Dictionary:
	if args.size() > 0:
		var cmd := find(args[0])
		if cmd.is_empty():
			return fail("Unknown command: /%s" % args[0])
		return ok("%s\n%s" % [cmd["usage"], cmd["help"]])
	var lines: Array[String] = []
	for n in names():
		lines.append("%s  %s" % [_commands[n]["usage"], _commands[n]["help"]])
	return ok("\n".join(lines))
