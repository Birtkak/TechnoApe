//! Items and slots (ADR 0074, docs/design/items.md steps 1 to 3). Every block kind is an item with its block
//! id; the rest come from `data/items.toml`, with ids from `ITEM_BASE` up. An inventory is a list of slots and
//! everything that holds things is one: the ape (the hotbar's 9, the worn pack's slot, the rows it opens) and
//! a pile on the ground (4). One rule moves a stack between any two slots (`shift`), on the server and in the
//! client's prediction alike.
use crate::blocks::BLOCKS;
use crate::world::Block;
use serde::{Deserialize, Serialize};

/// The first id of an item that is not a block: far over any block id (the shader's 8 bits, ADR 0062's 16).
pub const ITEM_BASE: u16 = 0x8000;
/// A full slot of a block, and of an item whose row names no `stack`.
pub const STACK: u16 = 64;
/// The ape's slots: the hotbar's 9, then the worn pack's, then the rows it opens (at most a frame pack's 27).
pub const HOTBAR: usize = 9;
pub const WORN: usize = 9;
pub const ROWS_AT: usize = 10;
pub const APE_SLOTS: usize = ROWS_AT + 27;
/// The crafting grid's cells (draft ADR crafting-grid): the 9 slots after those, a 3 x 3 row by row (cell k at
/// `CRAFT_AT + k`, row k / 3, column k % 3). The inventory's 2 x 2 is its top left; the rest count at a workbench.
/// Real slots of the player, so moves, saves and the ledger see them; nothing goes into or out of them but by a
/// move or a make (`give`, `count` and `take` keep to the first `APE_SLOTS`).
pub const CRAFT_AT: usize = APE_SLOTS;
pub const CRAFT: usize = 9;
/// Every slot a player has.
pub const SLOTS: usize = CRAFT_AT + CRAFT;

/// How far from the feet a workbench (a block with a window) may be, to the middle of its cell, for its use and
/// while its 3 x 3 counts: the server's rule, which the client keeps too.
pub const BENCH_REACH: f64 = 6.0;

/// Whether grid cell k (0..9) is in the inventory's 2 x 2.
pub fn in_two(k: usize) -> bool {
    k % 3 < 2 && k / 3 < 2
}
/// A pile's slots (items.md: up to 4 stacks a cell).
pub const PILE_SLOTS: usize = 4;
/// The creative screen's tabs (ADR 0075).
pub const TABS: [&str; 6] = ["nature", "rock", "ore", "made", "lamps", "items"];

/// A stack: an item, how many, and 8 bytes its kind gives a meaning (none yet). shortcut: no kind uses the
/// payload yet (tools' wear, food's day, a vessel's fluid are items.md steps 4 and 6); stacks of equal payload
/// merge, which is the plain kind's rule.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Stack {
    pub item: u16,
    pub n: u16,
    pub payload: u64,
}

impl Stack {
    pub fn new(item: u16, n: u16) -> Stack {
        Stack { item, n, payload: 0 }
    }
}

pub type Slot = Option<Stack>;

/// What holds slots, as a move names it: the ape itself, the pile in a cell, creative's endless grid of every
/// item (its slot is the item's id; a stack put there is gone), or the ground ahead (thrown: a new pile).
/// shortcut: no containers or machines (block entities) yet; they become a holder here, items.md step 5.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
pub enum Holder {
    Ape,
    Pile(Block),
    Grid,
    Ground,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    #[serde(default = "full")]
    stack: u16,
    #[serde(default)]
    pack: u8,
    look: String,
    tool: Option<String>,
    head: Option<String>,
    knap: Option<(String, u8)>,
    leaves: Option<String>,
}

fn full() -> u16 {
    STACK
}

/// A tool head's material (items.md "Tools", ADR 0110): what every tool of it digs like.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Material {
    name: String,
    speed: f32,
    grade: u8,
    uses: u16,
    #[serde(default)]
    prospect: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    made: Vec<String>,
    #[serde(default)]
    material: Vec<Material>,
    #[serde(default)]
    item: Vec<Row>,
}

/// A tool (ADR 0110): its kind (`blocks::TOOLS`; None counts as every kind, the unhafted hand axe) and its head
/// material's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub kind: Option<String>,
    /// Dig time is `hardness / speed` on a block whose `tool` is its kind.
    pub speed: f32,
    /// It takes a block that is not loose when this is at least the block's grade.
    pub grade: u8,
    /// Digs before it is worn away (each costs one); a stack's payload counts those it has had.
    pub uses: u16,
    /// How far a prospecting strike of it hears ore, metres (ADR 0138); 0: it
    /// does not prospect.
    pub prospect: f32,
    /// What its last use leaves in its slot (a metal tool's stick); None: nothing.
    pub leaves: Option<u16>,
}

/// The longest a material's `prospect` may reach, metres.
pub const PROSPECT_MAX: f32 = 64.0;
/// Seconds between two prospecting strikes that are answered.
pub const PROSPECT_WAIT_S: f64 = 1.0;

impl Tool {
    /// A prospecting strike's range with it, metres: a pick's or an every-kind tool's whose head reaches; None
    /// for any other.
    pub fn prospects(&self) -> Option<f64> {
        (self.prospect > 0.0 && self.kind.as_deref().is_none_or(|k| k == "pick")).then_some(self.prospect as f64)
    }

    /// It is the kind of tool block `d` names.
    pub fn fits(&self, d: &crate::blocks::Def) -> bool {
        d.tool.is_some() && self.kind.as_ref().is_none_or(|k| d.tool.as_ref() == Some(k))
    }

    /// It takes block `d` whole when `d` is not loose: the right kind, of at least its grade.
    pub fn takes(&self, d: &crate::blocks::Def) -> bool {
        self.fits(d) && self.grade >= d.grade
    }
}

pub struct Def {
    pub id: u16,
    pub name: String,
    pub stack: u16,
    /// Slots it opens worn: 0 for anything that is not a pack.
    pub pack: u8,
    /// The block it is drawn as.
    pub look: u16,
    pub tool: Option<Tool>,
    /// Struck this many times on hard rock (`knaps`) it becomes that item, one of it at a time.
    pub knap: Option<(u16, u8)>,
}

/// Seconds between two strikes that count (ADR 0110): holding or hammering the button does not knap faster.
pub const KNAP_WAIT_S: f64 = 0.4;

/// Hard rock, what a knapper strikes on: a block a pick digs that bare hands do not take (not soil, wood,
/// gravel or rubble).
pub fn knaps(d: &crate::blocks::Def) -> bool {
    d.tool.as_deref() == Some("pick") && !d.loose
}

/// The tool a stack is, if it is one.
pub fn tool(s: Option<Stack>) -> Option<&'static Tool> {
    ITEMS.get(s?.item)?.tool.as_ref()
}

/// The digs a tool's stack has left: its uses less the payload (the digs it has had); None for any other stack.
pub fn left(s: Option<Stack>) -> Option<u16> {
    Some(tool(s)?.uses.saturating_sub(s?.payload.min(u16::MAX as u64) as u16))
}

/// One dig's wear on the tool in `slot`: true when that was its last and it is gone (ADR 0047), leaving in the
/// slot one of what its row `leaves` (a metal tool's stick), else nothing; the caller's ledger counts both.
pub fn wear(slot: &mut Slot) -> bool {
    let Some(s) = slot.as_mut().filter(|s| tool(Some(**s)).is_some()) else { return false };
    s.payload += 1;
    if left(Some(*s)) == Some(0) {
        *slot = tool(Some(*s)).and_then(|t| t.leaves).map(|l| Stack::new(l, 1));
        return true;
    }
    false
}

pub struct Items {
    pub defs: Vec<Def>,
    /// Each of `TABS` and the items it shows, blocks in id order, then the items in their rows' order.
    pub tabs: Vec<Vec<u16>>,
    /// Over the file's text and the blocks' hash: a client of another build refuses to play (`Welcome`).
    pub hash: u64,
}

impl Items {
    /// Reads and checks the items: names unique and no block's, a stack of 1..64, a pack of 9, 18 or 27 slots
    /// and a stack of one, a known `look` and `made` blocks.
    pub fn parse(text: &str) -> Result<Items, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let block = |n: &str| BLOCKS.placeable().find(|d| d.name == n).map(|d| d.id);
        let mut defs = Vec::new();
        for (k, r) in f.item.iter().enumerate() {
            if block(&r.name).is_some() || f.item.iter().filter(|o| o.name == r.name).count() > 1 {
                return Err(format!("item '{}': the name is a block's or another item's", r.name));
            }
            if !(1..=STACK).contains(&r.stack) {
                return Err(format!("item '{}': stack outside 1..64", r.name));
            }
            if r.pack != 0 && (![9, 18, 27].contains(&r.pack) || r.stack != 1) {
                return Err(format!("item '{}': a pack opens 9, 18 or 27 slots and stacks by one", r.name));
            }
            let look = block(&r.look).ok_or(format!("item '{}': no block '{}' to look like", r.name, r.look))?;
            let tool = match (&r.tool, &r.head) {
                (None, None) => None,
                (Some(kind), Some(head)) => {
                    let m = f.material.iter().find(|m| &m.name == head).ok_or(format!("item '{}': no material '{head}' for its head", r.name))?;
                    if !(crate::blocks::TOOLS.contains(&kind.as_str()) || kind == "any") || r.stack != 1 {
                        return Err(format!("item '{}': a tool is one of {:?} or \"any\", and stacks by one", r.name, crate::blocks::TOOLS));
                    }
                    Some(Tool { kind: (kind != "any").then(|| kind.clone()), speed: m.speed, grade: m.grade, uses: m.uses, prospect: m.prospect, leaves: None })
                }
                _ => return Err(format!("item '{}': a tool names both its kind and its head", r.name)),
            };
            defs.push(Def { id: ITEM_BASE + k as u16, name: r.name.clone(), stack: r.stack, pack: r.pack, look, tool, knap: None });
        }
        for m in &f.material {
            if !(m.speed > 1.0 && m.speed <= 20.0) || !(1..=4).contains(&m.grade) || !(1..=10000).contains(&m.uses) || f.material.iter().filter(|o| o.name == m.name).count() > 1 {
                return Err(format!("material '{}': speed over 1 to 20, grade 1..4, uses 1..10000, a name used once", m.name));
            }
            if !(0.0..=PROSPECT_MAX).contains(&m.prospect) {
                return Err(format!("material '{}': prospect 0 to {PROSPECT_MAX} m", m.name));
            }
        }
        for (k, r) in f.item.iter().enumerate() {
            if let Some((into, n)) = &r.knap {
                let to = defs.iter().find(|d| &d.name == into).map(|d| d.id).ok_or(format!("item '{}': knaps into no item '{into}'", r.name))?;
                if !(1..=20).contains(n) || to == defs[k].id {
                    return Err(format!("item '{}': knap takes 1..20 strikes into another item", r.name));
                }
                defs[k].knap = Some((to, *n));
            }
            if let Some(l) = &r.leaves {
                let to = block(l).or_else(|| defs.iter().find(|d| &d.name == l).map(|d| d.id)).ok_or(format!("item '{}': leaves no item '{l}'", r.name))?;
                let t = defs[k].tool.as_mut().ok_or(format!("item '{}': only a tool leaves something when worn", r.name))?;
                t.leaves = Some(to);
            }
        }
        let made: Vec<u16> = f.made.iter().map(|n| block(n).ok_or(format!("made: no block '{n}'"))).collect::<Result<_, _>>()?;
        // a mineral's sample lies on the ground as an item of this file (the minerals' rows read before these)
        for k in &crate::ore::MINERALS.kinds {
            if let Some(s) = k.sample.as_ref().filter(|s| !defs.iter().any(|d| &d.name == *s)) {
                return Err(format!("kind '{}' (data/minerals.toml): sample '{s}' is no item", k.name));
            }
        }
        let ores: Vec<u16> = crate::ore::MINERALS.kinds.iter().map(|k| k.block).collect();
        let rocks: Vec<u16> = crate::rock::ROCKS.provinces.iter().flat_map(|p| p.layers.iter().map(|l| l.0)).collect();
        let mut tabs = vec![Vec::new(); TABS.len()];
        for d in BLOCKS.placeable() {
            let t = if d.light != [0.0; 3] {
                4
            } else if ores.contains(&d.id) {
                2
            } else if rocks.contains(&d.id) {
                1
            } else if made.contains(&d.id) {
                3
            } else {
                0
            };
            tabs[t].push(d.id);
        }
        tabs[5] = defs.iter().map(|d| d.id).collect();
        Ok(Items { defs, tabs, hash: crate::world::fnv(BLOCKS.hash, text.as_bytes()) })
    }

    /// An item's row; None for a block or an unknown id.
    pub fn get(&self, id: u16) -> Option<&Def> {
        self.defs.get(id.checked_sub(ITEM_BASE)? as usize)
    }

    /// A placeable block or an item of this build.
    pub fn known(&self, id: u16) -> bool {
        self.get(id).is_some() || (id != crate::blocks::AIR && id < ITEM_BASE && BLOCKS.get(id).is_some())
    }

    pub fn name(&self, id: u16) -> &str {
        match self.get(id) {
            Some(d) => &d.name,
            None => BLOCKS.get(id).filter(|_| id != crate::blocks::AIR).map_or("?", |d| d.name.as_str()),
        }
    }

    /// The block or item of this name, any case; `_` stands for a space (`/give hand_axe`).
    pub fn named(&self, name: &str) -> Option<u16> {
        let name = &name.replace('_', " ");
        BLOCKS.placeable().find(|d| d.name.eq_ignore_ascii_case(name)).map(|d| d.id).or_else(|| self.defs.iter().find(|d| d.name.eq_ignore_ascii_case(name)).map(|d| d.id))
    }

    pub fn stack(&self, id: u16) -> u16 {
        self.get(id).map(|d| d.stack).or(BLOCKS.get(id).map(|d| d.stack)).unwrap_or(STACK)
    }

    /// The block an item is drawn as; a block is itself.
    pub fn look(&self, id: u16) -> u16 {
        self.get(id).map_or(id, |d| d.look)
    }

    /// Every block then every item: what creative's grid holds.
    pub fn all(&self) -> impl Iterator<Item = u16> + '_ {
        BLOCKS.placeable().map(|d| d.id).chain(self.defs.iter().map(|d| d.id))
    }

    /// The items' (id, name) table a save keeps (blocks have their own, `Save::blocks`).
    pub fn table(&self) -> Vec<(u16, String)> {
        self.defs.iter().map(|d| (d.id, d.name.clone())).collect()
    }
}

/// The item rows in force, made over the blocks in force (`live::apply`, ADR 0134).
pub static ITEMS: crate::live::Live<Items> = crate::live::Live::new(|| Items::parse(crate::live::ITEMS_TOML).unwrap_or_else(|e| panic!("data/items.toml: {e}")));

/// A fresh ape's slots, the grid's cells with them: all empty.
pub fn ape() -> Vec<Slot> {
    vec![None; SLOTS]
}

/// The slots past the worn one an ape may fill now: those its pack opens.
pub fn open(slots: &[Slot]) -> usize {
    ROWS_AT + slots.get(WORN).copied().flatten().and_then(|s| ITEMS.get(s.item)).map_or(0, |d| d.pack as usize)
}

/// The ape's slots things go into, in order: the hotbar, then the open rows.
pub fn usable(slots: &[Slot]) -> impl Iterator<Item = usize> + Clone + use<> {
    (0..HOTBAR).chain(ROWS_AT..open(slots).min(slots.len()))
}

/// Whether an ape's slots keep the rules: every stack of a known item, 1 to its stack size; the worn slot
/// a pack or nothing; nothing in a row its pack does not open (the grid's cells take anything).
pub fn ape_fits(slots: &[Slot]) -> bool {
    let worn = slots.get(WORN).copied().flatten();
    slots.len() == SLOTS && stacks_fit(slots) && worn.is_none_or(|s| ITEMS.get(s.item).is_some_and(|d| d.pack > 0)) && slots[open(slots)..APE_SLOTS].iter().all(Option::is_none)
}

/// Every stack of a known item, 1 to its stack size.
pub fn stacks_fit(slots: &[Slot]) -> bool {
    slots.iter().flatten().all(|s| ITEMS.known(s.item) && (1..=ITEMS.stack(s.item)).contains(&s.n))
}

/// Puts `s` into `slots` at the indices `at`: onto stacks of the same item first, then into empty slots.
/// Returns how many did not fit.
pub fn add(slots: &mut [Slot], at: impl Iterator<Item = usize> + Clone, s: Stack) -> u16 {
    let (mut left, max) = (s.n, ITEMS.stack(s.item));
    for k in at.clone() {
        if let Some(t) = slots[k].as_mut().filter(|t| t.item == s.item && t.payload == s.payload) {
            let put = left.min(max.saturating_sub(t.n));
            (t.n, left) = (t.n + put, left - put);
        }
    }
    for k in at {
        if left > 0 && slots[k].is_none() {
            let put = left.min(max);
            slots[k] = Some(Stack { n: put, ..s });
            left -= put;
        }
    }
    left
}

/// Puts `s` into an ape's usable slots (`add`); how many did not fit.
pub fn give(slots: &mut [Slot], s: Stack) -> u16 {
    let at = usable(slots);
    add(slots, at, s)
}

/// How many of `item` the slots hold (an ape's grid cells not counted).
pub fn count(slots: &[Slot], item: u16) -> u32 {
    slots.iter().take(APE_SLOTS).flatten().filter(|s| s.item == item).map(|s| s.n as u32).sum()
}

/// Takes `n` of `item` from the slots, the first slots first; all of it or nothing.
pub fn take(slots: &mut [Slot], item: u16, n: u32) -> bool {
    if count(slots, item) < n {
        return false;
    }
    let mut left = n;
    for s in slots.iter_mut().take(APE_SLOTS) {
        if let Some(t) = s.as_mut().filter(|t| t.item == item && left > 0) {
            let k = left.min(t.n as u32) as u16;
            (t.n, left) = (t.n - k, left - k as u32);
            if t.n == 0 {
                *s = None;
            }
        }
    }
    true
}

/// A whole number of full stacks (and one less) of `item`, `n` of it: what a dig or a /give hands out.
pub fn stacks(item: u16, n: u32) -> impl Iterator<Item = Stack> {
    let max = ITEMS.stack(item) as u32;
    (0..n.div_ceil(max)).map(move |k| Stack::new(item, (n - k * max).min(max) as u16))
}

/// The move rule (Minecraft's): `n` of the stack in `from` go to `to`. Into an empty slot they go; onto the
/// same item as many as it has room for (the rest stay); a whole stack onto another item swaps the two. False
/// when nothing moved: an empty `from`, `n` of 0 or more than it holds, a part onto another item, a full stack.
pub fn rule(from: &mut Slot, to: &mut Slot, n: u16) -> bool {
    let Some(f) = *from else { return false };
    if n == 0 || n > f.n {
        return false;
    }
    let moved = match *to {
        None => {
            *to = Some(Stack { n, ..f });
            n
        }
        Some(t) if t.item == f.item && t.payload == f.payload => {
            let k = n.min(ITEMS.stack(t.item).saturating_sub(t.n));
            if k == 0 {
                return false;
            }
            *to = Some(Stack { n: t.n + k, ..t });
            k
        }
        Some(t) if n == f.n => {
            (*from, *to) = (Some(t), Some(f));
            return true;
        }
        Some(_) => return false,
    };
    *from = (f.n > moved).then_some(Stack { n: f.n - moved, ..f });
    true
}

/// `rule` between slot `i` of `a` and slot `j` of `b`, or of `a` itself when `b` is None. Indices out of range
/// move nothing. The caller checks the holders' own rules after it (`ape_fits`).
pub fn shift(a: &mut [Slot], i: usize, b: Option<&mut [Slot]>, j: usize, n: u16) -> bool {
    match b {
        None if i == j || i >= a.len() || j >= a.len() => false,
        None => {
            let (mut x, mut y) = (a[i], a[j]);
            let ok = rule(&mut x, &mut y, n);
            (a[i], a[j]) = (x, y);
            ok
        }
        Some(b) if i < a.len() && j < b.len() => rule(&mut a[i], &mut b[j], n),
        Some(_) => false,
    }
}

/// An old save's counts of each block (before slots, ADR 0014) as stacks in block id order: the server puts
/// them into the slots and lays what does not fit at the player's feet.
pub fn from_counts(counts: &[(u16, u32)]) -> Vec<Slot> {
    let mut c = counts.to_vec();
    c.sort_unstable();
    c.into_iter().flat_map(|(m, n)| stacks(m, n)).map(Some).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block of full stacks of 64 (stone stacks by 16 since ADR 0093).
    fn stone() -> u16 {
        BLOCKS.id("snow")
    }

    #[test]
    fn every_block_is_an_item_and_names_and_ids_are_unique() {
        let all: Vec<u16> = ITEMS.all().collect();
        assert!(BLOCKS.placeable().all(|d| all.contains(&d.id) && ITEMS.name(d.id) == d.name && ITEMS.stack(d.id) == d.stack));
        let mut names: Vec<&str> = all.iter().map(|&i| ITEMS.name(i)).collect();
        names.sort_unstable();
        let n = names.len();
        names.dedup();
        assert_eq!(names.len(), n, "a name twice");
        assert!(ITEMS.defs.iter().all(|d| d.id >= ITEM_BASE && (1..=STACK).contains(&d.stack) && BLOCKS.get(d.look).is_some()));
        // every block and item is in exactly one tab
        let mut tabbed: Vec<u16> = ITEMS.tabs.concat();
        tabbed.sort_unstable();
        let mut want = all.clone();
        want.sort_unstable();
        assert_eq!(tabbed, want);
        assert!(ITEMS.tabs.iter().all(|t| !t.is_empty()), "an empty tab");
        assert_eq!(ITEMS.named("Hide Pack").and_then(|i| ITEMS.get(i)).map(|d| d.pack), Some(18));
    }

    #[test]
    fn bad_rows_are_refused_with_their_name() {
        for (row, says) in [
            ("[[item]]\nname='stone'\nlook='stone'", "block's"),
            ("[[item]]\nname='x'\nstack=65\nlook='stone'", "stack"),
            ("[[item]]\nname='x'\npack=10\nstack=1\nlook='stone'", "pack"),
            ("[[item]]\nname='x'\npack=9\nlook='stone'", "pack"),
            ("[[item]]\nname='x'\nlook='nothing'", "look"),
            ("made=['nothing']", "made"),
            ("[[item]]\nname='x'\nstack=1\nlook='stone'\ntool='pick'\nhead='bone'", "material"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=1\nuses=9\n[[item]]\nname='x'\nlook='stone'\ntool='pick'\nhead='m'", "stacks by one"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=1\nuses=9\n[[item]]\nname='x'\nstack=1\nlook='stone'\ntool='spoon'\nhead='m'", "tool"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=9\nuses=9", "grade"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=1\nuses=9\n[[material]]\nname='m'\nspeed=3\ngrade=1\nuses=9", "once"),
            ("[[item]]\nname='x'\nstack=1\nlook='stone'\ntool='pick'", "both"),
            ("[[item]]\nname='x'\nlook='stone'\nknap=['y', 4]", "knaps into"),
            ("[[item]]\nname='x'\nlook='stone'\nknap=['x', 4]", "strikes"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=1\nuses=9\nprospect=-1", "prospect"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=1\nuses=9\nprospect=500", "prospect"),
            ("[[material]]\nname='m'\nspeed=2\ngrade=1\nuses=9\n[[item]]\nname='x'\nstack=1\nlook='stone'\ntool='pick'\nhead='m'\nleaves='nothing'", "leaves"),
            ("[[item]]\nname='x'\nlook='stone'\nleaves='stone'", "leaves"),
        ] {
            let e = Items::parse(row).err().unwrap_or_default();
            assert!(e.contains(says), "{row}: {e}");
        }
        // a mineral's sample (data/minerals.toml) names an item of this file
        let text = include_str!("../../../data/items.toml").replace("\r\n", "\n").replace("name = \"gold nugget\"", "name = \"gold flake\"");
        let e = Items::parse(&text).err().unwrap_or_default();
        assert!(e.contains("sample 'gold nugget'"), "{e}");
    }

    /// Prospecting (ADR 0138): the hand axe, of flint, hears ore 16 m off; a pick or
    /// an every-kind tool prospects, an axe does not, nor a head of no reach.
    #[test]
    fn a_flint_tool_prospects_sixteen_metres() {
        let axe = ITEMS.get(ITEMS.named("hand axe").unwrap()).unwrap().tool.clone().unwrap();
        assert_eq!(axe.prospects(), Some(16.0));
        assert_eq!(Tool { kind: Some("axe".into()), ..axe.clone() }.prospects(), None);
        assert_eq!(Tool { kind: Some("pick".into()), ..axe.clone() }.prospects(), Some(16.0));
        assert_eq!(Tool { prospect: 0.0, ..axe }.prospects(), None);
    }

    /// ADR 0110 (items.md "Tests"): every block that is not loose names a tool, and a tool of that kind (or of
    /// every kind) and of at least its grade exists, or the block says it waits for a `later` one; every block a
    /// tool kind names is one of `blocks::TOOLS`.
    #[test]
    fn every_block_that_is_not_loose_has_a_tool_that_takes_it_or_waits() {
        let tools: Vec<&Tool> = ITEMS.defs.iter().filter_map(|d| d.tool.as_ref()).collect();
        assert!(!tools.is_empty(), "no tool in data/items.toml");
        for d in BLOCKS.placeable().filter(|d| !d.loose) {
            assert!(d.tool.as_deref().is_some_and(|t| crate::blocks::TOOLS.contains(&t)), "{} names no tool", d.name);
            let reached = tools.iter().any(|t| t.takes(d));
            assert!(reached != d.later, "{}: a tool takes it {reached}, later {}", d.name, d.later);
        }
        // the first tool, from flint (ADR 0110): every kind at grade 1, faster than hands, a few dozen digs
        let axe = ITEMS.named("hand_axe").and_then(|i| ITEMS.get(i)).expect("a hand axe");
        let t = axe.tool.as_ref().unwrap();
        assert!(t.kind.is_none() && t.grade == 1 && t.speed > 1.0 && axe.stack == 1);
        assert_eq!(ITEMS.get(ITEMS.named("flint").unwrap()).unwrap().knap, Some((axe.id, 4)));
    }

    /// A tool's payload counts its digs (ADR 0110): it wears one a dig and is gone at its last, leaving nothing.
    #[test]
    fn a_tool_wears_one_use_a_dig_and_is_gone_at_its_last() {
        let axe = ITEMS.named("hand axe").unwrap();
        let uses = ITEMS.get(axe).unwrap().tool.as_ref().unwrap().uses;
        let mut slot = Some(Stack::new(axe, 1));
        assert_eq!(left(slot), Some(uses));
        for k in 1..uses {
            assert!(!wear(&mut slot), "gone after {k}");
            assert_eq!(left(slot), Some(uses - k));
        }
        assert!(wear(&mut slot) && slot.is_none());
        // a stack that is no tool wears nothing
        let mut stone = Some(Stack::new(BLOCKS.id("stone"), 3));
        assert!(!wear(&mut stone) && stone == Some(Stack::new(BLOCKS.id("stone"), 3)) && left(stone).is_none());
        // the block it digs: every kind fits; stone is the hand axe's to take, copper waits for grade 2
        let t = tool(Some(Stack::new(axe, 1))).unwrap();
        let get = |n: &str| BLOCKS.get(BLOCKS.id(n)).unwrap();
        assert!(t.takes(get("stone")) && t.fits(get("log")) && t.fits(get("copper")) && !t.takes(get("copper")));
        assert!(knaps(get("granite")) && !knaps(get("gravel")) && !knaps(get("dirt")) && !knaps(get("log")) && !knaps(get("stone rubble")));
    }

    /// Copper (draft ADR recipe-list-and-copper-tools): each hafted copper tool is its kind at speed 4, grade 2 and
    /// 160 uses, takes every block of its kind up to grade 2, and leaves a stick at its last use; the hammer fits no
    /// block yet (none names it); the pick prospects 24 m.
    #[test]
    fn copper_tools_take_grade_two_of_their_kind_and_leave_a_stick() {
        let stick = ITEMS.named("stick").unwrap();
        for (name, kind) in [("copper pick", "pick"), ("copper axe", "axe"), ("copper shovel", "shovel"), ("copper knife", "knife"), ("copper hammer", "hammer")] {
            let id = ITEMS.named(name).unwrap_or_else(|| panic!("no {name}"));
            let t = tool(Some(Stack::new(id, 1))).unwrap();
            assert_eq!((t.kind.as_deref(), t.speed, t.grade, t.uses, t.leaves), (Some(kind), 4.0, 2, 160, Some(stick)), "{name}");
            let of_kind: Vec<_> = BLOCKS.placeable().filter(|d| d.tool.as_deref() == Some(kind)).collect();
            assert_eq!(of_kind.is_empty(), kind == "hammer", "{name}");
            assert!(of_kind.iter().all(|d| t.fits(d) && t.takes(d) == (d.grade <= 2)), "{name}");
            let mut slot = Some(Stack::new(id, 1));
            for k in 1..t.uses {
                assert!(!wear(&mut slot), "{name} gone after {k}");
            }
            assert!(wear(&mut slot) && slot == Some(Stack::new(stick, 1)), "{name}: {slot:?}");
        }
        let pick = tool(Some(Stack::new(ITEMS.named("copper pick").unwrap(), 1))).unwrap();
        assert!(pick.takes(BLOCKS.get(BLOCKS.id("copper")).unwrap()) && pick.prospects() == Some(24.0));
        assert_eq!(tool(Some(Stack::new(ITEMS.named("hand axe").unwrap(), 1))).unwrap().leaves, None);
    }

    #[test]
    fn stacks_merge_up_to_their_size_then_fill_empty_slots() {
        let mut s = ape();
        assert_eq!(give(&mut s, Stack::new(stone(), 50)), 0);
        assert_eq!(give(&mut s, Stack::new(stone(), 30)), 0);
        assert_eq!((s[0], s[1]), (Some(Stack::new(stone(), 64)), Some(Stack::new(stone(), 16))));
        // nine slots and no pack: 576 fit, the rest is handed back
        assert_eq!(give(&mut s, Stack::new(stone(), 600)), 600 - (576 - 80));
        assert!(s[WORN..].iter().all(Option::is_none) && ape_fits(&s));
        assert!(take(&mut s, stone(), 100) && count(&s, stone()) == 476 && s[0].is_none());
        assert!(!take(&mut s, stone(), 477) && count(&s, stone()) == 476, "all or nothing");
    }

    #[test]
    fn the_move_rule_moves_merges_swaps_or_refuses() {
        let (d, st) = (BLOCKS.id("dirt"), stone());
        let mut a: Vec<Slot> = vec![Some(Stack::new(st, 10)), None, Some(Stack::new(d, 60)), Some(Stack::new(d, 10))];
        assert!(shift(&mut a, 0, None, 1, 4), "a part into an empty slot");
        assert_eq!((a[0].unwrap().n, a[1].unwrap().n), (6, 4));
        assert!(shift(&mut a, 3, None, 2, 10), "onto the same item: what has room");
        assert_eq!((a[2].unwrap().n, a[3].unwrap().n), (64, 6));
        assert!(!shift(&mut a, 3, None, 2, 6), "onto a full stack: nothing");
        assert!(!shift(&mut a, 3, None, 0, 3), "a part onto another item: nothing");
        assert!(shift(&mut a, 3, None, 0, 6), "a whole stack onto another: a swap");
        assert_eq!((a[0].unwrap().item, a[3].unwrap().item), (d, st));
        assert!(!shift(&mut a, 0, None, 0, 1) && !shift(&mut a, 9, None, 0, 1) && !shift(&mut a, 1, None, 0, 0) && !shift(&mut a, 1, None, 0, 5));
        // between two holders
        let mut pile: Vec<Slot> = vec![None; PILE_SLOTS];
        assert!(shift(&mut a, 2, Some(&mut pile), 3, 64) && a[2].is_none() && pile[3].unwrap().n == 64);
    }

    #[test]
    fn a_pack_opens_its_rows_and_cannot_leave_while_they_hold_things() {
        let basket = ITEMS.named("basket").unwrap();
        let mut s = ape();
        s[0] = Some(Stack::new(basket, 1));
        // into the worn slot: the rows open
        assert!(shift(&mut s, 0, None, WORN, 1) && ape_fits(&s) && open(&s) == ROWS_AT + 9);
        assert_eq!(give(&mut s, Stack::new(stone(), 64 * 18)), 0);
        assert!(s[ROWS_AT + 8].is_some());
        // taken off with its rows full: the slots break the rule, so the server refuses the move
        let mut t = s.clone();
        t[0] = None;
        assert!(shift(&mut t, WORN, None, 0, 1) && !ape_fits(&t), "rows full and no pack");
        // stone in the worn slot breaks the rule too
        let mut v = ape();
        v[WORN] = Some(Stack::new(stone(), 1));
        assert!(!ape_fits(&v));
    }

    #[test]
    fn old_counts_become_stacks_in_block_id_order() {
        let (d, st) = (BLOCKS.id("dirt"), stone());
        let s = from_counts(&[(st, 70), (d, 3)]);
        let want = if d < st { vec![Stack::new(d, 3), Stack::new(st, 64), Stack::new(st, 6)] } else { vec![Stack::new(st, 64), Stack::new(st, 6), Stack::new(d, 3)] };
        assert_eq!(s, want.into_iter().map(Some).collect::<Vec<_>>());
    }
}
