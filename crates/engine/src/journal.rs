//! The journal (docs/design/journal.md, draft ADR journal-data): what each ape has noticed and knows. The
//! entries are `data/journal.toml`'s rows, checked at load; each ape holds a state per entry (ADR 0047), the
//! server raises it from triggers and tells the ape's own client.
use crate::blocks::BLOCKS;
use crate::items::Items;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An entry's state for one ape, in the order it rises.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum State {
    #[default]
    Unknown,
    /// Its question shows.
    Noticed,
    /// Its text shows.
    Known,
}

impl State {
    pub fn name(self) -> &'static str {
        match self {
            State::Unknown => "unknown",
            State::Noticed => "noticed",
            State::Known => "known",
        }
    }

    pub fn named(s: &str) -> Option<State> {
        [State::Unknown, State::Noticed, State::Known].into_iter().find(|t| t.name() == s)
    }
}

/// The ages an entry may belong to (docs/design/progression.md).
pub const AGES: [&str; 7] = ["steam", "lv", "mv", "hv", "ev", "iv", "exotic"];

/// What raises an entry, resolved at load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    /// `look:block:<name>`: the crosshair on that block.
    Look(u16),
    /// `have:<name>`: that block or item in the slots.
    Have(u16),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub age: String,
    pub title: String,
    pub question: String,
    pub text: String,
    pub noticed_by: Vec<Trigger>,
    pub known_by: Vec<Trigger>,
    /// Entries noticed when this one becomes known, by index.
    pub points_at: Vec<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    age: String,
    title: String,
    question: String,
    text: String,
    #[serde(default)]
    noticed_by: Vec<String>,
    #[serde(default)]
    known_by: Vec<String>,
    #[serde(default)]
    points_at: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    entry: Vec<Row>,
}

pub struct Journal {
    pub entries: Vec<Entry>,
    /// The rows each trigger raises, and to what: the table from what happened to rows (journal.md).
    look: HashMap<u16, Vec<(usize, State)>>,
    have: HashMap<u16, Vec<(usize, State)>>,
}

impl Journal {
    /// Reads and checks the entries: names unique, an age of `AGES`, words in every text, every trigger of a
    /// kind built so far naming a block or item there is, at least one trigger that makes it known, and every
    /// `points_at` another entry.
    pub fn parse(text: &str, items: &Items) -> Result<Journal, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let index = |n: &str| f.entry.iter().position(|r| r.name == n);
        let trigger = |t: &str| -> Result<Trigger, String> {
            let (kind, what) = t.split_once(':').ok_or(format!("trigger '{t}' has no kind (look:, have:)"))?;
            match kind {
                "look" => {
                    let name = what.strip_prefix("block:").ok_or(format!("trigger '{t}': look: names a block, as look:block:clay"))?;
                    BLOCKS.placeable().find(|d| d.name == name).map(|d| Trigger::Look(d.id)).ok_or(format!("trigger '{t}': data/blocks.toml has no block '{name}'"))
                }
                // `named` reads '_' as a space for typed commands (/give hand_axe); the file writes names as
                // data/items.toml does, so a search for a row by its name finds its triggers
                "have" if what.contains('_') => Err(format!("trigger '{t}': write the name as data/items.toml does, with spaces, not '_'")),
                "have" => items.named(what).map(Trigger::Have).ok_or(format!("trigger '{t}': data/items.toml has no block or item '{what}'")),
                "do" | "see" | "need" => Err(format!("trigger '{t}': {kind}: comes with the process runner (journal.md, step 4)")),
                _ => Err(format!("trigger '{t}': no kind '{kind}' (look:, have:)")),
            }
        };
        let mut entries = Vec::new();
        for (k, r) in f.entry.iter().enumerate() {
            let at = |e: String| format!("entry '{}': {e}", r.name);
            if r.name.is_empty() || index(&r.name) != Some(k) {
                return Err(at("a name, used once".into()));
            }
            if !AGES.contains(&r.age.as_str()) {
                return Err(at(format!("age '{}' is none of {}", r.age, AGES.join(", "))));
            }
            if [&r.title, &r.question, &r.text].iter().any(|s| s.trim().is_empty()) {
                return Err(at("a title, a question and a text".into()));
            }
            if r.known_by.is_empty() {
                return Err(at("nothing makes it known (known_by)".into()));
            }
            let noticed_by = r.noticed_by.iter().map(|t| trigger(t)).collect::<Result<_, _>>().map_err(at)?;
            let known_by = r.known_by.iter().map(|t| trigger(t)).collect::<Result<_, _>>().map_err(at)?;
            let points_at = r.points_at.iter().map(|p| index(p).filter(|&i| i != k).ok_or(at(format!("points_at '{p}' is no other entry")))).collect::<Result<_, _>>()?;
            entries.push(Entry { name: r.name.clone(), age: r.age.clone(), title: r.title.clone(), question: r.question.clone(), text: r.text.clone(), noticed_by, known_by, points_at });
        }
        let (mut look, mut have) = (HashMap::<u16, Vec<(usize, State)>>::new(), HashMap::<u16, Vec<(usize, State)>>::new());
        for (k, e) in entries.iter().enumerate() {
            for (t, to) in e.noticed_by.iter().map(|t| (t, State::Noticed)).chain(e.known_by.iter().map(|t| (t, State::Known))) {
                match *t {
                    Trigger::Look(b) => look.entry(b).or_default().push((k, to)),
                    Trigger::Have(i) => have.entry(i).or_default().push((k, to)),
                }
            }
        }
        Ok(Journal { entries, look, have })
    }

    pub fn index(&self, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.name == name)
    }

    /// A new ape's journal: every entry unknown.
    pub fn fresh(&self) -> Vec<State> {
        vec![State::Unknown; self.entries.len()]
    }

    /// Raises entry `k` to `to` if it is lower; one become known notices the unknown entries it points at.
    pub fn raise(&self, s: &mut [State], k: usize, to: State) {
        if s[k] >= to {
            return;
        }
        s[k] = to;
        if to == State::Known {
            for &p in &self.entries[k].points_at {
                self.raise(s, p, State::Noticed);
            }
        }
    }

    /// The crosshair is on a block of this id.
    pub fn looked(&self, s: &mut [State], block: u16) {
        for &(k, to) in self.look.get(&block).into_iter().flatten() {
            self.raise(s, k, to);
        }
    }

    /// A block or item of this id is in the slots.
    pub fn had(&self, s: &mut [State], item: u16) {
        for &(k, to) in self.have.get(&item).into_iter().flatten() {
            self.raise(s, k, to);
        }
    }

    /// What a save keeps: each entry not unknown by name, in the file's order, then `kept` (the save's
    /// entries this file does not have, `load`) as they came.
    pub fn saved(&self, s: &[State], kept: &[(String, State)]) -> Vec<(String, State)> {
        self.entries.iter().zip(s).filter(|(_, t)| **t != State::Unknown).map(|(e, t)| (e.name.clone(), *t)).chain(kept.iter().cloned()).collect()
    }

    /// A save's entries as states, and those this file does not have, kept for the next save untouched: an
    /// entry renamed back, or a typo fixed, finds its state again.
    pub fn load(&self, v: &[(String, State)]) -> (Vec<State>, Vec<(String, State)>) {
        let (mut s, mut kept) = (self.fresh(), Vec::new());
        for (name, t) in v {
            match self.index(name) {
                Some(k) => s[k] = *t,
                None => kept.push((name.clone(), *t)),
            }
        }
        (s, kept)
    }
}

/// The entries. shortcut: not reloaded live (ADR 0134 has blocks, items, creatures and herds): a reload would
/// have to keep every ape's states by name and tell the clients; when the starter's entries are written.
pub static JOURNAL: std::sync::LazyLock<Journal> = std::sync::LazyLock::new(|| Journal::parse(include_str!("../../../data/journal.toml"), &crate::items::ITEMS).unwrap_or_else(|e| panic!("data/journal.toml: {e}")));

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ITEMS;

    const ROW: &str = "[[entry]]\nname = \"a\"\nage = \"steam\"\ntitle = \"A\"\nquestion = \"q\"\ntext = \"t\"\n";

    fn parse(extra: &str) -> Result<Journal, String> {
        Journal::parse(&format!("{ROW}known_by = [\"have:flint\"]\n{extra}"), &ITEMS)
    }

    #[test]
    fn the_file_loads_and_names_every_trigger_it_can() {
        let j = &*JOURNAL;
        assert!(j.entries.len() >= 5, "a handful of entries");
        for e in &j.entries {
            assert!(!e.known_by.is_empty(), "{}: something makes it known", e.name);
        }
    }

    #[test]
    fn a_bad_row_is_refused_with_its_name() {
        let bad = |extra: &str, says: &str| {
            let e = parse(extra).err().unwrap_or_else(|| panic!("'{extra}' loaded"));
            assert!(e.contains(says), "'{extra}': {e}");
        };
        bad("noticed_by = [\"look:block:no such block\"]", "no block 'no such block'");
        bad("noticed_by = [\"look:clay\"]", "look: names a block");
        bad("noticed_by = [\"have:no such item\"]", "no block or item 'no such item'");
        bad("noticed_by = [\"have:hand_axe\"]", "with spaces, not '_'");
        bad("noticed_by = [\"see:process:dry:clay\"]", "step 4");
        bad("noticed_by = [\"smell:clay\"]", "no kind 'smell'");
        bad("noticed_by = [\"clay\"]", "has no kind");
        bad("points_at = [\"b\"]", "points_at 'b' is no other entry");
        bad("points_at = [\"a\"]", "points_at 'a' is no other entry");
        bad("colour = 3", "unknown field");
        let twice = format!("{ROW}known_by = [\"have:flint\"]\n{ROW}known_by = [\"have:flint\"]\n");
        assert!(Journal::parse(&twice, &ITEMS).err().is_some_and(|e| e.contains("used once")));
        let unknowable = format!("{ROW}noticed_by = [\"have:flint\"]\n");
        assert!(Journal::parse(&unknowable, &ITEMS).err().is_some_and(|e| e.contains("nothing makes it known")));
        let old = ROW.replace("steam", "stone");
        assert!(Journal::parse(&format!("{old}known_by = [\"have:flint\"]\n"), &ITEMS).err().is_some_and(|e| e.contains("age 'stone'")));
        let wordless = ROW.replace("\"q\"", "\" \"");
        assert!(Journal::parse(&format!("{wordless}known_by = [\"have:flint\"]\n"), &ITEMS).err().is_some_and(|e| e.contains("a question")));
        assert!(parse("noticed_by = [\"look:block:clay\", \"have:hand axe\"]").is_ok());
    }

    /// Unknown, then noticed by a look, then known by having it, which notices what it points at; nothing
    /// lowers a state, and a known entry is not noticed again.
    #[test]
    fn an_entry_rises_through_its_three_states() {
        let j = Journal::parse(
            &format!("{ROW}noticed_by = [\"look:block:clay\"]\nknown_by = [\"have:clay\"]\npoints_at = [\"b\"]\n{}known_by = [\"have:flint\"]\n", ROW.replace("\"a\"", "\"b\"")),
            &ITEMS,
        )
        .unwrap();
        let (clay, flint) = (BLOCKS.id("clay"), ITEMS.named("flint").unwrap());
        let mut s = j.fresh();
        assert_eq!(s, [State::Unknown, State::Unknown]);
        j.had(&mut s, flint);
        assert_eq!(s, [State::Unknown, State::Known], "b known on its own, a not");
        let mut s = j.fresh();
        j.looked(&mut s, BLOCKS.id("stone"));
        assert_eq!(s, [State::Unknown; 2], "another block");
        j.looked(&mut s, clay);
        assert_eq!(s, [State::Noticed, State::Unknown]);
        j.had(&mut s, clay);
        assert_eq!(s, [State::Known, State::Noticed], "known; it points at b");
        j.looked(&mut s, clay);
        assert_eq!(s, [State::Known, State::Noticed], "a look does not lower it");
        j.had(&mut s, flint);
        assert_eq!(s, [State::Known, State::Known]);
        assert_eq!(j.saved(&s, &[]), [("a".to_string(), State::Known), ("b".to_string(), State::Known)]);
    }

    /// An entry the file does not have (renamed, or a typo) is kept through a load and a save untouched, and a
    /// file that has it again finds its state.
    #[test]
    fn an_entry_the_file_lacks_survives_a_load_and_a_save() {
        let b = ROW.replace("\"a\"", "\"b\"");
        let both = Journal::parse(&format!("{ROW}known_by = [\"have:flint\"]\n{b}known_by = [\"have:flint\"]\n"), &ITEMS).unwrap();
        let only_b = Journal::parse(&format!("{b}known_by = [\"have:flint\"]\n"), &ITEMS).unwrap();
        let first = vec![("a".to_string(), State::Known), ("b".to_string(), State::Noticed)];
        let (s, kept) = only_b.load(&first);
        assert_eq!((&s[..], &kept[..]), (&[State::Noticed][..], &[("a".to_string(), State::Known)][..]), "a kept aside, not a state");
        let again = only_b.saved(&s, &kept);
        let (s, kept) = both.load(&again);
        assert_eq!((s, kept.len()), (vec![State::Known, State::Noticed], 0), "back in the file: a is known again");
    }

    /// The state's bytes on the wire and on disk (draft ADR journal-data): a change here breaks old saves.
    #[test]
    fn a_state_keeps_its_bytes() {
        assert_eq!(postcard::to_allocvec(&[State::Unknown, State::Noticed, State::Known]).unwrap(), [0, 1, 2]);
        let msg = crate::net::ToClient::Journal(vec![("flint".into(), State::Known)]);
        assert_eq!(postcard::to_allocvec(&msg).unwrap(), [23, 1, 5, b'f', b'l', b'i', b'n', b't', 2]);
    }
}
