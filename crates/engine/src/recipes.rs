//! Recipes of the crafting grid (`data/recipes.toml`; ADR 0140, superseding ADR 0139's list): a
//! shaped pattern of items and what it makes. The grid is 9 of the player's own slots (`items::CRAFT_AT`): the
//! inventory's 2 x 2 is its top left, a workbench's 3 x 3 all of it. The wire names a recipe by its name.
use crate::items::{self, Items, Slot, Stack, CRAFT, CRAFT_AT, ITEMS};
use serde::Deserialize;
use std::collections::BTreeMap;

/// The grid's side at a workbench; the inventory's is 2.
pub const SIDE: usize = 3;

#[derive(Deserialize)]
#[serde(untagged)]
enum Key {
    One(String),
    Any(Vec<String>),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    grid: Vec<String>,
    key: BTreeMap<String, Key>,
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
    /// The pattern's width and height, 1..3.
    pub w: usize,
    pub h: usize,
    /// Row by row, `h` rows of `w`: the items a cell takes (any one of them), None for an empty cell.
    pub cells: Vec<Option<Vec<u16>>>,
    pub out: (u16, u16),
}

pub struct Recipes {
    pub rows: Vec<Recipe>,
}

/// The grid's 9 cells as items, row by row (`items::CRAFT_AT`).
pub fn grid(slots: &[Slot]) -> [Option<u16>; CRAFT] {
    std::array::from_fn(|k| slots.get(CRAFT_AT + k).copied().flatten().map(|s| s.item))
}

impl Recipe {
    /// Wider or taller than the inventory's 2 x 2: it needs a workbench.
    pub fn bench(&self) -> bool {
        self.w > 2 || self.h > 2
    }

    /// The cell at row r, column c, read mirrored left to right or not.
    fn at(&self, r: usize, c: usize, mirror: bool) -> &Option<Vec<u16>> {
        &self.cells[r * self.w + if mirror { self.w - 1 - c } else { c }]
    }

    /// Whether the grid's cells (`grid`) hold this pattern: anywhere in the grid, mirrored or not, every cell
    /// outside it empty, each of its cells one of the items it takes.
    pub fn fits(&self, g: &[Option<u16>; CRAFT]) -> bool {
        let full: Vec<usize> = (0..CRAFT).filter(|&k| g[k].is_some()).collect();
        let (Some(r0), Some(c0)) = (full.iter().map(|k| k / SIDE).min(), full.iter().map(|k| k % SIDE).min()) else { return false };
        let (r1, c1) = (full.iter().map(|k| k / SIDE).max().unwrap(), full.iter().map(|k| k % SIDE).max().unwrap());
        if (r1 - r0 + 1, c1 - c0 + 1) != (self.h, self.w) {
            return false;
        }
        [false, true].into_iter().any(|m| (0..self.h).all(|r| (0..self.w).all(|c| match (self.at(r, c, m), g[(r0 + r) * SIDE + c0 + c]) {
            (None, None) => true,
            (Some(any), Some(i)) => any.contains(&i),
            _ => false,
        })))
    }

    /// Whether some grid could hold both patterns: one size, and mirrored or not every cell empty in both or
    /// taking an item in common.
    fn overlaps(&self, o: &Recipe) -> bool {
        (self.w, self.h) == (o.w, o.h)
            && [false, true].into_iter().any(|m| {
                (0..self.h).all(|r| (0..self.w).all(|c| match (self.at(r, c, false), o.at(r, c, m)) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.iter().any(|i| b.contains(i)),
                    _ => false,
                }))
            })
    }
}

impl Recipes {
    /// Reads and checks the recipes against `items`, refused with the row's name: an unknown item, a key letter
    /// unused or undefined, ragged rows, larger than 3 x 3, an empty row or column at the pattern's edge, an
    /// amount of 0, a name used twice, its output among its inputs, two recipes that can match one grid.
    pub fn parse(text: &str, items: &Items) -> Result<Recipes, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut rows: Vec<Recipe> = Vec::new();
        for r in &f.recipe {
            let bad = |why: &str| Err(format!("recipe '{}': {why}", r.name));
            let item = |n: &str| items.named(n).ok_or(format!("recipe '{}': no item '{n}'", r.name));
            let (h, w) = (r.grid.len(), r.grid.first().map_or(0, |l| l.chars().count()));
            if h == 0 || w == 0 || h > SIDE || w > SIDE || r.grid.iter().any(|l| l.chars().count() != w) {
                return bad("its grid is 1 to 3 rows of 1 to 3 cells, every row as long");
            }
            let letters: Vec<char> = r.grid.iter().flat_map(|l| l.chars()).collect();
            let mut key: BTreeMap<char, Vec<u16>> = BTreeMap::new();
            for (k, v) in &r.key {
                let mut ch = k.chars();
                let (Some(c), None) = (ch.next(), ch.next()) else { return bad(&format!("key '{k}' is not one letter")) };
                let names: Vec<&str> = match v {
                    Key::One(n) => vec![n.as_str()],
                    Key::Any(l) => l.iter().map(String::as_str).collect(),
                };
                if names.is_empty() || c == ' ' || !letters.contains(&c) {
                    return bad(&format!("key '{k}' unused, a space, or naming no item"));
                }
                key.insert(c, names.into_iter().map(item).collect::<Result<_, _>>()?);
            }
            if let Some(c) = letters.iter().find(|c| **c != ' ' && !key.contains_key(c)) {
                return bad(&format!("letter '{c}' has no key"));
            }
            let cells: Vec<Option<Vec<u16>>> = letters.iter().map(|c| key.get(c).cloned()).collect();
            let edge = |ks: Vec<usize>| ks.into_iter().all(|k| cells[k].is_none());
            if edge((0..w).collect()) || edge(((h - 1) * w..h * w).collect()) || edge((0..h).map(|y| y * w).collect()) || edge((0..h).map(|y| y * w + w - 1).collect()) {
                return bad("an empty row or column at its pattern's edge");
            }
            let out = (item(&r.out.0)?, r.out.1);
            if out.1 == 0 {
                return bad("an amount of 0");
            }
            if f.recipe.iter().filter(|o| o.name.eq_ignore_ascii_case(&r.name)).count() > 1 {
                return bad("the name used twice");
            }
            if cells.iter().flatten().any(|any| any.contains(&out.0)) {
                return bad("it makes its own input");
            }
            let me = Recipe { name: r.name.clone(), w, h, cells, out };
            if let Some(o) = rows.iter().find(|o| o.overlaps(&me)) {
                return bad(&format!("one grid could match it and '{}'", o.name));
            }
            rows.push(me);
        }
        Ok(Recipes { rows })
    }

    /// The recipe of this name, any case; `_` stands for a space.
    pub fn named(&self, name: &str) -> Option<&Recipe> {
        let name = name.replace('_', " ");
        self.rows.iter().find(|r| r.name.eq_ignore_ascii_case(&name))
    }

    /// The recipe that makes `item`, if one does (the viewer's).
    pub fn making(&self, item: u16) -> Option<&Recipe> {
        self.rows.iter().find(|r| r.out.0 == item)
    }

    /// The recipe the grid's cells in `slots` hold: of the 3 x 3 at a workbench (`three`), else of the 2 x 2 only
    /// (anything outside it, or a recipe that needs the bench, matches nothing).
    pub fn matched(&self, slots: &[Slot], three: bool) -> Option<&Recipe> {
        let g = grid(slots);
        if !three && (0..CRAFT).any(|k| !items::in_two(k) && g[k].is_some()) {
            return None;
        }
        self.rows.iter().find(|r| (three || !r.bench()) && r.fits(&g))
    }
}

/// The recipes in force, read against the items in force on first use. Items are only added or changed in
/// place while a game runs (ADR 0134), so the ids it holds stay right.
pub static RECIPES: std::sync::LazyLock<Recipes> = std::sync::LazyLock::new(|| Recipes::parse(include_str!("../../../data/recipes.toml"), &ITEMS).unwrap_or_else(|e| panic!("data/recipes.toml: {e}")));

/// Makes `r` up to `n` times from the grid's cells in an ape's slots: each make takes one item from every full
/// cell and puts the output into the usable slots (`items::give`). It stops when the cells no longer hold `r`
/// or the output has no room (that make changes nothing). Returns how many were made and every item taken.
pub fn make(slots: &mut Vec<Slot>, r: &Recipe, n: u32) -> (u32, Vec<u16>) {
    let mut used = Vec::new();
    for made in 0..n {
        if !r.fits(&grid(slots)) {
            return (made, used);
        }
        let mut s = slots.clone();
        let mut took = Vec::new();
        for c in s[CRAFT_AT..CRAFT_AT + CRAFT].iter_mut() {
            if let Some(st) = *c {
                took.push(st.item);
                *c = (st.n > 1).then_some(Stack { n: st.n - 1, ..st });
            }
        }
        if items::give(&mut s, Stack::new(r.out.0, r.out.1)) > 0 {
            return (made, used);
        }
        *slots = s;
        used.extend(took);
    }
    (n, used)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: &str) -> u16 {
        ITEMS.named(n).unwrap_or_else(|| panic!("no item {n}"))
    }

    /// A grid of 9 cells from rows of item names ("" empty).
    fn g(cells: [&str; 9]) -> [Option<u16>; CRAFT] {
        cells.map(|n| (!n.is_empty()).then(|| id(n)))
    }

    #[test]
    fn the_recipes_read_the_copper_tools_are_a_head_above_a_stick_and_four_logs_a_workbench() {
        for (tool, head) in [("copper pick", "copper pick head"), ("copper axe", "copper axe head"), ("copper shovel", "copper shovel head"), ("copper knife", "copper knife blade"), ("copper hammer", "copper hammer head")] {
            let r = RECIPES.named(&tool.replace(' ', "_")).unwrap_or_else(|| panic!("no recipe {tool}"));
            assert_eq!((r.w, r.h, r.cells.clone(), r.out), (1, 2, vec![Some(vec![id(head)]), Some(vec![id("stick")])], (id(tool), 1)));
            assert!(!r.bench() && RECIPES.making(id(tool)) == Some(r));
        }
        let b = RECIPES.named("workbench").unwrap();
        assert_eq!((b.w, b.h, b.out, b.bench()), (2, 2, (id("workbench"), 1), false));
        assert!(b.cells.iter().all(|c| c.as_ref().is_some_and(|any| any.contains(&id("log")) && any.contains(&id("oak log")))));
        assert!(RECIPES.making(id("stick")).is_none());
    }

    #[test]
    fn a_pattern_matches_anywhere_mirrored_with_alternatives_and_nothing_else() {
        let pick = RECIPES.named("copper pick").unwrap();
        let (h, s) = ("copper pick head", "stick");
        // anywhere in the grid: top left, the far column, the bottom rows
        assert!(pick.fits(&g([h, "", "", s, "", "", "", "", ""])));
        assert!(pick.fits(&g(["", "", h, "", "", s, "", "", ""])));
        assert!(pick.fits(&g(["", "", "", "", h, "", "", s, ""])));
        // upside down, side by side, an extra item anywhere, or a cell apart: nothing
        assert!(!pick.fits(&g([s, "", "", h, "", "", "", "", ""])));
        assert!(!pick.fits(&g([h, s, "", "", "", "", "", "", ""])));
        assert!(!pick.fits(&g([h, "", "", s, "", "", "", "", "flint"])));
        assert!(!pick.fits(&g([h, "", "", "", "", "", s, "", ""])));
        assert!(!pick.fits(&g([""; 9])));
        // alternatives: any log in any cell of the bench's 2 x 2
        let bench = RECIPES.named("workbench").unwrap();
        assert!(bench.fits(&g(["log", "oak log", "", "pine log", "palm log", "", "", "", ""])));
        assert!(bench.fits(&g(["", "", "", "", "oak log", "oak log", "", "oak log", "oak log"])));
        assert!(!bench.fits(&g(["log", "oak log", "", "pine log", "stick", "", "", "", ""])));
        assert!(!bench.fits(&g(["log", "oak log", "", "pine log", "", "", "", "", ""])));
        // mirrored left to right: an L of a test recipe and its mirror both match, never upside down
        let t = Recipes::parse("[[recipe]]\nname='l'\ngrid=['A ', 'AB']\nkey={A='stick', B='flint'}\nout=['hand axe', 1]\n", &ITEMS).unwrap();
        let l = &t.rows[0];
        assert!(l.fits(&g(["stick", "", "", "stick", "flint", "", "", "", ""])));
        assert!(l.fits(&g(["", "", "stick", "", "flint", "stick", "", "", ""])));
        assert!(!l.fits(&g(["stick", "flint", "", "stick", "", "", "", "", ""])));
    }

    #[test]
    fn the_grid_matches_its_recipe_the_2x2_only_without_a_bench() {
        let mut s = items::ape();
        let at = |k: usize, n: &str| (CRAFT_AT + k, Some(Stack::new(id(n), 1)));
        for (k, v) in [at(1, "copper pick head"), at(4, "stick")] {
            s[k] = v;
        }
        assert_eq!(RECIPES.matched(&s, false).map(|r| r.name.as_str()), Some("copper pick"));
        assert_eq!(RECIPES.matched(&s, true).map(|r| r.name.as_str()), Some("copper pick"));
        // the far column is the bench's: away from one it matches nothing
        let mut far = items::ape();
        for (k, v) in [at(2, "copper pick head"), at(5, "stick")] {
            far[k] = v;
        }
        assert_eq!(RECIPES.matched(&far, false), None);
        assert_eq!(RECIPES.matched(&far, true).map(|r| r.name.as_str()), Some("copper pick"));
        // a recipe taller than 2 needs the bench even in the 2 x 2's own cells
        let t = Recipes::parse("[[recipe]]\nname='tall'\ngrid=['S', 'S', 'S']\nkey={S='stick'}\nout=['hand axe', 1]\n", &ITEMS).unwrap();
        let mut tall = items::ape();
        for k in [0, 3, 6] {
            tall[CRAFT_AT + k] = Some(Stack::new(id("stick"), 1));
        }
        assert!(t.rows[0].bench() && t.matched(&tall, false).is_none() && t.matched(&tall, true) == Some(&t.rows[0]));
    }

    #[test]
    fn bad_recipes_are_refused_with_their_name() {
        let row = |name: &str, grid: &str, key: &str, out: &str| format!("[[recipe]]\nname='{name}'\ngrid=[{grid}]\nkey={{{key}}}\nout={out}\n");
        let ok = row("r", "'H', 'S'", "H='flint', S='stick'", "['hand axe', 1]");
        assert!(Recipes::parse(&ok, &ITEMS).is_ok());
        let other = row("q", "'SS'", "S='stick'", "['flint', 1]");
        for (text, says) in [
            (row("r", "'H', 'S'", "H='spoon', S='stick'", "['hand axe', 1]"), "no item 'spoon'"),
            (row("r", "'H', 'S'", "H=['flint', 'spoon'], S='stick'", "['hand axe', 1]"), "no item 'spoon'"),
            (row("r", "'H', 'S'", "H='flint', S='stick'", "['spoon', 1]"), "no item 'spoon'"),
            (row("r", "'H', 'S'", "H='flint', S='stick', X='flint'", "['hand axe', 1]"), "unused"),
            (row("r", "'H', 'S'", "H='flint'", "['hand axe', 1]"), "no key"),
            (row("r", "'HH', 'S'", "H='flint', S='stick'", "['hand axe', 1]"), "every row as long"),
            (row("r", "'HHHH'", "H='flint'", "['hand axe', 1]"), "1 to 3"),
            (row("r", "'H', 'S', 'S', 'S'", "H='flint', S='stick'", "['hand axe', 1]"), "1 to 3"),
            (row("r", "", "", "['hand axe', 1]"), "1 to 3"),
            (row("r", "'H ', 'S '", "H='flint', S='stick'", "['hand axe', 1]"), "edge"),
            (row("r", "' ', 'H'", "H='flint'", "['hand axe', 1]"), "edge"),
            (row("r", "'H', 'S'", "H='flint', S='stick'", "['hand axe', 0]"), "amount"),
            (format!("{ok}{ok}"), "twice"),
            (row("r", "'H', 'S'", "H='flint', S='stick'", "['flint', 2]"), "its own input"),
            // two that one grid could match: the same pattern for another output, a mirror, a shared alternative
            (format!("{other}{}", row("r", "'SS'", "S='stick'", "['hand axe', 1]")), "and 'q'"),
            (format!("{}{}", row("q", "'A ', 'AB'", "A='stick', B='flint'", "['hand axe', 1]"), row("r", "' A', 'BA'", "A='stick', B='flint'", "['log', 1]")), "and 'q'"),
            (format!("{other}{}", row("r", "'SL'", "S=['log', 'stick'], L=['flint', 'stick']", "['hand axe', 1]")), "and 'q'"),
        ] {
            let e = Recipes::parse(&text, &ITEMS).err().unwrap_or_default();
            assert!(e.contains(says) && e.contains("recipe 'r'"), "{text}: {e}");
        }
        // an upside-down pattern is another recipe
        assert!(Recipes::parse(&format!("{ok}{}", row("q", "'S', 'H'", "H='flint', S='stick'", "['log', 1]")), &ITEMS).is_ok());
    }

    #[test]
    fn a_make_takes_one_from_each_cell_and_gives_its_output_or_changes_nothing() {
        let r = RECIPES.named("copper pick").unwrap();
        let (head, stick, pick) = (id("copper pick head"), id("stick"), id("copper pick"));
        let mut s = items::ape();
        s[CRAFT_AT + 1] = Some(Stack::new(head, 2));
        s[CRAFT_AT + 4] = Some(Stack::new(stick, 5));
        // three asked, two made: then the head's cell is empty and the grid holds no pick
        let (made, used) = make(&mut s, r, 3);
        assert_eq!((made, used.len()), (2, 4));
        assert_eq!((items::count(&s, pick), s[CRAFT_AT + 1], s[CRAFT_AT + 4]), (2, None, Some(Stack::new(stick, 3))));
        assert!(items::ape_fits(&s));
        let before = s.clone();
        assert_eq!(make(&mut s, r, 1).0, 0);
        assert_eq!(s, before, "no head: nothing changes");
        // no room for the output: nothing changes
        let mut full = items::ape();
        while items::give(&mut full, Stack::new(id("flint"), 16)) == 0 {}
        full[CRAFT_AT] = Some(Stack::new(head, 1));
        full[CRAFT_AT + 3] = Some(Stack::new(stick, 1));
        let before = full.clone();
        assert_eq!(make(&mut full, r, 1), (0, vec![]));
        assert_eq!(full, before, "no room: nothing changes");
    }
}
