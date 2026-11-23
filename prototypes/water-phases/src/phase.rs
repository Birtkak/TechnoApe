//! The rule (README "The rule"): water's three phases as counted units, moved by heat.
//!
//! A node holds `i` units of ice and `w` of water (U a block, as `liquid::U`) and an enthalpy `h` in millijoules,
//! measured from "every unit at 0 C in the phase it is in". Heat flows are added to `h` in whole millijoules and
//! booked to where they came from, so energy is a ledger too. `settle` then moves whole units between ice and
//! water by the latent heat of fusion (the enthalpy method); the vapour exchange moves whole units to and from
//! the air cell by the latent heat of vaporisation, at a rate from the vapour pressures. Nothing switches: every
//! change is a heat flow (W) over a latent heat (J a unit).
use std::f64::consts::PI;
use std::sync::OnceLock;

/// Units of water in a block (`crates/engine/src/liquid.rs`, `U`).
pub const U: i64 = 1 << 12;
/// The game day (`sky::DAY`), s.
pub const DAY: f64 = 1800.0;
/// The year under real gravity, game days (`docs/decisions/bodies-and-orbits.md`: the bake's climate runs 334 days).
pub const YEAR_DAYS: f64 = 334.0;
/// The server's tick (`net::TICK_HZ`); water steps at `phys::DT` = 1/60 s, up to three a tick.
pub const TICK_HZ: f64 = 20.0;
/// kg in a block of water; shortcut: a block of ice is the same U units and mass (real ice is 917 kg/m3, so a
/// block of water would freeze to 1.09 blocks); upgrade when ice must float by its own buoyancy.
pub const RHO: f64 = 1000.0;
/// kg in a unit.
pub const MU: f64 = RHO / U as f64;
const R: f64 = 8.314462618;
const M_W: f64 = 0.018015;
/// Water's triple point: every vapour pressure curve below passes through it.
const P_T: f64 = 611.657;
const T_T: f64 = 273.16;
pub const T0K: f64 = 273.15;
pub const ATM: f64 = 101_325.0;
/// Latent heat of fusion, J/kg (at 0 C).
pub const L_F: f64 = 333_550.0;
const C_W: f64 = 4186.0;
const C_I: f64 = 2050.0;
/// Ice's thermal conductivity, W/m/K (at 0 C).
pub const K_ICE: f64 = 2.22;

/// Per-unit constants in millijoules, worked out once.
pub struct K {
    /// L_v M / R and L_s M / R, K: the slopes of the two Clausius-Clapeyron curves.
    pub lv_r: f64,
    pub ls_r: f64,
    /// L_v, J/kg: fitted so the liquid curve passes the triple point and 100 C at 1 atm. shortcut: one L_v at
    /// every temperature (real 2.50 MJ/kg at 0 C, 2.26 at 100 C), so the curve is within 4 % of the real one.
    pub l_v: f64,
    pub lfu: i64,
    pub lvu: i64,
    /// Sublimation, exactly lfu + lvu, so a unit's way round the three phases closes to the millijoule.
    pub lsu: i64,
    pub cwu: f64,
    pub ciu: f64,
}

pub fn k() -> &'static K {
    static C: OnceLock<K> = OnceLock::new();
    C.get_or_init(|| {
        let lv_r = (ATM / P_T).ln() / (1.0 / T_T - 1.0 / 373.15);
        let l_v = lv_r * R / M_W;
        let ls_r = lv_r + L_F * M_W / R;
        let lfu = (L_F * MU * 1000.0).round() as i64;
        let lvu = (l_v * MU * 1000.0).round() as i64;
        K { lv_r, ls_r, l_v, lfu, lvu, lsu: lfu + lvu, cwu: C_W * MU * 1000.0, ciu: C_I * MU * 1000.0 }
    })
}

/// Saturation vapour pressure over water or ice at `t` C, Pa (Clausius-Clapeyron through the triple point).
pub fn p_sat(t: f64, ice: bool) -> f64 {
    let a = if ice { k().ls_r } else { k().lv_r };
    P_T * (-a * (1.0 / (t + T0K) - 1.0 / T_T)).exp()
}

/// Where water leaves for vapour at total pressure `p` Pa, C: the boiling point over the triple point's
/// pressure, the sublimation point under it (no liquid there), absolute zero in no air at all.
pub fn t_transition(p: f64) -> f64 {
    if p <= 0.0 {
        return -T0K;
    }
    let a = if p >= P_T { k().lv_r } else { k().ls_r };
    1.0 / (1.0 / T_T - (p / P_T).ln() / a) - T0K
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Node {
    pub i: i64,
    pub w: i64,
    /// mJ over "all at 0 C in its phase"; |h| < lfu while both phases are there (then it is at 0 C).
    pub h: i64,
    /// Latent heat paid toward the next unit to or from the vapour, mJ.
    pub vc: i64,
}

impl Node {
    pub fn water(w: i64, t: f64) -> Node {
        Node { w, h: (w as f64 * k().cwu * t).round() as i64, ..Node::default() }
    }
    pub fn ice(i: i64, t: f64) -> Node {
        Node { i, h: (i as f64 * k().ciu * t).round() as i64, ..Node::default() }
    }
    pub fn m(&self) -> i64 {
        self.i + self.w
    }
    pub fn temp(&self) -> f64 {
        if self.h < 0 && self.w == 0 && self.i > 0 {
            self.h as f64 / (self.i as f64 * k().ciu)
        } else if self.h > 0 && self.i == 0 && self.w > 0 {
            self.h as f64 / (self.w as f64 * k().cwu)
        } else {
            0.0
        }
    }
    /// mJ per K.
    pub fn cap(&self) -> f64 {
        self.i as f64 * k().ciu + self.w as f64 * k().cwu
    }
    /// Melt or freeze whole units by the heat the node holds past 0 C.
    pub fn settle(&mut self) {
        let l = k().lfu;
        if self.h >= l && self.i > 0 {
            let n = (self.h / l).min(self.i);
            self.i -= n;
            self.w += n;
            self.h -= n * l;
        }
        if self.h <= -l && self.w > 0 {
            let n = (-self.h / l).min(self.w);
            self.w -= n;
            self.i += n;
            self.h += n * l;
        }
    }
    /// The node's share of the energy ledger, mJ, from ice at 0 C.
    pub fn energy(&self) -> i64 {
        self.h + self.vc + k().lfu * self.w
    }
}

/// The air over a surface: an air-grid cell, a reservoir of heat (`heat`: what it was given, mJ) and of
/// vapour (`v` units).
#[derive(Clone, Debug)]
pub struct Air {
    pub t: f64,
    /// Total pressure, Pa.
    pub p: f64,
    /// m3: the air grid's surface layer of one column is 128 x 128 x 30 m.
    pub vol: f64,
    pub v: i64,
    pub heat: i64,
    /// Convective and linearised radiative heat transfer, W/m2/K.
    pub h_c: f64,
    pub h_r: f64,
    /// False: no vapour exchange (a test that isolates conduction).
    pub vapour: bool,
}

pub const CELL: f64 = 128.0 * 128.0 * 30.0;

impl Air {
    pub fn new(t: f64, p: f64, h_c: f64, h_r: f64) -> Air {
        Air { t, p, vol: CELL, v: 0, heat: 0, h_c, h_r, vapour: true }
    }
    /// With vapour at partial pressure `pv` Pa.
    pub fn at_pv(mut self, pv: f64) -> Air {
        self.v = (pv * self.vol * M_W / (R * (self.t + T0K)) / MU).round() as i64;
        self
    }
    pub fn pv(&self) -> f64 {
        self.v as f64 * MU / M_W * R * (self.t + T0K) / self.vol
    }
    pub fn h(&self) -> f64 {
        if self.p > 0.0 { self.h_c + self.h_r } else { self.h_r }
    }
}

/// The vapour flux off a surface at `t_s` C, kg/m2/s (negative: it deposits), and whether it boils.
/// Boiling (its vapour pressure at or over the air's whole pressure; always in no air): the vapour pushes the
/// air aside and leaves at the kinetic limit (Hertz-Knudsen, sticking 1). Otherwise it diffuses through the
/// air: the Lewis analogy from the convective transfer, 0.622 h_c (p_sat - p_v) / (c_p p).
/// shortcut: c_p and the molar mass ratio of Earth's air on every body (Mars's CO2 is 0.41, not 0.622).
pub fn vapour_flux(t_s: f64, ice: bool, air: &Air) -> (f64, bool) {
    let ps = p_sat(t_s, ice);
    let pv = air.pv();
    if ps >= air.p {
        ((ps - pv).max(0.0) * (M_W / (2.0 * PI * R * (t_s + T0K))).sqrt(), true)
    } else {
        (0.622 * air.h_c / (1005.0 * air.p) * (ps - pv), false)
    }
}

/// Heat `q_w` watts for `dt` s out of node `n` into `to`, whole millijoules.
pub fn flow(n: &mut Node, to: &mut i64, q_w: f64, dt: f64) {
    let q = (q_w * dt * 1000.0).round() as i64;
    n.h -= q;
    *to += q;
}

/// The vapour exchange of a surface of `area` m2 at `t_s` C. Boiling is limited by the heat the node holds
/// over its transition point: the vapour can only take what is there, so a pot stays at its boiling point.
pub fn exchange(n: &mut Node, t_s: f64, ice: bool, area: f64, dt: f64, air: &mut Air) {
    if !air.vapour || n.m() == 0 {
        return;
    }
    let c = k();
    let (j, boils) = vapour_flux(t_s, ice, air);
    let mut q = j * area * dt * if ice { c.l_v + L_F } else { c.l_v } * 1000.0;
    if boils {
        q = q.min(((n.temp() - t_transition(air.p)) * n.cap()).max(0.0));
    }
    let q = q.round() as i64;
    n.h -= q;
    n.vc += q;
    loop {
        // an ice surface gives ice (sea ice sublimates, the water under it stays), a water surface water
        let from_ice = if ice { n.i > 0 } else { n.w == 0 };
        let l = if from_ice { c.lsu } else { c.lvu };
        if n.vc >= l && n.m() > 0 {
            // the unit carries its sensible heat to the air with it
            let t = n.temp();
            let s = (t * if from_ice { c.ciu } else { c.cwu }).round() as i64;
            if from_ice { n.i -= 1 } else { n.w -= 1 }
            n.h -= s;
            air.heat += s;
            n.vc -= l;
            air.v += 1;
            continue;
        }
        let l = if ice { c.lsu } else { c.lvu };
        if n.vc <= -l && air.v > 0 {
            // a deposited unit arrives at 0 C (shortcut: its sensible heat from the air is not followed)
            if ice { n.i += 1 } else { n.w += 1 }
            n.vc += l;
            air.v -= 1;
            continue;
        }
        break;
    }
}

/// An ice block standing on the ground: five faces to the air (the ground under it is not followed), the
/// shape staying a cube as it shrinks, its melt running off into `pool` (the gap under it) at 0 C.
#[derive(Clone, Debug)]
pub struct Block {
    pub n: Node,
    pub pool: i64,
}

impl Block {
    pub fn area(&self) -> f64 {
        // the ice left, counting the part of the next unit already paid for (else the last units melt too fast)
        let left = self.n.i as f64 - self.n.h.max(0) as f64 / k().lfu as f64;
        // shortcut: the last half unit (a 5 cm cube) melts at that cube's area, not shrinking (else it never ends):
        // the cube's law gives the last 5 cm 5 % of the time, this a third of it, so a block melts 3.3 % early
        let s = (left.max(0.5) / U as f64).cbrt();
        5.0 * s * s
    }
    /// One step; `q_in` W from a source (`src` books what it gave).
    pub fn step(&mut self, air: &mut Air, src: &mut i64, q_in: f64, dt: f64) {
        if self.n.i == 0 {
            return;
        }
        let a = self.area();
        let t = self.n.temp();
        let q = air.h() * a * (t - air.t);
        flow(&mut self.n, &mut air.heat, q, dt);
        flow(&mut self.n, src, -q_in, dt);
        exchange(&mut self.n, t, true, a, dt, air);
        self.n.settle();
        if self.n.w > 0 {
            // the melt leaves at 0 C; once the last ice is gone the water's warmth goes with it (to the air)
            self.pool += self.n.w;
            self.n.w = 0;
            if self.n.i == 0 {
                air.heat += self.n.h;
                self.n.h = 0;
            }
        }
    }
}

/// A pot: water over a fire, `loss` m2 of pot losing heat to the air, `top` m2 of open water.
pub fn step_pot(n: &mut Node, air: &mut Air, src: &mut i64, fire: f64, loss: f64, top: f64, dt: f64) {
    let t = n.temp();
    let q = air.h() * loss * (t - air.t);
    flow(n, &mut air.heat, q, dt);
    flow(n, src, -fire, dt);
    // the surface as it is after this step's heat, so the boil takes this step's excess
    exchange(n, n.temp(), n.w == 0, top, dt, air);
    n.settle();
}

/// A stored column of 1 m2: water in the gap, ice as a layer on its top (thickness i / U blocks), `f_w` W/m2 of
/// heat from below (the sea's, or the ground's). The ice holds no heat (quasi-steady: a straight profile from
/// 0 C at its bottom to its surface; ice's Stefan number at 10 K is 0.06). Returns the heat up through the
/// surface, W/m2.
pub fn step_column(n: &mut Node, air: &mut Air, src: &mut i64, f_w: f64, dt: f64) -> f64 {
    let ha = air.h();
    let t = n.temp();
    let (q, t_s) = if n.i > 0 {
        let d = n.i as f64 / U as f64;
        if n.w > 0 && air.t > 0.0 {
            // warm air on ice over water: the top is at 0 C and melts
            (ha * -air.t, 0.0)
        } else {
            // ice over water at 0 C conducts through all of it; frozen solid, from its middle
            let (tb, r) = if n.w > 0 { (0.0, d / K_ICE) } else { (t, d / (2.0 * K_ICE)) };
            let q = (tb - air.t) / (r + 1.0 / ha);
            (q, air.t + q / ha)
        }
    } else {
        (ha * (t - air.t), t)
    };
    flow(n, &mut air.heat, q, dt);
    flow(n, src, -f_w, dt);
    exchange(n, t_s, n.i > 0, 1.0, dt, air);
    n.settle();
    q
}

/// The formula for untouched water: the sea ice's thickness at rest, units, under air at `t` C (the year's
/// mean as the bake counts it), the sea giving `f_w` W/m2 from below, a sea `depth` units deep. At rest the
/// heat up through the ice, (0 - t) / (d / k + 1 / h), equals the sea's from below.
pub fn rest_ice(t: f64, f_w: f64, ha: f64, depth: i64) -> i64 {
    if t >= 0.0 {
        return 0;
    }
    if f_w <= 0.0 {
        return depth;
    }
    ((K_ICE * (-t / f_w - 1.0 / ha)).max(0.0) * U as f64).round().min(depth as f64) as i64
}

/// The sea's heat from below that makes the bake's rule its rest state: a sheet of exactly one block where
/// the year's mean is `t_sheet` (`LifeParams::t_sea_ice`, -8 C).
pub fn f_for_sheet(t_sheet: f64, ha: f64) -> f64 {
    -t_sheet / (1.0 / K_ICE + 1.0 / ha)
}

/// Both ledgers, asserted every step: units (ice + water + vapour + melt pools) and energy (mJ, from ice at 0 C;
/// `srcs` is what the fires and the ground below took in, negative for what they gave).
pub struct Ledger {
    pub units: i64,
    pub energy: i64,
}

impl Ledger {
    pub fn of(nodes: &[&Node], pools: i64, airs: &[&Air], srcs: i64) -> Ledger {
        let c = k();
        let units = nodes.iter().map(|n| n.m()).sum::<i64>() + pools + airs.iter().map(|a| a.v).sum::<i64>();
        let energy = nodes.iter().map(|n| n.energy()).sum::<i64>() + c.lfu * pools + airs.iter().map(|a| c.lsu * a.v + a.heat).sum::<i64>() + srcs;
        Ledger { units, energy }
    }
    pub fn check(&self, now: Ledger, what: &str) {
        assert_eq!(now.units, self.units, "{what}: units");
        assert_eq!(now.energy, self.energy, "{what}: energy");
    }
}

// ---- the scenes, shared by the tests and `main` ----

pub struct Melt {
    pub seconds: f64,
    pub water: i64,
    pub vapour: i64,
}

/// An ice block at `t0` C in air at `t_air` C, its vapour pressure that of ice at 0 C (about 50 % humidity at
/// +10 C), until it has gone.
pub fn melt_block(t0: f64, t_air: f64, h_c: f64, h_r: f64, dt: f64) -> Melt {
    let mut air = Air::new(t_air, ATM, h_c, h_r).at_pv(p_sat(0.0, true));
    let v0 = air.v;
    let mut b = Block { n: Node::ice(U, t0), pool: 0 };
    let mut src = 0;
    let led = Ledger::of(&[&b.n], b.pool, &[&air], src);
    let mut t = 0.0;
    while b.n.i > 0 {
        b.step(&mut air, &mut src, 0.0, dt);
        t += dt;
        led.check(Ledger::of(&[&b.n], b.pool, &[&air], src), "melt");
        assert!(t < 1e9, "never melts");
    }
    Melt { seconds: t, water: b.pool, vapour: air.v - v0 }
}

/// The time a shrinking cube of side 1 takes to melt from 0 C: 3 rho L / (5 h dT) (dm/dt = -h dT 5 s^2 / L).
pub fn melt_analytic(t_air: f64, h: f64) -> f64 {
    3.0 * RHO * L_F / (5.0 * h * t_air)
}

pub struct Freeze {
    /// (thickness m, s, the heat up W/m2) as the ice reached each tenth of the pond.
    pub marks: Vec<(f64, f64, f64)>,
    pub solid: f64,
}

/// A pond `depth` units deep at `t0` C under air at `t_air`, no heat from the ground, until frozen solid.
pub fn freeze_pond(depth: i64, t0: f64, t_air: f64, ha: f64, dt: f64, vapour: bool) -> Freeze {
    let mut air = Air::new(t_air, ATM, ha - 4.5, 4.5).at_pv(p_sat(t_air, true));
    air.vapour = vapour;
    let mut n = Node::water(depth, t0);
    let mut src = 0;
    let led = Ledger::of(&[&n], 0, &[&air], src);
    let (mut t, mut marks, mut next) = (0.0, Vec::new(), 1);
    while n.w > 0 {
        let q = step_column(&mut n, &mut air, &mut src, 0.0, dt);
        t += dt;
        led.check(Ledger::of(&[&n], 0, &[&air], src), "pond");
        if n.i * 10 >= next * depth {
            marks.push((n.i as f64 / U as f64, t, q));
            next += 1;
        }
        assert!(t < 1e10, "never freezes");
    }
    Freeze { marks, solid: t }
}

/// Stefan's law with the air's resistance (quasi-steady, from water at 0 C): the time to grow `d` m of ice.
pub fn stefan(d: f64, t_air: f64, ha: f64) -> f64 {
    RHO * L_F / -t_air * (d * d / (2.0 * K_ICE) + d / ha)
}

pub struct Boil {
    pub to_boil: f64,
    pub dry: f64,
    pub plateau: f64,
    pub vapour: i64,
    pub max_t: f64,
}

/// A pot of `w` units at `t0` C over a fire of `fire` W in air at `t_air` C and `p` Pa, until dry.
pub fn boil_pot(w: i64, t0: f64, t_air: f64, p: f64, fire: f64, dt: f64) -> Boil {
    let mut air = Air::new(t_air, p, 5.0, 5.0);
    if p > 0.0 {
        air = air.at_pv((0.5 * p_sat(t_air, t_air < 0.0)).min(0.5 * p));
    }
    let v0 = air.v;
    let mut n = Node::water(w, t0);
    let mut src = 0;
    let led = Ledger::of(&[&n], 0, &[&air], src);
    let tb = t_transition(p);
    let (mut t, mut to_boil, mut max_t, mut plateau) = (0.0, f64::NAN, f64::MIN, 0.0);
    while n.m() > 0 {
        step_pot(&mut n, &mut air, &mut src, fire, 0.3, 0.05, dt);
        t += dt;
        led.check(Ledger::of(&[&n], 0, &[&air], src), "pot");
        let tt = n.temp();
        max_t = max_t.max(tt);
        if to_boil.is_nan() && tt >= tb - 0.5 {
            to_boil = t;
        }
        if n.m() == w / 2 {
            plateau = tt;
        }
        assert!(t < 1e8, "never dries");
    }
    Boil { to_boil, dry: t, plateau, vapour: air.v - v0, max_t }
}

pub struct Vacuum {
    pub max_water: i64,
    pub ice: i64,
    pub vapour: i64,
    pub temp: f64,
}

/// An ice block at `t0` C under air of `p` Pa at `t_air` C, given `q_in` W (sunlight, a fire), for `secs` s.
pub fn sublimate(t0: f64, t_air: f64, p: f64, q_in: f64, secs: f64, dt: f64) -> Vacuum {
    let mut air = Air::new(t_air, p, 1.0, 0.0);
    if p == 0.0 {
        air.vol = 1e15; // space: what leaves never comes back
    }
    let mut b = Block { n: Node::ice(U, t0), pool: 0 };
    let mut src = 0;
    let led = Ledger::of(&[&b.n], b.pool, &[&air], src);
    let (mut t, mut max_water) = (0.0, 0);
    while t < secs && b.n.i > 0 {
        b.step(&mut air, &mut src, q_in, dt);
        t += dt;
        led.check(Ledger::of(&[&b.n], b.pool, &[&air], src), "vacuum");
        max_water = max_water.max(b.pool + b.n.w);
    }
    Vacuum { max_water, ice: b.n.i, vapour: air.v, temp: b.n.temp() }
}

pub struct Sea {
    /// (air C, formula units, from the formula after the run, from open water after the run)
    pub rows: Vec<(f64, i64, i64, i64)>,
}

/// A stored stretch of `cols` sea columns `depth` units deep, the air from `t_warm` to `t_cold` C along it,
/// stepped `secs` s twice: laid at the formula, and from open water at 0 C.
pub fn sea_stretch(cols: usize, depth: i64, t_warm: f64, t_cold: f64, f_w: f64, ha: f64, secs: f64, dt: f64) -> Sea {
    let mut rows = Vec::new();
    for c in 0..cols {
        let ta = t_warm + (t_cold - t_warm) * c as f64 / (cols - 1) as f64;
        let rest = rest_ice(ta, f_w, ha, depth);
        let mut out = [0i64; 2];
        for (s, o) in out.iter_mut().enumerate() {
            // over the sea ice, the air grid owns the vapour: no exchange here (README, "not covered")
            let mut air = Air::new(ta, ATM, ha - 4.5, 4.5);
            air.vapour = false;
            let mut n = if s == 0 { Node { i: rest, w: depth - rest, ..Node::default() } } else { Node::water(depth, 0.0) };
            let mut src = 0;
            let led = Ledger::of(&[&n], 0, &[&air], src);
            let mut t = 0.0;
            while t < secs {
                step_column(&mut n, &mut air, &mut src, f_w, dt);
                t += dt;
                led.check(Ledger::of(&[&n], 0, &[&air], src), "sea");
            }
            *o = n.i;
        }
        rows.push((ta, rest, out[0], out[1]));
    }
    Sea { rows }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curves_pass_their_anchors() {
        assert!((t_transition(ATM) - 100.0).abs() < 1e-9);
        assert!((p_sat(0.01, false) - P_T).abs() < 1e-6 && (p_sat(0.01, true) - P_T).abs() < 1e-6);
        // the real curve: 2339 Pa at 20 C over water, 260 Pa at -10 C over ice
        assert!((p_sat(20.0, false) / 2339.0 - 1.0).abs() < 0.05, "{}", p_sat(20.0, false));
        assert!((p_sat(-10.0, true) / 259.9 - 1.0).abs() < 0.05, "{}", p_sat(-10.0, true));
        assert_eq!(k().lsu, k().lfu + k().lvu);
    }

    #[test]
    fn an_ice_block_in_warm_air_melts_to_exactly_its_units() {
        let m = melt_block(0.0, 10.0, 5.0, 5.0, 1.0);
        assert_eq!(m.water + m.vapour, U, "the ledger");
        assert_eq!(m.water, U, "air at the ice's own vapour pressure: all of it is water");
        // the cube's law, less the 3.3 % its last 5 cm lose to the half-unit floor (`Block::area`)
        let want = melt_analytic(10.0, 10.0) * (1.0 - 2.0 / 3.0 * (0.5 / U as f64).cbrt());
        assert!((m.seconds / want - 1.0).abs() < 0.005, "{} s, the cube's law {want} s", m.seconds);
        // from -10 C (a block from the cap): longer by its sensible heat; a little frost settles first
        let cold = melt_block(-10.0, 10.0, 5.0, 5.0, 1.0);
        assert_eq!(cold.water + cold.vapour, U);
        assert!(cold.seconds > m.seconds);
    }

    #[test]
    fn a_pond_freezes_from_the_top_by_stefans_law() {
        let ha = 14.5;
        let f = freeze_pond(U, 0.0, -10.0, ha, 10.0, false);
        for &(d, t, _) in &f.marks {
            let want = stefan(d, -10.0, ha);
            assert!((t / want - 1.0).abs() < 0.01, "{d} m at {t} s, Stefan {want} s");
        }
        // the root of time: past the air's resistance (k / h = 0.15 m) four times the time is twice the ice
        let (d1, t1, q1) = f.marks[2];
        let (d9, t9, q9) = f.marks[8];
        let p = (d9 / d1).ln() / (t9 / t1).ln();
        assert!((0.5..0.65).contains(&p), "ice grows as t^{p}");
        // and it conducts less as it thickens
        assert!(q9 < q1 / 2.0, "{q1} W/m2 at {d1} m, {q9} at {d9}");
        // from +4 C the water first cools (about a real day), then freezes the same way
        let warm = freeze_pond(U, 4.0, -10.0, ha, 10.0, false);
        assert!(warm.solid > f.solid && warm.solid < f.solid + 2.0 * DAY * 50.0);
    }

    #[test]
    fn a_pot_over_a_fire_boils_off_into_the_air_cell() {
        let b = boil_pot(41, 15.0, 15.0, ATM, 3000.0, 0.05);
        assert_eq!(b.vapour, 41, "every unit is in the air cell's vapour");
        assert!((b.plateau - 100.0).abs() < 0.05 && b.max_t < 100.05, "boils at {} (max {})", b.plateau, b.max_t);
    }

    #[test]
    fn thin_air_boils_cold_and_no_air_sublimates() {
        // Mars's 0.006 atm (608 Pa, just under the triple point): water leaves at 0 C
        let mars = t_transition(0.006 * ATM);
        assert!(mars.abs() < 0.1, "{mars}");
        // at 700 Pa a pot boils at 2 C
        let b = boil_pot(41, 1.0, -60.0, 700.0, 3000.0, 0.05);
        assert_eq!(b.vapour, 41);
        assert!((b.plateau - t_transition(700.0)).abs() < 0.05 && (b.plateau - 1.95).abs() < 0.1, "{}", b.plateau);
        // at 608 Pa an ice block over a fire goes to vapour, never water
        let m = sublimate(-20.0, -60.0, 0.006 * ATM, 2000.0, 20.0 * DAY, 0.05);
        assert_eq!(m.max_water, 0);
        assert!(m.vapour > 0 && m.ice + m.vapour == U, "{} ice {} vapour", m.ice, m.vapour);
        // in no air the sun's 500 W sublimates it, the block colder than -50 C, no water ever
        let v = sublimate(-10.0, -100.0, 0.0, 500.0, 20.0 * DAY, 0.05);
        assert_eq!(v.max_water, 0);
        assert!(v.vapour > 0 && v.ice + v.vapour == U && v.temp < -50.0, "{} C", v.temp);
    }

    #[test]
    fn the_untouched_seas_formula_is_where_stored_sea_settles() {
        let ha = 14.5;
        let f = f_for_sheet(-8.0, ha);
        assert_eq!(rest_ice(-8.0, f, ha, 20 * U), U, "the bake's sheet at its threshold");
        let real_year = 3.2e7;
        let s = sea_stretch(32, 20 * U, -0.5, -30.0, f, ha, 25.0 * real_year, 3600.0);
        for &(t, rest, kept, open) in &s.rows {
            assert!((kept - rest).abs() <= 1, "{t} C: laid at {rest}, now {kept}");
            assert!((open - rest).abs() <= (rest / 50).max(4), "{t} C: formula {rest}, from open water {open}");
        }
    }
}
