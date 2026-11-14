//! The inventory screens (ADR 0075), Minecraft's: the inventory key (E) opens the ape's slots. Survival shows
//! the hotbar, the worn pack's slot and the rows it opens, and the pile at the feet; creative shows every block
//! and item in a grid with tabs and a search over the hotbar. A click lifts a stack (a right click half of it)
//! and the next puts it down (a right click one of it); shift-click sends it across; a click outside the
//! panel throws it. Every move is one `ToServer::Items`, predicted here with the server's rule (ADR 0074).
use super::*;
use engine::items::{self, Holder, Slot, Stack, ITEMS, TABS};

const SIZE: f32 = 52.0;
const GAP: f32 = 6.0;
const COLS: usize = 9;
/// Rows of creative's grid in view; the wheel scrolls it.
pub(crate) const GRID_ROWS: usize = 5;
/// The search tab, after `items::TABS`.
pub(crate) const SEARCH: usize = TABS.len();
/// A pile within this of the feet is shown beside the slots (the server's reach for one).
const PILE_SHOWN: f64 = 4.0;

/// Where the screen's parts lie, in UI pixels: each slot with its holder and index (creative's grid: the
/// item's id), the tabs (creative), and the panel; a click outside the panel throws.
#[derive(Default, Clone)]
pub(crate) struct Layout {
    pub spots: Vec<(Holder, u16, [f32; 4])>,
    pub tabs: Vec<[f32; 4]>,
    pub panel: [f32; 4],
}

fn inside(b: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]
}

/// The screen on a `lw` x `lh` UI. Creative: the tabs, `grid`'s items from row `scroll` on, the hotbar under
/// them. Survival: the rows the worn pack opens (`open`, `items::open`) over the hotbar, the worn slot to their
/// left, a pile's 4 slots to their right.
pub(crate) fn layout(lw: f32, lh: f32, creative: bool, grid: &[u16], scroll: usize, open: usize, pile: Option<Block>) -> Layout {
    let step = SIZE + GAP;
    let wide = COLS as f32 * step - GAP;
    let x0 = (lw - wide) / 2.0;
    let mut l = Layout::default();
    let at = |k: usize, y: f32| [x0 + (k % COLS) as f32 * step, y + (k / COLS) as f32 * step, SIZE, SIZE];
    let (top, hot_y);
    if creative {
        let h = 44.0 + GRID_ROWS as f32 * step + 18.0 + SIZE;
        top = ((lh - h) / 2.0).max(40.0);
        let tw = (wide + GAP) / (TABS.len() + 1) as f32;
        l.tabs = (0..=TABS.len()).map(|k| [x0 + k as f32 * tw, top, tw - GAP, 32.0]).collect();
        for (k, id) in grid.iter().skip(scroll * COLS).take(GRID_ROWS * COLS).enumerate() {
            l.spots.push((Holder::Grid, *id, at(k, top + 44.0)));
        }
        hot_y = top + 44.0 + GRID_ROWS as f32 * step + 18.0;
    } else {
        let rows = (open.saturating_sub(items::ROWS_AT)).div_ceil(COLS);
        let h = rows as f32 * step + if rows > 0 { 18.0 } else { 0.0 } + SIZE;
        top = ((lh - h) / 2.0).max(40.0);
        for k in 0..open.saturating_sub(items::ROWS_AT) {
            l.spots.push((Holder::Ape, (items::ROWS_AT + k) as u16, at(k, top)));
        }
        hot_y = top + h - SIZE;
        l.spots.push((Holder::Ape, items::WORN as u16, [x0 - step - 18.0, top, SIZE, SIZE]));
        if let Some(b) = pile {
            for k in 0..items::PILE_SLOTS {
                l.spots.push((Holder::Pile(b), k as u16, [x0 + wide + 18.0 + (k % 2) as f32 * step, top + (k / 2) as f32 * step, SIZE, SIZE]));
            }
        }
    }
    for k in 0..items::HOTBAR {
        l.spots.push((Holder::Ape, k as u16, at(k, hot_y)));
    }
    let (left, right) = if creative { (x0, x0 + wide) } else { (x0 - step - 18.0, x0 + wide + if pile.is_some() { 18.0 + 2.0 * step - GAP } else { 0.0 }) };
    l.panel = [left - 18.0, top - 34.0, right - left + 36.0, hot_y + SIZE - top + 52.0];
    l
}

#[test]
fn the_screens_put_every_slot_apart_and_inside_the_panel() {
    let grid: Vec<u16> = ITEMS.tabs[0].clone();
    let pile = Block { body: 0, face: 0, pos: glam::IVec3::ZERO };
    for (creative, open, pile) in [(true, items::ROWS_AT, None), (false, items::ROWS_AT, None), (false, items::APE_SLOTS, Some(pile))] {
        let l = layout(1280.0, 720.0, creative, &grid, 0, open, pile);
        let want = if creative { grid.len().min(GRID_ROWS * COLS) + 9 } else { 9 + 1 + (open - items::ROWS_AT) + pile.map_or(0, |_| 4) };
        assert_eq!(l.spots.len(), want);
        for (k, a) in l.spots.iter().enumerate() {
            let b = a.2;
            assert!(inside(l.panel, (b[0], b[1])) && inside(l.panel, (b[0] + b[2] - 1.0, b[1] + b[3] - 1.0)), "{creative} slot {k} out of the panel");
            assert!(l.spots[..k].iter().all(|o| !inside(o.2, (b[0] + 1.0, b[1] + 1.0))), "{creative} slot {k} over another");
        }
    }
}

impl App {
    /// The items creative's grid shows on the open tab: a tab's own, or every one whose name holds the search.
    pub(crate) fn grid_items(&self) -> Vec<u16> {
        if self.inv_tab < TABS.len() {
            return ITEMS.tabs[self.inv_tab].clone();
        }
        let q = self.inv_query.to_lowercase();
        ITEMS.all().filter(|&i| ITEMS.name(i).to_lowercase().contains(&q)).collect()
    }

    /// The nearest pile in reach of the feet, if any.
    pub(crate) fn pile_near(&self) -> Option<Block> {
        let me = self.me.pos;
        self.piles.keys().map(|b| (self.world.block_centre(*b).distance(me), *b)).filter(|(d, _)| *d <= PILE_SHOWN).min_by(|a, b| a.0.total_cmp(&b.0)).map(|p| p.1)
    }

    pub(crate) fn inv_layout(&self) -> Layout {
        let (lw, lh) = self.ui_size();
        layout(lw, lh, self.creative, &self.grid_items(), self.inv_scroll, items::open(&self.inv), self.pile_near())
    }

    /// What lies in a slot as this client has it; creative's grid holds a full stack of anything.
    fn slot_at(&self, h: Holder, i: u16) -> Slot {
        match h {
            Holder::Ape => self.inv.get(i as usize).copied().flatten(),
            Holder::Pile(b) => self.piles.get(&b).and_then(|p| p.get(i as usize).copied().flatten()),
            Holder::Grid => Some(Stack::new(i, ITEMS.stack(i))),
            Holder::Ground => None,
        }
    }

    /// The inventory key: opens the screen, or closes it.
    pub(crate) fn toggle_inventory(&mut self) {
        match self.menu {
            Some(Menu::Inventory) => self.set_menu(None),
            None if self.home() != Menu::Title && self.dead.is_none() => {
                self.lifted = None;
                self.set_menu(Some(Menu::Inventory));
            }
            _ => {}
        }
    }

    /// A click at the pointer on the screen: `right` the right button, `shift` with shift held.
    /// shortcut: no drag that spreads a stack, no number keys sending to a hotbar slot, the pack's rows not
    /// drawn as that pack, no container beside the slots; items.md "The screen", when players miss them.
    pub(crate) fn inv_click(&mut self, right: bool, shift: bool) {
        let l = self.inv_layout();
        let p = self.pointer;
        // a lifted stack the server took away meanwhile (a pile picked up, a refused move): let go
        if let Some((h, i, n)) = self.lifted {
            if self.slot_at(h, i).is_none_or(|s| s.n < n) {
                self.lifted = None;
            }
        }
        if let Some(k) = l.tabs.iter().position(|b| inside(*b, p)) {
            (self.inv_tab, self.inv_scroll) = (k, 0);
            return;
        }
        let spot = l.spots.iter().find(|s| inside(s.2, p)).map(|s| (s.0, s.1));
        match (self.lifted, spot) {
            (None, Some((h, i))) => {
                let Some(st) = self.slot_at(h, i) else { return };
                if shift {
                    if let Some(to) = self.across(h, i, st) {
                        self.send_items((h, i), to, st.n);
                    }
                } else {
                    let n = if h == Holder::Grid && right { 1 } else if right { st.n.div_ceil(2) } else { st.n };
                    self.lifted = Some((h, i, n));
                }
            }
            (Some((fh, fi, n)), Some((h, i))) => {
                let k = if right { 1 } else { n };
                if (fh, fi) == (h, i) {
                    self.lifted = None;
                    return;
                }
                if h == Holder::Grid {
                    // put into creative's grid: gone (a stack lifted from the grid is only let go)
                    if fh != Holder::Grid {
                        self.send_items((fh, fi), (Holder::Grid, 0), n);
                    }
                    self.lifted = None;
                    return;
                }
                self.send_items((fh, fi), (h, i), k);
                self.lifted = (right && n > 1).then_some((fh, fi, n - 1));
            }
            (Some((fh, fi, n)), None) if !inside(l.panel, p) => {
                let k = if right { 1 } else { n };
                self.send_items((fh, fi), (Holder::Ground, 0), k);
                self.lifted = (right && n > 1).then_some((fh, fi, n - 1));
            }
            _ => {}
        }
    }

    /// Where shift-click sends the stack in slot `i` of `h`: from the hotbar to the pack's rows, from them to
    /// the hotbar, from a pile or the grid into the ape; the first slot of the same item with room, else the
    /// first empty one.
    fn across(&self, h: Holder, i: u16, st: Stack) -> Option<(Holder, u16)> {
        let hot = (h, (i as usize) < items::HOTBAR) == (Holder::Ape, true);
        let to: Vec<usize> = match h {
            Holder::Ape if hot => (items::ROWS_AT..items::open(&self.inv)).collect(),
            Holder::Ape => (0..items::HOTBAR).collect(),
            _ => items::usable(&self.inv).collect(),
        };
        let room = |k: &usize| self.inv[*k].is_some_and(|t| t.item == st.item && t.payload == st.payload && t.n < ITEMS.stack(t.item));
        to.iter().find(|k| room(k)).or_else(|| to.iter().find(|k| self.inv[**k].is_none())).map(|&k| (Holder::Ape, k as u16))
    }

    /// Sends a move and shows it at once: the ape's own slots with the server's rule; a pile's change comes
    /// with the server's answer (shortcut: piles are not predicted, they are a tick away).
    pub(crate) fn send_items(&mut self, from: (Holder, u16), to: (Holder, u16), n: u16) {
        let seq = self.pred.seq();
        self.items_pending.push(seq);
        self.link.send(ToServer::Items { seq, from, to, n });
        self.audio.play("ui.tick", None, 1.0, 1.0, false);
        let mut s = self.inv.clone();
        let (fi, ti) = (from.1 as usize, to.1 as usize);
        let ok = match (from.0, to.0) {
            (Holder::Ape, Holder::Ape) => items::shift(&mut s, fi, None, ti, n),
            (Holder::Grid, Holder::Ape) if ti < s.len() => {
                if s[ti].is_some_and(|o| o.item != from.1) {
                    s[ti] = None;
                }
                items::rule(&mut Some(Stack::new(from.1, ITEMS.stack(from.1))), &mut s[ti], n)
            }
            (Holder::Ape, Holder::Grid | Holder::Ground) => match s.get_mut(fi) {
                Some(Some(st)) if n <= st.n => {
                    st.n -= n;
                    if st.n == 0 {
                        s[fi] = None;
                    }
                    true
                }
                _ => false,
            },
            _ => false,
        };
        if ok && items::ape_fits(&s) {
            self.inv = s;
        }
    }

    /// Creative's middle click on a block in the world (ADR 0075): the hotbar slot that holds it is taken,
    /// else a stack of it goes into the slot in hand.
    pub(crate) fn pick_block(&mut self) {
        let Some((hit, _)) = self.aim.filter(|_| self.creative) else { return };
        let m = self.world.block(hit);
        if !ITEMS.known(m) {
            return;
        }
        if let Some(k) = (0..items::HOTBAR).find(|&k| self.inv[k].is_some_and(|s| s.item == m)) {
            self.slot = k;
        } else {
            self.send_items((Holder::Grid, m), (Holder::Ape, self.slot as u16), ITEMS.stack(m));
        }
    }

    /// The screen over the world: the panel, its slots and what they hold, the lifted stack at the pointer and
    /// the name of the one under it.
    pub(crate) fn inv_draw(&self, p: &mut hud::Paint, season: (f64, f64)) {
        let l = self.inv_layout();
        let pointer = self.pointer;
        hud::glass(p, l.panel[0], l.panel[1], l.panel[2], l.panel[3]);
        let title = if self.creative { "Every block and item" } else { "Carried" };
        p.text(l.panel[0] + 18.0, l.panel[1] + 8.0, title, [0.95, 0.95, 0.95, 1.0]);
        for (k, b) in l.tabs.iter().enumerate() {
            let on = k == self.inv_tab;
            p.rect(b[0], b[1], b[2], b[3], if on { [CYAN[0], CYAN[1], CYAN[2], 0.35] } else { [0.09, 0.12, 0.17, 0.85] }, 0);
            let name = if k == SEARCH { if self.inv_query.is_empty() { "search".to_string() } else { self.inv_query.clone() } } else { TABS[k].to_string() };
            let px = hud::PX * 0.75;
            let name: String = name.chars().take(((b[2] - 8.0) / (hud::ADV * px)) as usize).collect();
            p.text_px(b[0] + 6.0, b[1] + 10.0, &name, if on { [1.0; 4] } else { [0.7, 0.78, 0.86, 1.0] }, px);
        }
        let mut hover = None;
        for &(h, i, b) in &l.spots {
            let lit = inside(b, pointer);
            p.rect(b[0], b[1], b[2], b[3], if lit { [0.16, 0.22, 0.3, 1.0] } else { [0.09, 0.12, 0.17, 0.85] }, 0);
            if (h, i as usize) == (Holder::Ape, self.slot) {
                hud::brackets(p, [b[0] - 2.0, b[1] - 2.0, b[2] + 4.0, b[3] + 4.0], 8.0, false, CYAN);
            }
            let mut st = self.slot_at(h, i);
            if let (Some((fh, fi, n)), Some(s)) = (self.lifted, st.as_mut()) {
                if (fh, fi) == (h, i) && h != Holder::Grid {
                    s.n = s.n.saturating_sub(n);
                }
            }
            match st.filter(|s| s.n > 0) {
                Some(s) => {
                    icon(p, s, b[0] + 10.0, b[1] + 10.0, 32.0, season, h != Holder::Grid);
                    if lit {
                        hover = Some(s.item);
                    }
                }
                None if (h, i as usize) == (Holder::Ape, items::WORN) => p.text_px(b[0] + 8.0, b[1] + 20.0, "pack", [0.5, 0.58, 0.66, 1.0], hud::PX * 0.75),
                None => {}
            }
        }
        if let Some((fh, fi, n)) = self.lifted {
            if let Some(s) = self.slot_at(fh, fi) {
                icon(p, Stack { n, ..s }, pointer.0 - 16.0, pointer.1 - 16.0, 32.0, season, true);
            }
        } else if let Some(item) = hover {
            let name = ITEMS.name(item);
            let w = hud::text_width(name) + 16.0;
            hud::glass(p, pointer.0 + 14.0, pointer.1 - 30.0, w, hud::LINE + 6.0);
            p.text(pointer.0 + 22.0, pointer.1 - 26.0, name, [0.95, 0.95, 0.95, 1.0]);
        }
        let hint = if self.creative { "Click: a stack. Right click: one. Shift-click: into the hotbar. Wheel: more." } else { "Click: lift, put. Right click: half, one. Shift-click: across. Outside: throw." };
        p.text_px(l.panel[0] + 18.0, l.panel[1] + l.panel[3] + 8.0, hint, [0.75, 0.82, 0.9, 0.9], hud::PX * 0.75);
    }
}

/// A stack's look in a square of `size` at (x, y): the cube of its block (an item: of the block it looks
/// like, with a mark of the accent in its corner), and its count over one when `count`.
fn icon(p: &mut hud::Paint, s: Stack, x: f32, y: f32, size: f32, season: (f64, f64), count: bool) {
    let Some(d) = BLOCKS.get(ITEMS.look(s.item)) else { return };
    let (colour, side, pattern) = crate::gpu::shown(d, season);
    let pt = crate::gpu::painting(d);
    hud::cube(&mut |x, y, w, h, c, g| p.rect(x, y, w, h, c, g), x, y, size, colour, side, pattern, 1.0, [pt[0].as_ref(), pt[1].as_ref()]);
    if s.item >= items::ITEM_BASE {
        p.rect(x - 6.0, y - 6.0, 8.0, 8.0, CYAN, 0);
    }
    if let Some(left) = crate::play::wear(s) {
        hud::wear_bar(p, x - 2.0, y + size + 2.0, size + 4.0, left);
    } else if count && s.n > 1 {
        let t = s.n.to_string();
        let px = hud::PX * 0.75;
        p.text_px(x + size + 6.0 - t.len() as f32 * hud::ADV * px, y + size - 4.0, &t, [1.0; 4], px);
    }
}

use hud::CYAN;

/// Piles farther than this are not drawn: a quarter-metre cube is a few pixels there.
const PILE_DRAWN: f64 = 64.0;

impl App {
    /// The piles on the ground as small cubes of what they hold (ADR 0074), one a stack, laid among this
    /// frame's particles: still debris, turned by the cell so a pile looks the same each frame.
    pub(crate) fn lay_piles(&mut self) {
        let me = self.me.pos;
        let mut still = Vec::new();
        for (b, slots) in &self.piles {
            if self.world.block_centre(*b).distance(me) > PILE_DRAWN {
                continue;
            }
            let at = fx::At::floor_of(&self.world, *b);
            let seed = b.pos.x as u32 ^ (b.pos.y as u32).rotate_left(11) ^ (b.pos.z as u32).rotate_left(22);
            for (k, s) in slots.iter().enumerate().filter_map(|(k, s)| Some((k, (*s)?))) {
                let side = [(-0.15, -0.15), (0.15, 0.1), (-0.1, 0.17), (0.17, -0.15)][k % 4];
                let p = at.p + at.axes[0] * 0.13 + at.axes[1] * side.0 + at.axes[2] * side.1;
                let info = ITEMS.look(s.item) as u32 | (b.body as u32) << 8;
                still.push(fx::FxGpu { p: p.to_array(), size: 0.25, info, angle: k as f32 * 0.7, a: seed.wrapping_add(k as u32), b: 0 });
            }
        }
        self.fx.lay(still);
    }
}
