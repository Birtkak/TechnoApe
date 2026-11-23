//! A body's interior (planet-model.md step 1, 0162-planet-interior): its zones, mass, gravity at any radius, the
//! weight of its rock and its magnetic field, derived once from its row's makeup (`bodies::Makeup`) and its radius
//! and kept on the `Body` (`Body::interior`), so no reader recomputes them and none keeps a number of its own.
use crate::bodies::Makeup;

/// The felt pull at the first body's ground, m/s^2 (ADR 0007, 0024, 0037): twice the Earth's, which felt floaty
/// on 1 m blocks (the developer, 2026-11-15), whatever the planet's size. It sets the world's gravitational
/// constant (`Interior::k`), from which every other body's pull follows by its mass.
pub const G: f64 = 19.62;

/// The zones, centre out: their index in `Interior::zones`.
pub const INNER_CORE: usize = 0;
pub const OUTER_CORE: usize = 1;
pub const MANTLE: usize = 2;
pub const CRUST: usize = 3;
pub const ZONE_NAMES: [&str; 4] = ["inner core", "outer core", "mantle", "crust"];

/// A zone: its outer radius, metres, and its density, kg/m3 (its inner radius is the zone under it's outer, or 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub outer: f64,
    pub density: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Interior {
    pub radius: f64,
    /// Solid inner core, liquid outer core, mantle, crust; a zone may be 0 thick (a dead core has no liquid).
    pub zones: [Zone; 4],
    /// kg, and the world's gravitational constant (m^3 / kg s^2, scaled so the first body pulls `G`).
    pub mass: f64,
    pub k: f64,
    /// The pull at the ground, m/s^2.
    pub surface_g: f64,
    /// The pull the weight of rock and water is reckoned with (`Makeup::weight_g`, else `surface_g`).
    pub weight_g: f64,
    /// The liquid core's volume, m^3, and its share of the body's.
    pub liquid: f64,
    pub liquid_share: f64,
    /// Turns a second.
    pub spin: f64,
    /// The magnetic field's strength over the first body's as its row is written (1 for Terra, 0 without a
    /// liquid core).
    pub field: f64,
    /// The pull's profile (`pull`): the core's radius over the body's, its density over the mantle's, the
    /// core's share of the volume, and the mass over a body all of mantle rock.
    core_x: f64,
    ratio: f64,
    a3: f64,
    norm: f64,
}

impl Interior {
    /// The interior of a body of `radius` m made of `m`, spinning `spin` turns a second; `first` the first
    /// body's (its gravitational constant and the field's unit), None for the first body itself.
    pub fn new(m: &Makeup, radius: f64, spin: f64, first: Option<&Interior>) -> Interior {
        let [crust, mantle, core] = m.density;
        let rc = radius * m.iron;
        // the inner core freezes out from the centre as the body cools
        let ri = rc * (1.0 - m.heat);
        // the crust's floor `DEEP` under the ground (the grid's crust, ADR 0063), never under the core's top
        let moho = (radius - crate::world::DEEP).max(rc);
        let zones = [Zone { outer: ri, density: core }, Zone { outer: rc, density: core }, Zone { outer: moho, density: mantle }, Zone { outer: radius, density: crust }];
        let (ratio, a3) = (core / mantle, m.iron.powi(3));
        // as ADR 0152's profile has it (`pull`), so a body pulls and weighs as it did before its row had a makeup
        let norm = ratio * a3 + 1.0 - a3;
        let ball = 4.0 / 3.0 * std::f64::consts::PI;
        let mass = ball * mantle * norm * radius.powi(3);
        let (k, surface_g) = match first {
            None => (constant(m, radius), G),
            Some(f) => (f.k, f.k * mass / (radius * radius)),
        };
        let liquid = ball * (rc.powi(3) - ri.powi(3));
        let liquid_share = liquid / (ball * radius.powi(3));
        let (unit, unit_spin) = first.map_or((liquid_share, spin), |f| (f.liquid_share, f.spin));
        let field = if liquid_share > 0.0 && unit > 0.0 { liquid_share / unit * (spin / unit_spin) } else { 0.0 };
        Interior { radius, zones, mass, k, surface_g, weight_g: m.weight_g.unwrap_or(surface_g), liquid, liquid_share, spin, field, core_x: m.iron, ratio, a3, norm }
    }
    /// The pull at x = r / R over the pull at the ground (ADR 0152): the square law outside; inside, the metal
    /// core's and the rock's over it, growing going down to the core's top, then falling evenly to nothing at the
    /// centre. The crust is reckoned at the mantle's density here (ADR 0152's one shell; 0162-planet-interior).
    pub fn pull(&self, x: f64) -> f64 {
        let (a3, m) = (self.a3, self.norm);
        if x >= 1.0 {
            1.0 / (x * x)
        } else if x >= self.core_x {
            (self.ratio * a3 + x.powi(3) - a3) / (m * x * x)
        } else {
            self.ratio * x / m
        }
    }
    /// `pull` integrated from x0 to x1 (x0 <= x1), closed form: the weight of a column is its density times
    /// the pull at the ground and the radius times this.
    pub fn pull_integral(&self, x0: f64, x1: f64) -> f64 {
        let (a3, m, c, k) = (self.a3, self.norm, self.core_x, self.ratio);
        // an antiderivative of each piece, joined where they meet
        let f = |x: f64| {
            let inner = |x: f64| k * x * x / (2.0 * m);
            let shell = |x: f64| inner(c) + ((k - 1.0) * a3 * (1.0 / c - 1.0 / x) + (x * x - c * c) / 2.0) / m;
            if x < c {
                inner(x)
            } else if x < 1.0 {
                shell(x)
            } else {
                shell(1.0) + 1.0 - 1.0 / x
            }
        };
        f(x1) - f(x0)
    }
    /// The pull at radius r, m/s^2.
    pub fn gravity(&self, r: f64) -> f64 {
        self.surface_g * self.pull(r / self.radius)
    }
    /// Zone k's density as the pull reckons it: the crust's is the mantle's (`pull`).
    fn pull_density(&self, k: usize) -> f64 {
        self.zones[if k == CRUST { MANTLE } else { k }].density
    }
    /// The mass inside radius r, kg, by the pull's densities.
    pub fn enclosed(&self, r: f64) -> f64 {
        (0..4).map(|k| {
            let lo = if k == 0 { 0.0 } else { self.zones[k - 1].outer };
            4.0 / 3.0 * std::f64::consts::PI * self.pull_density(k) * (r.clamp(lo, self.zones[k].outer).powi(3) - lo.powi(3))
        }).sum()
    }
    /// Zone k's mass, kg, by the pull's densities.
    pub fn zone_mass(&self, k: usize) -> f64 {
        let lo = if k == 0 { 0.0 } else { self.zones[k - 1].outer };
        4.0 / 3.0 * std::f64::consts::PI * self.pull_density(k) * (self.zones[k].outer.powi(3) - lo.powi(3))
    }
    /// The crust's floor, m from the centre.
    pub fn moho(&self) -> f64 {
        self.zones[MANTLE].outer
    }
    /// The core's radius over the body's (its row's `iron`).
    pub fn core_share(&self) -> f64 {
        self.core_x
    }
    /// The weight of the rock between radius r and the ground at radius `ground` over it, MPa: the crust's density
    /// over the crust's floor, the mantle's under it, with the pull inside the body in closed form.
    pub fn rock_weight(&self, ground: f64, r: f64) -> f64 {
        let moho = self.moho();
        let w = |lo: f64, hi: f64, rho: f64| if hi > lo { rho * self.weight_g * self.radius * 1e-6 * self.pull_integral(lo / self.radius, hi / self.radius) } else { 0.0 };
        w(r.max(moho), ground, self.zones[CRUST].density) + w(r, ground.min(moho), self.zones[MANTLE].density)
    }
}

/// The world's one constant of gravity (ADR 0180), m^3 / (kg s^2): the one at which the home body of makeup `home`
/// and `radius` m pulls `G` at its ground (2.24e-7 on the 4 km Terra, about 3,350 times the real one). Every pull,
/// orbit and tide follows from it and the masses; it goes as 1 / radius, so planet= scales the whole system.
pub fn constant(home: &Makeup, radius: f64) -> f64 {
    let [_, mantle, core] = home.density;
    let a3 = home.iron.powi(3);
    let mass = 4.0 / 3.0 * std::f64::consts::PI * mantle * (core / mantle * a3 + 1.0 - a3) * radius.powi(3);
    G * radius * radius / mass
}

/// A body's spin, turns a second, as the sky turns it (`sky::Sky::planet`, `circle`): the home once a sidereal
/// day (None), a moon, its face to its parent, once a sidereal `month`.
pub fn spin(month: Option<f64>) -> f64 {
    use crate::sky::{DAY, YEAR};
    month.map_or(1.0 / DAY + 1.0 / YEAR, |m| 1.0 / m + 1.0 / YEAR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::{Kind, BODIES};

    fn rocky(iron: f64, heat: f64) -> Makeup {
        Makeup { kind: Kind::Rocky, iron, heat, density: [2800.0, 3400.0, 8000.0], weight_g: None }
    }
    const R: f64 = 3000.0;
    const SPIN: f64 = 1.0 / 1800.0;

    fn rel(a: f64, b: f64) -> f64 {
        if a == b { 0.0 } else { (a - b).abs() / a.abs().max(b.abs()) }
    }

    #[test]
    fn mass_adds_up_over_the_zones() {
        for (iron, heat) in [(0.5, 0.5), (0.3, 0.0), (0.8, 1.0)] {
            let a = Interior::new(&rocky(iron, heat), R, SPIN, None);
            let sum: f64 = (0..4).map(|k| a.zone_mass(k)).sum();
            assert!(rel(sum, a.mass) < 1e-12 && rel(a.enclosed(R), a.mass) < 1e-12, "{sum} {}", a.mass);
            assert_eq!(a.enclosed(0.0), 0.0);
            assert!(rel(a.enclosed(2.0 * R), a.mass) < 1e-12, "nothing outside");
            // ordered zones ending at the ground
            assert!(a.zones.windows(2).all(|z| z[0].outer <= z[1].outer) && a.zones[CRUST].outer == R);
        }
    }

    #[test]
    fn gravity_is_continuous_zero_at_the_centre_and_falls_as_the_square_outside() {
        let a = Interior::new(&rocky(0.5, 0.5), R, SPIN, None);
        assert_eq!(a.gravity(0.0), 0.0);
        assert!(rel(a.gravity(R), a.surface_g) < 1e-12 && a.surface_g == G, "the first body pulls G");
        for b in a.zones.iter().map(|z| z.outer) {
            let (lo, hi) = (a.gravity(b - 1e-6), a.gravity(b + 1e-6));
            assert!(rel(lo, hi) < 1e-6, "at {b} m: {lo} under, {hi} over");
        }
        for x in [1.5, 2.0, 3.0, 10.0] {
            assert!(rel(a.gravity(x * R) * x * x, a.surface_g) < 1e-12, "at {x} R");
        }
        // the general form: the mass inside over the square of the radius, at any depth
        for r in [10.0, 700.0, 1499.0, 1501.0, 2000.0, 2900.0, R] {
            assert!(rel(a.gravity(r), a.k * a.enclosed(r) / (r * r)) < 1e-12, "at {r} m");
        }
    }

    #[test]
    fn pressure_grows_down_without_a_step() {
        let a = Interior::new(&rocky(0.5, 0.5), R, SPIN, None);
        assert_eq!(a.rock_weight(R, R), 0.0);
        let mut last = 0.0;
        for k in 1..300 {
            let r = R - k as f64 * 5.0;
            let w = a.rock_weight(R, r);
            assert!(w > last, "at {r} m: {w} after {last}");
            last = w;
        }
        let m = a.moho();
        assert!(m < R && m > a.zones[OUTER_CORE].outer);
        assert!((a.rock_weight(R, m - 1e-6) - a.rock_weight(R, m + 1e-6)).abs() < 1e-6, "at the crust's floor");
        // by hand at the crust's floor: its density times the mean pull times its thickness
        let by_hand = 2800.0 * (G + a.gravity(m)) / 2.0 * (R - m) * 1e-6;
        assert!(rel(a.rock_weight(R, m), by_hand) < 1e-3, "{} {by_hand}", a.rock_weight(R, m));
    }

    #[test]
    fn heat_and_iron_shape_the_core() {
        let dead = Interior::new(&rocky(0.5, 0.0), R, SPIN, None);
        assert_eq!((dead.liquid, dead.field, dead.zones[INNER_CORE].outer), (0.0, 0.0, dead.zones[OUTER_CORE].outer), "a dead core is all solid");
        let young = Interior::new(&rocky(0.5, 1.0), R, SPIN, None);
        assert_eq!(young.zones[INNER_CORE].outer, 0.0, "a young one all liquid");
        let [small, big] = [0.3, 0.6].map(|iron| Interior::new(&rocky(iron, 0.5), R, SPIN, None));
        assert!(big.zones[OUTER_CORE].outer > small.zones[OUTER_CORE].outer && big.mass > small.mass && big.liquid > small.liquid);
    }

    #[test]
    fn the_field_rises_with_liquid_metal_and_spin() {
        let first = Interior::new(&rocky(0.5, 0.5), R, SPIN, None);
        assert_eq!(first.field, 1.0, "the unit");
        let at = |heat: f64, spin: f64| Interior::new(&rocky(0.5, heat), R / 2.0, spin, Some(&first)).field;
        assert!(at(0.2, SPIN) < at(0.5, SPIN) && at(0.5, SPIN) < at(0.9, SPIN));
        assert!(at(0.5, SPIN / 30.0) < at(0.5, SPIN) && at(0.5, SPIN) < at(0.5, SPIN * 2.0));
        assert!(rel(at(0.5, SPIN), 1.0) < 1e-12, "its size does not change it");
        assert_eq!(at(0.0, SPIN), 0.0);
    }

    #[test]
    fn surface_gravity_comes_from_mass_and_radius() {
        let first = Interior::new(&rocky(0.5, 0.5), R, SPIN, None);
        assert!(rel(first.k * first.mass / (R * R), G) < 1e-12);
        // a denser body of the same size pulls more, by its mass
        let heavy = Makeup { density: [3000.0, 4000.0, 9000.0], ..rocky(0.5, 0.5) };
        let b = Interior::new(&heavy, R, SPIN, Some(&first));
        assert!(rel(b.surface_g, first.k * b.mass / (R * R)) < 1e-12 && b.surface_g > G);
        assert_eq!(b.weight_g, b.surface_g, "its own pull weighs its rock");
        let _ = &BODIES[0];
    }

    /// The gas giant's row (step 6a): about 300 of Terra's masses in 11 of her radii, so about 2.5 times her pull
    /// at the cloud tops (the real Jupiter's 2.5 g); its conducting fluid deep and its day short, so a field far
    /// over Terra's by the same rule (`field`); its pull inside the gas falls smoothly to nothing at the centre.
    #[test]
    fn a_gas_giant_pulls_and_shields_from_its_row() {
        let r = |n: i32| n as f64 / (2.0 * std::f64::consts::PI / 3.0).sqrt();
        let terra = Interior::new(&BODIES[0].makeup, r(6144), spin(None), None);
        let g = Interior::new(&BODIES[3].makeup, r(6144 * 11), giant_spin(), Some(&terra));
        let masses = g.mass / terra.mass;
        assert!((290.0..310.0).contains(&masses), "{masses} Terra masses");
        let pull = g.surface_g / G;
        assert!((2.4..2.6).contains(&pull), "{pull} times Terra's pull at the cloud tops");
        assert!((3.0..5.0).contains(&g.field), "field {} of Terra's", g.field);
        assert_eq!(g.gravity(0.0), 0.0);
        let mut last = 0.0;
        for k in 1..=100 {
            let x = g.radius * k as f64 / 100.0;
            let a = g.gravity(x);
            // rising to the dense fluid's top, then falling a little through the light gas over it: never a step
            assert!(a > 0.0 && (a - last).abs() < 0.05 * g.surface_g, "at {x} m: {a} after {last}");
            last = a;
        }
        assert!(rel(g.gravity(2.0 * g.radius) * 4.0, g.surface_g) < 1e-12, "the square law outside");
    }

    /// The giant's spin from its row's `day` (in Terra's days), as the world gives it (`World::of`).
    fn giant_spin() -> f64 {
        1.0 / (BODIES[3].day.unwrap() * crate::sky::DAY)
    }
}
