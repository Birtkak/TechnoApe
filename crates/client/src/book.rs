//! The journal's book (docs/design/journal.md, "The book"; draft ADR journal-book): the entries this ape has
//! noticed or knows, the open questions first, then the known ones by age; the chosen one's page at the right.
//! What the server told of the journal (`ToClient::Journal`) is kept here; the layout is in named rects so the
//! visual pass changes the drawing, not the structure.
use crate::app::App;
use crate::hud;
use crate::menu::Menu;
use engine::journal::{State, JOURNAL};

const MARGIN: f32 = 24.0;
const MAX_W: f32 = 1100.0;
const MAX_H: f32 = 680.0;
const PAD: f32 = 22.0;
const TITLE_H: f32 = 50.0;
const HINT_H: f32 = 28.0;
const GAP: f32 = 20.0;
/// Text at the HUD's own size (a whole number of screen pixels a font pixel: the pixel font stays whole), the
/// page title a step larger (3 pixels a font pixel, as the windows' titles).
const TEXT: f32 = hud::PX;
const BIG: f32 = hud::PX * 1.5;
/// A row of the list, a line of the page (`hud::LINE`, the menus' line), the page title's room; a list row
/// leaves room for its changed mark.
const ROW_H: f32 = 28.0;
const LINE_H: f32 = hud::LINE;
const HEAD_H: f32 = 40.0;
const MARK_W: f32 = 16.0;
const HINT: &str = "Click: choose   Wheel: scroll   J: close";

/// Where the book's parts lie in UI pixels, and how much text fits in them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Layout {
    pub panel: [f32; 4],
    /// The title row's text.
    pub title: [f32; 4],
    /// The list of entries (left); the chosen entry's page (right): its title (`head`) over its lines (`body`);
    /// the hint under both.
    pub list: [f32; 4],
    pub page: [f32; 4],
    pub head: [f32; 4],
    pub body: [f32; 4],
    pub hint: [f32; 4],
    /// Rows of the list and lines of the body that fit; characters a list row, a body line, the page title.
    pub rows: usize,
    pub lines: usize,
    pub row_cols: usize,
    pub cols: usize,
    pub head_cols: usize,
}

/// The book on a `lw` x `lh` UI: a window in the middle, as large as fits up to `MAX_W` x `MAX_H`.
pub(crate) fn layout(lw: f32, lh: f32) -> Layout {
    let (w, h) = ((lw - 2.0 * MARGIN).min(MAX_W), (lh - 2.0 * MARGIN).min(MAX_H));
    let (x, y) = (((lw - w) / 2.0).floor(), ((lh - h) / 2.0).floor());
    let (y0, ih) = (y + TITLE_H, h - TITLE_H - HINT_H - PAD / 2.0);
    let list_w = ((w - 2.0 * PAD - GAP) * 0.42).floor();
    let list = [x + PAD, y0, list_w, ih];
    let page = [list[0] + list_w + GAP, y0, x + w - PAD - (list[0] + list_w + GAP), ih];
    let adv = |px: f32| hud::ADV * px;
    Layout {
        panel: [x, y, w, h],
        title: [x + PAD, y + 13.0, w - 2.0 * PAD, 24.0],
        list,
        page,
        head: [page[0], page[1], page[2], HEAD_H],
        body: [page[0], page[1] + HEAD_H, page[2], ih - HEAD_H],
        hint: [x + PAD, y + h - HINT_H - PAD / 2.0, w - 2.0 * PAD, HINT_H],
        rows: (ih / ROW_H).floor() as usize,
        lines: ((ih - HEAD_H) / LINE_H).floor() as usize,
        row_cols: ((list_w - MARK_W) / adv(TEXT)).floor() as usize,
        cols: (page[2] / adv(TEXT)).floor() as usize,
        head_cols: (page[2] / adv(BIG)).floor() as usize,
    }
}

impl Layout {
    /// The list's i-th row in view, the body's i-th line in view.
    pub fn row(&self, i: usize) -> [f32; 4] {
        [self.list[0], self.list[1] + i as f32 * ROW_H, self.list[2], ROW_H]
    }
    pub fn line(&self, i: usize) -> [f32; 4] {
        [self.body[0], self.body[1] + i as f32 * LINE_H, self.body[2], LINE_H]
    }
}

fn inside(b: [f32; 4], (x, y): (f32, f32)) -> bool {
    x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]
}

/// A row of the list: a group's heading or an entry (its index in `JOURNAL`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    Head(String),
    Entry(usize),
}

/// A line of the page: its text, and the entry it leads to when clicked.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
    pub text: String,
    pub lead: Option<usize>,
}

/// The journal as this client knows it, and the book's own state (not saved).
#[derive(Default)]
pub(crate) struct Book {
    /// Each entry's state as the server last told; None before its first message.
    pub states: Option<Vec<State>>,
    /// Entries that changed since the first message and have not been chosen since.
    pub marked: Vec<bool>,
    pub sel: Option<usize>,
    /// How far the list and the page were scrolled; read through `Book::scrolls`, which holds them to what
    /// there is to scroll now.
    pub list_scroll: usize,
    pub page_scroll: usize,
}

/// `s` cut to `cols` characters, ".." at its end when it was longer.
fn fit(s: &str, cols: usize) -> String {
    if s.chars().count() <= cols {
        return s.to_string();
    }
    let mut t: String = s.chars().take(cols.saturating_sub(2)).collect();
    t.push_str("..");
    t
}

/// A text in lines of at most `cols` characters: broken between words, a word longer than a line broken across
/// lines, and a blank line between paragraphs (a blank line in the text). The inventory's `wrap` stays as it was.
pub(crate) fn wrap(s: &str, cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    let mut out: Vec<String> = Vec::new();
    for (k, para) in s.replace("\r\n", "\n").split("\n\n").map(str::trim).filter(|p| !p.is_empty()).enumerate() {
        if k > 0 {
            out.push(String::new());
        }
        let mut line = String::new();
        for word in para.split_whitespace() {
            let mut w: Vec<char> = word.chars().collect();
            if !line.is_empty() && line.chars().count() + 1 + w.len() <= cols {
                line.push(' ');
                line.extend(&w);
                continue;
            }
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            while w.len() > cols {
                out.push(w.drain(..cols).collect());
            }
            line = w.into_iter().collect();
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

/// An age's heading (progression.md's names).
fn age_head(age: &str) -> String {
    match age {
        "steam" => "THE STEAM AGE".into(),
        a => a.to_uppercase(),
    }
}

impl Book {
    pub fn state(&self, k: usize) -> State {
        self.states.as_ref().and_then(|s| s.get(k).copied()).unwrap_or_default()
    }

    /// A `Journal` message: the states it names, and the chat's lines for it, a question noticed and an entry
    /// learned. The first message is the journal the ape joined with: no lines, no marks. The entry open on the
    /// page is not marked (it is in view); one become unknown is shown no more, so it is chosen no more.
    pub fn take(&mut self, v: &[(String, State)]) -> Vec<String> {
        let first = self.states.is_none();
        let n = JOURNAL.entries.len();
        let states = self.states.get_or_insert_with(|| vec![State::Unknown; n]);
        self.marked.resize(n, false);
        let mut lines = Vec::new();
        for (name, s) in v {
            let Some(k) = JOURNAL.index(name) else { continue };
            states[k] = *s;
            let e = &JOURNAL.entries[k];
            if *s == State::Unknown && self.sel == Some(k) {
                (self.sel, self.page_scroll) = (None, 0);
            }
            if first {
                continue;
            }
            self.marked[k] = *s != State::Unknown && self.sel != Some(k);
            match s {
                State::Noticed => lines.push(format!("Journal: {}", e.question)),
                State::Known => lines.push(format!("Journal: learned {}", e.title.to_lowercase())),
                State::Unknown => {}
            }
        }
        lines
    }

    /// The list: the open questions (noticed) first, then the known entries under their ages, each group in
    /// the file's order; an unknown entry is not there, and a group with none shown has no heading.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        let noticed: Vec<usize> = (0..JOURNAL.entries.len()).filter(|&k| self.state(k) == State::Noticed).collect();
        if !noticed.is_empty() {
            rows.push(Row::Head("OPEN QUESTIONS".into()));
            rows.extend(noticed.into_iter().map(Row::Entry));
        }
        let mut ages: Vec<&str> = Vec::new();
        for e in &JOURNAL.entries {
            if !ages.contains(&e.age.as_str()) {
                ages.push(&e.age);
            }
        }
        for a in ages {
            let known: Vec<usize> = (0..JOURNAL.entries.len()).filter(|&k| JOURNAL.entries[k].age == a && self.state(k) == State::Known).collect();
            if !known.is_empty() {
                rows.push(Row::Head(age_head(a)));
                rows.extend(known.into_iter().map(Row::Entry));
            }
        }
        rows
    }

    /// A list row's text: a known entry's title, a noticed one's question, cut to `cols`.
    pub fn row_text(&self, r: &Row, cols: usize) -> String {
        match r {
            Row::Head(h) => fit(h, cols),
            Row::Entry(k) => {
                let e = &JOURNAL.entries[*k];
                fit(if self.state(*k) == State::Known { &e.title } else { &e.question }, cols)
            }
        }
    }

    /// The chosen entry's page, fitted to the layout: its title (a known entry's; cut to the head's room) and its
    /// lines: a known one its text and leads (the entries it points at that are noticed or known), a noticed one
    /// its question alone; with none chosen, a line that says what to do (or that nothing is noticed yet).
    pub fn page(&self, l: &Layout) -> (Option<String>, Vec<Line>) {
        let body = |s: &str| wrap(s, l.cols).into_iter().map(|t| Line { text: t, lead: None }).collect::<Vec<_>>();
        let Some(k) = self.sel.filter(|&k| self.state(k) != State::Unknown) else {
            let s = if self.rows().is_empty() { "Nothing noticed yet. What you look at and pick up raises questions here." } else { "Choose an entry on the left." };
            return (None, body(s));
        };
        let e = &JOURNAL.entries[k];
        if self.state(k) == State::Noticed {
            return (None, body(&e.question));
        }
        let mut v = body(&e.text);
        let leads: Vec<usize> = e.points_at.iter().copied().filter(|&p| self.state(p) != State::Unknown).collect();
        if !leads.is_empty() {
            v.push(Line { text: String::new(), lead: None });
            v.push(Line { text: "LEADS TO".into(), lead: None });
            for p in leads {
                let t = &JOURNAL.entries[p];
                let what = if self.state(p) == State::Known { &t.title } else { &t.question };
                v.push(Line { text: fit(&format!("> {what}"), l.cols), lead: Some(p) });
            }
        }
        (Some(fit(&e.title, l.head_cols)), v)
    }

    /// The list's and the page's scroll as shown: each held to what runs over its room now, so a journal that
    /// shrank under an open book never leaves it scrolled past its end.
    pub fn scrolls(&self, l: &Layout) -> (usize, usize) {
        (self.list_scroll.min(self.rows().len().saturating_sub(l.rows)), self.page_scroll.min(self.page(l).1.len().saturating_sub(l.lines)))
    }

    /// Chooses entry k: its page from the top, its mark gone.
    pub fn select(&mut self, k: usize) {
        self.sel = Some(k);
        self.page_scroll = 0;
        if let Some(m) = self.marked.get_mut(k) {
            *m = false;
        }
    }

    /// The entry a click at `at` chooses: a row of the list in view, or a lead on the page in view.
    pub fn hit(&self, l: &Layout, at: (f32, f32)) -> Option<usize> {
        let (ls, ps) = self.scrolls(l);
        for (i, r) in self.rows().iter().skip(ls).take(l.rows).enumerate() {
            if let Row::Entry(k) = r {
                if inside(l.row(i), at) {
                    return Some(*k);
                }
            }
        }
        self.page(l).1.iter().skip(ps).take(l.lines).enumerate().find(|(i, line)| line.lead.is_some() && inside(l.line(*i), at)).and_then(|(_, line)| line.lead)
    }

    /// The wheel by `d` rows: the list's when `at` is over it, else the page's, each only as far as it runs over.
    pub fn scroll(&mut self, l: &Layout, at: (f32, f32), d: i32) {
        let (ls, ps) = self.scrolls(l);
        let step = |pos: usize, most: usize| (pos as i32 + d).clamp(0, most as i32) as usize;
        (self.list_scroll, self.page_scroll) = (ls, ps);
        if inside(l.list, at) {
            self.list_scroll = step(ls, self.rows().len().saturating_sub(l.rows));
        } else {
            self.page_scroll = step(ps, self.page(l).1.len().saturating_sub(l.lines));
        }
    }
}

impl App {
    /// The book's key: opens it, or closes it (as the inventory's).
    pub(crate) fn toggle_book(&mut self) {
        match self.menu {
            Some(Menu::Book) => self.set_menu(None),
            None if self.home() != Menu::Title && self.dead.is_none() => self.set_menu(Some(Menu::Book)),
            _ => {}
        }
    }

    pub(crate) fn book_layout(&self) -> Layout {
        let (lw, lh) = self.ui_size();
        layout(lw, lh)
    }

    pub(crate) fn book_click(&mut self) {
        let l = self.book_layout();
        if let Some(k) = self.book.hit(&l, self.pointer) {
            self.book.select(k);
        }
    }

    pub(crate) fn book_scroll(&mut self, d: i32) {
        let l = self.book_layout();
        self.book.scroll(&l, self.pointer, d);
    }

    pub(crate) fn book_draw(&self, p: &mut hud::Paint) {
        draw(&self.book, &self.book_layout(), p);
    }
}

/// Plain drawing over the named rects (the visual pass is the overseer's).
pub(crate) fn draw(b: &Book, l: &Layout, p: &mut hud::Paint) {
    let [x, y, w, h] = l.panel;
    let cyan = hud::CYAN;
    let dim = [0.62, 0.68, 0.76, 0.95];
    p.rect(0.0, 0.0, p.w, p.h, [0.0, 0.01, 0.03, 0.35], 0);
    hud::cut(p, x, y, w, h, [hud::PANEL[0], hud::PANEL[1], hud::PANEL[2], 1.0]);
    hud::glass(p, x, y, w, h);
    hud::brackets(p, [x - 4.0, y - 4.0, w + 8.0, h + 8.0], 14.0, false, cyan);
    p.rect(l.title[0], l.title[1], 3.0, 24.0, cyan, 0);
    p.text_px(l.title[0] + 14.0, l.title[1], "JOURNAL", [1.0; 4], BIG);
    p.rect(l.list[0] + l.list[2] + GAP / 2.0, l.list[1], 1.0, l.list[3], [cyan[0], cyan[1], cyan[2], 0.25], 0);
    // text in the middle of its row, at the size it is drawn
    let ty = |r: [f32; 4], px: f32| r[1] + ((r[3] - 8.0 * px) / 2.0).floor();
    let (ls, ps) = b.scrolls(l);
    let rows = b.rows();
    for (i, r) in rows.iter().skip(ls).take(l.rows).enumerate() {
        let at = l.row(i);
        let text = b.row_text(r, l.row_cols);
        match r {
            Row::Head(_) => p.text_px(at[0], ty(at, TEXT), &text, cyan, TEXT),
            Row::Entry(k) => {
                if b.sel == Some(*k) {
                    p.rect(at[0], at[1], at[2], at[3], [cyan[0], cyan[1], cyan[2], 0.18], 0);
                }
                if b.marked.get(*k).copied().unwrap_or(false) {
                    p.rect(at[0] + 4.0, at[1] + at[3] / 2.0 - 3.0, 6.0, 6.0, cyan, 0);
                }
                let c = if b.state(*k) == State::Known { [1.0; 4] } else { dim };
                p.text_px(at[0] + MARK_W, ty(at, TEXT), &text, c, TEXT);
            }
        }
    }
    // a thin bar where the list or the page runs over its room
    let bar = |p: &mut hud::Paint, r: [f32; 4], pos: usize, fit: usize, n: usize| {
        if n > fit {
            let (bh, by) = (r[3] * fit as f32 / n as f32, r[3] * pos as f32 / n as f32);
            p.rect(r[0] + r[2] - 3.0, r[1] + by, 3.0, bh, [cyan[0], cyan[1], cyan[2], 0.5], 0);
        }
    };
    bar(p, l.list, ls, l.rows, rows.len());
    let (title, lines) = b.page(l);
    if let Some(t) = title {
        p.text_px(l.head[0], ty(l.head, BIG), &t, [1.0; 4], BIG);
    }
    for (i, line) in lines.iter().skip(ps).take(l.lines).enumerate() {
        let at = l.line(i);
        let c = if line.lead.is_some() { cyan } else { [0.9, 0.92, 0.95, 1.0] };
        p.text_px(at[0], ty(at, TEXT), &line.text, c, TEXT);
    }
    bar(p, l.body, ps, l.lines, lines.len());
    p.text_px(l.hint[0], ty(l.hint, TEXT), HINT, dim, TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use State::*;

    fn book(set: &[(&str, State)]) -> Book {
        let mut b = Book::default();
        b.take(&set.iter().map(|(n, s)| (n.to_string(), *s)).collect::<Vec<_>>());
        b
    }

    fn k(name: &str) -> usize {
        JOURNAL.index(name).unwrap()
    }

    fn texts(v: &[Line]) -> String {
        v.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join(" ")
    }

    /// The book's parts at 1280 x 720 and 1920 x 1080 (and the smallest UI a scale of 2 leaves at 1280 x 720):
    /// inside the panel and the screen, the list and the page side by side with no overlap, the page's title over
    /// its lines, the hint under both, room for rows and lines, the text at the HUD's whole size.
    #[test]
    fn the_books_parts_lie_apart_inside_the_panel_at_both_sizes() {
        let within = |a: [f32; 4], b: [f32; 4]| a[0] >= b[0] && a[1] >= b[1] && a[0] + a[2] <= b[0] + b[2] && a[1] + a[3] <= b[1] + b[3];
        assert!(TEXT.fract() == 0.0 && BIG.fract() == 0.0, "whole screen pixels a font pixel");
        assert!(ROW_H >= 8.0 * TEXT + 8.0 && LINE_H >= 8.0 * TEXT + 4.0 && HEAD_H >= 8.0 * BIG + 8.0, "rooms as tall as their text");
        for (lw, lh) in [(1280.0, 720.0), (1920.0, 1080.0), (640.0, 360.0)] {
            let l = layout(lw, lh);
            assert!(within(l.panel, [0.0, 0.0, lw, lh]), "{lw}: the panel on screen");
            for (name, r) in [("title", l.title), ("list", l.list), ("page", l.page), ("hint", l.hint)] {
                assert!(within(r, l.panel), "{lw}: {name} {r:?} in {:?}", l.panel);
            }
            assert!(within(l.head, l.page) && within(l.body, l.page) && l.head[1] + l.head[3] <= l.body[1], "{lw}: the page's title over its lines");
            assert!(l.list[0] + l.list[2] < l.page[0], "{lw}: the list left of the page");
            assert!(l.title[1] + l.title[3] <= l.list[1] && l.list[1] + l.list[3] <= l.hint[1], "{lw}: title, then list and page, then the hint");
            assert!(within(l.row(l.rows - 1), l.list) && within(l.line(l.lines - 1), l.body), "{lw}: the last row and line in their rooms");
            assert!(l.rows >= 6 && l.lines >= 6 && l.cols >= 24 && l.row_cols >= 15, "{lw}: room: {} rows, {} lines, {} columns", l.rows, l.lines, l.cols);
            assert!(HINT.len() as f32 * hud::ADV * TEXT <= l.hint[2], "{lw}: the hint fits");
        }
        let big = layout(1920.0, 1080.0);
        assert_eq!(big.panel[2], MAX_W, "a large screen: the book at its width");
        assert_eq!(layout(1280.0, 720.0).panel[2], MAX_W, "at 1280 x 720 too");
    }

    /// What is listed: unknown entries not at all (no heading, no gap), noticed ones under the open questions by
    /// their question, known ones under their age by their title; a new ape's book is empty.
    #[test]
    fn the_list_holds_only_what_the_ape_has_noticed_or_knows() {
        assert!(book(&[]).rows().is_empty(), "a new ape: nothing listed");
        let b = book(&[("flint", Known), ("hand-axe", Noticed)]);
        assert_eq!(b.rows(), [Row::Head("OPEN QUESTIONS".into()), Row::Entry(k("hand-axe")), Row::Head("THE STEAM AGE".into()), Row::Entry(k("flint"))]);
        assert_eq!(b.row_text(&Row::Entry(k("flint")), 40), "Flint");
        assert_eq!(b.row_text(&Row::Entry(k("hand-axe")), 40), "Struck the right way, could a flint ho..");
        let b = book(&[("clay", Known)]);
        assert_eq!(b.rows(), [Row::Head("THE STEAM AGE".into()), Row::Entry(k("clay"))], "no open questions: no heading");
    }

    /// A page: a known entry's title, text and leads to noticed or known entries only; a noticed one its
    /// question and nothing more; none chosen, a line that says what to do.
    #[test]
    fn a_page_shows_what_its_state_allows() {
        let l = layout(1280.0, 720.0);
        let mut b = book(&[("clay", Known), ("copper", Noticed)]);
        assert!(texts(&b.page(&l).1).starts_with("Choose an entry"));
        b.select(k("clay"));
        let (title, page) = b.page(&l);
        assert_eq!(title.as_deref(), Some("Clay"));
        assert!(texts(&page).contains("Clay is soft when wet"), "{page:?}");
        assert!(page.iter().all(|x| x.lead.is_none()), "the crucible it points at is unknown: no lead");
        let mut b = book(&[("flint", Known), ("hand-axe", Noticed), ("crucible", Noticed)]);
        b.select(k("flint"));
        let leads: Vec<usize> = b.page(&l).1.iter().filter_map(|x| x.lead).collect();
        assert_eq!(leads, [k("hand-axe")], "a lead to the noticed hand axe");
        b.select(k("hand-axe"));
        let (title, page) = b.page(&l);
        assert_eq!((title, texts(&page)), (None, JOURNAL.entries[k("hand-axe")].question.clone()), "a noticed entry: its question alone");
        assert!(texts(&book(&[]).page(&l).1).starts_with("Nothing noticed yet"));
        for line in page {
            assert!(line.text.chars().count() <= l.cols, "wrapped to the page: {line:?}");
        }
    }

    /// The book's wrap: between words, a blank line between paragraphs, a word longer than a line broken across
    /// lines with nothing dropped; the inventory's own `wrap` is untouched (its tests hold it).
    #[test]
    fn the_books_text_keeps_its_paragraphs_and_long_words() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("first part.\n\nsecond\npart.", 20), ["first part.", "", "second part."]);
        assert_eq!(wrap("ab abcdefghij", 4), ["ab", "abcd", "efgh", "ij"]);
        assert_eq!(wrap("  \n\n ", 10), Vec::<String>::new());
    }

    /// Choosing: a click on a row chooses its entry, a click on a heading nothing; a click on a lead chooses
    /// the entry it leads to; the changed mark is set by a change after the first message, not on the entry
    /// open on the page, and goes when chosen.
    #[test]
    fn a_click_chooses_and_the_mark_goes_when_chosen() {
        let l = layout(1280.0, 720.0);
        let mut b = book(&[("flint", Known)]);
        assert!(b.marked.iter().all(|m| !m), "the journal it joined with is not marked");
        assert_eq!(b.take(&[("hand-axe".into(), Noticed)]), ["Journal: Struck the right way, could a flint hold an edge?"]);
        assert!(b.marked[k("hand-axe")] && !b.marked[k("flint")]);
        let mid = |r: [f32; 4]| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        assert_eq!(b.hit(&l, mid(l.row(0))), None, "a heading");
        assert_eq!(b.hit(&l, mid(l.row(1))), Some(k("hand-axe")));
        b.select(k("flint"));
        let lead = b.page(&l).1.iter().position(|x| x.lead.is_some()).unwrap();
        assert_eq!(b.hit(&l, mid(l.line(lead))), Some(k("hand-axe")), "a lead");
        assert!(b.marked[k("hand-axe")], "not chosen yet");
        b.select(k("hand-axe"));
        assert!(!b.marked[k("hand-axe")], "chosen: unmarked");
        // a change to the entry open on the page: in view, so not marked
        b.take(&[("hand-axe".into(), Known)]);
        assert!(!b.marked[k("hand-axe")], "the open entry is not marked");
    }

    /// Scrolling: a list longer than its room scrolls under the pointer to its last row and no further; a page
    /// that fits does not move; the page scrolls when it runs over (a narrow layout).
    #[test]
    fn the_list_and_the_page_scroll_as_far_as_they_run_over() {
        let mut l = layout(1280.0, 720.0);
        let all: Vec<(&str, State)> = JOURNAL.entries.iter().map(|e| (e.name.as_str(), Known)).collect();
        let mut b = book(&all);
        let n = b.rows().len();
        l.rows = 3;
        let over_list = (l.list[0] + 5.0, l.list[1] + 5.0);
        b.scroll(&l, over_list, 100);
        assert_eq!(b.scrolls(&l).0, n - 3, "to the last row");
        b.scroll(&l, over_list, -100);
        assert_eq!(b.scrolls(&l).0, 0);
        b.select(k("copper"));
        let over_page = (l.body[0] + 5.0, l.body[1] + 5.0);
        b.scroll(&l, over_page, 5);
        assert_eq!(b.scrolls(&l).1, 0, "a page that fits stays");
        l.lines = 2;
        b.scroll(&l, over_page, 100);
        assert_eq!(b.scrolls(&l).1, b.page(&l).1.len() - 2);
        assert_eq!(b.scrolls(&l).0, 0, "the list did not move");
    }

    /// The journal shrinking under an open book (the review of draft journal-book): the list scrolled to its end
    /// shows rows again when entries go, and the page scrolled down shows its lines again when the chosen entry
    /// is lowered to unknown (none chosen then) or when the selection changes.
    #[test]
    fn a_journal_that_shrinks_under_the_open_book_leaves_nothing_scrolled_past_its_end() {
        let mut l = layout(1280.0, 720.0);
        let all: Vec<(&str, State)> = JOURNAL.entries.iter().map(|e| (e.name.as_str(), Known)).collect();
        let mut b = book(&all);
        (l.rows, l.lines) = (3, 2);
        b.scroll(&l, (l.list[0] + 5.0, l.list[1] + 5.0), 100);
        b.select(k("copper"));
        b.scroll(&l, (l.body[0] + 5.0, l.body[1] + 5.0), 100);
        assert!(b.scrolls(&l).0 > 0 && b.scrolls(&l).1 > 0);
        let gone: Vec<(String, State)> = JOURNAL.entries.iter().filter(|e| e.name != "flint").map(|e| (e.name.clone(), Unknown)).collect();
        b.take(&gone);
        assert_eq!(b.rows().len(), 2, "a heading and flint");
        assert_eq!(b.scrolls(&l), (0, 0), "the list from its top; copper unknown, the page from its top");
        assert_eq!(b.sel, None, "copper is chosen no more");
        let mid = |r: [f32; 4]| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
        assert_eq!(b.hit(&l, mid(l.row(1))), Some(k("flint")), "the row in view is the one clicked");
    }

    /// The open book's cost on the frame thread: every entry known, copper's page chosen, drawn at 1920 x 1080
    /// (the rects it makes and the time); `--profile play` for the number.
    #[test]
    fn the_open_book_costs_little_a_frame() {
        let all: Vec<(&str, State)> = JOURNAL.entries.iter().map(|e| (e.name.as_str(), Known)).collect();
        let mut b = book(&all);
        b.select(k("copper"));
        let l = layout(1920.0, 1080.0);
        let n = 1000;
        let t0 = std::time::Instant::now();
        let mut rects = 0;
        for _ in 0..n {
            let mut p = hud::Paint::new(1920.0, 1080.0);
            draw(&b, &l, &mut p);
            rects = p.out.len();
        }
        let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
        eprintln!("the open book: {us:.1} us a frame, {rects} rects");
        assert!(us < 2000.0, "{us} us");
    }
}
