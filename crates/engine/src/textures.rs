//! Hand-painted block textures (ADR 0045, docs/design/studio.md "Textures"): `data/textures/<block>.png` is a
//! block's every face, `<block>_top.png`, `_side.png` (the four sides) and `_bottom.png` override one. The rule:
//! 8-bit RGBA, 16 px high, 16, 32, 48 or 64 px wide (1 to 4 variants side by side), every pixel opaque. A file
//! that breaks it is refused by name and its block keeps its pattern. No GPU here, so the Studio can run the
//! same check.
use crate::blocks::Registry;
use std::path::Path;

/// One 16 x 16 variant, rows from the top, RGBA. The bytes are the same numbers as `blocks.toml`'s colours
/// (x 255).
pub type Pixels = [[u8; 4]; 256];

/// The faces in the order `Painted::faces` holds them.
pub const FACES: [&str; 3] = ["top", "side", "bottom"];

#[derive(Clone, Debug, PartialEq)]
pub struct Face {
    pub variants: Vec<Pixels>,
    /// The mean of every pixel of every variant, 0..1, in `colour`'s numbers.
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
    pub refused: Vec<String>,
}

/// One file's variants, or why it breaks the rule.
pub fn decode(bytes: &[u8]) -> Result<Vec<Pixels>, String> {
    let mut r = png::Decoder::new(std::io::Cursor::new(bytes)).read_info().map_err(|e| format!("not a PNG ({e})"))?;
    let i = r.info();
    let (w, h, colour, depth) = (i.width, i.height, i.color_type, i.bit_depth);
    if colour != png::ColorType::Rgba || depth != png::BitDepth::Eight {
        return Err(format!("{colour:?} at {depth:?} bits, a texture is 8-bit RGBA"));
    }
    if h != 16 || !matches!(w, 16 | 32 | 48 | 64) {
        return Err(format!("{w} x {h} px, a texture is 16 high and 16, 32, 48 or 64 wide"));
    }
    let mut buf = vec![0; r.output_buffer_size().ok_or("too large")?];
    r.next_frame(&mut buf).map_err(|e| format!("unreadable ({e})"))?;
    if let Some(p) = buf.chunks_exact(4).position(|p| p[3] != 255) {
        return Err(format!("pixel ({}, {}) is not opaque", p as u32 % w, p as u32 / w));
    }
    Ok((0..w as usize / 16)
        .map(|v| std::array::from_fn(|k| {
            let at = ((k / 16) * w as usize + v * 16 + k % 16) * 4;
            [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
        }))
        .collect())
}

fn face(variants: Vec<Pixels>) -> Face {
    let n = (variants.len() * 256) as f32;
    let mean = std::array::from_fn(|c| variants.iter().flatten().map(|p| p[c] as f32).sum::<f32>() / n / 255.0);
    Face { variants, mean }
}

/// Each file's last good variants, by file name: what a running game keeps showing while a file is bad.
pub type Kept = std::collections::BTreeMap<String, Vec<Pixels>>;

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
    // per block: its <block>.png, then its own top, side, bottom
    let mut found: Vec<(u16, [Option<Face>; 4])> = Vec::new();
    let block = |n: &str| reg.placeable().find(|d| d.name == n).map(|d| d.id);
    for name in names {
        let stem = &name[..name.len() - 4];
        let which = block(stem).map(|id| (id, 0)).or_else(|| {
            FACES.iter().enumerate().find_map(|(f, s)| stem.strip_suffix(&format!("_{s}")).and_then(block).map(|id| (id, f + 1)))
        });
        let Some((id, slot)) = which else {
            set.refused.push(format!("{name}: no block '{stem}' in data/blocks.toml (a face is _top, _side or _bottom)"));
            continue;
        };
        let got = std::fs::read(dir.join(&name)).map_err(|e| e.to_string()).and_then(|b| decode(&b));
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
    for (id, [all, top, side, bottom]) in found {
        set.blocks.push((id, Painted { faces: [top, side, bottom].map(|f| f.or_else(|| all.clone())) }));
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
            let v = decode(&rgba(w, 16, |x, y| [(x / 16) as u8, x as u8, y as u8, 255])).unwrap();
            assert_eq!(v.len(), n, "{w} px wide");
            for (i, p) in v.iter().enumerate() {
                assert_eq!(p[3 * 16 + 5], [i as u8, (i * 16 + 5) as u8, 3, 255], "variant {i}, pixel (5, 3)");
            }
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
            assert!(decode(&bytes).is_err(), "{why}");
        }
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
