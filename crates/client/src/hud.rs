//! What the overlay pipelines draw (`overlay.wgsl`, ADR 0017): the ghost of a shape as lines, and the 2D
//! layer of crosshair and hotbar as rectangles; text is a rectangle per character of the game's own 8 x 8 font (data/font.txt, ADR 0023):
//! the chat, the F3 overlay and the bug report's form.
use engine::edit::Shape;
use engine::world::{FxMap, World};
use glam::IVec3;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GhostVert {
    /// Relative to the body's centre.
    pub pos: [f32; 3],
    pub body: u32,
    pub colour: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rect {
    /// x0, y0, x1, y1 in clip space, y0 below y1.
    pub xy: [f32; 4],
    pub colour: [f32; 4],
    /// 0 solid, 1..10 the digits 0..9 of the small font, 32..126 that ASCII character of the font.
    pub glyph: u32,
}

/// The most ghost vertices: every edge of every cell of the largest shape.
pub const GHOST_MAX: usize = engine::edit::MAX_CELLS as usize * 24;
/// Enough for F3, a full chat being typed, the hotbar's and the hand's cubes (192 rectangles each) and
/// sixteen waypoints together: about 11,000 at the most, two rectangles (shadow and face) a character.
pub const RECTS_MAX: usize = 16384;

/// The game's own font (data/font.txt): 8 x 8 cells from ' ' to '~', a byte a row from the top, the
/// leftmost pixel the lowest bit. Errors name what is wrong with the file.
pub fn font_cells(text: &str) -> Result<Vec<[u8; 8]>, String> {
    let mut drawn: FxMap<char, Vec<u8>> = FxMap::default();
    let mut at: Option<char> = None;
    for line in text.lines().map(str::trim_end) {
        let mut cs = line.chars();
        if let (Some(c), None) = (cs.next(), cs.next()) {
            at = Some(c);
            drawn.insert(c, Vec::new());
        } else if !line.is_empty() && line.chars().all(|c| c == '#' || c == '.') {
            let c = at.ok_or("a row before the first character")?;
            if line.len() > 6 {
                return Err(format!("{c:?}: a row wider than 6"));
            }
            // one column in from the cell's left
            drawn.get_mut(&c).unwrap().push(line.chars().enumerate().filter(|(_, p)| *p == '#').fold(0, |b, (k, _)| b | 2 << k));
        }
    }
    (' '..='~')
        .map(|c| {
            let mut cell = [0u8; 8];
            let rows = match (drawn.get(&c), drawn.get(&c.to_ascii_uppercase())) {
                (Some(r), _) => r.clone(),
                // a small capital: the capital without rows 1 and 4
                (None, Some(r)) if c.is_ascii_lowercase() && r.len() == 7 => [0, 2, 3, 5, 6].map(|k| r[k]).to_vec(),
                (None, _) if c == ' ' => vec![0; 7],
                (None, _) => return Err(format!("{c:?} is not drawn")),
            };
            match rows.len() {
                7 => cell[..7].copy_from_slice(&rows),
                5 => cell[2..7].copy_from_slice(&rows),
                8 => cell.copy_from_slice(&rows),
                n => return Err(format!("{c:?} has {n} rows, not 7 (5 for a small letter, 8 for one with a tail)")),
            }
            Ok(cell)
        })
        .collect()
}

/// The font for `overlay.wgsl`, as WGSL: two words a character from ' ' to '~'.
pub fn font_wgsl() -> String {
    let cells = font_cells(include_str!("../../../data/font.txt")).unwrap_or_else(|e| panic!("data/font.txt: {e}"));
    let words: Vec<String> = cells
        .iter()
        .flat_map(|g| [u32::from_le_bytes([g[0], g[1], g[2], g[3]]), u32::from_le_bytes([g[4], g[5], g[6], g[7]])])
        .map(|w| format!("{w}u"))
        .collect();
    format!("const FONT = array<u32, {}>({});\n", words.len(), words.join(", "))
}

/// Draws into clip space from pixels measured from the top-left of a `w` x `h` screen.
pub struct Paint {
    pub w: f32,
    pub h: f32,
    pub out: Vec<Rect>,
}

/// The places and the colours their names are written in: Terra green, Luna a light grey, Sol yellow.
/// shortcut: a table here; a colour per row of data/bodies.toml when there are more bodies than these.
pub const PLACES: [(&str, [f32; 3]); 3] = [("Terra", [0.42, 0.86, 0.45]), ("Luna", [0.82, 0.84, 0.88]), ("Sol", [1.0, 0.85, 0.25])];

/// For each character of `s`, the colour of the place whose name it is part of: whole words, any case
/// ("Terra's" and "LUNA" count, "solid" does not).
pub fn place_colours(s: &str) -> Vec<Option<[f32; 3]>> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = vec![None; chars.len()];
    let mut k = 0;
    while k < chars.len() {
        if !chars[k].is_ascii_alphabetic() {
            k += 1;
            continue;
        }
        let end = (k..chars.len()).find(|j| !chars[*j].is_ascii_alphabetic()).unwrap_or(chars.len());
        let word: String = chars[k..end].iter().collect();
        if let Some((_, c)) = PLACES.iter().find(|p| p.0.eq_ignore_ascii_case(&word)) {
            out[k..end].fill(Some(*c));
        }
        k = end;
    }
    out
}

/// Font pixels from one character to the next: a glyph is 5 wide, so one pixel stands between two. (A glyph
/// is still drawn as its whole 8 x 8 cell, one column in; the cells' empty columns overlap.)
pub const ADV: f32 = 6.0;

/// Screen pixels per font pixel, and the height of a line of text.
pub const PX: f32 = 2.0;
pub const LINE: f32 = 10.0 * PX + 2.0;

impl Paint {
    pub fn new(w: f32, h: f32) -> Paint {
        Paint { w, h, out: Vec::new() }
    }

    pub fn rect(&mut self, x: f32, y: f32, rw: f32, rh: f32, colour: [f32; 4], glyph: u32) {
        let (x0, x1) = (x / self.w * 2.0 - 1.0, (x + rw) / self.w * 2.0 - 1.0);
        let (y0, y1) = (1.0 - (y + rh) / self.h * 2.0, 1.0 - y / self.h * 2.0);
        self.out.push(Rect { xy: [x0, y0, x1, y1], colour, glyph });
    }

    /// One line of text with its top-left at (x, y) and a shadow under it; anything but printable ASCII
    /// shows as '?'.
    pub fn text(&mut self, x: f32, y: f32, s: &str, colour: [f32; 4]) {
        self.text_px(x, y, s, colour, PX);
    }

    /// Text `px` screen pixels a font pixel.
    pub fn text_px(&mut self, x: f32, y: f32, s: &str, colour: [f32; 4], px: f32) {
        self.glyphs(x, y, s, colour, px, true);
    }

    /// Text with no shadow: dark text on a light box.
    pub fn text_flat(&mut self, x: f32, y: f32, s: &str, colour: [f32; 4], px: f32) {
        self.glyphs(x, y, s, colour, px, false);
    }

    fn glyphs(&mut self, x: f32, y: f32, s: &str, colour: [f32; 4], px: f32, shadow: bool) {
        let size = 8.0 * px;
        // a place's name is written in the place's own colour wherever it stands (the developer, 2026-11-11);
        // not in dark text on a lit button, where the ink is the contrast
        let places = if shadow { place_colours(s) } else { Vec::new() };
        for (k, ch) in s.chars().enumerate() {
            let colour = match places.get(k) {
                Some(Some(c)) => [c[0], c[1], c[2], colour[3]],
                _ => colour,
            };
            let g = if (' '..='~').contains(&ch) { ch as u32 } else { '?' as u32 };
            if g == ' ' as u32 {
                continue;
            }
            let cx = x + k as f32 * ADV * px;
            if shadow {
                self.rect(cx + px, y + px, size, size, [0.0, 0.0, 0.0, colour[3] * 0.7], g);
            }
            self.rect(cx, y, size, size, colour, g);
        }
    }
}

pub fn text_width(s: &str) -> f32 {
    s.chars().count() as f32 * ADV * PX
}

/// `s` broken into lines of at most `cols` characters, at spaces where it can.
pub fn wrap(s: &str, cols: usize) -> Vec<String> {
    // a line that fits keeps its spaces, so indented columns stay lined up
    if s.chars().count() <= cols {
        return vec![s.to_string()];
    }
    let mut out = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        let mut word: Vec<char> = word.chars().collect();
        // a word longer than a line is cut
        while word.len() > cols {
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            out.push(word.drain(..cols).collect());
        }
        let word: String = word.into_iter().collect();
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > cols {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

/// The chat's width in characters, how long a line stays, and the lines shown while not typing.
pub const CHAT_COLS: usize = 56;
pub const CHAT_SECS: f32 = 10.0;
pub const CHAT_LINES: usize = 10;

/// A chat line's colour: a player's words white, an echoed command grey, the game's answers and news yellow.
fn chat_colour(line: &str) -> [f32; 3] {
    if line.starts_with('<') {
        [1.0, 1.0, 1.0]
    } else if line.starts_with("> ") {
        [0.62, 0.62, 0.62]
    } else {
        [1.0, 0.9, 0.55]
    }
}

/// The chat in the bottom-left: `lines` (oldest first, with their age in seconds), the newest at the bottom
/// over the hotbar. While `typing` holds the line being written, the bar runs along the bottom with a caret
/// and older lines show too; otherwise a line fades out after CHAT_SECS.
/// `hints`: the command line's completions from byte `at` of the typed line, `pick` the one Tab takes (None:
/// the list is hidden).
pub fn chat(p: &mut Paint, lines: &[(f32, &str)], typing: Option<&str>, caret: bool, at: usize, hints: &[String], pick: Option<usize>) {
    let mut rows: Vec<(f32, [f32; 3], String)> = Vec::new();
    for (age, l) in lines {
        let a = if typing.is_some() { 1.0 } else { ((CHAT_SECS - age) / 2.0).clamp(0.0, 1.0) };
        if a > 0.0 {
            let c = chat_colour(l);
            rows.extend(wrap(l, CHAT_COLS).into_iter().map(|r| (a, c, r)));
        }
    }
    let keep = if typing.is_some() { CHAT_LINES * 2 } else { CHAT_LINES };
    let rows = &rows[rows.len().saturating_sub(keep)..];
    let width = rows.iter().map(|r| text_width(&r.2)).fold(0.0, f32::max) + 22.0;
    let bottom = p.h - 128.0;
    for (k, (a, c, r)) in rows.iter().rev().enumerate() {
        let y = bottom - (k + 1) as f32 * LINE;
        p.rect(4.0, y, width, LINE, [PANEL[0], PANEL[1], PANEL[2], PANEL[3] * a], 0);
        p.rect(4.0, y, 3.0, LINE, [CYAN[0], CYAN[1], CYAN[2], 0.8 * a], 0);
        p.text(12.0, y + 2.0, r, [c[0], c[1], c[2], *a]);
        // who said it, in the accent, over the line's own white
        if let Some(end) = r.find('>').filter(|_| r.starts_with('<')) {
            p.text(12.0, y + 2.0, &r[..=end], [CYAN[0], CYAN[1], CYAN[2], *a]);
        }
    }
    if let Some(t) = typing {
        // the end of a long line shows
        let cols = ((p.w - 24.0) / (ADV * PX)) as usize - 1;
        let shown: String = t.chars().skip(t.chars().count().saturating_sub(cols)).collect();
        let y = p.h - LINE - 8.0;
        p.rect(4.0, y - 2.0, p.w - 8.0, LINE + 4.0, [PANEL[0], PANEL[1], PANEL[2], 0.9], 0);
        p.rect(4.0, y - 2.0, 3.0, LINE + 4.0, CYAN, 0);
        p.text(12.0, y + 2.0, &shown, [1.0, 1.0, 1.0, 1.0]);
        if caret {
            p.text(12.0 + text_width(&shown), y + 2.0, "_", [1.0, 1.0, 1.0, 1.0]);
        }
        // the completions over the bar, under the word they complete, the picked one yellow
        if let (Some(k), false) = (pick, hints.is_empty()) {
            const SHOW: usize = 10;
            let first = k.saturating_sub(SHOW - 1).min(hints.len().saturating_sub(SHOW));
            let list = &hints[first..(first + SHOW).min(hints.len())];
            let x = 12.0 + if shown.len() == t.len() { text_width(&t[..at.min(t.len())]) } else { 0.0 };
            let wide = list.iter().map(|h| text_width(h)).fold(0.0, f32::max) + 8.0;
            for (r, h) in list.iter().enumerate() {
                let hy = y - (list.len() - r) as f32 * LINE;
                p.rect(x - 4.0, hy, wide, LINE, [PANEL[0], PANEL[1], PANEL[2], 0.92], 0);
                p.text(x, hy + 2.0, h, if first + r == k { [1.0, 0.9, 0.3, 1.0] } else { [0.75, 0.75, 0.75, 1.0] });
            }
        }
    }
}

/// The F3 overlay: lines down the left and, right-aligned, down the right, each on a grey band.
pub fn panels(p: &mut Paint, left: &[Section], right: &[Section]) {
    // the text as large as lets the two columns stand side by side and on the screen: 2, 1.5 or 1 pixels a dot
    let px = [PX, PX * 0.75, PX * 0.5].into_iter().find(|px| {
        let (l, r) = (panels_size(left, *px), panels_size(right, *px));
        // side by side, the left column clear of the chat under it
        l.0 + r.0 + 24.0 <= p.w && l.1 + CHAT_LINES as f32 * LINE + 60.0 <= p.h && r.1 + 6.0 <= p.h
    });
    let px = px.unwrap_or(PX * 0.5);
    let line = 10.0 * px + 2.0;
    for (side, list) in [left, right].into_iter().enumerate() {
        let mut y = 6.0;
        // every panel of a column as wide as its widest, so the column has one edge
        let w = panels_size(list, px).0;
        for s in list {
            let (_, h, label_w) = section_size(s, px);
            let x = if side == 0 { 6.0 } else { p.w - w - 6.0 };
            glass(p, x, y, w, h);
            p.rect(x, y + 2.0, 3.0, h - 4.0, CYAN, 0);
            p.text_px(x + 10.0, y + 5.0, &s.title.to_uppercase(), CYAN, px);
            for (k, (label, value, tone)) in s.rows.iter().enumerate() {
                let ry = y + 5.0 + (k + 1) as f32 * line;
                p.text_px(x + 10.0, ry, label, [0.62, 0.72, 0.82, 1.0], px);
                p.text_px(x + 10.0 + label_w, ry, value, TONES[(*tone as usize).min(3)], px);
            }
            if let Some((bars, mark)) = &s.graph {
                let top = y + 5.0 + (s.rows.len() + 1) as f32 * line + 4.0;
                let (base, graph_w) = (top + GRAPH_H, bars.len() as f32 * BAR);
                p.rect(x + 10.0, top, graph_w, GRAPH_H, [1.0, 1.0, 1.0, 0.06], 0);
                for (i, b) in bars.iter().enumerate() {
                    let tone = if *b > 1.0 { 3 } else if b > mark { 2 } else { 1 };
                    let bh = (b.clamp(0.0, 1.0) * GRAPH_H).max(1.0);
                    p.rect(x + 10.0 + i as f32 * BAR, base - bh, BAR, bh, TONES[tone], 0);
                }
                p.rect(x + 10.0, base - mark * GRAPH_H, graph_w, 1.0, [1.0, 1.0, 1.0, 0.55], 0);
            }
            y += h + 8.0;
        }
    }
}

/// A section's width and height at `px` pixels a dot, and where its values start after the labels: in one
/// column, two characters after the longest label.
fn section_size(s: &Section, px: f32) -> (f32, f32, f32) {
    let (glyph, line) = (ADV * px, 10.0 * px + 2.0);
    let chars = |t: &str| t.chars().count() as f32 * glyph;
    let label_w = (s.rows.iter().map(|r| r.0.len()).max().unwrap_or(0) + 2) as f32 * glyph;
    let rows_w = s.rows.iter().map(|r| label_w + chars(&r.1)).fold(chars(s.title), f32::max);
    let graph = s.graph.as_ref().map_or((0.0, 0.0), |g| (g.0.len() as f32 * BAR, GRAPH_H + 10.0));
    (rows_w.max(graph.0) + 20.0, (s.rows.len() + 1) as f32 * line + graph.1 + 10.0, label_w)
}

/// A column of sections: its width and its height with the gaps between them.
fn panels_size(list: &[Section], px: f32) -> (f32, f32) {
    list.iter().map(|s| section_size(s, px)).fold((0.0, 6.0), |a, s| (a.0.max(s.0), a.1 + s.1 + 8.0))
}

/// What the crosshair is on, top middle: a swatch of the block's colour, its name, and under it what is
/// worth knowing about it (empty for most blocks).
pub fn looking_at(p: &mut Paint, name: &str, colour: [f32; 3], side: [f32; 3], pattern: u8, note: &str, y: f32) {
    let w = text_width(name).max(text_width(note)) + 34.0 + 20.0;
    let h = if note.is_empty() { LINE + 12.0 } else { 2.0 * LINE + 12.0 };
    let x = (p.w - w) / 2.0;
    glass(p, x, y, w, h);
    brackets(p, [x - 4.0, y - 4.0, w + 8.0, h + 8.0], 8.0, false, CYAN);
    cube(&mut |x, y, w, h, c, g| p.rect(x, y, w, h, c, g), x + 6.0, y + (h - 16.0) / 2.0, 16.0, colour, side, pattern, 1.0);
    p.text(x + 10.0 + 34.0, y + 7.0, name, [0.95, 0.95, 0.95, 1.0]);
    if !note.is_empty() {
        p.text(x + 10.0 + 34.0, y + 7.0 + LINE, note, [0.62, 0.72, 0.82, 1.0]);
    }
}

/// A group of F3's numbers: a heading, rows of label and value, and a bar graph under them.
pub struct Section {
    pub title: &'static str,
    /// Label, value, and how the value stands: 0 plain, 1 good, 2 near its limit, 3 over it.
    pub rows: Vec<(&'static str, String, u8)>,
    /// Bars, the newest last, each a share of the graph's height (over 1 is cut off and red), and the height
    /// of the line across it (what a bar should stay under).
    pub graph: Option<(Vec<f32>, f32)>,
}

/// A value's colour by how it stands (`Section::rows`): plain, good, near its limit, over it.
const TONES: [[f32; 4]; 4] = [[0.95, 0.95, 0.95, 1.0], [0.5, 0.9, 0.55, 1.0], [1.0, 0.8, 0.35, 1.0], [1.0, 0.42, 0.38, 1.0]];
/// A graph's height and one bar's width, in pixels.
const GRAPH_H: f32 = 48.0;
const BAR: f32 = 2.0;

/// The dark glass every panel of the HUD is made of: F3's groups, the chat, the hotbar, the labels.
pub const PANEL: [f32; 4] = [0.03, 0.05, 0.08, 0.72];

/// The HUD's accent colour (the menus' look, ADR 0023: HUD brackets).
pub const CYAN: [f32; 4] = [0.44, 0.83, 1.0, 1.0];

/// L-shaped corner brackets `arm` pixels long round a box: top-left and bottom-right, or all four corners.
pub fn brackets(p: &mut Paint, b: [f32; 4], arm: f32, all: bool, colour: [f32; 4]) {
    let ([x, y, w, h], t) = (b, 2.0);
    let corners: &[(bool, bool)] = if all { &[(false, false), (true, false), (false, true), (true, true)] } else { &[(false, false), (true, true)] };
    for &(right, low) in corners {
        let (cx, cy) = (if right { x + w - arm } else { x }, if low { y + h - t } else { y });
        p.rect(cx, cy, arm, t, colour, 0);
        let (vx, vy) = (if right { x + w - t } else { x }, if low { y + h - arm } else { y });
        p.rect(vx, vy, t, arm, colour, 0);
    }
}

/// The settings' high contrast interface: the menu's shade nearly solid, its numbers full strength.
static CONTRAST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn set_contrast(on: bool) {
    CONTRAST.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Screen pixels a font pixel of the menu's buttons.
const MENU_PX: f32 = PX * 1.5;

/// A menu row that is a slider: where its value sits on the track (0..1), the value as text, and whether that
/// text is being typed (its number was clicked).
pub type Slider = Option<(f32, String, bool)>;

/// A slider's track, after its label, and the room its number takes after that.
const TRACK: f32 = 320.0;
const NUMBER: f32 = 7.0 * ADV * MENU_PX;
/// Slider labels take at least this many characters' room, so the tracks line up (the longest one's).
const SLIDER_LABEL: usize = 17;

/// The menu's buttons on a `h` pixels high screen: x, y, width and height in pixels, a column on the left,
/// each as wide as its number and label (and a slider's track and value): what is drawn lit takes the click.
pub fn menu_boxes(h: f32, items: &[String], sliders: &[Slider]) -> Vec<[f32; 4]> {
    let (bh, gap, glyph) = (50.0, 10.0, ADV * MENU_PX);
    let top = (h * 0.36).max(230.0);
    let width = |k: usize, l: &String| {
        let slider = if sliders.get(k).is_some_and(|s| s.is_some()) { 24.0 + TRACK + 24.0 + NUMBER } else { 0.0 };
        let chars = if slider > 0.0 { l.chars().count().max(SLIDER_LABEL) } else { l.chars().count() };
        20.0 + 4.0 * glyph + chars as f32 * glyph + slider + 12.0
    };
    items.iter().enumerate().map(|(k, l)| [96.0, top + k as f32 * (bh + gap), width(k, l), bh]).collect()
}

/// A slider row's track and number, each x, y, width and height, from its box and label.
pub fn slider_parts(b: [f32; 4], label: &str) -> ([f32; 4], [f32; 4]) {
    let glyph = ADV * MENU_PX;
    let x = b[0] + 20.0 + 4.0 * glyph + label.chars().count().max(SLIDER_LABEL) as f32 * glyph + 24.0;
    ([x, b[1], TRACK, b[3]], [x + TRACK + 24.0, b[1], NUMBER, b[3]])
}

/// The game's name: the title page draws it as the logo instead of plain text.
pub const GAME_NAME: &str = "TechnoApe";

/// The logo (`logo` below): the font's capitals `px` pixels a dot, the O a planet with a moon's orbit.
pub fn logo(p: &mut Paint, x: f32, y: f32, px: f32) {
    let cells = font_cells(include_str!("../../../data/font.txt")).unwrap_or_else(|e| panic!("data/font.txt: {e}"));
    for (k, ch) in "TECHNOAPE".chars().enumerate() {
        let cx = x + k as f32 * 7.0 * px;
        if ch == 'O' {
            continue; // the planet, after the letters, so its orbit crosses over them
        }
        let cell = cells[ch as usize - 32];
        // a navy shadow down and right, then the dots: white above, cyan in the lower three rows
        for pass in 0..2 {
            for (row, bits) in cell.iter().enumerate() {
                for col in 0..8 {
                    if bits >> col & 1 == 0 {
                        continue;
                    }
                    let (dx, dy) = (cx + (col as f32 - 1.0) * px, y + row as f32 * px);
                    if pass == 0 {
                        p.rect(dx + px * 0.35, dy + px * 0.35, px, px, [0.0, 0.05, 0.15, 0.85], 0);
                    } else {
                        p.rect(dx, dy, px, px, if row < 4 { [0.96, 0.98, 1.0, 1.0] } else { CYAN }, 0);
                    }
                }
            }
        }
    }
    planet(p, x + 5.0 * 7.0 * px + 2.5 * px, y + 3.5 * px, 3.5 * px);
}

/// A planet of radius `r` at (x, y): a dark disc lit from the upper right, a tilted orbit through it whose
/// back half it hides, and a moon on the near half.
fn planet(p: &mut Paint, x: f32, y: f32, r: f32) {
    let s = (r / 8.0).max(1.0);
    let ring = |p: &mut Paint, front: bool| {
        let (a, b, tilt) = (r * 1.75, r * 0.42, -0.35f32);
        for k in 0..96 {
            let t = k as f32 / 96.0 * std::f32::consts::TAU;
            if (t.sin() > 0.0) != front {
                continue;
            }
            let (ex, ey) = (a * t.cos(), b * t.sin());
            let (rx, ry) = (ex * tilt.cos() - ey * tilt.sin(), ex * tilt.sin() + ey * tilt.cos());
            p.rect(x + rx - s * 0.6, y + ry - s * 0.6, s * 1.2, s * 1.2, [0.44, 0.83, 1.0, 0.9], 0);
        }
    };
    ring(p, false);
    let n = (2.0 * r / s).ceil() as i32;
    for j in 0..n {
        let dy = -r + (j as f32 + 0.5) * s;
        let half = (r * r - dy * dy).max(0.0).sqrt();
        p.rect(x - half, y + dy - s / 2.0, 2.0 * half, s, [0.07, 0.2, 0.36, 1.0], 0);
        // the lit side: a disc offset towards the light, cut to the planet
        let (lx, ly) = (r * 0.35, -r * 0.35);
        let ld = dy - ly;
        let lh = (r * r * 0.8 - ld * ld).max(0.0).sqrt();
        let (l0, l1) = ((x + lx - lh).max(x - half), (x + lx + lh).min(x + half));
        if l1 > l0 {
            p.rect(l0, y + dy - s / 2.0, l1 - l0, s, [0.2, 0.55, 0.78, 1.0], 0);
        }
    }
    ring(p, true);
    let (mx, my) = (x - r * 1.45, y + r * 0.85);
    p.rect(mx - s * 1.5, my - s * 1.5, s * 3.0, s * 3.0, [0.96, 0.98, 1.0, 1.0], 0);
}

/// The menu (the title's and Esc's): a shade fading out from the left, the page's title large with a
/// readout under it (the title page's is the logo), and a numbered list; the button under the pointer a
/// solid cyan box in corner brackets.
/// `first` is the number of rows above the ones shown (the page scrolled). `ease` is how far the page has
/// come in and how far the lit row has slid out, each 0 to 1 over about a tenth of a second.
/// `tones` marks rows: 1 a warning (a key two actions share), drawn red; 2 a heading, drawn as plain text.
pub fn menu(p: &mut Paint, title: &str, items: &[String], sliders: &[Slider], tones: &[u8], hint: &str, hover: Option<usize>, caret: bool, first: usize, ease: (f32, f32)) {
    // the world a little darker under any menu but the title's, whose backdrop is the picture
    if title != GAME_NAME {
        p.rect(0.0, 0.0, p.w, p.h, [0.0, 0.01, 0.03, 0.35], 0);
    }
    let smooth = |t: f32| {
        let t = t.clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    let contrast = CONTRAST.load(std::sync::atomic::Ordering::Relaxed);
    let dark = if contrast { 0.94 } else { 0.45 };
    let boxes = menu_boxes(p.h, items, sliders);
    let (x, top) = (96.0, boxes.first().map_or(p.h * 0.36, |b| b[1]));
    let shade = x + 900.0;
    p.rect(0.0, 0.0, shade, p.h, [0.0, 0.02, 0.05, dark], 0);
    for k in 0..16 {
        p.rect(shade + k as f32 * 16.0, 0.0, 16.0, p.h, [0.0, 0.02, 0.05, dark * (1.0 - (k as f32 + 1.0) / 17.0)], 0);
    }
    if title == GAME_NAME {
        logo(p, x, top - 190.0, 12.0);
        p.text_px(x + 4.0, top - 82.0, hint, [0.7, 0.8, 0.9, 1.0], PX * 1.25);
    } else {
        p.rect(x, top - 150.0, 3.0, 84.0, CYAN, 0);
        p.text_px(x + 22.0, top - 150.0, &title.to_uppercase(), [1.0, 1.0, 1.0, 1.0], PX * 3.0);
        p.text_px(x + 22.0, top - 92.0, hint, [0.7, 0.8, 0.9, 1.0], PX * 1.25);
    }
    let lp = MENU_PX;
    for (k, (b, label)) in boxes.iter().zip(items).enumerate() {
        let lit = hover == Some(k);
        // the rows come in from the left one after another; the lit one stands out a little to the right
        let dx = -36.0 * (1.0 - smooth(ease.0 * (1.0 + 0.12 * items.len() as f32) - 0.12 * k as f32)) + if lit { 8.0 * smooth(ease.1) } else { 0.0 };
        let b = &[b[0] + dx, b[1], b[2], b[3]];
        let ty = b[1] + (b[3] - 7.0 * lp) / 2.0;
        let tone = tones.get(k).copied().unwrap_or(0);
        if tone == 2 {
            p.text_px(b[0] + 4.0, ty + 6.0, &label.to_uppercase(), CYAN, PX * 1.25);
            continue;
        }
        let red = [1.0, 0.42, 0.38, 1.0];
        if !lit {
            cut(p, b[0], b[1], b[2], b[3], [PANEL[0], PANEL[1], PANEL[2], 0.5]);
            let bar = if tone == 1 { red } else { [CYAN[0], CYAN[1], CYAN[2], 0.4] };
            p.rect(b[0], b[1] + 2.0, 3.0, b[3] - 4.0, bar, 0);
        }
        let tw = b[2];
        if lit {
            // the lit row glows as the slot in hand does
            for (grow, a) in [(8.0, 0.07), (5.0, 0.12), (3.0, 0.2)] {
                cut(p, b[0] - grow, b[1] - grow, tw + 2.0 * grow, b[3] + 2.0 * grow, [CYAN[0], CYAN[1], CYAN[2], a]);
            }
            cut(p, b[0], b[1], tw, b[3], CYAN);
            brackets(p, [b[0] - 6.0, b[1] - 6.0, tw + 12.0, b[3] + 12.0], 12.0, false, CYAN);
        }
        let (n, lx) = (format!("{:02}", first + k + 1), b[0] + 20.0 + 4.0 * ADV * lp);
        if lit {
            let ink = [0.01, 0.06, 0.12, 1.0];
            p.text_flat(b[0] + 20.0, ty, &n, ink, lp);
            p.text_flat(lx, ty, label, ink, lp);
        } else {
            p.text_px(b[0] + 20.0, ty, &n, [0.44, 0.83, 1.0, if contrast { 1.0 } else { 0.6 }], lp);
            p.text_px(lx, ty, label, if tone == 1 { red } else { [0.92, 0.95, 0.98, 0.95] }, lp);
        }
        if let Some(Some((at, value, typing))) = sliders.get(k) {
            let (t, n) = slider_parts(*b, label);
            let (ink, dim) = if lit { ([0.01, 0.06, 0.12, 1.0], [0.01, 0.06, 0.12, 0.35]) } else { (CYAN, [0.44, 0.83, 1.0, 0.25]) };
            let (cy, at) = (t[1] + t[3] / 2.0, at.clamp(0.0, 1.0));
            p.rect(t[0], cy - 2.0, t[2], 4.0, dim, 0);
            p.rect(t[0], cy - 2.0, t[2] * at, 4.0, ink, 0);
            p.rect(t[0] + t[2] * at - 5.0, cy - 12.0, 10.0, 24.0, if lit { ink } else { [0.96, 0.98, 1.0, 1.0] }, 0);
            let shown = if *typing && caret { format!("{value}_") } else { value.clone() };
            if *typing {
                // the number being typed sits in its own dark field, bracketed
                p.rect(n[0] - 8.0, n[1] + 4.0, n[2], n[3] - 8.0, [0.02, 0.04, 0.07, 0.95], 0);
                brackets(p, [n[0] - 8.0, n[1] + 4.0, n[2], n[3] - 8.0], 8.0, true, CYAN);
                p.text_px(n[0], ty, &shown, [1.0, 1.0, 1.0, 1.0], lp);
            } else if lit {
                p.text_flat(n[0], ty, &shown, ink, lp);
            } else {
                p.text_px(n[0], ty, &shown, [0.92, 0.95, 0.98, 0.95], lp);
            }
        }
    }
}

/// The bug report's form (F9): a panel in the middle of the screen with the text written so far.
pub fn bug_form(p: &mut Paint, text: &str, caret: bool) {
    let cols = 60;
    let (pw, ph) = (cols as f32 * ADV * PX + 40.0, 15.0 * LINE + 50.0);
    let (x, y) = ((p.w - pw) / 2.0, (p.h - ph) / 2.0);
    p.rect(0.0, 0.0, p.w, p.h, [0.0, 0.0, 0.0, 0.35], 0);
    p.rect(x, y, pw, ph, [0.02, 0.04, 0.07, 0.9], 0);
    brackets(p, [x, y, pw, ph], 16.0, true, CYAN);
    p.text_px(x + 20.0, y + 16.0, "BUG REPORT", [1.0, 1.0, 1.0, 1.0], PX * 1.5);
    p.text(x + 20.0, y + 20.0 + LINE * 2.0, "What went wrong? A screenshot and the game's state", [0.7, 0.76, 0.82, 1.0]);
    p.text(x + 20.0, y + 20.0 + LINE * 3.0, "are saved with it. Enter saves, Esc cancels.", [0.7, 0.76, 0.82, 1.0]);
    let top = y + 20.0 + LINE * 4.6;
    p.rect(x + 14.0, top - 4.0, pw - 28.0, LINE * 10.0 + 8.0, [0.44, 0.83, 1.0, 0.06], 0);
    p.rect(x + 14.0, top - 4.0, 2.0, LINE * 10.0 + 8.0, [0.44, 0.83, 1.0, 0.5], 0);
    let mut lines = wrap(text, cols);
    if caret {
        let last = lines.last_mut().unwrap();
        if last.chars().count() < cols {
            last.push('_');
        } else {
            lines.push("_".into());
        }
    }
    let lines = &lines[lines.len().saturating_sub(10)..];
    for (k, l) in lines.iter().enumerate() {
        p.text(x + 20.0, top + k as f32 * LINE, l, [1.0, 1.0, 1.0, 1.0]);
    }
}

/// The edges of every face of the shape that has no shape cell beside it: its outline on the curved grid.
pub fn ghost(world: &World, s: &Shape, colour: [u8; 4]) -> Vec<GhostVert> {
    let cells: FxMap<IVec3, ()> = s.cells().map(|b| (b.pos, ())).collect();
    let body = &world.bodies[s.body as usize];
    let corner = |p: IVec3| {
        let v = body.dir(s.face as usize, p.x as f64, p.y as f64) * body.layer_r(p.z as f64);
        GhostVert { pos: v.as_vec3().to_array(), body: s.body as u32, colour }
    };
    let mut out = Vec::new();
    for p in cells.keys() {
        for a in 0..3 {
            for side in [0, 1] {
                let mut e = IVec3::ZERO;
                e[a] = side * 2 - 1;
                if cells.contains_key(&(*p + e)) {
                    continue;
                }
                let (mut u, mut v) = (IVec3::ZERO, IVec3::ZERO);
                u[(a + 1) % 3] = 1;
                v[(a + 2) % 3] = 1;
                let mut o = *p;
                o[a] += side;
                let q = [o, o + u, o + u + v, o + v];
                for k in 0..4 {
                    out.push(corner(q[k]));
                    out.push(corner(q[(k + 1) % 4]));
                }
            }
        }
        if out.len() >= GHOST_MAX {
            out.truncate(GHOST_MAX);
            break;
        }
    }
    out
}

/// One hotbar slot: the block's colour and, in survival, how many are held.
/// The compass (docs/design/navigation.md): a strip top middle showing 60 degrees either side of `heading`
/// (degrees east of north). The four winds by their letters, north in the accent, the four between them
/// small, a number every 30 degrees elsewhere, a tick every 15; all fade towards the ends. A pointer in the
/// accent marks straight ahead. `marks` are waypoints at their bearings in their colours, held dim at the
/// strip's end when further round. Under it the heading in degrees and `text` (where the player is).
/// Returns where it ends.
pub fn compass(p: &mut Paint, heading: f64, marks: &[(f64, [f32; 3], usize)], text: &str) -> f32 {
    let (w, h, y) = (560.0, 40.0, 10.0);
    let (x, mid) = ((p.w - w) / 2.0, p.w / 2.0);
    glass(p, x, y, w, h);
    // a bearing's place on the strip in degrees from straight ahead, and how strongly it shows there
    let at = |b: f64| ((b - heading + 540.0).rem_euclid(360.0) - 180.0) as f32;
    let fade = |d: f32| 1.0 - (d.abs() / 60.0).powi(2) * 0.9;
    let dim = [0.62, 0.72, 0.82];
    for k in 0..24 {
        let d = at(k as f64 * 15.0);
        if d.abs() > 57.0 {
            continue;
        }
        let (tx, a) = (mid + d / 60.0 * (w / 2.0), fade(d));
        // the tick along the strip's floor, longer under a letter or a number
        let tall = if k % 2 == 0 { 7.0 } else { 4.0 };
        p.rect(tx - 1.0, y + h - 3.0 - tall, 2.0, tall, [dim[0], dim[1], dim[2], 0.85 * a], 0);
        match k {
            0 | 6 | 12 | 18 => {
                let c = if k == 0 { CYAN } else { [0.96, 0.97, 1.0, 1.0] };
                p.text(tx - 7.0, y + 6.0, ["N", "E", "S", "W"][k / 6], [c[0], c[1], c[2], a]);
            }
            3 | 9 | 15 | 21 => {
                let px = PX * 0.75;
                p.text_px(tx - ADV * px, y + 9.0, ["NE", "SE", "SW", "NW"][k / 6], [dim[0], dim[1], dim[2], a], px);
            }
            _ if k % 2 == 0 => {
                let (px, t) = (PX * 0.5, format!("{}", k * 15));
                p.text_px(tx - t.len() as f32 * ADV / 2.0 * px, y + 12.0, &t, [dim[0], dim[1], dim[2], 0.8 * a], px);
            }
            _ => {}
        }
    }
    for (b, c, kind) in marks {
        let d = at(*b);
        let tx = mid + d.clamp(-60.0, 60.0) / 60.0 * (w / 2.0 - 8.0);
        let a = if d.abs() > 60.0 { 0.5 } else { 1.0 };
        // a hairline up the strip and its icon on its floor
        p.rect(tx - 0.5, y + 4.0, 1.0, h - 8.0, [c[0], c[1], c[2], 0.35 * a], 0);
        icon(p, tx, y + h - 2.0, 5.0, *kind, [c[0], c[1], c[2], a]);
    }
    // straight ahead: a pointer down from the top edge and a faint line under it
    for (k, hw) in [5.0, 4.0, 3.0, 2.0, 1.0].into_iter().enumerate() {
        p.rect(mid - hw, y - 3.0 + k as f32 * 2.0, 2.0 * hw, 2.0, CYAN, 0);
    }
    p.rect(mid - 0.5, y + 7.0, 1.0, h - 9.0, [CYAN[0], CYAN[1], CYAN[2], 0.35], 0);
    // under it: the heading in the accent, then where the player is
    let (px, deg) = (PX * 0.75, format!("{:03.0}", heading.round().rem_euclid(360.0)));
    let chars = |t: &str| t.chars().count() as f32 * ADV * px;
    let tw = chars(&deg) + if text.is_empty() { 0.0 } else { 16.0 + chars(text) };
    let (bx, by, bh) = (mid - tw / 2.0, y + h + 7.0, 10.0 * px + 8.0);
    glass(p, bx - 10.0, by, tw + 20.0, bh);
    p.text_px(bx, by + 4.0, &deg, CYAN, px);
    p.text_px(bx + chars(&deg) + 16.0, by + 4.0, text, [0.85, 0.9, 0.95, 1.0], px);
    by + bh
}

/// A waypoint seen in the world: a beam of its colour from where it stands (`base`, on the screen) up to
/// `top`, fading as it rises, and at the top a tag: its icon (`nav::ICONS`), its name and how far.
pub fn marker(p: &mut Paint, base: (f32, f32), top: (f32, f32), colour: [f32; 3], kind: usize, name: &str, far: &str) {
    const STEPS: usize = 14;
    for k in 0..STEPS {
        let (t0, t1) = (k as f32 / STEPS as f32, (k + 1) as f32 / STEPS as f32);
        let (x, y0, y1) = (base.0 + (top.0 - base.0) * (t0 + t1) / 2.0, base.1 + (top.1 - base.1) * t0, base.1 + (top.1 - base.1) * t1);
        // a soft wide one under a bright narrow one
        let a = 0.85 - 0.6 * t0;
        p.rect(x - 3.0, y0.min(y1), 6.0, (y1 - y0).abs().max(1.0), [colour[0], colour[1], colour[2], 0.22 * a], 0);
        p.rect(x - 1.0, y0.min(y1), 2.0, (y1 - y0).abs().max(1.0), [colour[0], colour[1], colour[2], a], 0);
    }
    // where it stands: a short bar on the ground
    p.rect(base.0 - 6.0, base.1 - 1.0, 12.0, 2.0, [colour[0], colour[1], colour[2], 0.9], 0);
    // the tag, centred over the beam's top
    let px = PX * 0.75;
    let chars = |t: &str| t.chars().count() as f32 * ADV * px;
    let w = 26.0 + chars(name) + 12.0 + chars(far) + 10.0;
    let (h, x) = (10.0 * px + 10.0, top.0 - w / 2.0);
    let y = top.1 - h - 4.0;
    glass(p, x, y, w, h);
    p.rect(x, y + 2.0, 3.0, h - 4.0, [colour[0], colour[1], colour[2], 1.0], 0);
    icon(p, x + 15.0, y + h / 2.0, 6.0, kind, [colour[0], colour[1], colour[2], 1.0]);
    p.text_px(x + 26.0, y + 5.0, name, [0.96, 0.97, 1.0, 1.0], px);
    p.text_px(x + 26.0 + chars(name) + 12.0, y + 5.0, far, [0.62, 0.72, 0.82, 1.0], px);
}

/// A waypoint's icon `r` pixels from its middle (cx, cy) to its edge: a diamond, a square, a plus or a ring
/// (`nav::ICONS`), as bars: each its middle's offset and its half width and height, in shares of `r`.
pub fn icon(p: &mut Paint, cx: f32, cy: f32, r: f32, kind: usize, c: [f32; 4]) {
    let bars: &[(f32, f32, f32, f32)] = match kind % 4 {
        1 => &[(0.0, 0.0, 0.8, 0.8)],
        2 => &[(0.0, 0.0, 1.0, 1.0 / 3.0), (0.0, 0.0, 1.0 / 3.0, 1.0)],
        3 => &[(0.0, -2.0 / 3.0, 0.9, 0.25), (0.0, 2.0 / 3.0, 0.9, 0.25), (-2.0 / 3.0, 0.0, 0.25, 0.9), (2.0 / 3.0, 0.0, 0.25, 0.9)],
        _ => &[(0.0, 0.0, 1.0, 1.0 / 3.0), (0.0, 0.0, 2.0 / 3.0, 2.0 / 3.0), (0.0, 0.0, 1.0 / 3.0, 1.0)],
    };
    for (ox, oy, hw, hh) in bars {
        p.rect(cx + (ox - hw) * r, cy + (oy - hh) * r, 2.0 * hw * r, 2.0 * hh * r, c, 0);
    }
}

/// Sparks of a block's colour flying from the middle of the screen after a dig or a place: `age` seconds
/// since it happened, gone after `SPARK_S`. Where each goes is fixed by its number and `seed`, so the burst
/// is the same every frame of its life.
pub const SPARK_S: f32 = 0.45;
pub fn sparks(p: &mut Paint, age: f32, colour: [f32; 3], seed: u32) {
    if !(0.0..SPARK_S).contains(&age) {
        return;
    }
    let (cx, cy) = (p.w / 2.0, p.h / 2.0);
    for k in 0..14u32 {
        let mut h = seed.wrapping_add(k).wrapping_mul(0x9E37_79B1);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        let (a, b, c) = ((h & 255) as f32 / 255.0, (h >> 8 & 255) as f32 / 255.0, (h >> 16 & 255) as f32 / 255.0);
        // out and a little up, then down under their weight
        let ang = a * std::f32::consts::TAU;
        let speed = 120.0 + 260.0 * b;
        let (x, y) = (cx + ang.cos() * speed * age, cy + (ang.sin() * speed - 140.0) * age + 900.0 * age * age);
        let (size, tone) = (4.0 + 5.0 * c, 0.75 + 0.35 * b);
        let fade = 1.0 - (age / SPARK_S).powi(2);
        p.rect(x - size / 2.0, y - size / 2.0, size, size, [(colour[0] * tone).min(1.0), (colour[1] * tone).min(1.0), (colour[2] * tone).min(1.0), fade], 0);
    }
}

/// A little cube of a block, seen from above a corner, in the square at (x, y), drawn texel by texel as
/// the world draws a block (shader.wgsl `texel`): 8 x 8 texels a face in three tones of the block's colour,
/// a grass block's sides its side block's with grass hanging over the top edge, dark cracks in stone. The
/// left side is in half shade, the right in more. `pattern` is `engine::blocks::Pattern as u8`.
pub fn cube(rect: &mut dyn FnMut(f32, f32, f32, f32, [f32; 4], u32), x: f32, y: f32, size: f32, top: [f32; 3], side: [f32; 3], pattern: u8, dim: f32) {
    // a texel is 2u wide and u high on the top, u wide and u high on a side; the cube is 16u across and high
    let u = (size / 16.0).floor().max(1.0);
    let (x, y) = (x + (size - 16.0 * u) / 2.0, y + (size - 16.0 * u) / 2.0);
    // the same noise for the same block every frame: seeded by its colours
    let seed = (top[0] * 255.0) as u32 ^ ((top[1] * 255.0) as u32) << 8 ^ ((side[2] * 255.0) as u32) << 16;
    let noise = |i: u32, j: u32, face: u32| {
        let mut h = seed ^ i.wrapping_mul(0x9E37_79B1) ^ j.wrapping_mul(0x85EB_CA77) ^ face.wrapping_mul(0xC2B2_AE3D);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        (h & 0xFFFF) as f32 / 65536.0
    };
    let paint = |c: [f32; 3], h: f32, shade: f32| {
        let mut k = (0.82 + 0.12 * (h * 3.0).floor()) * shade * dim;
        if pattern == 2 && h > 0.94 {
            k *= 0.6;
        }
        [(c[0] * k).min(1.0), (c[1] * k).min(1.0), (c[2] * k).min(1.0), 1.0]
    };
    for i in 0..8u32 {
        for j in 0..8u32 {
            // the top: a diamond lattice, each texel a brick; the far ones first, the near ones over them
            let h = noise(i, j, 0);
            let c = paint(top, h, if pattern == 1 && h > 0.9 { 1.15 } else { 1.0 });
            rect(x + (7.0 + i as f32 - j as f32) * u, y + (i + j) as f32 * u / 2.0, 2.0 * u, u, c, 0);
        }
    }
    for i in 0..8u32 {
        // the two sides, column by column down from the top's lower edges
        let fringe = 2 + (noise(i, 99, 1) * 3.0) as u32;
        for k in 0..8u32 {
            for (face, cx, cy, shade) in [
                (1, x + i as f32 * u, y + 4.0 * u + (i as f32 + 1.0) * u / 2.0, 0.78),
                (2, x + (8.0 + i as f32) * u, y + 8.0 * u - i as f32 * u / 2.0, 0.56),
            ] {
                // grass hangs two to four texels over a grass block's sides
                let c = if pattern == 1 && k < fringe { top } else { side };
                rect(cx, cy + k as f32 * u, u, u, paint(c, noise(i, k, face), shade), 0);
            }
        }
    }
}

pub struct Slot {
    pub colour: [f32; 3],
    /// The colour of its sides (a grass block's are another block's).
    pub side: [f32; 3],
    /// Its pattern (`engine::blocks::Pattern as u8`): plain, grass, stone.
    pub pattern: u8,
    pub count: Option<u32>,
}

/// A box with its corners cut by two pixels: full width less the corners, and the two steps in.
pub fn cut(p: &mut Paint, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
    p.rect(x + 2.0, y, w - 4.0, h, c, 0);
    p.rect(x, y + 2.0, 2.0, h - 4.0, c, 0);
    p.rect(x + w - 2.0, y + 2.0, 2.0, h - 4.0, c, 0);
}

/// A pane of the HUD's glass at (x, y), what every floating part of the HUD stands on: a soft shadow down
/// and right of it, corners cut by two pixels, the glass a shade lighter in its upper half, a hairline of the
/// accent round it and a light along its top.
pub fn glass(p: &mut Paint, x: f32, y: f32, w: f32, h: f32) {
    cut(p, x + 3.0, y + 4.0, w, h, [0.0, 0.0, 0.0, 0.2]);
    cut(p, x - 1.0, y - 1.0, w + 2.0, h + 2.0, [CYAN[0], CYAN[1], CYAN[2], 0.28]);
    cut(p, x, y, w, h, [PANEL[0], PANEL[1], PANEL[2], 0.86]);
    p.rect(x + 2.0, y, w - 4.0, (h / 2.0).floor(), [1.0, 1.0, 1.0, 0.035], 0);
    p.rect(x + 2.0, y, w - 4.0, 1.0, [1.0, 1.0, 1.0, 0.16], 0);
}

/// Crosshair and hotbar for a `w` x `h` pixel screen. `tool_height` shows beside the hotbar for cylinders
/// and cones; `held` is the name of the block in hand and how strongly it shows (it fades after a change).
pub fn layer(w: f32, h: f32, slots: &[Slot], selected: usize, creative: bool, tool_height: Option<i32>, held: Option<(&str, f32)>) -> Vec<Rect> {
    let mut p = Paint::new(w, h);
    let (cx, cy) = (w / 2.0, h / 2.0);
    // the crosshair: four short arms round a gap and a dot, each on a dark edge so it shows on snow and in
    // a cave alike
    for pass in 0..2 {
        let (grow, colour) = if pass == 0 { (1.0, [0.0, 0.0, 0.0, 0.45]) } else { (0.0, [1.0, 1.0, 1.0, 0.92]) };
        for (dx, dy, aw, ah) in [(-11.0, -1.0, 7.0, 2.0), (4.0, -1.0, 7.0, 2.0), (-1.0, -11.0, 2.0, 7.0), (-1.0, 4.0, 2.0, 7.0), (-1.0, -1.0, 2.0, 2.0)] {
            p.rect(cx + dx - grow, cy + dy - grow, aw + 2.0 * grow, ah + 2.0 * grow, colour, 0);
        }
    }

    // the hotbar: nine slots on one pane of glass, the one in hand lifted, lit and framed in the accent
    let (size, gap, pad, lift) = (52.0, 6.0, 10.0, 6.0);
    let n = 9.0;
    let wide = n * size + (n - 1.0) * gap;
    let x0 = cx - wide / 2.0;
    let y0 = h - size - 22.0;
    glass(&mut p, x0 - pad, y0 - pad, wide + 2.0 * pad, size + 2.0 * pad);
    brackets(&mut p, [x0 - pad - 4.0, y0 - pad - 4.0, wide + 2.0 * pad + 8.0, size + 2.0 * pad + 8.0], 10.0, false, CYAN);
    let digits = |p: &mut Paint, x_right: f32, y_bottom: f32, value: u32, px: f32, colour: [f32; 4]| {
        let s = value.to_string();
        for (k, ch) in s.chars().rev().enumerate() {
            let x = x_right - (k as f32 + 1.0) * px * 4.0;
            let g = ch.to_digit(10).unwrap() + 1;
            p.rect(x + 1.0, y_bottom - px * 5.0 + 1.0, px * 3.0, px * 5.0, [0.0, 0.0, 0.0, 0.8 * colour[3]], g);
            p.rect(x, y_bottom - px * 5.0, px * 3.0, px * 5.0, colour, g);
        }
    };
    for k in 0..9 {
        let on = k == selected;
        let (x, y) = (x0 + k as f32 * (size + gap), if on { y0 - lift } else { y0 });
        if on {
            // a glow of the accent, wider and fainter three times, then the frame
            for (grow, a) in [(9.0, 0.08), (6.0, 0.14), (4.0, 0.22)] {
                p.rect(x - grow, y - grow, size + 2.0 * grow, size + 2.0 * grow, [CYAN[0], CYAN[1], CYAN[2], a], 0);
            }
            p.rect(x - 2.0, y - 2.0, size + 4.0, size + 4.0, CYAN, 0);
        }
        p.rect(x, y, size, size, if on { [0.07, 0.11, 0.17, 1.0] } else { [0.09, 0.12, 0.17, 0.85] }, 0);
        p.rect(x, y, size, 1.0, [1.0, 1.0, 1.0, 0.1], 0);
        // the key that takes it, small in the corner
        digits(&mut p, x + 4.0 + 6.0, y + 4.0 + 7.5, k as u32 + 1, 1.5, if on { CYAN } else { [0.62, 0.72, 0.82, 0.7] });
        if let Some(sl) = slots.get(k) {
            let dim = if sl.count == Some(0) { 0.35 } else { 1.0 };
            cube(&mut |x, y, w, h, c, g| p.rect(x, y, w, h, c, g), x + 10.0, y + 12.0, 32.0, sl.colour, sl.side, sl.pattern, dim);
            if let Some(c) = sl.count {
                digits(&mut p, x + size - 3.0, y + size - 3.0, c, 2.0, [1.0, 1.0, 1.0, 1.0]);
            }
        }
    }
    // what is in hand by name, over the bar, and the mode as a small word at its left end
    if let Some((name, a)) = held.filter(|h| h.1 > 0.0) {
        let tw = text_width(name);
        p.rect(cx - tw / 2.0 - 10.0, y0 - pad - 34.0, tw + 20.0, LINE + 6.0, [PANEL[0], PANEL[1], PANEL[2], PANEL[3] * a], 0);
        p.text(cx - tw / 2.0, y0 - pad - 30.0, name, [0.95, 0.95, 0.95, a]);
    }
    let (word, colour) = if creative { ("CREATIVE", [0.98, 0.8, 0.3, 0.95]) } else { ("SURVIVAL", [0.62, 0.72, 0.82, 0.8]) };
    p.text_px(x0 - pad, y0 - pad - 16.0, word, colour, PX * 0.75);
    if let Some(hgt) = tool_height {
        let x = x0 - pad - size - 14.0;
        glass(&mut p, x, y0, size, size);
        p.rect(x, y0, 3.0, size, CYAN, 0);
        digits(&mut p, x + size - 6.0, y0 + size - 8.0, hgt.max(0) as u32, 4.0, [1.0, 1.0, 1.0, 1.0]);
    }
    p.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::edit::Kind;
    use engine::world::Block;

    #[test]
    fn the_compass_puts_north_in_the_middle_when_facing_it_and_a_waypoint_at_its_bearing() {
        // a wind's letter: the full-size glyph (the winds between them and the numbers are smaller)
        let glyph_x = |p: &Paint, g: char| p.out.iter().filter(|r| r.glyph == g as u32 && (r.xy[2] - r.xy[0]) * 960.0 > 15.0).map(|r| (r.xy[0] + r.xy[2]) / 2.0 * 960.0).last();
        let mark_x = |p: &Paint| p.out.iter().filter(|r| r.colour[..3] == [1.0, 0.0, 1.0]).last().map(|r| ((r.xy[0] + r.xy[2]) / 2.0 * 960.0, r.colour[3])).unwrap();
        // facing north: N in the middle, E and W out of the strip's 60 degrees, a waypoint 30 degrees east
        // a quarter of the strip to the right
        let mut p = Paint::new(1920.0, 1080.0);
        compass(&mut p, 0.0, &[(30.0, [1.0, 0.0, 1.0], 0)], "");
        assert!(glyph_x(&p, 'N').unwrap().abs() < 2.0, "{:?}", glyph_x(&p, 'N'));
        assert!(glyph_x(&p, 'S').is_none() && glyph_x(&p, 'W').is_none());
        assert!((mark_x(&p).0 - 136.0).abs() < 2.0 && mark_x(&p).1 == 1.0, "{:?}", mark_x(&p));
        // facing east: E in the middle; a waypoint 160 degrees round to the right sits dim at the right end
        let mut p = Paint::new(1920.0, 1080.0);
        compass(&mut p, 90.0, &[(250.0, [1.0, 0.0, 1.0], 0)], "");
        assert!(glyph_x(&p, 'E').unwrap().abs() < 2.0);
        assert!(mark_x(&p).0 > 200.0 && mark_x(&p).1 < 1.0, "{:?}", mark_x(&p));
        // across north the strip does not jump: facing 350, north is 10 degrees to the right
        let mut p = Paint::new(1920.0, 1080.0);
        compass(&mut p, 350.0, &[], "");
        assert!((glyph_x(&p, 'N').unwrap() - 280.0 / 6.0).abs() < 2.0, "{:?}", glyph_x(&p, 'N'));
        // the heading under it, three digits, and a waypoint's beam from the ground up to its tag
        assert!(p.out.iter().any(|r| r.glyph == '3' as u32) && p.out.iter().any(|r| r.glyph == '5' as u32));
        let mut p = Paint::new(1920.0, 1080.0);
        marker(&mut p, (960.0, 800.0), (960.0, 500.0), [1.0, 0.0, 1.0], 0, "Home", "120 m");
        let beam: Vec<&Rect> = p.out.iter().filter(|r| r.colour[..3] == [1.0, 0.0, 1.0] && r.glyph == 0).collect();
        let (lo, hi) = beam.iter().fold((1f32, -1f32), |a, r| (a.0.min(r.xy[1]), a.1.max(r.xy[3])));
        let y = |v: f32| (1.0 - v) / 2.0 * 1080.0;
        assert!(y(lo) >= 799.0 && y(lo) <= 802.0 && y(hi) < 500.0, "from {} up to {}", y(lo), y(hi));
        assert_eq!(p.out.iter().filter(|r| r.glyph != 0).count(), 2 * ("Home".len() + "120m".len()));
        // each icon its own shape (bugs/2026-11-11_13-15-26): a diamond, a square, a plus, a ring
        let bars: Vec<usize> = (0..4)
            .map(|k| {
                let mut p = Paint::new(1920.0, 1080.0);
                icon(&mut p, 100.0, 100.0, 6.0, k, [1.0; 4]);
                p.out.len()
            })
            .collect();
        assert_eq!(bars, [3, 1, 2, 4]);
    }

    #[test]
    fn sparks_fly_out_from_the_middle_fade_and_end() {
        let spread = |age: f32| {
            let mut p = Paint::new(1920.0, 1080.0);
            sparks(&mut p, age, [0.5, 0.3, 0.1], 7);
            let far = p.out.iter().map(|r| ((r.xy[0] + r.xy[2]) / 2.0 * 960.0).hypot((r.xy[1] + r.xy[3]) / 2.0 * 540.0)).fold(0.0, f32::max);
            (p.out.len(), far, p.out.first().map_or(0.0, |r| r.colour[3]))
        };
        let (early, late) = (spread(0.05), spread(0.4));
        assert_eq!((early.0, late.0), (14, 14));
        assert!(late.1 > early.1 * 3.0 && late.2 < early.2, "{early:?} then {late:?}");
        assert_eq!((spread(SPARK_S).0, spread(-0.1).0), (0, 0), "none before it happens or after its time");
    }

    #[test]
    fn a_block_s_cube_fills_its_square_in_three_shades() {
        let mut p = Paint::new(100.0, 100.0);
        let mut rect = |x: f32, y: f32, w: f32, h: f32, c: [f32; 4], g: u32| p.rect(x, y, w, h, c, g);
        cube(&mut rect, 10.0, 10.0, 32.0, [0.2, 0.8, 0.2], [0.5, 0.3, 0.1], 1, 1.0);
        // 64 texels a face on three faces; a grass block: green on top and hanging over the brown sides
        assert_eq!(p.out.len(), 3 * 64);
        let green = |r: &&Rect| r.colour[1] > r.colour[0];
        assert_eq!(p.out[..64].iter().filter(green).count(), 64, "the top is grass");
        let sides = p.out[64..].iter().filter(green).count();
        assert!((2 * 8 * 2..=2 * 8 * 4).contains(&sides), "two to four rows of grass over each side: {sides}");
        let tones: std::collections::HashSet<u32> = p.out[..64].iter().map(|r| (r.colour[1] * 1000.0) as u32).collect();
        assert!(tones.len() >= 3, "the top in at least three tones");
        // inside the square, and as wide as it where the top is widest
        let px = |v: f32| (v + 1.0) / 2.0 * 100.0;
        assert!(p.out.iter().all(|r| px(r.xy[0]) >= 9.99 && px(r.xy[2]) <= 42.01));
        let (l, r) = p.out.iter().fold((f32::MAX, f32::MIN), |a, q| (a.0.min(px(q.xy[0])), a.1.max(px(q.xy[2]))));
        assert!(r - l >= 27.9, "{l} to {r}");
    }

    #[test]
    fn a_place_s_name_is_written_in_its_colour_and_nothing_else_is() {
        let tint = place_colours("on Terra's far side, LUNA and Sol; solid terrain");
        let of = |w: &str| PLACES.iter().find(|p| p.0 == w).unwrap().1;
        let text: Vec<char> = "on Terra's far side, LUNA and Sol; solid terrain".chars().collect();
        for (k, c) in text.iter().enumerate() {
            let want = match k {
                3..=7 => Some(of("Terra")),
                21..=24 => Some(of("Luna")),
                30..=32 => Some(of("Sol")),
                _ => None,
            };
            assert_eq!(tint[k], want, "{k}: {c:?}");
        }
        // drawn: the name's glyphs in the place's colour at the text's own strength, its shadow still dark
        let mut p = Paint::new(1920.0, 1080.0);
        p.text(0.0, 0.0, "to Luna", [1.0, 1.0, 1.0, 0.5]);
        let l = of("Luna");
        assert_eq!(p.out.iter().filter(|r| r.colour == [l[0], l[1], l[2], 0.5]).count(), 4);
        assert_eq!(p.out.iter().filter(|r| r.colour == [1.0, 1.0, 1.0, 0.5]).count(), 2);
        // dark ink on a lit button keeps its ink
        let mut p = Paint::new(1920.0, 1080.0);
        p.text_flat(0.0, 0.0, "Luna", [0.0, 0.0, 0.0, 1.0], PX);
        assert!(p.out.iter().all(|r| r.colour == [0.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn the_looked_at_block_is_named_in_the_middle_with_its_colour() {
        let mut p = Paint::new(1920.0, 1080.0);
        looking_at(&mut p, "Copper", [0.8, 0.5, 0.2], [0.4, 0.2, 0.1], 0, "metal", 10.0);
        // its cube: texels in tones of its colour (red over green over blue, as copper is)
        let swatch = p.out.iter().find(|r| r.glyph == 0 && r.colour[0] > r.colour[1] && r.colour[1] > r.colour[2] && r.colour[0] > 0.5).expect("the block's colour");
        assert!(swatch.xy[0] < 0.0 && swatch.xy[3] > 0.9, "left of the middle, at the top");
        // the name and the note, a glyph and its shadow per letter
        let glyphs = p.out.iter().filter(|r| r.glyph != 0).count();
        assert_eq!(glyphs, 2 * ("Copper".len() + "metal".len()));
        let (l, r) = p.out.iter().fold((1f32, -1f32), |a, q| (a.0.min(q.xy[0]), a.1.max(q.xy[2])));
        assert!((l + r).abs() < 0.01, "centred: {l} {r}");
    }

    #[test]
    fn a_section_draws_its_rows_in_their_colours_and_a_bar_per_tick() {
        let s = Section { title: "Simulation", rows: vec![("Tick", "3.0 ms".into(), 1), ("Speed", "20.0".into(), 3)], graph: Some((vec![0.1, 0.6, 1.4], 0.5)) };
        let mut p = Paint::new(1920.0, 1080.0);
        panels(&mut p, &[s], &[]);
        let has = |c: [f32; 4], n: usize| p.out.iter().filter(|r| r.colour == c).count() >= n;
        // the heading and the bar down the side in the accent; a good and a bad value; a bar of each tone
        assert!(has(CYAN, "SIMULATION".len() + 1));
        assert!(has(TONES[1], "3.0ms".len() + 1), "a good value and the bar under the line");
        assert!(has(TONES[2], 1), "the bar over the line");
        assert!(has(TONES[3], "20.0".len() + 1), "a value over its limit and the bar off the top");
        // every bar stands on the graph's floor and none is taller than the graph
        let bars: Vec<&Rect> = p.out.iter().filter(|r| r.glyph == 0 && TONES[1..].contains(&r.colour)).collect();
        assert_eq!(bars.len(), 3);
        let px = |r: &Rect| (r.xy[3] - r.xy[1]) / 2.0 * 1080.0;
        assert!(bars.iter().all(|r| px(r) <= GRAPH_H + 0.01 && (r.xy[1] - bars[0].xy[1]).abs() < 1e-6));
        assert!((px(bars[2]) - GRAPH_H).abs() < 0.01 && (px(bars[0]) - 0.1 * GRAPH_H).abs() < 0.01);
    }

    #[test]
    fn the_columns_shrink_their_text_before_they_overlap() {
        let wide = || Section { title: "Position", rows: vec![("Looking at", "x".repeat(40), 0)], graph: None };
        let reach = |w: f32| {
            let mut p = Paint::new(w, 720.0);
            panels(&mut p, &[wide()], &[wide()]);
            // the left column's right edge and the right column's left edge, in pixels
            let mid = |r: &&Rect| r.xy[0] < 0.0;
            let l = p.out.iter().filter(mid).map(|r| r.xy[2]).fold(-1f32, f32::max);
            let r = p.out.iter().filter(|r| !mid(r)).map(|r| r.xy[0]).fold(1f32, f32::min);
            ((l + 1.0) / 2.0 * w, (r + 1.0) / 2.0 * w)
        };
        // 52 characters a column: side by side at full size on 1920, only smaller on 1280 and on 900
        for w in [1920.0, 1280.0, 900.0] {
            let (l, r) = reach(w);
            assert!(l <= r, "{w}: the left column ends at {l}, the right starts at {r}");
        }
    }

    #[test]
    fn the_font_draws_every_printable_character() {
        let cells = font_cells(include_str!("../../../data/font.txt")).unwrap();
        assert_eq!(cells.len(), 95);
        let at = |c: char| cells[c as usize - 32];
        assert_eq!(at(' '), [0; 8]);
        assert!((b'!'..=b'~').all(|c| at(c as char) != [0; 8]), "a character with no pixels");
        // the small letters are drawn: a low one on the baseline under two empty rows, a tall one from the
        // top, one with a tail down into the eighth row; none is its capital shrunk
        assert_eq!((at('a')[..2].to_vec(), at('a')[7]), (vec![0, 0], 0));
        assert!(at('b')[0] != 0 && at('b')[7] == 0);
        assert!(at('g')[7] != 0 && at('y')[7] != 0 && at('p')[7] != 0);
        assert!(('a'..='z').all(|c| at(c) != at(c.to_ascii_uppercase())));
        // a letter the file does not draw is still its capital without rows 1 and 4
        let small = font_cells("A\n.###.\n#...#\n#...#\n#####\n#...#\n#...#\n#...#\n");
        assert!(small.is_err(), "the other characters are missing, so the file as a whole is refused");
        assert!(font_cells("A\n#..#..#\n").is_err() && font_cells("A\n#\n").is_err());
    }

    #[test]
    fn a_ghost_outlines_only_the_outside() {
        let w = World::new(256, 32, 1);
        let b = Block { body: 0, face: 4, pos: IVec3::new(10, 10, 20) };
        let one = ghost(&w, &Shape::one(b), [255; 4]);
        assert_eq!(one.len(), 6 * 8, "a cube: six faces of four edges");
        let brush = Shape { max: b.pos + 1, ..Shape::one(b) };
        assert_eq!(ghost(&w, &brush, [255; 4]).len(), 24 * 8, "a 2 x 2 x 2 cube shows its 24 outer faces");
        let big = Shape { max: b.pos + IVec3::new(31, 31, 1), kind: Kind::Box, ..Shape::one(b) };
        assert!(ghost(&w, &big, [255; 4]).len() <= GHOST_MAX);
    }

    #[test]
    fn wrap_breaks_at_spaces_and_cuts_long_words() {
        assert_eq!(wrap("one two three", 7), vec!["one two", "three"]);
        assert_eq!(wrap("  /time    the time", 40), vec!["  /time    the time"], "a line that fits keeps its spaces");
        assert_eq!(wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(wrap("", 4), vec![""]);
        assert!(wrap(&"word ".repeat(40), 12).iter().all(|l| l.chars().count() <= 12));
    }

    #[test]
    fn text_is_a_glyph_per_character_and_the_font_covers_ascii() {
        let mut p = Paint::new(1920.0, 1080.0);
        p.text(10.0, 10.0, "Hi é!", [1.0; 4]);
        let glyphs: Vec<u32> = p.out.iter().map(|r| r.glyph).collect();
        assert_eq!(glyphs, vec![72, 72, 105, 105, 63, 63, 33, 33], "shadow then face; the space is skipped, é is '?'");
        let f = font_wgsl();
        assert_eq!(f.matches('u').count(), 2 * 95 + 1, "two words a character, and the u of u32");
        let mut c = Paint::new(1920.0, 1080.0);
        chat(&mut c, &[(1.0, "hello"), (30.0, "old")], None, false, 0, &[], None);
        assert_eq!(c.out.iter().filter(|r| r.glyph >= 32).count(), 10, "the old line has faded");
        // the completions of "/ti" stand over the bar, on screen
        let n = c.out.len();
        chat(&mut c, &[(30.0, "old")], Some("/ti"), true, 1, &["time".to_string(), "timewarp".to_string()], Some(1));
        assert!(c.out.len() - n > 2 * 12, "the bar and both completions");
        assert!(c.out.iter().all(|r| r.xy.iter().all(|v| (-1.0..=1.0).contains(v))));
    }

    #[test]
    fn the_layer_has_a_crosshair_and_nine_slots() {
        let slots = [Slot { colour: [0.5; 3], side: [0.4; 3], pattern: 2, count: Some(12) }];
        let r = layer(1280.0, 720.0, &slots, 0, false, None, Some(("Grass", 1.0)));
        assert!(r.len() >= 2 + 1 + 9 * 2 + 4, "{}", r.len());
        assert!(r.iter().all(|r| r.xy[0] < r.xy[2] && r.xy[1] < r.xy[3] && r.xy.iter().all(|v| (-1.0..=1.0).contains(v))));
        assert!(r.iter().any(|r| r.glyph == 2) && r.iter().any(|r| r.glyph == 3), "the count 12 as digits");
    }
}
