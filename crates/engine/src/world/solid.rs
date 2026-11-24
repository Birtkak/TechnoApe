//! The solid world as the water sees it (`liquid::Ground`).
use super::*;

/// The solid world as the water sees it (`liquid::Ground`): terrain from the seed plus the edits.
pub struct Solid<'a> {
    pub bodies: &'a [Arc<Body>],
    pub edits: &'a FxMap<Key, FxMap<IVec3, u16>>,
    /// The planet's sea at its tide (ADR 0067): the whole second of world time it was taken at and the tide
    /// then; None, the mean sea.
    pub tide: Option<(i64, Tide)>,
    /// The air water's phases go by (`liquid::phases`; `World::cool_fluids`): None, `Ground::air`'s default.
    pub air: Option<Air<'a>>,
}

/// The air over the ground at a moment, for water's phases: the sky (its year sets ADR 0187's pace, its sun the
/// season and the hour), and the temperature at a place where a live air grid gives it (the server's weather), else
/// the baked climate's year with the season and the hour on top (`sky::temperature`).
#[derive(Clone, Copy)]
pub struct Air<'a> {
    pub sky: &'a Sky,
    pub t: f64,
    pub temp: Option<&'a dyn Fn(Col, f64) -> Option<f64>>,
}

impl<'a> Solid<'a> {
    /// The solid world of `bodies` and `edits` with the sea at its tide at world time t, taken at the whole
    /// second: generation's water steps once a second (3 mm at most), so the water's caches hold for one.
    pub fn at(bodies: &'a [Arc<Body>], edits: &'a FxMap<Key, FxMap<IVec3, u16>>, sky: &Sky, t: f64) -> Solid<'a> {
        let s = t.floor();
        let tide = bodies.first().filter(|b| b.sea.is_some()).map(|b| (s as i64, sky.tide(s, &b.ocean)));
        Solid { bodies, edits, tide, air: None }
    }
    /// The air at layer z of column c, C and Pa (`Ground::air`).
    fn air_at(&self, a: &Air, c: Col, z: f64) -> (f64, f64) {
        let b = &self.bodies[c.body() as usize];
        let (face, fi, fj) = (c.face() as usize, c.i() as f64 + 0.5, c.j() as f64 + 0.5);
        let ground = || b.heat(c.face(), c.i(), c.j(), z).map_or(crate::liquid::AMBIENT, |t| t) as f64;
        if !b.air {
            // no air: the ground's own heat, nothing to hold a liquid
            return (ground(), 0.0);
        }
        let over = b.layer_r(z) - b.sea.map_or(b.radius, |s| b.layer_r(s));
        let t = match a.temp.and_then(|f| f(c, z)) {
            Some(t) => t,
            // the planet's baked year (the biome's band on an older map), the season and the hour; elsewhere its rock
            None if c.body() == 0 => {
                let d = b.dir(face, fi, fj);
                let ap = crate::bake::air::Params::default();
                let clim = match b.climate_at(face, fi, fj) {
                    Some((theta, swing, _)) => Some(((theta - ap.cool(over.max(0.0) as f32)) as f64, swing as f64)),
                    None => b.biome_at(face, fi, fj, d).map(|bio| crate::sky::BIOME_CLIMATE[bio as usize]),
                };
                match clim {
                    Some((mean, swing)) => crate::sky::temperature(mean, swing, d.z.clamp(-1.0, 1.0).asin(), a.sky.declination(a.t), a.sky.hour_angle(a.t, d)),
                    None => ground(),
                }
            }
            None => ground(),
        };
        (t, crate::liquid::phases::pressure(over, t, self.g(c.body())))
    }
    fn water(&self, c: Col) -> Option<(f64, u32, Option<(f64, f64, f64)>)> {
        let tide = if c.body() == 0 { self.tide.as_ref().map(|t| &t.1) } else { None };
        self.bodies[c.body() as usize].water_tide(c.face(), c.i(), c.j(), tide)
    }
}

impl Solid<'_> {
    /// The first generated air layer of column c.
    pub(super) fn gen_floor(&self, c: Col) -> i32 {
        let b = &self.bodies[c.body() as usize];
        // the same test as `generated_under`, so the two never disagree by a layer
        b.first_air(b.col_height(c.face(), c.i(), c.j()))
    }
}

impl Ground for Solid<'_> {
    fn gaps(&self, c: Col) -> Vec<(i32, i32)> {
        let zs = self.gen_floor(c);
        let b = &self.bodies[c.body() as usize];
        let (i, j) = (c.i(), c.j());
        let mut edits: Vec<(i32, bool)> = Vec::new();
        for zc in 0..=(b.layers + CS) / CS {
            let k = Key { body: c.body(), face: c.face(), level: 0, pos: IVec3::new(i.div_euclid(CS), j.div_euclid(CS), zc) };
            if let Some(cells) = self.edits.get(&k) {
                for z in zc * CS..(zc + 1) * CS {
                    if let Some(m) = cells.get(&IVec3::new(i, j, z)) {
                        edits.push((z, BLOCKS.solid(*m)));
                    }
                }
            }
        }
        // the plants over the ground, as solid as edits, where no edit is (ADR 0043)
        let n_edits = edits.len();
        for (z, m) in b.col_plants(c.face(), i, j) {
            if z >= zs && !edits[..n_edits].iter().any(|e| e.0 == z) {
                edits.push((z, BLOCKS.solid(m)));
            }
        }
        edits.sort_unstable_by_key(|e| e.0);
        let caves = b.col_caves(c.face(), i, j);
        let lo = edits.first().map_or(zs, |e| e.0.min(zs)).min(caves.iter().map(|c| c.0).min().unwrap_or(zs)).max(0);
        let hi = edits.last().map_or(zs, |e| (e.0 + 1).max(zs));
        let solid = |z: i32| edits.iter().find(|e| e.0 == z).map_or(z < zs && !caves.iter().any(|&(a, b)| (a..b).contains(&z)), |e| e.1);
        let mut out = Vec::new();
        let mut start = None;
        for z in lo..hi {
            match (solid(z), start) {
                (false, None) => start = Some(z),
                (true, Some(s)) => {
                    out.push((s, z));
                    start = None;
                }
                _ => {}
            }
        }
        out.push((start.unwrap_or(hi), OPEN));
        out
    }
    fn generated(&self, c: Col) -> (i64, i64) {
        let Some((top, _, _)) = self.water(c) else { return (0, 0) };
        (self.gen_floor(c) as i64 * U, (top * U as f64) as i64)
    }
    fn generated_other(&self, c: Col) -> Vec<(i64, i64, i64, u8)> {
        self.bodies[c.body() as usize].lava_col(c.face(), c.i(), c.j())
    }
    fn heat(&self, c: Col, z: f64) -> f32 {
        self.bodies[c.body() as usize].rest_heat(c.face(), c.i(), c.j(), z)
    }
    fn air(&self, c: Col, z: f64) -> (f64, f64) {
        match &self.air {
            Some(a) => self.air_at(a, c, z),
            None => (crate::liquid::AMBIENT as f64, crate::liquid::phases::ATM),
        }
    }
    fn pace(&self) -> f64 {
        self.air.as_ref().map_or(1.0, |a| crate::liquid::phases::REAL_YEAR / a.sky.year)
    }
    fn ice(&self, c: Col, z: i32) -> bool {
        let Some(f) = crate::fluids::FLUIDS.now().get(crate::fluids::WATER).freeze.as_ref() else { return false };
        let ice = BLOCKS.id(&f.into);
        let k = Key { body: c.body(), face: c.face(), level: 0, pos: IVec3::new(c.i().div_euclid(CS), c.j().div_euclid(CS), z.div_euclid(CS)) };
        match self.edits.get(&k).and_then(|cells| cells.get(&IVec3::new(c.i(), c.j(), z))) {
            Some(&m) => m == ice,
            // the bake's: its sea ice and the plants over the ground
            None => self.bodies[c.body() as usize].col_plants(c.face(), c.i(), c.j()).iter().any(|&(zz, m)| zz == z && m == ice),
        }
    }
    fn reach(&self, c: Col) -> u32 {
        self.water(c).map_or(crate::liquid::SEA, |w| w.1)
    }
    fn river(&self, c: Col) -> Option<(f64, f64, f64)> {
        self.water(c).and_then(|w| w.2)
    }
    fn tide_epoch(&self) -> i64 {
        self.tide.map_or(0, |t| t.0)
    }
    fn g(&self, body: u8) -> f64 {
        // `step` asks for every slot up to `bodies::MAX`, more than a world may have
        match self.bodies.get(body as usize) {
            Some(b) if b.gen >= crate::world::REAL => b.interior.surface_g,
            _ => crate::liquid::EARTH_G,
        }
    }
    fn tide_top(&self, c: Col) -> i64 {
        let b = &self.bodies[c.body() as usize];
        b.sea.map_or(i64::MIN, |s| ((s + b.tide_high / (b.layer_r(s) * b.lnq)) * U as f64) as i64 + 1)
    }
    fn down(&self, acct: u32) -> (u32, f64) {
        // reaches are Terra's baked map's (Luna has none); the map's NONE (to the sea) is the sea's account
        let r = self.bodies[0].baked.as_ref().and_then(|b| b.map.lines.reaches.get(acct as usize));
        r.map_or((crate::liquid::SEA, 0.0), |r| (r.down, r.hold as f64))
    }
    fn base(&self, acct: u32) -> f64 {
        // a river reach's generated flow where it leaves (its last segment's), the dry-weather base, units a second
        let Some(l) = self.bodies[0].baked.as_ref().map(|b| &b.map.lines) else { return 0.0 };
        let q = l.reaches.get(acct as usize).filter(|r| !r.lake).and_then(|r| r.segs.last()).map_or(0.0, |&k| l.segs[k as usize].q[1] as f64);
        q * crate::weather::BASE * U as f64
    }
    fn neighbours(&self, c: Col) -> [Option<Col>; 4] {
        let n = self.bodies[c.body() as usize].n;
        let (i, j) = (c.i(), c.j());
        [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].map(|(a, b)| {
            let (f, a, b) = fold(c.face() as usize, a, b, n);
            Some(Col::new(c.body(), f as u8, a, b))
        })
    }
}
