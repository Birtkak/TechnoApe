//! The game's own font (`data/font.txt`, ADR 0023) as a TrueType file in memory, so the Studio's interface is
//! set in it (ADR 0046): egui reads outlines, the game reads 8 x 8 cells, and the cells stay the one source.
//! A lit pixel run is a rectangle; a character is as wide as its lit columns and one more (the game steps a
//! fixed 6: here a label column has to hold "front right leg").

/// Font units a pixel of the cell.
const U: i16 = 100;
/// Font units an em: a text of size 14 draws a cell's pixel 1.5 screen pixels wide.
pub const EM: u16 = 933;

/// The lit runs of a cell as (column, columns, row from the top), column 0 the cell's second (the game draws
/// a character one column in).
fn runs(cell: &[u8; 8]) -> Vec<(i16, i16, i16)> {
    let mut v = vec![];
    for (r, row) in cell.iter().enumerate() {
        let mut c = 0;
        while c < 7 {
            let n = (c..7).take_while(|k| *row >> (*k + 1) & 1 == 1).count() as i16;
            if n > 0 {
                v.push((c, n, r as i16));
            }
            c += n.max(1);
        }
    }
    v
}

/// A character's runs with the empty columns on its left taken off, and how far it steps in pixels: its lit
/// columns and one, the space 4. A digit keeps the game's cell and fixed 6: a number being dragged must not
/// shake.
pub fn glyph(c: char, cell: &[u8; 8]) -> (Vec<(i16, i16, i16)>, i16) {
    let mut rs = runs(cell);
    if c.is_ascii_digit() {
        return (rs, 6);
    }
    let left = rs.iter().map(|r| r.0).min().unwrap_or(0);
    rs.iter_mut().for_each(|r| r.0 -= left);
    let step = rs.iter().map(|(c, n, _)| c + n + 1).max().unwrap_or(4);
    (rs, step)
}

/// Characters the Studio's texts use that the game's font does not draw (degree, plus-minus, squared, middle
/// dot, times, ellipsis), as cells of the same kind, in the order of their codes.
pub const MORE: [(char, [u8; 8]); 6] = [
    ('\u{b0}', [4, 10, 4, 0, 0, 0, 0, 0]),
    ('\u{b1}', [0, 8, 62, 8, 0, 62, 0, 0]),
    ('\u{b2}', [6, 8, 4, 2, 14, 0, 0, 0]),
    ('\u{b7}', [0, 0, 0, 2, 0, 0, 0, 0]),
    ('\u{d7}', [0, 0, 34, 20, 8, 20, 34, 0]),
    ('\u{2026}', [0, 0, 0, 0, 0, 0, 42, 0]),
];

/// The TrueType file of `glyphs`: each character and its cell (as `client::font_cells` gives the game's), in
/// the order of their codes.
/// shortcut: table checksums are left 0 (egui's reader does not check them), fill them in if the file is
/// ever written out for another program.
pub fn ttf(glyphs: &[(char, [u8; 8])]) -> Vec<u8> {
    assert!(glyphs.windows(2).all(|w| w[0].0 < w[1].0), "the characters in the order of their codes");
    fn w16(v: &mut Vec<u8>, x: i32) {
        v.extend((x as u16).to_be_bytes());
    }
    fn w32(v: &mut Vec<u8>, x: u32) {
        v.extend(x.to_be_bytes());
    }
    // glyph 0 is "no such character" (empty); then one a cell
    let (mut glyf, mut loca, mut hmtx) = (vec![], vec![0u32, 0], vec![]);
    w16(&mut hmtx, 4 * U as i32);
    w16(&mut hmtx, 0);
    let (mut most_points, mut most_contours) = (0, 0);
    for (c, cell) in glyphs {
        let (rs, step) = glyph(*c, cell);
        if !rs.is_empty() {
            // the baseline is under row 6; row 7 is a tail's
            let pts: Vec<(i16, i16)> = rs.iter().flat_map(|&(c, n, r)| [(c, 6 - r), (c, 7 - r), (c + n, 7 - r), (c + n, 6 - r)]).map(|(x, y)| (x * U, y * U)).collect();
            let (xs, ys) = (pts.iter().map(|p| p.0), pts.iter().map(|p| p.1));
            w16(&mut glyf, rs.len() as i32);
            for b in [xs.clone().min(), ys.clone().min(), xs.clone().max(), ys.clone().max()] {
                w16(&mut glyf, b.unwrap() as i32);
            }
            for k in 0..rs.len() {
                w16(&mut glyf, k as i32 * 4 + 3);
            }
            w16(&mut glyf, 0);
            glyf.extend(std::iter::repeat(1u8).take(pts.len()));
            for coords in [xs.collect::<Vec<_>>(), ys.collect::<Vec<_>>()] {
                let mut last = 0;
                for c in coords {
                    w16(&mut glyf, (c - last) as i32);
                    last = c;
                }
            }
            glyf.resize(glyf.len().next_multiple_of(4), 0);
            (most_points, most_contours) = (most_points.max(pts.len()), most_contours.max(rs.len()));
        }
        loca.push(glyf.len() as u32);
        w16(&mut hmtx, (step * U) as i32);
        w16(&mut hmtx, 0);
    }
    let n = glyphs.len() as i32 + 1;
    let mut head = vec![];
    for x in [0x0001_0000, 0x0001_0000, 0, 0x5F0F_3CF5] {
        w32(&mut head, x);
    }
    w16(&mut head, 0);
    w16(&mut head, EM as i32);
    head.extend([0u8; 16]);
    for x in [0, -(U as i32), 7 * U as i32, 7 * U as i32, 0, 6, 2, 1, 0] {
        w16(&mut head, x);
    }
    let mut hhea = vec![];
    w32(&mut hhea, 0x0001_0000);
    // (the line: a capital's 7 pixels in its middle, a tail's 1 under them, and room over and under)
    for x in [850, -150, 0, 7 * U as i32, 0, 0, 7 * U as i32, 1, 0, 0, 0, 0, 0, 0, 0, n] {
        w16(&mut hhea, x);
    }
    let mut maxp = vec![];
    w32(&mut maxp, 0x0001_0000);
    for x in [n, most_points as i32, most_contours as i32, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0] {
        w16(&mut maxp, x);
    }
    // characters that follow each other are a segment: (first, last, the first's glyph); the last segment
    // is the format's own end
    let mut segs: Vec<(i32, i32, i32)> = vec![];
    for (i, (c, _)) in glyphs.iter().enumerate() {
        match segs.last_mut() {
            Some(s) if s.1 + 1 == *c as i32 => s.1 += 1,
            _ => segs.push((*c as i32, *c as i32, i as i32 + 1)),
        }
    }
    segs.push((0xFFFF, 0xFFFF, 0x1_0000));
    let (n2, log) = (segs.len() as i32 * 2, segs.len().ilog2() as i32);
    let mut cmap = vec![];
    for x in [0, 1, 3, 1, 0, 12, 4, 16 + 4 * n2, 0, n2, 2 << log, log, n2 - (2 << log)] {
        w16(&mut cmap, x);
    }
    for col in [segs.iter().map(|s| s.1).collect::<Vec<_>>(), vec![0], segs.iter().map(|s| s.0).collect(), segs.iter().map(|s| s.2 - s.0).collect(), vec![0; segs.len()]] {
        for x in col {
            w16(&mut cmap, x);
        }
    }
    let loca: Vec<u8> = loca.iter().flat_map(|o| o.to_be_bytes()).collect();
    let tables: [(&[u8; 4], Vec<u8>); 7] = [(b"cmap", cmap), (b"glyf", glyf), (b"head", head), (b"hhea", hhea), (b"hmtx", hmtx), (b"loca", loca), (b"maxp", maxp)];
    let mut out = vec![];
    w32(&mut out, 0x0001_0000);
    for x in [7, 64, 2, 48] {
        w16(&mut out, x);
    }
    let mut at = 12 + 16 * tables.len();
    for (tag, t) in &tables {
        out.extend(*tag);
        w32(&mut out, 0);
        w32(&mut out, at as u32);
        w32(&mut out, t.len() as u32);
        at += t.len().next_multiple_of(4);
    }
    for (_, t) in &tables {
        out.extend(t);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui;

    /// egui reads the made file: a text in it is as wide as its characters' cells say, and an "l" is narrower
    /// than an "m" (each as wide as it is drawn).
    #[test]
    fn egui_sets_text_in_the_games_font_each_character_as_wide_as_drawn() {
        let cells = client::font_cells(include_str!("../../../data/font.txt")).unwrap();
        let advance = |c: char| glyph(c, &cells.get(c as usize - 32).copied().unwrap_or_else(|| MORE.iter().find(|m| m.0 == c).unwrap().1)).1;
        assert_eq!((advance('A'), advance(' '), advance('1'), advance('0')), (6, 4, 6, 6));
        assert!(advance('l') < advance('m'));
        // (no empty column is left before a character)
        assert!(" !ilj.".chars().all(|c| glyph(c, &cells[c as usize - 32]).0.iter().map(|r| r.0).min().unwrap_or(0) == 0));
        assert_eq!(runs(&[0b0111110, 0b0100010, 0, 0, 0, 0, 0, 0]), vec![(0, 5, 0), (0, 1, 1), (4, 1, 1)]);
        let ctx = egui::Context::default();
        crate::ui::look::fonts(&ctx);
        let mut wide = 0.0;
        let mut out = ctx.run_pass(Default::default(), |ctx| {
            wide = ctx.fonts_mut(|f| f.layout_no_wrap("Ape lamp".into(), egui::FontId::proportional(EM as f32 / 100.0), egui::Color32::WHITE)).size().x;
        });
        // (the font atlas it made is no window's to take)
        out.textures_delta.clear();
        // at this size a cell's pixel is a point
        let want: i16 = "Ape lamp".chars().map(advance).sum();
        assert!((wide - want as f32).abs() < 1.0, "{wide} wide, the cells say {want}");
        // the characters the game's font has not are the Studio's own cells, not another font's: an ellipsis
        // is its three dots and a step, a degree sign its ring
        assert_eq!((advance('\u{2026}'), advance('\u{b0}')), (6, 4));
        let mut more = 0.0;
        let mut out = ctx.run_pass(Default::default(), |ctx| {
            more = ctx.fonts_mut(|f| f.layout_no_wrap("8\u{b0} \u{b1}2\u{2026}".into(), egui::FontId::proportional(EM as f32 / 100.0), egui::Color32::WHITE)).size().x;
        });
        out.textures_delta.clear();
        let want: i16 = "8\u{b0} \u{b1}2\u{2026}".chars().map(advance).sum();
        assert!((more - want as f32).abs() < 1.0, "{more} wide, the cells say {want}");
    }
}
