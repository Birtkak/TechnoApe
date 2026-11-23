//! Throwaway spike (docs/design/liquids.md, "Forces"): the tides the bodies' rows imply, and the tide as a head
//! term on the water's links. `cargo run --release` prints question A; `cargo run --release -- bench` question B's
//! costs; `cargo test --release` the four tests.
mod sim;

use engine::sky::{DAY, MONTH, YEAR};
use engine::world::{World, INERTIAL};
use std::f64::consts::TAU;

fn main() {
    if std::env::args().any(|a| a == "bench") {
        sim::bench();
        return;
    }
    tides();
}

/// Question A: every block body's equilibrium tide from every other body, by the engine's own numbers.
fn tides() {
    let n = 6144;
    let w = World::with_grid(n, engine::bodies::moon_columns(n), 1, None, None, 0.0, 7);
    let b = &w.bodies;
    let k = b[0].interior.k;
    // the sun has no row and no mass: the one Kepler's law gives from Terra's year at Terra's distance
    let m_sun = (TAU / YEAR).powi(2) * b[0].orbit.powi(3) / k;
    let sun_rho = m_sun / (4.0 / 3.0 * std::f64::consts::PI * w.sky.sun_radius.powi(3));
    println!("world constant k = {k:.4e} (scaled so Terra pulls {} m/s^2; real G 6.674e-11)", engine::interior::G);
    println!("the sun: mass from Terra's year {m_sun:.4e} kg = {:.0} Terra masses; radius {:.0} m, so {sun_rho:.0} kg/m3 (the real Sun 1408)", m_sun / b[0].interior.mass, w.sky.sun_radius);
    println!();
    println!("{:<9} {:>8} {:>9} {:>11} {:>9} {:>10} {:>10} {:>9} {:>9} {:>11}", "body", "radius", "mass/Ter", "surface_g", "weight_g", "orbit", "period s", "Kepler s", "ratio", "sun dist");
    let sun_dist = |i: usize| if b[i].parent == INERTIAL { b[i].orbit } else { b[b[i].parent].orbit };
    for (i, x) in b.iter().enumerate() {
        let kep = if x.parent == INERTIAL {
            TAU * (x.orbit.powi(3) / (k * (m_sun + x.interior.mass))).sqrt()
        } else {
            TAU * (x.orbit.powi(3) / (k * (b[x.parent].interior.mass + x.interior.mass))).sqrt()
        };
        // a moon's pose turns 1/period + 1/YEAR (sky::circle): its sidereal period is that rate's inverse
        let sidereal = if x.parent == INERTIAL { x.period } else { 1.0 / (1.0 / x.period + 1.0 / YEAR) };
        println!("{:<9} {:>8.0} {:>9.4} {:>11.3} {:>9.3} {:>10.0} {:>10.0} {:>9.0} {:>9.3} {:>11.4e}", x.name, x.radius, x.interior.mass / b[0].interior.mass, x.interior.surface_g, x.interior.weight_g, x.orbit, sidereal, kep, sidereal / kep, sun_dist(i));
    }
    println!();
    println!("Equilibrium tides: h = (M_k / M) (R / d)^3 R x (g_own / g_felt); range 1.5 h (raiser in the equator plane, at the equator).");
    println!("{:<9} {:<9} {:>12} {:>10} {:>12} {:>12} {:>14}  {}", "body", "raiser", "M_k / M", "d / R", "range m", "range m wg", "highs every s", "note");
    for (i, x) in b.iter().enumerate() {
        if x.gas {
            continue;
        }
        let (r, m, g) = (x.radius, x.interior.mass, x.interior.surface_g);
        let mut rows: Vec<(String, f64, f64, f64, &str)> = Vec::new();
        // inertial turn rate of the body's frame, turns a second (sky.rs: the home, a circling moon, a planet that
        // does not turn without a `day`; Mars has none)
        let turn = if i == 0 { 1.0 / DAY + 1.0 / YEAR } else if x.parent != INERTIAL { 1.0 / x.period + 1.0 / YEAR } else { 0.0 };
        // the sun: its direction from the body turns once the body's (or its parent's) year
        let year = if x.parent == INERTIAL { x.period } else { b[x.parent].period };
        let sun_rate = (turn - 1.0 / year).abs();
        rows.push(("sun".into(), m_sun / m, sun_dist(i) / r, 0.5 / sun_rate, "Kepler mass"));
        for (j, y) in b.iter().enumerate() {
            if j == i {
                continue;
            }
            if x.parent == j {
                rows.push((y.name.clone(), y.interior.mass / m, x.orbit / r, f64::INFINITY, "parent: locked, fixed bulge"));
            } else if y.parent == i {
                let rate = (turn - (1.0 / y.period + 1.0 / YEAR)).abs();
                rows.push((y.name.clone(), y.interior.mass / m, y.orbit / r, 0.5 / rate, "moon"));
            } else {
                // any other body at its closest (both at their sun distances, side by side)
                let d = (sun_dist(i) - sun_dist(j)).abs().max(1.0);
                rows.push((y.name.clone(), y.interior.mass / m, d / r, f64::NAN, "closest approach"));
            }
        }
        for (who, q, dr, every, note) in rows {
            let h = q / dr.powi(3) * r;
            // the rise is the potential over the pull the water feels: phys uses surface_g (frames.rs), which is
            // k M / R^2, so h needs no correction; weight_g (rock) only differs on Luna
            let wg = h * g / x.interior.weight_g;
            println!("{:<9} {:<9} {:>12.4e} {:>10.1} {:>12.4e} {:>12.4e} {:>14.0}  {}", x.name, who, q, dr, 1.5 * h, 1.5 * wg, every, note);
        }
    }
    // today's tide against the derived one, and what Luna's row would need for today's 1 m springs
    let (r, ml, mt) = (b[0].radius, b[1].interior.mass, b[0].interior.mass);
    let h_l = ml / mt * (r / b[1].orbit).powi(3) * r;
    let h_s = m_sun / mt * (r / b[0].orbit).powi(3) * r;
    println!();
    println!("Terra: today TIDE_LUNA 0.5 m (range 0.75 m), sun 1/3 (0.25 m), springs 1 m. Derived: Luna {:.3} mm, sun {:.3} mm, springs {:.3} mm (sun/Luna {:.2}).", 1.5e3 * h_l, 1.5e3 * h_s, 1.5e3 * (h_l + h_s), h_s / h_l);
    let want = 0.75 / 1.5;
    let rho = |i: usize| b[i].interior.mass / b[i].radius.powi(3);
    let roche = 2.44 * (rho(0) / rho(1)).cbrt();
    println!("For Luna's 0.75 m range: her orbit at {:.2} Terra radii (60 now; the fluid Roche limit {roche:.2}), or her mass x {:.0}.", b[1].orbit / r * (h_l / want).powf(1.0 / 3.0), want / h_l);
    let _ = MONTH;
}
