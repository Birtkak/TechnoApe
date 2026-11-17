//! The inventory screens (ADR 0075), Minecraft's: the inventory key (E) opens the ape's slots. Survival shows
//! the hotbar, the worn pack's slot and the rows it opens, and the pile at the feet; creative shows every block
//! and item in a grid with tabs and a search over the hotbar. A click lifts a stack (a right click half of it)
//! and the next puts it down (a right click one of it); shift-click sends it across; a click outside the
//! panel throws it. Every move is one `ToServer::Items`, predicted here with the server's rule (ADR 0074).
use super::*;
use engine::items::{self, Holder, Slot, Stack, CRAFT, CRAFT_AT, ITEMS, TABS};
use engine::recipes::{Recipe, RECIPES};

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
/// Room between the inventory's columns.
const SEP: f32 = 20.0;
/// Survival's column of the ape: its picture's inset.
const APE_W: f32 = 196.0;
const APE_H: f32 = 196.0;
/// Creative's search field on the title row.
const SEARCH_W: f32 = 220.0;
/// The hints' text size, and each screen's (short enough to fit its window).
const HINT_PX: f32 = hud::PX * 0.75;
const HINT_SURVIVAL: &str = "Click: lift, put    Right click: half, one    Shift-click: across, take all made    Click outside: throw";
const HINT_BLOCK: &str = "Click: lift  Right: half  Shift: across  Outside: throw";
/// The crafting grid's room with its arrow and output: the workbench's 3 x 3 (also a furnace's top part's
/// width), the inventory's 2 x 2.
const CRAFT_W: f32 = 3.0 * (SIZE + GAP) - GAP + 34.0 + SIZE;
const CRAFT2_W: f32 = 2.0 * (SIZE + GAP) - GAP + 34.0 + SIZE;
/// The recipe viewer, docked at the screen's right edge: its items' squares, how many a row and how many rows
/// in view (the wheel scrolls them), the squares of the recipe's pattern, the lines of text under it.
const VSIZE: f32 = 40.0;
const VGAP: f32 = 4.0;
pub(crate) const VCOLS: usize = 5;
pub(crate) const VROWS: usize = 6;
const PSIZE: f32 = 36.0;
const VPAD: f32 = 16.0;
const DOCK_W: f32 = VCOLS as f32 * (VSIZE + VGAP) - VGAP + 2.0 * VPAD;
const TEXT_LINES: usize = 5;
const TEXT_LH: f32 = 18.0;
const HINT_CREATIVE: &str = "Click: a stack  Right: one  Shift: to hotbar  Wheel: more";

/// Which window is open: creative's grid, survival's own inventory, or a block's window (the developer,
/// 2026-11-17: a block's window apart from the inventory). A block's window is its own top part over the pack's
/// rows and the hotbar; a new kind adds a variant, its arm in `top`, and its drawing in `App::top_draw`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Screen {
    Creative,
    Inventory,
    Bench,
    /// A furnace (ADR 0141): its block and how many casts it offers.
    Furnace(Block, usize),
}

/// Where the screen's parts lie, in UI pixels: each slot with its holder and index (creative's grid: the
/// item's id), the tabs (creative; the last the search field), and the panel; a click outside the panel and the
/// viewer throws.
#[derive(Default, Clone)]
pub(crate) struct Layout {
    pub spots: Vec<(Holder, u16, [f32; 4])>,
    pub tabs: Vec<[f32; 4]>,
    pub panel: [f32; 4],
    /// The line of hints under the slots, as wide as its text.
    pub hint: [f32; 4],
    /// The inventory: the ape's picture.
    pub ape: [f32; 4],
    /// Survival: the pack's rows (three rows' room); creative: the grid and its scrollbar.
    pub rows: [f32; 4],
    /// The inventory: the pile's room. All: the hotbar's row.
    pub ground: [f32; 4],
    pub hot: [f32; 4],
    /// The crafting grid's room (its cells are spots of the ape's `items::CRAFT_AT` on) and the output's square;
    /// a furnace's top part.
    pub craft: [f32; 4],
    pub out: [f32; 4],
    /// The recipe viewer, docked right (none in creative): its room, its search field, each item in view with its
    /// square, the room they fill, the recipe's pattern (3 x 3 squares), the Fill button (none at a furnace, which
    /// has no grid) and the lines of text under them.
    pub dock: [f32; 4],
    pub search: [f32; 4],
    pub list: Vec<(u16, [f32; 4])>,
    pub view: [f32; 4],
    pub pattern: [f32; 4],
    pub fill: [f32; 4],
    pub text: [f32; 4],
    /// A furnace's window (ADR 0141), in its top part: its heat bar and the casts to choose from (its four slots
    /// are spots of `Holder::Furnace`, the output's at `out`).
    pub heat: [f32; 4],
    pub casts: Vec<[f32; 4]>,
    /// UI pixels a pixel of this layout: below 1 where the window would not fit the UI, so the whole screen is
    /// drawn smaller (every part above is in the layout's pixels; the pointer is divided by it).
    pub k: f32,
}

fn inside(b: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]
}

/// A block window's own part at (x, y), its slots and boxes put in `l`; its width and height.
fn top(screen: Screen, x: f32, y: f32, l: &mut Layout) -> (f32, f32) {
    let step = SIZE + GAP;
    let y0 = y + HEAD;
    match screen {
        Screen::Bench => {
            l.craft = [x, y0, 3.0 * step - GAP, 3.0 * step - GAP];
            for k in 0..items::CRAFT {
                l.spots.push((Holder::Ape, (items::CRAFT_AT + k) as u16, [x + (k % 3) as f32 * step, y0 + (k / 3) as f32 * step, SIZE, SIZE]));
            }
            l.out = [x + 3.0 * step - GAP + 34.0, y0 + (l.craft[3] - SIZE) / 2.0, SIZE, SIZE];
            (CRAFT_W, HEAD + l.craft[3])
        }
        Screen::Furnace(b, casts) => {
            use engine::furnace::{FUEL, IN, IN2, MOULD, OUT};
            l.craft = [x, y0, CRAFT_W, 3.0 * step - GAP];
            l.out = [x + CRAFT_W - SIZE, y0, SIZE, SIZE];
            // the two inputs side by side, the arrow to the out slot; the fuel under them, the mould under the out
            for (k, at) in [(IN, [x, y0]), (IN2, [x + step, y0]), (MOULD, [l.out[0], y0 + 2.0 * step]), (FUEL, [x, y0 + 2.0 * step]), (OUT, [l.out[0], l.out[1]])] {
                l.spots.push((Holder::Furnace(b), k as u16, [at[0], at[1], SIZE, SIZE]));
            }
            l.heat = [x, y0 + 3.0 * step - GAP + 14.0, CRAFT_W, 10.0];
            l.casts = (0..casts).map(|k| [x + k as f32 * (VSIZE + VGAP), l.heat[1] + 30.0, VSIZE, VSIZE]).collect();
            // and the chosen cast's line under them
            (CRAFT_W, l.heat[1] + 30.0 + VSIZE + 20.0 - y)
        }
        Screen::Creative | Screen::Inventory => (0.0, 0.0),
    }
}

/// The screen on a `lw` x `lh` UI. Creative: one window in the middle: the title and the search field, the
/// tabs, `grid`'s items from row `scroll` on with a scrollbar, the hotbar under a divider. Otherwise the recipe
/// viewer docked at the right edge (its search over `grid`'s items from row `scroll` on, the recipe shown under
/// them) and the window in the room left of it: the inventory, the ape over the worn slot | the rows the worn
/// pack opens (`open`, `items::open`) over the hotbar | a pile's 4 slots over the 2 x 2 and its output; a block's
/// window, its own part (`top`) over the pack's rows and the hotbar.
pub(crate) fn layout(lw: f32, lh: f32, screen: Screen, grid: &[u16], scroll: usize, open: usize, pile: Option<Block>) -> Layout {
    let step = SIZE + GAP;
    let wide = COLS as f32 * step - GAP;
    let vstep = VSIZE + VGAP;
    let mut l = Layout::default();
    // the pack's rows (three rows' room) over the hotbar
    let pack_h = HEAD + 3.0 * step - GAP + 20.0 + HEAD + SIZE;
    let (tw, th) = top(screen, 0.0, 0.0, &mut Layout::default());
    // the content's width and height under the title row (the hint's line under that)
    let (iw, ih) = match screen {
        Screen::Creative => (wide + 14.0, 30.0 + 12.0 + GRID_ROWS as f32 * step - GAP + 40.0 + SIZE),
        Screen::Inventory => (APE_W + SEP + wide + SEP + CRAFT2_W, pack_h),
        _ => (wide, th + 8.0 + pack_h),
    };
    let (pw, ph) = (iw + 2.0 * PAD, TITLE_H + ih + 18.0 + 8.0 * HINT_PX + 18.0);
    // the viewer's width and the height its parts need
    let pw3 = 3.0 * (PSIZE + VGAP) - VGAP;
    let (dw, dh) = if screen == Screen::Creative { (0.0, 0.0) } else { (DOCK_W, TITLE_H + 38.0 + VROWS as f32 * vstep - VGAP + 20.0 + HEAD + pw3 + 10.0 + TEXT_LINES as f32 * TEXT_LH + 16.0) };
    // a UI too small for the window (a small window at a large interface scale) draws all of it smaller, so every
    // slot stays on the screen and takes its click
    l.k = (lw / (pw + 16.0 + dw)).min(lh / (ph.max(dh) + 16.0)).min(1.0);
    let (lw, lh) = (lw / l.k, lh / l.k);
    // the window in the middle of the room left of the viewer
    let (left, top_y) = (((lw - dw - pw) / 2.0).floor(), ((lh - ph) / 2.0).max(8.0).floor());
    l.panel = [left, top_y, pw, ph];
    let (x0, ct) = (left + PAD, top_y + TITLE_H);
    let (hx, hot_y);
    match screen {
        Screen::Creative => {
            let tw = ((iw + GAP) / TABS.len() as f32 - GAP).floor();
            l.tabs = (0..TABS.len()).map(|k| [x0 + k as f32 * (tw + GAP), ct, tw, 30.0]).collect();
            l.tabs.push([left + pw - PAD - SEARCH_W, top_y + 10.0, SEARCH_W, 30.0]);
            let gy = ct + 42.0;
            l.rows = [x0, gy, iw, GRID_ROWS as f32 * step - GAP];
            for (k, id) in grid.iter().skip(scroll * COLS).take(GRID_ROWS * COLS).enumerate() {
                l.spots.push((Holder::Grid, *id, [x0 + (k % COLS) as f32 * step, gy + (k / COLS) as f32 * step, SIZE, SIZE]));
            }
            (hx, hot_y) = (x0, gy + l.rows[3] + 40.0);
        }
        Screen::Inventory => {
            l.ape = [x0, ct, APE_W, APE_H];
            let mx = x0 + APE_W + SEP;
            l.rows = [mx, ct + HEAD, wide, 3.0 * step - GAP];
            (hx, hot_y) = (mx, l.rows[1] + l.rows[3] + 20.0 + HEAD);
            l.spots.push((Holder::Ape, items::WORN as u16, [x0, hot_y, SIZE, SIZE]));
            // the pile over the 2 x 2
            let cx = mx + wide + SEP;
            l.ground = [cx, ct + HEAD, 2.0 * step - GAP, 2.0 * step - GAP];
            if let Some(b) = pile {
                for k in 0..items::PILE_SLOTS {
                    l.spots.push((Holder::Pile(b), k as u16, [l.ground[0] + (k % 2) as f32 * step, l.ground[1] + (k / 2) as f32 * step, SIZE, SIZE]));
                }
            }
            l.craft = [cx, l.ground[1] + l.ground[3] + 20.0 + HEAD, 2.0 * step - GAP, 2.0 * step - GAP];
            for (n, k) in (0..items::CRAFT).filter(|&k| items::in_two(k)).enumerate() {
                l.spots.push((Holder::Ape, (items::CRAFT_AT + k) as u16, [cx + (n % 2) as f32 * step, l.craft[1] + (n / 2) as f32 * step, SIZE, SIZE]));
            }
            l.out = [cx + 2.0 * step - GAP + 34.0, l.craft[1] + (l.craft[3] - SIZE) / 2.0, SIZE, SIZE];
        }
        _ => {
            top(screen, x0 + ((wide - tw) / 2.0).floor(), ct, &mut l);
            l.rows = [x0, ct + th + 8.0 + HEAD, wide, 3.0 * step - GAP];
            (hx, hot_y) = (x0, l.rows[1] + l.rows[3] + 20.0 + HEAD);
        }
    }
    if screen != Screen::Creative {
        for k in 0..open.saturating_sub(items::ROWS_AT) {
            l.spots.push((Holder::Ape, (items::ROWS_AT + k) as u16, [l.rows[0] + (k % COLS) as f32 * step, l.rows[1] + (k / COLS) as f32 * step, SIZE, SIZE]));
        }
        // the viewer: the title, the search, the items in view, the recipe's pattern with Fill beside it, its lines
        l.dock = [lw - dw, 0.0, dw, lh];
        let (vx, vw) = (lw - dw + VPAD, dw - 2.0 * VPAD);
        l.search = [vx, TITLE_H, vw, 30.0];
        l.view = [vx, TITLE_H + 38.0, vw, VROWS as f32 * vstep - VGAP];
        for (k, &id) in grid.iter().skip(scroll * VCOLS).take(VROWS * VCOLS).enumerate() {
            l.list.push((id, [vx + (k % VCOLS) as f32 * vstep, l.view[1] + (k / VCOLS) as f32 * vstep, VSIZE, VSIZE]));
        }
        l.pattern = [vx, l.view[1] + l.view[3] + 20.0 + HEAD, pw3, pw3];
        if !matches!(screen, Screen::Furnace(..)) {
            l.fill = [vx + pw3 + 12.0, l.pattern[1] + pw3 - 30.0, vw - pw3 - 12.0, 30.0];
        }
        l.text = [vx, l.pattern[1] + pw3 + 10.0, vw, TEXT_LINES as f32 * TEXT_LH];
    }
    l.hot = [hx, hot_y, wide, SIZE];
    for k in 0..items::HOTBAR {
        l.spots.push((Holder::Ape, k as u16, [hx + k as f32 * step, hot_y, SIZE, SIZE]));
    }
    let hint = hint_of(screen);
    l.hint = [x0, ct + ih + 18.0, hint.chars().count() as f32 * hud::ADV * HINT_PX, 8.0 * HINT_PX];
    l
}

fn hint_of(screen: Screen) -> &'static str {
    match screen {
        Screen::Creative => HINT_CREATIVE,
        Screen::Inventory => HINT_SURVIVAL,
        _ => HINT_BLOCK,
    }
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
        // creative, the inventory bare and with a pack and a pile, a workbench's window and a crucible's (ADR 0141):
        // its five slots, the heat's bar and five casts
        for (screen, open, pile) in [(Screen::Creative, items::ROWS_AT, None), (Screen::Inventory, items::ROWS_AT, None), (Screen::Inventory, items::APE_SLOTS, Some(pile)), (Screen::Bench, items::APE_SLOTS, Some(pile)), (Screen::Furnace(pile, 5), items::APE_SLOTS, Some(pile))] {
            let l = layout(lw, lh, screen, &grid, 0, open, pile);
            let at = format!("{lw}x{lh} {screen:?}");
            let creative = screen == Screen::Creative;
            let furnace = matches!(screen, Screen::Furnace(..));
            let block = matches!(screen, Screen::Bench | Screen::Furnace(..));
            let (sw, sh) = (lw / l.k, lh / l.k);
            // the usual sizes need no shrinking
            if lw >= 1280.0 {
                assert_eq!(l.k, 1.0, "{at}: drawn smaller");
            }
            assert_eq!(l.list.len(), if creative { 0 } else { grid.len().min(VCOLS * VROWS) }, "{at}");
            assert_eq!(l.casts.len(), if furnace { 5 } else { 0 });
            // the window's own boxes: the crafting output or the heat's bar, the casts; in the panel, apart, over no slot
            let own: Vec<[f32; 4]> = if creative { vec![] } else { std::iter::once(if furnace { l.heat } else { l.out }).chain(l.casts.iter().copied()).collect() };
            for (k, b) in own.iter().enumerate() {
                assert!(within(l.panel, *b), "{at}: box {k} {b:?} out of the panel");
                assert!(own[..k].iter().all(|o| !over(*o, *b)) && l.spots.iter().all(|s| !over(s.2, *b)), "{at}: box {k} over another");
            }
            // the viewer: docked at the right edge, all the screen's height, beside the window and never over it; its
            // search, items, pattern, Fill (none at a furnace) and text inside it and apart
            if creative {
                assert_eq!(l.dock, [0.0; 4]);
            } else {
                assert_eq!((l.dock[0] + l.dock[2], l.dock[1], l.dock[3]), (sw, 0.0, sh), "{at}: the viewer is not docked right");
                assert!(!over(l.dock, l.panel) && l.panel[0] + l.panel[2] <= l.dock[0], "{at}: the window under the viewer");
                assert_eq!(l.fill[2] > 0.0, !furnace, "{at}: Fill");
                let boxes: Vec<[f32; 4]> = [l.search, l.pattern, l.text].into_iter().chain((!furnace).then_some(l.fill)).chain(l.list.iter().map(|v| v.1)).collect();
                for (k, b) in boxes.iter().enumerate() {
                    assert!(within(l.dock, *b), "{at}: viewer box {k} {b:?} out of the viewer");
                    assert!(boxes[..k].iter().all(|o| !over(*o, *b)), "{at}: viewer box {k} over another");
                }
                // the window in the middle of the room left of the viewer
                assert!((l.panel[0] - (l.dock[0] - l.panel[0] - l.panel[2])).abs() <= 2.0, "{at}: the window not centred left of the viewer");
            }
            // which slots each window has: the inventory's own, a block window's top part over the pack and hotbar
            let has = |f: &dyn Fn(&(Holder, u16, [f32; 4])) -> bool| l.spots.iter().any(f);
            assert_eq!(has(&|s| (s.0, s.1 as usize) == (Holder::Ape, items::WORN)), screen == Screen::Inventory, "{at}: the worn slot");
            assert_eq!(has(&|s| matches!(s.0, Holder::Pile(_))), screen == Screen::Inventory && pile.is_some(), "{at}: the pile");
            assert_eq!(has(&|s| matches!(s.0, Holder::Furnace(_))), furnace, "{at}: a furnace's slots");
            assert_eq!(l.ape[2] > 0.0, screen == Screen::Inventory, "{at}: the ape");
            if block {
                assert!(l.craft[1] + l.craft[3] <= l.rows[1] - HEAD, "{at}: the block's part over the pack's rows");
            }
            let cells = match screen {
                Screen::Furnace(..) => engine::furnace::SLOTS,
                Screen::Bench => 9,
                Screen::Inventory => 4,
                Screen::Creative => 0,
            };
            let want = match screen {
                Screen::Creative => grid.len().min(GRID_ROWS * COLS) + 9,
                Screen::Inventory => 9 + 1 + (open - items::ROWS_AT) + pile.map_or(0, |_| 4) + cells,
                _ => 9 + (open - items::ROWS_AT) + cells,
            };
            assert_eq!(l.spots.len(), want, "{at}");
            // everything that takes a click on the screen, in UI pixels
            let shown = |b: [f32; 4]| b.map(|v| v * l.k);
            assert!(l.k > 0.0 && l.k <= 1.0, "{at}: scale {}", l.k);
            for b in [l.panel, l.dock].into_iter().filter(|b| b[2] > 0.0).chain(l.spots.iter().map(|s| s.2)).chain(l.tabs.iter().copied()) {
                assert!(within([0.0, 0.0, lw, lh], shown(b)), "{at}: {b:?} off the screen at scale {}", l.k);
            }
            for (k, a) in l.spots.iter().enumerate() {
                let b = a.2;
                assert!(within(l.panel, b), "{at}: slot {k} out of the panel");
                assert!(l.spots[..k].iter().all(|o| !over(o.2, b)), "{at}: slot {k} over another");
                assert!(!over(l.ape, b), "{at}: slot {k} over the ape");
                // a click on its middle finds this slot
                let p = (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0);
                assert_eq!(l.spots.iter().find(|s| inside(s.2, p)).map(|s| (s.0, s.1)), Some((a.0, a.1)), "{at}: slot {k} misses its click");
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
            if screen == Screen::Inventory {
                assert!(l.ape[2] >= 150.0 && within(l.panel, l.ape), "{at}: the ape's picture");
            }
        }
    }
}

/// `s` in lines of at most `cols` characters, broken between words (a word longer than a line cut).
fn wrap(s: &str, cols: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for w in s.split_whitespace() {
        let w: String = w.chars().take(cols).collect();
        match lines.last_mut() {
            Some(l) if l.chars().count() + 1 + w.chars().count() <= cols => {
                l.push(' ');
                l.push_str(&w);
            }
            _ => lines.push(w),
        }
    }
    lines
}

#[test]
fn the_viewers_text_wraps_between_words() {
    assert_eq!(wrap("no recipe: knapped from flint on rock", 16), ["no recipe:", "knapped from", "flint on rock"]);
    assert_eq!(wrap("makes 1", 24), ["makes 1"]);
    assert_eq!(wrap("abcdefgh", 4), ["abcd"]);
}

/// The viewer's cells for a furnace's work, left to right: its input, an alloy's second input beside it, the
/// mould last (an empty cell where it has none).
fn work_cells(w: &engine::recipes::Work) -> Vec<Option<(u16, u16)>> {
    std::iter::once(Some(w.input)).chain(w.with.map(Some)).chain([w.mould.map(|m| (m, 1))]).collect()
}

#[test]
fn the_viewer_shows_both_of_an_alloys_inputs() {
    let id = |n: &str| ITEMS.named(n).unwrap();
    let bronze = RECIPES.works.iter().find(|w| w.out.0 == id("bronze ingot") && w.mould.is_none()).unwrap();
    assert_eq!(work_cells(bronze), [Some((id("copper ingot"), 3)), Some((id("tin ingot"), 1)), None]);
    let cast = RECIPES.work_making(id("bronze pick head")).unwrap();
    assert_eq!(work_cells(cast), [Some((id("bronze ingot"), 2)), Some((id("clay mould"), 1))]);
}

impl App {
    /// The items creative's grid shows on the open tab: a tab's own, or every one whose name holds the search. On
    /// the crafting screen the recipe viewer's: every one whose name holds the search, those with a recipe first.
    pub(crate) fn grid_items(&self) -> Vec<u16> {
        let q = self.inv_query.to_lowercase();
        let named = ITEMS.all().filter(|&i| ITEMS.name(i).to_lowercase().contains(&q));
        if self.crafting() {
            let mut v: Vec<u16> = named.collect();
            v.sort_by_key(|&i| RECIPES.making(i).is_none() && RECIPES.work_making(i).is_none());
            return v;
        }
        if self.inv_tab < TABS.len() {
            return ITEMS.tabs[self.inv_tab].clone();
        }
        named.collect()
    }

    /// Whether the screen is survival's, with the crafting grid and the viewer: in survival, or at a workbench.
    pub(crate) fn crafting(&self) -> bool {
        !self.creative || self.bench.is_some() || self.furnace.is_some()
    }

    /// The open furnace's row (`recipes::FurnaceDef`) and the casts it offers, each shape once (the first row of
    /// it: the metal in the input slot picks the row that runs), in its window's order.
    fn furnace_def(&self) -> Option<(&'static engine::recipes::FurnaceDef, Vec<&'static engine::recipes::Work>)> {
        let (b, _) = self.furnace.as_ref()?;
        let d = RECIPES.furnace(self.world.block(*b))?;
        Some((d, RECIPES.shapes(d.top)))
    }

    /// The rows the grid or the viewer scrolls by at most.
    pub(crate) fn inv_scroll_most(&self) -> usize {
        let (cols, rows) = if self.crafting() { (VCOLS, VROWS) } else { (COLS, GRID_ROWS) };
        self.grid_items().len().div_ceil(cols).saturating_sub(rows)
    }

    /// The block aimed at whose place button opens a window (ADR 0140): a use, which comes before
    /// a strike or a place against it, alive and within `items::BENCH_REACH` of the feet, as the server's rule.
    pub(crate) fn use_target(&self) -> Option<Block> {
        let (hit, _) = self.aim.filter(|_| self.dead.is_none())?;
        let near = self.world.block_centre(hit).distance(self.me.pos) <= items::BENCH_REACH;
        (near && BLOCKS.get(self.world.block(hit))?.window.is_some()).then_some(hit)
    }

    /// The place button on a block with a window: the server opens it, and its window opens (not the inventory).
    pub(crate) fn use_block(&mut self) {
        let Some(b) = self.use_target() else { return };
        self.link.send(ToServer::Use(b));
        if RECIPES.furnace(self.world.block(b)).is_some() {
            self.furnace = Some((b, Default::default()));
        } else {
            self.bench = Some(b);
        }
        self.lifted = None;
        self.set_menu(Some(Menu::Inventory));
    }

    /// The workbench or furnace open is gone or out of reach: its screen closes (the server puts the cells away too).
    pub(crate) fn bench_left(&self) -> bool {
        self.bench.or(self.furnace.as_ref().map(|f| f.0)).is_some_and(|b| BLOCKS.get(self.world.block(b)).is_none_or(|d| d.window.is_none()) || self.world.block_centre(b).distance(self.me.pos) > items::BENCH_REACH)
    }

    /// Fill (the viewer's button): the recipe shown laid out in the grid from what the slots hold, one make's worth,
    /// through the ordinary moves (`fill_plan`); the cells it found nothing for are drawn red.
    pub(crate) fn fill(&mut self) {
        // a furnace's window has no grid to fill
        let Some(r) = self.viewing.and_then(|i| RECIPES.making(i)).filter(|_| self.furnace.is_none()) else { return };
        let Some((moves, lack)) = fill_plan(&self.inv, r, self.bench.is_some()) else { return };
        for (a, b, n) in moves {
            self.send_items((Holder::Ape, a as u16), (Holder::Ape, b as u16), n);
        }
        self.lack = lack;
    }

    /// The nearest pile in reach of the feet, if any.
    pub(crate) fn pile_near(&self) -> Option<Block> {
        let me = self.me.pos;
        self.piles.keys().map(|b| (self.world.block_centre(*b).distance(me), *b)).filter(|(d, _)| *d <= PILE_SHOWN).min_by(|a, b| a.0.total_cmp(&b.0)).map(|p| p.1)
    }

    /// The window open: a block's (a furnace's, a workbench's), else creative's grid or the inventory.
    pub(crate) fn screen(&self) -> Screen {
        match (&self.furnace, self.bench) {
            (Some(f), _) => Screen::Furnace(f.0, self.furnace_def().map_or(0, |d| d.1.len())),
            (None, Some(_)) => Screen::Bench,
            _ if self.creative => Screen::Creative,
            _ => Screen::Inventory,
        }
    }

    pub(crate) fn inv_layout(&self) -> Layout {
        let (lw, lh) = self.ui_size();
        layout(lw, lh, self.screen(), &self.grid_items(), self.inv_scroll, items::open(&self.inv), self.pile_near())
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
            Holder::Furnace(b) => self.furnace.as_ref().filter(|f| f.0 == b).and_then(|f| f.1.slots.get(i as usize).copied().flatten()),
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
        if self.crafting() {
            // the viewer's search field takes the keys until a click elsewhere
            self.inv_tab = if inside(l.search, p) { SEARCH } else { 0 };
            if let Some(&(id, _)) = l.list.iter().find(|v| inside(v.1, p)) {
                (self.viewing, self.lack) = (Some(id), Vec::new());
                return;
            }
            if inside(l.fill, p) {
                return self.fill();
            }
            // a furnace's cast: chosen, or (clicked again) none
            if let (Some(k), Some((b, f)), Some((_, casts))) = (l.casts.iter().position(|c| inside(*c, p)), self.furnace.as_ref(), self.furnace_def()) {
                let name = casts[k].name.clone();
                let on = f.choice.as_deref().and_then(|c| RECIPES.shape(c)) == casts[k].shape.as_deref();
                let cast = (!on).then_some(name);
                self.link.send(ToServer::Choose { at: *b, cast });
                self.audio.play("ui.tick", None, 1.0, 1.0, false);
                return;
            }
            // the output: one made, with shift as many as the cells hold (the server makes them and sends the slots)
            if inside(l.out, p) && self.furnace.is_none() {
                if let Some(r) = RECIPES.matched(&self.inv, self.bench.is_some()).filter(|_| self.lifted.is_none()) {
                    self.link.send(ToServer::Craft { recipe: r.name.clone(), n: if shift { u16::MAX } else { 1 } });
                    self.audio.play("ui.tick", None, 1.0, 1.0, false);
                }
                return;
            }
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
            (Some((fh, fi, n)), None) if !inside(l.panel, p) && !inside(l.dock, p) => {
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
        // a furnace open: from the ape's own slots into it, a fuel to its fuel, a mould to its mould, else into the
        // first input that holds it or is empty
        if let (Holder::Ape, Some((b, f))) = (h, self.furnace.as_ref()) {
            use engine::furnace::{FUEL, IN, IN2, MOULD};
            let fits = |k: usize| f.slots[k].is_none_or(|t| t.item == st.item);
            let k = if RECIPES.fuel(st.item).is_some() { FUEL } else if RECIPES.mould(st.item) { MOULD } else if !fits(IN) || f.slots[IN2].is_some_and(|t| t.item == st.item) { IN2 } else { IN };
            return fits(k).then_some((Holder::Furnace(*b), k as u16));
        }
        let hot = (h, (i as usize) < items::HOTBAR) == (Holder::Ape, true);
        let to: Vec<usize> = match h {
            Holder::Ape if i as usize >= CRAFT_AT => items::usable(&self.inv).collect(),
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
        // a block's window is titled by its block
        let title = match self.screen() {
            Screen::Creative => "CREATIVE".to_string(),
            Screen::Inventory => "INVENTORY".to_string(),
            _ => self.bench.or(self.furnace.as_ref().map(|f| f.0)).and_then(|b| BLOCKS.get(self.world.block(b))).map_or("BLOCK".into(), |d| d.name.to_uppercase()),
        };
        p.text_px(px0 + PAD + 14.0, py0 + 13.0, &title, [1.0; 4], hud::PX * 1.5);
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
        if !self.crafting() {
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
            self.top_draw(p, l, season, pointer);
            self.viewer_draw(p, l, season, pointer);
        }
        // the inventory's own parts: the ape, the pile, the worn slot with health and hunger
        if self.screen() == Screen::Inventory {
            self.ape_draw(p, l, season);
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
            // a grid cell the last Fill found nothing for, while it is still empty: red
            let lacks = h == Holder::Ape && (i as usize) >= CRAFT_AT && self.lack.contains(&(i as usize - CRAFT_AT)) && self.slot_at(h, i).is_none();
            p.rect(b[0], b[1], b[2], b[3], if lacks { RED } else if lit { [0.17, 0.24, 0.33, 1.0] } else { [0.07, 0.1, 0.15, 0.9] }, 0);
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
        let hint = hint_of(self.screen());
        p.text_px(l.hint[0], l.hint[1], hint, [0.62, 0.72, 0.82, 0.8], HINT_PX);
    }

    /// A window's crafting part: the grid's heading, its arrow and output (what the cells hold now); a furnace's
    /// window instead (`furnace_draw`). A new block window's kind draws its top part here.
    fn top_draw(&self, p: &mut hud::Paint, l: &Layout, season: (f64, f64), pointer: (f32, f32)) {
        let head = |p: &mut hud::Paint, x: f32, y: f32, s: &str| p.text_px(x, y - 18.0, &s.to_uppercase(), [CYAN[0], CYAN[1], CYAN[2], 0.85], hud::PX * 0.75);
        let (small, dim) = (hud::PX * 0.75, [0.55, 0.62, 0.7, 0.9]);
        if self.furnace.is_some() {
            return self.furnace_draw(p, l, season, pointer);
        }
        let three = self.bench.is_some();
        head(p, l.craft[0], l.craft[1], if three { "Craft 3 x 3" } else { "Craft 2 x 2" });
        // the arrow from the grid to the output
        let (ax, ay) = (l.out[0] - 30.0, l.out[1] + SIZE / 2.0);
        p.rect(ax, ay - 1.5, 18.0, 3.0, [CYAN[0], CYAN[1], CYAN[2], 0.6], 0);
        for k in 0..5 {
            let k = k as f32;
            p.rect(ax + 18.0 + k * 1.6, ay - 6.0 + k * 1.2, 1.6, 12.0 - k * 2.4, [CYAN[0], CYAN[1], CYAN[2], 0.6], 0);
        }
        let made = RECIPES.matched(&self.inv, three);
        let lit = made.is_some() && self.lifted.is_none() && inside(l.out, pointer);
        let o = l.out;
        p.rect(o[0], o[1], o[2], o[3], if lit { [0.17, 0.24, 0.33, 1.0] } else { [0.07, 0.1, 0.15, 0.9] }, 0);
        edge(p, o, if made.is_some() { [CYAN[0], CYAN[1], CYAN[2], 0.7] } else { [1.0, 1.0, 1.0, 0.14] });
        if let Some(r) = made {
            self.icon(p, Stack::new(r.out.0, r.out.1), o[0] + 6.0, o[1] + 6.0, 40.0, season, true);
            // its name under the output, kept inside the window
            let cols = ((l.panel[0] + l.panel[2] - 8.0 - o[0]) / (hud::ADV * small)) as usize;
            for (k, s) in wrap(ITEMS.name(r.out.0), cols).iter().take(2).enumerate() {
                p.text_px(o[0], o[1] + SIZE + 6.0 + k as f32 * 16.0, s, dim, small);
            }
        }
    }

    /// The recipe viewer docked at the right (ADR 0140; the developer, 2026-11-17: on the right, opaque): its
    /// title, the search, the items, and the recipe of the one clicked with its Fill button (none at a furnace) and
    /// its lines under it.
    /// shortcut: a "hold to analyze" explainer for complicated items would stand in the recipe's lines.
    fn viewer_draw(&self, p: &mut hud::Paint, l: &Layout, season: (f64, f64), pointer: (f32, f32)) {
        let head = |p: &mut hud::Paint, x: f32, y: f32, s: &str| p.text_px(x, y - 18.0, &s.to_uppercase(), [CYAN[0], CYAN[1], CYAN[2], 0.85], hud::PX * 0.75);
        let (small, dim) = (hud::PX * 0.75, [0.55, 0.62, 0.7, 0.9]);
        let faint = [0.55, 0.62, 0.7, 0.6];
        let three = self.bench.is_some();
        // its room, opaque, and a hairline of the accent down its left edge; the title row as the window's
        let d = l.dock;
        p.rect(d[0], d[1], d[2], d[3], [hud::PANEL[0], hud::PANEL[1], hud::PANEL[2], 1.0], 0);
        p.rect(d[0], d[1], 1.0, d[3], [CYAN[0], CYAN[1], CYAN[2], 0.4], 0);
        let s = l.search;
        p.rect(s[0], 13.0, 3.0, 24.0, CYAN, 0);
        p.text_px(s[0] + 14.0, 13.0, "RECIPES", [1.0; 4], hud::PX * 1.5);
        // the search field (lit while it takes the keys), the items
        let on = self.inv_tab == SEARCH;
        p.rect(s[0], s[1], s[2], s[3], [0.01, 0.03, 0.06, 1.0], 0);
        edge(p, s, if on { CYAN } else { [1.0, 1.0, 1.0, 0.16] });
        let q = &self.inv_query;
        let caret = self.start.elapsed().as_millis() / 500 % 2 == 0;
        if q.is_empty() && !on {
            p.text_px(s[0] + 8.0, s[1] + 8.0, "Search...", dim, hud::PX);
        } else {
            p.text_px(s[0] + 8.0, s[1] + 8.0, &if on && caret { format!("{q}_") } else { q.clone() }, [1.0; 4], hud::PX);
        }
        for &(id, b) in &l.list {
            let lit = inside(b, pointer) || self.viewing == Some(id);
            p.rect(b[0], b[1], b[2], b[3], if lit { [0.17, 0.24, 0.33, 1.0] } else { [0.07, 0.1, 0.15, 1.0] }, 0);
            edge(p, b, if self.viewing == Some(id) { CYAN } else { [1.0, 1.0, 1.0, if RECIPES.making(id).is_some() || RECIPES.work_making(id).is_some() { 0.2 } else { 0.07 }] });
            self.icon(p, Stack::new(id, 1), b[0] + 4.0, b[1] + 4.0, VSIZE - 8.0, season, false);
        }
        if l.list.is_empty() {
            p.text_px(l.view[0], l.view[1] + 8.0, &format!("Nothing named '{q}'"), dim, small);
        }
        // the recipe shown: its pattern from the top left of a 3 x 3, red where the slots hold none of a cell's
        // items; its lines under it
        let pt = l.pattern;
        head(p, pt[0], pt[1], "Recipe");
        let mut lines: Vec<(String, [f32; 4])> = Vec::new();
        // Fill: none at a furnace; dim for a furnace's work, or while the recipe needs the bench and only the 2 x 2 is open
        let mut fill = Some(false);
        match self.viewing {
            None => {
                lines.push(("Click an item to see how it is made".into(), faint));
                fill = None;
            }
            Some(item) => {
                lines.push((ITEMS.name(item).into(), [1.0; 4]));
                // a furnace's work (ADR 0141): what goes in (both of an alloy's, and the mould) in the pattern's first
                // cells, the furnace and heat it needs; several works of one output shown in turn; Fill lays nothing
                let works: Vec<&engine::recipes::Work> = RECIPES.works.iter().filter(|w| w.out.0 == item).collect();
                match RECIPES.making(item) {
                    None if !works.is_empty() => {
                        let w = works[self.start.elapsed().as_secs() as usize % works.len()];
                        for (k, it) in work_cells(w).into_iter().enumerate() {
                            let b = [pt[0] + k as f32 * (PSIZE + VGAP), pt[1], PSIZE, PSIZE];
                            p.rect(b[0], b[1], b[2], b[3], [0.07, 0.1, 0.15, 1.0], 0);
                            edge(p, b, [1.0, 1.0, 1.0, 0.1]);
                            if let Some((i, n)) = it {
                                self.icon(p, Stack::new(i, n), b[0] + 3.0, b[1] + 3.0, PSIZE - 6.0, season, true);
                            }
                        }
                        let at = RECIPES.furnaces.iter().filter(|f| f.top >= w.heat).map(|f| ITEMS.name(f.block)).next().unwrap_or("no furnace yet");
                        lines.push((format!("makes {}{}", w.out.1, if w.mould.is_some() { ", mould kept" } else { "" }), dim));
                        lines.push((format!("in a {at}, at {:.0} C, {:.0} s", w.heat, w.secs), faint));
                    }
                    None => {
                        lines.push((source(item), dim));
                        fill = None;
                    }
                    Some(r) => {
                        let carried = |any: &Vec<u16>| any.iter().copied().find(|&i| items::count(&self.inv, i) > 0 || engine::recipes::grid(&self.inv).contains(&Some(i)));
                        let t = self.start.elapsed().as_secs() as usize;
                        for k in 0..CRAFT {
                            let (row, col) = (k / 3, k % 3);
                            let b = [pt[0] + col as f32 * (PSIZE + VGAP), pt[1] + row as f32 * (PSIZE + VGAP), PSIZE, PSIZE];
                            let cell = (row < r.h && col < r.w).then(|| r.cells[row * r.w + col].as_ref()).flatten();
                            let has = cell.and_then(carried);
                            p.rect(b[0], b[1], b[2], b[3], if cell.is_some() && has.is_none() { RED } else { [0.07, 0.1, 0.15, 1.0] }, 0);
                            edge(p, b, [1.0, 1.0, 1.0, 0.1]);
                            // a cell of alternatives shows the one carried, else each in turn
                            if let Some(any) = cell {
                                self.icon(p, Stack::new(has.unwrap_or(any[t % any.len()]), 1), b[0] + 3.0, b[1] + 3.0, PSIZE - 6.0, season, false);
                            }
                        }
                        lines.push((format!("makes {}", r.out.1), dim));
                        let need = r.bench() && !three;
                        lines.push((if r.bench() { "needs a workbench" } else { "by hand, 2 x 2" }.into(), if need { [1.0, 0.55, 0.45, 0.95] } else { faint }));
                        fill = Some(!need);
                    }
                }
            }
        }
        let cols = (l.text[2] / (hud::ADV * small)) as usize;
        let shown = lines.iter().flat_map(|(s, c)| wrap(s, cols).into_iter().map(move |w| (w, *c))).take(TEXT_LINES);
        for (k, (s, c)) in shown.enumerate() {
            p.text_px(l.text[0], l.text[1] + k as f32 * TEXT_LH, &s, c, small);
        }
        let f = l.fill;
        let (Some(live), true) = (fill, f[2] > 0.0) else { return };
        let lit = live && inside(f, pointer);
        hud::cut(p, f[0], f[1], f[2], f[3], if !live { [0.09, 0.12, 0.17, 1.0] } else if lit { CYAN } else { [0.16, 0.22, 0.3, 1.0] });
        let label = "Fill";
        let lx = f[0] + (f[2] - hud::text_width(label)) / 2.0;
        if lit {
            p.text_flat(lx, f[1] + 8.0, label, [0.01, 0.06, 0.12, 1.0], hud::PX);
        } else {
            p.text_px(lx, f[1] + 8.0, label, if live { [0.9, 0.95, 1.0, 1.0] } else { [0.5, 0.55, 0.6, 0.5] }, hud::PX);
        }
    }

    /// A furnace's window in the grid's room (ADR 0141): its slots' headings, the arrow to what comes out
    /// filling with the work done, the fire's seconds and the heat as a number and a bar marked where the work
    /// needs it, and the casts to choose from (the chosen one lit, its ingots under it).
    fn furnace_draw(&self, p: &mut hud::Paint, l: &Layout, season: (f64, f64), pointer: (f32, f32)) {
        use engine::furnace::{FUEL, IN, IN2, MOULD};
        let head = |p: &mut hud::Paint, x: f32, y: f32, s: &str| p.text_px(x, y - 18.0, &s.to_uppercase(), [CYAN[0], CYAN[1], CYAN[2], 0.85], hud::PX * 0.75);
        let (small, dim, faint) = (hud::PX * 0.75, [0.55, 0.62, 0.7, 0.9], [0.55, 0.62, 0.7, 0.6]);
        let (Some((_, f)), Some((def, casts))) = (self.furnace.as_ref(), self.furnace_def()) else { return };
        let spot = |k: usize| l.spots.iter().find(|s| matches!(s.0, Holder::Furnace(_)) && s.1 as usize == k).map_or([0.0; 4], |s| s.2);
        let (i, j, m, u) = (spot(IN), spot(IN2), spot(MOULD), spot(FUEL));
        head(p, i[0], i[1], "In (two to alloy)");
        head(p, m[0], m[1], "Mould");
        head(p, l.out[0], l.out[1], "Out");
        head(p, u[0], u[1], "Fuel");
        let want = f.wants(def, &RECIPES);
        // the arrow from the second input to what comes out, filling with the work's seconds
        let (ax, ay, aw) = (j[0] + SIZE + 12.0, i[1] + SIZE / 2.0, l.out[0] - 22.0 - (j[0] + SIZE + 12.0));
        p.rect(ax, ay - 2.0, aw, 4.0, [1.0, 1.0, 1.0, 0.1], 0);
        let done = want.map_or(0.0, |w| (f.work / w.secs).clamp(0.0, 1.0));
        p.rect(ax, ay - 2.0, aw * done, 4.0, [1.0, 0.6, 0.25, 0.95], 0);
        for k in 0..5 {
            let k = k as f32;
            p.rect(ax + aw + k * 1.6, ay - 6.0 + k * 1.2, 1.6, 12.0 - k * 2.4, if done >= 1.0 { [1.0, 0.6, 0.25, 0.95] } else { [CYAN[0], CYAN[1], CYAN[2], 0.6] }, 0);
        }
        let doing = match want {
            Some(w) if f.heat >= w.heat => format!("{} {:.0} %", ITEMS.name(w.out.0), done * 100.0),
            Some(w) => format!("{}: needs {:.0} C", ITEMS.name(w.out.0), w.heat),
            None if f.slots[IN].or(f.slots[IN2]).is_some_and(|s| RECIPES.works.iter().any(|w| w.mould.is_some() && w.input.0 == s.item)) => "choose a cast below".into(),
            None => "nothing to work".into(),
        };
        // under the in and mould slots: beside the arrow it ran over the out slot and the window's edge
        p.text_px(i[0], i[1] + SIZE + 8.0, &doing, dim, small);
        // the fire beside the fuel: its seconds left, or out
        // (short: the mould slot stands at the row's end)
        let fire = if f.burning() { format!("fire {:.0} s left", f.burn.ceil()) } else { "fire out".into() };
        p.text_px(u[0] + SIZE + 14.0, u[1] + 6.0, &fire, if f.burning() { [1.0, 0.7, 0.35, 1.0] } else { faint }, small);
        p.text_px(u[0] + SIZE + 14.0, u[1] + 26.0, &format!("{:.0} C of {:.0}", f.heat, def.top), [1.0; 4], small);
        // the heat's bar, marked at what the work needs
        let h = l.heat;
        p.rect(h[0], h[1], h[2], h[3], [1.0, 1.0, 1.0, 0.08], 0);
        let share = |c: f32| ((c - engine::furnace::AIR_C) / (def.top - engine::furnace::AIR_C)).clamp(0.0, 1.0);
        p.rect(h[0], h[1], h[2] * share(f.heat), h[3], [1.0, 0.45 + 0.25 * share(f.heat), 0.2, 0.95], 0);
        if let Some(w) = want {
            let x = h[0] + h[2] * share(w.heat);
            p.rect(x - 1.0, h[1] - 4.0, 2.0, h[3] + 8.0, [1.0; 4], 0);
            p.text_px((x - 50.0).max(h[0]), h[1] - 16.0, &format!("{:.0} C", w.heat), faint, small);
        }
        // the casts, each shape once, drawn as the row the metal in an input slot would run (else the first)
        let chosen = f.choice.as_deref().and_then(|c| RECIPES.shape(c));
        let metal = |c: &'static engine::recipes::Work| {
            RECIPES.casts(def.top).find(|w| w.shape == c.shape && [IN, IN2].iter().any(|&k| f.slots[k].is_some_and(|s| s.item == w.input.0))).unwrap_or(c)
        };
        for (k, (c, b)) in casts.iter().zip(&l.casts).enumerate() {
            let on = chosen.is_some() && chosen == c.shape.as_deref();
            let lit = inside(*b, pointer);
            p.rect(b[0], b[1], b[2], b[3], if on { [0.17, 0.24, 0.33, 1.0] } else if lit { [0.12, 0.17, 0.24, 1.0] } else { [0.07, 0.1, 0.15, 0.9] }, 0);
            edge(p, *b, if on { CYAN } else { [1.0, 1.0, 1.0, 0.14] });
            self.icon(p, Stack::new(metal(c).out.0, 1), b[0] + 4.0, b[1] + 4.0, VSIZE - 8.0, season, false);
            if k == 0 {
                head(p, b[0], b[1], "Cast (click to choose)");
            }
        }
        if let Some(c) = casts.iter().find(|c| chosen.is_some() && chosen == c.shape.as_deref()).map(|c| metal(c)) {
            p.text_px(l.casts[0][0], l.casts[0][1] + VSIZE + 6.0, &format!("{}: {} ingots", ITEMS.name(c.out.0), c.input.1), dim, small);
        }
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
    /// Its rects are kept (`hud::Looks`) by all they are made from: the item, its block's colours this season,
    /// the textures and models loaded, the square's and the screen's size. A seasonal block's colour drifts every
    /// frame of the year: it is made again when a colour has moved by 1/4096 (a sixteenth of a step of the
    /// screen's 8 bits), not every frame.
    pub(crate) fn look_of(&self, p: &mut hud::Paint, item: u16, x: f32, y: f32, size: f32, season: (f64, f64)) {
        let (colour, side, pattern) = BLOCKS.get(ITEMS.look(item)).map_or(([0.0; 3], [0.0; 3], 0), |d| crate::gpu::shown(d, season));
        let two = |a: f32, b: f32| a.to_bits() as u64 | (b.to_bits() as u64) << 32;
        let step = |a: f32, b: f32| two((a * 4096.0).round(), (b * 4096.0).round());
        let key = [
            item as u64 | (pattern as u64) << 16,
            step(colour[0], colour[1]),
            step(colour[2], side[0]),
            step(side[1], side[2]),
            crate::gpu::PAINT_GEN.load(Ordering::Relaxed),
            self.models.gen,
            two(size, p.w),
            p.h.to_bits() as u64,
        ];
        self.looks.borrow_mut().draw(key, p, x, y, |q| self.look_at(q, item, 0.0, 0.0, size, season));
    }

    /// `look_of`, drawn: the reference its kept rects are made from.
    fn look_at(&self, p: &mut hud::Paint, item: u16, x: f32, y: f32, size: f32, season: (f64, f64)) {
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
        for screen in [Screen::Creative, Screen::Inventory, Screen::Bench] {
            let l = layout(lw, lh, screen, &[], 0, items::ROWS_AT, None);
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

/// The red of a grid cell, or a pattern's cell, whose item the slots lack.
const RED: [f32; 4] = [0.5, 0.1, 0.09, 0.9];

/// Fill (ADR 0140): the moves, (from, to, n) within the ape's slots in order, that lay one make of
/// `r` in the grid from its top left: first what lies in a cell the pattern does not want there goes back into the
/// usable slots, then one of a wanted item from them into each pattern cell not holding one already. Also the cells
/// whose items the slots lack (nothing is moved for them). None when `r` needs the bench and only the 2 x 2 is open.
pub(crate) fn fill_plan(slots: &[Slot], r: &Recipe, three: bool) -> Option<(Vec<(usize, usize, u16)>, Vec<usize>)> {
    if r.bench() && !three {
        return None;
    }
    let want = |k: usize| (k / 3 < r.h && k % 3 < r.w).then(|| r.cells[k / 3 * r.w + k % 3].as_ref()).flatten();
    let (mut s, mut moves, mut lack) = (slots.to_vec(), Vec::new(), Vec::new());
    for k in 0..CRAFT {
        let at = CRAFT_AT + k;
        let Some(st) = s[at].filter(|st| !want(k).is_some_and(|any| any.contains(&st.item))) else { continue };
        let room = |t: Slot| t.is_some_and(|t| t.item == st.item && t.payload == st.payload && t.n + st.n <= ITEMS.stack(t.item));
        let to = items::usable(&s).find(|&j| room(s[j])).or_else(|| items::usable(&s).find(|&j| s[j].is_none()));
        if let Some(j) = to.filter(|&j| items::shift(&mut s, at, None, j, st.n)) {
            moves.push((at, j, st.n));
        }
    }
    for k in 0..CRAFT {
        let Some(any) = want(k).filter(|_| s[CRAFT_AT + k].is_none()) else { continue };
        match items::usable(&s).find(|&j| s[j].is_some_and(|t| any.contains(&t.item))) {
            Some(j) if items::shift(&mut s, j, None, CRAFT_AT + k, 1) => moves.push((j, CRAFT_AT + k, 1)),
            _ => lack.push(k),
        }
    }
    Some((moves, lack))
}

#[test]
fn fill_lays_a_recipe_from_the_slots_and_marks_what_is_lacking() {
    let id = |n: &str| ITEMS.named(n).unwrap();
    let pick = RECIPES.named("copper pick").unwrap();
    let mut s = items::ape();
    items::give(&mut s, Stack::new(id("copper pick head"), 2));
    items::give(&mut s, Stack::new(id("stick"), 3));
    // flint in a cell the pick does not want goes back first
    s[CRAFT_AT + 4] = Some(Stack::new(id("flint"), 2));
    let (moves, lack) = fill_plan(&s, pick, false).unwrap();
    assert_eq!((moves.len(), lack), (3, vec![]));
    for (a, b, n) in moves {
        assert!(items::shift(&mut s, a, None, b, n) && items::ape_fits(&s));
    }
    assert_eq!(RECIPES.matched(&s, false), Some(pick), "the grid now holds the pick");
    assert_eq!((s[CRAFT_AT], s[CRAFT_AT + 3], s[CRAFT_AT + 4]), (Some(Stack::new(id("copper pick head"), 1)), Some(Stack::new(id("stick"), 1)), None));
    assert_eq!(items::count(&s, id("flint")), 2);
    assert_eq!(fill_plan(&s, pick, false), Some((vec![], vec![])), "filled already: nothing to move");
    // no stick: the head goes in, the stick's cell is lacking
    let mut s = items::ape();
    items::give(&mut s, Stack::new(id("copper pick head"), 1));
    assert_eq!(fill_plan(&s, pick, false), Some((vec![(0, CRAFT_AT, 1)], vec![3])));
    // any log for the workbench's four cells, from two kinds
    let mut s = items::ape();
    items::give(&mut s, Stack::new(id("oak log"), 3));
    items::give(&mut s, Stack::new(id("pine log"), 5));
    let (moves, lack) = fill_plan(&s, RECIPES.named("workbench").unwrap(), false).unwrap();
    assert_eq!((moves.iter().map(|m| m.1 - CRAFT_AT).collect::<Vec<_>>(), lack), (vec![0, 1, 3, 4], vec![]));
    // a recipe that needs the bench: nothing with the 2 x 2, laid out at a bench
    let t = engine::recipes::Recipes::parse("[[recipe]]\nname='tall'\ngrid=['S', 'S', 'S']\nkey={S='stick'}\nout=['hand axe', 1]\n", &ITEMS).unwrap();
    assert_eq!(fill_plan(&s, &t.rows[0], false), None);
    assert_eq!(fill_plan(&s, &t.rows[0], true).map(|f| f.1), Some(vec![0, 3, 6]));
}

/// Where an item with no recipe comes from, when the data knows: knapped from a stone, held in a block dug, left
/// by digging rock; else only that it has no recipe.
fn source(item: u16) -> String {
    if let Some(from) = ITEMS.all().find(|&i| ITEMS.get(i).and_then(|d| d.knap).is_some_and(|k| k.0 == item)) {
        return format!("no recipe: knapped from {} on rock", ITEMS.name(from));
    }
    let name = ITEMS.name(item);
    if let Some(b) = BLOCKS.placeable().find(|d| d.holds.as_ref().is_some_and(|h| h.0 == name)) {
        return format!("no recipe: found digging {}", b.name);
    }
    if let Some(rock) = BLOCKS.get(item).and_then(|d| d.rubble_of) {
        return format!("no recipe: left by digging {}", BLOCKS.get(rock).map_or("rock", |d| d.name.as_str()));
    }
    "no recipe".into()
}

#[test]
fn an_item_with_no_recipe_says_where_it_comes_from_if_the_data_knows() {
    assert_eq!(source(ITEMS.named("hand axe").unwrap()), "no recipe: knapped from flint on rock");
    assert_eq!(source(ITEMS.named("flint").unwrap()), "no recipe: found digging gravel");
    assert_eq!(source(ITEMS.named("granite rubble").unwrap()), "no recipe: left by digging granite");
    assert_eq!(source(ITEMS.named("stick").unwrap()), "no recipe");
}

use hud::CYAN;
