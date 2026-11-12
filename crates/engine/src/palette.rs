//! The one fixed palette every texture and model is painted from (`data/palette.toml`, docs/design/studio.md,
//! ADR 0044): named sRGB colours in the file's order, so a PNG pixel matches one exactly or not at all.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    rgb: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ramp: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    colour: Vec<Row>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Colour {
    pub name: String,
    pub rgb: [u8; 3],
    /// The ramp it belongs to (docs/design/studio-tools.md "Palette: ramps"): a run of colours from dark to
    /// light in the file's order. None: it stands alone.
    pub ramp: Option<String>,
}

impl Colour {
    pub fn new(name: impl Into<String>, rgb: [u8; 3]) -> Colour {
        Colour { name: name.into(), rgb, ramp: None }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Palette {
    pub colours: Vec<Colour>,
}

const HEADER: &str = "# The one palette every texture and model is painted from (docs/design/studio.md; schema in\n\
# crates/engine/src/palette.rs). `name`: unique, lower case. `rgb`: \"#rrggbb\", sRGB, the bytes a PNG holds.\n\
# The order here is the order on the Studio's Palette page. `ramp` (optional): the colours with one ramp name\n\
# are a run from dark to light, in this order. Written by the Studio: comments are not kept.\n\n";

/// "#rrggbb" to its bytes.
pub fn hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6 && h.is_ascii())?;
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

pub fn to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

impl Palette {
    /// Reads and checks a palette: every `rgb` is "#rrggbb", names are non-empty, lower case and unique, and no
    /// colour is listed twice (a pixel names one colour).
    pub fn parse(text: &str) -> Result<Palette, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut p = Palette::default();
        for r in f.colour {
            let rgb = hex(&r.rgb).ok_or(format!("colour '{}': rgb '{}' is not #rrggbb", r.name, r.rgb))?;
            p.check(&r.name, rgb, None)?;
            if let Some(ramp) = r.ramp.as_ref().filter(|n| n.is_empty() || **n != n.to_lowercase()) {
                return Err(format!("colour '{}': ramp name '{ramp}' must be lower case and not empty", r.name));
            }
            p.colours.push(Colour { name: r.name, rgb, ramp: r.ramp });
        }
        Ok(p)
    }

    /// Whether `name` and `rgb` may stand in the palette beside every colour but the one at `skip`.
    pub fn check(&self, name: &str, rgb: [u8; 3], skip: Option<usize>) -> Result<(), String> {
        if name.is_empty() || name != name.to_lowercase() {
            return Err(format!("colour name '{name}' must be lower case and not empty"));
        }
        for (i, c) in self.colours.iter().enumerate().filter(|(i, _)| Some(*i) != skip) {
            if c.name == name {
                return Err(format!("colour name '{name}' is used twice"));
            }
            if c.rgb == rgb {
                return Err(format!("colour '{name}': {} is already '{}' (row {})", to_hex(rgb), c.name, i + 1));
            }
        }
        Ok(())
    }

    pub fn write(&self) -> String {
        let f = File { colour: self.colours.iter().map(|c| Row { name: c.name.clone(), rgb: to_hex(c.rgb), ramp: c.ramp.clone() }).collect() };
        HEADER.to_string() + &toml::to_string(&f).expect("a palette always serialises")
    }

    /// The place of the colour with these bytes.
    pub fn find(&self, rgb: [u8; 3]) -> Option<usize> {
        self.colours.iter().position(|c| c.rgb == rgb)
    }

    /// Every ramp, in the order its first colour comes, with its colours' places from dark to light.
    pub fn ramps(&self) -> Vec<(String, Vec<usize>)> {
        let mut out: Vec<(String, Vec<usize>)> = vec![];
        for (i, c) in self.colours.iter().enumerate() {
            let Some(r) = &c.ramp else { continue };
            match out.iter_mut().find(|(n, _)| n == r) {
                Some((_, v)) => v.push(i),
                None => out.push((r.clone(), vec![i])),
            }
        }
        out
    }

    /// The places of one ramp's colours, dark to light.
    pub fn ramp(&self, name: &str) -> Vec<usize> {
        self.colours.iter().enumerate().filter(|(_, c)| c.ramp.as_deref() == Some(name)).map(|(i, _)| i).collect()
    }

    /// The colour `steps` along the ramp of `rgb` (negative: darker), stopping at its ends; a colour on no ramp
    /// stays itself. None: `rgb` is not in the palette.
    pub fn step(&self, rgb: [u8; 3], steps: i32) -> Option<[u8; 3]> {
        let i = self.find(rgb)?;
        let Some(r) = &self.colours[i].ramp else { return Some(rgb) };
        let run = self.ramp(r);
        let at = run.iter().position(|&k| k == i)? as i32;
        Some(self.colours[run[(at + steps).clamp(0, run.len() as i32 - 1) as usize]].rgb)
    }

    /// The run of one ramp from colour `a` to colour `b` (either way round, both ends included). None: they
    /// are not on one ramp.
    pub fn between(&self, a: [u8; 3], b: [u8; 3]) -> Option<Vec<[u8; 3]>> {
        let (i, j) = (self.find(a)?, self.find(b)?);
        let r = self.colours[i].ramp.as_ref().filter(|r| self.colours[j].ramp.as_ref() == Some(*r))?;
        let run = self.ramp(r);
        let (p, q) = (run.iter().position(|&k| k == i)?, run.iter().position(|&k| k == j)?);
        let mut v: Vec<[u8; 3]> = run[p.min(q)..=p.max(q)].iter().map(|&k| self.colours[k].rgb).collect();
        if p > q {
            v.reverse();
        }
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palette_round_trips_in_its_order() {
        let text = "[[colour]]\nname = \"moss\"\nrgb = \"#4d9433\"\n\n[[colour]]\nname = \"bark dark\"\nrgb = \"#2A1b0f\"\n";
        let p = Palette::parse(text).unwrap();
        assert_eq!(p.colours, vec![Colour::new("moss", [0x4d, 0x94, 0x33]), Colour::new("bark dark", [0x2a, 0x1b, 0x0f])]);
        assert_eq!(Palette::parse(&p.write()).unwrap(), p);
        assert!(p.write().contains("rgb = \"#2a1b0f\""), "written lower case: {}", p.write());
        assert_eq!(p.find([0x2a, 0x1b, 0x0f]), Some(1));
        assert_eq!(Palette::parse("").unwrap().colours.len(), 0);
    }

    #[test]
    fn bad_palettes_are_refused() {
        let moss = "[[colour]]\nname = \"moss\"\nrgb = \"#4d9433\"\n";
        for (bad, why) in [
            (format!("{moss}[[colour]]\nname = \"moss\"\nrgb = \"#000000\"\n"), "name twice"),
            (format!("{moss}[[colour]]\nname = \"leaf\"\nrgb = \"#4d9433\"\n"), "colour twice"),
            ("[[colour]]\nname = \"Moss\"\nrgb = \"#4d9433\"\n".into(), "upper case"),
            ("[[colour]]\nname = \"\"\nrgb = \"#4d9433\"\n".into(), "empty name"),
            ("[[colour]]\nname = \"moss\"\nrgb = \"4d9433\"\n".into(), "no #"),
            ("[[colour]]\nname = \"moss\"\nrgb = \"#4d943\"\n".into(), "short"),
            ("[[colour]]\nname = \"moss\"\nrgb = \"#4d94zz\"\n".into(), "not hex"),
            ("[[colour]]\nname = \"moss\"\nrgb = \"#4d9433\"\nalpha = 1\n".into(), "unknown field"),
            ("[[color]]\nname = \"moss\"\nrgb = \"#4d9433\"\n".into(), "unknown table"),
        ] {
            assert!(Palette::parse(&bad).is_err(), "{why}");
        }
    }

    #[test]
    fn the_data_file_loads() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        assert!(!p.colours.is_empty());
        assert!(p.ramps().iter().any(|(n, v)| n == "grass" && v.len() == 3), "the draft colours are grouped by block: {:?}", p.ramps());
    }

    const RAMPS: &str = "[[colour]]\nname = \"moss dark\"\nrgb = \"#203010\"\nramp = \"moss\"\n\n\
        [[colour]]\nname = \"lone\"\nrgb = \"#ff0000\"\n\n\
        [[colour]]\nname = \"moss\"\nrgb = \"#406020\"\nramp = \"moss\"\n\n\
        [[colour]]\nname = \"sand\"\nrgb = \"#c0b080\"\nramp = \"sand\"\n\n\
        [[colour]]\nname = \"moss light\"\nrgb = \"#80c040\"\nramp = \"moss\"\n";

    #[test]
    fn ramps_parse_in_the_files_order_and_round_trip() {
        let p = Palette::parse(RAMPS).unwrap();
        assert_eq!(p.ramps(), vec![("moss".to_string(), vec![0, 2, 4]), ("sand".to_string(), vec![3])]);
        assert_eq!(p.colours[1].ramp, None, "a colour with no ramp stands alone");
        assert_eq!(Palette::parse(&p.write()).unwrap(), p);
        assert!(!p.write().contains("ramp = \"\""), "no empty ramp written for a lone colour");
        assert!(Palette::parse("[[colour]]\nname = \"a\"\nrgb = \"#000000\"\nramp = \"Moss\"\n").is_err(), "ramp names are lower case");
        assert!(Palette::parse("[[colour]]\nname = \"a\"\nrgb = \"#000000\"\nramp = \"\"\n").is_err(), "and not empty");
    }

    #[test]
    fn a_step_walks_the_ramp_and_stops_at_its_ends() {
        let p = Palette::parse(RAMPS).unwrap();
        let (dark, mid, light) = ([0x20, 0x30, 0x10], [0x40, 0x60, 0x20], [0x80, 0xc0, 0x40]);
        assert_eq!(p.step(mid, -1), Some(dark));
        assert_eq!(p.step(mid, 1), Some(light));
        assert_eq!(p.step(dark, -1), Some(dark), "the dark end stays");
        assert_eq!(p.step(dark, 5), Some(light), "the light end stops it");
        assert_eq!(p.step([0xff, 0, 0], 1), Some([0xff, 0, 0]), "a lone colour stays");
        assert_eq!(p.step([1, 2, 3], 1), None);
        assert_eq!(p.between(light, dark), Some(vec![light, mid, dark]));
        assert_eq!(p.between(dark, mid), Some(vec![dark, mid]));
        assert_eq!(p.between(dark, [0xc0, 0xb0, 0x80]), None, "two ramps");
    }
}
