//! Hand-painted block textures (ADR 0045, docs/design/studio.md "Textures"): `data/textures/<block>.png` is a
//! block's every face, `<block>_top.png`, `_side.png` (the four sides) and `_bottom.png` override one. The rule:
//! 8-bit RGBA, 16 px high, 16, 32, 48 or 64 px wide (1 to 4 variants side by side), or 32 px high and 32 to 128
//! wide for a fine painting (32 px a metre: flora, ADR 0090), every pixel opaque, or for a
//! cutout block (`see = "cutout"`, ADR 0069) every pixel wholly there or wholly empty (alpha 255 or 0). A file
//! that breaks it is refused by name and its block keeps its pattern. A season's paint is the same name with
//! `@spring`, `@autumn` or `@winter` before `.png` (docs/design/seasonal-looks.md); the file without one is
//! summer's and the base, and a face a season does not paint is the base's. No GPU here, so the Studio can run the
//! same check.
use crate::blocks::{Registry, See};
use std::path::Path;

/// One 16 x 16 variant, rows from the top, RGBA. The bytes are the same numbers as `blocks.toml`'s colours
/// (x 255).
pub type Pixels = [[u8; 4]; 256];

/// The faces in the order `Painted::faces` holds them.
pub const FACES: [&str; 3] = ["top", "side", "bottom"];

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    /// Each variant 16 x 16; a fine face's are its 32 x 32 variants' quarters in turn: top left, top right,
    /// bottom left, bottom right (ADR 0090).
    pub variants: Vec<Pixels>,
    pub fine: bool,
    /// The mean of every pixel that is there (alpha 255) of every variant, 0..1, in `colour`'s numbers.
    pub mean: [f32; 3],
}

/// A block's painted faces: top, side, bottom; `None` keeps the pattern.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Painted {
    pub faces: [Option<Face>; 3],
}

impl Painted {
    /// The block's one colour (far terrain, the bounce light): the top's mean, else the side's.
    pub fn colour(&self) -> Option<[f32; 3]> {
        self.faces[0].as_ref().or(self.faces[1].as_ref()).map(|f| f.mean)
    }
}

/// Every painted block of a folder, and why each refused file was refused (its name first).
#[derive(Default, Debug)]
pub struct Set {
    pub blocks: Vec<(u16, Painted)>,
    /// The seasons' paint: block, season (its row in `blocks::Def::tone`: 1 spring, 2 autumn, 3 winter) and its
    /// faces, each face its own or else the base's.
    pub seasons: Vec<(u16, usize, Painted)>,
    pub refused: Vec<String>,
}

/// One file's variants for a block seen as `see`, or why it breaks the rule (a fine file's quarters, `Face`).
pub fn decode(bytes: &[u8], see: See) -> Result<Vec<Pixels>, String> {
    decode_fine(bytes, see).map(|(v, _)| v)
}

/// `decode`, and whether the file is fine (32 px high).
pub fn decode_fine(bytes: &[u8], see: See) -> Result<(Vec<Pixels>, bool), String> {
    let mut r = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().map_err(|e| format!("not a PNG ({e})"))?;
    let i = r.info();
    let (w, h, colour, depth) = (i.width, i.height, i.color_type, i.bit_depth);
    if colour != png::ColorType::Rgba || depth != png::BitDepth::Eight {
        return Err(format!("{colour:?} at {depth:?} bits, a texture is 8-bit RGBA"));
    }
    if !(h == 16 && matches!(w, 16 | 32 | 48 | 64) || h == 32 && matches!(w, 32 | 64 | 96 | 128)) {
        return Err(format!("{w} x {h} px, a texture is 16 high and 16, 32, 48 or 64 wide, or 32 high and 32, 64, 96 or 128 wide"));
    }
    let mut buf = vec![0; r.output_buffer_size().ok_or("too large")?];
    r.next_frame(&mut buf).map_err(|e| format!("unreadable ({e})"))?;
    let empty_ok = see == See::Cutout;
    // glass takes any alpha (ADR 0081)
    if let Some(p) = buf.chunks_exact(4).position(|p| p[3] != 255 && !(empty_ok && p[3] == 0) && see != See::Glass) {
        let (x, y) = (p as u32 % w, p as u32 / w);
        return Err(if empty_ok { format!("pixel ({x}, {y}) is partly see-through (alpha {}), a cutout block's pixels are 255 or 0", buf[p * 4 + 3]) } else { format!("pixel ({x}, {y}) is not opaque") });
    }
    // 16 x 16 tiles: a fine variant's four quarters in turn
    let fine = h == 32;
    let tile = |x0: usize, y0: usize| -> Pixels {
        std::array::from_fn(|k| {
            let at = ((y0 + k / 16) * w as usize + x0 + k % 16) * 4;
            [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
        })
    };
    let tiles = if fine {
        (0..w as usize / 32).flat_map(|v| [(0, 0), (16, 0), (0, 16), (16, 16)].map(|(x, y)| tile(v * 32 + x, y))).collect()
    } else {
        (0..w as usize / 16).map(|v| tile(v * 16, 0)).collect()
    };
    Ok((tiles, fine))
}

fn face((variants, fine): (Vec<Pixels>, bool)) -> Face {
    // an empty pixel is not seen: the mean is of those there (a face wholly empty is black)
    let n = (variants.iter().flatten().filter(|p| p[3] != 0).count() as f32).max(1.0);
    let mean = std::array::from_fn(|c| variants.iter().flatten().filter(|p| p[3] != 0).map(|p| p[c] as f32).sum::<f32>() / n / 255.0);
    Face { variants, fine, mean }
}

/// Each file's last good variants, by file name: what a running game keeps showing while a file is bad.
pub type Kept = std::collections::BTreeMap<String, (Vec<Pixels>, bool)>;

/// `load` for a running game: a file refused now that was good before keeps its last good paint (still named
/// in `refused`).
pub fn load_keeping(dir: &Path, reg: &Registry, kept: &mut Kept) -> Set {
    load_with(dir, reg, Some(kept))
}

/// Reads every `.png` of `dir` against the block names of `reg`. A missing folder is an empty set.
pub fn load(dir: &Path, reg: &Registry) -> Set {
    load_with(dir, reg, None)
}

fn load_with(dir: &Path, reg: &Registry, mut kept: Option<&mut Kept>) -> Set {
    let mut set = Set::default();
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".png"))
        .collect();
    names.sort();
    // per block and season (0 the base, summer's): its <block>.png, then its own top, side, bottom
    let mut found: Vec<((u16, usize), [Option<Face>; 4])> = Vec::new();
    let block = |n: &str| reg.placeable().find(|d| d.name == n).map(|d| d.id);
    for name in names {
        let full = &name[..name.len() - 4];
        let (stem, season) = match full.rsplit_once('@') {
            None => (full, 0),
            Some((stem, q)) => match crate::blocks::SEASONS.iter().position(|&n| n == q).map(|k| crate::blocks::ROW[k]) {
                Some(row) if row != 0 => (stem, row),
                _ => {
                    set.refused.push(format!("{name}: no season '{q}' (a season is @spring, @autumn or @winter; summer's is the file without one)"));
                    continue;
                }
            },
        };
        let which = block(stem).map(|id| (id, 0)).or_else(|| {
            FACES.iter().enumerate().find_map(|(f, s)| stem.strip_suffix(&format!("_{s}")).and_then(block).map(|id| (id, f + 1)))
        });
        let Some((id, slot)) = which else {
            set.refused.push(format!("{name}: no block '{stem}' in data/blocks.toml (a face is _top, _side or _bottom)"));
            continue;
        };
        let id = (id, season);
        let got = std::fs::read(dir.join(&name)).map_err(|e| e.to_string()).and_then(|b| decode_fine(&b, reg.get(id.0).map_or(See::Opaque, |d| d.see)));
        let got = match (got, kept.as_deref_mut()) {
            (Ok(v), Some(k)) => {
                k.insert(name.clone(), v.clone());
                Ok(v)
            }
            // refused now, but painted before: named, and the last good paint stays
            (Err(why), Some(k)) if k.contains_key(&name) => {
                set.refused.push(format!("{name}: {why}; its last good paint stays"));
                Ok(k[&name].clone())
            }
            (got, _) => got,
        };
        match got {
            Err(why) => set.refused.push(format!("{name}: {why}")),
            Ok(v) => {
                if !found.iter().any(|(b, _)| *b == id) {
                    found.push((id, Default::default()));
                }
                found.iter_mut().find(|(b, _)| *b == id).unwrap().1[slot] = Some(face(v));
            }
        }
    }
    if let Some(k) = kept {
        k.retain(|n, _| dir.join(n).exists());
    }
    let faces = |[all, top, side, bottom]: [Option<Face>; 4]| [top, side, bottom].map(|f| f.or_else(|| all.clone()));
    for ((id, _), f) in found.iter().filter(|((_, q), _)| *q == 0) {
        set.blocks.push((*id, Painted { faces: faces(f.clone()) }));
    }
    for ((id, q), f) in found.iter().filter(|((_, q), _)| *q != 0) {
        let base = set.blocks.iter().find(|(b, _)| b == id).map(|(_, p)| p.faces.clone()).unwrap_or_default();
        let own = faces(f.clone());
        set.seasons.push((*id, *q, Painted { faces: std::array::from_fn(|i| own[i].clone().or_else(|| base[i].clone())) }));
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BLOCKS;

    fn png(w: u32, h: u32, colour: png::ColorType, depth: png::BitDepth, px: impl Fn(u32, u32) -> Vec<u8>) -> Vec<u8> {
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, w, h);
        e.set_color(colour);
        e.set_depth(depth);
        let mut wr = e.write_header().unwrap();
        let px = &px;
        let data: Vec<u8> = (0..h).flat_map(|y| (0..w).flat_map(move |x| px(x, y))).collect();
        wr.write_image_data(&data).unwrap();
        wr.finish().unwrap();
        out
    }
    fn rgba(w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        png(w, h, png::ColorType::Rgba, png::BitDepth::Eight, move |x, y| px(x, y).to_vec())
    }

    #[test]
    fn the_rule_takes_good_files_and_counts_their_variants() {
        for (w, n) in [(16, 1), (32, 2), (48, 3), (64, 4)] {
            let v = decode(&rgba(w, 16, |x, y| [(x / 16) as u8, x as u8, y as u8, 255]), See::Opaque).unwrap();
            assert_eq!(v.len(), n, "{w} px wide");
            for (i, p) in v.iter().enumerate() {
                assert_eq!(p[3 * 16 + 5], [i as u8, (i * 16 + 5) as u8, 3, 255], "variant {i}, pixel (5, 3)");
            }
        }
    }

    /// ADR 0090: a fine painting, 32 px high, is 32 x 32 variants, each kept as its four quarters in turn.
    #[test]
    fn a_fine_file_is_32_high_and_kept_as_quarters() {
        for (w, n) in [(32, 1), (64, 2), (128, 4)] {
            let (v, fine) = decode_fine(&rgba(w, 32, |x, y| [(x / 32) as u8, x as u8, y as u8, 255]), See::Opaque).unwrap();
            assert!(fine);
            assert_eq!(v.len(), 4 * n, "{w} px wide");
            for i in 0..n {
                // pixel (21, 19) of variant i: its bottom right quarter, (5, 3) in it
                assert_eq!(v[4 * i + 3][3 * 16 + 5], [i as u8, (i * 32 + 21) as u8, 19, 255], "variant {i}");
                assert_eq!(v[4 * i + 1][0], [i as u8, (i * 32 + 16) as u8, 0, 255], "variant {i}, top right");
            }
        }
        assert!(!decode_fine(&rgba(32, 16, |_, _| [9, 9, 9, 255]), See::Opaque).unwrap().1, "16 high is not fine");
        for (w, h) in [(48, 32), (160, 32), (32, 24), (16, 32)] {
            assert!(decode(&rgba(w, h, |_, _| [9, 9, 9, 255]), See::Opaque).is_err(), "{w} x {h}");
        }
    }

    #[test]
    fn the_rule_refuses_each_kind_of_bad_file() {
        let wrong = [
            (b"not a png at all".to_vec(), "not a PNG"),
            (png(16, 16, png::ColorType::Rgb, png::BitDepth::Eight, |_, _| vec![9; 3]), "RGB"),
            (png(16, 16, png::ColorType::Rgba, png::BitDepth::Sixteen, |_, _| vec![255; 8]), "16-bit"),
            (png(16, 16, png::ColorType::GrayscaleAlpha, png::BitDepth::Eight, |_, _| vec![9, 255]), "grey"),
            (rgba(16, 8, |_, _| [9, 9, 9, 255]), "8 high"),
            (rgba(24, 16, |_, _| [9, 9, 9, 255]), "24 wide"),
            (rgba(80, 16, |_, _| [9, 9, 9, 255]), "5 variants"),
            (rgba(16, 16, |x, y| [9, 9, 9, if (x, y) == (7, 7) { 254 } else { 255 }]), "a see-through pixel"),
        ];
        for (bytes, why) in wrong {
            assert!(decode(&bytes, See::Opaque).is_err(), "{why}");
            assert!(decode(&bytes, See::Cutout).is_err(), "{why}, cutout");
        }
    }

    /// ADR 0069: a cutout block may have empty pixels, nothing in between; an opaque block none, as before.
    #[test]
    fn a_cutout_block_may_have_empty_pixels_and_an_opaque_one_not() {
        let holed = rgba(16, 16, |x, y| [9, 9, 9, if (x + y) % 3 == 0 { 0 } else { 255 }]);
        assert_eq!(decode(&holed, See::Opaque).unwrap_err(), "pixel (0, 0) is not opaque");
        let v = decode(&holed, See::Cutout).unwrap();
        assert_eq!((v[0][0][3], v[0][1][3]), (0, 255), "the alpha byte is kept");
        let half = rgba(16, 16, |x, y| [9, 9, 9, if (x, y) == (3, 2) { 128 } else { 255 }]);
        assert!(decode(&half, See::Cutout).unwrap_err().contains("pixel (3, 2) is partly see-through (alpha 128)"));
        // glass takes any alpha (ADR 0081)
        assert_eq!(decode(&half, See::Glass).unwrap()[0][2 * 16 + 3][3], 128);
        assert!(decode(&holed, See::Glass).is_ok());
        // the mean leaves the empty pixels out
        let f = face((vec![std::array::from_fn(|k| if k % 2 == 0 { [0, 0, 0, 0] } else { [255, 0, 0, 255] })], false));
        assert_eq!(f.mean, [1.0, 0.0, 0.0]);
    }

    fn folder(name: &str, files: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("technoape-textures-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        for (n, b) in files {
            std::fs::write(d.join(n), b).unwrap();
        }
        d
    }

    #[test]
    fn a_face_file_wins_over_the_block_file_and_bad_files_are_refused_by_name() {
        let flat = |c: u8| rgba(16, 16, move |_, _| [c, c, c, 255]);
        let d = folder(
            "faces",
            &[
                ("stone.png", flat(10)),
                ("stone_top.png", flat(20)),
                ("dirt_side.png", rgba(32, 16, |_, _| [30, 30, 30, 255])),
                ("grass_bottom.png", rgba(16, 15, |_, _| [9, 9, 9, 255])),
                ("nosuchblock.png", flat(40)),
                ("dirt_front.png", flat(50)),
                ("notes.txt", b"not a texture".to_vec()),
            ],
        );
        let s = load(&d, &BLOCKS);
        let get = |n: &str| s.blocks.iter().find(|(id, _)| *id == BLOCKS.id(n)).map(|(_, p)| p);
        let stone = get("stone").unwrap();
        let first = |f: &Option<Face>| f.as_ref().map(|f| (f.variants.len(), f.variants[0][0][0]));
        assert_eq!(stone.faces.each_ref().map(first), [Some((1, 20)), Some((1, 10)), Some((1, 10))], "top its own, the rest stone.png");
        let dirt = get("dirt").unwrap();
        assert_eq!(dirt.faces.each_ref().map(first), [None, Some((2, 30)), None], "only the sides, two variants");
        assert!(get("grass").is_none(), "a refused file leaves its block unpainted");
        assert_eq!(s.refused.len(), 3, "{:?}", s.refused);
        for n in ["grass_bottom.png", "nosuchblock.png", "dirt_front.png"] {
            assert!(s.refused.iter().any(|r| r.starts_with(n)), "{n} refused by name: {:?}", s.refused);
        }
        let _ = std::fs::remove_dir_all(&d);
        assert!(load(&d, &BLOCKS).blocks.is_empty(), "no folder, nothing painted");
    }

    #[test]
    fn a_running_game_keeps_a_files_last_good_paint_when_it_turns_bad() {
        let flat = |c: u8| rgba(16, 16, move |_, _| [c, c, c, 255]);
        let d = folder("keep", &[("grass_top.png", flat(70))]);
        let mut kept = Kept::default();
        let top = |s: &Set| s.blocks.iter().find(|(id, _)| *id == BLOCKS.id("grass")).and_then(|(_, p)| p.faces[0].as_ref().map(|f| f.variants[0][0][0]));
        assert_eq!(top(&load_keeping(&d, &BLOCKS, &mut kept)), Some(70));
        std::fs::write(d.join("grass_top.png"), rgba(16, 17, |_, _| [9, 9, 9, 255])).unwrap();
        let s = load_keeping(&d, &BLOCKS, &mut kept);
        assert_eq!(top(&s), Some(70), "the old paint stays");
        assert!(s.refused.iter().any(|r| r.starts_with("grass_top.png")), "and the file is named: {:?}", s.refused);
        std::fs::write(d.join("grass_top.png"), flat(90)).unwrap();
        assert_eq!(top(&load_keeping(&d, &BLOCKS, &mut kept)), Some(90), "a good file again is taken");
        std::fs::remove_file(d.join("grass_top.png")).unwrap();
        assert_eq!(top(&load_keeping(&d, &BLOCKS, &mut kept)), None, "a file gone is gone");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_season_paints_beside_the_base_and_takes_its_faces() {
        let flat = |c: u8| rgba(16, 16, move |_, _| [c, c, c, 255]);
        let d = folder(
            "seasons",
            &[
                ("grass.png", flat(10)),
                ("grass_top@autumn.png", flat(20)),
                ("broadleaf leaves@winter.png", flat(30)),
                ("grass@summer.png", flat(40)),
                ("grass@fall.png", flat(50)),
                ("nosuchblock@spring.png", flat(60)),
            ],
        );
        let s = load(&d, &BLOCKS);
        let first = |f: &Option<Face>| f.as_ref().map(|f| f.variants[0][0][0]);
        let season = |n: &str, q: usize| s.seasons.iter().find(|(id, k, _)| *id == BLOCKS.id(n) && *k == q).map(|(_, _, p)| p.faces.each_ref().map(first));
        assert_eq!(s.blocks.iter().find(|(id, _)| *id == BLOCKS.id("grass")).map(|(_, p)| p.faces.each_ref().map(first)), Some([Some(10); 3]), "the base is summer's");
        assert_eq!(season("grass", 2), Some([Some(20), Some(10), Some(10)]), "autumn's top, the base's side and bottom");
        assert_eq!(season("broadleaf leaves", 3), Some([Some(30); 3]), "a season with no base paint");
        assert!(s.blocks.iter().all(|(id, _)| *id != BLOCKS.id("broadleaf leaves")), "and the base keeps its pattern");
        assert_eq!(s.seasons.len(), 2, "{:?}", s.refused);
        for n in ["grass@summer.png", "grass@fall.png", "nosuchblock@spring.png"] {
            assert!(s.refused.iter().any(|r| r.starts_with(n)), "{n} refused by name: {:?}", s.refused);
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_one_colour_is_the_mean_of_the_top_else_the_side() {
        // half the pixels 0, half 255 in red; green 51 everywhere; blue 0 in variant one, 102 in variant two
        let d = folder(
            "mean",
            &[
                ("copper_top.png", rgba(32, 16, |x, y| [if (x + y) % 2 == 0 { 255 } else { 0 }, 51, if x < 16 { 0 } else { 102 }, 255])),
                ("copper_side.png", rgba(16, 16, |_, _| [0, 0, 255, 255])),
                ("lamp_side.png", rgba(16, 16, |_, _| [0, 255, 0, 255])),
            ],
        );
        let s = load(&d, &BLOCKS);
        let get = |n: &str| &s.blocks.iter().find(|(id, _)| *id == BLOCKS.id(n)).unwrap().1;
        let near = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
        let copper = get("copper");
        assert!(near(copper.faces[0].as_ref().unwrap().mean, [0.5, 0.2, 0.2]), "{:?}", copper.faces[0].as_ref().unwrap().mean);
        assert!(near(copper.colour().unwrap(), [0.5, 0.2, 0.2]), "the top's");
        assert!(near(get("lamp").colour().unwrap(), [0.0, 1.0, 0.0]), "no top: the side's");
        let _ = std::fs::remove_dir_all(&d);
    }
}
