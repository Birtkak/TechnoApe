//! The block registry: every block's id, name and look, from `data/blocks.toml` (ADR 0013). The file is built
//! into the binary, so a client and a server from one build agree; `Welcome` carries `BLOCKS.hash` so builds
//! that do not agree refuse each other.
use serde::Deserialize;
use std::sync::LazyLock;

/// A block id (ADR 0062): 16 bits, of which the registry uses the low 8 until the shader reads more.
pub const AIR: u16 = 0;

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
    /// Leafy texels with holes of `size` texels, `amount` of them (ADR 0069); a hole is empty on a cutout block,
    /// a dark texel on any other.
    Leaves = 11,
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
            Leaves => Some((2, 0.3)),
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
    id: u16,
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
    #[serde(default)]
    falls: bool,
    #[serde(default)]
    soft: bool,
    model: Option<String>,
    #[serde(default)]
    see: See,
    #[serde(default)]
    stagger: f32,
    #[serde(default)]
    joins: bool,
    #[serde(default)]
    wood: bool,
    fluff: Option<Fluff>,
    rubble: Option<u16>,
    #[serde(default)]
    bedding: f32,
    #[serde(default)]
    joints: u16,
    #[serde(default = "full")]
    stack: u16,
    holds: Option<(String, f32)>,
    spring: Option<RawTone>,
    autumn: Option<RawTone>,
    winter: Option<RawTone>,
}
/// What a season's table may name: look fields only, nothing the server reads.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
struct RawTone {
    colour: Option<[f32; 3]>,
    colour2: Option<[f32; 3]>,
    colour3: Option<[f32; 3]>,
    amount: Option<f32>,
    contrast: Option<f32>,
}
/// The fluff of a cutout block (ADR 0090): the planes its faces open on air stand out of the plant, so a crown, a
/// shrub or a reed bed is a soft mass, not cubes. `fluff` in its row; what it leaves out is the broadleaf's.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Fluff {
    /// A plane's width in faces, 0..3 (0: no fluff).
    pub size: f32,
    /// How far it stands out, metres, 0..1.2 (more at a clump, less in a dent).
    pub push: f32,
    /// How far its outer parts hang down, metres a metre out, 0..1.2 (conifers, willows).
    pub droop: f32,
    /// Its length over its width, 1..4, along the way from the wood (a frond, a reed's blade).
    pub long: f32,
    /// The share of faces that have one, 0..1.
    pub dense: f32,
    /// The share of the sun one face of it lets through to a shadow ray, 0..1 (fast leaves; a ray through a crown
    /// passes two faces or more): the shade under it.
    pub light: f32,
}

impl Default for Fluff {
    fn default() -> Self {
        Fluff { size: 1.7, push: 0.35, droop: 0.0, long: 1.0, dense: 1.0, light: 0.5 }
    }
}

impl Fluff {
    /// Six bits a number for the shader's look row (`vs_fluff` reads them back): size / 0.05, push / 0.025,
    /// droop / 0.025, (long - 1) / 0.05, dense * 63.
    pub fn packed(&self) -> u32 {
        let q = |v: f32, step: f32| ((v / step).round() as u32).min(63);
        q(self.size, 0.05) | q(self.push, 0.025) << 6 | q(self.droop, 0.025) << 12 | q(self.long - 1.0, 0.05) << 18 | ((self.dense * 63.0).round() as u32).min(63) << 24
    }

    fn check(&self) -> bool {
        (0.0..=3.0).contains(&self.size) && (0.0..=1.2).contains(&self.push) && (0.0..=1.2).contains(&self.droop) && (1.0..=4.0).contains(&self.long) && (0.0..=1.0).contains(&self.dense) && (0.0..=1.0).contains(&self.light)
    }
}

/// How a block is seen through (docs/design/see-through-and-boxes.md, ADR 0069): `see` in its row.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum See {
    /// Every pixel there: the block hides what is behind it (every block before ADR 0069).
    #[default]
    Opaque,
    /// Pixels wholly there or wholly empty (alpha 255 or 0): leaves, a shrub, reed.
    Cutout,
    /// Pixels partly clear (any alpha), drawn blended after the light and tinting the light through it (ADR 0081).
    Glass,
}

/// The part of a look a season may change (docs/design/seasonal-looks.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tone {
    pub colour: [f32; 3],
    pub colour2: [f32; 3],
    pub colour3: [f32; 3],
    pub amount: f32,
    pub contrast: f32,
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
fn full() -> u16 {
    crate::items::STACK
}
/// Seconds a rubble block takes by hand (mining.md: fast).
pub const RUBBLE_S: f32 = 0.5;
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
    pub id: u16,
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
    pub side: u16,
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
    /// It falls when the cell under it opens and slides off a step of two (sand, gravel; ADR 0079).
    pub falls: bool,
    /// Sound passes it without an echo (leaves, a shrub, reed): no wall to the cave reverb (ADR 0054).
    pub soft: bool,
    /// The model (`data/models/<model>.toml`, ADR 0047) drawn in its cell in place of the cube; the cell
    /// still collides as a whole block.
    pub model: Option<String>,
    /// Whether it may have empty pixels (ADR 0069): the mesher keeps the faces behind a cutout block.
    pub see: See,
    /// Its looks in spring, autumn and winter; the row's own look is summer's, and a season the row does not
    /// name is that too. None: the block looks the same all year.
    pub seasons: Option<[Tone; 3]>,
    /// How far a column's own season runs ahead of or behind the place's, days either way, 0..10: trees turn
    /// one by one.
    pub stagger: f32,
    /// Neighbours of it in a wall join into one surface (lamps, ADR 0085): the mesher tells each face which of its
    /// four sides meet the same block, and the shader reads that side's frame texels from inside instead.
    pub joins: bool,
    /// Wood a tree grows from (log, branches): a bare crown's twigs start at it (ADR 0090).
    pub wood: bool,
    /// A cutout block's fluff (ADR 0090); None for any other.
    pub fluff: Option<Fluff>,
    /// The loose block a survival dig leaves in its cell (rock and ore, mining.md 4): `<name> rubble`, made from
    /// this row with the id it names. None: the dig takes the block itself.
    pub rubble: Option<u16>,
    /// Layered rock (ADR 0094): metres between its bedding planes, 0 for none.
    pub bedding: f32,
    /// Massive rock: about how many columns between its joints, 0 for none.
    pub joints: u16,
    /// The rock or ore a rubble block is of; None for any other block.
    pub rubble_of: Option<u16>,
    /// How many make a full slot (ADR 0074), 1..64.
    pub stack: u16,
    /// What a generated, never-edited cell of it may hold besides itself, and the chance (ADR 0097: flint in
    /// gravel): an item by name, as the items' rows read these (`edit::held` resolves it).
    pub holds: Option<(String, f32)>,
}

impl Def {
    /// Its look in season `s`: 0 summer (the row's own), 1 spring, 2 autumn, 3 winter, the rows of the
    /// renderer's look table.
    pub fn tone(&self, s: usize) -> Tone {
        let base = Tone { colour: self.colour, colour2: self.colour2, colour3: self.colour3, amount: self.amount, contrast: self.contrast };
        match (s, self.seasons) {
            (1..=3, Some(t)) => t[s - 1],
            _ => base,
        }
    }
}

/// The seasons in the year's order (`<block>@<season>.png`, the Studio's tabs); `ROW[k]` is season k's look in
/// `Def::tone` and the renderer's look table.
pub const SEASONS: [&str; 4] = ["spring", "summer", "autumn", "winter"];
pub const ROW: [usize; 4] = [1, 0, 2, 3];

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
            // shortcut: the shader unpacks 8 bits of material (`q.y & 255u`); widen it there, then lift this
            if r.id > 255 {
                return Err(format!("block '{}': id {} over 255, the shader's material field", r.name, r.id));
            }
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
            if !(0.0..=10.0).contains(&r.stagger) {
                return Err(format!("block '{}': stagger outside 0..10 days", r.name));
            }
            let base = Tone { colour: r.colour, colour2: r.colour2.unwrap_or(darker(r.colour)), colour3: r.colour3.unwrap_or(lighter(r.colour)), amount, contrast: r.contrast };
            let mut seasons = [base; 3];
            for (o, (s, season)) in seasons.iter_mut().zip([(r.spring, "spring"), (r.autumn, "autumn"), (r.winter, "winter")]) {
                let Some(s) = s else { continue };
                if [s.colour, s.colour2, s.colour3].iter().flatten().flatten().any(|c| !(0.0..=1.0).contains(c)) {
                    return Err(format!("block '{}': {season}: a colour outside 0..1", r.name));
                }
                if s.amount.is_some_and(|a| r.pattern.defaults().is_none() || !(0.0..=1.0).contains(&a)) {
                    return Err(format!("block '{}': {season}: amount outside 0..1, or on a pattern that takes none", r.name));
                }
                if s.contrast.is_some_and(|c| !(0.0..=2.0).contains(&c)) {
                    return Err(format!("block '{}': {season}: contrast outside 0..2", r.name));
                }
                // a season's colour brings its own darker and lighter parts, unless the season or the row names them
                let colour = s.colour.unwrap_or(base.colour);
                let part = |named: Option<[f32; 3]>, rows: Option<[f32; 3]>, made: [f32; 3]| named.or(rows).unwrap_or(made);
                *o = Tone { colour, colour2: part(s.colour2, r.colour2, darker(colour)), colour3: part(s.colour3, r.colour3, lighter(colour)), amount: s.amount.unwrap_or(amount), contrast: s.contrast.unwrap_or(r.contrast) };
            }
            if r.fluff.is_some() && r.see != See::Cutout {
                return Err(format!("block '{}': fluff on a block that is not see = \"cutout\"", r.name));
            }
            let fluff = (r.see == See::Cutout).then(|| r.fluff.unwrap_or_default());
            if fluff.is_some_and(|f| !f.check()) {
                return Err(format!("block '{}': fluff outside its ranges (size 0..3, push and droop 0..1.2, long 1..4, dense and light 0..1)", r.name));
            }
            let seasons = (r.spring.is_some() || r.autumn.is_some() || r.winter.is_some()).then_some(seasons);
            if seasons.is_none() && r.stagger != 0.0 {
                return Err(format!("block '{}': a stagger without a season", r.name));
            }
            if !(r.bedding == 0.0 || (1.0..=50.0).contains(&r.bedding)) || r.joints == 1 || r.joints > 100 || (r.bedding > 0.0 || r.joints > 0) && r.rubble.is_none() {
                return Err(format!("block '{}': bedding 1..50 m, joints 2..100 columns, and only on a rock that makes rubble", r.name));
            }
            if !(1..=crate::items::STACK).contains(&r.stack) {
                return Err(format!("block '{}': stack outside 1..64", r.name));
            }
            if r.holds.as_ref().is_some_and(|h| !(h.1 > 0.0 && h.1 <= 1.0) || !r.loose) {
                return Err(format!("block '{}': holds is for a loose block, with a chance over 0 and at most 1", r.name));
            }
            let def = Def { id: r.id, name: r.name.clone(), solid: r.solid, colour: r.colour, pattern: r.pattern, colour2: r.colour2.unwrap_or(darker(r.colour)), colour3: r.colour3.unwrap_or(lighter(r.colour)), size, amount, contrast: r.contrast, side, light: r.light, metal: r.metal, was: r.was.clone(), sound: r.sound.clone(), hardness: r.hardness, loose: r.loose, falls: r.falls, soft: r.soft, model: r.model.clone(), see: r.see, seasons, stagger: r.stagger, joins: r.joins, rubble: r.rubble, rubble_of: None, stack: r.stack, holds: r.holds.clone(), wood: r.wood, fluff, bedding: r.bedding, joints: r.joints };
            if let Some(t) = r.rubble {
                // the twin: the rock's colours in pebbles, loose and falling (ADR 0093)
                let name = format!("{} rubble", r.name);
                if t > 255 || defs[t as usize].is_some() || f.block.iter().any(|o| o.id == t || o.name == name) {
                    return Err(format!("block '{}': its rubble's id {t} or name is taken or over 255", r.name));
                }
                if r.loose || r.falls || !r.solid {
                    return Err(format!("block '{}': only a solid block that is not loose makes rubble", r.name));
                }
                defs[t as usize] = Some(Def { id: t, name, pattern: Pattern::Pebbles, size: 2, amount: 0.6, side: t, light: [0.0; 3], was: Vec::new(), hardness: RUBBLE_S, loose: true, falls: true, model: None, see: See::Opaque, seasons: None, stagger: 0.0, joins: false, rubble: None, rubble_of: Some(r.id), holds: None, wood: false, fluff: None, bedding: 0.0, joints: 0, ..def.clone() });
            }
            if defs[r.id as usize].is_some() {
                return Err(format!("block id {} is used twice", r.id));
            }
            defs[r.id as usize] = Some(def);
        }
        match &defs[0] {
            Some(d) if d.name == "air" && !d.solid => {}
            _ => return Err("block 0 must be a non-solid 'air'".into()),
        }
        Ok(Registry { defs, hash: crate::world::fnv(crate::world::FNV0, text.as_bytes()) })
    }

    pub fn get(&self, id: u16) -> Option<&Def> {
        self.defs.get(id as usize)?.as_ref()
    }

    /// The id of a block the code itself names (generation); a missing one is a broken data file.
    pub fn id(&self, name: &str) -> u16 {
        self.defs.iter().flatten().find(|d| d.name == name).unwrap_or_else(|| panic!("data/blocks.toml has no block '{name}'")).id
    }

    pub fn solid(&self, id: u16) -> bool {
        self.get(id).is_some_and(|d| d.solid)
    }

    /// Whether block `id` hides what is behind it: not air, not cutout. An id with no row counts as opaque, as
    /// the mesher always took it.
    pub fn opaque(&self, id: u16) -> bool {
        id != AIR && self.get(id).is_none_or(|d| d.see == See::Opaque)
    }

    /// Whether block `id` is glass (ADR 0081).
    pub fn glass(&self, id: u16) -> bool {
        self.get(id).is_some_and(|d| d.see == See::Glass)
    }

    /// Whether block `id` joins its neighbours in a wall (ADR 0085).
    pub fn joins(&self, id: u16) -> bool {
        self.get(id).is_some_and(|d| d.joins)
    }

    /// Whether block `id` is a tree's wood (ADR 0090).
    pub fn wood(&self, id: u16) -> bool {
        self.get(id).is_some_and(|d| d.wood)
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

    /// ADR 0090: a cutout block's fluff is data: a row's `fluff` table, the rest the broadleaf's, packed six bits
    /// a number for the shader; a row may name none (size 0), a number out of range is refused by name.
    #[test]
    fn a_cutout_rows_fluff_is_data_with_defaults_and_packs_for_the_shader() {
        let row = |extra: &str| format!("[[block]]
id = 0
name = \"air\"
solid = false
[[block]]
id = 1
name = \"leaf\"
see = \"cutout\"
colour = [0.2, 0.4, 0.1]
hardness = 0.3
{extra}");
        let r = Registry::parse(&row("")).unwrap();
        assert_eq!(r.get(1).unwrap().fluff, Some(Fluff::default()));
        assert_eq!(Fluff::default(), Fluff { size: 1.7, push: 0.35, droop: 0.0, long: 1.0, dense: 1.0, light: 0.5 });
        let r = Registry::parse(&row("fluff = { size = 1.2, droop = 0.4, long = 2.5, dense = 0.5 }")).unwrap();
        let f = r.get(1).unwrap().fluff.unwrap();
        assert_eq!((f.size, f.push, f.droop, f.long, f.dense), (1.2, 0.35, 0.4, 2.5, 0.5));
        // six bits each, in turn: size / 0.05, push / 0.025, droop / 0.025, (long - 1) / 0.05, dense * 63
        assert_eq!(f.packed(), 24 | 14 << 6 | 16 << 12 | 30 << 18 | 32 << 24);
        assert_eq!(Fluff { size: 0.0, ..f }.packed() & 63, 0, "size 0: no fluff");
        assert!(Registry::parse(&row("fluff = { size = 4.0 }")).err().unwrap().contains("leaf"));
        assert!(Registry::parse(&row("fluff = { long = 0.5 }")).is_err());
        assert!(Registry::parse(&row("fluff = { light = 1.5 }")).is_err());
        assert_eq!(BLOCKS.get(BLOCKS.id("acacia leaves")).unwrap().fluff.unwrap().light, 0.65, "sparse leaves let more through");
        // an opaque block has none, and naming one is refused
        assert_eq!(BLOCKS.get(BLOCKS.id("stone")).unwrap().fluff, None);
        assert!(Registry::parse(&row("fluff = { size = 1.0 }").replace("see = \"cutout\"
", "")).err().unwrap().contains("cutout"));
        assert!(BLOCKS.get(BLOCKS.id("spruce leaves")).unwrap().fluff.unwrap().droop > 0.0, "conifers droop");
    }

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

    /// ADR 0093 (mining.md 4): every rock and hard ore digs into a rubble twin that falls and bare hands take;
    /// soil, sand, wood and leaves do not; rock, rubble and ore stack to 16.
    #[test]
    fn rock_and_ore_dig_into_rubble_and_stack_by_16() {
        let rocks = crate::rock::ROCKS.provinces.iter().flat_map(|p| p.layers.iter().map(|l| l.0));
        let ores = crate::ore::MINERALS.kinds.iter().map(|k| k.block);
        let luna = ["anorthosite", "mare basalt", "iron-nickel", "kreep", "stone", "iron-stained rock", "copper-stained rock", "quartz-veined rock", "greisen", "bleached rock"].map(|n| BLOCKS.id(n));
        for id in rocks.chain(ores).chain(luna) {
            let d = BLOCKS.get(id).unwrap();
            assert_eq!(d.stack, 16, "{}", d.name);
            if d.loose {
                assert!(d.rubble.is_none(), "{}: loose ore goes as itself", d.name);
                continue;
            }
            let r = BLOCKS.get(d.rubble.unwrap_or_else(|| panic!("{} makes no rubble", d.name))).unwrap();
            assert!(r.solid && r.loose && r.falls && r.hardness < 1.0 && r.stack == 16 && r.name == format!("{} rubble", d.name), "{}", r.name);
            assert_eq!((r.colour, r.sound.as_str()), (d.colour, d.sound.as_str()), "{} looks and sounds like its rock", r.name);
        }
        for n in ["dirt", "grass", "sand", "gravel", "log", "spruce leaves", "clay", "ice", "copper", "glass"] {
            let d = BLOCKS.get(BLOCKS.id(n)).unwrap();
            assert!(d.rubble.is_none() && d.stack == 64, "{n}");
        }
        let air = "[[block]]
id = 0
name = \"air\"
solid = false
";
        let rock = |extra: &str| Registry::parse(&format!("{air}[[block]]
id = 1
name = \"x\"
hardness = 1
{extra}"));
        assert!(rock("rubble = 2
").is_ok_and(|r| r.get(2).is_some_and(|t| t.name == "x rubble")));
        assert!(rock("rubble = 1
").is_err() && rock("rubble = 0
").is_err() && rock("rubble = 2
loose = true
").is_err(), "a taken id, a loose rock");
        assert!(rock("stack = 0
").is_err() && rock("stack = 65
").is_err());
        // a second drop (ADR 0097): a loose block's, a chance in (0, 1]
        assert!(rock("loose = true
holds = [\"flint\", 0.25]
").is_ok() && rock("holds = [\"flint\", 0.25]
").is_err() && rock("loose = true
holds = [\"flint\", 1.5]
").is_err());
    }

    /// ADR 0054: a forest is no cave. Plants let sound through; ground and rock, the grass blocks too, echo.
    #[test]
    fn plants_are_soft_and_ground_is_not() {
        for d in BLOCKS.placeable() {
            let plant = d.name.ends_with("leaves") || ["shrub", "reed"].contains(&d.name.as_str());
            assert_eq!(d.soft, plant, "{}", d.name);
        }
    }

    /// ADR 0069: the plants are seen through, and of the rest only glass (ADR 0081).
    #[test]
    fn leaves_shrub_and_reed_are_cutout() {
        for d in BLOCKS.placeable().filter(|d| !d.name.ends_with("glass")) {
            let plant = d.name.ends_with("leaves") || ["shrub", "reed"].contains(&d.name.as_str());
            assert_eq!(d.see == See::Cutout, plant, "{}", d.name);
            assert_eq!(BLOCKS.opaque(d.id), !plant, "{}", d.name);
        }
        assert!(!BLOCKS.opaque(AIR) && BLOCKS.opaque(250), "air is not opaque, an unknown id is");
        let air = "[[block]]
id = 0
name = \"air\"
solid = false
";
        assert!(Registry::parse(&format!("{air}[[block]]
id = 1
name = \"x\"
hardness = 1
see = \"opaque\"
")).is_ok());
        assert!(Registry::parse(&format!("{air}[[block]]
id = 1
name = \"x\"
hardness = 1
see = \"glass\"
")).is_ok(), "glass, ADR 0081");
    }

    /// ADR 0081: the glass rows are glass, seen through and not cutout.
    #[test]
    fn glass_is_glass() {
        let g: Vec<_> = BLOCKS.placeable().filter(|d| d.name.ends_with("glass")).collect();
        assert!(g.len() >= 3);
        for d in g {
            assert_eq!(d.see, See::Glass, "{}", d.name);
            assert!(BLOCKS.glass(d.id) && !BLOCKS.opaque(d.id) && d.solid, "{}", d.name);
        }
        assert!(!BLOCKS.glass(AIR) && !BLOCKS.glass(BLOCKS.id("stone")));
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
    fn a_row_may_name_its_seasons_looks() {
        let head = "[[block]]\nid = 0\nname = \"air\"\nsolid = false\n[[block]]\nid = 1\nname = \"x\"\nhardness = 1\ncolour = [0.2, 0.4, 0.2]\n";
        let r = Registry::parse(&format!("{head}pattern = \"speckle\"\nstagger = 3.0\n[block.autumn]\ncolour = [0.6, 0.4, 0.2]\ncolour3 = [0.9, 0.1, 0.1]\namount = 0.9\n[block.winter]\ncontrast = 0.5\n")).unwrap();
        let d = r.get(1).unwrap();
        let (base, autumn, winter) = (d.tone(0), d.tone(2), d.tone(3));
        assert_eq!(d.stagger, 3.0);
        assert_eq!((base.colour, base.amount, base.contrast), ([0.2, 0.4, 0.2], 0.6, 1.0), "the row's own look is summer's");
        assert_eq!(d.tone(1), base, "a season it does not name is the base");
        // a season's colour brings its own darker part unless the season or the row names one
        assert_eq!((autumn.colour, autumn.colour2, autumn.colour3, autumn.amount), ([0.6, 0.4, 0.2], [0.33, 0.22, 0.11], [0.9, 0.1, 0.1], 0.9));
        assert_eq!(winter, Tone { contrast: 0.5, ..base });
        assert_eq!(r.get(0).unwrap().seasons, None);
        assert!(Registry::parse(&format!("{head}colour2 = [0.1, 0.1, 0.1]\n[block.spring]\ncolour = [0.5, 0.5, 0.5]\n")).unwrap().get(1).unwrap().tone(1).colour2 == [0.1, 0.1, 0.1], "the row's named part stays");
        for (bad, why) in [
            ("[block.autumn]\npattern = \"bricks\"\n", "a season names look fields only"),
            ("[block.autumn]\nsolid = false\n", "nothing the server reads"),
            ("[block.summer]\ncolour = [0.5, 0.5, 0.5]\n", "summer is the row itself"),
            ("[block.winter]\ncolour = [1.5, 0, 0]\n", "colour"),
            ("[block.winter]\ncolour2 = [0, -1, 0]\n", "colour2"),
            ("[block.winter]\namount = 0.5\n", "amount on plain"),
            ("[block.winter]\ncontrast = 3.0\n", "contrast"),
            ("stagger = 3.0\n", "a stagger without a season"),
            ("stagger = 11.0\n[block.winter]\ncontrast = 0.5\n", "stagger"),
            ("stagger = -1.0\n[block.winter]\ncontrast = 0.5\n", "stagger"),
        ] {
            assert!(Registry::parse(&format!("{head}{bad}")).is_err(), "{why}");
        }
        // the first rows with seasons; the evergreens keep one look
        for (n, has) in [("grass", true), ("broadleaf leaves", true), ("shrub", true), ("reed", true), ("meadow grass", true), ("spruce leaves", false), ("jungle leaves", false), ("savanna grass", false), ("dirt", false)] {
            assert_eq!(BLOCKS.get(BLOCKS.id(n)).unwrap().seasons.is_some(), has, "{n}");
        }
    }

    #[test]
    fn a_row_may_name_a_model() {
        let r = Registry::parse("[[block]]\nid = 0\nname = \"air\"\nsolid = false\n[[block]]\nid = 1\nname = \"furnace\"\nhardness = 3\nmodel = \"furnace\"\n").unwrap();
        assert_eq!(r.get(1).unwrap().model.as_deref(), Some("furnace"));
        assert!(BLOCKS.placeable().all(|d| d.model.is_none()), "no row of data/blocks.toml uses a model yet");
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
            let d = r.get(i as u16 + 1).unwrap();
            assert_eq!((d.pattern as u8, d.size, d.amount, d.contrast), (i as u8 + 3, *size, *amount, 1.0), "{p}");
            assert_eq!(d.colour2, [0.33, 0.22, 0.44], "{p}: darker");
            assert_eq!(d.colour3, [0.95, 0.7, 1.0], "{p}: lighter");
        }
        let d = r.get(20).unwrap();
        assert_eq!((d.colour2, d.colour3, d.size, d.amount, d.contrast), ([0.1, 0.2, 0.3], [0.9, 0.8, 0.7], 8, 1.0, 0.0));
        assert_eq!(d.side, r.id("rings"), "any block may take another's look on its sides");
    }
}
