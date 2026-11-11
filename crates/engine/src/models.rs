//! Voxel models (docs/design/studio-tools.md "Models", ADR 0047): `data/models/<name>.toml` read and written,
//! each part's transform at a time in a clip, and each part meshed. No GPU, so the Studio calls the same code.
use crate::palette::Palette;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// One voxel's edge in metres: the fine cell's size (ADR 0033), 8 to a block's edge.
pub const VOXEL: f32 = 0.125;
/// A part's box is 1..MAX_BOX voxels a side.
pub const MAX_BOX: u32 = 32;
/// The letters of the voxel rows, in the order of `colours`; '.' is empty.
pub const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Palette names; a voxel's letter picks one.
    pub colours: Vec<String>,
    #[serde(default, rename = "part", skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    #[serde(default, rename = "point", skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
    #[serde(default, rename = "clip", skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<Clip>,
    /// `colours`' bytes from the palette (filled by `parse`).
    #[serde(skip)]
    pub rgb: Vec<[u8; 3]>,
    /// Each part's parent's place in `parts` (filled by `parse`).
    #[serde(skip)]
    pub up: Vec<Option<usize>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub name: String,
    /// "" for a root, else another part's name.
    #[serde(default)]
    pub parent: String,
    /// Where the box's corner sits in the parent's voxels (a root: in the cell).
    #[serde(default)]
    pub at: [i32; 3],
    /// The point it turns about, in its own voxels.
    #[serde(default)]
    pub pivot: [f32; 3],
    /// x, y (up), z in voxels.
    #[serde(rename = "box")]
    pub size: [u32; 3],
    pub voxels: String,
    /// 0 empty, else a colour's place + 1, at x + size.x * (z + size.z * y) (filled by `parse`).
    #[serde(skip)]
    pub cells: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub name: String,
    pub part: String,
    pub at: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub name: String,
    /// Seconds.
    pub length: f32,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    #[serde(default)]
    pub key: Vec<Key>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub part: String,
    pub t: f32,
    /// In voxels.
    #[serde(default, rename = "move")]
    pub shift: [f32; 3],
    /// Degrees about x, y, z at the pivot (x first).
    #[serde(default)]
    pub turn: [f32; 3],
    /// How it leaves this key.
    #[serde(default)]
    pub ease: Ease,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    #[default]
    Linear,
    Smooth,
    Step,
}

/// One merged face of a part: its min corner in the part's voxels, its direction (0..6 = +x,-x,+y,-y,+z,-z),
/// its extent along the axes (a+1)%3 and (a+2)%3 of its axis a, and its colour's place in `colours`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quad {
    pub at: [u8; 3],
    pub dir: u8,
    pub w: u8,
    pub h: u8,
    pub colour: u8,
}

/// The voxel rows of a part: layers from the bottom up, a blank line between, one row per z, one letter per x.
pub fn decode(size: [u32; 3], text: &str, colours: usize) -> Result<Vec<u8>, String> {
    let [sx, sy, sz] = size.map(|v| v as usize);
    let mut layers: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines().map(str::trim_end) {
        if line.is_empty() {
            if !layers.last().unwrap().is_empty() {
                layers.push(Vec::new());
            }
        } else {
            layers.last_mut().unwrap().push(line);
        }
    }
    if layers.last().is_some_and(|l| l.is_empty()) {
        layers.pop();
    }
    if layers.len() != sy {
        return Err(format!("{} layers, the box is {sy} high", layers.len()));
    }
    let mut cells = vec![0u8; sx * sy * sz];
    for (y, rows) in layers.iter().enumerate() {
        if rows.len() != sz {
            return Err(format!("layer {} has {} rows, the box is {sz} deep", y + 1, rows.len()));
        }
        for (z, row) in rows.iter().enumerate() {
            if row.len() != sx {
                return Err(format!("layer {} row {} is {} long, the box is {sx} wide", y + 1, z + 1, row.len()));
            }
            for (x, ch) in row.bytes().enumerate() {
                if ch == b'.' {
                    continue;
                }
                let k = LETTERS.iter().position(|l| *l == ch).ok_or(format!("layer {} row {}: '{}' is no voxel letter", y + 1, z + 1, ch as char))?;
                if k >= colours {
                    return Err(format!("layer {} row {}: '{}' names no colour ({colours} listed)", y + 1, z + 1, ch as char));
                }
                cells[x + sx * (z + sz * y)] = k as u8 + 1;
            }
        }
    }
    Ok(cells)
}

/// `decode`'s inverse.
pub fn encode(size: [u32; 3], cells: &[u8]) -> String {
    let [sx, sy, sz] = size.map(|v| v as usize);
    let layers: Vec<String> = (0..sy)
        .map(|y| {
            (0..sz)
                .map(|z| (0..sx).map(|x| match cells[x + sx * (z + sz * y)] { 0 => '.', c => LETTERS[c as usize - 1] as char }).collect::<String>() + "\n")
                .collect()
        })
        .collect();
    layers.join("\n")
}

impl Model {
    /// Reads and checks a model; every refusal names `file` and why.
    pub fn parse(file: &str, text: &str, palette: &Palette) -> Result<Model, String> {
        let mut m: Model = toml::from_str(text).map_err(|e| format!("{file}: {e}"))?;
        m.check(palette).map_err(|e| format!("{file}: {e}"))?;
        Ok(m)
    }

    fn check(&mut self, palette: &Palette) -> Result<(), String> {
        if self.colours.len() > LETTERS.len() {
            return Err(format!("{} colours, at most {}", self.colours.len(), LETTERS.len()));
        }
        self.rgb = self
            .colours
            .iter()
            .map(|n| palette.colours.iter().find(|c| &c.name == n).map(|c| c.rgb).ok_or(format!("colour '{n}' is not in the palette")))
            .collect::<Result<_, _>>()?;
        let find = |n: &str| self.parts.iter().position(|p| p.name == n);
        let mut up = Vec::new();
        for (i, p) in self.parts.iter().enumerate() {
            if find(&p.name) != Some(i) {
                return Err(format!("part '{}' is named twice", p.name));
            }
            if p.size.iter().any(|v| !(1..=MAX_BOX).contains(v)) {
                return Err(format!("part '{}': box {:?} outside 1..{MAX_BOX}", p.name, p.size));
            }
            up.push(match p.parent.as_str() {
                "" => None,
                n => Some(find(n).ok_or(format!("part '{}': no part '{n}' for its parent", p.name))?),
            });
        }
        for (i, p) in self.parts.iter().enumerate() {
            let (mut at, mut steps) = (up[i], 0);
            while let Some(k) = at {
                steps += 1;
                if steps > self.parts.len() {
                    return Err(format!("part '{}': its parents run in a loop", p.name));
                }
                at = up[k];
            }
        }
        self.up = up;
        let n = self.colours.len();
        for p in &mut self.parts {
            p.cells = decode(p.size, &p.voxels, n).map_err(|e| format!("part '{}': {e}", p.name))?;
        }
        let find = |n: &str| self.parts.iter().position(|p| p.name == n);
        for pt in &self.points {
            find(&pt.part).ok_or(format!("point '{}': no part '{}'", pt.name, pt.part))?;
        }
        for c in &self.clips {
            if !(c.length > 0.0) {
                return Err(format!("clip '{}': length must be over 0", c.name));
            }
            for k in &c.key {
                find(&k.part).ok_or(format!("clip '{}': a key names no part '{}'", c.name, k.part))?;
                if !(0.0..=c.length).contains(&k.t) {
                    return Err(format!("clip '{}': key of '{}' at t = {} is outside 0..{}", c.name, k.part, k.t, c.length));
                }
            }
        }
        Ok(())
    }

    /// The file's text (the voxels as their rows; parse it again to check it).
    pub fn write(&self) -> String {
        toml::to_string(self).expect("a model always serialises")
    }

    /// Each part's transform from its voxels to the cell's metres (x, y up, z; the cell's corner at 0) at `t`
    /// seconds into `clip` (a looping clip wraps, another holds its end); no clip, or one it lacks, rests.
    pub fn pose(&self, clip: Option<&str>, t: f32) -> Vec<Mat4> {
        let clip = clip.and_then(|n| self.clips.iter().find(|c| c.name == n));
        let t = clip.map_or(0.0, |c| if c.looping { t.rem_euclid(c.length) } else { t.clamp(0.0, c.length) });
        let mut out: Vec<Option<Mat4>> = vec![None; self.parts.len()];
        for i in 0..self.parts.len() {
            self.place(i, clip, t, &mut out);
        }
        let s = Mat4::from_scale(Vec3::splat(VOXEL));
        out.into_iter().map(|m| s * m.unwrap()).collect()
    }

    /// Part i in the model's voxels, its parents first.
    fn place(&self, i: usize, clip: Option<&Clip>, t: f32, out: &mut Vec<Option<Mat4>>) -> Mat4 {
        if let Some(m) = out[i] {
            return m;
        }
        let p = &self.parts[i];
        let (shift, turn) = clip.map_or((Vec3::ZERO, Vec3::ZERO), |c| sample(c, &p.name, t));
        let pivot = Vec3::from(p.pivot);
        let turn = turn * (std::f32::consts::PI / 180.0);
        let local = Mat4::from_translation(Vec3::from(p.at.map(|v| v as f32)) + pivot + shift)
            * Mat4::from_rotation_z(turn.z)
            * Mat4::from_rotation_y(turn.y)
            * Mat4::from_rotation_x(turn.x)
            * Mat4::from_translation(-pivot);
        let m = match self.up[i] {
            Some(k) => self.place(k, clip, t, out) * local,
            None => local,
        };
        out[i] = Some(m);
        m
    }
}

/// A part's move and turn at t in a clip: held before its first key and after its last, eased between.
fn sample(c: &Clip, part: &str, t: f32) -> (Vec3, Vec3) {
    let mut keys: Vec<&Key> = c.key.iter().filter(|k| k.part == part).collect();
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    let val = |k: &Key| (Vec3::from(k.shift), Vec3::from(k.turn));
    let Some(first) = keys.first() else { return (Vec3::ZERO, Vec3::ZERO) };
    let Some(n) = keys.iter().position(|k| k.t > t) else { return val(keys.last().unwrap()) };
    if n == 0 {
        return val(first);
    }
    let (a, b) = (keys[n - 1], keys[n]);
    let f = (t - a.t) / (b.t - a.t);
    let f = match a.ease {
        Ease::Linear => f,
        Ease::Smooth => f * f * (3.0 - 2.0 * f),
        Ease::Step => 0.0,
    };
    let ((s0, r0), (s1, r1)) = (val(a), val(b));
    (s0.lerp(s1, f), r0.lerp(r1, f))
}

/// A part's faces: hidden ones (against a voxel of the part) dropped, flat runs of one colour merged.
pub fn mesh(part: &Part) -> Vec<Quad> {
    let s = part.size.map(|v| v as i32);
    let get = |p: [i32; 3]| -> u8 {
        if (0..3).all(|a| (0..s[a]).contains(&p[a])) {
            part.cells[(p[0] + s[0] * (p[2] + s[2] * p[1])) as usize]
        } else {
            0
        }
    };
    let mut out = Vec::new();
    for a in 0..3 {
        let (ua, va) = ((a + 1) % 3, (a + 2) % 3);
        for sgn in 0..2 {
            let step = if sgn == 0 { 1 } else { -1 };
            for d in 0..s[a] {
                // this slice's visible faces: their colour + 1 at [v][u]
                let (nu, nv) = (s[ua] as usize, s[va] as usize);
                let mut m = vec![0u8; nu * nv];
                for v in 0..nv {
                    for u in 0..nu {
                        let mut p = [0; 3];
                        p[a] = d;
                        p[ua] = u as i32;
                        p[va] = v as i32;
                        let c = get(p);
                        p[a] += step;
                        if c != 0 && get(p) == 0 {
                            m[v * nu + u] = c;
                        }
                    }
                }
                for v in 0..nv {
                    let mut u = 0;
                    while u < nu {
                        let c = m[v * nu + u];
                        if c == 0 {
                            u += 1;
                            continue;
                        }
                        let mut w = 1;
                        while u + w < nu && m[v * nu + u + w] == c {
                            w += 1;
                        }
                        let mut h = 1;
                        while v + h < nv && (u..u + w).all(|x| m[(v + h) * nu + x] == c) {
                            h += 1;
                        }
                        for y in v..v + h {
                            m[y * nu + u..y * nu + u + w].fill(0);
                        }
                        let mut at = [0u8; 3];
                        at[a] = d as u8;
                        at[ua] = u as u8;
                        at[va] = v as u8;
                        out.push(Quad { at, dir: (a * 2 + sgn) as u8, w: w as u8, h: h as u8, colour: c - 1 });
                        u += w;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::parse("[[colour]]\nname = \"stone\"\nrgb = \"#808080\"\n\n[[colour]]\nname = \"copper\"\nrgb = \"#b87333\"\n").unwrap()
    }

    const BOX: &str = r#"colours = ["stone", "copper"]

[[part]]
name = "body"
box = [2, 2, 1]
voxels = """
ab

aa
"""

[[part]]
name = "door"
parent = "body"
at = [2, 0, 0]
pivot = [0, 0, 0]
box = [1, 1, 1]
voxels = """
b
"""

[[point]]
name = "flame"
part = "body"
at = [1, 1, 0]

[[clip]]
name = "work"
length = 1.0
loop = true
key = [
  { part = "door", t = 0.0, turn = [0, 0, 0], ease = "linear" },
  { part = "door", t = 0.5, turn = [0, 90, 0], ease = "step" },
  { part = "door", t = 0.75, move = [4, 0, 0] },
]
"#;

    #[test]
    fn a_model_reads_and_round_trips() {
        let m = Model::parse("box.toml", BOX, &palette()).unwrap();
        assert_eq!(m.rgb, vec![[0x80; 3], [0xb8, 0x73, 0x33]]);
        assert_eq!(m.up, vec![None, Some(0)]);
        // layer 0 (bottom) is "ab": x = 1, y = 0 is copper; layer 1 "aa"
        assert_eq!(m.parts[0].cells, vec![1, 2, 1, 1]);
        let again = Model::parse("again", &m.write(), &palette()).unwrap();
        assert_eq!(again, m);
        assert_eq!(decode([2, 2, 1], &encode([2, 2, 1], &m.parts[0].cells), 2).unwrap(), m.parts[0].cells);
    }

    #[test]
    fn bad_models_are_refused_by_file_and_reason() {
        let p = palette();
        let many: Vec<String> = (0..63).map(|_| "\"stone\"".to_string()).collect();
        for (bad, why) in [
            (BOX.replace("\"copper\"]", "\"gold\"]"), "not in the palette"),
            (format!("colours = [{}]\n", many.join(", ")), "63 colours"),
            (BOX.replace("box = [2, 2, 1]", "box = [33, 2, 1]"), "outside 1..32"),
            (BOX.replace("box = [2, 2, 1]", "box = [0, 2, 1]"), "outside 1..32"),
            (BOX.replace("ab\n\naa", "ab\n\naaa"), "row 1 is 3 long"),
            (BOX.replace("ab\n\naa", "ab"), "1 layers"),
            (BOX.replace("ab\n\naa", "ac\n\naa"), "'c' names no colour"),
            (BOX.replace("parent = \"body\"", "parent = \"lid\""), "no part 'lid'"),
            (BOX.replace("parent = \"body\"", "parent = \"door\""), "loop"),
            (BOX.replace("{ part = \"door\", t = 0.75", "{ part = \"knob\", t = 0.75"), "names no part 'knob'"),
            (BOX.replace("t = 0.75", "t = 1.5"), "outside 0..1"),
            (BOX.replace("name = \"flame\"", "name = \"flame\"\nsize = 2"), "unknown field"),
        ] {
            let e = Model::parse("data/models/box.toml", &bad, &p).expect_err(why);
            assert!(e.starts_with("data/models/box.toml: ") && e.contains(why), "{why}: {e}");
        }
    }

    #[test]
    fn a_clip_eases_turns_about_the_pivot_and_children_follow() {
        let mut m = Model::parse("box.toml", BOX, &palette()).unwrap();
        let at = |m: &Model, clip, t, part: usize, p: Vec3| m.pose(clip, t)[part].transform_point3(p) / VOXEL;
        let near = |a: Vec3, b: Vec3| assert!(a.distance(b) < 1e-4, "{a} vs {b}");
        // at rest the door sits at its `at`
        near(at(&m, None, 0.0, 1, Vec3::ZERO), Vec3::new(2.0, 0.0, 0.0));
        // linear half way to 90 degrees about y at the pivot: the far corner x = 1 swings towards -z
        let h = 45f32.to_radians();
        near(at(&m, Some("work"), 0.25, 1, Vec3::X), Vec3::new(2.0 + h.cos(), 0.0, -h.sin()));
        // step holds 90 until the next key, then the move; the turn left zero there (no turn given)
        near(at(&m, Some("work"), 0.6, 1, Vec3::X), Vec3::new(2.0, 0.0, -1.0));
        near(at(&m, Some("work"), 0.75, 1, Vec3::ZERO), Vec3::new(6.0, 0.0, 0.0));
        // looping wraps: 1.25 is 0.25
        near(at(&m, Some("work"), 1.25, 1, Vec3::X), at(&m, Some("work"), 0.25, 1, Vec3::X));
        // smooth: f = 0.5 eases to 0.5, f = 0.25 to 0.15625
        m.clips[0].key[0].ease = Ease::Smooth;
        let a = (90.0f32 * 0.15625).to_radians();
        near(at(&m, Some("work"), 0.125, 1, Vec3::X), Vec3::new(2.0 + a.cos(), 0.0, -a.sin()));
        // a child follows its parent: the body moved, the door moves with it
        m.clips[0].key.push(Key { part: "body".into(), t: 0.0, shift: [0.0, 3.0, 0.0], turn: [0.0; 3], ease: Ease::Linear });
        near(at(&m, Some("work"), 0.0, 1, Vec3::ZERO), Vec3::new(2.0, 3.0, 0.0));
    }

    #[test]
    fn the_mesher_drops_hidden_faces_and_merges_flat_runs() {
        let part = |size: [u32; 3], cells: Vec<u8>| Part { name: "p".into(), parent: String::new(), at: [0; 3], pivot: [0.0; 3], size, voxels: String::new(), cells };
        assert_eq!(mesh(&part([1, 1, 1], vec![1])).len(), 6);
        // a bar of one colour: six faces; of two colours: the four long sides split in two
        assert_eq!(mesh(&part([2, 1, 1], vec![1, 1])).len(), 6);
        assert_eq!(mesh(&part([2, 1, 1], vec![1, 2])).len(), 10);
        // a full 4 x 4 x 4 cube: six faces, each 4 x 4, nothing inside
        let q = mesh(&part([4, 4, 4], vec![1; 64]));
        assert_eq!(q.len(), 6);
        assert!(q.iter().all(|q| q.w == 4 && q.h == 4));
        let top = q.iter().find(|q| q.dir == 2).unwrap();
        assert_eq!(top.at, [0, 3, 0], "a + face's corner is in its voxel; the face is on its far side");
    }

    #[test]
    fn the_draft_furnace_loads() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let m = Model::parse("data/models/furnace.toml", include_str!("../../../data/models/furnace.toml"), &p).unwrap();
        assert!(m.clips.iter().any(|c| c.name == "work") && m.points.iter().any(|p| p.name == "flame"));
    }
}
