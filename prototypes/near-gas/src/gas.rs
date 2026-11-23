//! Store (i): a sparse grid of 1 m gas cells, only where the air is off its rest, inside one coarse cell.
//!
//! A cell holds each species as whole mass units (`MU_PER_KG` a kilogram, so a liquid unit of water is a whole
//! number of them) and its sensible heat `e` in millijoules from 0 K. The air around the fine cells is the coarse
//! cell (`air.md`'s 128 m cell): every species and the heat there are one number each. A fine cell is made by
//! taking exactly its share of the coarse cell at its height, and dissolves by giving back exactly what it holds,
//! so the per-species ledger (fine + coarse) closes in whole units through every hand-off.
//!
//! Motion is one rule on every open face: the face's velocity is sped up by the pressure difference across it and,
//! on an upright face, by gravity on the density difference against the coarse air at rest (buoyancy), and slowed
//! by a friction; mass, species and heat then move from the upwind cell in whole units. The pressure is the ideal
//! gas against the coarse air's hydrostatic pressure at the cell's height, stiffened to a slowed sound speed
//! (`C_ART`) so a 20 Hz step is stable (the liquid's own `K` does the same under a ceiling). Thermal expansion,
//! stack draught, plumes, heavy gas running downhill and pressure through openings all come from that one rule.
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// The step, s: the server's tick (`net::TICK_HZ` = 20).
pub const DT: f64 = 0.05;
/// Mass units a kilogram: 1/8 mg, so a liquid unit (1000 kg / 4096) is exactly `LIQ_UNIT_MU`.
pub const MU_PER_KG: i64 = 8_000_000;
pub const LIQ_UNIT_MU: i64 = 1_953_125;
/// Terra's surface pull (ADR 0180), m/s^2.
pub const G: f64 = 19.62;
pub const R: f64 = 8.314462618;
pub const ATM: f64 = 101_325.0;
/// The slowed sound speed, m/s: the explicit step is stable while C_ART DT / 1 m < 1/sqrt(3).
/// shortcut: pressure waves run at 10 m/s, not 340; flows of a few m/s are right to Mach^2 (a few %), a blast
/// from an opened over-pressured room is slow; upgrade to an implicit pressure solve if blasts must be fast.
pub const C_ART: f64 = 10.0;
/// A face loses this share of its speed a step. shortcut: a stand-in for turbulent drag (the liquid's
/// `FRICTION`), not derived; it decides how fast sloshing dies, not where gas goes.
pub const FRICTION: f64 = 1.0 / 64.0;
/// A cell gives at most this share of itself a step.
const OUT_MAX: f64 = 0.5;

pub const NS: usize = 5;
pub const N2: usize = 0;
pub const O2: usize = 1;
pub const CO2: usize = 2;
pub const H2O: usize = 3;
pub const SOOT: usize = 4;

/// A gas species: the gas half of a substance row (ADR 0183).
pub struct Species {
    pub name: &'static str,
    /// kg/mol; 0 for particles, which weigh but press nothing.
    pub molar: f64,
    /// J/kg/K at constant pressure. shortcut: one value at every temperature.
    pub cp: f64,
    /// kg/m3 off the coarse air before a cell counts as off its rest.
    pub tol: f64,
}

pub const SPECIES: [Species; NS] = [
    Species { name: "nitrogen", molar: 0.028_013_4, cp: 1040.0, tol: 1e-3 },
    Species { name: "oxygen", molar: 0.031_998, cp: 918.0, tol: 1e-3 },
    Species { name: "carbon dioxide", molar: 0.044_009, cp: 844.0, tol: 1e-4 },
    Species { name: "water vapour", molar: 0.018_015, cp: 2010.0, tol: 1e-4 },
    Species { name: "soot", molar: 0.0, cp: 710.0, tol: 1e-7 },
];
/// Dry air by mass (argon folded into nitrogen).
pub const AIR: [f64; NS] = [0.7684, 0.2310, 0.0006, 0.0, 0.0];

#[derive(Default)]
pub struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, b: &[u8]) {
        for &x in b {
            self.write_u64(x as u64)
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}
pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

pub fn key(p: [i32; 3]) -> u64 {
    (p[0] as u32 as u64 & 0x1F_FFFF) << 42 | (p[1] as u32 as u64 & 0x1F_FFFF) << 21 | (p[2] as u32 as u64 & 0x1F_FFFF)
}
/// The six faces: +i, -i, +j, -j, +k, -k; `d ^ 1` is the opposite one.
pub const DIRS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
pub fn step_to(p: [i32; 3], d: usize) -> [i32; 3] {
    [p[0] + DIRS[d][0], p[1] + DIRS[d][1], p[2] + DIRS[d][2]]
}

/// The blocks: what the gas needs from the world. Inside `lo..hi` (the near box, where fine cells may live) a
/// block id a cell, outside it solid under `out_solid_below` and coarse air over it.
pub struct Ground {
    pub lo: [i32; 3],
    pub hi: [i32; 3],
    pub blocks: Vec<u8>,
    pub out_solid_below: i32,
}
pub const AIR_BLOCK: u8 = 0;
impl Ground {
    pub fn new(lo: [i32; 3], hi: [i32; 3], out_solid_below: i32) -> Ground {
        let n = ((hi[0] - lo[0]) * (hi[1] - lo[1]) * (hi[2] - lo[2])) as usize;
        Ground { lo, hi, blocks: vec![AIR_BLOCK; n], out_solid_below }
    }
    pub fn near(&self, p: [i32; 3]) -> bool {
        (0..3).all(|a| p[a] >= self.lo[a] && p[a] < self.hi[a])
    }
    fn idx(&self, p: [i32; 3]) -> usize {
        let s = [self.hi[0] - self.lo[0], self.hi[1] - self.lo[1]];
        ((p[2] - self.lo[2]) * s[0] * s[1] + (p[1] - self.lo[1]) * s[0] + (p[0] - self.lo[0])) as usize
    }
    pub fn block(&self, p: [i32; 3]) -> u8 {
        if self.near(p) {
            self.blocks[self.idx(p)]
        } else if p[2] < self.out_solid_below {
            1
        } else {
            AIR_BLOCK
        }
    }
    pub fn set(&mut self, p: [i32; 3], b: u8) {
        let i = self.idx(p);
        self.blocks[i] = b;
    }
    pub fn fill(&mut self, lo: [i32; 3], hi: [i32; 3], b: u8) {
        for k in lo[2]..hi[2] {
            for j in lo[1]..hi[1] {
                for i in lo[0]..hi[0] {
                    self.set([i, j, k], b);
                }
            }
        }
    }
}

/// The coarse cell: every species and its heat as one number, over `vol` m3 whose mean height is `z_mean`.
#[derive(Clone)]
pub struct Coarse {
    pub m: [i64; NS],
    pub e: i64,
    pub vol: f64,
    pub z_mean: f64,
}

/// The coarse air at rest at one height, as one 1 m3 cell would hold it.
#[derive(Clone, Copy, Default)]
pub struct Amb {
    pub m: [i64; NS],
    pub e: i64,
    pub rho: f64,
    pub t: f64,
    pub p: f64,
}

pub fn heat_cap(m: &[i64; NS]) -> f64 {
    (0..NS).map(|s| m[s] as f64 / MU_PER_KG as f64 * SPECIES[s].cp).sum()
}
pub fn moles(m: &[i64; NS]) -> f64 {
    (0..NS).filter(|&s| SPECIES[s].molar > 0.0).map(|s| m[s] as f64 / MU_PER_KG as f64 / SPECIES[s].molar).sum()
}
pub fn mass(m: &[i64; NS]) -> f64 {
    m.iter().sum::<i64>() as f64 / MU_PER_KG as f64
}
pub fn temp(m: &[i64; NS], e: i64) -> f64 {
    e as f64 / 1000.0 / heat_cap(m)
}

impl Coarse {
    /// Still dry air at `t` K and 1 atm at its mean height.
    pub fn standard(vol: f64, z_mean: f64, t: f64) -> Coarse {
        let mmix = 1.0 / (0..NS).filter(|&s| SPECIES[s].molar > 0.0).map(|s| AIR[s] / SPECIES[s].molar).sum::<f64>();
        let rho = ATM * mmix / (R * t);
        let mut m = [0; NS];
        for s in 0..NS {
            m[s] = (AIR[s] * rho * vol * MU_PER_KG as f64) as i64;
        }
        let e = (heat_cap(&m) * t * 1000.0) as i64;
        Coarse { m, e, vol, z_mean }
    }
    pub fn temp(&self) -> f64 {
        temp(&self.m, self.e)
    }
    /// The coarse air at rest at height `z` (m, a cell's middle) in one cell, with `free` m3 not taken by fine cells:
    /// the mean share, leaned by the scale height.
    pub fn amb(&self, z: f64, free: f64) -> Amb {
        let t = self.temp();
        let mmix = mass(&self.m) / moles(&self.m);
        let f = (-(z - self.z_mean) * mmix * G / (R * t)).exp() / free;
        let mut a = Amb { t, ..Default::default() };
        for s in 0..NS {
            a.m[s] = (self.m[s] as f64 * f) as i64;
        }
        a.e = (heat_cap(&a.m) * t * 1000.0) as i64;
        a.rho = mass(&a.m);
        a.p = moles(&a.m) * R * t;
        a
    }
}

pub const SOLID: u32 = u32::MAX;
pub const BG: u32 = u32::MAX - 1;

#[derive(Clone)]
pub struct Cell {
    pub p: [i32; 3],
    pub m: [i64; NS],
    pub e: i64,
    /// Face speeds along +axis, m/s. v[d] for an even d is the face to the +d neighbour; for an odd d it is the
    /// face to the -d neighbour only while that neighbour is coarse (a fine one owns it as its even face).
    pub v: [f64; 6],
    pub nb: [u32; 6],
    /// A source beside it keeps it (`heat.rs`).
    pub pin: bool,
}

pub struct Gas {
    pub cells: Vec<Cell>,
    pub at: FxMap<u64, u32>,
    pub coarse: Coarse,
    /// Mass units each species gained from sources (negative: lost to sinks), the ledger's other side.
    pub made: [i64; NS],
    /// The coarse wind, m/s along +i and +j: faces' speeds relax to it through the friction.
    pub wind: [f64; 2],
    pub steps: u64,
    /// Fine cells made and dissolved, for the stats.
    pub made_cells: u64,
    pub gone_cells: u64,
    /// Species and heat at faces by van Leer's reconstruction (else the donor's own: first order).
    pub second: bool,
    amb: Vec<Amb>,
    k0: i32,
}

/// The value at a face from the cell behind the donor, the donor and the cell the flow goes to, at a Courant
/// number c: the donor's, plus van Leer's limited slope (second order where smooth, first at a front).
fn face(behind: f64, donor: f64, to: f64, c: f64) -> f64 {
    let (a, b) = (donor - behind, to - donor);
    let slope = if a * b <= 0.0 { 0.0 } else { 2.0 * a * b / (a + b) };
    donor + 0.5 * (1.0 - c) * slope
}

/// x times f in whole units, down.
fn part(x: i64, f: f64) -> i64 {
    (x as f64 * f).floor() as i64
}

impl Gas {
    pub fn new(coarse: Coarse, g: &Ground) -> Gas {
        let mut gas = Gas {
            cells: Vec::new(),
            at: FxMap::default(),
            coarse,
            made: [0; NS],
            wind: [0.0; 2],
            steps: 0,
            made_cells: 0,
            gone_cells: 0,
            second: true,
            amb: Vec::new(),
            k0: g.lo[2],
        };
        gas.refresh_amb(g);
        gas
    }
    fn refresh_amb(&mut self, g: &Ground) {
        let free = self.coarse.vol - self.cells.len() as f64;
        self.amb = (g.lo[2]..g.hi[2] + 1).map(|k| self.coarse.amb(k as f64 + 0.5, free)).collect();
    }
    pub fn amb_at(&self, k: i32) -> &Amb {
        &self.amb[(k - self.k0).clamp(0, self.amb.len() as i32 - 1) as usize]
    }
    pub fn total(&self, s: usize) -> i64 {
        self.coarse.m[s] + self.cells.iter().map(|c| c.m[s]).sum::<i64>()
    }
    pub fn energy(&self) -> i64 {
        self.coarse.e + self.cells.iter().map(|c| c.e).sum::<i64>()
    }
    pub fn cell_at(&self, p: [i32; 3]) -> Option<u32> {
        self.at.get(&key(p)).copied()
    }
    pub fn temp(&self, i: u32) -> f64 {
        let c = &self.cells[i as usize];
        temp(&c.m, c.e)
    }

    /// Make the open block at `p` a fine cell (it takes its share of the coarse cell), or find it.
    pub fn activate(&mut self, g: &Ground, p: [i32; 3]) -> Option<u32> {
        if let Some(i) = self.cell_at(p) {
            return Some(i);
        }
        if !g.near(p) || g.block(p) != AIR_BLOCK {
            return None;
        }
        let a = *self.amb_at(p[2]);
        for s in 0..NS {
            self.coarse.m[s] -= a.m[s];
        }
        self.coarse.e -= a.e;
        let n = self.cells.len() as u32;
        let mut c = Cell { p, m: a.m, e: a.e, v: [0.0; 6], nb: [BG; 6], pin: false };
        for d in 0..6 {
            let q = step_to(p, d);
            if g.block(q) != AIR_BLOCK {
                c.nb[d] = SOLID;
            } else if let Some(o) = self.cell_at(q) {
                c.nb[d] = o;
                let oc = &mut self.cells[o as usize];
                oc.nb[d ^ 1] = n;
                if d % 2 == 0 {
                    // o was at our +d side; it kept that face as its coarse -d face: it is ours now
                    c.v[d] = oc.v[d ^ 1];
                }
            }
        }
        self.cells.push(c);
        self.at.insert(key(p), n);
        self.made_cells += 1;
        Some(n)
    }

    /// Give cell `i` back to the coarse cell.
    fn dissolve(&mut self, i: u32) {
        let c = self.cells[i as usize].clone();
        for s in 0..NS {
            self.coarse.m[s] += c.m[s];
        }
        self.coarse.e += c.e;
        for d in 0..6 {
            let o = c.nb[d];
            if o < BG {
                let oc = &mut self.cells[o as usize];
                oc.nb[d ^ 1] = BG;
                if d % 2 == 0 {
                    oc.v[d ^ 1] = c.v[d];
                }
            }
        }
        self.at.remove(&key(c.p));
        let last = self.cells.len() as u32 - 1;
        self.cells.swap_remove(i as usize);
        if i != last {
            let moved = self.cells[i as usize].clone();
            self.at.insert(key(moved.p), i);
            for d in 0..6 {
                let o = moved.nb[d];
                if o < BG {
                    self.cells[o as usize].nb[d ^ 1] = i;
                }
            }
        }
        self.gone_cells += 1;
    }

    /// Add `units` of species `s` to cell `i` from a source (booked).
    pub fn add(&mut self, i: u32, s: usize, units: i64) {
        self.cells[i as usize].m[s] += units;
        self.made[s] += units;
    }

    /// One step of the gas: faces' speeds, then whole units upwind, then the cells made and dissolved.
    pub fn step(&mut self, g: &Ground) {
        self.refresh_amb(g);
        self.transport(g);
        self.rest(g);
        self.steps += 1;
    }

    /// The pressure of cell `i` over the coarse air's at its height and temperature, stiffened to `C_ART`, Pa.
    pub fn over(&self, i: usize) -> f64 {
        let c = &self.cells[i];
        let a = self.amb_at(c.p[2]);
        let r = mass(&c.m);
        r * C_ART * C_ART * (moles(&c.m) * R * temp(&c.m, c.e) / a.p - 1.0)
    }
    /// The real pressure (the ideal gas, unstiffened), Pa.
    pub fn pressure(&self, i: usize) -> f64 {
        let c = &self.cells[i];
        moles(&c.m) * R * temp(&c.m, c.e)
    }

    fn transport(&mut self, _g: &Ground) {
        let n = self.cells.len();
        let rho: Vec<f64> = self.cells.iter().map(|c| mass(&c.m)).collect();
        let pp: Vec<f64> = (0..n).map(|i| self.over(i)).collect();
        // (lower side, upper side, the lower side's height, axis, speed); a side is a cell or BG
        let mut faces: Vec<(u32, u32, i32, usize, f64)> = Vec::with_capacity(n * 3);
        for a in 0..n {
            let k = self.cells[a].p[2];
            for d in 0..6 {
                let o = self.cells[a].nb[d];
                if o == SOLID || (d % 2 == 1 && o != BG) {
                    continue;
                }
                let axis = d / 2;
                let up = if axis == 2 { 1 } else { 0 };
                // the lower (-axis) and upper (+axis) sides: their density, over-pressure and height
                let (lo, hi, klo) = if d % 2 == 0 { (a as u32, o, k) } else { (o, a as u32, k - up) };
                let side = |s: u32, kk: i32| if s == BG { (self.amb_at(kk).rho, 0.0) } else { (rho[s as usize], pp[s as usize]) };
                let (r_lo, p_lo) = side(lo, klo);
                let (r_hi, p_hi) = side(hi, klo + up);
                let r = 0.5 * (r_lo + r_hi);
                let mut f = p_lo - p_hi;
                if axis == 2 {
                    let r_amb = 0.5 * (self.amb_at(klo).rho + self.amb_at(klo + 1).rho);
                    f -= (r - r_amb) * G;
                }
                let v = &mut self.cells[a].v[d];
                *v += DT * f / r;
                let rest = if axis < 2 { self.wind[axis] } else { 0.0 };
                *v -= (*v - rest) * FRICTION;
                faces.push((lo, hi, klo, axis, *v));
            }
        }
        // a cell gives at most OUT_MAX of itself a step
        let mut out = vec![0.0; n];
        for &(lo, hi, _, _, w) in &faces {
            let donor = if w > 0.0 { lo } else { hi };
            if donor < BG {
                out[donor as usize] += w.abs() * DT;
            }
        }
        let mut dm = vec![[0i64; NS]; n];
        let mut de = vec![0i64; n];
        for &(lo, hi, klo, axis, w) in &faces {
            let (donor, to, kd) = if w > 0.0 { (lo, hi, klo) } else { (hi, lo, klo + if axis == 2 { 1 } else { 0 }) };
            let mut f = w.abs() * DT;
            let (m, e) = if donor == BG {
                let a = self.amb_at(kd);
                (a.m, a.e)
            } else {
                let i = donor as usize;
                f *= (OUT_MAX / out[i]).min(1.0);
                (self.cells[i].m, self.cells[i].e)
            };
            let mut mv = [0i64; NS];
            let mut ev = part(e, f);
            for s in 0..NS {
                mv[s] = part(m[s], f);
            }
            if donor != BG && self.second {
                // the share of each species (and the heat a unit) at the face, from the donor's and its
                // neighbours' on the line of the flow, van Leer limited; whole units still leave the donor
                let i = donor as usize;
                let up = if axis == 2 { 1 } else { 0 };
                let (behind, kb, kt) = if w > 0.0 { (self.cells[i].nb[2 * axis + 1], klo - up, klo + up) } else { (self.cells[i].nb[2 * axis], klo + 2 * up, klo) };
                let share = |side: u32, kk: i32| -> ([f64; NS], f64) {
                    let (m, e) = match side {
                        BG => (self.amb_at(kk).m, self.amb_at(kk).e),
                        SOLID => (self.cells[i].m, self.cells[i].e),
                        o => (self.cells[o as usize].m, self.cells[o as usize].e),
                    };
                    let t = m.iter().sum::<i64>() as f64;
                    (m.map(|x| x as f64 / t), e as f64 / t)
                };
                let (qd, hd) = share(donor, 0);
                let (qb, hb) = share(behind, kb);
                let (qt, ht) = share(to, kt);
                let units = f * m.iter().sum::<i64>() as f64;
                for s in 0..NS {
                    mv[s] = ((face(qb[s], qd[s], qt[s], f) * units).floor() as i64).clamp(0, part(m[s], 2.0 * f));
                }
                ev = ((face(hb, hd, ht, f) * units).floor() as i64).clamp(0, part(e, 2.0 * f));
            }
            for (side, sign) in [(donor, -1), (to, 1)] {
                if side == BG {
                    for s in 0..NS {
                        self.coarse.m[s] += sign * mv[s];
                    }
                    self.coarse.e += sign * ev;
                } else {
                    for s in 0..NS {
                        dm[side as usize][s] += sign * mv[s];
                    }
                    de[side as usize] += sign * ev;
                }
            }
        }
        for i in 0..n {
            let c = &mut self.cells[i];
            for s in 0..NS {
                c.m[s] += dm[i][s];
            }
            c.e += de[i];
        }
    }

    /// Cell `i` is off the coarse air's rest: a species, its temperature, its pressure, or a face moving.
    pub fn off(&self, i: usize) -> bool {
        let c = &self.cells[i];
        let a = self.amb_at(c.p[2]);
        if (0..NS).any(|s| ((c.m[s] - a.m[s]) as f64 / MU_PER_KG as f64).abs() > SPECIES[s].tol) {
            return true;
        }
        let t = temp(&c.m, c.e);
        if (t - a.t).abs() > 0.5 || (moles(&c.m) * R * t / a.p - 1.0).abs() > 1e-4 {
            return true;
        }
        (0..6).any(|d| {
            let o = c.nb[d];
            let v = if d % 2 == 1 && o < BG { self.cells[o as usize].v[d ^ 1] } else { c.v[d] };
            v.abs() > 0.02
        })
    }

    /// Dissolve cells at rest among cells at rest; grow fine cells into the coarse air beside cells off it.
    fn rest(&mut self, g: &Ground) {
        let n = self.cells.len();
        let off: Vec<bool> = (0..n).map(|i| self.off(i)).collect();
        let mut gone = Vec::new();
        let mut grow = Vec::new();
        for i in 0..n {
            let c = &self.cells[i];
            if off[i] {
                for d in 0..6 {
                    if c.nb[d] == BG {
                        grow.push(step_to(c.p, d));
                    }
                }
            } else if !c.pin && c.nb.iter().all(|&o| o >= BG || !off[o as usize]) {
                gone.push(i as u32);
            }
        }
        for &i in gone.iter().rev() {
            self.dissolve(i);
        }
        for q in grow {
            self.activate(g, q);
        }
    }
}
