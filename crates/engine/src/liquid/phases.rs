//! Water's phases (liquids.md "Phases" and step 5; ADR 0183, 0187; the spike `prototypes/water-phases`): ice, water
//! and vapour on one ledger, moved by heat.
//!
//! Stored water at rest where its rest is liquid carries nothing (`Liquids::heat` holds no entry for it). A stored
//! node is given a heat (`Therm`) when it is off that rest: open to air under its freeze point, on rock over its
//! boiling point, or under air too thin for a liquid (`wake_water`). Then, once a second on the server
//! (`water_heat`), heat flows by real laws: to the air over an open surface (convection and radiation), up through
//! the ice over it to cold air (Stefan's law with the air's resistance), from the sea under it (the heat that makes
//! the bake's sea ice the rest state), from hot rock under and beside it (held at its heat: steady). Heat runs on the
//! world's two constants (`interior::HEAT_PACE` on every flow, `interior::LIGHT` under every stored heat), so what a
//! block's heat holds back runs 300 times real and every steady temperature is the real one. What the cold takes past
//! the freeze point is progress (`Therm::frac`) while the water stays liquid; each whole block frozen leaves the node,
//! booked to
//! `Liquids::froze`, as a block of the row's `freeze` block (ice) at the water's top, through the edits as lava's
//! basalt does. Over the boiling point at the place's pressure, where hot rock drives it, units leave for
//! `Liquids::vapour`. Stored sea is left to the formula (the bake's sheet) in this step.
//!
//! Ice blocks a player placed, the water froze, or an edit touched are followed (`Liquids::ice`, `thaw`): air over the
//! freeze point and a hot fluid beside them melt them at the same laws, and a block that melts through is its U units
//! of water poured where it stood (under the triple point's pressure it sublimates: its units go to `vapour`, no water).
//!
//! One ledger: `froze[water]` is the units turned to ice blocks less those melted back, so `counted + vapour + froze` of
//! water changes only by water carried in or out (a bucket, creative), never by a change of phase.
use super::*;

/// One standard atmosphere, Pa.
pub const ATM: f64 = 101_325.0;
const R_GAS: f64 = 8.314_462_618;
const T0K: f64 = 273.15;
/// Earth's air: its molar mass, kg/mol (its scale height), and heat capacity, J/(kg K) (the Lewis analogy's).
/// shortcut: Earth's air on every body (Mars's CO2 would be 0.044); from a body's row once one carries its air.
const M_AIR: f64 = 0.028_97;
const CP_AIR: f64 = 1005.0;
/// The rock's conductivity, W/(m K) (`ROCK_EFFUSIVITY`'s rock, Turcotte and Schubert table 4.1), and the boundary layer
/// of it a held rock warms a hollow's water through, m: steady conduction from a field held at its heat far off into
/// a sphere of radius r is k dT / r at its face (Carslaw and Jaeger), r half the block (4 W/(m^2 K) of rock).
const ROCK_K: f64 = 2.0;
const ROCK_LAYER: f64 = 0.5;

/// Heat transfer between a surface at about `t` C and the air and sky, W/(m^2 K): free convection (`H_AIR`, none in
/// no air) and radiation at emissivity `e`, linearised (4 e sigma T^3: about 5 at 15 C).
pub fn h_air(t: f64, e: f64, air: bool) -> f64 {
    4.0 * e * SIGMA * (t + T0K).max(0.0).powi(3) + if air { H_AIR } else { 0.0 }
}

/// The air's pressure `over` m over the sea at `t` C under a pull of `g`, Pa: one atmosphere at the sea and the
/// isothermal scale height R T / (M g) (4.2 km on Terra at 19.62 m/s^2).
pub fn pressure(over: f64, t: f64, g: f64) -> f64 {
    ATM * (-over.max(0.0) * M_AIR * g / (R_GAS * (t + T0K).max(1.0))).exp()
}

/// A substance's numbers for its phases, from its row (`data/fluids.toml`), per unit (`U` a block).
pub struct Phases {
    /// Its freeze point, C, and its triple point's pressure, Pa (taken at the freeze point: 0.01 K off for water).
    pub tf: f64,
    pub pt: f64,
    tt: f64,
    /// The slopes of its two vapour pressure curves, L M / R, K: over the liquid fitted so the curve passes the
    /// triple point and `boil` at one atmosphere (one L at every temperature: within 4 % of water's real curve, the
    /// spike), over the solid that plus the freeze's latent heat.
    lv_r: f64,
    ls_r: f64,
    /// J a unit: to freeze, to boil (the row's `vaporise`), and a kelvin of the liquid, as the world stores them (a
    /// `LIGHT`th of the real).
    pub lf: f64,
    pub lv: f64,
    pub c: f64,
    /// kg a unit and kg/mol.
    mu: f64,
    molar: f64,
    /// W/(m K), the world's (at `HEAT_PACE`): the liquid's and its solid's; the open surface's emissivity.
    k: f64,
    pub ki: f64,
    e: f64,
    /// The heat the sea gives its ice from below, W/m^2: what makes the bake's sheet (one block where the year's mean
    /// is `t_sea_ice`, `bake::life`) the rest state, `(tf - t_sea_ice) / (1 m / ki + 1 / h)`: 15.4 for water. The
    /// real Arctic's ocean gives 2 to 10, and summer melt does the rest of the real balance, which a year's mean
    /// cannot see (the spike).
    pub sea: f64,
}

impl Phases {
    /// None for a row with no phases (no `freeze`, `boil`, or the heat's `solid_conduct`, `molar`, `triple`).
    pub fn of(row: &crate::fluids::Fluid) -> Option<Phases> {
        let h = row.heat.as_ref()?;
        let (f, b) = (row.freeze.as_ref()?, row.boil.as_ref()?);
        if !(h.solid_conduct > 0.0 && h.molar > 0.0 && h.triple > 0.0 && h.vaporise > 0.0) {
            return None;
        }
        let mu = row.density as f64 / U as f64;
        let (tf, tt, pt, molar) = (f.at as f64, f.at as f64 + T0K, h.triple as f64, h.molar as f64);
        let lv_r = (ATM / pt).ln() / (1.0 / tt - 1.0 / (b.at as f64 + T0K));
        // the world's heat (`interior::HEAT_PACE`, `LIGHT`): conductivities and the air's terms paced, the heats stored a
        // `LIGHT`th; the vapour curves keep the real latent heat (where it boils is the real place's)
        let (ki, e) = (h.solid_conduct as f64 * HEAT_PACE, h.emissivity as f64);
        let ts = crate::bake::life::LifeParams::default().t_sea_ice as f64;
        Some(Phases {
            tf,
            pt,
            tt,
            lv_r,
            ls_r: lv_r + h.latent as f64 * molar / R_GAS,
            lf: h.latent as f64 * mu / LIGHT,
            lv: h.vaporise as f64 * mu / LIGHT,
            c: h.capacity as f64 * mu / LIGHT,
            mu,
            molar,
            k: h.conduct as f64 * HEAT_PACE,
            ki,
            e,
            sea: (tf - ts) / (1.0 / ki + 1.0 / (h_air(ts, e, true) * HEAT_PACE)),
        })
    }
    /// Water's (`data/fluids.toml` row 0).
    pub fn water() -> Phases {
        Phases::of(crate::fluids::FLUIDS.now().get(crate::fluids::WATER)).expect("water's row has its phases")
    }
    /// Its vapour's saturation pressure at `t` C over its liquid or its solid, Pa (Clausius-Clapeyron).
    pub fn p_sat(&self, t: f64, solid: bool) -> f64 {
        self.pt * (-(if solid { self.ls_r } else { self.lv_r }) * (1.0 / (t + T0K) - 1.0 / self.tt)).exp()
    }
    /// Where it leaves for its gas at pressure `p` Pa, C: its boiling point over the triple point's pressure, its
    /// sublimation point under it (no liquid there), absolute zero in no air.
    pub fn boils_at(&self, p: f64) -> f64 {
        if p <= 0.0 {
            return -T0K;
        }
        let a = if p >= self.pt { self.lv_r } else { self.ls_r };
        1.0 / (1.0 / self.tt - (p / self.pt).ln() / a) - T0K
    }
    /// `h_air` of its surface, the world's (at `HEAT_PACE`).
    pub fn h(&self, t: f64, p: f64) -> f64 {
        h_air(t, self.e, p > 0.0) * HEAT_PACE
    }
    /// The formula for untouched sea (the state stored sea settles to): its ice's thickness at rest under air at `t`
    /// C, m, where the heat up through the ice, (tf - t) / (d / ki + 1 / h), is the sea's from below.
    pub fn rest_ice(&self, t: f64) -> f64 {
        (self.ki * ((self.tf - t) / self.sea - 1.0 / self.h(t, ATM))).max(0.0)
    }
    /// The vapour leaving its solid's surface at `ts` C into dry air at `p` Pa, kg/(m^2 s): at the kinetic limit
    /// (Hertz-Knudsen) where its vapour pressure is at or over the air's (always in no air), else diffusing through
    /// the air (the Lewis analogy from the free convection). shortcut: dry air (the air grid's vapour, step 6).
    fn flux(&self, ts: f64, p: f64) -> f64 {
        let ps = self.p_sat(ts, true);
        if ps >= p {
            ps * (self.molar / (2.0 * std::f64::consts::PI * R_GAS * (ts + T0K))).sqrt()
        } else {
            0.622 * H_AIR / (CP_AIR * p) * ps
        }
    }
}

impl Liquids {
    /// Ice block `z` of column `c` is followed from now on (placed, frozen, or an edit beside it): its heat is
    /// stepped by `cool` until it melts or is dug.
    pub fn watch_ice(&mut self, c: Col, z: i32) {
        self.ice.entry((c, z)).or_insert(0.0);
    }
    /// Ice block `z` of column `c` melted, but its water had no place (the gap at its most layers): it is ice again,
    /// booked, and followed melted through, so it tries again the next second.
    pub fn refreeze(&mut self, c: Col, z: i32, units: i64) {
        *self.froze.entry(crate::fluids::WATER).or_default() += units;
        self.ice.insert((c, z), U as f64);
    }
    /// How many ice blocks are followed.
    pub fn watched_ice(&self) -> usize {
        self.ice.len()
    }
    /// The units of ice block `z` of column `c` melted so far, if it is followed (tests, F3).
    pub fn ice_melted(&self, c: Col, z: i32) -> Option<f64> {
        self.ice.get(&(c, z)).copied()
    }

    /// Stored water off its rest is given a heat: water alone in its gap, open to the air (or under ice) where the
    /// air is under its freeze point or too thin for a liquid, or on rock over its boiling point. Liquid at rest it
    /// carries none and costs a look at its air a second.
    /// shortcut: every stored water node is looked at once a second (the spike's 0.06 ms for 100,000); round-robin
    /// over tiles if many stored columns ever make it dear.
    pub(super) fn wake_water(&mut self, g: &impl Ground, ph: &Phases) {
        let mut wake = Vec::new();
        for &a in self.cols.values().flatten() {
            let n = &self.nodes[a as usize];
            if n.kind != crate::fluids::WATER || n.water == 0 || n.stack != 1 || self.heat.contains_key(&a) {
                continue;
            }
            let (ta, p) = g.air(n.col, n.surface() as f64 / U as f64);
            let tb = ph.boils_at(p);
            // shortcut: stored sea does not freeze, until ice by physics (roadmap step 7) gives the sea's sheet a rule
            // that returns to generation
            let (lo, hi) = g.generated(n.col);
            let aired = lo >= hi && (n.ceil == OPEN || g.ice(n.col, n.ceil));
            // its held rock is looked at only where it could matter: rock under or at it over its boiling point, or
            // freezing air over it. shortcut: the walls' rock is taken as near the column's own as that tells
            let near = (g.heat(n.col, n.floor as f64) as f64).max(g.heat(n.col, n.surface() as f64 / U as f64) as f64);
            let freezing = aired && (ta < ph.tf || p < ph.pt);
            if near < tb + REST_K && !freezing {
                continue;
            }
            let open = lo >= hi && n.ceil == OPEN;
            let (boils, ts, ..) = self.drive(g, a, ph, open, ta, p, tb);
            let settles = if open { ta.max(ts) } else { ta };
            if boils || (freezing && (settles < ph.tf || p < ph.pt)) {
                // it comes at what its place held it at (the rock's, or the air's on a hot place), liquid
                let t = if boils { ta } else { self.rest_of(g, a) };
                wake.push((a, t.clamp(ph.tf, tb.max(ph.tf))));
            }
        }
        for (a, t) in wake {
            self.heat.insert(a, Therm { t, age: 0.0, frac: 0.0, solid: 0 });
        }
    }

    /// What drives node a's heat: its held rock (`Ground::held`), steady through `ROCK_LAYER` of rock and through half
    /// its own depth (the floor) or half a block (each wall cell, by the share of the water's depth beside it), and
    /// the open air over it, `open` (all at `HEAT_PACE`, which cancels in T*). Every flow is linear in the water's
    /// heat, so they settle it at
    /// T* = (sum G T) / (sum G); it boils where T* passes its boiling point by `REST_K` with held rock among them.
    /// Returns (it boils, T*, the hottest held rock's heat, the held rock's (sum G, sum G T)).
    #[allow(clippy::too_many_arguments)]
    fn drive(&self, g: &impl Ground, a: u32, ph: &Phases, open: bool, ta: f64, p: f64, tb: f64) -> (bool, f64, f64, (f64, f64)) {
        let n = &self.nodes[a as usize];
        let (c, lo, hi) = (n.col, n.bottom(), n.surface());
        let deep = n.water.min(n.cap()) as f64 / U as f64;
        let r = ROCK_LAYER / (ROCK_K * HEAT_PACE);
        let (mut gs, mut gt, mut top) = (0.0, 0.0, f64::MIN);
        let mut add = |k: f64, t: f64| {
            gs += k;
            gt += k * t;
            top = top.max(t);
        };
        if lo == n.floor as i64 * U && g.held(c, n.floor - 1) {
            add(1.0 / (r + deep * 0.5 / ph.k), g.heat(c, n.floor as f64) as f64);
        }
        if hi > lo {
            for d in g.neighbours(c).into_iter().flatten() {
                let gaps = g.gaps(d);
                for z in lo.div_euclid(U) as i32..=(hi - 1).div_euclid(U) as i32 {
                    if !gaps.iter().any(|&(f, t)| f <= z && z < t) && g.held(d, z) {
                        let part = (hi.min((z as i64 + 1) * U) - lo.max(z as i64 * U)) as f64 / U as f64;
                        add(part / (r + 0.5 / ph.k), g.heat(d, z as f64 + 0.5) as f64);
                    }
                }
            }
        }
        let ga = if open { ph.h(ta, p) } else { 0.0 };
        let ts = if gs + ga > 0.0 { (gt + ga * ta) / (gs + ga) } else { ta };
        (gs > 0.0 && ts > tb + REST_K, ts, top, (gs, gt))
    }

    /// One heat step of `dt` s for stored water node a holding heat `h`: None when it is back at rest (liquid, short of
    /// freezing, in air over its freeze point, on rock under its boiling point), out of the air's reach, or gone.
    /// What it froze is progress (`Therm::frac`, units' worth of latent heat taken) and stays liquid water until a whole
    /// block has frozen: then the block's U units leave the node, are booked to `froze`, and the block of ice goes to
    /// `out` (column, layer, water's kind) at the node's top cell, as `cool`'s basalt; a step that freezes several sets
    /// every one, downward from there. While a block waits for its cell (`waiting`, a player in it) the node freezes no
    /// further. shortcut: less than a block of water never freezes, until partial layers (ADR 0189).
    pub(super) fn water_heat(&mut self, g: &impl Ground, a: u32, mut h: Therm, dt: f64, ph: &Phases, out: &mut Vec<(Col, i32, u8)>) -> Option<Therm> {
        let water = crate::fluids::WATER;
        let n = &self.nodes[a as usize];
        let (c, ceil, w, top, layered) = (n.col, n.ceil, n.water, n.surface(), n.stack > 1);
        let on_top = n.layer + 1 == n.stack;
        let (ta, p) = g.air(c, top as f64 / U as f64);
        let tb = ph.boils_at(p);
        // ice over it: blocks over its gap's ceiling
        let iced = if ceil == OPEN { 0 } else { (0..16).take_while(|&k| g.ice(c, ceil + k)).count() };
        // the air reaches it over its open surface, or through an ice lid; not under another block or another layer,
        // nor in a column of generated sea (shortcut: stored sea does not freeze, until ice by physics (roadmap step 7)
        // gives the sea's sheet a rule that returns to generation)
        let (lo, hi) = g.generated(c);
        let (open, lid) = if on_top && lo >= hi { (ceil == OPEN, iced > 0) } else { (false, false) };
        let aired = open || lid;
        // (a part-block handed on by an edit is progress too)
        h.frac += std::mem::take(&mut h.solid) as f64;
        let ice = h.frac.max(0.0);
        // at rest where its held rock and air cannot bring it to its boiling point and it is not freezing: it settles
        // at T*, liquid, and carries no heat
        // (under an ice lid the cold air still takes it through the ice: there the air's heat decides, as before)
        let (boils, ts, hottest, rock) = self.drive(g, a, ph, open, ta, p, tb);
        let settles = if open { ta.max(ts) } else { ta };
        if !boils && (!aired || (ice == 0.0 && settles >= ph.tf && p >= ph.pt)) {
            return None;
        }
        // a block waiting for its cell holds the freeze where it is
        let waits = out.iter().any(|o| o.0 == c && o.2 == water);
        let blocks;
        let mut boiled = 0i64;
        if p < ph.pt && aired {
            if waits {
                return Some(h);
            }
            // no liquid under the triple point's pressure: its heat over the freeze point and the latent heat of what
            // freezes boil a share off at once, (c dT + Lf) / (Lv + Lf) (13 % from 0 C), every whole block of the rest
            // freezes, and what is short of a block boils with the share. shortcut: short of a block boils at once (it
            // would freeze and sublimate), until partial layers (ADR 0189)
            let x = (ph.c * (h.t - ph.tf).max(0.0) + ph.lf) / (ph.lv + ph.lf);
            blocks = (w - (w as f64 * x).ceil() as i64).max(0) / U;
            boiled = w - blocks * U;
            (h.t, h.frac) = (ph.tf, 0.0);
        } else {
            // (every conductance here is the world's, at `HEAT_PACE`; the rock's as the air's)
            let ha = ph.h(ta, p);
            let d = iced as f64 + ice / U as f64;
            let mut q = 0.0;
            if boils {
                // the held rock under it and beside it (`drive`), sum G (T_rock - T)
                q += rock.1 - rock.0 * h.t;
            }
            if aired && d > 0.0 {
                if ta < ph.tf {
                    // up through the ice (its bottom at the freeze point) and the air's own resistance: Stefan's law
                    q -= (ph.tf - ta) / (d / ph.ki + 1.0 / ha);
                } else if open {
                    // warm air melts its skin; a lid gives it nothing (its own melt is `thaw`'s)
                    q += ha * (ta - ph.tf);
                }
            } else if open {
                q -= ha * (h.t - ta);
            }
            let mut e = q * dt;
            if e < 0.0 {
                // (a boil paid for short of a unit gives its heat back first)
                if h.frac < 0.0 {
                    let used = (-h.frac * ph.lv).min(-e);
                    e += used;
                    h.frac += used / ph.lv;
                }
                // it cools to the air, not past it; under its freeze point the loss freezes it
                let cap = w as f64 * ph.c;
                let s = ((h.t - ph.tf.max(ta)).max(0.0) * cap).min(-e);
                h.t -= s / cap;
                e += s;
                if ta < ph.tf && aired {
                    h.frac -= e / ph.lf;
                }
            } else {
                // its skin melts first, then it warms; never hotter than what heats it (the air, the rock), and what
                // is left boils only where the rock drives it over its boiling point
                if ice > 0.0 {
                    let m = (e / ph.lf).min(ice);
                    e -= m * ph.lf;
                    h.frac -= m;
                }
                let cap = w as f64 * ph.c;
                let s = ((tb.min(ta.max(hottest)) - h.t).max(0.0) * cap).min(e);
                h.t += s / cap;
                e -= s;
                // at its boiling point the rest boils it: `frac` under 0 is the next unit's boil paid for so far
                if e > 0.0 && boils {
                    h.frac -= e / ph.lv;
                    boiled = ((-h.frac).floor() as i64).min(w);
                    h.frac += boiled as f64;
                }
            }
            // every whole block frozen leaves the node, none while one waits; progress never past what is left, nor past
            // a block
            blocks = if waits { 0 } else { ((h.frac.max(0.0) / U as f64).floor() as i64).min((w - boiled) / U) };
            h.frac -= (blocks * U) as f64;
            h.frac = h.frac.min((w - boiled - blocks * U).min(U) as f64);
        }
        let cell = ((top - 1).div_euclid(U)) as i32;
        if blocks > 0 {
            *self.froze.entry(water).or_default() += blocks * U;
            out.extend((0..blocks as i32).map(|k| (c, cell - k, water)));
        }
        if boiled > 0 {
            *self.vapour.entry(water).or_default() += boiled;
            self.boils.push((c, cell, water));
        }
        let left = w - boiled - blocks * U;
        if left != w {
            let n = &mut self.nodes[a as usize];
            n.water = left;
            if n.quiet == u16::MAX {
                self.wake(a);
            }
            self.dirty.insert(c, ());
            if layered {
                // a layer emptied goes, as every emptied layer does (`layers_after`)
                if left == 0 {
                    self.heat.remove(&a);
                    self.drop_layer(a);
                    self.awake.retain(|&i| i != a);
                    self.free.push(a);
                }
                self.restack(c);
            }
        }
        if left == 0 {
            return None;
        }
        h.age += dt;
        Some(h)
    }

    /// One heat step of `dt` s for every ice block followed: its open faces (air beside, over or under it) take heat
    /// from the air, a hot fluid beside it (lava) gives it through half a block of itself, both at `HEAT_PACE`.
    /// Over the triple point's pressure what it takes melts it at the latent heat; under it, it sublimates, its
    /// surface where the heat it takes pays for the vapour leaving. The melt is held in the block (its area shrinking
    /// as a cube's, the last half unit at that cube's) until all of it went: the block is then its U units of water
    /// poured where it stood, or its units in `vapour` (`thawed` for the caller). A block dug is forgotten.
    /// shortcut: ice under freezing air keeps (its own cold and its sublimation into dry air are not followed, step 6);
    /// water beside a block gives it no heat.
    pub(super) fn thaw(&mut self, g: &impl Ground, dt: f64, ph: &Phases) {
        if self.ice.is_empty() {
            return;
        }
        let water = crate::fluids::WATER;
        let rows = crate::fluids::FLUIDS.now();
        let mut cells: Vec<(Col, i32)> = self.ice.keys().copied().collect();
        cells.sort_unstable();
        for (c, z) in cells {
            if !g.ice(c, z) {
                self.ice.remove(&(c, z));
                continue;
            }
            let (ta, p) = g.air(c, z as f64 + 0.5);
            let (mut open, mut src) = (0u32, 0.0f64);
            let beside: Vec<(Col, i32)> = g.neighbours(c).into_iter().flatten().map(|d| (d, z)).chain([(c, z - 1), (c, z + 1)]).collect();
            for (d, y) in beside {
                if !g.gaps(d).iter().any(|&(f, t)| f <= y && y < t) {
                    continue;
                }
                match self.fluid_heat(g, d, y) {
                    None => open += 1,
                    Some((k, t)) if k != water => {
                        if let Some(hr) = rows.get(k).heat.as_ref() {
                            src += super::ice_face(t, ph.tf, hr.conduct);
                        }
                    }
                    Some(_) => {}
                }
            }
            let m = self.ice[&(c, z)];
            let s = ((U as f64 - m).max(0.5) / U as f64).cbrt();
            let area = open as f64 * s * s;
            let ha = ph.h(ta, p);
            let gone = if p >= ph.pt {
                (ha * area * (ta - ph.tf) + src).max(0.0) * dt / ph.lf
            } else {
                // units a second leaving its surface at ts: what carries the real latent heat away at the pace, paid
                // for from the block's stored (a `LIGHT`th), so HEAT_PACE x LIGHT times the real mass flux; the surface
                // where it balances the heat coming in is the real one
                let vap = |ts: f64| ph.flux(ts, p) * area * HEAT_PACE * LIGHT / ph.mu;
                let (mut lo, mut hi) = (1.0 - T0K, ph.tf);
                for _ in 0..48 {
                    let ts = 0.5 * (lo + hi);
                    if ha * area * (ta - ts) + src > vap(ts) * (ph.lf + ph.lv) {
                        lo = ts;
                    } else {
                        hi = ts;
                    }
                }
                vap(0.5 * (lo + hi)) * dt
            };
            let m = m + gone;
            if m < U as f64 {
                self.ice.insert((c, z), m);
                continue;
            }
            self.ice.remove(&(c, z));
            *self.froze.entry(water).or_default() -= U;
            if p >= ph.pt {
                self.thawed.push((c, z, U));
            } else {
                *self.vapour.entry(water).or_default() += U;
                self.thawed.push((c, z, 0));
            }
        }
    }

    /// The fluid in cell z of column c and its heat, C: stored (its node's), else as generation gives it (at its
    /// place's rest heat).
    fn fluid_heat(&self, g: &impl Ground, c: Col, z: i32) -> Option<(u8, f64)> {
        let u = z as i64 * U + U / 2;
        if let Some(ids) = self.cols.get(&c) {
            let a = ids.iter().copied().find(|&i| {
                let n = &self.nodes[i as usize];
                n.water > 0 && n.bottom() <= u && u < n.surface()
            })?;
            return Some((self.nodes[a as usize].kind, self.temp_of(g, a)));
        }
        self.kind_at(g, c, z as f64 + 0.5).map(|k| (k, g.heat(c, z as f64 + 0.5) as f64))
    }
}

#[cfg(test)]
#[path = "phase_tests.rs"]
mod tests;
