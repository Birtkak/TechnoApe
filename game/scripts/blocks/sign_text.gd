class_name SignText
extends RefCounted

## Player text is a trust boundary (ADR 0164): everything a sign, a note, an owner name or a post's
## title carries goes through here, on the client that typed it and again on the server before it
## is stored or relayed. Control characters, bidi and zero-width marks go; lines and columns are capped.

const SIGN_LINES := 4
const SIGN_COLS := 15
const NOTE_LINES := 12
const NOTE_COLS := 28
const NAME_MAX := 24


static func _bad(c: int) -> bool:
	return (c < 32 and c != 10) or (c >= 127 and c <= 159) or (c >= 0x200B and c <= 0x200F) \
		or (c >= 0x2028 and c <= 0x202E) or (c >= 0x2060 and c <= 0x206F) or c == 0xFEFF or (c >= 0xD800 and c <= 0xDFFF)


## `s` as at most `max_lines` lines of `max_cols` characters: no control characters, no trailing blanks.
static func clean(s: String, max_lines: int, max_cols: int) -> String:
	var t := s.replace("\r\n", "\n").replace("\r", "\n").replace("\t", " ")
	var lines: Array = []
	for line in t.split("\n"):
		var o := ""
		for i in line.length():
			if not _bad(line.unicode_at(i)):
				o += line[i]
		lines.append(o.left(max_cols).strip_edges(false, true))
		if lines.size() >= max_lines:
			break
	while not lines.is_empty() and String(lines.back()) == "":
		lines.pop_back()
	return "\n".join(lines)


static func sign(s: String) -> String:
	return clean(s, SIGN_LINES, SIGN_COLS)


static func note(s: String) -> String:
	return clean(s, NOTE_LINES, NOTE_COLS)


## A name on one line.
static func name_of(s: String) -> String:
	return clean(s, 1, NAME_MAX).strip_edges()
