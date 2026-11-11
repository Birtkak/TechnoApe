//! The block registry: every block's id, name and look, from `data/blocks.toml` (ADR 0013). The file is built
//! into the binary, so a client and a server from one build agree; `Welcome` carries `BLOCKS.hash` so builds
//! that do not agree refuse each other.
use serde::Deserialize;
use std::sync::LazyLock;

pub const AIR: u8 = 0;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
/// The numbers are the shader's (`shader.wgsl`, `Look.info`).
pub enum Pattern {
    #[default]
    Plain = 0,
    Grass = 1,
    Stone = 2,
    Pebbles = 3,
    Speckle = 4,
    Frame = 5,
    Weave = 6,
    Planks = 7,
    Tiles = 8,
    Bricks = 9,
    Rings = 10,
}

impl Pattern {
    /// A pattern with parts' default `size` and `amount` (tools/review/workbench.html `PATTERNS[..].def`);
    /// plain, grass and stone take neither.
    pub fn defaults(self) -> Option<(u8, f32)> {
        use Pattern::*;
        match self {
            Plain | Grass | Stone => None,
            Pebbles => Some((1, 0.5)),
            Speckle => Some((5, 0.6)),
            Frame => Some((2, 0.8)),
            Weave => Some((4, 0.35)),
            Planks => Some((4, 0.5)),
            Tiles => Some((8, 0.3)),
            Bricks => Some((4, 0.5)),
            Rings => Some((2, 0.5)),
        }
    }
}

/// The page's `darker` and `lighter`, rounded to two places as it writes them.
fn two_places(v: f32) -> f32 {
    ((v as f64 * 100.0).round() / 100.0) as f32
}
fn darker(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| two_places(v * 0.55))
}
fn lighter(c: [f32; 3]) -> [f32; 3] {
    c.map(|v| two_places((v * 1.25 + 0.2).min(1.0)))
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Raw {
    id: u8,
    name: String,
    #[serde(default = "yes")]
    solid: bool,
    #[serde(default = "grey")]
    colour: [f32; 3],
    #[serde(default)]
    pattern: Pattern,
    colour2: Option<[f32; 3]>,
    colour3: Option<[f32; 3]>,
    size: Option<u8>,
    amount: Option<f32>,
    #[serde(default = "one")]
    contrast: f32,
    side: Option<String>,
    #[serde(default)]
    light: [f32; 3],
    #[serde(default)]
    metal: f32,
    #[serde(default)]
    was: Vec<String>,
    #[serde(default = "stone")]
    sound: String,
    #[serde(default)]
    hardness: f32,
    #[serde(default)]
    loose: bool,
}
fn stone() -> String {
    "stone".into()
}
fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}
fn grey() -> [f32; 3] {
    [0.5, 0.5, 0.5]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    block: Vec<Raw>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Def {
    pub id: u8,
    pub name: String,
    pub solid: bool,
    pub colour: [f32; 3],
    pub pattern: Pattern,
    /// A pattern's second and third parts (ADR 0039); default a darker and a lighter `colour`.
    pub colour2: [f32; 3],
    pub colour3: [f32; 3],
    /// A pattern's scale in texels, 1..8, and how much of its parts it shows, 0..1; 0 for plain, grass, stone.
    pub size: u8,
    pub amount: f32,
    /// How strong the three tones of grain are, 0..2; 1 is the look before ADR 0039.
    pub contrast: f32,
    /// The block whose look its four sides and foot take: a grass block's under its fringe, any other's
    /// wholly. Its own id when it names none.
    pub side: u8,
    /// The light it gives (ADR 0022), as a colour times its strength; zero for most blocks.
    pub light: [f32; 3],
    /// How much it reflects like a metal, 0..1.
    pub metal: f32,
    /// Names it had before: a save that knew it by one of them still loads.
    pub was: Vec<String>,
    /// Its material's sound (docs/design/sound.md): digging it plays `dig.<sound>`, walking on it
    /// `step.<sound>`. The client checks that those rows exist.
    pub sound: String,
    /// Seconds a survival player digs it with bare hands (ADR 0034); every solid block has one.
    pub hardness: f32,
    /// Bare hands take it (dirt, sand, a lamp): dug in survival it goes into the inventory. A block that is
    /// not loose breaks by hand in its time but gives nothing (the tools that take it come later).
    pub loose: bool,
}

pub struct Registry {
    defs: Vec<Option<Def>>,
    pub hash: u64,
}

impl Registry {
    /// Reads and checks a registry: 0 is a non-solid "air", ids and names are unique, every `side` exists,
    /// colours lie in 0..1, `size`, `amount` and `contrast` in their ranges. `hash` covers the whole text,
    /// so a client and a server whose looks differ in any field refuse each other.
    pub fn parse(text: &str) -> Result<Registry, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut defs: Vec<Option<Def>> = vec![None; 256];
        for r in &f.block {
            if defs[r.id as usize].is_some() {
                return Err(format!("block id {} is used twice", r.id));
            }
            if f.block.iter().filter(|o| o.name == r.name).count() > 1 {
                return Err(format!("block name '{}' is used twice", r.name));
            }
            if r.colour.iter().any(|c| !(0.0..=1.0).contains(c)) {
                return Err(format!("block '{}': colour outside 0..1", r.name));
            }
            if r.light.iter().any(|c| !(0.0..=4.0).contains(c)) {
                return Err(format!("block '{}': light outside 0..4", r.name));
            }
            if !(0.0..=1.0).contains(&r.metal) {
                return Err(format!("block '{}': metal outside 0..1", r.name));
            }
            for (c, what) in [(r.colour2, "colour2"), (r.colour3, "colour3")] {
                if c.is_some_and(|c| c.iter().any(|c| !(0.0..=1.0).contains(c))) {
                    return Err(format!("block '{}': {what} outside 0..1", r.name));
                }
            }
            if !(0.0..=2.0).contains(&r.contrast) {
                return Err(format!("block '{}': contrast outside 0..2", r.name));
            }
            let (size, amount) = match r.pattern.defaults() {
                None if r.size.is_some() || r.amount.is_some() => {
                    return Err(format!("block '{}': pattern {:?} takes no size or amount", r.name, r.pattern));
                }
                None => (0, 0.0),
                Some((s, a)) => (r.size.unwrap_or(s), r.amount.unwrap_or(a)),
            };
            if r.pattern.defaults().is_some() && !(1..=8).contains(&size) {
                return Err(format!("block '{}': size outside 1..8 texels", r.name));
            }
            if !(0.0..=1.0).contains(&amount) {
                return Err(format!("block '{}': amount outside 0..1", r.name));
            }
            if r.solid && !(r.hardness > 0.0 && r.hardness <= 60.0) {
                return Err(format!("block '{}': a solid block needs a hardness in 0..60 s", r.name));
            }
            let side = match &r.side {
                None => r.id,
                Some(s) => f.block.iter().find(|o| &o.name == s).ok_or(format!("block '{}': no block '{s}' for its side", r.name))?.id,
            };
            defs[r.id as usize] = Some(Def { id: r.id, name: r.name.clone(), solid: r.solid, colour: r.colour, pattern: r.pattern, colour2: r.colour2.unwrap_or(darker(r.colour)), colour3: r.colour3.unwrap_or(lighter(r.colour)), size, amount, contrast: r.contrast, side, light: r.light, metal: r.metal, was: r.was.clone(), sound: r.sound.clone(), hardness: r.hardness, loose: r.loose });
        }
        match &defs[0] {
            Some(d) if d.name == "air" && !d.solid => {}
            _ => return Err("block 0 must be a non-solid 'air'".into()),
        }
        Ok(Registry { defs, hash: crate::world::fnv(crate::world::FNV0, text.as_bytes()) })
    }

    pub fn get(&self, id: u8) -> Option<&Def> {
        self.defs[id as usize].as_ref()
    }

    /// The id of a block the code itself names (generation); a missing one is a broken data file.
    pub fn id(&self, name: &str) -> u8 {
        self.defs.iter().flatten().find(|d| d.name == name).unwrap_or_else(|| panic!("data/blocks.toml has no block '{name}'")).id
    }

    pub fn solid(&self, id: u8) -> bool {
        self.defs[id as usize].as_ref().is_some_and(|d| d.solid)
    }

    /// Every block but air, in id order.
    pub fn placeable(&self) -> impl Iterator<Item = &Def> {
        self.defs.iter().flatten().filter(|d| d.id != AIR)
    }
}

pub static BLOCKS: LazyLock<Registry> =
    LazyLock::new(|| Registry::parse(include_str!("../../../data/blocks.toml")).unwrap_or_else(|e| panic!("data/blocks.toml: {e}")));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_file_loads_and_names_what_generation_needs() {
        for n in ["air", "grass", "dirt", "stone"] {
            BLOCKS.id(n);
        }
        assert!(!BLOCKS.solid(AIR) && BLOCKS.solid(BLOCKS.id("stone")));
        assert_eq!(BLOCKS.get(BLOCKS.id("grass")).unwrap().side, BLOCKS.id("dirt"));
    }

    #[test]
    fn every_solid_block_takes_its_time_and_bare_hands_take_only_loose_things() {
        for d in BLOCKS.placeable() {
            assert!(d.hardness > 0.0 && d.hardness <= 60.0, "{}: {} s", d.name, d.hardness);
        }
        let get = |n: &str| BLOCKS.get(BLOCKS.id(n)).unwrap().clone();
        let (dirt, stone) = (get("dirt"), get("stone"));
        assert!(dirt.loose && dirt.hardness < 1.0, "dirt goes by hand in under a second");
        assert!(!stone.loose && stone.hardness > 5.0, "stone is slow by hand and gives nothing");
    }

    #[test]
    fn bad_files_are_refused() {
        let air = "[[block]]\nid = 0\nname = \"air\"\nsolid = false\n";
        assert!(Registry::parse(air).is_ok());
        assert!(Registry::parse(&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 0.5\nloose = true\n")).is_ok());
        for (bad, why) in [
            ("[[block]]\nid = 0\nname = \"rock\"\n", "0 is not air"),
            (&format!("{air}[[block]]\nid = 0\nname = \"x\"\n"), "id twice"),
            (&format!("{air}[[block]]\nid = 1\nname = \"air\"\n"), "name twice"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\nside = \"y\"\n"), "unknown side"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\ncolour = [2.0, 0, 0]\n"), "colour"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 3\nwobble = 3\n"), "unknown field"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\n"), "a solid block without its hardness"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 100\n"), "hardness"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\nlight = [5.0, 0, 0]\n"), "light"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\nmetal = 1.5\n"), "metal"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\npattern = \"bricks\"\nsize = 0\n"), "size 0"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\npattern = \"bricks\"\nsize = 9\n"), "size 9"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\npattern = \"tiles\"\namount = 1.2\n"), "amount"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\ncontrast = 2.5\n"), "contrast"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\ncolour2 = [0, -1, 0]\n"), "colour2"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\ncolour3 = [0, 1.5, 0]\n"), "colour3"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\nsize = 4\n"), "size on plain"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\npattern = \"stone\"\namount = 0.5\n"), "amount on stone"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 1\npattern = \"marble\"\n"), "unknown pattern"),
        ] {
            assert!(Registry::parse(bad).is_err(), "{why}");
        }
    }

    #[test]
    fn every_pattern_with_parts_parses_with_the_pages_defaults() {
        // the defaults are tools/review/workbench.html `PATTERNS[..].def`, `darker` and `lighter`
        let mut text = String::from("[[block]]\nid = 0\nname = \"air\"\nsolid = false\n");
        let all = [
            ("pebbles", 1, 0.5),
            ("speckle", 5, 0.6),
            ("frame", 2, 0.8),
            ("weave", 4, 0.35),
            ("planks", 4, 0.5),
            ("tiles", 8, 0.3),
            ("bricks", 4, 0.5),
            ("rings", 2, 0.5),
        ];
        for (i, (p, _, _)) in all.iter().enumerate() {
            text += &format!("[[block]]\nid = {}\nname = \"{p}\"\nhardness = 1\npattern = \"{p}\"\ncolour = [0.6, 0.4, 0.8]\n", i + 1);
        }
        text += "[[block]]\nid = 20\nname = \"set\"\nhardness = 1\npattern = \"bricks\"\ncolour2 = [0.1, 0.2, 0.3]\ncolour3 = [0.9, 0.8, 0.7]\nsize = 8\namount = 1.0\ncontrast = 0.0\nside = \"rings\"\n";
        let r = Registry::parse(&text).unwrap();
        for (i, (p, size, amount)) in all.iter().enumerate() {
            let d = r.get(i as u8 + 1).unwrap();
            assert_eq!((d.pattern as u8, d.size, d.amount, d.contrast), (i as u8 + 3, *size, *amount, 1.0), "{p}");
            assert_eq!(d.colour2, [0.33, 0.22, 0.44], "{p}: darker");
            assert_eq!(d.colour3, [0.95, 0.7, 1.0], "{p}: lighter");
        }
        let d = r.get(20).unwrap();
        assert_eq!((d.colour2, d.colour3, d.size, d.amount, d.contrast), ([0.1, 0.2, 0.3], [0.9, 0.8, 0.7], 8, 1.0, 0.0));
        assert_eq!(d.side, r.id("rings"), "any block may take another's look on its sides");
    }
}
