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
pub(crate) const GRID_ROWS: usize = 6;
/// The search tab, after `items::TABS`: its field on the title row.
pub(crate) const SEARCH: usize = TABS.len();
/// A pile within this of the feet is shown beside the slots (the server's reach for one).
const PILE_SHOWN: f64 = 4.0;
/// The window's margin, its title row, a section's heading over its slots (ADR 0113).
const PAD: f32 = 22.0;
const TITLE_H: f32 = 50.0;
const HEAD: f32 = 24.0;
/// Survival's column of the ape: its picture's inset.
const APE_W: f32 = 196.0;
const APE_H: f32 = 196.0;
/// Creative's search field on the title row.
const SEARCH_W: f32 = 220.0;
/// The hints' text size, and each screen's (short enough to fit its window).
const HINT_PX: f32 = hud::PX * 0.75;
const HINT_SURVIVAL: &str = "Click: lift, put    Right click: half, one    Shift-click: across    Click outside: throw";
const HINT_CREATIVE: &str = "Click: a stack  Right: one  Shift: to hotbar  Wheel: more";

/// Where the screen's parts lie, in UI pixels: each slot with its holder and index (creative's grid: the
/// item's id), the tabs (creative; the last the search field), and the panel; a click outside the panel throws.
#[derive(Default, Clone)]
pub(crate) struct Layout {
    pub spots: Vec<(Holder, u16, [f32; 4])>,
    pub tabs: Vec<[f32; 4]>,
    pub panel: [f32; 4],
    /// The line of hints under the slots, as wide as its text.
    pub hint: [f32; 4],
    /// Survival: the ape's picture.
    pub ape: [f32; 4],
    /// Survival: the pack's rows (three rows' room); creative: the grid and its scrollbar.
    pub rows: [f32; 4],
    /// Survival: the pile's room. Both: the hotbar's row.
    pub ground: [f32; 4],
    pub hot: [f32; 4],
    /// UI pixels a pixel of this layout: below 1 where the window would not fit the UI, so the whole screen is
    /// drawn smaller (every part above is in the layout's pixels; the pointer is divided by it).
    pub k: f32,
}

fn inside(b: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]
}

/// The screen on a `lw` x `lh` UI: one window in the middle. Creative: the title and the search field, the
/// tabs, `grid`'s items from row `scroll` on with a scrollbar, the hotbar under a divider. Survival: the ape
/// over the worn slot | the rows the worn pack opens (`open`, `items::open`) over the hotbar | a pile's 4 slots.
pub(crate) fn layout(lw: f32, lh: f32, creative: bool, grid: &[u16], scroll: usize, open: usize, pile: Option<Block>) -> Layout {
    let step = SIZE + GAP;
    let wide = COLS as f32 * step - GAP;
    let mut l = Layout::default();
    // the content's width and height under the title row (the hint's line under that)
    let (iw, ih) = if creative {
        (wide + 14.0, 30.0 + 12.0 + GRID_ROWS as f32 * step - GAP + 40.0 + SIZE)
    } else {
        (APE_W + 28.0 + wide + 28.0 + 2.0 * step - GAP, HEAD + 3.0 * step - GAP + 20.0 + HEAD + SIZE)
    };
    let (pw, ph) = (iw + 2.0 * PAD, TITLE_H + ih + 18.0 + 8.0 * HINT_PX + 18.0);
    // a UI too small for the window (a small window at a large interface scale) draws all of it smaller, so every
    // slot stays on the screen and takes its click
    l.k = (lw / (pw + 16.0)).min(lh / (ph + 16.0)).min(1.0);
    let (lw, lh) = (lw / l.k, lh / l.k);
    let (left, top) = (((lw - pw) / 2.0).floor(), ((lh - ph) / 2.0).max(8.0).floor());
    l.panel = [left, top, pw, ph];
    let (x0, ct) = (left + PAD, top + TITLE_H);
    let hot_y;
    if creative {
        let tw = ((iw + GAP) / TABS.len() as f32 - GAP).floor();
        l.tabs = (0..TABS.len()).map(|k| [x0 + k as f32 * (tw + GAP), ct, tw, 30.0]).collect();
        l.tabs.push([left + pw - PAD - SEARCH_W, top + 10.0, SEARCH_W, 30.0]);
        let gy = ct + 42.0;
        l.rows = [x0, gy, iw, GRID_ROWS as f32 * step - GAP];
        for (k, id) in grid.iter().skip(scroll * COLS).take(GRID_ROWS * COLS).enumerate() {
            l.spots.push((Holder::Grid, *id, [x0 + (k % COLS) as f32 * step, gy + (k / COLS) as f32 * step, SIZE, SIZE]));
        }
        hot_y = gy + l.rows[3] + 40.0;
    } else {
        l.ape = [x0, ct, APE_W, APE_H];
        let mx = x0 + APE_W + 28.0;
        l.rows = [mx, ct + HEAD, wide, 3.0 * step - GAP];
        for k in 0..open.saturating_sub(items::ROWS_AT) {
            l.spots.push((Holder::Ape, (items::ROWS_AT + k) as u16, [mx + (k % COLS) as f32 * step, l.rows[1] + (k / COLS) as f32 * step, SIZE, SIZE]));
        }
        hot_y = l.rows[1] + l.rows[3] + 20.0 + HEAD;
        l.spots.push((Holder::Ape, items::WORN as u16, [x0, hot_y, SIZE, SIZE]));
        l.ground = [mx + wide + 28.0, ct + HEAD, 2.0 * step - GAP, 2.0 * step - GAP];
        if let Some(b) = pile {
            for k in 0..items::PILE_SLOTS {
                l.spots.push((Holder::Pile(b), k as u16, [l.ground[0] + (k % 2) as f32 * step, l.ground[1] + (k / 2) as f32 * step, SIZE, SIZE]));
            }
        }
    }
    let hx = if creative { x0 } else { x0 + APE_W + 28.0 };
    l.hot = [hx, hot_y, wide, SIZE];
    for k in 0..items::HOTBAR {
        l.spots.push((Holder::Ape, k as u16, [hx + k as f32 * step, hot_y, SIZE, SIZE]));
    }
    let hint = if creative { HINT_CREATIVE } else { HINT_SURVIVAL };
    l.hint = [x0, ct + ih + 18.0, hint.chars().count() as f32 * hud::ADV * HINT_PX, 8.0 * HINT_PX];
    l
}

#[test]
fn the_screens_put_every_slot_apart_and_inside_the_panel() {
    let grid: Vec<u16> = ITEMS.tabs[0].clone();
    let pile = Block { body: 0, face: 0, pos: glam::IVec3::ZERO };
    let within = |o: [f32; 4], b: [f32; 4]| inside(o, (b[0], b[1])) && inside(o, (b[0] + b[2] - 1.0, b[1] + b[3] - 1.0));
    let over = |a: [f32; 4], b: [f32; 4]| a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3];
    // the settings' smallest window (640 x 360) at their largest interface scale (3), a 1280 x 720 window at 1.5,
    // and the usual sizes
    for (lw, lh) in [(640.0 / 3.0, 120.0), (1280.0 / 1.5, 480.0), (1280.0, 720.0), (1920.0, 1080.0)] {
        for (creative, open, pile) in [(true, items::ROWS_AT, None), (false, items::ROWS_AT, None), (false, items::APE_SLOTS, Some(pile))] {
            let l = layout(lw, lh, creative, &grid, 0, open, pile);
            let at = format!("{lw}x{lh} creative {creative}");
            let want = if creative { grid.len().min(GRID_ROWS * COLS) + 9 } else { 9 + 1 + (open - items::ROWS_AT) + pile.map_or(0, |_| 4) };
            assert_eq!(l.spots.len(), want);
            // everything that takes a click on the screen, in UI pixels
            let shown = |b: [f32; 4]| b.map(|v| v * l.k);
            assert!(l.k > 0.0 && l.k <= 1.0, "{at}: scale {}", l.k);
            for b in std::iter::once(l.panel).chain(l.spots.iter().map(|s| s.2)).chain(l.tabs.iter().copied()) {
                assert!(within([0.0, 0.0, lw, lh], shown(b)), "{at}: {b:?} off the screen at scale {}", l.k);
            }
            for (k, a) in l.spots.iter().enumerate() {
                let b = a.2;
                assert!(within(l.panel, b), "{at}: slot {k} out of the panel");
                assert!(l.spots[..k].iter().all(|o| !over(o.2, b)), "{at}: slot {k} over another");
                assert!(!over(l.ape, b), "{at}: slot {k} over the ape");
            }
            // creative's tabs and its search field (the last) side by side in the panel, over no slot
            assert_eq!(l.tabs.len(), if creative { TABS.len() + 1 } else { 0 });
            for (k, t) in l.tabs.iter().enumerate() {
                assert!(within(l.panel, *t), "{at}: tab {k} out of the panel");
                assert!(l.tabs[..k].iter().all(|o| !over(*o, *t)), "{at}: tab {k} over another");
                assert!(l.spots.iter().all(|s| !over(s.2, *t)), "{at}: tab {k} over a slot");
                // a tab's name fits in it
                if k < TABS.len() {
                    assert!(hud::text_width(TABS[k]) + 8.0 <= t[2], "{at}: tab {k}'s name is cut");
                }
            }
            assert!(within(l.panel, l.hint), "{at}: the hint leaves the panel");
            assert!(l.spots.iter().all(|s| !over(s.2, l.hint)), "{at}: the hint over a slot");
            if !creative {
                assert!(l.ape[2] >= 150.0 && within(l.panel, l.ape), "{at}: the ape's picture");
            }
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

    /// The pointer in the layout's pixels (`Layout::k`).
    fn inv_pointer(&self, l: &Layout) -> (f32, f32) {
        (self.pointer.0 / l.k, self.pointer.1 / l.k)
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
        let p = self.inv_pointer(&l);
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

    /// The screen over the world (ADR 0113): the window in the Esc menu's glass, its title row, its sections
    /// under small headings, the slots and what they hold (pictures of their models where there are some),
    /// survival's ape, the lifted stack at the pointer and a tooltip on the one under it.
    pub(crate) fn inv_draw(&self, out: &mut hud::Paint, season: (f64, f64)) {
        let l = self.inv_layout();
        // drawn on a canvas 1 / k as large over the same screen: all of it smaller when the UI is small
        let mut scaled = hud::Paint::new(out.w / l.k, out.h / l.k);
        self.inv_draw_in(&mut scaled, &l, season);
        out.out.extend(scaled.out);
    }

    fn inv_draw_in(&self, p: &mut hud::Paint, l: &Layout, season: (f64, f64)) {
        let pointer = self.inv_pointer(l);
        let [px0, py0, pw, _] = l.panel;
        // the world a little darker under it, as under the menu
        p.rect(0.0, 0.0, p.w, p.h, [0.0, 0.01, 0.03, 0.35], 0);
        hud::glass(p, l.panel[0], l.panel[1], l.panel[2], l.panel[3]);
        hud::brackets(p, [px0 - 4.0, py0 - 4.0, pw + 8.0, l.panel[3] + 8.0], 14.0, false, CYAN);
        // the title row: the accent's bar and the name, as the menu's pages; a hairline under it
        p.rect(px0 + PAD, py0 + 13.0, 3.0, 24.0, CYAN, 0);
        p.text_px(px0 + PAD + 14.0, py0 + 13.0, if self.creative { "CREATIVE" } else { "INVENTORY" }, [1.0; 4], hud::PX * 1.5);
        p.rect(px0 + PAD, py0 + TITLE_H - 6.0, pw - 2.0 * PAD, 1.0, [CYAN[0], CYAN[1], CYAN[2], 0.25], 0);
        let head = |p: &mut hud::Paint, x: f32, y: f32, s: &str| p.text_px(x, y - 18.0, &s.to_uppercase(), [CYAN[0], CYAN[1], CYAN[2], 0.85], hud::PX * 0.75);
        let dim = [0.55, 0.62, 0.7, 0.9];
        let small = hud::PX * 0.75;
        let caret = self.start.elapsed().as_millis() / 500 % 2 == 0;
        for (k, b) in l.tabs.iter().enumerate() {
            let on = k == self.inv_tab;
            if k == SEARCH {
                // the search field: always there; lit, with a caret, while it is the open tab
                p.rect(b[0], b[1], b[2], b[3], [0.01, 0.03, 0.06, 0.95], 0);
                edge(p, *b, if on { CYAN } else { [1.0, 1.0, 1.0, 0.16] });
                let cols = ((b[2] - 16.0) / (hud::ADV * hud::PX)) as usize;
                let q: String = self.inv_query.chars().rev().take(cols.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect();
                if q.is_empty() && !on {
                    p.text_px(b[0] + 8.0, b[1] + 8.0, "Search...", dim, hud::PX);
                } else {
                    let shown = if on && caret { format!("{q}_") } else { q };
                    p.text_px(b[0] + 8.0, b[1] + 8.0, &shown, [1.0; 4], hud::PX);
                }
                continue;
            }
            let name = TABS[k];
            let tx = b[0] + (b[2] - hud::text_width(name)) / 2.0;
            if on {
                hud::cut(p, b[0], b[1], b[2], b[3], CYAN);
                p.text_flat(tx, b[1] + 8.0, name, [0.01, 0.06, 0.12, 1.0], hud::PX);
            } else {
                let lit = inside(*b, pointer);
                hud::cut(p, b[0], b[1], b[2], b[3], if lit { [0.16, 0.22, 0.3, 1.0] } else { [0.09, 0.12, 0.17, 0.85] });
                p.rect(b[0] + 2.0, b[1] + b[3] - 2.0, b[2] - 4.0, 2.0, [CYAN[0], CYAN[1], CYAN[2], 0.35], 0);
                p.text_px(tx, b[1] + 8.0, name, [0.8, 0.86, 0.92, 1.0], hud::PX);
            }
        }
        if self.creative {
            // the grid's scrollbar, its count, and the divider over the hotbar
            let all = self.grid_items();
            let total = all.len().div_ceil(COLS).max(1);
            let track = [l.rows[0] + l.rows[2] - 6.0, l.rows[1], 6.0, l.rows[3]];
            p.rect(track[0], track[1], track[2], track[3], [1.0, 1.0, 1.0, 0.06], 0);
            let shown = (GRID_ROWS as f32 / total as f32).min(1.0);
            let at = (self.inv_scroll as f32 / total as f32).min(1.0 - shown);
            p.rect(track[0], track[1] + track[3] * at, track[2], (track[3] * shown).max(12.0), if total > GRID_ROWS { CYAN } else { [CYAN[0], CYAN[1], CYAN[2], 0.3] }, 0);
            let n = all.len();
            let below = l.rows[1] + l.rows[3] + 8.0;
            p.text_px(l.rows[0], below, &format!("{n} {}", if n == 1 { "item" } else { "items" }), dim, small);
            p.rect(l.rows[0], l.hot[1] - 14.0, l.rows[2], 1.0, [CYAN[0], CYAN[1], CYAN[2], 0.25], 0);
            if all.is_empty() {
                let s = format!("Nothing named '{}'", self.inv_query);
                p.text(l.rows[0] + (l.rows[2] - hud::text_width(&s)) / 2.0, l.rows[1] + l.rows[3] / 2.0 - 10.0, &s, dim);
            }
        } else {
            self.ape_draw(p, l, season);
            let worn = self.inv.get(items::WORN).copied().flatten().and_then(|s| ITEMS.get(s.item));
            match worn {
                Some(d) => head(p, l.rows[0], l.rows[1], &format!("{} - {} slots", d.name, d.pack)),
                None => {
                    head(p, l.rows[0], l.rows[1], "Pack");
                    let s = "No pack worn";
                    p.text(l.rows[0] + (l.rows[2] - hud::text_width(s)) / 2.0, l.rows[1] + l.rows[3] / 2.0 - 10.0, s, dim);
                    p.text_px(l.rows[0] + (l.rows[2] - 41.0 * hud::ADV * small) / 2.0, l.rows[1] + l.rows[3] / 2.0 + 18.0, "a basket or a pack in the Pack slot opens rows", [0.55, 0.62, 0.7, 0.6], small);
                }
            }
            head(p, l.hot[0], l.hot[1], "Hotbar");
            head(p, l.ground[0], l.ground[1], "On the ground");
            if self.pile_near().is_none() {
                p.text_px(l.ground[0], l.ground[1] + 4.0, "Nothing near", [0.55, 0.62, 0.7, 0.6], small);
            }
            // the worn slot's heading, and health and hunger beside it
            let wb = l.spots.iter().find(|s| (s.0, s.1 as usize) == (Holder::Ape, items::WORN)).map(|s| s.2).unwrap_or_default();
            head(p, wb[0], wb[1], "Pack");
            let max = engine::vitals::MAX;
            p.text_px(wb[0] + SIZE + 14.0, wb[1] + 8.0, &format!("Health {}/{max}", self.vitals.0), hud::HEALTH, small);
            p.text_px(wb[0] + SIZE + 14.0, wb[1] + 30.0, &format!("Hunger {}/{max}", self.vitals.1), hud::HUNGER, small);
        }
        let mut hover = None;
        for &(h, i, b) in &l.spots {
            let lit = inside(b, pointer);
            let on = (h, i as usize) == (Holder::Ape, self.slot);
            p.rect(b[0], b[1], b[2], b[3], if lit { [0.17, 0.24, 0.33, 1.0] } else { [0.07, 0.1, 0.15, 0.9] }, 0);
            p.rect(b[0] + 1.0, b[1] + 1.0, b[2] - 2.0, 1.0, [1.0, 1.0, 1.0, 0.07], 0);
            edge(p, b, if on { CYAN } else if lit { [CYAN[0], CYAN[1], CYAN[2], 0.7] } else { [1.0, 1.0, 1.0, 0.14] });
            if on {
                hud::brackets(p, [b[0] - 4.0, b[1] - 4.0, b[2] + 8.0, b[3] + 8.0], 10.0, false, CYAN);
            }
            if h == Holder::Ape && (i as usize) < items::HOTBAR {
                p.text_px(b[0] + 4.0, b[1] + 3.0, &(i + 1).to_string(), if on { CYAN } else { [0.62, 0.72, 0.82, 0.6] }, hud::PX * 0.5);
            }
            let mut st = self.slot_at(h, i);
            if let (Some((fh, fi, n)), Some(s)) = (self.lifted, st.as_mut()) {
                if (fh, fi) == (h, i) && h != Holder::Grid {
                    s.n = s.n.saturating_sub(n);
                }
            }
            if let Some(s) = st.filter(|s| s.n > 0) {
                self.icon(p, s, b[0] + 6.0, b[1] + 6.0, 40.0, season, h != Holder::Grid);
                if lit {
                    hover = Some((h, s, b));
                }
            }
        }
        if let Some((fh, fi, n)) = self.lifted {
            if let Some(s) = self.slot_at(fh, fi) {
                self.icon(p, Stack { n, ..s }, pointer.0 - 20.0, pointer.1 - 20.0, 40.0, season, true);
            }
        } else if let Some((h, s, b)) = hover {
            let name = ITEMS.name(s.item);
            let more = tip(h, s);
            let w = hud::text_width(name).max(more.as_ref().map_or(0.0, |m| m.chars().count() as f32 * hud::ADV * small)) + 20.0;
            let th = hud::LINE + 8.0 + if more.is_some() { 16.0 } else { 0.0 };
            let (x, y) = tip_place(b, w, th, l.panel);
            hud::glass(p, x, y, w, th);
            p.text(x + 10.0, y + 5.0, name, [0.97, 0.97, 0.97, 1.0]);
            if let Some(m) = more {
                p.text_px(x + 10.0, y + hud::LINE + 4.0, &m, dim, small);
            }
        }
        let hint = if self.creative { HINT_CREATIVE } else { HINT_SURVIVAL };
        p.text_px(l.hint[0], l.hint[1], hint, [0.62, 0.72, 0.82, 0.8], HINT_PX);
    }

    /// Survival's ape on its dark inset (ADR 0113): its picture standing and facing out, a worn pack's colour
    /// standing out behind its shoulders, the item in hand at its right hand.
    fn ape_draw(&self, p: &mut hud::Paint, l: &Layout, season: (f64, f64)) {
        let a = l.ape;
        p.rect(a[0], a[1], a[2], a[3], [0.0, 0.01, 0.03, 0.55], 0);
        edge(p, a, [1.0, 1.0, 1.0, 0.1]);
        // a floor under the feet: a soft shadow
        let (cx, floor) = (a[0] + a[2] / 2.0, a[1] + a[3] - 14.0);
        for (w, al) in [(110.0, 0.18), (80.0, 0.22), (52.0, 0.28)] {
            p.rect(cx - w / 2.0, floor - 3.0, w, 6.0, [0.0, 0.0, 0.0, al], 0);
        }
        let Some(fig) = self.models.picture(crate::models::FIGURE, 1) else { return };
        let size = crate::models::FIGURE_PX as f32;
        let (x, y) = (cx - size / 2.0, floor - size + 4.0);
        // shortcut: the pack is a block of its look's colour behind the shoulders, placed by the ape's proportions;
        // a model of each pack, worn, when packs get their own
        let pack = self.inv.get(items::WORN).copied().flatten().and_then(|s| Some((BLOCKS.get(ITEMS.look(s.item))?, ITEMS.get(s.item)?.pack)));
        let pack = pack.map(|(d, n)| (crate::gpu::shown(d, season).0, n as f32 / COLS as f32));
        if let Some((c, rows)) = pack {
            // the pack behind it, larger the more rows it opens, standing out over and beside the shoulders
            // wider than the shoulders (the body is half the picture wide) and over them, so it shows from the front
            let (pw, top) = (size * (0.6 + 0.04 * rows), y + size * (0.25 - 0.02 * rows));
            p.rect(cx - pw / 2.0, top, pw, size * 0.34, [c[0] * 0.75, c[1] * 0.75, c[2] * 0.75, 1.0], 0);
            p.rect(cx - pw / 2.0, top, pw, 4.0, [c[0], c[1], c[2], 1.0], 0);
        }
        fig.draw(&mut |x, y, w, h, c| p.rect(x, y, w, h, c, 0), x, y, size);
        if let Some((c, _)) = pack {
            // its two straps over the chest
            for sx in [0.4, 0.57] {
                p.rect(x + size * sx, y + size * 0.31, 6.0, size * 0.22, [c[0] * 0.55, c[1] * 0.55, c[2] * 0.55, 1.0], 0);
            }
        }
        if let Some(s) = self.inv.get(self.slot).copied().flatten() {
            self.icon(p, Stack { n: 1, ..s }, x + size * 0.12, y + size * 0.55, 34.0, season, false);
        }
    }

    /// A stack's look in a square of `size` at (x, y): its model's picture when it has one by its name (ADR
    /// 0113), else the cube of its block (an item: of the block it looks like, with a mark of the accent in its
    /// corner); its count over one when `count`, or a tool's wear.
    fn icon(&self, p: &mut hud::Paint, s: Stack, x: f32, y: f32, size: f32, season: (f64, f64), count: bool) {
        self.look_of(p, s.item, x, y, size, season);
        if let Some(left) = crate::play::wear(s).filter(|_| count) {
            hud::wear_bar(p, x + 2.0, y + size, size - 4.0, left);
        } else if count && s.n > 1 {
            let t = s.n.to_string();
            let px = hud::PX * 0.75;
            p.text_px(x + size + 2.0 - t.len() as f32 * hud::ADV * px, y + size - 8.0, &t, [1.0; 4], px);
        }
    }

    /// `item`'s look in a square of `size` at (x, y), for the inventory and the hotbar alike (`look`).
    pub(crate) fn look_of(&self, p: &mut hud::Paint, item: u16, x: f32, y: f32, size: f32, season: (f64, f64)) {
        let block = (item < items::ITEM_BASE).then(|| BLOCKS.get(item)).flatten();
        match look(item, block, &|n| self.models.picture(n, 0).is_some()) {
            Look::Model(name) => {
                // 3 px in from the square's edge: a picture is fitted to its own edges (flint fills it)
                if let Some(pic) = self.models.picture(name, 0) {
                    pic.draw(&mut |x, y, w, h, c| p.rect(x, y, w, h, c, 0), x + 3.0, y + 3.0, size - 6.0);
                }
            }
            Look::Cube => {
                let Some(d) = BLOCKS.get(ITEMS.look(item)) else { return };
                let (colour, side, pattern) = crate::gpu::shown(d, season);
                let pt = crate::gpu::painting(d);
                let k = size * 0.8;
                hud::cube(&mut |x, y, w, h, c, g| p.rect(x, y, w, h, c, g), x + (size - k) / 2.0, y + (size - k) / 2.0, k, colour, side, pattern, 1.0, [pt[0].as_ref(), pt[1].as_ref()]);
                if item >= items::ITEM_BASE {
                    p.rect(x - 2.0, y - 2.0, 7.0, 7.0, CYAN, 0);
                }
            }
        }
    }
}

/// How a slot shows a block or item (ADR 0113): as it looks in the world.
#[derive(Debug, PartialEq)]
enum Look {
    /// The picture of the model of this name: an item's own (by its name), a block's `model`.
    Model(&'static str),
    /// The cube of its block, or of the block an item looks like; a cutout's (leaves) with its painting's holes.
    Cube,
}

/// The look of `item` (`block` its def when it is a block, `has` whether a model of a name has a picture).
fn look(item: u16, block: Option<&'static engine::blocks::Def>, has: &dyn Fn(&str) -> bool) -> Look {
    let name: &'static str = ITEMS.name(item);
    if has(name) {
        return Look::Model(name);
    }
    let Some(d) = block else { return Look::Cube };
    match d.model.as_deref().filter(|m| has(m)) {
        Some(m) => Look::Model(m),
        None => Look::Cube,
    }
}

#[test]
fn a_slot_shows_a_block_as_the_world_does() {
    let leaves = BLOCKS.get(ITEMS.named("broadleaf leaves").unwrap()).unwrap();
    let granite = BLOCKS.get(ITEMS.named("granite").unwrap()).unwrap();
    let none = |_: &str| false;
    // ADR 0114 second round: a cutout block is a cube in the world, so its cube (hud::cube leaves its holes open)
    assert_eq!(look(leaves.id, Some(leaves), &none), Look::Cube, "a cutout block: its cube, holes and all");
    assert_eq!(look(granite.id, Some(granite), &none), Look::Cube, "a solid block keeps its cube");
    let axe = ITEMS.named("hand axe").unwrap();
    assert_eq!(look(axe, None, &|n| n == "hand axe"), Look::Model("hand axe"));
    assert_eq!(look(axe, None, &none), Look::Cube, "no picture yet: the cube of its look");
}

/// A one pixel edge round a box.
fn edge(p: &mut hud::Paint, b: [f32; 4], c: [f32; 4]) {
    p.rect(b[0], b[1], b[2], 1.0, c, 0);
    p.rect(b[0], b[1] + b[3] - 1.0, b[2], 1.0, c, 0);
    p.rect(b[0], b[1] + 1.0, 1.0, b[3] - 2.0, c, 0);
    p.rect(b[0] + b[2] - 1.0, b[1] + 1.0, 1.0, b[3] - 2.0, c, 0);
}

/// The tooltip's second line for stack `s` in a slot of `h`, when there is something to say: a tool's uses left,
/// what a pack opens worn, a stack's count against its size (creative's grid: the size).
fn tip(h: Holder, s: Stack) -> Option<String> {
    if let (Some(t), Some(left)) = (items::tool(Some(s)), items::left(Some(s))) {
        return Some(format!("{left} of {} uses", t.uses));
    }
    if let Some(d) = ITEMS.get(s.item).filter(|d| d.pack > 0) {
        return Some(format!("worn: opens {} slots", d.pack));
    }
    let stack = ITEMS.stack(s.item);
    match h {
        _ if stack <= 1 => None,
        Holder::Grid => Some(format!("stacks to {stack}")),
        _ => Some(format!("{} of {stack}", s.n)),
    }
}

/// ADR 0114 second round: a tooltip of `w` x `h` beside slot `b`, never over it nor over a heading (which stand
/// over a section's first row): right of the slot, its middle at the slot's, else left of it at the panel's right
/// edge; held inside `panel`.
#[test]
fn a_tooltip_stands_beside_its_slot_inside_the_panel() {
    let panel = [100.0, 50.0, 800.0, 500.0];
    let over = |a: [f32; 4], b: [f32; 4]| a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3];
    for b in [[120.0, 70.0, 52.0, 52.0], [500.0, 300.0, 52.0, 52.0], [828.0, 300.0, 52.0, 52.0], [828.0, 478.0, 52.0, 52.0], [120.0, 478.0, 52.0, 52.0]] {
        let (w, h) = (180.0, 44.0);
        let (x, y) = tip_place(b, w, h, panel);
        let t = [x, y, w, h];
        assert!(!over(t, b), "{t:?} covers its slot {b:?}");
        assert!(x >= panel[0] && x + w <= panel[0] + panel[2] && y >= panel[1] && y + h <= panel[1] + panel[3], "{t:?} outside {panel:?}");
        assert!(y >= b[1] - 4.0 && y + h <= b[1] + b[3] + 4.0, "in its slot's band, under any heading: {t:?} {b:?}");
    }
    // the review: a wide tip (a long second line) on the hotbar's middle and edge columns, at the layout test's
    // sizes, fits neither beside; it stands above (or below) its slot, inside the window, never over the slot
    for (lw, lh) in [(640.0 / 3.0, 120.0), (1280.0 / 1.5, 480.0), (1280.0, 720.0)] {
        for creative in [true, false] {
            let l = layout(lw, lh, creative, &[], 0, items::ROWS_AT, None);
            let hot: Vec<[f32; 4]> = l.spots.iter().filter(|s| s.0 == Holder::Ape && (s.1 as usize) < items::HOTBAR).map(|s| s.2).collect();
            for b in [hot[0], hot[4], hot[8]] {
                let (w, h) = (300.0, 44.0);
                let (x, y) = tip_place(b, w, h, l.panel);
                let t = [x, y, w, h];
                let p = l.panel;
                assert!(!over(t, b), "{lw}x{lh}: {t:?} covers its slot {b:?}");
                assert!(x >= p[0] && x + w <= p[0] + p[2] && y >= p[1] && y + h <= p[1] + p[3], "{lw}x{lh}: {t:?} outside {p:?}");
            }
        }
    }
}

/// Where the tooltip of `w` x `h` for slot `b` stands (`a_tooltip_stands_beside_its_slot_inside_the_panel`): right
/// of the slot, its middle at the slot's, else left of it; too wide for either, above it (below when there is no
/// room above), its middle over the slot's; held inside `panel`.
fn tip_place(b: [f32; 4], w: f32, h: f32, panel: [f32; 4]) -> (f32, f32) {
    let (right, left) = (b[0] + b[2] + 6.0, b[0] - 6.0 - w);
    let y = (b[1] + (b[3] - h) / 2.0).clamp(panel[1] + 4.0, panel[1] + panel[3] - h - 4.0);
    if right + w <= panel[0] + panel[2] - 4.0 {
        return (right, y);
    }
    if left >= panel[0] + 4.0 {
        return (left, y);
    }
    let x = (b[0] + (b[2] - w) / 2.0).clamp(panel[0] + 4.0, (panel[0] + panel[2] - w - 4.0).max(panel[0] + 4.0));
    let above = b[1] - 6.0 - h;
    (x, if above >= panel[1] + 4.0 { above } else { b[1] + b[3] + 6.0 })
}

#[test]
fn a_tooltip_says_a_tools_uses_and_what_a_pack_opens() {
    let axe = ITEMS.named("hand axe").unwrap();
    let pack = ITEMS.named("frame pack").unwrap();
    let granite = ITEMS.named("granite").unwrap();
    assert_eq!(tip(Holder::Ape, Stack { payload: 6, ..Stack::new(axe, 1) }).as_deref(), Some("34 of 40 uses"));
    assert_eq!(tip(Holder::Ape, Stack::new(pack, 1)).as_deref(), Some("worn: opens 27 slots"));
    assert_eq!(tip(Holder::Ape, Stack::new(granite, 12)), Some(format!("12 of {}", ITEMS.stack(granite))));
    assert_eq!(tip(Holder::Grid, Stack::new(granite, 64)), Some(format!("stacks to {}", ITEMS.stack(granite))));
}

use hud::CYAN;
