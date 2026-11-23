//! The fluids (docs/design/liquids.md "Fluids are rows"): every fluid's row from `data/fluids.toml`, built into
//! the binary as the blocks are. A row's place is its kind (`liquid::Node::kind`), on the wire and in saves; row 0
//! is water, which every world before fluid rows holds.
//! shortcut: not reloaded live (ADR 0134 has blocks, items, creatures and herds): a kind is a row's place, and a
//! reload would have to reach every client's copy; add it to `live::apply` once a fluid's numbers need tuning in play.
use serde::Deserialize;

/// Kinds a node may hold: the wire packs one into a gap's top byte, kept positive (`liquid::pack`).
pub const KINDS: usize = 128;

/// A later step's change of phase: at `at` degrees C it turns into `into` (liquids.md step 5; unused yet).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub at: f32,
    pub into: String,
}

/// A later step's reaction: meeting `with` it gives `gives` and `heat` kJ a unit (liquids.md step 4; unused yet).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct React {
    pub with: String,
    pub gives: Vec<String>,
    #[serde(default)]
    pub heat: f32,
}

/// How a fluid holds and gives off heat (liquids.md "Phases"), SI units: what `liquid::Liquids::cool` steps it by.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Heat {
    /// The temperature it is poured at, C.
    pub pour: f32,
    /// Specific heat, J/(kg K).
    pub capacity: f32,
    /// Latent heat of its freeze, J/kg.
    pub latent: f32,
    /// Thermal conductivity, W/(m K).
    pub conduct: f32,
    /// Emissivity of its open surface, 0..1.
    pub emissivity: f32,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Fluid {
    pub name: String,
    /// kg/m3.
    pub density: f32,
    /// Times water's friction a link loses a step (`liquid.rs`): water 1.
    pub viscosity: u16,
    pub colour: [f32; 3],
    /// The light it gives, a colour times its strength (0..4, as a block's); one that gives light glows by it.
    #[serde(default)]
    pub light: [f32; 3],
    pub freeze: Option<Phase>,
    pub boil: Option<Phase>,
    #[serde(default)]
    pub reacts: Vec<React>,
    /// Its heat (none: it carries no temperature; water's waits for liquids.md step 5).
    pub heat: Option<Heat>,
}

impl Fluid {
    pub fn glows(&self) -> bool {
        self.light.iter().any(|&c| c > 0.0)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    fluid: Vec<Fluid>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fluids {
    pub rows: Vec<Fluid>,
}

impl Fluids {
    pub fn parse(text: &str) -> Result<Fluids, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        if f.fluid.first().is_none_or(|r| r.name != "water") {
            return Err("the first fluid must be water: every older world's stored water is kind 0".into());
        }
        if f.fluid.len() > KINDS {
            return Err(format!("{} fluids, over the {KINDS} a gap's kind holds", f.fluid.len()));
        }
        for r in &f.fluid {
            let bad = |what: &str| Err(format!("fluid '{}': {what}", r.name));
            if f.fluid.iter().filter(|o| o.name == r.name).count() > 1 {
                return bad("name used twice");
            }
            if !(r.density > 0.0) {
                return bad("density must be over 0");
            }
            if r.viscosity == 0 {
                return bad("viscosity must be 1 or more");
            }
            if r.colour.iter().any(|c| !(0.0..=1.0).contains(c)) {
                return bad("colour outside 0..1");
            }
            if r.light.iter().any(|c| !(0.0..=4.0).contains(c)) {
                return bad("light outside 0..4");
            }
            if let Some(h) = &r.heat {
                if !(h.capacity > 0.0 && h.latent > 0.0 && h.conduct > 0.0 && (0.0..=1.0).contains(&h.emissivity)) {
                    return bad("heat: capacity, latent and conduct over 0, emissivity 0..1");
                }
                if r.freeze.as_ref().is_some_and(|f| h.pour <= f.at) {
                    return bad("heat: poured at or under its freeze point");
                }
            }
        }
        if f.fluid[0].viscosity != 1 {
            return Err("fluid 'water': viscosity must be 1 (water's friction is the unit)".into());
        }
        Ok(Fluids { rows: f.fluid })
    }
    /// The kind of the fluid named `name`.
    pub fn kind(&self, name: &str) -> Option<u8> {
        self.rows.iter().position(|r| r.name == name).map(|k| k as u8)
    }
    pub fn get(&self, kind: u8) -> &Fluid {
        &self.rows[(kind as usize).min(self.rows.len() - 1)]
    }
    pub fn names(&self) -> String {
        self.rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
    }
}

pub const FLUIDS_TOML: &str = include_str!("../../../data/fluids.toml");
/// The fluid rows in force.
pub static FLUIDS: crate::live::Live<Fluids> = crate::live::Live::new(|| Fluids::parse(FLUIDS_TOML).expect("data/fluids.toml"));
/// Water's kind.
pub const WATER: u8 = 0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fluids_file_parses_with_water_first() {
        let f = Fluids::parse(FLUIDS_TOML).unwrap();
        assert_eq!((f.rows[0].name.as_str(), f.rows[0].viscosity), ("water", 1));
        let lava = f.get(f.kind("lava").unwrap());
        assert!(lava.viscosity > 1 && lava.glows() && lava.density > f.rows[0].density);
        assert!(!f.rows[0].glows());
    }

    #[test]
    fn a_bad_fluid_row_is_refused_by_name() {
        let ok = "[[fluid]]\nname = \"water\"\ndensity = 1000.0\nviscosity = 1\ncolour = [0.1, 0.3, 0.4]\n";
        let row = |extra: &str| format!("{ok}[[fluid]]\nname = \"tar\"\ndensity = 1200.0\ncolour = [0.1, 0.1, 0.1]\n{extra}\n");
        assert!(Fluids::parse(&row("viscosity = 8")).is_ok());
        let e = Fluids::parse(&row("viscosity = 8\nsmell = 3")).unwrap_err();
        assert!(e.contains("smell"), "an unknown field is named: {e}");
        let e = Fluids::parse(&row("viscosity = 0")).unwrap_err();
        assert!(e.contains("'tar'") && e.contains("viscosity"), "{e}");
        let e = Fluids::parse(&row("viscosity = 8\nlight = [9.0, 0.0, 0.0]")).unwrap_err();
        assert!(e.contains("'tar'") && e.contains("light"), "{e}");
        let e = Fluids::parse(&row("viscosity = 8\ndensity = 3.0")).unwrap_err();
        assert!(e.contains("density"), "a field twice is refused: {e}");
        let e = Fluids::parse(&format!("{}{}", &row("viscosity = 8")[ok.len()..], ok)).unwrap_err();
        assert!(e.contains("first fluid must be water"), "{e}");
    }
}
