//! The one fixed palette every texture and model is painted from (`data/palette.toml`, docs/design/studio.md,
//! ADR 0044): named sRGB colours in the file's order, so a PNG pixel matches one exactly or not at all.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    rgb: String,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Palette {
    pub colours: Vec<Colour>,
}

const HEADER: &str = "# The one palette every texture and model is painted from (docs/design/studio.md; schema in\n\
# crates/engine/src/palette.rs). `name`: unique, lower case. `rgb`: \"#rrggbb\", sRGB, the bytes a PNG holds.\n\
# The order here is the order on the Studio's Palette page. Written by the Studio: comments are not kept.\n\n";

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
            p.colours.push(Colour { name: r.name, rgb });
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
        let f = File { colour: self.colours.iter().map(|c| Row { name: c.name.clone(), rgb: to_hex(c.rgb) }).collect() };
        HEADER.to_string() + &toml::to_string(&f).expect("a palette always serialises")
    }

    /// The place of the colour with these bytes.
    pub fn find(&self, rgb: [u8; 3]) -> Option<usize> {
        self.colours.iter().position(|c| c.rgb == rgb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_palette_round_trips_in_its_order() {
        let text = "[[colour]]\nname = \"moss\"\nrgb = \"#4d9433\"\n\n[[colour]]\nname = \"bark dark\"\nrgb = \"#2A1b0f\"\n";
        let p = Palette::parse(text).unwrap();
        assert_eq!(p.colours, vec![Colour { name: "moss".into(), rgb: [0x4d, 0x94, 0x33] }, Colour { name: "bark dark".into(), rgb: [0x2a, 0x1b, 0x0f] }]);
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
    }
}
