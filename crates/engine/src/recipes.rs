//! Recipes (`data/recipes.toml`; draft ADR recipe-list-and-copper-tools): what goes in, what comes out, and which
//! process makes it. Only `hand` is run now, the inventory's recipe list (`ToServer::Craft`); the wire and the
//! bot name a recipe by its name, so rows may be added anywhere.
use crate::items::{self, Items, Slot, Stack, ITEMS};
use serde::Deserialize;

/// The processes a recipe may name. shortcut: only the hand; the crucible adds `melt` and `cast`.
pub const PROCESSES: [&str; 1] = ["hand"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    process: String,
    #[serde(rename = "in")]
    inp: Vec<(String, u16)>,
    out: (String, u16),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    recipe: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub name: String,
    pub process: String,
    /// Each (item, n) it takes, each item once.
    pub inp: Vec<(u16, u16)>,
    pub out: (u16, u16),
}

pub struct Recipes {
    pub rows: Vec<Recipe>,
}

impl Recipes {
    /// Reads and checks the recipes against `items`: every item known, a known process, amounts over 0, names
    /// and an item's place in `in` once, and never its own input as its output; refused with the row's name.
    pub fn parse(text: &str, items: &Items) -> Result<Recipes, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut rows = Vec::new();
        for r in &f.recipe {
            let bad = |why: &str| Err(format!("recipe '{}': {why}", r.name));
            let item = |n: &str| items.named(n).ok_or(format!("recipe '{}': no item '{n}'", r.name));
            let inp = r.inp.iter().map(|(n, k)| Ok((item(n)?, *k))).collect::<Result<Vec<_>, String>>()?;
            let out = (item(&r.out.0)?, r.out.1);
            if !PROCESSES.contains(&r.process.as_str()) {
                return bad(&format!("no process '{}' (one of {PROCESSES:?})", r.process));
            }
            if inp.is_empty() || out.1 == 0 || inp.iter().any(|i| i.1 == 0) {
                return bad("an amount of 0, or nothing in");
            }
            if f.recipe.iter().filter(|o| o.name.eq_ignore_ascii_case(&r.name)).count() > 1 || inp.iter().any(|i| inp.iter().filter(|j| j.0 == i.0).count() > 1) {
                return bad("the name used twice, or an item twice in `in`");
            }
            if inp.iter().any(|i| i.0 == out.0) {
                return bad("it makes its own input");
            }
            rows.push(Recipe { name: r.name.clone(), process: r.process.clone(), inp, out });
        }
        Ok(Recipes { rows })
    }

    /// The recipe of this name, any case; `_` stands for a space (the bot's `craft copper_pick`).
    pub fn named(&self, name: &str) -> Option<&Recipe> {
        let name = name.replace('_', " ");
        self.rows.iter().find(|r| r.name.eq_ignore_ascii_case(&name))
    }

    /// The recipes a process runs, in their rows' order.
    pub fn of<'a>(&'a self, process: &'a str) -> impl Iterator<Item = &'a Recipe> + 'a {
        self.rows.iter().filter(move |r| r.process == process)
    }
}

/// The recipes in force, read against the items in force on first use. Items are only added or changed in
/// place while a game runs (ADR 0134), so the ids it holds stay right.
pub static RECIPES: std::sync::LazyLock<Recipes> = std::sync::LazyLock::new(|| Recipes::parse(include_str!("../../../data/recipes.toml"), &ITEMS).unwrap_or_else(|e| panic!("data/recipes.toml: {e}")));

impl Recipe {
    /// How many times the slots hold its inputs (room for the output not counted).
    pub fn makeable(&self, slots: &[Slot]) -> u32 {
        self.inp.iter().map(|&(i, n)| items::count(slots, i) / n as u32).min().unwrap_or(0)
    }

    /// What the slots lack for one make: each input short and by how many.
    pub fn missing(&self, slots: &[Slot]) -> Vec<(u16, u32)> {
        self.inp.iter().map(|&(i, n)| (i, (n as u32).saturating_sub(items::count(slots, i)))).filter(|m| m.1 > 0).collect()
    }
}

/// Makes `r` up to `n` times in an ape's slots, one make at a time: its inputs out of any slot, its output into
/// the usable slots (`items::give`). It stops at the first make that lacks an input or has no room for all of its
/// output (that make changes nothing), so as many as fit are made. Returns how many were.
pub fn craft(slots: &mut Vec<Slot>, r: &Recipe, n: u32) -> u32 {
    let most = n.min(r.makeable(slots));
    for made in 0..most {
        let mut s = slots.clone();
        let took = r.inp.iter().all(|&(i, k)| items::take(&mut s, i, k as u32));
        // the inputs may come out of the worn slot (a pack): what is left must still keep the ape's rules
        if !took || items::give(&mut s, Stack::new(r.out.0, r.out.1)) > 0 || !items::ape_fits(&s) {
            return made;
        }
        *slots = s;
    }
    most
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: &str) -> u16 {
        ITEMS.named(n).unwrap_or_else(|| panic!("no item {n}"))
    }

    #[test]
    fn the_recipes_read_and_the_copper_tools_are_a_head_and_a_stick() {
        for (tool, head) in [("copper pick", "copper pick head"), ("copper axe", "copper axe head"), ("copper shovel", "copper shovel head"), ("copper knife", "copper knife blade"), ("copper hammer", "copper hammer head")] {
            let r = RECIPES.named(&tool.replace(' ', "_")).unwrap_or_else(|| panic!("no recipe {tool}"));
            assert_eq!((r.process.as_str(), r.inp.clone(), r.out), ("hand", vec![(id(head), 1), (id("stick"), 1)], (id(tool), 1)));
        }
        assert_eq!(RECIPES.of("hand").count(), RECIPES.rows.len());
    }

    #[test]
    fn bad_recipes_are_refused_with_their_name() {
        let row = |name: &str, process: &str, inp: &str, out: &str| format!("[[recipe]]\nname='{name}'\nprocess='{process}'\nin=[{inp}]\nout={out}\n");
        let ok = row("r", "hand", "['stick', 1]", "['flint', 1]");
        assert!(Recipes::parse(&ok, &ITEMS).is_ok());
        for (text, says) in [
            (row("r", "hand", "['spoon', 1]", "['flint', 1]"), "no item 'spoon'"),
            (row("r", "hand", "['stick', 1]", "['spoon', 1]"), "no item 'spoon'"),
            (row("r", "smelt", "['stick', 1]", "['flint', 1]"), "process"),
            (row("r", "hand", "['stick', 0]", "['flint', 1]"), "amount"),
            (row("r", "hand", "['stick', 1]", "['flint', 0]"), "amount"),
            (row("r", "hand", "", "['flint', 1]"), "amount"),
            (format!("{ok}{ok}"), "twice"),
            (row("r", "hand", "['stick', 1], ['stick', 2]", "['flint', 1]"), "twice"),
            (row("r", "hand", "['stick', 1], ['flint', 1]", "['flint', 2]"), "its own input"),
        ] {
            let e = Recipes::parse(&text, &ITEMS).err().unwrap_or_default();
            assert!(e.contains(says) && e.contains("recipe 'r'"), "{text}: {e}");
        }
    }

    #[test]
    fn a_craft_takes_its_inputs_and_gives_its_output_or_changes_nothing() {
        let r = RECIPES.named("copper pick").unwrap();
        let (head, stick, pick) = (id("copper pick head"), id("stick"), id("copper pick"));
        let mut s = items::ape();
        items::give(&mut s, Stack::new(head, 2));
        items::give(&mut s, Stack::new(stick, 5));
        assert_eq!(r.makeable(&s), 2);
        // three asked, two made: the third lacks a head and changes nothing
        assert_eq!(craft(&mut s, r, 3), 2);
        assert_eq!((items::count(&s, head), items::count(&s, stick), items::count(&s, pick)), (0, 3, 2));
        assert!(items::ape_fits(&s));
        assert_eq!(r.missing(&s), vec![(head, 1)]);
        let before = s.clone();
        assert_eq!(craft(&mut s, r, 1), 0);
        assert_eq!(s, before, "missing a head: nothing changes");
        // no room: every usable slot full of something else, and neither input's stack emptied by the make
        let mut full = items::ape();
        items::give(&mut full, Stack::new(head, 2));
        items::give(&mut full, Stack::new(stick, 2));
        while items::give(&mut full, Stack::new(id("flint"), 16)) == 0 {}
        let before = full.clone();
        assert_eq!(r.makeable(&full), 2);
        assert_eq!(craft(&mut full, r, 1), 0);
        assert_eq!(full, before, "no room: nothing changes");
    }
}
