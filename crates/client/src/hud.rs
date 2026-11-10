//! What the overlay pipelines draw (`overlay.wgsl`, ADR 0017): the ghost of a shape as lines, and the 2D
//! layer of crosshair and hotbar as rectangles; text is a rectangle per character of an 8 x 8 font (ADR 0023):
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
    /// 0 solid, 1..10 the digits 0..9 of the small font, 32..126 that ASCII character of the 8 x 8 font.
    pub glyph: u32,
}

/// The most ghost vertices: every edge of every cell of the largest shape.
pub const GHOST_MAX: usize = engine::edit::MAX_CELLS as usize * 24;
/// Enough for the F3 overlay and a full chat: two rectangles (shadow and face) a character.
pub const RECTS_MAX: usize = 8192;

/// The 8 x 8 font for `overlay.wgsl`, as WGSL: two words a character from ' ' to '~', rows from the top,
/// the leftmost pixel the lowest bit (font8x8's public-domain basic set).
pub fn font_wgsl() -> String {
    let words: Vec<String> = font8x8::legacy::BASIC_LEGACY[32..127]
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

/// Screen pixels per font pixel, and the height of a line of text.
pub const PX: f32 = 2.0;
pub const LINE: f32 = 9.0 * PX + 2.0;

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
        let size = 8.0 * px;
        for (k, ch) in s.chars().enumerate() {
            let g = if (' '..='~').contains(&ch) { ch as u32 } else { '?' as u32 };
            if g == ' ' as u32 {
                continue;
            }
            let cx = x + k as f32 * size;
            self.rect(cx + px, y + px, size, size, [0.0, 0.0, 0.0, colour[3] * 0.7], g);
            self.rect(cx, y, size, size, colour, g);
        }
    }
}

pub fn text_width(s: &str) -> f32 {
    s.chars().count() as f32 * 8.0 * PX
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
    let width = CHAT_COLS as f32 * 8.0 * PX + 8.0;
    let bottom = p.h - 84.0;
    for (k, (a, c, r)) in rows.iter().rev().enumerate() {
        let y = bottom - (k + 1) as f32 * LINE;
        p.rect(4.0, y, width, LINE, [0.0, 0.0, 0.0, 0.45 * a], 0);
        p.text(8.0, y + 2.0, r, [c[0], c[1], c[2], *a]);
    }
    if let Some(t) = typing {
        // the end of a long line shows
        let cols = ((p.w - 24.0) / (8.0 * PX)) as usize - 1;
        let shown: String = t.chars().skip(t.chars().count().saturating_sub(cols)).collect();
        let y = p.h - LINE - 8.0;
        p.rect(4.0, y, p.w - 8.0, LINE, [0.0, 0.0, 0.0, 0.6], 0);
        p.text(8.0, y + 2.0, &shown, [1.0, 1.0, 1.0, 1.0]);
        if caret {
            p.text(8.0 + text_width(&shown), y + 2.0, "_", [1.0, 1.0, 1.0, 1.0]);
        }
        // the completions over the bar, under the word they complete, the picked one yellow
        if let (Some(k), false) = (pick, hints.is_empty()) {
            const SHOW: usize = 10;
            let first = k.saturating_sub(SHOW - 1).min(hints.len().saturating_sub(SHOW));
            let list = &hints[first..(first + SHOW).min(hints.len())];
            let x = 8.0 + if shown.len() == t.len() { text_width(&t[..at.min(t.len())]) } else { 0.0 };
            let wide = list.iter().map(|h| text_width(h)).fold(0.0, f32::max) + 8.0;
            for (r, h) in list.iter().enumerate() {
                let hy = y - (list.len() - r) as f32 * LINE;
                p.rect(x - 4.0, hy, wide, LINE, [0.0, 0.0, 0.0, 0.8], 0);
                p.text(x, hy + 2.0, h, if first + r == k { [1.0, 0.9, 0.3, 1.0] } else { [0.75, 0.75, 0.75, 1.0] });
            }
        }
    }
}

/// The F3 overlay: lines down the left and, right-aligned, down the right, each on a grey band.
pub fn columns(p: &mut Paint, left: &[String], right: &[String]) {
    for (k, l) in left.iter().enumerate() {
        let y = 4.0 + k as f32 * LINE;
        if !l.is_empty() {
            p.rect(2.0, y, text_width(l) + 6.0, LINE, [0.1, 0.1, 0.1, 0.55], 0);
            p.text(4.0, y + 2.0, l, [0.95, 0.95, 0.95, 1.0]);
        }
    }
    for (k, l) in right.iter().enumerate() {
        let y = 4.0 + k as f32 * LINE;
        let x = p.w - text_width(l) - 6.0;
        if !l.is_empty() {
            p.rect(x - 2.0, y, text_width(l) + 6.0, LINE, [0.1, 0.1, 0.1, 0.55], 0);
            p.text(x, y + 2.0, l, [0.95, 0.95, 0.95, 1.0]);
        }
    }
}

/// The menu's buttons on a `w` x `h` screen: x, y, width and height in pixels, a column in the middle.
pub fn menu_boxes(w: f32, h: f32, n: usize) -> Vec<[f32; 4]> {
    let (bw, bh, gap) = (640.0, 48.0, 12.0);
    let top = (h - n as f32 * (bh + gap)) / 2.0 + 30.0;
    (0..n).map(|k| [(w - bw) / 2.0, top + k as f32 * (bh + gap), bw, bh]).collect()
}

/// The menu (Esc): the game darkened, the page's title, its buttons (`hover` lighter) and a hint under them.
pub fn menu(p: &mut Paint, title: &str, items: &[String], hint: &str, hover: Option<usize>) {
    p.rect(0.0, 0.0, p.w, p.h, [0.0, 0.0, 0.0, 0.55], 0);
    let boxes = menu_boxes(p.w, p.h, items.len());
    let top = boxes.first().map_or(p.h / 2.0, |b| b[1]);
    let tw = text_width(title) * 2.0;
    p.text_px((p.w - tw) / 2.0, top - 90.0, title, [1.0, 1.0, 1.0, 1.0], PX * 2.0);
    for (k, (b, label)) in boxes.iter().zip(items).enumerate() {
        let lit = hover == Some(k);
        p.rect(b[0] - 2.0, b[1] - 2.0, b[2] + 4.0, b[3] + 4.0, if lit { [1.0, 1.0, 1.0, 0.9] } else { [0.0, 0.0, 0.0, 0.8] }, 0);
        p.rect(b[0], b[1], b[2], b[3], if lit { [0.42, 0.44, 0.5, 0.95] } else { [0.24, 0.25, 0.28, 0.95] }, 0);
        p.text(b[0] + (b[2] - text_width(label)) / 2.0, b[1] + (b[3] - 8.0 * PX) / 2.0, label, [1.0, 1.0, 1.0, 1.0]);
    }
    if let Some(b) = boxes.last() {
        p.text((p.w - text_width(hint)) / 2.0, b[1] + b[3] + 24.0, hint, [0.75, 0.75, 0.75, 1.0]);
    }
}

/// The bug report's form (F9): a panel in the middle of the screen with the text written so far.
pub fn bug_form(p: &mut Paint, text: &str, caret: bool) {
    let cols = 60;
    let (pw, ph) = (cols as f32 * 8.0 * PX + 40.0, 15.0 * LINE + 40.0);
    let (x, y) = ((p.w - pw) / 2.0, (p.h - ph) / 2.0);
    p.rect(0.0, 0.0, p.w, p.h, [0.0, 0.0, 0.0, 0.35], 0);
    p.rect(x, y, pw, ph, [0.08, 0.08, 0.1, 0.92], 0);
    p.rect(x, y, pw, 4.0, [0.95, 0.45, 0.2, 1.0], 0);
    p.text(x + 20.0, y + 16.0, "Bug report", [1.0, 0.8, 0.5, 1.0]);
    p.text(x + 20.0, y + 16.0 + LINE * 1.5, "What went wrong? A screenshot and the game's state", [0.7, 0.7, 0.7, 1.0]);
    p.text(x + 20.0, y + 16.0 + LINE * 2.5, "are saved with it. Enter saves, Esc cancels.", [0.7, 0.7, 0.7, 1.0]);
    let top = y + 16.0 + LINE * 4.0;
    p.rect(x + 14.0, top - 4.0, pw - 28.0, LINE * 10.0 + 8.0, [0.0, 0.0, 0.0, 0.6], 0);
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
pub struct Slot {
    pub colour: [f32; 3],
    pub count: Option<u32>,
}

/// Crosshair and hotbar for a `w` x `h` pixel screen. `tool_height` shows beside the hotbar for cylinders
/// and cones.
pub fn layer(w: f32, h: f32, slots: &[Slot], selected: usize, creative: bool, tool_height: Option<i32>) -> Vec<Rect> {
    let mut p = Paint::new(w, h);
    let mut rect = |x: f32, y: f32, rw: f32, rh: f32, colour: [f32; 4], glyph: u32| p.rect(x, y, rw, rh, colour, glyph);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let white = [1.0, 1.0, 1.0, 0.85];
    rect(cx - 8.0, cy - 1.0, 16.0, 2.0, white, 0);
    rect(cx - 1.0, cy - 8.0, 2.0, 16.0, white, 0);

    let (size, gap) = (44.0, 6.0);
    let n = 9.0;
    let x0 = cx - (n * size + (n - 1.0) * gap) / 2.0;
    let y0 = h - size - 16.0;
    // a strip over the hotbar: gold in creative, grey in survival
    let mode = if creative { [0.95, 0.75, 0.2, 0.9] } else { [0.6, 0.6, 0.6, 0.6] };
    rect(x0, y0 - 10.0, n * size + (n - 1.0) * gap, 4.0, mode, 0);
    let digits = |x_right: f32, y_bottom: f32, value: u32, px: f32, rect: &mut dyn FnMut(f32, f32, f32, f32, [f32; 4], u32)| {
        let s = value.to_string();
        for (k, ch) in s.chars().rev().enumerate() {
            let x = x_right - (k as f32 + 1.0) * px * 4.0;
            let g = ch.to_digit(10).unwrap() + 1;
            rect(x + 1.0, y_bottom - px * 5.0 + 1.0, px * 3.0, px * 5.0, [0.0, 0.0, 0.0, 0.8], g);
            rect(x, y_bottom - px * 5.0, px * 3.0, px * 5.0, [1.0, 1.0, 1.0, 1.0], g);
        }
    };
    for k in 0..9 {
        let x = x0 + k as f32 * (size + gap);
        let frame = if k == selected { [1.0, 1.0, 1.0, 0.95] } else { [0.0, 0.0, 0.0, 0.5] };
        rect(x - 3.0, y0 - 3.0, size + 6.0, size + 6.0, frame, 0);
        match slots.get(k) {
            Some(sl) => {
                let dim = if sl.count == Some(0) { 0.35 } else { 1.0 };
                rect(x, y0, size, size, [sl.colour[0] * dim, sl.colour[1] * dim, sl.colour[2] * dim, 1.0], 0);
                if let Some(c) = sl.count {
                    digits(x + size - 3.0, y0 + size - 3.0, c, 2.0, &mut rect);
                }
            }
            None => rect(x, y0, size, size, [0.1, 0.1, 0.1, 0.4], 0),
        }
    }
    if let Some(hgt) = tool_height {
        let x = x0 - size - gap - 6.0;
        rect(x, y0, size, size, [0.15, 0.2, 0.3, 0.8], 0);
        digits(x + size - 6.0, y0 + size - 8.0, hgt.max(0) as u32, 4.0, &mut rect);
    }
    p.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::edit::Kind;
    use engine::world::Block;

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
        let slots = [Slot { colour: [0.5; 3], count: Some(12) }];
        let r = layer(1280.0, 720.0, &slots, 0, false, None);
        assert!(r.len() >= 2 + 1 + 9 * 2 + 4, "{}", r.len());
        assert!(r.iter().all(|r| r.xy[0] < r.xy[2] && r.xy[1] < r.xy[3] && r.xy.iter().all(|v| (-1.0..=1.0).contains(v))));
        assert!(r.iter().any(|r| r.glyph == 2) && r.iter().any(|r| r.glyph == 3), "the count 12 as digits");
    }
}
