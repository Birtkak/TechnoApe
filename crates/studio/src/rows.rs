//! One row of a shared data file (`data/sounds.toml`, `data/effects.toml`) as text, without a window: where each
//! row's text lies in the file, its keys as written, and a save that rewrites only that row's text, every other
//! byte of the file (comments, spacing, line endings) as it was.

/// Where one row lies in a file: `[[header]]` to its last line of its own (the blank lines and comments after it
/// belong to what follows).
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub header: String,
    pub name: String,
    pub at: std::ops::Range<usize>,
}

/// Every `[[...]]` row of `text` with a `name = "..."` line, in order.
pub fn spans(text: &str) -> Vec<Span> {
    let mut lines = vec![];
    let mut at = 0;
    for l in text.split_inclusive('\n') {
        lines.push((at, l));
        at += l.len();
    }
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        let head = lines[i].1.trim();
        if !(head.starts_with("[[") && head.ends_with("]]")) {
            i += 1;
            continue;
        }
        let start = lines[i].0;
        let mut j = i + 1;
        while j < lines.len() && !lines[j].1.starts_with('[') {
            j += 1;
        }
        // back over the blank lines and comments that lead into the next row
        let mut end = j;
        while end > i + 1 && (lines[end - 1].1.trim().is_empty() || lines[end - 1].1.trim_start().starts_with('#')) {
            end -= 1;
        }
        let stop = lines.get(end).map_or(text.len(), |l| l.0);
        let name = lines[i + 1..end].iter().find_map(|(_, l)| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "name").then(|| v.trim().trim_matches('"').to_string())
        });
        if let Some(name) = name {
            out.push(Span { header: head.trim_matches(['[', ']']).to_string(), name, at: start..stop });
        }
        i = j;
    }
    out
}

/// The line ending a file uses: CRLF when it has one, else LF.
pub fn eol(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// `text` with the row `name` under `[[header]]` replaced by `row` (written with LF; put in the file's own line
/// endings), or `row` added at the end after a blank line when there is none. Every other byte stays.
pub fn put(text: &str, header: &str, name: &str, row: &str) -> String {
    let e = eol(text);
    let mut row = row.replace("\r\n", "\n").trim_end().replace('\n', e);
    row.push_str(e);
    match spans(text).into_iter().find(|s| s.header == header && s.name == name) {
        Some(s) => format!("{}{row}{}", &text[..s.at.start], &text[s.at.end..]),
        None => {
            let body = text.trim_end_matches(['\r', '\n']);
            format!("{body}{e}{e}{row}")
        }
    }
}

/// Writes `put`'s result to `path` when `check` (the game's own reader) takes it, through a temporary file
/// and a rename. Returns the new text.
pub fn save(path: &str, header: &str, name: &str, row: &str, check: impl Fn(&str) -> Result<(), String>) -> Result<String, String> {
    let old = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let new = put(&old, header, name, row);
    check(&new).map_err(|e| format!("{path}: {e}"))?;
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, &new).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{path}: {e}")
    })?;
    Ok(new)
}

/// `text` with the row `name`'s sub-table `[header.sub]` (a block's season, `[block.autumn]`) set to `body` (its
/// keys, LF), or dropped when `body` is None. A new one goes after the row's last sub-table. Every other byte
/// stays; a row that is not there leaves the text as it was.
pub fn put_table(text: &str, header: &str, name: &str, sub: &str, body: Option<&str>) -> String {
    let Some(row) = spans(text).into_iter().find(|s| s.header == header && s.name == name) else { return text.to_string() };
    let e = eol(text);
    let mut lines = vec![];
    let mut at = 0;
    for l in text.split_inclusive('\n') {
        lines.push((at, l));
        at += l.len();
    }
    // the row's sub-tables follow its span: each `[header.x]` to the next line opening a table
    let open = format!("[{header}.");
    let mut end = row.at.end;
    let mut found: Option<std::ops::Range<usize>> = None;
    let mut i = lines.iter().position(|(a, _)| *a >= row.at.end).unwrap_or(lines.len());
    while i < lines.len() {
        let t = lines[i].1.trim();
        if t.is_empty() || t.starts_with('#') {
            i += 1;
            continue;
        }
        if !t.starts_with(&open) {
            break;
        }
        let start = lines[i].0;
        let mut j = i + 1;
        while j < lines.len() && !lines[j].1.starts_with('[') {
            j += 1;
        }
        // back over the blank lines and comments that lead into what follows
        let mut k = j;
        while k > i + 1 && (lines[k - 1].1.trim().is_empty() || lines[k - 1].1.trim_start().starts_with('#')) {
            k -= 1;
        }
        let stop = lines.get(k).map_or(text.len(), |l| l.0);
        if t == format!("[{header}.{sub}]") {
            found = Some(start..stop);
        }
        end = stop;
        i = j;
    }
    let table = body.map(|b| format!("[{header}.{sub}]{e}{}", b.trim_end().replace("\r\n", "\n").replace('\n', e) + e));
    match (found, table) {
        (Some(r), t) => format!("{}{}{}", &text[..r.start], t.unwrap_or_default(), &text[r.end..]),
        (None, Some(t)) => {
            // after the last line of the row or its tables, which may end without a line ending
            let sep = if text[..end].ends_with('\n') { "" } else { e };
            format!("{}{sep}{t}{}", &text[..end], &text[end..])
        }
        (None, None) => text.to_string(),
    }
}

/// A row's keys as written, in order: (key, its value's text). A key of "" is a line kept as it is (a comment).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Keys {
    pub header: String,
    pub keys: Vec<(String, String)>,
}

/// How far a line opens brackets, outside strings and comments.
fn depth(s: &str) -> i32 {
    let (mut d, mut q, mut esc) = (0, None, false);
    for c in s.chars() {
        match q {
            Some(o) => {
                if esc {
                    esc = false;
                } else if c == '\\' && o == '"' {
                    esc = true;
                } else if c == o {
                    q = None;
                }
            }
            None => match c {
                '"' | '\'' => q = Some(c),
                '[' | '{' => d += 1,
                ']' | '}' => d -= 1,
                '#' => break,
                _ => {}
            },
        }
    }
    d
}

impl Keys {
    pub fn parse(row: &str) -> Keys {
        let mut k = Keys::default();
        let mut open = 0;
        for l in row.lines() {
            let t = l.trim_end_matches('\r');
            if open > 0 {
                let v = &mut k.keys.last_mut().unwrap().1;
                v.push('\n');
                v.push_str(t);
                open += depth(t);
                continue;
            }
            let s = t.trim();
            if s.starts_with("[[") && k.header.is_empty() && k.keys.is_empty() {
                k.header = s.trim_matches(['[', ']']).to_string();
            } else if let Some((key, v)) = s.split_once('=').filter(|_| !s.starts_with('#')) {
                k.keys.push((key.trim().to_string(), v.trim().to_string()));
                open = depth(v);
            } else {
                k.keys.push((String::new(), t.to_string()));
            }
        }
        // a comment or blank line at the end belongs to what follows
        while k.keys.last().is_some_and(|(key, _)| key.is_empty()) {
            k.keys.pop();
        }
        k
    }
    pub fn get(&self, key: &str) -> Option<&str> {
        self.keys.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
    /// Sets a key's text (a new key goes last); None removes it.
    pub fn set(&mut self, key: &str, value: Option<String>) {
        let at = self.keys.iter().position(|(k, _)| k == key);
        match (at, value) {
            (Some(i), Some(v)) => self.keys[i].1 = v,
            (None, Some(v)) => self.keys.push((key.into(), v)),
            (Some(i), None) => {
                self.keys.remove(i);
            }
            (None, None) => {}
        }
    }
    /// `set`, but a new key goes after the last of `after` the row has (so a number sits with the numbers).
    pub fn set_after(&mut self, key: &str, value: Option<String>, after: &[&str]) {
        let at = self.get(key).is_none().then(|| self.keys.iter().rposition(|(k, _)| after.contains(&k.as_str()))).flatten();
        self.set(key, value.clone());
        if let (Some(i), Some(_)) = (at, value) {
            let k = self.keys.pop().unwrap();
            self.keys.insert(i + 1, k);
        }
    }
    pub fn text(&self) -> String {
        let mut s = format!("[[{}]]\n", self.header);
        for (k, v) in &self.keys {
            if k.is_empty() {
                s.push_str(v);
            } else {
                s.push_str(&format!("{k} = {v}"));
            }
            s.push('\n');
        }
        s
    }
    /// The row read as TOML: its one table.
    pub fn table(&self) -> Result<toml::Table, String> {
        let t: toml::Table = toml::from_str(&self.text()).map_err(|e| e.to_string())?;
        t.get(&self.header).and_then(|a| a.as_array()).and_then(|a| a.first()).and_then(|r| r.as_table()).cloned().ok_or("no row".into())
    }
}

/// A number as a row writes it: no more digits than it needs (3 below 10, 1 below 100, none above).
pub fn num(v: f32) -> String {
    let d = if v.abs() >= 100.0 { 0 } else if v.abs() >= 10.0 { 1 } else { 3 };
    let s = format!("{v:.d$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# head\r\n# more\r\n\r\n[[sound]]\r\nname = \"a\"\r\nlayer = [\r\n  { x = 1 },\r\n]\r\n\r\n# --- b's own comment ---\r\n[[sound]]\r\nname = \"b\"\r\ngain = 0.5 # loud\r\n\r\n[[group]]\r\nname = \"g\"\r\nrows = [\"a\"]\r\n";

    #[test]
    fn the_rows_are_found_with_their_text_only() {
        let s = spans(FILE);
        assert_eq!(s.iter().map(|s| (s.header.as_str(), s.name.as_str())).collect::<Vec<_>>(), vec![("sound", "a"), ("sound", "b"), ("group", "g")]);
        assert_eq!(&FILE[s[0].at.clone()], "[[sound]]\r\nname = \"a\"\r\nlayer = [\r\n  { x = 1 },\r\n]\r\n");
        assert_eq!(&FILE[s[1].at.clone()], "[[sound]]\r\nname = \"b\"\r\ngain = 0.5 # loud\r\n");
        assert!(FILE[s[2].at.clone()].ends_with("rows = [\"a\"]\r\n"));
    }

    #[test]
    fn a_rewrite_keeps_every_other_byte_and_the_line_endings() {
        let new = put(FILE, "sound", "b", "[[sound]]\nname = \"b\"\ngain = 0.25\n");
        assert_eq!(new, FILE.replace("gain = 0.5 # loud", "gain = 0.25"));
        // the same row written back as it was: the file unchanged
        let same = put(FILE, "sound", "a", &FILE[spans(FILE)[0].at.clone()]);
        assert_eq!(same, FILE);
        // LF files stay LF
        let lf = FILE.replace("\r\n", "\n");
        assert_eq!(put(&lf, "sound", "a", "[[sound]]\r\nname = \"a\"\r\nlayer = []\r\n"), lf.replace("layer = [\n  { x = 1 },\n]", "layer = []"));
        // a new row goes at the end after one blank line
        let added = put(FILE, "sound", "c", "[[sound]]\nname = \"c\"\n");
        assert_eq!(added, format!("{FILE}\r\n[[sound]]\r\nname = \"c\"\r\n"));
        // the same name under another header is another row
        assert!(put(FILE, "effect", "a", "[[effect]]\nname = \"a\"\n").starts_with(FILE));
    }

    #[test]
    fn keys_read_and_write_back() {
        let row = &FILE[spans(FILE)[0].at.clone()];
        let k = Keys::parse(row);
        assert_eq!(k.header, "sound");
        assert_eq!(k.keys, vec![("name".into(), "\"a\"".into()), ("layer".into(), "[\n  { x = 1 },\n]".into())]);
        assert_eq!(k.text(), row.replace("\r\n", "\n"));
        assert_eq!(k.table().unwrap()["name"].as_str(), Some("a"));
        let mut k = k;
        k.set("gain", Some("2".into()));
        k.set("layer", None);
        assert_eq!(k.text(), "[[sound]]\nname = \"a\"\ngain = 2\n");
        assert_eq!(depth("x = [\"]\", { a = '[' }"), 1);
    }

    #[test]
    fn a_rows_table_is_set_and_dropped_and_nothing_else_moves() {
        let text = "[[block]]\r\nname = \"grass\"\r\ncolour = [0.3, 0.5, 0.2]\r\n[block.autumn]\r\ncolour = [0.5, 0.5, 0.2]\r\n\r\n# the dirt\r\n[[block]]\r\nname = \"dirt\"\r\n";
        let with = put_table(text, "block", "grass", "winter", Some("colour = [0.4, 0.4, 0.2]\n"));
        assert_eq!(with, text.replace("[block.autumn]\r\ncolour = [0.5, 0.5, 0.2]\r\n", "[block.autumn]\r\ncolour = [0.5, 0.5, 0.2]\r\n[block.winter]\r\ncolour = [0.4, 0.4, 0.2]\r\n"));
        assert_eq!(put_table(&with, "block", "grass", "winter", None), text, "dropped again, byte for byte");
        let changed = put_table(text, "block", "grass", "autumn", Some("colour = [0.6, 0.4, 0.1]"));
        assert_eq!(changed, text.replace("[0.5, 0.5, 0.2]", "[0.6, 0.4, 0.1]"));
        let no_autumn = put_table(text, "block", "grass", "autumn", None);
        assert_eq!(no_autumn, text.replace("[block.autumn]\r\ncolour = [0.5, 0.5, 0.2]\r\n", ""));
        // the last row of a file, no line ending at its end
        let last = put_table("[[block]]\nname = \"dirt\"", "block", "dirt", "spring", Some("colour = [0.1, 0.2, 0.3]"));
        assert_eq!(last, "[[block]]\nname = \"dirt\"\n[block.spring]\ncolour = [0.1, 0.2, 0.3]\n");
        assert_eq!(put_table(text, "block", "sand", "spring", Some("colour = [0, 0, 0]")), text, "no such row");
        assert_eq!(put_table(text, "block", "dirt", "spring", None), text, "no such table");
    }

    #[test]
    fn numbers_are_written_short() {
        assert_eq!((num(1800.0), num(0.7), num(0.70000005), num(-0.0001), num(12.34), num(3.0)), ("1800".into(), "0.7".into(), "0.7".into(), "0".into(), "12.3".into(), "3".into()));
    }
}
