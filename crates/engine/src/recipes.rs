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

/// A furnace's work (ADR 0141): a `melt` or a `cast` row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkRow {
    name: String,
    #[serde(rename = "in")]
    input: (String, u16),
    /// A melt's second input, in the other input slot (draft ADR tin-and-bronze).
    with: Option<(String, u16)>,
    mould: Option<String>,
    /// A cast's shape: the window offers each shape once, the metal in the input slot picks the row.
    shape: Option<String>,
    out: (String, u16),
    heat: f32,
    seconds: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FuelRow {
    item: String,
    seconds: f32,
    heat: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FurnaceRow {
    block: String,
    top: f32,
    rise: f32,
    fall: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    recipe: Vec<Row>,
    #[serde(default)]
    melt: Vec<WorkRow>,
    #[serde(default)]
    cast: Vec<WorkRow>,
    #[serde(default)]
    fuel: Vec<FuelRow>,
    #[serde(default)]
    furnace: Vec<FurnaceRow>,
}

/// What a furnace works (ADR 0141): `in` of one item in an input slot becomes `out` once it has
/// been at least `heat` (C) for `secs`. A melt `with` a second input needs both, one in each input slot, in
/// either order; a work of one input runs only while the other input slot is empty, so what runs is never in
/// doubt (draft ADR tin-and-bronze). A cast needs `mould` in the mould slot (kept) and its `shape` chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct Work {
    pub name: String,
    pub input: (u16, u16),
    pub with: Option<(u16, u16)>,
    pub mould: Option<u16>,
    pub shape: Option<String>,
    pub out: (u16, u16),
    pub heat: f32,
    pub secs: f32,
}

/// A fuel: one item burns `secs`, its fire heading for `heat` (C).
#[derive(Clone, Debug, PartialEq)]
pub struct Fuel {
    pub item: u16,
    pub secs: f32,
    pub heat: f32,
}

/// A furnace block: its fire's heat is held to `top`; heat rises `rise` and falls `fall` C a second.
#[derive(Clone, Debug, PartialEq)]
pub struct FurnaceDef {
    pub block: u16,
    pub top: f32,
    pub rise: f32,
    pub fall: f32,
}

/// The hottest a fire, a work or a furnace may name (C).
pub const HEAT_MAX: f32 = 4000.0;
/// The longest a work or a fuel may take (s).
pub const SECS_MAX: f32 = 600.0;

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
    /// The furnaces' melts, then their casts (ADR 0141).
    pub works: Vec<Work>,
    pub fuels: Vec<Fuel>,
    pub furnaces: Vec<FurnaceDef>,
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
        let (works, fuels, furnaces) = works(&f, items, &rows)?;
        Ok(Recipes { rows, works, fuels, furnaces })
    }

    /// The furnace work that makes `item`, if one does (the viewer's).
    pub fn work_making(&self, item: u16) -> Option<&Work> {
        self.works.iter().find(|w| w.out.0 == item)
    }

    /// The furnace that block `m` is, if it is one.
    pub fn furnace(&self, m: u16) -> Option<&FurnaceDef> {
        self.furnaces.iter().find(|f| f.block == m)
    }

    pub fn fuel(&self, item: u16) -> Option<&Fuel> {
        self.fuels.iter().find(|f| f.item == item)
    }

    /// Whether `item` is a mould: some cast names it.
    pub fn mould(&self, item: u16) -> bool {
        self.works.iter().any(|w| w.mould == Some(item))
    }

    /// The casts a furnace of `top` C can work, in the file's order.
    pub fn casts(&self, top: f32) -> impl Iterator<Item = &Work> {
        self.works.iter().filter(move |w| w.mould.is_some() && w.heat <= top)
    }

    /// The first cast of each shape a furnace of `top` C can work: what its window offers to choose from.
    pub fn shapes(&self, top: f32) -> Vec<&Work> {
        let mut v: Vec<&Work> = Vec::new();
        for w in self.casts(top) {
            if !v.iter().any(|o| o.shape == w.shape) {
                v.push(w);
            }
        }
        v
    }

    /// The shape of the cast named `name` (a chosen cast: any row of that shape stands for all of them).
    pub fn shape(&self, name: &str) -> Option<&str> {
        self.works.iter().find(|w| w.name == name)?.shape.as_deref()
    }
}

/// Reads and checks the furnaces' rows (ADR 0141), refused with the row's name: an unknown item or
/// block, an amount of 0, a heat or seconds out of range, a melt with a mould or a cast without, a work that
/// makes its own input or mould, a name any other recipe or work has, two melts of one input (or one pair of
/// inputs, either order), a second input on a cast or the same as the first, a cast without a shape or a melt
/// with one, two casts of one shape and input, a fuel twice, a furnace whose block opens no crucible's window.
fn works(f: &File, items: &Items, grid: &[Recipe]) -> Result<(Vec<Work>, Vec<Fuel>, Vec<FurnaceDef>), String> {
    let item = |who: &str, n: &str| items.named(n).ok_or(format!("'{who}': no item '{n}'"));
    let heat = |who: &str, h: f32| if h > 0.0 && h <= HEAT_MAX { Ok(h) } else { Err(format!("'{who}': a heat over 0 to {HEAT_MAX} C")) };
    let secs = |who: &str, s: f32| if s > 0.0 && s <= SECS_MAX { Ok(s) } else { Err(format!("'{who}': seconds over 0 to {SECS_MAX}")) };
    let mut works: Vec<Work> = Vec::new();
    for (r, cast) in f.melt.iter().map(|r| (r, false)).chain(f.cast.iter().map(|r| (r, true))) {
        let who = format!("{} '{}'", if cast { "cast" } else { "melt" }, r.name);
        let bad = |why: &str| Err(format!("{who}: {why}"));
        let (input, out) = ((item(&who, &r.input.0)?, r.input.1), (item(&who, &r.out.0)?, r.out.1));
        if input.1 == 0 || out.1 == 0 {
            return bad("an amount of 0");
        }
        let mould = r.mould.as_ref().map(|m| item(&who, m)).transpose()?;
        if mould.is_some() != cast {
            return bad("a cast names its mould, a melt none");
        }
        let with = match &r.with {
            Some((n, k)) => Some((item(&who, n)?, *k)),
            None => None,
        };
        if with.is_some_and(|w| w.1 == 0) {
            return bad("an amount of 0");
        }
        if cast && with.is_some() || with.is_some_and(|w| w.0 == input.0) {
            return bad("a second input only on a melt, and another item than the first");
        }
        if cast != r.shape.is_some() {
            return bad("a cast names its shape, a melt none");
        }
        if out.0 == input.0 || Some(out.0) == mould || with.is_some_and(|w| w.0 == out.0) {
            return bad("it makes its own input or mould");
        }
        let named = |n: &str| n.eq_ignore_ascii_case(&r.name);
        if grid.iter().any(|g| named(&g.name)) || f.melt.iter().chain(&f.cast).filter(|o| named(&o.name)).count() > 1 {
            return bad("the name used twice");
        }
        // a melt's inputs as a pair, either order: no two melts of one
        let pair = |a: u16, b: Option<u16>| b.map_or((a, None), |b| (a.min(b), Some(a.max(b))));
        if !cast && works.iter().any(|w| w.mould.is_none() && pair(w.input.0, w.with.map(|x| x.0)) == pair(input.0, with.map(|x| x.0))) {
            return bad("two melts of one input");
        }
        if cast && works.iter().any(|w| w.shape == r.shape && w.input.0 == input.0) {
            return bad("two casts of one shape and input");
        }
        works.push(Work { name: r.name.clone(), input, with, mould, shape: r.shape.clone(), out, heat: heat(&who, r.heat)?, secs: secs(&who, r.seconds)? });
    }
    let mut fuels: Vec<Fuel> = Vec::new();
    for r in &f.fuel {
        let who = format!("fuel '{}'", r.item);
        let it = item(&who, &r.item)?;
        if fuels.iter().any(|o| o.item == it) {
            return Err(format!("{who}: the item twice"));
        }
        fuels.push(Fuel { item: it, secs: secs(&who, r.seconds)?, heat: heat(&who, r.heat)? });
    }
    let mut furnaces: Vec<FurnaceDef> = Vec::new();
    for r in &f.furnace {
        let who = format!("furnace '{}'", r.block);
        let d = crate::blocks::BLOCKS.placeable().find(|d| d.name == r.block).ok_or(format!("{who}: no block '{}'", r.block))?;
        if d.window.as_deref() != Some("crucible") {
            return Err(format!("{who}: its block opens no crucible's window"));
        }
        if furnaces.iter().any(|o| o.block == d.id) {
            return Err(format!("{who}: the block twice"));
        }
        if !(r.rise > 0.0 && r.fall > 0.0) {
            return Err(format!("{who}: rise and fall over 0"));
        }
        furnaces.push(FurnaceDef { block: d.id, top: heat(&format!("{who} top"), r.top)?, rise: r.rise, fall: r.fall });
    }
    Ok((works, fuels, furnaces))
}

impl Recipes {

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
        let tools = ["pick", "axe", "shovel", "knife", "hammer"].into_iter().zip(["pick head", "axe head", "shovel head", "knife blade", "hammer head"]);
        for (tool, head) in ["copper", "bronze"].into_iter().flat_map(|m| tools.clone().map(move |(t, h)| (format!("{m} {t}"), format!("{m} {h}")))) {
            let (tool, head) = (tool.as_str(), head.as_str());
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

    /// Draft ADR crucible: the crucible is a ring of 8 clay (a bench recipe), the mould two clay by hand; the
    /// furnace, its fuels, melts and casts read.
    #[test]
    fn the_crucible_needs_the_bench_and_its_rows_read() {
        let c = RECIPES.named("crucible").unwrap();
        assert!(c.bench() && c.out == (id("crucible"), 1) && c.cells.iter().filter(|k| k.is_some()).count() == 8 && c.cells[4].is_none());
        let m = RECIPES.named("clay mould").unwrap();
        assert!(!m.bench() && m.out == (id("clay mould"), 1) && (m.w, m.h) == (2, 1));
        let f = RECIPES.furnace(id("crucible")).unwrap();
        assert_eq!((f.top, f.rise > 0.0, f.fall > 0.0), (1100.0, true, true));
        assert!(RECIPES.mould(id("clay mould")) && !RECIPES.mould(id("clay")));
        assert!(RECIPES.fuel(id("pine log")).is_some() && RECIPES.fuel(id("copper ingot")).is_none());
        assert_eq!(RECIPES.work_making(id("copper ingot")).map(|w| w.heat), Some(1085.0));
        assert_eq!(RECIPES.work_making(id("copper hammer head")).map(|w| (w.input.1, w.mould)), Some((3, Some(id("clay mould")))));
    }

    #[test]
    fn bad_furnace_rows_are_refused_with_their_name() {
        let melt = |name: &str, inp: &str, out: &str, more: &str| format!("[[melt]]\nname='{name}'\nin={inp}\nout={out}\nheat=1085\nseconds=8\n{more}");
        let cast = |name: &str, more: &str| format!("[[cast]]\nname='{name}'\nin=['copper ingot', 2]\nout=['copper pick head', 1]\nheat=1085\nseconds=5\n{more}");
        let ok = format!("{}{}", melt("m", "['copper gravel', 2]", "['copper ingot', 1]", ""), cast("c", "mould='clay mould'\nshape='pick'\n"));
        let r = Recipes::parse(&ok, &ITEMS).unwrap();
        assert_eq!(r.works.len(), 2);
        // draft ADR tin-and-bronze: a melt of two inputs, and one of either of them alone, read
        let alloy = |name: &str, inp: &str, with: &str| melt(name, inp, "['bronze ingot', 4]", &format!("with={with}\n"));
        let two = alloy("a", "['copper ingot', 3]", "['tin ingot', 1]");
        assert_eq!(Recipes::parse(&format!("{two}{}", melt("t", "['tin ingot', 1]", "['flint', 1]", "")), &ITEMS).map(|r| r.works[0].with), Ok(Some((id("tin ingot"), 1))));
        let furnace = |block: &str, top: &str| format!("[[furnace]]\nblock='{block}'\ntop={top}\nrise=100\nfall=25\n");
        let fuel = |item: &str, s: &str, h: &str| format!("[[fuel]]\nitem='{item}'\nseconds={s}\nheat={h}\n");
        assert!(Recipes::parse(&format!("{}{}", furnace("crucible", "1100"), fuel("stick", "8", "700")), &ITEMS).is_ok());
        for (text, who, says) in [
            (melt("m", "['spoon', 2]", "['copper ingot', 1]", ""), "'m'", "no item 'spoon'"),
            (melt("m", "['copper gravel', 0]", "['copper ingot', 1]", ""), "'m'", "amount"),
            (melt("m", "['copper gravel', 2]", "['copper ingot', 0]", ""), "'m'", "amount"),
            (melt("m", "['copper gravel', 2]", "['copper gravel', 1]", ""), "'m'", "its own"),
            (melt("m", "['copper gravel', 2]", "['copper ingot', 1]", "mould='clay mould'\n"), "'m'", "mould"),
            (cast("c", "shape='pick'\n"), "'c'", "mould"),
            (cast("c", "mould='copper pick head'\nshape='pick'\n"), "'c'", "its own"),
            // a second input: unknown, of 0, on a cast, the first again, its own output; a cast with no shape
            // or two of one shape and input; two melts of one pair, either order
            (alloy("a", "['copper ingot', 3]", "['spoon', 1]"), "'a'", "no item 'spoon'"),
            (alloy("a", "['copper ingot', 3]", "['tin ingot', 0]"), "'a'", "amount"),
            (alloy("a", "['copper ingot', 3]", "['copper ingot', 1]"), "'a'", "second input"),
            (alloy("a", "['copper ingot', 3]", "['bronze ingot', 1]"), "'a'", "its own"),
            (cast("c", "mould='clay mould'\nshape='pick'\nwith=['tin ingot', 1]\n"), "'c'", "second input"),
            (cast("c", "mould='clay mould'\n"), "'c'", "shape"),
            (melt("m", "['copper gravel', 2]", "['copper ingot', 1]", "shape='pick'\n"), "'m'", "shape"),
            (format!("{ok}{}", cast("d", "mould='clay mould'\nshape='pick'\n").replace("copper pick head", "copper axe head")), "'d'", "one shape"),
            (format!("{two}{}", alloy("b", "['tin ingot', 1]", "['copper ingot', 3]")), "'b'", "one input"),
            (format!("{two}{}", alloy("b", "['copper ingot', 2]", "['tin ingot', 2]")), "'b'", "one input"),
            (melt("m", "['copper gravel', 2]", "['copper ingot', 1]", "").replace("heat=1085", "heat=0"), "'m'", "heat"),
            (melt("m", "['copper gravel', 2]", "['copper ingot', 1]", "").replace("heat=1085", "heat=9000"), "'m'", "heat"),
            (melt("m", "['copper gravel', 2]", "['copper ingot', 1]", "").replace("seconds=8", "seconds=0"), "'m'", "seconds"),
            (format!("{ok}{}", cast("c", "mould='clay mould'\n")), "'c'", "twice"),
            (melt("workbench", "['copper gravel', 2]", "['copper ingot', 1]", "").replace("[[melt]]", "[[recipe]]\nname='workbench'\ngrid=['L']\nkey={L='log'}\nout=['stick', 1]\n[[melt]]"), "'workbench'", "twice"),
            (format!("{ok}{}", melt("n", "['copper gravel', 4]", "['copper ingot', 2]", "")), "'n'", "one input"),
            (fuel("spoon", "8", "700"), "'spoon'", "no item"),
            (format!("{}{}", fuel("stick", "8", "700"), fuel("stick", "9", "700")), "'stick'", "twice"),
            (fuel("stick", "0", "700"), "'stick'", "seconds"),
            (fuel("stick", "8", "-1"), "'stick'", "heat"),
            (furnace("spoon", "1100"), "'spoon'", "no block"),
            (furnace("workbench", "1100"), "'workbench'", "crucible"),
            (furnace("crucible", "0"), "'crucible'", "top"),
            (format!("{}{}", furnace("crucible", "1100"), furnace("crucible", "1400")), "'crucible'", "twice"),
            (furnace("crucible", "1100").replace("rise=100", "rise=0"), "'crucible'", "rise"),
        ] {
            let e = Recipes::parse(&text, &ITEMS).err().unwrap_or_default();
            assert!(e.contains(says) && e.contains(who), "{text}: {e}");
        }
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
