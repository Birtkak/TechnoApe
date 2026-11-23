//! The sparse heat store of the blocks beside off-rest gas, and fire as a reaction in it.
//!
//! The water-phases spike's idea (its README, "Where heat is held"): heat is held only where something happens.
//! A block joins when the gas beside it is off the coarse air's temperature, and leaves at rest. A block is two
//! nodes: a skin as deep as heat soaks in over ten minutes (`sqrt(alpha 600 s)`: 9 mm of wood, 3 cm of stone) on
//! its open faces, and the core behind it. The skin trades heat with the gas cells on its faces (convection and
//! the soot's radiation) and with its core (conduction). Water in a skin holds it at 100 C until it has boiled
//! off, whole liquid units into the gas's water vapour. A fuel's skin at its ignition point, with oxygen beside it,
//! burns at its row's rate; the heat goes first to hold the burning skin at its ignition point (the flame's feedback),
//! the rest into the gas cells on its faces with the products, and the oxygen comes out of them.
use crate::gas::*;

pub const STONE: u8 = 1;
pub const WOOD: u8 = 2;
pub const POT: u8 = 3;

/// A block's substance row: the solid half (ADR 0183's "a row is a substance").
pub struct Mat {
    pub name: &'static str,
    /// kg/m3, J/kg/K, W/m/K.
    pub rho: f64,
    pub cp: f64,
    pub k: f64,
    /// Skin kg when fixed (a pot), else 0: the skin is sqrt(alpha 600 s) deep on every open face.
    pub skin_kg: f64,
    /// Area of a face that trades heat, m2 (a pot is smaller than its block).
    pub face: f64,
    /// Fuel, kg in a block (0: it does not burn), and the fire's row.
    pub fuel: f64,
    pub ignite: f64,
    /// kg of fuel a m2 of burning face gives a second.
    pub burn: f64,
    /// Heat of combustion, mJ a mass unit (15 MJ/kg: 1875).
    pub hc: i64,
    /// kg of oxygen used, carbon dioxide and soot made, a kg of fuel; the water vapour is the rest of the mass.
    pub o2: f64,
    pub co2: f64,
    pub soot: f64,
    /// Oxygen's mass share under which the flame cannot stand (about 13 % by volume).
    pub o2_min: f64,
    /// Lateral flame spread (Quintiere's LIFT, measured together): the flame heating parameter, W2/m3, and the
    /// effective k rho c, W2 s/m4/K2; the front crosses a face at phi / (krc (T_ig - T_s)^2).
    pub phi: f64,
    pub krc: f64,
    /// The share of the heat released that the flame radiates.
    pub chi_r: f64,
}

pub const MATS: [Mat; 4] = [
    Mat { name: "air", rho: 0.0, cp: 0.0, k: 0.0, skin_kg: 0.0, face: 0.0, fuel: 0.0, ignite: 0.0, burn: 0.0, hc: 0, o2: 0.0, co2: 0.0, soot: 0.0, o2_min: 0.0, phi: 0.0, krc: 0.0, chi_r: 0.0 },
    Mat { name: "stone", rho: 2700.0, cp: 790.0, k: 2.8, skin_kg: 0.0, face: 1.0, fuel: 0.0, ignite: 0.0, burn: 0.0, hc: 0, o2: 0.0, co2: 0.0, soot: 0.0, o2_min: 0.0, phi: 0.0, krc: 0.0, chi_r: 0.0 },
    // planks: 500 kg/m3 dry, 80 % of it burns (the rest char); 11 g/m2/s burning (a cone at 25-50 kW/m2);
    // LIFT for 12.7 mm plywood (Quintiere and Harkleroad 1984): ignition 390 C, phi 12.9 kW2/m3, krc 0.54;
    // radiant share 0.3; 15 MJ/kg; 13.1 MJ a kg of oxygen (Huggett) gives 1.145; CO2 1.33, soot 0.015 (SFPE tables for wood)
    Mat { name: "wood", rho: 500.0, cp: 1700.0, k: 0.12, skin_kg: 0.0, face: 1.0, fuel: 400.0, ignite: 663.15, burn: 0.011, hc: 1875, o2: 1.145, co2: 1.33, soot: 0.015, o2_min: 0.15, phi: 12.9e6, krc: 0.54e6, chi_r: 0.3 },
    // a 2 kg iron pot with 0.3 m2 losing heat over its five open faces
    Mat { name: "pot", rho: 7870.0, cp: 450.0, k: 80.0, skin_kg: 2.0, face: 0.06, fuel: 0.0, ignite: 0.0, burn: 0.0, hc: 0, o2: 0.0, co2: 0.0, soot: 0.0, o2_min: 0.0, phi: 0.0, krc: 0.0, chi_r: 0.0 },
];

/// kg in a liquid unit, water's heat capacities and latent heat of vaporisation in mJ a unit.
pub const UNIT_KG: f64 = 0.244_140_625;
pub const C_W: f64 = 4186.0;
pub const LV_UNIT: i64 = 551_025_391;
pub const T_BOIL: f64 = 373.15;
/// Convection between a face and its gas cell, W/m2/K (free convection is 5 to 25).
pub const H_C: f64 = 10.0;
const SIGMA: f64 = 5.670_374e-8;
/// The least burning rate a flame stands at, kg/m2/s (Rasbash's critical mass flux for wood, 2.5 g/m2/s):
/// with less oxygen than that rate needs, the flame goes out.
/// shortcut: one value for every fuel; it belongs in the fuel's row once there is a second fuel.
pub const BURN_MIN: f64 = 0.0025;
/// The radiation pass runs once a second (the water-phases spike's heat step).
pub const RAD_EVERY: u64 = 20;
/// Steps a block stays after it joined or last took radiation: three radiation passes.
pub const HOLD: u16 = 60;

#[derive(Clone)]
pub struct Block {
    pub p: [i32; 3],
    pub mat: u8,
    pub skin_kg: f64,
    pub core_kg: f64,
    pub skin_h: i64,
    pub core_h: i64,
    pub fuel: i64,
    pub fuel0: i64,
    /// Liquid units in the skin and the core (the core's never move: shortcut, see README).
    pub water: i64,
    pub water_core: i64,
    pub water0: i64,
    /// Heat over the boiling point waiting to boil a whole unit, mJ.
    pub credit: i64,
    pub burning: bool,
    /// A source's watts (a fire under a pot), booked as supplied.
    pub watts: f64,
    /// When it first caught, s.
    pub caught: Option<f64>,
    /// The share of its open faces the flame covers (0..1): its burning area grows by the spread rate.
    pub front: f64,
    /// Radiant heat released since the last radiation pass, mJ.
    pub rad: i64,
    /// Steps it stays in the store though at rest: heat arriving in small parts (radiation, a slow warm-up) is
    /// not handed to the coarse air between them.
    pub hold: u16,
}

impl Block {
    pub fn m(&self) -> &'static Mat {
        &MATS[self.mat as usize]
    }
    pub fn c_skin(&self) -> f64 {
        (self.skin_kg * self.m().cp + self.water as f64 * UNIT_KG * C_W) * 1000.0
    }
    pub fn c_core(&self) -> f64 {
        (self.core_kg * self.m().cp + self.water_core as f64 * UNIT_KG * C_W) * 1000.0
    }
    pub fn t_skin(&self) -> f64 {
        self.skin_h as f64 / self.c_skin()
    }
    pub fn t_core(&self) -> f64 {
        if self.core_kg > 0.0 { self.core_h as f64 / self.c_core() } else { self.t_skin() }
    }
}

pub struct Sim {
    pub g: Ground,
    pub gas: Gas,
    pub blocks: Vec<Block>,
    pub at: FxMap<u64, u32>,
    /// Liquid units a block holds when it joins (from the liquid ledger).
    pub wet: FxMap<u64, i64>,
    /// Energy books, mJ: heat that came from outside (sources, a test's match), the latent heat the boiled
    /// units carry, and the rest heat blocks brought in when they joined (negative) and took back.
    pub supplied: i64,
    pub latent: i64,
    pub rest: i64,
    /// Mass books, units: fuel and water blocks brought in when they joined, fuel burnt, units boiled.
    pub fuel_in: i64,
    pub water_in: i64,
    pub burnt: i64,
    pub boiled: i64,
    /// Gas a test released from nowhere (a vent), units.
    pub released: i64,
    /// Vents: (block, species, units a step, steps left).
    pub vents: Vec<([i32; 3], usize, i64, u64)>,
    /// Heaters: (block, watts) into the gas cell there, booked as supplied.
    pub heaters: Vec<([i32; 3], f64)>,
    pub t: f64,
    start: Option<Ledger>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ledger {
    pub species: [i64; NS],
    pub mass: i64,
    pub water: i64,
    pub energy: i64,
}

impl Sim {
    pub fn new(g: Ground, t_air: f64) -> Sim {
        let gas = Gas::new(Coarse::standard(128.0 * 128.0 * 30.0, 15.0, t_air), &g);
        Sim {
            g,
            gas,
            blocks: Vec::new(),
            at: FxMap::default(),
            wet: FxMap::default(),
            supplied: 0,
            latent: 0,
            rest: 0,
            fuel_in: 0,
            water_in: 0,
            burnt: 0,
            boiled: 0,
            released: 0,
            vents: Vec::new(),
            heaters: Vec::new(),
            t: 0.0,
            start: None,
        }
    }

    pub fn block_at(&self, p: [i32; 3]) -> Option<&Block> {
        self.at.get(&key(p)).map(|&i| &self.blocks[i as usize])
    }

    /// The solid block at `p` joins the heat store at the coarse air's temperature (its rest).
    pub fn join(&mut self, p: [i32; 3]) -> Option<u32> {
        if let Some(&i) = self.at.get(&key(p)) {
            return Some(i);
        }
        let mat = self.g.block(p);
        if mat == AIR_BLOCK || !self.g.near(p) {
            return None;
        }
        let m = &MATS[mat as usize];
        let open = (0..6).filter(|&d| self.g.block(step_to(p, d)) == AIR_BLOCK).count().max(1) as f64;
        let skin_kg = if m.skin_kg > 0.0 { m.skin_kg } else { m.rho * (m.k / (m.rho * m.cp) * 600.0).sqrt() * open };
        let core_kg = if m.skin_kg > 0.0 { 0.0 } else { m.rho - skin_kg };
        let water_all = self.wet.get(&key(p)).copied().unwrap_or(0);
        let water = if core_kg > 0.0 { (water_all as f64 * skin_kg / m.rho).round() as i64 } else { water_all };
        let fuel = (m.fuel * MU_PER_KG as f64) as i64;
        let t = self.gas.coarse.temp();
        let mut b = Block {
            p,
            mat,
            skin_kg,
            core_kg,
            skin_h: 0,
            core_h: 0,
            fuel,
            fuel0: fuel,
            water,
            water_core: water_all - water,
            water0: water,
            credit: 0,
            burning: false,
            watts: 0.0,
            caught: None,
            front: 0.0,
            rad: 0,
            hold: HOLD,
        };
        b.skin_h = (b.c_skin() * t) as i64;
        b.core_h = (b.c_core() * t) as i64;
        self.rest -= b.skin_h + b.core_h + fuel * m.hc;
        self.fuel_in += fuel;
        self.water_in += water_all;
        let n = self.blocks.len() as u32;
        self.blocks.push(b);
        self.at.insert(key(p), n);
        Some(n)
    }

    /// A match: put the block's skin at `t` K, the heat booked as supplied.
    pub fn heat_to(&mut self, p: [i32; 3], t: f64) {
        let i = self.join(p).unwrap() as usize;
        let b = &mut self.blocks[i];
        let h = (b.c_skin() * t) as i64;
        self.supplied += h - b.skin_h;
        b.skin_h = h;
    }

    pub fn ledger(&self) -> Ledger {
        let mut species = [0; NS];
        for s in 0..NS {
            species[s] = self.gas.total(s) - self.gas.made[s];
        }
        let fuel: i64 = self.blocks.iter().map(|b| b.fuel).sum();
        let water: i64 = self.blocks.iter().map(|b| b.water + b.water_core).sum();
        let gas_mass: i64 = (0..NS).map(|s| self.gas.total(s)).sum();
        let heat: i64 = self.blocks.iter().map(|b| b.skin_h + b.core_h + b.credit + b.rad + b.fuel * b.m().hc).sum();
        Ledger {
            species,
            mass: gas_mass - self.released + fuel - self.fuel_in + (water - self.water_in) * LIQ_UNIT_MU,
            water: water + self.boiled - self.water_in,
            energy: self.gas.energy() + heat + self.latent + self.rest - self.supplied,
        }
    }

    /// Every ledger against the first call's: species (net of what sources booked), mass, water, energy.
    pub fn check(&mut self) {
        let now = self.ledger();
        match self.start {
            None => self.start = Some(now),
            Some(s) => {
                assert_eq!(s, now, "ledger moved at t = {} s", self.t);
                let made: i64 = self.gas.made.iter().sum();
                assert_eq!(made, self.burnt + self.boiled * LIQ_UNIT_MU + self.released, "the books of the reactions");
            }
        }
    }

    pub fn step(&mut self) {
        self.step_timed();
    }

    /// One step; returns the ms of its sources and heat store (with fire), and of its gas.
    pub fn step_timed(&mut self) -> (f64, f64) {
        let t0 = std::time::Instant::now();
        for v in 0..self.vents.len() {
            let (p, s, u, left) = self.vents[v];
            if left > 0 {
                self.release_units(p, s, u);
                self.vents[v].3 -= 1;
            }
        }
        for h in 0..self.heaters.len() {
            let (p, w) = self.heaters[h];
            let i = self.gas.activate(&self.g, p).unwrap() as usize;
            let e = (w * DT * 1000.0) as i64;
            self.gas.cells[i].e += e;
            self.supplied += e;
        }
        self.couple();
        for h in 0..self.heaters.len() {
            let i = self.gas.cell_at(self.heaters[h].0).unwrap() as usize;
            self.gas.cells[i].pin = true;
        }
        let t1 = std::time::Instant::now();
        self.gas.step(&self.g);
        self.t += DT;
        let t2 = std::time::Instant::now();
        ((t1 - t0).as_secs_f64() * 1000.0, (t2 - t1).as_secs_f64() * 1000.0)
    }

    fn couple(&mut self) {
        for c in &mut self.gas.cells {
            c.pin = false;
        }
        let tc = self.gas.coarse.temp();
        let mut join = Vec::new();
        for c in &self.gas.cells {
            if (temp(&c.m, c.e) - tc).abs() > 1.0 {
                for d in 0..6 {
                    let q = step_to(c.p, d);
                    if c.nb[d] == SOLID && self.g.near(q) && !self.at.contains_key(&key(q)) {
                        join.push(q);
                    }
                }
            }
        }
        for q in join {
            self.join(q);
        }
        for bi in 0..self.blocks.len() {
            self.block_step(bi, tc);
        }
        self.contact();
        if self.gas.steps % RAD_EVERY == 0 {
            self.radiate();
        }
        let mut bi = 0;
        while bi < self.blocks.len() {
            if self.at_rest(bi, tc) {
                self.leave(bi, tc);
            } else {
                bi += 1;
            }
        }
    }

    /// A face-neighbour of a fully burning fuel block catches at the seam (its front starts at 0) once its skin is dry.
    fn contact(&mut self) {
        let mut catch = Vec::new();
        for b in &self.blocks {
            if b.burning && b.front >= 1.0 {
                for d in 0..6 {
                    let q = step_to(b.p, d);
                    if MATS[self.g.block(q) as usize].fuel > 0.0 {
                        catch.push(q);
                    }
                }
            }
        }
        for q in catch {
            let Some(j) = self.join(q) else { continue };
            let t = self.t;
            let b = &mut self.blocks[j as usize];
            if !b.burning && b.fuel > 0 && b.water == 0 {
                b.burning = true;
                b.caught.get_or_insert(t);
            }
        }
    }

    /// Once a second: each flame radiates what it gathered from a point at half its height (Heskestad's flame
    /// height, leaned by the wind as the AGA correlation leans it) onto the open faces of solid blocks within 3 m,
    /// by cos / (4 pi r^2); what no face takes goes to the coarse air.
    /// shortcut: no shadowing (a wall between does not block it); upgrade with a ray march over the blocks.
    fn radiate(&mut self) {
        let secs = RAD_EVERY as f64 * DT;
        let wind = self.gas.wind;
        let u = (wind[0] * wind[0] + wind[1] * wind[1]).sqrt();
        let mut give: Vec<([i32; 3], i64)> = Vec::new();
        for bi in 0..self.blocks.len() {
            let b = &self.blocks[bi];
            if b.rad <= 0 {
                continue;
            }
            let m = b.m();
            let q_kw = b.rad as f64 / 1e6 / secs / m.chi_r;
            let lf = (0.235 * q_kw.powf(0.4) - 1.02).max(0.5);
            let uc = (G * m.burn / 1.2).cbrt();
            let cos = if u <= uc { 1.0 } else { (uc / u).sqrt() };
            let lean = 0.5 * lf * (1.0 - cos * cos).sqrt() / cos;
            let (wx, wy) = if u > 0.0 { (wind[0] / u, wind[1] / u) } else { (0.0, 0.0) };
            let src = [b.p[0] as f64 + 0.5 + wx * lean, b.p[1] as f64 + 0.5 + wy * lean, b.p[2] as f64 + 1.0 + 0.5 * lf];
            let mut shares: Vec<([i32; 3], f64)> = Vec::new();
            for dk in -3..=3 {
                for dj in -3..=3 {
                    for di in -3..=3 {
                        let q = [b.p[0] + di, b.p[1] + dj, b.p[2] + dk];
                        if q == b.p || !self.g.near(q) || self.g.block(q) == AIR_BLOCK {
                            continue;
                        }
                        let face = MATS[self.g.block(q) as usize].face;
                        for d in 0..6 {
                            if self.g.block(step_to(q, d)) != AIR_BLOCK {
                                continue;
                            }
                            let n = DIRS[d];
                            let c = [0, 1, 2].map(|a| q[a] as f64 + 0.5 + 0.5 * n[a] as f64);
                            let v = [0, 1, 2].map(|a| src[a] - c[a]);
                            let dot: f64 = (0..3).map(|a| v[a] * n[a] as f64).sum();
                            if dot <= 0.0 {
                                continue;
                            }
                            let r2: f64 = v.iter().map(|x| x * x).sum();
                            shares.push((q, dot / r2.sqrt() / (4.0 * std::f64::consts::PI * r2) * face));
                        }
                    }
                }
            }
            let sum: f64 = shares.iter().map(|s| s.1).sum();
            let norm = if sum > 1.0 { 1.0 / sum } else { 1.0 };
            let rad = b.rad;
            let mut given = 0;
            for (q, f) in shares {
                let e = (rad as f64 * f * norm) as i64;
                given += e;
                give.push((q, e));
            }
            self.gas.coarse.e += rad - given;
            self.blocks[bi].rad = 0;
        }
        for (q, e) in give {
            let j = self.join(q).unwrap() as usize;
            self.blocks[j].skin_h += e;
            self.blocks[j].hold = HOLD;
        }
    }

    fn at_rest(&self, bi: usize, tc: f64) -> bool {
        let b = &self.blocks[bi];
        !b.burning
            && b.watts == 0.0
            && b.fuel == b.fuel0
            && b.water == b.water0
            && b.credit == 0
            && b.rad == 0
            && b.hold == 0
            && (b.t_skin() - tc).abs() < 0.5
            && (b.t_core() - tc).abs() < 0.5
    }

    /// The block gives its heat over its rest to the coarse air and leaves the store.
    fn leave(&mut self, bi: usize, tc: f64) {
        let b = self.blocks.swap_remove(bi);
        self.at.remove(&key(b.p));
        if bi < self.blocks.len() {
            self.at.insert(key(self.blocks[bi].p), bi as u32);
        }
        let rest_h = (b.c_skin() * tc) as i64 + (b.c_core() * tc) as i64;
        self.gas.coarse.e += b.skin_h + b.core_h - rest_h;
        self.rest += rest_h + b.fuel * b.m().hc;
        self.fuel_in -= b.fuel;
        self.water_in -= b.water + b.water_core;
    }

    fn block_step(&mut self, bi: usize, tc: f64) {
        let p = self.blocks[bi].p;
        let m = self.blocks[bi].m();
        self.blocks[bi].hold = self.blocks[bi].hold.saturating_sub(1);
        let hot = {
            let b = &self.blocks[bi];
            b.burning || b.watts > 0.0 || (b.t_skin() - tc).abs() > 1.0
        };
        // the gas cells on its open faces (+k first: steam and flame go up)
        let mut cells: Vec<u32> = Vec::with_capacity(6);
        let mut open = 0.0;
        for d in [4, 0, 1, 2, 3, 5] {
            let q = step_to(p, d);
            if self.g.block(q) != AIR_BLOCK {
                continue;
            }
            open += 1.0;
            let i = if hot { self.gas.activate(&self.g, q) } else { self.gas.cell_at(q) };
            if let Some(i) = i {
                self.gas.cells[i as usize].pin |= hot;
                cells.push(i);
            }
        }
        let b = &mut self.blocks[bi];
        if b.watts > 0.0 {
            let w = (b.watts * DT * 1000.0) as i64;
            b.skin_h += w;
            self.supplied += w;
        }
        // the skin and the gas: convection and the soot's radiation
        for &i in &cells {
            let c = &mut self.gas.cells[i as usize];
            let tg = temp(&c.m, c.e);
            let ts = b.skin_h as f64 / b.c_skin();
            let fv = c.m[SOOT] as f64 / MU_PER_KG as f64 / 1800.0;
            let eps = 1.0 - (-1862.0 * fv * tg).exp();
            let q = (H_C * (tg - ts) + eps * SIGMA * (tg.powi(4) - ts.powi(4))) * m.face;
            let cg = heat_cap(&c.m) * 1000.0;
            let cs = b.c_skin();
            let cap = (tg - ts).abs() * cg * cs / (cg + cs) / 7.0;
            let de = (q * DT * 1000.0).clamp(-cap, cap) as i64;
            c.e -= de;
            b.skin_h += de;
        }
        // the skin and the core: conduction over half a block
        if b.core_kg > 0.0 {
            let (ts, tk) = (b.t_skin(), b.t_core());
            let (cs, ck) = (b.c_skin(), b.c_core());
            let cap = (ts - tk).abs() * cs * ck / (cs + ck) / 2.0;
            let de = (m.k * open * m.face / 0.5 * (ts - tk) * DT * 1000.0).clamp(-cap, cap) as i64;
            b.skin_h -= de;
            b.core_h += de;
        }
        // water in the skin holds it at the boiling point; whole units boil off into the gas over it
        if b.water > 0 && !cells.is_empty() {
            let hb = (b.c_skin() * T_BOIL) as i64;
            if b.skin_h > hb {
                b.credit += b.skin_h - hb;
                b.skin_h = hb;
            }
            let s_l = (UNIT_KG * C_W * T_BOIL * 1000.0) as i64;
            let s_v = (UNIT_KG * SPECIES[H2O].cp * T_BOIL * 1000.0) as i64;
            while b.credit >= LV_UNIT && b.water > 0 {
                b.credit -= LV_UNIT;
                b.water -= 1;
                b.skin_h -= s_l;
                self.gas.add(cells[0], H2O, LIQ_UNIT_MU);
                self.gas.cells[cells[0] as usize].e += s_v;
                self.latent += LV_UNIT + s_l - s_v;
                self.boiled += 1;
            }
            if b.water == 0 {
                b.skin_h += b.credit;
                b.credit = 0;
            }
        }
        // fire: the reaction at its row's rate over the share of its faces the flame covers, where oxygen stands by
        if m.fuel > 0.0 && b.fuel > 0 && !cells.is_empty() {
            if b.water == 0 && b.t_skin() >= m.ignite && (!b.burning || b.front < 1.0) {
                // the whole skin at its ignition point
                b.front = 1.0;
                b.burning = true;
                b.caught.get_or_insert(self.t);
            }
            if b.burning {
                if b.front < 1.0 {
                    // the flame heats the skin ahead of it to ignition (LIFT): a cold or wet skin slows it
                    let d = (m.ignite - b.t_skin()).max(1.0);
                    b.front = (b.front + m.phi / (m.krc * d * d) * DT).min(1.0);
                }
                let o2: i64 = cells.iter().map(|&i| self.gas.cells[i as usize].m[O2]).sum();
                let all: f64 = cells.iter().map(|&i| mass(&self.gas.cells[i as usize].m)).sum();
                let y = o2 as f64 / MU_PER_KG as f64 / all;
                let lim = ((y - m.o2_min) / (AIR[O2] - m.o2_min)).clamp(0.0, 1.0);
                let rate = m.burn * lim;
                let area = cells.len() as f64 * m.face * b.front;
                let mut dm = (rate * area * DT * MU_PER_KG as f64) as i64;
                dm = dm.min(b.fuel).min((o2 as f64 * 0.5 / m.o2) as i64);
                if rate < BURN_MIN {
                    b.burning = false;
                } else if dm > 0 {
                    let used = (dm as f64 * m.o2) as i64;
                    let co2 = (dm as f64 * m.co2) as i64;
                    let soot = (dm as f64 * m.soot) as i64;
                    let h2o = dm + used - co2 - soot;
                    // each cell gives its share of the oxygen; what rounding leaves, the richest one
                    let mut left = used;
                    let mut richest = cells[0];
                    for &i in &cells {
                        let o = self.gas.cells[i as usize].m[O2];
                        if o > self.gas.cells[richest as usize].m[O2] {
                            richest = i;
                        }
                        let share = (used as i128 * o as i128 / o2 as i128) as i64;
                        left -= share;
                        self.gas.add(i, O2, -share);
                    }
                    self.gas.add(richest, O2, -left);
                    let k = cells.len() as i64;
                    let q = dm * m.hc;
                    let radiant = (q as f64 * m.chi_r) as i64;
                    b.rad += radiant;
                    let give = ((b.c_skin() * m.ignite) as i64 - b.skin_h).clamp(0, q - radiant);
                    b.skin_h += give;
                    for (s, x) in [(CO2, co2), (H2O, h2o), (SOOT, soot)] {
                        for (n, &i) in cells.iter().enumerate() {
                            self.gas.add(i, s, x / k + if n == 0 { x % k } else { 0 });
                        }
                    }
                    let heat = q - radiant - give;
                    for (n, &i) in cells.iter().enumerate() {
                        self.gas.cells[i as usize].e += heat / k + if n == 0 { heat % k } else { 0 };
                    }
                    b.fuel -= dm;
                    self.burnt += dm;
                    if b.fuel == 0 {
                        b.burning = false;
                    }
                }
            }
        }
    }
}
