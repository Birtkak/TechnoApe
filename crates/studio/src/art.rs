//! The Studio's art without a window: a texture's pixels, the tools that paint them, undo, the PNG rule and the
//! palette check, the drafts list and the tiled sheet. Everything here is a pure function of its inputs, tested.
use engine::blocks::Registry;
use engine::palette::{to_hex, Palette};
use std::path::Path;

/// A texture's side in pixels: one 1 m face.
pub const S: usize = 16;
pub const FACES: [&str; 4] = ["", "top", "side", "bottom"];

/// A texture: 16 high, 1 to 4 variants of 16 x 16 side by side, every pixel opaque (so only its colour is kept).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Img {
    pub w: usize,
    pub px: Vec<[u8; 3]>,
}

impl Img {
    pub fn filled(variants: usize, c: [u8; 3]) -> Img {
        Img { w: variants * S, px: vec![c; variants * S * S] }
    }
    pub fn variants(&self) -> usize {
        self.w / S
    }
    pub fn get(&self, v: usize, x: usize, y: usize) -> [u8; 3] {
        self.px[y * self.w + v * S + x]
    }
    fn set(&mut self, v: usize, x: usize, y: usize, c: [u8; 3]) {
        let w = self.w;
        self.px[y * w + v * S + x] = c;
    }
    pub fn uses(&self, c: [u8; 3]) -> bool {
        self.px.contains(&c)
    }
}

/// The file of a block's face: `FACES[0]` is every face.
pub fn file_name(block: &str, face: usize) -> String {
    match FACES[face] {
        "" => format!("{block}.png"),
        f => format!("{block}_{f}.png"),
    }
}

/// The file of a block's face in season `season` (`engine::blocks::SEASONS`): `<face's file>@<season>.png`,
/// summer's the base file without one (docs/design/seasonal-looks.md).
pub fn season_file(block: &str, face: usize, season: usize) -> String {
    let base = file_name(block, face);
    match season {
        1 => base,
        q => format!("{}@{}.png", base.strip_suffix(".png").unwrap_or(&base), engine::blocks::SEASONS[q]),
    }
}

/// Whether `file` is one of block `name`'s texture files: any face, any season.
pub fn names_block(name: &str, file: &str) -> bool {
    (0..4).any(|f| (0..4).any(|q| season_file(name, f, q) == file))
}

fn mirrored(x: usize, mirror: bool) -> Vec<usize> {
    if mirror && x != S - 1 - x {
        vec![x, S - 1 - x]
    } else {
        vec![x]
    }
}

/// Paints one pixel of variant `v` (and its mirror image across the variant's middle).
pub fn pencil(img: &mut Img, v: usize, x: usize, y: usize, c: [u8; 3], mirror: bool) {
    for x in mirrored(x, mirror) {
        img.set(v, x, y, c);
    }
}

/// Fills the 4-connected area of one colour that holds (x, y) within variant `v`.
pub fn fill(img: &mut Img, v: usize, x: usize, y: usize, c: [u8; 3], mirror: bool) {
    for x in mirrored(x, mirror) {
        let from = img.get(v, x, y);
        if from == c {
            continue;
        }
        let mut todo = vec![(x, y)];
        while let Some((x, y)) = todo.pop() {
            if img.get(v, x, y) != from {
                continue;
            }
            img.set(v, x, y, c);
            if x > 0 {
                todo.push((x - 1, y));
            }
            if x + 1 < S {
                todo.push((x + 1, y));
            }
            if y > 0 {
                todo.push((x, y - 1));
            }
            if y + 1 < S {
                todo.push((x, y + 1));
            }
        }
    }
}

/// A straight line of pixels from a to b, both ends included (Bresenham).
#[cfg(test)]
pub fn line(img: &mut Img, v: usize, a: (usize, usize), b: (usize, usize), c: [u8; 3], mirror: bool) {
    let (mut x, mut y) = (a.0 as i32, a.1 as i32);
    let (x1, y1) = (b.0 as i32, b.1 as i32);
    let (dx, dy) = ((x1 - x).abs(), -(y1 - y).abs());
    let (sx, sy) = ((x1 - x).signum(), (y1 - y).signum());
    let mut err = dx + dy;
    loop {
        pencil(img, v, x as usize, y as usize, c, mirror);
        if (x, y) == (x1, y1) {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// A fifth variant does not fit: the widest file is 64 px.
pub fn add_variant(img: &Img, copy: usize) -> Option<Img> {
    if img.variants() >= 4 {
        return None;
    }
    let w = img.w + S;
    let px = (0..S).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| if x < img.w { img.px[y * img.w + x] } else { img.get(copy, x - img.w, y) }).collect();
    Some(Img { w, px })
}

/// The file without variant `v`; None when it is the only one.
pub fn remove_variant(img: &Img, v: usize) -> Option<Img> {
    if img.variants() <= 1 || v >= img.variants() {
        return None;
    }
    let px = (0..S).flat_map(|y| (0..img.w).map(move |x| (x, y))).filter(|(x, _)| x / S != v).map(|(x, y)| img.px[y * img.w + x]).collect();
    Some(Img { w: img.w - S, px })
}

/// Whether variant `v` is one colour all over (nothing to vary, nothing lost).
pub fn flat(img: &Img, v: usize) -> bool {
    let c = img.get(v, 0, 0);
    (0..S * S).all(|i| img.get(v, i % S, i / S) == c)
}

/// One undo step: the picture before an edit, and the edit's name for the history list.
#[derive(Clone, Debug)]
pub struct Step<T = Img> {
    pub name: String,
    pub img: T,
}

/// Undo and redo of one file: whole snapshots, a few KB each, each with its edit's name.
/// (any snapshot type: a model's history is whole models, ADR 0049)
#[derive(Clone)]
pub struct History<T = Img> {
    pub undo: Vec<Step<T>>,
    pub redo: Vec<Step<T>>,
}
impl<T> Default for History<T> {
    fn default() -> Self {
        History { undo: vec![], redo: vec![] }
    }
}

impl<T> History<T> {
    /// Call before an edit with the picture as it was and the edit's name ("Pencil", "Fill").
    pub fn push(&mut self, before: T, name: &str) {
        self.undo.push(Step { name: name.into(), img: before });
        self.redo.clear();
    }
    pub fn undo(&mut self, now: &mut T) -> bool {
        self.undo.pop().map(|s| self.redo.push(Step { img: std::mem::replace(now, s.img), name: s.name })).is_some()
    }
    pub fn redo(&mut self, now: &mut T) -> bool {
        self.redo.pop().map(|s| self.undo.push(Step { img: std::mem::replace(now, s.img), name: s.name })).is_some()
    }
    /// Undoes or redoes until `n` steps are left to undo (a click on the history list).
    pub fn back_to(&mut self, now: &mut T, n: usize) {
        while self.undo.len() > n && self.undo(now) {}
        while self.undo.len() < n && self.redo(now) {}
    }
    /// Every picture it holds, to repaint them all when a palette colour changes.
    pub fn imgs(&mut self) -> impl Iterator<Item = &mut T> {
        self.undo.iter_mut().chain(self.redo.iter_mut()).map(|s| &mut s.img)
    }
}

/// One texture file open in the Studio: the picture, its undo, and the file as it is on disk.
pub struct Doc {
    pub img: Img,
    pub hist: History,
    /// The file as it is on disk; none for a file not yet saved.
    pub saved: Option<Img>,
}
impl Doc {
    pub fn loaded(img: Img) -> Doc {
        Doc { img: img.clone(), hist: History::default(), saved: Some(img) }
    }
    /// It differs from the disk's, so undoing back to the saved picture clears its dot.
    pub fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.img)
    }
}

/// The block a texture file belongs to: `<block>.png` or `<block>_top/_side/_bottom.png`, by the names given.
pub fn block_of<'a>(file: &str, names: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let stem = file.rsplit(['/', '\\']).next()?.strip_suffix(".png")?;
    names.into_iter().find(|n| names_block(n, &format!("{stem}.png"))).map(String::from)
}

/// `check=1` over `data/`: the palette's colours, the textures that passed, and every refusal.
pub fn check_data(data: &Path) -> Result<(usize, usize, Vec<String>), String> {
    let read = |f: &str| std::fs::read_to_string(data.join(f)).map_err(|e| format!("data/{f}: {e}"));
    let reg = Registry::parse(&read("blocks.toml")?).map_err(|e| format!("data/blocks.toml: {e}"))?;
    let pal = Palette::parse(&read("palette.toml")?).map_err(|e| format!("data/palette.toml: {e}"))?;
    let (ok, mut bad) = check_dir(&data.join("textures"), &pal, &reg);
    // the models too, through the game's own reader (ADR 0049)
    bad.extend(crate::voxel::check_models(&data.join("models"), &pal).1);
    bad.extend(check_structures(&data.join("structures"), &reg));
    bad.extend(check_creatures(data, &pal));
    // the paths as the developer reads them, whatever folder `data` was given as
    let bad = bad.into_iter().map(|b| match b.split_once("/textures/") {
        Some((_, rest)) => format!("data/textures/{rest}"),
        None => b,
    });
    Ok((pal.colours.len(), ok, bad.collect()))
}

/// `data/creatures.toml` through the game's own reader (ADR 0053): its models from `data/models`, its sounds from
/// `data/sounds.toml`; the refusal, naming the row. No file is no refusal.
pub fn check_creatures(data: &Path, pal: &Palette) -> Option<String> {
    let text = std::fs::read_to_string(data.join("creatures.toml")).ok()?.replace("\r\n", "\n");
    let sounds = std::fs::read_to_string(data.join("sounds.toml")).unwrap_or_default();
    let sounds: Vec<String> = crate::rows::spans(&sounds).into_iter().filter(|s| s.header == "sound").map(|s| s.name).collect();
    let model = |n: &str| std::fs::read_to_string(data.join("models").join(format!("{n}.toml"))).ok().and_then(|t| engine::models::Model::parse(n, &t, pal).ok());
    engine::creatures::parse("data/creatures.toml", &text, &model, &|s| sounds.iter().any(|k| k == s)).err()
}

/// Every `data/structures/*.toml` through the game's own reader (ADR 0052); the refusals, each naming its file.
pub fn check_structures(dir: &Path, reg: &Registry) -> Vec<String> {
    let mut files: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".toml")).collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|n| {
            let shown = format!("data/structures/{n}");
            std::fs::read_to_string(dir.join(&n)).map_err(|e| format!("{shown}: {e}")).and_then(|t| engine::structures::Structure::parse(&shown, &t, reg)).err()
        })
        .collect()
}

/// Reads a texture held to the game's rule (`engine::textures::decode`, ADR 0045).
pub fn decode(bytes: &[u8]) -> Result<Img, String> {
    let v = engine::textures::decode(bytes)?;
    let px = (0..S).flat_map(|y| v.iter().flat_map(move |p| p[y * S..(y + 1) * S].iter().map(|c| [c[0], c[1], c[2]]))).collect();
    Ok(Img { w: v.len() * S, px })
}

/// Writes a texture: to a temporary file renamed over it, so a game reading the folder never sees half a file.
/// A `new` file (not loaded from disk) never replaces one that is there: that one was refused or came later.
pub fn save(path: &Path, img: &Img, new: bool) -> Result<(), String> {
    let why = |e: std::io::Error| format!("{}: {e}", path.display());
    if new && path.exists() {
        return Err(format!("{}: on disk but not loaded (refused or made since), not replaced", path.display()));
    }
    let tmp = path.with_extension("png.tmp");
    std::fs::write(&tmp, encode(img)).map_err(why)?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        why(e)
    })
}

pub fn encode(img: &Img) -> Vec<u8> {
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, img.w as u32, S as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let mut wr = e.write_header().expect("a PNG header in memory");
    let data: Vec<u8> = img.px.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    wr.write_image_data(&data).expect("a PNG in memory");
    wr.finish().expect("a PNG in memory");
    out
}

/// The first pixel whose colour is not in the palette, as (x, y) in the file.
pub fn off_palette(img: &Img, pal: &Palette) -> Option<(usize, usize, [u8; 3])> {
    img.px.iter().enumerate().find(|(_, c)| pal.find(**c).is_none()).map(|(n, c)| (n % img.w, n / img.w, *c))
}

/// One texture file against the rule, the palette and the block names; the error names the file.
pub fn check_file(path: &str, bytes: &[u8], pal: &Palette, reg: &Registry) -> Result<Img, String> {
    let stem = path.rsplit(['/', '\\']).next().unwrap_or(path).strip_suffix(".png").unwrap_or("");
    if !reg.placeable().any(|d| names_block(&d.name, &format!("{stem}.png"))) {
        return Err(format!("{path}: names no block (<block>.png, <block>_top, _side or _bottom, each with @spring, @autumn or @winter for a season)"));
    }
    let img = decode(bytes).map_err(|e| format!("{path}: {e}"))?;
    if let Some((x, y, c)) = off_palette(&img, pal) {
        return Err(format!("{path}: pixel ({x}, {y}) is {}, not a palette colour", to_hex(c)));
    }
    Ok(img)
}

/// Every PNG in `dir`, by file name; a missing folder has none.
pub fn pngs(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".png")).collect();
    v.sort();
    v
}

/// `check=1` over a folder: how many files passed, and each refusal.
pub fn check_dir(dir: &Path, pal: &Palette, reg: &Registry) -> (usize, Vec<String>) {
    let (mut ok, mut bad) = (0, vec![]);
    for n in pngs(dir) {
        let path = format!("{}/{n}", dir.display()).replace('\\', "/");
        match std::fs::read(dir.join(&n)).map_err(|e| format!("{path}: {e}")).and_then(|b| check_file(&path, &b, pal, reg)) {
            Ok(_) => ok += 1,
            Err(e) => bad.push(e),
        }
    }
    (ok, bad)
}

/// Replaces one colour by another; whether anything changed.
pub fn repaint(img: &mut Img, from: [u8; 3], to: [u8; 3]) -> bool {
    if from == to {
        return false;
    }
    let mut any = false;
    for c in img.px.iter_mut().filter(|c| **c == from) {
        *c = to;
        any = true;
    }
    any
}

/// Repaints every texture in `dir` that uses `from` and writes it: the files it wrote, and the ones it could not
/// write (each with why; the others are still repainted).
pub fn repaint_dir(dir: &Path, from: [u8; 3], to: [u8; 3]) -> (Vec<String>, Vec<(String, String)>) {
    let (mut done, mut failed) = (vec![], vec![]);
    for n in pngs(dir) {
        let Ok(mut img) = std::fs::read(dir.join(&n)).map_err(|e| e.to_string()).and_then(|b| decode(&b)) else { continue };
        if repaint(&mut img, from, to) {
            match save(&dir.join(&n), &img, false) {
                Ok(()) => done.push(n),
                Err(e) => failed.push((n, e)),
            }
        }
    }
    (done, failed)
}

/// Changes several colours at once (so a pair that swaps two colours works); whether anything changed.
pub fn remap(img: &mut Img, pairs: &[([u8; 3], [u8; 3])]) -> bool {
    let mut any = false;
    for c in img.px.iter_mut() {
        if let Some((_, to)) = pairs.iter().find(|(from, to)| from == c && from != to) {
            *c = *to;
            any = true;
        }
    }
    any
}

/// "Swap ramp": repaints a texture from one ramp to another of the same length, colour for colour.
pub fn swap_ramp(img: &mut Img, from: &[[u8; 3]], to: &[[u8; 3]]) -> Result<bool, String> {
    if from.len() != to.len() {
        return Err(format!("the ramps differ in length ({} and {} colours)", from.len(), to.len()));
    }
    Ok(remap(img, &from.iter().copied().zip(to.iter().copied()).collect::<Vec<_>>()))
}

/// [`remap`] over every texture in `dir`, as [`repaint_dir`] does for one colour.
pub fn remap_dir(dir: &Path, pairs: &[([u8; 3], [u8; 3])]) -> (Vec<String>, Vec<(String, String)>) {
    let (mut done, mut failed) = (vec![], vec![]);
    for n in pngs(dir) {
        let Ok(mut img) = std::fs::read(dir.join(&n)).map_err(|e| e.to_string()).and_then(|b| decode(&b)) else { continue };
        if remap(&mut img, pairs) {
            match save(&dir.join(&n), &img, false) {
                Ok(()) => done.push(n),
                Err(e) => failed.push((n, e)),
            }
        }
    }
    (done, failed)
}

/// The paths `data/drafts.txt` lists, one per line.
pub fn drafts(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()
}

/// The drafts list with `path` on it or off it.
pub fn set_draft(text: &str, path: &str, listed: bool) -> String {
    let mut v: Vec<String> = drafts(text).into_iter().filter(|p| p != path).collect();
    if listed {
        v.push(path.into());
    }
    v.iter().map(|p| format!("{p}\n")).collect()
}

/// Which variant the block at tile (i, j) shows.
/// The variant the game paints on the block at `cell` (`paint_texel` in `crates/client/src/shader.wgsl`); a
/// flat preview lays its tiles at cells (i, j, 0).
pub fn variant_at(cell: [i32; 3], variants: usize) -> usize {
    let n = variants.max(1);
    ((crate::pattern::hash(cell[0], cell[1], cell[2] + 4099) * n as f32) as usize).min(n - 1)
}

/// The texture tiled n x n, variants mixed, each pixel `scale` px: (width, RGBA bytes).
pub fn tiled(img: &Img, n: usize, scale: usize) -> (usize, Vec<u8>) {
    let side = n * S * scale;
    let mut out = Vec::with_capacity(side * side * 4);
    for py in 0..side {
        for px in 0..side {
            let (x, y) = (px / scale, py / scale);
            let c = img.get(variant_at([(x / S) as i32, (y / S) as i32, 0], img.variants()), x % S, y % S);
            out.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    (side, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::palette::Colour;

    const A: [u8; 3] = [10, 20, 30];
    const B: [u8; 3] = [200, 100, 50];
    const C: [u8; 3] = [1, 2, 3];

    fn pal() -> Palette {
        Palette { colours: vec![Colour::new("a", A), Colour::new("b", B)] }
    }
    fn reg() -> Registry {
        Registry::parse("[[block]]\nid = 0\nname = \"air\"\nsolid = false\n[[block]]\nid = 1\nname = \"rock\"\nhardness = 1\n").unwrap()
    }
    fn painted(img: &Img, v: usize) -> Vec<(usize, usize)> {
        (0..S).flat_map(|y| (0..S).map(move |x| (x, y))).filter(|&(x, y)| img.get(v, x, y) == B).collect()
    }
    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("technoape-studio-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn the_pencil_paints_one_pixel_of_its_variant_and_the_mirror_its_twin() {
        let mut img = Img::filled(2, A);
        pencil(&mut img, 1, 3, 5, B, false);
        assert_eq!(painted(&img, 1), vec![(3, 5)]);
        assert_eq!(painted(&img, 0), vec![], "the other variant is untouched");
        pencil(&mut img, 0, 2, 7, B, true);
        assert_eq!(painted(&img, 0), vec![(2, 7), (13, 7)]);
    }

    #[test]
    fn fill_takes_the_connected_area_within_the_variant() {
        let mut img = Img::filled(2, A);
        for y in 0..S {
            pencil(&mut img, 0, 4, y, C, false); // a wall at x = 4
        }
        fill(&mut img, 0, 0, 0, B, false);
        assert_eq!(painted(&img, 0).len(), 4 * S, "left of the wall only");
        assert_eq!(painted(&img, 1), vec![], "not into the next variant");
        fill(&mut img, 0, 0, 0, B, false);
        assert_eq!(painted(&img, 0).len(), 4 * S, "filling with its own colour changes nothing");
        let mut m = Img::filled(1, A);
        for y in 0..S {
            pencil(&mut m, 0, 4, y, C, true); // walls at 4 and 11
        }
        fill(&mut m, 0, 0, 0, B, true);
        assert_eq!(painted(&m, 0).len(), 8 * S, "the mirror fills the right edge too");
    }

    #[test]
    fn a_line_joins_its_ends_without_gaps() {
        let mut img = Img::filled(1, A);
        line(&mut img, 0, (0, 0), (15, 15), B, false);
        assert_eq!(painted(&img, 0), (0..S).map(|i| (i, i)).collect::<Vec<_>>());
        let mut img = Img::filled(1, A);
        line(&mut img, 0, (2, 9), (9, 6), B, false);
        let p = painted(&img, 0);
        assert_eq!(p.len(), 8, "one pixel per column of a shallow line: {p:?}");
        assert!(p.contains(&(2, 9)) && p.contains(&(9, 6)));
        let mut img = Img::filled(1, A);
        line(&mut img, 0, (1, 3), (1, 5), B, true);
        assert_eq!(painted(&img, 0), vec![(1, 3), (14, 3), (1, 4), (14, 4), (1, 5), (14, 5)]);
    }

    #[test]
    fn undo_and_redo_step_through_the_snapshots() {
        let mut img = Img::filled(1, A);
        let mut h = History::default();
        let first = img.clone();
        h.push(img.clone(), "Pencil");
        pencil(&mut img, 0, 0, 0, B, false);
        let second = img.clone();
        h.push(img.clone(), "Fill");
        pencil(&mut img, 0, 1, 0, B, false);
        let third = img.clone();
        assert!(h.undo(&mut img) && img == second);
        assert!(h.undo(&mut img) && img == first);
        assert!(!h.undo(&mut img) && img == first, "nothing more to undo");
        assert!(h.redo(&mut img) && img == second);
        assert!(h.redo(&mut img) && img == third);
        assert!(!h.redo(&mut img));
        h.undo(&mut img);
        h.push(img.clone(), "Line");
        assert!(h.redo.is_empty(), "a new edit drops the redo");
    }

    #[test]
    fn the_history_names_its_steps_and_a_click_goes_back_to_one() {
        let mut img = Img::filled(1, A);
        let mut h = History::default();
        let pics: Vec<Img> = (0..4)
            .map(|i| {
                let before = img.clone();
                h.push(img.clone(), ["Pencil", "Fill", "Shade", "Line"][i]);
                pencil(&mut img, 0, i, 0, B, false);
                before
            })
            .collect();
        let last = img.clone();
        assert_eq!(h.undo.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Pencil", "Fill", "Shade", "Line"]);
        h.back_to(&mut img, 1);
        assert_eq!((h.undo.len(), h.redo.len(), &img), (1, 3, &pics[1]), "back to after the first step");
        assert_eq!(h.redo.last().unwrap().name, "Fill", "the next redo is the step after it");
        h.back_to(&mut img, 4);
        assert_eq!(img, last, "and forward again");
        h.back_to(&mut img, 0);
        assert_eq!(img, pics[0]);
    }

    #[test]
    fn a_new_variant_copies_one_and_stops_at_four() {
        let mut img = Img::filled(1, A);
        pencil(&mut img, 0, 3, 3, B, false);
        let two = add_variant(&img, 0).unwrap();
        assert_eq!((two.w, painted(&two, 1)), (32, vec![(3, 3)]));
        let four = add_variant(&add_variant(&two, 1).unwrap(), 0).unwrap();
        assert_eq!(four.variants(), 4);
        assert!(add_variant(&four, 0).is_none());
    }

    #[test]
    fn a_texture_round_trips_through_its_png() {
        let mut img = Img::filled(3, A);
        pencil(&mut img, 2, 15, 15, B, false);
        assert_eq!(decode(&encode(&img)).unwrap(), img);
    }

    #[test]
    fn check_names_the_file_and_the_pixel() {
        let (p, r) = (pal(), reg());
        let mut img = Img::filled(2, A);
        assert!(check_file("data/textures/rock.png", &encode(&img), &p, &r).is_ok());
        pencil(&mut img, 1, 2, 5, C, false);
        let e = check_file("data/textures/rock_top.png", &encode(&img), &p, &r).unwrap_err();
        assert_eq!(e, "data/textures/rock_top.png: pixel (18, 5) is #010203, not a palette colour");
        let wide = Img { w: 20, px: vec![A; 20 * S] };
        let e = check_file("data/textures/rock.png", &encode(&wide), &p, &r).unwrap_err();
        assert!(e.starts_with("data/textures/rock.png: 20 x 16 px"), "{e}");
        let e = check_file("data/textures/sand.png", &encode(&Img::filled(1, A)), &p, &r).unwrap_err();
        assert!(e.contains("sand.png: names no block"), "{e}");
        let d = temp("check");
        std::fs::write(d.join("rock.png"), encode(&Img::filled(1, A))).unwrap();
        std::fs::write(d.join("rock_side.png"), encode(&img)).unwrap();
        let (ok, bad) = check_dir(&d, &p, &r);
        assert_eq!(ok, 1);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].ends_with("/rock_side.png: pixel (18, 5) is #010203, not a palette colour"), "{bad:?}");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_translucent_or_grey_png_is_refused() {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, 16, 16);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut w = e.write_header().unwrap();
        let mut data = vec![255u8; 16 * 16 * 4];
        data[(3 * 16 + 4) * 4 + 3] = 128;
        w.write_image_data(&data).unwrap();
        w.finish().unwrap();
        assert_eq!(decode(&out).unwrap_err(), "pixel (4, 3) is not opaque");
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, 16, 16);
        e.set_color(png::ColorType::Grayscale);
        let mut w = e.write_header().unwrap();
        w.write_image_data(&[0; 256]).unwrap();
        w.finish().unwrap();
        assert!(decode(&out).unwrap_err().contains("8-bit RGBA"));
    }

    #[test]
    fn changing_a_palette_colour_repaints_the_textures_that_use_it() {
        let d = temp("repaint");
        let mut img = Img::filled(1, A);
        pencil(&mut img, 0, 0, 0, B, false);
        std::fs::write(d.join("rock.png"), encode(&img)).unwrap();
        std::fs::write(d.join("rock_top.png"), encode(&Img::filled(1, A))).unwrap();
        assert_eq!(repaint_dir(&d, B, C), (vec!["rock.png".to_string()], vec![]));
        let back = decode(&std::fs::read(d.join("rock.png")).unwrap()).unwrap();
        assert_eq!((back.get(0, 0, 0), back.get(0, 1, 0)), (C, A));
        assert_eq!(decode(&std::fs::read(d.join("rock_top.png")).unwrap()).unwrap(), Img::filled(1, A));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_colour_changed_to_itself_rewrites_nothing() {
        let mut img = Img::filled(1, A);
        assert!(!repaint(&mut img, A, A), "nothing changed");
        let d = temp("same");
        std::fs::write(d.join("rock.png"), encode(&img)).unwrap();
        assert_eq!(repaint_dir(&d, A, A), (vec![], vec![]));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_file_that_cannot_be_written_does_not_stop_the_repaint_of_the_others() {
        let d = temp("locked");
        for n in ["a.png", "b.png"] {
            std::fs::write(d.join(n), encode(&Img::filled(1, A))).unwrap();
        }
        let mut ro = std::fs::metadata(d.join("a.png")).unwrap().permissions();
        ro.set_readonly(true);
        std::fs::set_permissions(d.join("a.png"), ro.clone()).unwrap();
        let (done, failed) = repaint_dir(&d, A, B);
        assert_eq!(done, vec!["b.png".to_string()]);
        assert_eq!(failed.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(), vec!["a.png"]);
        assert_eq!(decode(&std::fs::read(d.join("b.png")).unwrap()).unwrap(), Img::filled(1, B));
        #[allow(clippy::permissions_set_readonly_false)]
        ro.set_readonly(false);
        std::fs::set_permissions(d.join("a.png"), ro).unwrap();
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_new_file_never_replaces_one_on_disk_and_a_save_leaves_no_temporary() {
        let d = temp("save");
        let p = d.join("rock.png");
        std::fs::write(&p, b"a file the Studio refused").unwrap();
        assert!(save(&p, &Img::filled(1, B), true).is_err(), "a new picture over a file it never loaded");
        assert_eq!(std::fs::read(&p).unwrap(), b"a file the Studio refused");
        save(&p, &Img::filled(1, B), false).unwrap();
        assert_eq!(decode(&std::fs::read(&p).unwrap()).unwrap(), Img::filled(1, B));
        save(&d.join("rock_top.png"), &Img::filled(1, A), true).unwrap();
        assert_eq!(pngs(&d), vec!["rock.png", "rock_top.png"]);
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 2, "no temporary left");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_texture_file_names_its_block() {
        let names = ["grass", "dirt", "blue lamp"];
        assert_eq!(block_of("data/textures/grass_top.png", names), Some("grass".into()));
        assert_eq!(block_of("dirt.png", names), Some("dirt".into()));
        assert_eq!(block_of("blue lamp_side.png", names), Some("blue lamp".into()));
        assert_eq!(block_of("sand.png", names), None);
        assert_eq!(block_of("grass_left.png", names), None);
        // a season's files (docs/design/seasonal-looks.md); summer's is the base file
        assert_eq!(block_of("grass_top@autumn.png", names), Some("grass".into()));
        assert_eq!(block_of("dirt@winter.png", names), Some("dirt".into()));
        assert_eq!(block_of("dirt@summer.png", names), None);
        assert_eq!(block_of("dirt@fall.png", names), None);
        assert_eq!((season_file("grass", 2, 0), season_file("grass", 0, 1)), ("grass_side@spring.png".into(), "grass.png".into()));
    }

    #[test]
    fn remap_changes_colours_at_once_and_swap_ramp_needs_equal_lengths() {
        let mut img = Img::filled(1, A);
        pencil(&mut img, 0, 0, 0, B, false);
        assert!(remap(&mut img, &[(A, B), (B, A)]));
        assert_eq!((img.get(0, 0, 0), img.get(0, 1, 0)), (A, B), "swapped, not both one colour");
        assert!(!remap(&mut img, &[(C, A), (A, A)]), "nothing to change");
        let mut s = Img::filled(1, A);
        pencil(&mut s, 0, 3, 3, B, false);
        assert_eq!(swap_ramp(&mut s, &[A, B], &[C, A]), Ok(true));
        assert_eq!((s.get(0, 0, 0), s.get(0, 3, 3)), (C, A));
        assert!(swap_ramp(&mut s, &[A, B], &[C]).is_err());
        let d = temp("remap");
        std::fs::write(d.join("rock.png"), encode(&Img::filled(1, A))).unwrap();
        assert_eq!(remap_dir(&d, &[(A, C)]), (vec!["rock.png".to_string()], vec![]));
        assert_eq!(decode(&std::fs::read(d.join("rock.png")).unwrap()).unwrap(), Img::filled(1, C));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_drafts_list_takes_files_on_and_off() {
        let t = "data/palette.toml\n\ndata/textures/rock.png\n";
        assert_eq!(drafts(t), vec!["data/palette.toml", "data/textures/rock.png"]);
        assert_eq!(drafts(""), Vec::<String>::new());
        let t = set_draft(t, "data/textures/rock.png", false);
        assert_eq!(t, "data/palette.toml\n");
        let t = set_draft(&t, "data/textures/rock_top.png", true);
        assert_eq!(drafts(&t), vec!["data/palette.toml", "data/textures/rock_top.png"]);
        assert_eq!(set_draft(&t, "data/textures/rock_top.png", true), t, "listed once");
    }

    #[test]
    fn the_sheet_tiles_the_face_three_by_three_with_every_variant() {
        let mut img = Img::filled(2, A);
        fill(&mut img, 1, 0, 0, B, false);
        let (side, px) = tiled(&img, 3, 2);
        assert_eq!((side, px.len()), (96, 96 * 96 * 4));
        let tiles: Vec<[u8; 3]> = (0..9).map(|t| (t % 3 * 32, t / 3 * 32)).map(|(x, y)| (y * side + x) * 4).map(|o| [px[o], px[o + 1], px[o + 2]]).collect();
        assert!(tiles.contains(&A) && tiles.contains(&B), "both variants show: {tiles:?}");
    }

    #[test]
    fn a_variant_is_removed_and_a_flat_one_is_seen() {
        let mut img = Img::filled(3, A);
        pencil(&mut img, 1, 2, 3, B, false);
        pencil(&mut img, 2, 0, 0, C, false);
        assert!(flat(&img, 0) && !flat(&img, 1) && !flat(&img, 2));
        let r = remove_variant(&img, 1).unwrap();
        assert_eq!((r.w, r.variants()), (32, 2));
        assert!(flat(&r, 0) && r.get(1, 0, 0) == C, "variant 3 moved into 2's place");
        assert_eq!(remove_variant(&remove_variant(&r, 0).unwrap(), 0), None, "the last variant stays");
    }

    #[test]
    fn the_preview_picks_variants_as_the_game_does() {
        // `paint_texel` in the shader, word for word: a change there must come here too
        let wgsl = include_str!("../../client/src/shader.wgsl");
        assert!(wgsl.contains("let v = min(u32(hash(cell + vec3i(0, 0, 4099)) * f32(nv)), nv - 1u);"), "the shader's pick changed");
        let game = |c: [i32; 3], nv: u32| ((crate::pattern::hash(c[0], c[1], c[2] + 4099) * nv as f32) as u32).min(nv - 1) as usize;
        for nv in 1..=4u32 {
            for j in -3..8 {
                for i in -3..8 {
                    assert_eq!(variant_at([i, j, 0], nv as usize), game([i, j, 0], nv), "cell {i} {j}, {nv} variants");
                }
            }
        }
        // a field of 64 cells with 4 variants uses them all
        let seen: std::collections::BTreeSet<usize> = (0..64).map(|k| variant_at([k % 8, k / 8, 0], 4)).collect();
        assert_eq!(seen.len(), 4);
    }
}
